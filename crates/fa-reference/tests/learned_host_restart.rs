//! Original learned inference, typed KV restore and original effect containment.
//! Tiny weights and supplied ballots are controls, not restart qualification.
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod text_fixture;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose,
    ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{PublicationEndpoint, stream::StreamProfile};
use fa_reference::action::consequence::gate::{TargetCeiling, ReviewBinding};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow,
    action_frame, decoder_host::HostedStopPolicy, decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy},
    learned_host::{checkpoint::{HostedLearnedCheckpointHandle, HostedLearnedResetRequest},
        sidecar::{LearnedSidecar, LearnedSidecarRequest}, text::LearnedTextTarget},
    learned_source::{LearnedAvailability, LearnedEvidenceLimits, LearnedSourceConfig},
    sidecar::{SidecarCongressBudget, SidecarIdentity},
};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderModel, monitoring::restart::KvRestartBudget,
        sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationEvent,
            GenerationSpec, GenerationTelemetryBudget, GenerationStatus, LearnedGeneration}}},
    model::MAX_MODEL_KV_VALUES,
};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn empty_owner(human: bool, stream: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>) {
    // A caller-declared reference grade, not an empirical deployment claim.
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap();
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = if stream {
        PublicationEndpoint::new_stream(target(), StreamProfile::new(1, 1, 4, 128, 512).unwrap(), 1000, 8).unwrap()
    } else { PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap() };
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8, actor, suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contract).unwrap();
    let reviewer = if human { Some(owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8,
    }).unwrap()) } else { None };
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, reviewer)
}
fn config(model: &DecoderModel, mode: u8, top_k: usize) -> LearnedSourceConfig {
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}
fn attach(owner: &mut OversightBroker, model: &DecoderModel, config: LearnedSourceConfig) {
    owner.own_learned_generation(model.clone(), config, LearnedDecoderBindingLimits::default()).unwrap();
}
fn step(owner: &mut OversightBroker) -> Result<Rc<GenerationEvent>, Error> {
    let current = owner.hosted_learned_generation()?;
    owner.advance_hosted_learned(current.actor_revision, current.position)
}
fn control(model: &DecoderModel, config: LearnedSourceConfig) -> LearnedGeneration {
    model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
        config.spec, config.policy, config.budget, config.telemetry).unwrap()
}
fn capture(owner: &mut OversightBroker, id: u64) -> HostedLearnedCheckpointHandle {
    owner.capture_hosted_learned_checkpoint(id, owner.actor_revision()).unwrap()
}
fn request(owner: &OversightBroker, checkpoint: &HostedLearnedCheckpointHandle,
    config: &LearnedSourceConfig, round: u64) -> HostedLearnedResetRequest
{
    let state = owner.inspect();
    HostedLearnedResetRequest { checkpoint: checkpoint.clone(), expected_control_sequence: state.sequence,
        expected_actor_revision: owner.actor_revision(), expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[target()]).unwrap(),
        restart_budget: KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: config.policy.allowance() } }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(owner: &OversightBroker) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn propose(owner: &mut OversightBroker, id: u64) -> FrozenAction {
    owner.propose(id, spec(owner), &snapshot()).unwrap().action
}
fn text_input(owner: &mut OversightBroker, id: u64, action: &FrozenAction) -> CommitteeInput {
    let helper = &owner.contracts().members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection { projection_id: helper.projection_id(),
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(action, owner.contracts(), BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    owner.record_inputs(id, owner.input_revision(id).unwrap(), inputs.clone()).unwrap();
    inputs
}
fn sidecar(owner: &mut OversightBroker, attempt: u64) -> LearnedSidecar {
    owner.begin_learned_sidecar(attempt, owner.actor_revision(), LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1000 + attempt, generation: 1, transform_id: 7 },
        priority: Vec::new(), budget: SidecarCongressBudget::default(),
    }).unwrap()
}
fn review(owner: &mut OversightBroker, id: u64) -> ObservedReview {
    let mut session = owner.begin_review(id, id + 100, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let salt = [11; 32];
    let commitment = session.commitment("reviewer", Verdict::Allow, &salt).unwrap();
    session.commit("reviewer", commitment, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", Verdict::Allow, &salt, ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}
fn approve(owner: &mut OversightBroker, id: u64, input: &CommitteeInput) {
    let reviewed = review(owner, id);
    owner.apply_review(reviewed, Some(input), &snapshot()).unwrap();
}
fn same_step(actual: &GenerationEvent, expected: &GenerationEvent) {
    assert_eq!(actual.sample(), expected.sample());
    if let (Some(a), Some(b)) = (actual.sample(), expected.sample()) {
        assert_eq!(a.probability.to_bits(), b.probability.to_bits());
    }
    assert_eq!(fixture::logits(&actual.accepted().unwrap().logits),
        fixture::logits(&expected.accepted().unwrap().logits));
}

#[test]
fn exact_continuation_needs_fresh_source_sidecar_and_both_keys_after_original_reset() {
    let model = fixture::model(); let config = config(&model, 0, 3);
    let mut control = control(&model, config.clone());
    let (mut owner, mut endpoint, human) = empty_owner(true, false);
    attach(&mut owner, &model, config.clone()); owner.enable_learned_sidecar_requirement().unwrap();
    same_step(&step(&mut owner).unwrap(), &control.advance(0).unwrap());
    let checkpoint = capture(&mut owner, 1);
    let saved = owner.hosted_learned_generation().unwrap();
    let source = owner.hosted_learned_observation().unwrap();
    let old_evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let action = propose(&mut owner, 1); let prior_sidecar = sidecar(&mut owner, 1);
    let prior_input = prior_sidecar.round().input().clone(); approve(&mut owner, 1, &prior_input);
    let old_permit = owner.authorize(1, Some(&prior_input), &snapshot()).unwrap();
    let human = human.unwrap();
    let old_request = owner.request_human_approval(10, 1, Some(&prior_input), ElapsedTick(80)).unwrap();
    let old_key = human.approve(&old_request, ElapsedTick(1)).unwrap();
    let expected = control.advance(1).unwrap(); same_step(&step(&mut owner).unwrap(), &expected);
    let retained = owner.decoder_binding_usage(); let before = owner.hosted_learned_generation().unwrap();
    let reset = owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.control.cancelled, vec![1]);
    assert_eq!(reset.control.refunded_units, 16); assert_eq!(reset.control.incident_count, 1);
    assert_eq!(reset.resumed_stream, Some(22)); assert_eq!(reset.actor_revision, before.actor_revision + 2);
    assert_eq!(reset.position, 1); assert_eq!(reset.sampled_draws, 0);
    assert_eq!(reset.restart.historical_work(), saved.work);
    assert_eq!(reset.restart.historical_telemetry(), saved.telemetry);
    let restored = owner.hosted_learned_generation().unwrap();
    assert_eq!(restored.work, saved.work); assert_eq!(restored.sampled_draws, saved.sampled_draws);
    assert_eq!(restored.cumulative_work, before.cumulative_work);
    assert!(restored.cumulative_telemetry.source_check_values > before.cumulative_telemetry.source_check_values);
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap().restart_attempts, 1);
    let audit = reset.restart.kv().audit().unwrap().monitoring();
    assert!(audit.complete_quiet());
    assert!(audit.source().descriptor().layers().values().all(|layer| layer.stream == 21));
    assert_eq!(source.availability(), LearnedAvailability::Closed);
    assert!(source.validate(&old_evidence).is_err());
    assert_eq!(owner.decoder_binding_usage(), retained);
    assert!(owner.learned_sidecar_required());
    assert!(owner.current_learned_sidecar(&prior_sidecar).is_err());
    assert!(owner.dispatch_with_human(&old_permit, &old_key, &action, Some(&prior_input), &snapshot()).is_err());
    let resumed = owner.hosted_learned_observation().unwrap();
    assert_eq!(resumed.stream(), 22); assert_eq!(resumed.availability(), LearnedAvailability::Empty);
    assert_eq!(resumed.capture(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
    let premature = spec(&owner);
    assert_eq!(owner.propose(2, premature, &snapshot()).err(), Some(Error::Incomplete));
    same_step(&step(&mut owner).unwrap(), &expected);
    let fresh = resumed.capture(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(fresh.tokens(), control.accepted_tokens()); assert_eq!(fresh.audit().first_position(), 1);
    assert!(fresh.audit().source().descriptor().layers().values().all(|layer| layer.stream == 22));
    let action = propose(&mut owner, 2); let fresh_sidecar = sidecar(&mut owner, 2);
    let input = fresh_sidecar.round().input().clone(); approve(&mut owner, 2, &input);
    let permit = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    assert!(owner.dispatch_with_human(&permit, &old_key, &action, Some(&input), &snapshot()).is_err());
    let human_request = owner.request_human_approval(11, 2, Some(&input), ElapsedTick(80)).unwrap();
    let key = human.approve(&human_request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
}

#[test]
fn repeated_rewinds_cannot_refund_sampler_or_source_check_allowances() {
    for telemetry in [false, true] {
        let model = fixture::model(); let mut config = config(&model, 0, 1);
        if telemetry {
            config.telemetry.source_check_values = model.cache_profile().values_per_token() as u64 * 3;
        } else { config.budget.vocabulary_scores = 9; }
        let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
        step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
        for round in [900, 901] {
            step(&mut owner).unwrap();
            let reset = owner.reset_hosted_learned(request(&owner, &checkpoint, &config, round)).unwrap();
            assert!(reset.control.restored); assert_eq!(reset.position, 1); assert_eq!(reset.sampled_draws, 0);
        }
        if !telemetry { step(&mut owner).unwrap(); }
        assert_eq!(owner.hosted_learned_recovery_usage().unwrap().restart_attempts, 2);
        let before = owner.hosted_learned_generation().unwrap();
        assert_eq!(step(&mut owner).err(), Some(Error::Limit));
        let after = owner.hosted_learned_generation().unwrap();
        assert_eq!(after.position, before.position); assert_eq!(after.sampled_draws, before.sampled_draws);
        assert!(matches!(after.status, GenerationStatus::Failed(Error::Limit)));
        assert!(owner.hosted_learned_observation().unwrap().capture(LearnedEvidenceLimits::default()).is_err());
        assert!(owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 902)).is_err());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn decoder_product_budget_counts_abandoned_postcheckpoint_inference() {
    let model = fixture::model(); let mut config = config(&model, 0, 1);
    config.budget.decoder_products = model.estimate(0, 4).unwrap().scalar_products().unwrap();
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1); step(&mut owner).unwrap();
    let mut spent = owner.hosted_learned_generation().unwrap().work.reserved_decoder_products;
    owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).unwrap();
    loop {
        let before = owner.hosted_learned_generation().unwrap();
        assert!(before.status.is_active(), "rewind cannot buy an entire second continuation");
        assert_eq!(before.cumulative_work.reserved_decoder_products, spent);
        let next = model.estimate(before.position as usize, 1).unwrap().scalar_products().unwrap();
        if spent + next > config.budget.decoder_products {
            assert_eq!(step(&mut owner).err(), Some(Error::Limit)); break;
        }
        step(&mut owner).unwrap(); spent += next;
    }
    assert_eq!(owner.hosted_learned_generation().unwrap().cumulative_work.reserved_decoder_products, spent);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn already_published_unknown_effect_and_human_key_stay_spent_across_reset() {
    let model = fixture::model(); let config = config(&model, 0, 1);
    let (mut owner, mut endpoint, reviewer) = empty_owner(true, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    let action = propose(&mut owner, 1); let input = text_input(&mut owner, 1, &action); approve(&mut owner, 1, &input);
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    let human_request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let key = reviewer.unwrap().approve(&human_request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap(); owner.acknowledgment_lost(1).unwrap();
    step(&mut owner).unwrap(); let before = owner.inspect();
    let reset = owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.control.refunded_units, 0);
    assert!(reset.control.cancelled.is_empty()); assert!(reset.control.revocation_floor > before.ledger.epoch);
    assert!(owner.inspect().ledger.epoch > before.ledger.epoch);
    assert_eq!(owner.inspect().ledger.charged, 16); assert_eq!(owner.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert!(owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).is_err());
    assert!(owner.accept_receipt(receipt.clone()).unwrap()); assert!(!owner.accept_receipt(receipt).unwrap());
    assert_eq!(owner.inspect().ledger.charged, 16); assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn real_incident_threshold_suspends_without_a_third_resumed_source() {
    let model = fixture::model(); let config = config(&model, 0, 1);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    for incident in 1..=3 {
        step(&mut owner).unwrap();
        let reset = owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900 + incident)).unwrap();
        assert_eq!(reset.control.incident_count, incident);
        assert_eq!(reset.control.restored, incident < 3); assert_eq!(reset.resumed_stream.is_some(), incident < 3);
    }
    assert!(owner.inspect().suspended); assert!(step(&mut owner).is_err());
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap().restart_attempts, 3);
    assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Failed);
    assert!(owner.capture_hosted_learned_checkpoint(2, owner.actor_revision()).is_err());
    assert!(owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 904)).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn foreign_handles_and_stale_preconditions_do_not_poison_the_original_live_owner() {
    let model = fixture::model(); let config = config(&model, 0, 1);
    let (mut owner, _, _) = empty_owner(false, false); let (mut foreign, _, _) = empty_owner(false, false);
    attach(&mut owner, &model, config.clone()); attach(&mut foreign, &model, config.clone());
    step(&mut owner).unwrap(); step(&mut foreign).unwrap();
    let checkpoint = capture(&mut owner, 1); let other = capture(&mut foreign, 1);
    let original = request(&owner, &checkpoint, &config, 900);
    let before = owner.hosted_learned_generation().unwrap(); let control = owner.inspect();
    let usage = owner.hosted_learned_recovery_usage().unwrap();
    for kind in 0..6 {
        let mut bad = original.clone();
        match kind {
            0 => bad.checkpoint = other.clone(), 1 => bad.expected_actor_revision += 1,
            2 => bad.expected_control_sequence += 1, 3 => bad.expected_authority_epoch += 1,
            4 => bad.restart_budget.cache_values = 0, _ => bad.binding.evidence_root = [0; 32],
        }
        assert!(owner.reset_hosted_learned(bad).is_err());
        assert_eq!(owner.hosted_learned_generation().unwrap(), before); assert_eq!(owner.inspect(), control);
        assert_eq!(owner.hosted_learned_recovery_usage().unwrap(), usage);
    }
    assert!(owner.reset_hosted_learned(original).unwrap().control.restored);
}

#[test]
fn failed_fresh_restart_audit_does_not_republish_a_prior_quiet_observation() {
    let model = fixture::model(); let config = config(&model, 0, 1);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    let source = owner.hosted_learned_observation().unwrap();
    let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let before = owner.inspect(); let usage = owner.hosted_learned_recovery_usage().unwrap();
    let mut bad = request(&owner, &checkpoint, &config, 900);
    bad.restart_budget.audit.monitoring.probe_coordinates = 0;
    assert!(owner.reset_hosted_learned(bad).is_err());
    assert_eq!(owner.inspect(), before);
    assert_ne!(owner.hosted_learned_recovery_usage().unwrap(), usage);
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap().restart_attempts, usage.restart_attempts + 1);
    assert_eq!(source.availability(), LearnedAvailability::Failed);
    assert!(source.validate(&evidence).is_err());
    let proposal = spec(&owner); assert!(owner.propose(1, proposal, &snapshot()).is_err());
    assert!(step(&mut owner).is_err()); assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn automatic_stop_cannot_be_cleared_by_restoring_a_pre_alarm_checkpoint() {
    let model = fixture::model(); let config = config(&model, 2, 1);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    owner.enable_learned_host_stop(HostedStopPolicy::new(1, 1, 700).unwrap()).unwrap();
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    let held = step(&mut owner).unwrap(); assert!(held.accepted().is_none());
    assert!(owner.learned_host_stop_incident().is_some()); assert!(owner.stop_receipt().is_some());
    let before = owner.inspect(); let usage = owner.hosted_learned_recovery_usage().unwrap();
    assert!(owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).is_err());
    assert_eq!(owner.inspect(), before); assert_eq!(owner.hosted_learned_recovery_usage().unwrap(), usage);
    assert!(step(&mut owner).is_err()); assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn a_held_owner_resets_but_repeated_original_alarm_still_cannot_publish() {
    let model = fixture::model(); let config = config(&model, 2, 1);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    assert!(step(&mut owner).unwrap().accepted().is_none());
    assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Held);
    let before = owner.hosted_learned_generation().unwrap();
    let reset = owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.position, 1);
    assert_eq!(owner.hosted_learned_generation().unwrap().cumulative_work, before.cumulative_work);
    assert!(step(&mut owner).unwrap().accepted().is_none());
    assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Held);
    let action = spec(&owner); assert!(owner.propose(1, action, &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn finished_owner_can_rewind_to_active_prefix_but_terminal_checkpoint_cannot_be_registered() {
    let model = fixture::model(); let config = config(&model, 0, 3);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    let expected = step(&mut owner).unwrap();
    while owner.hosted_learned_generation().unwrap().status.is_active() { step(&mut owner).unwrap(); }
    let before = owner.hosted_learned_generation().unwrap(); let usage = owner.hosted_learned_recovery_usage().unwrap();
    assert!(matches!(before.status, GenerationStatus::Finished(_)));
    assert_eq!(owner.capture_hosted_learned_checkpoint(2, owner.actor_revision()).err(), Some(Error::WrongState));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap(), usage);
    assert!(owner.reset_hosted_learned(request(&owner, &checkpoint, &config, 900)).unwrap().control.restored);
    same_step(&step(&mut owner).unwrap(), &expected);
    assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Ready);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn audience_stream_ownership_has_no_checkpoint_reset_escape() {
    let (mut owner, endpoint, _) = empty_owner(false, true);
    let model = text_fixture::model(&[b'O' as u32, b'K' as u32, text_fixture::END]);
    owner.own_learned_text_stream(model.clone(), text_fixture::tokenizer(&model), text_fixture::config(&model),
        LearnedDecoderBindingLimits::default()).unwrap();
    step(&mut owner).unwrap(); let before = owner.hosted_learned_generation().unwrap();
    assert!(owner.learned_text_stream_required());
    assert!(owner.capture_hosted_learned_checkpoint(1, owner.actor_revision()).is_err());
    assert_eq!(owner.hosted_learned_generation().unwrap(), before); assert_eq!(endpoint.execution_count(), 0);
    while owner.hosted_learned_generation().unwrap().status.is_active() { step(&mut owner).unwrap(); }
    assert_eq!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}

#[test]
fn plain_text_reset_preserves_tokenizer_completion_and_exact_payload_but_requires_both_fresh_keys() {
    let (mut owner, mut endpoint, human) = empty_owner(true, false);
    let human = human.unwrap();
    let model = text_fixture::model(&[b'O' as u32, b'K' as u32, text_fixture::END]);
    let config = text_fixture::config(&model);
    owner.own_learned_text_generation(model.clone(), text_fixture::tokenizer(&model), config.clone(),
        LearnedDecoderBindingLimits::default()).unwrap();
    step(&mut owner).unwrap(); let checkpoint = capture(&mut owner, 1);
    while owner.hosted_learned_generation().unwrap().status.is_active() { step(&mut owner).unwrap(); }
    let original = owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(original.bytes(), b"OK");
    assert_eq!(original.evidence().tokens()[0], text_fixture::MERGED_PROMPT);
    let destination = |owner: &OversightBroker| LearnedTextTarget { target: target(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 };
    let old_action = owner.propose_learned_text(1, destination(&owner), &snapshot()).unwrap().action;
    let old_input = text_input(&mut owner, 1, &old_action); approve(&mut owner, 1, &old_input);
    let old_permit = owner.authorize(1, Some(&old_input), &snapshot()).unwrap();
    let old_request = owner.request_human_approval(10, 1, Some(&old_input), ElapsedTick(80)).unwrap();
    let old_key = human.approve(&old_request, ElapsedTick(1)).unwrap();
    let source = owner.hosted_learned_observation().unwrap();
    let before = owner.hosted_learned_generation().unwrap(); let control = owner.inspect();
    let reset = owner.reset_hosted_learned(HostedLearnedResetRequest { checkpoint,
        expected_control_sequence: control.sequence, expected_actor_revision: owner.actor_revision(),
        expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[target()]).unwrap(),
        restart_budget: KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: config.policy.allowance() },
    }).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.control.refunded_units, 16);
    assert!(owner.learned_text_required()); assert!(!owner.learned_text_stream_required());
    assert_eq!(source.availability(), LearnedAvailability::Closed);
    assert!(source.validate(original.evidence()).is_err());
    assert_eq!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
    assert_eq!(owner.propose_learned_text(2, destination(&owner), &snapshot()).err(), Some(Error::Incomplete));
    while owner.hosted_learned_generation().unwrap().status.is_active() { step(&mut owner).unwrap(); }
    let fresh = owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(fresh.bytes(), original.bytes()); assert_eq!(fresh.evidence().tokens(), original.evidence().tokens());
    assert_eq!(fresh.stop(), original.stop()); assert_eq!(fresh.output_tokens(), 2);
    assert_eq!(fresh.output_policy(), config.output); assert_eq!(fresh.tokenization_work(), original.tokenization_work());
    assert!(fresh.tokenizer().binds(model.profile()));
    assert_eq!(fresh.evidence().stream(), 22); assert_eq!(fresh.evidence().audit().first_position(), 3);
    assert_eq!(fresh.work(), original.work());
    assert!(owner.hosted_learned_generation().unwrap().cumulative_work.admitted_tokens > before.cumulative_work.admitted_tokens);
    assert!(owner.dispatch_with_human(&old_permit, &old_key, &old_action, Some(&old_input), &snapshot()).is_err());
    let mut substituted = spec(&owner); substituted.payload = b"OK ".to_vec();
    assert_eq!(owner.propose(2, substituted, &snapshot()).err(), Some(Error::Binding));
    let action = owner.propose_learned_text(2, destination(&owner), &snapshot()).unwrap().action;
    assert_eq!(action.spec().payload, b"OK");
    let input = text_input(&mut owner, 2, &action); approve(&mut owner, 2, &input);
    let permit = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    assert!(owner.dispatch_with_human(&permit, &old_key, &action, Some(&input), &snapshot()).is_err());
    let human_request = owner.request_human_approval(11, 2, Some(&input), ElapsedTick(80)).unwrap();
    let key = human.approve(&human_request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"OK"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
}

