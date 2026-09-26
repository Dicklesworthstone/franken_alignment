//! Run the actual monitored sampler inside the original effect-controller owner.
//! Numerical output is still evidence; the original congress and permits decide effects.

mod checkpoint;
mod automatic_stop;
pub mod sidecar;
pub use automatic_stop::{HostedStopCause, HostedStopIncident, HostedStopPolicy};
pub use checkpoint::{HostedCheckpointHandle, HostedRecoveryUsage, HostedResetReceipt, HostedResetRequest};

use super::{OversightBroker, decoder_gate::DecoderBindingLimits};
use crate::action::consequence::activation::identity::{ModelPassport, decoder::DecoderIdentityProbe};
use crate::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus, MonitoringWork};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder, MonitoredSampledStep};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    self, GenerationOwner, GenerationReport, GenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderProfile, DecoderWork};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::action::consequence::gate::containment::{ActorState, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES};
use crate::action::consequence::activation::{CaptureProfile, SourceFrame};
use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedDecoderInspection {
    pub actor_revision: u64,
    pub position: u64,
    pub sampled_draws: u64,
    pub status: MonitoringStatus,
    pub monitoring: MonitoringWork,
    pub numerical: DecoderWork,
    pub cache_bytes: usize,
    pub sampler_bytes: usize,
}

#[derive(Debug)]
pub(super) struct DecoderHost {
    run: MonitoredSampledDecoder,
    profile: RestartProfile,
    recovery: checkpoint::RecoveryState,
    automatic_stop: Option<automatic_stop::HostedStopState>,
}

impl OversightBroker {
    pub(super) fn bind_hosted_forecast_source(&mut self, layer: u64, profile: CaptureProfile,
        dimensions: usize, stream: u64) -> Result<(), Error>
    {
        self.decoder_host.as_mut().ok_or(Error::Incomplete)?.run
            .bind_forecast_residual(layer, profile, dimensions, stream)
    }
    pub(super) fn hosted_forecast_source(&self, layer: u64) -> Result<SourceFrame, Error> {
        self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.current_forecast_residual(layer)
    }

    /// Consume the only numerical owner at trusted bootstrap. Actor state is
    /// captured through the existing all-layer cache codec and sampler encoding.
    /// This can attach an empty or already fully reviewed prefix, never a held
    /// owner. External ActorState replacement/reset cannot bypass this profile.
    /// There is no mutable decoder getter, reseeding API, or ownership extraction.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::oversight::OversightBroker;
    /// fn bypass(broker: &mut OversightBroker) { broker.hosted_decoder_mut(); }
    /// ```
    pub fn own_sampled_decoder(&mut self, run: MonitoredSampledDecoder, limits: DecoderBindingLimits) -> Result<(), Error> {
        if self.decoder_host.is_some() || self.decoder.is_some() { return Err(Error::Duplicate); }
        if self.inspect().suspended || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 { return Err(Error::WrongState); }
        if run.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        let revision = self.actor_revision();
        revision.checked_add(1).ok_or(Error::Overflow)?;
        let profile = self.delivery.controller().actor().profile();
        let captured = run.capture_host_state(MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)?;
        let actor = ActorState::new(profile, captured.tokens, captured.cache, captured.sampler, captured.position)?;
        // Every remaining actor-update predicate has been preflighted. The
        // source bootstrap performs all identity/limit checks before mutation.
        self.enable_decoder_monitoring(run.observation(), limits)?;
        self.delivery.replace_actor_state(revision, actor).expect("preflighted owned actor update");
        self.decoder_host = Some(DecoderHost { run, profile, recovery: checkpoint::RecoveryState::new(), automatic_stop: None });
        Ok(())
    }