#[test]
fn insufficient_original_full_prefix_row_capacity_refuses_before_spending_or_source_withdrawal() {
    let model = fixture::model(); let config = config(&model, 0, 3);
    assert_eq!(config.policy.monitor().budget().rows, 4);
    let (mut owner, endpoint, _) = empty_owner(false, false); attach(&mut owner, &model, config.clone());
    step(&mut owner).unwrap(); let fitting = capture(&mut owner, 1);
    let expected = step(&mut owner).unwrap(); let too_wide = capture(&mut owner, 2);
    let source = owner.hosted_learned_observation().unwrap();
    let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let before = owner.hosted_learned_generation().unwrap(); let control = owner.inspect();
    let usage = owner.hosted_learned_recovery_usage().unwrap(); let retained = owner.decoder_binding_usage();
    assert_eq!(owner.reset_hosted_learned(request(&owner, &too_wide, &config, 900)).err(), Some(Error::Limit));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before); assert_eq!(owner.inspect(), control);
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap(), usage); assert_eq!(owner.decoder_binding_usage(), retained);
    assert_eq!(source.availability(), LearnedAvailability::Ready); source.validate(&evidence).unwrap();
    // The same operation round remains unused; the nearby original checkpoint
    // fits all four mandatory K/V rows without weakening the fixed monitor.
    let reset = owner.reset_hosted_learned(request(&owner, &fitting, &config, 900)).unwrap();
    assert!(reset.control.restored); assert_eq!(reset.position, 1);
    assert_eq!(owner.hosted_learned_recovery_usage().unwrap().restart_attempts, usage.restart_attempts + 1);
    same_step(&step(&mut owner).unwrap(), &expected);
    assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Ready);
    assert_eq!(endpoint.execution_count(), 0);
}