    /// Supervisor-only costs/state, without token IDs, logits, cache or RNG words.
    /// Numerical work includes completed abandoned runs and checkpoint replays.
    pub fn hosted_decoder(&self) -> Result<HostedDecoderInspection, Error> {
        let host = self.decoder_host.as_ref().ok_or(Error::Incomplete)?;
        let actor = self.delivery.controller().actor();
        Ok(HostedDecoderInspection { actor_revision: self.actor_revision(), position: host.run.position(),
            sampled_draws: host.run.sampled_draws(), status: host.run.status(),
            monitoring: host.run.monitoring_work(), numerical: host.recovery.cumulative(host.run.decoder_work())?,
            cache_bytes: actor.cache().len(), sampler_bytes: actor.sampler().len() })
    }

    /// Private comparison-only material for the durable owner. Even held state
    /// is included, but no public decoder/actor port can import or release it.
    pub(crate) fn hosted_replay_bytes(&self) -> Result<Vec<u8>, Error> {
        self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.replay_bytes()
    }

    /// Probe THIS owner's actual immutable parameters, not a caller-selected
    /// stand-in model. No live tokens, cache, logits, sampler or mutable model
    /// getter escapes. The returned observation-only run has separate work costs;
    /// advancing it neither charges the live decoder nor alters its monitor.
    /// Consumers still bind a current native/durable identity challenge and clock.
    pub fn hosted_identity_probe(&self, expected_actor_revision: u64, passport: &ModelPassport,
        measurement_sequence: u64, budget: DecoderBudget) -> Result<DecoderIdentityProbe, Error>
    {
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run
            .identity_probe(passport, measurement_sequence, budget)
    }

    pub fn advance_hosted_forced(&mut self, expected_actor_revision: u64, expected_position: u64,
        token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error>
    {
        self.with_hosted_stop(|owner| owner.advance_hosted(expected_actor_revision, expected_position,
            |run| run.advance_forced(expected_position, token, budget)))
    }

    pub fn advance_hosted_sampled(&mut self, expected_actor_revision: u64, expected_position: u64,
        budget: SampleBudget) -> Result<MonitoredSampledStep, Error>
    {
        self.with_hosted_stop(|owner| owner.advance_hosted(expected_actor_revision, expected_position,
            |run| run.advance_sampled(expected_position, budget)))
    }

    /// Execute a complete bounded generation request inside THIS enforcement
    /// owner. Every prompt/sample step uses the original hosted entry point,
    /// including consistency checks, actor/cache/sampler synchronization and the
    /// configured automatic-stop policy. There is no raw numerical fast path.
    ///
    /// The full prospective actor-state size, revision range and prompt numerical
    /// budget are preflighted. Sampling/product budgets belong to the whole
    /// invocation; the monitor's cumulative lifetime budget is neither reset nor
    /// enlarged. Generation is not a transaction: a computed prefix, held token
    /// or RNG draw is retained when a later step fails. The report identifies
    /// partial work; predictable numerical underfunding cannot truncate a prompt.
    ///
    /// Successful output is supervisor-side numerical evidence, NOT a Permit.
    /// Further inference invalidates old proposal evidence through the existing
    /// live decoder/actor revision checks. No congress, two-key, policy, delivery
    /// or endpoint check is bypassed and no external publication occurs here.
    /// This in-memory entry point provides no crash-recovery guarantee.
    pub fn generate_hosted(&mut self, expected_actor_revision: u64, expected_position: u64,
        request: GenerationRequest) -> Result<GenerationReport, Error>
    {
        self.prepare_hosted_generation(expected_actor_revision, expected_position, &request)?;
        generation::drive(self, expected_position, request)
    }

    // The durable adapter uses the SAME preflight before recording per-token
    // comparisons through the original driver. No alternate size/cost formula.
    pub(crate) fn prepare_hosted_generation(&mut self, expected_actor_revision: u64,
        expected_position: u64, request: &GenerationRequest) -> Result<(), Error>
    {
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        // Service already-observed containment before attempting new inference.
        if self.enforce_hosted_stop()?.is_some() || self.enforce_consistency_stop()?.is_some() {
            return Err(Error::WrongState);
        }
        if self.inspect().suspended { return Err(Error::WrongState); }
        let host = self.decoder_host.as_ref().ok_or(Error::Incomplete)?;
        if expected_position != host.run.position() { return Err(Error::Stale); }
        let count = request.prompt.len().checked_add(request.max_new_tokens).ok_or(Error::Overflow)?;
        if count > generation::MAX_GENERATION_TOKENS { return Err(Error::Limit); }
        let count = u64::try_from(count).map_err(|_| Error::Limit)?;
        expected_actor_revision.checked_add(count).ok_or(Error::Overflow)?;
        let end = expected_position.checked_add(count).ok_or(Error::Overflow)?;
        host.run.check_host_state_size(end, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)
    }

    // Only the two single-step methods above can supply this callback. The
    // bounded generation driver invokes those methods, never this seam directly.
    // No caller code runs between computation, capture and the original update.
    fn advance_hosted<T>(&mut self, expected_revision: u64, position: u64,
        advance: impl FnOnce(&mut MonitoredSampledDecoder) -> Result<T, Error>) -> Result<T, Error>
    {
        // An observed consistency incident also stops inference. A failed native
        // stop is retried before computation, never treated as a quiet source.
        if self.enforce_consistency_stop()?.is_some() { return Err(Error::WrongState); }
        if self.inspect().suspended { return Err(Error::WrongState); }
        if expected_revision != self.actor_revision() { return Err(Error::Stale); }
        expected_revision.checked_add(1).ok_or(Error::Overflow)?;
        let host = self.decoder_host.as_mut().ok_or(Error::Incomplete)?;
        if position != host.run.position() { return Err(Error::Stale); }
        host.run.check_host_state_size(position.checked_add(1).ok_or(Error::Overflow)?, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)?;
        let result = advance(&mut host.run)?;
        // A computed-but-held step also advances actual state and sampled draws.
        // Its source remains held and neither the token nor state bytes escape.
        let state = host.run.capture_host_state(MAX_CACHE_BYTES, MAX_SAMPLER_BYTES).and_then(|captured| {
            ActorState::new(host.profile, captured.tokens, captured.cache, captured.sampler, captured.position)
        });
        let state = match state {
            Ok(state) => state,
            Err(error) => { host.run.fail_host(error); return Err(error); }
        };
        if let Err(error) = self.delivery.replace_actor_state(expected_revision, state) {
            host.run.fail_host(error);
            return Err(error);
        }
        Ok(result)
    }
}

// This implementation is crate-private; external callers cannot provide a
// fabricated reviewed-step producer to the generation loop. All transitions
// still pass through the original owning authority's public hosted methods.
impl GenerationOwner for OversightBroker {
    fn generation_profile(&self) -> Result<&DecoderProfile, Error> {
        Ok(self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.profile())
    }
    fn generation_position(&self) -> Result<u64, Error> {
        Ok(self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.position())
    }
    fn generation_status(&self) -> Result<MonitoringStatus, Error> {
        if self.inspect().suspended { return Err(Error::WrongState); }
        Ok(self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.status())
    }
    fn generation_estimate(&self, tokens: usize) -> Result<DecoderWork, Error> {
        self.decoder_host.as_ref().ok_or(Error::Incomplete)?.run.estimate(tokens)
    }
    fn generation_forced(&mut self, position: u64, token: u32,
        budget: DecoderBudget) -> Result<MonitoredStep, Error>
    {
        self.advance_hosted_forced(self.actor_revision(), position, token, budget)
    }
    fn generation_sampled(&mut self, position: u64,
        budget: SampleBudget) -> Result<MonitoredStep, Error>
    {
        self.advance_hosted_sampled(self.actor_revision(), position, budget)
            .map(MonitoredSampledStep::into_monitored)
    }
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    use crate::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
    use crate::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget, RefinementMonitor};
    use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
        GenerationBudget, GenerationFinish, MAX_SAMPLING_ENTRIES,
    };
    use crate::action::consequence::activation::probe::LinearProbe;
    use crate::action::consequence::activation::tensor::kv::decoder::{
        DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderShape, MAX_DECODER_PRODUCTS,
    };
    use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
    use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use crate::action::consequence::delivery::{PublicationEndpoint, StopRequest};
    use crate::action::consequence::gate::TargetCeiling;
    use crate::action::consequence::gate::containment::RestartGrade;
    use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
    use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
    use crate::full_input::InputProfileBinding;
    use crate::reducer::Caps;
    use crate::Snapshot;
    use std::collections::BTreeMap;

    fn numerical(threshold: f32) -> MonitoredSampledDecoder {
        let profile = DecoderProfile::new(DecoderIdentity {
            tenant: 1, model: 2, model_generation: 3, tokenizer_generation: 4, profile_generation: 5,
        }, DecoderShape { vocabulary: 2, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
        // Prompt 0 has residual [1,0]; its readout selects token 1 under top-k=1.
        // Token 1 has residual [0,1], so the same probe can causally hold it.
        let model = DecoderModel::new(profile, vec![1.0, 0.0, 0.0, 1.0], vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
            up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0, 0.0, 2.0, 0.0]).unwrap();
        let allowance = RefinementBudget { encoded_bytes: 10000, probe_coordinates: 10000 };
        let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
            &[0.0, 1.0], 0.0, threshold).unwrap();
        MonitoredSampledDecoder::new(model, 7, 11,
            BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]),
            allowance, SamplingStart { policy: SamplingPolicy::new(1, 1, 2, 1.0, 1, 1.0).unwrap(),
                stream: 9, seed: 0 }).unwrap()
    }

    fn scope() -> Scope {
        Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
    }
    fn target() -> ResolvedTarget {
        ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
    }
    fn owner(threshold: f32) -> (OversightBroker, PublicationEndpoint) {
        let actor = ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 3, tokenizer_generation: 4,
            state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, Vec::new(), vec![0], vec![0], 0).unwrap();
        let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
                tokenizer_epoch: 0, policy_epoch: 0, model_epoch: 0 }, 1, b"approve?".to_vec()).unwrap(),
        )])).unwrap();
        let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
        let mut owner = OversightBroker::new(ControllerConfig {
            scope: scope(), total: 100, max_attempts: 8, actor, suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy {
                    cohort: "reference".to_owned(), weight: 1,
                })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
            },
            narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
        }, &mut endpoint, contracts).unwrap();
        owner.observe_time(ElapsedTick(1)).unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
        owner.own_sampled_decoder(numerical(threshold), DecoderBindingLimits::default()).unwrap();
        (owner, endpoint)
    }
    fn request(prompt: &[u32], max_new_tokens: usize) -> GenerationRequest {
        GenerationRequest { prompt: prompt.to_vec(), max_new_tokens, stop_tokens: Vec::new(),
            budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS,
                sampling_entries: MAX_SAMPLING_ENTRIES } }
    }
    fn snapshot() -> Snapshot {
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
    }
    fn propose(owner: &mut OversightBroker, id: u64) {
        owner.propose(id, ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
            payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: owner.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16,
        }, &snapshot()).unwrap();
    }

    #[test]
    fn hosted_completion_updates_real_actor_state_and_evidence_without_granting_authority() {
        let (mut owner, endpoint) = owner(100.0);
        let before = owner.hosted_decoder().unwrap();
        let ledger = owner.inspect();
        let report = owner.generate_hosted(before.actor_revision, 0, request(&[0], 2)).unwrap();
        assert_eq!(report.finish(), GenerationFinish::TokenLimit);
        assert_eq!(report.tokens().len(), 2);
        assert_eq!(report.tokens()[0], 1);
        let after = owner.hosted_decoder().unwrap();
        assert_eq!(after.position, 3);
        assert_eq!(after.actor_revision, before.actor_revision + 3);
        assert_eq!(after.sampled_draws, 2);
        assert_eq!(after.numerical.tokens, 3);
        assert_eq!(owner.inspect(), ledger);
        let mut expected = vec![0]; expected.extend_from_slice(report.tokens());
        assert_eq!(owner.delivery.controller().actor().tokens(), expected.as_slice());
        propose(&mut owner, 1);
        assert_eq!(owner.decoder_evidence(1).unwrap().unwrap().tokens(), expected.as_slice());
        owner.check_decoder(1).unwrap();
        // A complete quiet generation still cannot manufacture congress approval.
        assert!(matches!(owner.authorize(1, None, &snapshot()), Err(Error::Incomplete)));
        assert_eq!(endpoint.execution_count(), 0);
        assert_eq!(endpoint.payload(), b"initial");
    }

    #[test]
    fn later_generation_invalidates_old_proposal_basis_but_new_proposals_bind_actual_state() {
        let (mut owner, _) = owner(100.0);
        owner.generate_hosted(owner.actor_revision(), 0, request(&[0], 0)).unwrap();
        propose(&mut owner, 1);
        owner.check_decoder(1).unwrap();
        let old = owner.decoder_evidence(1).unwrap().unwrap().tokens().to_vec();
        let report = owner.generate_hosted(owner.actor_revision(), 1, request(&[], 1)).unwrap();
        assert_eq!(report.tokens().len(), 1);
        assert!(owner.check_decoder(1).is_err());
        assert_eq!(owner.decoder_evidence(1).unwrap().unwrap().tokens(), old.as_slice());
        propose(&mut owner, 2);
        owner.check_decoder(2).unwrap();
        assert_eq!(owner.decoder_evidence(2).unwrap().unwrap().tokens().len(), 2);
    }

    #[test]
    fn sampled_alarm_runs_original_containment_and_withholds_the_token() {
        let (mut owner, endpoint) = owner(0.5);
        owner.enable_hosted_stop(HostedStopPolicy::new(1, 1, 900).unwrap()).unwrap();
        let prefix = owner.generate_hosted(owner.actor_revision(), 0, request(&[0], 0)).unwrap();
        assert_eq!(prefix.reviewed_prompt_tokens(), 1);
        assert_eq!(prefix.finish(), GenerationFinish::TokenLimit);
        propose(&mut owner, 1);
        owner.check_decoder(1).unwrap();
        let report = owner.generate_hosted(owner.actor_revision(), 1, request(&[], 2)).unwrap();
        assert_eq!(report.finish(), GenerationFinish::Held);
        assert!(report.tokens().is_empty());
        assert_eq!(report.end_position(), 2);
        assert_eq!(report.work().attempted_samples, 1);
        let actual = owner.hosted_decoder().unwrap();
        assert_eq!(actual.sampled_draws, 1);
        assert_eq!(actual.status, MonitoringStatus::Held);
        assert!(owner.inspect().suspended);
        assert_eq!(owner.inspect().ledger.stages[&1], ActionState::Cancelled);
        let incident = owner.hosted_stop_incident().unwrap();
        assert_eq!(incident.cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
        assert!(incident.stop_receipt().is_some());
        assert_eq!(owner.generate_hosted(owner.actor_revision(), 2, request(&[], 1)).unwrap_err(), Error::WrongState);
        assert_eq!(owner.hosted_decoder().unwrap().sampled_draws, 1);
        assert_eq!(endpoint.execution_count(), 0);
        assert_eq!(owner.inspect().ledger.available, 100);
    }

    #[test]
    fn stop_token_stays_in_actual_decoder_evidence_even_when_suppressed_from_output() {
        let (mut owner, _) = owner(100.0);
        let mut input = request(&[0], 3); input.stop_tokens = vec![1];
        let report = owner.generate_hosted(owner.actor_revision(), 0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::StopToken);
        assert!(report.tokens().is_empty());
        assert_eq!(owner.hosted_decoder().unwrap().position, 2);
        propose(&mut owner, 1);
        assert_eq!(owner.decoder_evidence(1).unwrap().unwrap().tokens(), &[0, 1]);
        owner.check_decoder(1).unwrap();
    }

    #[test]
    fn malformed_or_stale_generation_does_not_partially_rewrite_owned_actor_state() {
        let (mut owner, _) = owner(100.0);
        let before = owner.hosted_decoder().unwrap();
        let actor = owner.delivery.controller().actor().clone();
        for (revision, position, input) in [
            (before.actor_revision + 1, 0, request(&[0], 1)),
            (before.actor_revision, 1, request(&[0], 1)),
            (before.actor_revision, 0, request(&[0, 2], 1)),
            (before.actor_revision, 0, request(&[0], 4)),
        ] {
            assert!(owner.generate_hosted(revision, position, input).is_err());
            assert_eq!(owner.hosted_decoder().unwrap(), before);
            assert_eq!(owner.delivery.controller().actor(), &actor);
        }
        assert_eq!(owner.generate_hosted(before.actor_revision, 0, request(&[0], 1)).unwrap().tokens().len(), 1);
    }

    #[test]
    fn request_budget_exhaustion_preserves_quiet_prefix_and_does_not_invent_a_monitor_alarm() {
        let (mut owner, _) = owner(100.0);
        owner.enable_hosted_stop(HostedStopPolicy::new(1, 1, 900).unwrap()).unwrap();
        let revision = owner.actor_revision();
        let mut input = request(&[0], 2);
        input.budget.scalar_products = owner.decoder_host.as_ref().unwrap().run.estimate(1).unwrap().scalar_products().unwrap();
        let report = owner.generate_hosted(revision, 0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
        assert_eq!(report.reviewed_prompt_tokens(), 1);
        assert_eq!(owner.actor_revision(), revision + 1);
        assert_eq!(owner.hosted_decoder().unwrap().sampled_draws, 0);
        assert!(owner.hosted_stop_incident().is_none());
        assert!(!owner.inspect().suspended);
        propose(&mut owner, 1);
        owner.check_decoder(1).unwrap();
    }

    #[test]
    fn manual_suspension_blocks_generation_without_any_more_computation() {
        let (mut owner, _) = owner(100.0);
        let inspection = owner.inspect();
        owner.request_stop(StopRequest { operation: 901, expected_control_sequence: inspection.sequence,
            expected_authority_epoch: inspection.ledger.epoch }).unwrap();
        let before = owner.hosted_decoder().unwrap();
        assert_eq!(owner.generate_hosted(before.actor_revision, before.position, request(&[0], 1)).unwrap_err(), Error::WrongState);
        assert_eq!(owner.hosted_decoder().unwrap(), before);
    }

    #[test]
    fn underfunded_complete_prompt_cannot_publish_a_new_partial_actor_basis() {
        let (mut owner, _) = owner(100.0);
        let before = owner.hosted_decoder().unwrap();
        let actor = owner.delivery.controller().actor().clone();
        let mut input = request(&[0, 1], 1);
        input.budget.scalar_products = 75;
        assert_eq!(owner.generate_hosted(before.actor_revision, 0, input).unwrap_err(), Error::Limit);
        assert_eq!(owner.hosted_decoder().unwrap(), before);
        assert_eq!(owner.delivery.controller().actor(), &actor);
        assert!(owner.prepare_decoder().is_err());
        let mut input = request(&[0, 1], 1);
        input.budget.scalar_products = 76;
        let report = owner.generate_hosted(before.actor_revision, 0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
        assert_eq!(report.reviewed_prompt_tokens(), 2);
        assert_eq!(owner.actor_revision(), before.actor_revision + 2);
        propose(&mut owner, 1);
        assert_eq!(owner.decoder_evidence(1).unwrap().unwrap().tokens(), &[0, 1]);
        owner.check_decoder(1).unwrap();
    }
}
