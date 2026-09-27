//! Real learned-K/V inference composed with original reference congress/permits.
//! The memory endpoint and synthetic helper are controls, not deployment proof.
mod model;
use model::{model, policy};
use crate::{Error, Snapshot};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::PublicationEndpoint;
use crate::action::consequence::gate::{TargetCeiling, ReviewBinding};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile, ResetRequest};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow, action_frame,
    decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy},
    learned_source::{LearnedEvidenceLimits, LearnedAvailability, LearnedSourceConfig},
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget, GenerationStatus, GenerationStop, LearnedGeneration,
};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};

fn config(model: &DecoderModel, alarm: bool, top_k: usize) -> LearnedSourceConfig {
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: policy(model, alarm), budget: GenerationBudget::default(),
        telemetry: GenerationTelemetryBudget::default() }
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn actor(tokens: &[u32]) -> ActorState {
    // A caller-declared reference profile, not a qualified deployment grade.
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, tokens.to_vec(), vec![0], vec![0], tokens.len() as u64).unwrap()
}
fn owner(tokens: &[u32], human: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>)
{
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8, actor: actor(tokens), suspend_at_incident: 3,
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
    owner.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, reviewer)
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(owner: &OversightBroker) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn proposal(owner: &mut OversightBroker, id: u64) -> (FrozenAction, CommitteeInput) {
    let action = owner.propose(id, spec(owner), &snapshot()).unwrap().action;
    let helper = &owner.contracts().members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
        projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, owner.contracts(),
        BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    owner.record_inputs(id, owner.input_revision(id).unwrap(), inputs.clone()).unwrap();
    (action, inputs)
}
fn review(owner: &mut OversightBroker, id: u64, verdict: Verdict) -> ObservedReview {
    let mut session = owner.begin_review(id, id + 100, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let salt = [11; 32];
    let commit = session.commitment("reviewer", verdict, &salt).unwrap();
    session.commit("reviewer", commit, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", verdict, &salt, ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}
fn approve(owner: &mut OversightBroker, id: u64, inputs: &CommitteeInput) {
    let review = review(owner, id, Verdict::Allow);
    owner.apply_review(review, Some(inputs), &snapshot()).unwrap();
}


use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::delivery::StopRequest;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use super::GenerationEvent;

fn attach(owner: &mut OversightBroker, model: &DecoderModel, config: LearnedSourceConfig) {
    owner.own_learned_generation(model.clone(), config, LearnedDecoderBindingLimits::default()).unwrap();
}
fn step(owner: &mut OversightBroker) -> Result<Rc<GenerationEvent>, Error> {
    let current = owner.hosted_learned_generation()?;
    owner.advance_hosted_learned(current.actor_revision, current.position)
}
fn original(model: &DecoderModel, config: LearnedSourceConfig) -> LearnedGeneration {
    model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
        config.spec, config.policy, config.budget, config.telemetry).unwrap()
}
fn same_actor(owner: &OversightBroker, original: &LearnedGeneration) {
    let state = owner.delivery.controller().actor();
    assert_eq!(state.tokens(), original.accepted_tokens());
    assert_eq!(state.cache(), original.accepted_cache_image().unwrap().encode().unwrap());
    assert_eq!(state.sampler(), original.sampler_state().encode());
    assert_eq!(state.next_position(), original.position());
    let view = owner.hosted_learned_generation().unwrap();
    assert_eq!(view.work, original.work());
    assert_eq!(view.telemetry, original.telemetry_work());
    assert_eq!(view.sampled_draws, original.sampler_state().draws());
    assert_eq!(view.status, original.status());
}

#[test]
fn owned_original_generation_synchronizes_exact_cache_sampler_and_work_at_every_step() {
    let model = model(); let config = config(&model, false, 3);
    let mut control = original(&model, config.clone());
    let (mut owner, endpoint, _) = owner(&[], false);
    let initial_revision = owner.actor_revision();
    attach(&mut owner, &model, config);
    let source = owner.hosted_learned_observation().unwrap();
    assert_eq!(source.availability(), LearnedAvailability::Empty);
    assert_eq!(owner.actor_revision(), initial_revision + 1);
    same_actor(&owner, &control);
    while control.status().is_active() {
        let before = owner.actor_revision();
        let left = control.advance(control.position()).unwrap();
        let right = step(&mut owner).unwrap();
        assert_eq!(left.sample(), right.sample());
        assert_eq!(left.accepted().unwrap().logits.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            right.accepted().unwrap().logits.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
        if let (Some(a), Some(b)) = (left.sample(), right.sample()) {
            assert_eq!(a.probability.to_bits(), b.probability.to_bits());
        }
        same_actor(&owner, &control);
        assert_eq!(owner.actor_revision(), before + 1);
        let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
        assert_eq!(evidence.tokens(), control.accepted_tokens());
        source.validate(&evidence).unwrap();
    }
    assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
    same_actor(&owner, &control);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn exact_owned_prefix_flows_through_original_congress_two_keys_and_one_use_delivery() {
    let model = model(); let (mut owner, mut endpoint, human) = owner(&[], true);
    attach(&mut owner, &model, config(&model, false, 1));
    assert_eq!(owner.propose(1, spec(&owner), &snapshot()).err(), Some(Error::Incomplete));
    step(&mut owner).unwrap();
    let (action, inputs) = proposal(&mut owner, 1);
    assert_eq!(owner.learned_decoder_evidence(1).unwrap().unwrap().tokens(), &[0]);
    assert!(owner.authorize(1, Some(&inputs), &snapshot()).is_err());
    approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
    let request = owner.request_human_approval(10, 1, Some(&inputs), ElapsedTick(80)).unwrap();
    let human = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    assert!(owner.accept_receipt(receipt).unwrap());
    assert_eq!(endpoint.payload(), b"visible");
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.spent, 16);
    assert!(owner.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).is_err());
}

#[test]
fn later_owned_inference_stales_old_approval_but_new_proposal_needs_no_manual_actor_copy() {
    for phase in 0..2 {
        let model = model(); let (mut owner, endpoint, _) = owner(&[], false);
        attach(&mut owner, &model, config(&model, false, 1)); step(&mut owner).unwrap();
        let (action, inputs) = proposal(&mut owner, 1);
        let pending = if phase == 0 { Some(review(&mut owner, 1, Verdict::Allow)) } else { None };
        let permit = if phase == 1 {
            approve(&mut owner, 1, &inputs); Some(owner.authorize(1, Some(&inputs), &snapshot()).unwrap())
        } else { None };
        step(&mut owner).unwrap();
        if let Some(review) = pending { assert_eq!(owner.apply_review(review, Some(&inputs), &snapshot()).err(), Some(Error::Stale)); }
        if let Some(permit) = permit { assert_eq!(owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).err(), Some(Error::Stale)); }
        let (_, current) = proposal(&mut owner, 2);
        approve(&mut owner, 2, &current);
        assert!(owner.authorize(2, Some(&current), &snapshot()).is_ok());
        assert_eq!(owner.learned_decoder_evidence(1).unwrap().unwrap().tokens(), &[0]);
        assert_eq!(owner.learned_decoder_evidence(2).unwrap().unwrap().tokens().len(), 2);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn held_and_failed_attempts_preserve_actor_bytes_and_uncommitted_draws_without_refunding_work() {
    for alarm in [false, true] {
        let model = model(); let mut config = config(&model, alarm, 1);
        if !alarm { config.telemetry.source_check_values = model.cache_profile().values_per_token() as u64; }
        let mut control = original(&model, config.clone());
        let (mut owner, endpoint, _) = owner(&[], false); attach(&mut owner, &model, config);
        step(&mut owner).unwrap(); control.advance(0).unwrap();
        let actor = owner.delivery.controller().actor().clone(); let revision = owner.actor_revision();
        let (_, inputs) = proposal(&mut owner, 1); approve(&mut owner, 1, &inputs);
        let a = step(&mut owner); let b = control.advance(1);
        if alarm {
            assert!(a.unwrap().accepted().is_none()); assert!(b.unwrap().sample().is_none());
            assert_eq!(owner.hosted_learned_generation().unwrap().status, GenerationStatus::Held(MonitorOutcome::Alarm));
        } else { assert_eq!(a.err(), Some(Error::Limit)); assert_eq!(b.err(), Some(Error::Limit)); }
        same_actor(&owner, &control);
        assert_eq!(owner.delivery.controller().actor(), &actor);
        assert_eq!(owner.actor_revision(), revision);
        assert_eq!(owner.hosted_learned_generation().unwrap().sampled_draws, 0);
        assert_eq!(owner.hosted_learned_generation().unwrap().work.sampling_attempts, 1);
        assert_eq!(owner.authorize(1, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
        assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn rejected_bootstrap_is_atomic_and_a_valid_nearby_bootstrap_still_succeeds() {
    let model = model(); let good = config(&model, false, 1);
    let foreign = model::sized_model(2, 2, 16, 99);
    let (mut nonempty, _, _) = owner(&[0], false);
    assert_eq!(nonempty.own_learned_generation(model.clone(), good.clone(), LearnedDecoderBindingLimits::default()), Err(Error::WrongState));
    assert_eq!(nonempty.delivery.controller().actor().tokens(), &[0]);
    let (mut owner, _, _) = owner(&[], false);
    let actor = owner.delivery.controller().actor().clone(); let revision = owner.actor_revision();
    let mut bad = good.clone(); bad.monitor_generation = 0;
    assert_eq!(owner.own_learned_generation(model.clone(), bad, LearnedDecoderBindingLimits::default()), Err(Error::InvalidInput));
    assert_eq!(owner.own_learned_generation(foreign.clone(), config(&foreign, false, 1), LearnedDecoderBindingLimits::default()), Err(Error::Binding));
    let mut bad = good.clone(); bad.budget.decoder_products = 0;
    assert_eq!(owner.own_learned_generation(model.clone(), bad, LearnedDecoderBindingLimits::default()), Err(Error::Limit));
    assert_eq!(owner.own_learned_generation(model.clone(), good.clone(), LearnedDecoderBindingLimits {
        encoded_bytes: 0, ..LearnedDecoderBindingLimits::default() }), Err(Error::InvalidInput));
    assert!(!owner.decoder_monitoring_required());
    assert_eq!(owner.delivery.controller().actor(), &actor); assert_eq!(owner.actor_revision(), revision);
    attach(&mut owner, &model, good);
    assert!(step(&mut owner).unwrap().accepted().is_some());
}

#[test]
fn whole_horizon_actor_capacity_is_preflighted_not_discovered_after_a_partial_generation() {
    let model = model::sized_model(32, 4, 1024, 3);
    let base = config(&model, false, 1);
    for horizon in [1023, 1024] {
        let (mut owner, _, _) = owner(&[], false);
        let mut config = base.clone();
        config.spec = GenerationSpec::new(vec![0], horizon - 1, BTreeSet::new(), base.spec.sampling().clone()).unwrap();
        let result = owner.own_learned_generation(model.clone(), config, LearnedDecoderBindingLimits::default());
        if horizon == 1023 { result.unwrap(); assert_eq!(owner.hosted_learned_generation().unwrap().position, 0); }
        else { assert_eq!(result, Err(Error::Limit)); assert!(!owner.decoder_monitoring_required()); assert_eq!(owner.actor_revision(), 0); }
    }
}

#[test]
fn owned_mode_refuses_raw_actor_reset_replacement_and_mixed_source_installation() {
    let model = model(); let (mut owner, _, _) = owner(&[], false);
    let saved = owner.capture_checkpoint(1, owner.actor_revision()).unwrap();
    let good = config(&model, false, 1); attach(&mut owner, &model, good.clone());
    let revision = owner.actor_revision();
    assert_eq!(owner.replace_actor_state(revision, actor(&[0])), Err(Error::WrongState));
    assert!(matches!(owner.capture_checkpoint(2, revision), Err(Error::WrongState)));
    assert!(matches!(owner.reset(ResetRequest { checkpoint: saved, expected_control_sequence: owner.inspect().sequence,
        expected_actor_revision: revision, binding: ReviewBinding { round: 7, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: TargetCeiling::new(&[target()]).unwrap() }), Err(Error::WrongState)));
    assert_eq!(owner.own_learned_generation(model.clone(), good.clone(), LearnedDecoderBindingLimits::default()), Err(Error::Duplicate));
    let external = model.observed_learned_generation(good).unwrap();
    assert_eq!(owner.enable_learned_decoder_monitoring(external.observation(), LearnedDecoderBindingLimits::default()), Err(Error::Duplicate));
    assert_eq!(owner.actor_revision(), revision);
    step(&mut owner).unwrap();
    assert_eq!(owner.delivery.controller().actor().tokens(), &[0]);
}

#[test]
fn stale_calls_and_manual_stop_do_not_spend_additional_work() {
    let model = model(); let (mut owner, _, _) = owner(&[], false);
    attach(&mut owner, &model, config(&model, false, 1)); step(&mut owner).unwrap();
    let source = owner.hosted_learned_observation().unwrap(); let evidence = source.capture(LearnedEvidenceLimits::default()).unwrap();
    let before = owner.hosted_learned_generation().unwrap();
    for (revision, position) in [(before.actor_revision - 1, 1), (before.actor_revision, 0), (before.actor_revision, 2)] {
        assert_eq!(owner.advance_hosted_learned(revision, position).err(), Some(Error::Stale));
        assert_eq!(owner.hosted_learned_generation().unwrap(), before); source.validate(&evidence).unwrap();
    }
    let view = owner.inspect();
    owner.request_stop(StopRequest { operation: 900, expected_control_sequence: view.sequence,
        expected_authority_epoch: view.ledger.epoch }).unwrap();
    assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
    drop(owner);
    assert_eq!(source.availability(), LearnedAvailability::Closed);
    assert_eq!(source.validate(&evidence), Err(Error::Incomplete));
}

#[test]
fn synchronization_error_and_unwind_withdraw_a_numerically_quiet_but_unsynchronized_source() {
    for unwind in [false, true] {
        let model = model(); let (mut owner, endpoint, _) = owner(&[], false);
        attach(&mut owner, &model, config(&model, false, 1));
        let source = owner.hosted_learned_observation().unwrap();
        let before = owner.delivery.controller().actor().clone(); let revision = owner.actor_revision();
        if unwind {
            // Inject an unwind at the private composition seam AFTER the real
            // generator publishes quiet, BEFORE the controller actor copy changes.
            assert!(catch_unwind(AssertUnwindSafe(|| {
                let host = owner.learned_host.as_mut().unwrap(); host.fault = Some(Error::Incomplete);
                let _guard = host.run.guard_host_sync(); host.run.advance(0).unwrap();
                assert_eq!(source.availability(), LearnedAvailability::Ready);
                panic!("injected actor synchronization interruption");
            })).is_err());
        } else {
            // Test-only corruption creates an actual original actor-profile
            // rejection, rather than mocking a successful production result.
            owner.learned_host.as_mut().unwrap().profile.model_generation += 1;
            assert_eq!(step(&mut owner).err(), Some(Error::Binding));
        }
        assert_eq!(source.availability(), LearnedAvailability::Failed);
        assert!(source.capture(LearnedEvidenceLimits::default()).is_err());
        assert_eq!(owner.actor_revision(), revision); assert_eq!(owner.delivery.controller().actor(), &before);
        assert_eq!(owner.hosted_learned_generation().unwrap().work.admitted_tokens, 1);
        assert_eq!(owner.propose(1, spec(&owner), &snapshot()).err(), Some(Error::Incomplete));
        assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn original_stop_token_remains_terminal_but_its_live_quiet_basis_is_still_reviewable() {
    let model = model(); let mut config = config(&model, false, 1);
    config.spec = GenerationSpec::new(vec![0], 3, BTreeSet::from([2]), config.spec.sampling().clone()).unwrap();
    let (mut owner, endpoint, _) = owner(&[], false); attach(&mut owner, &model, config);
    step(&mut owner).unwrap(); step(&mut owner).unwrap();
    let before = owner.hosted_learned_generation().unwrap();
    assert_eq!(before.status, GenerationStatus::Finished(GenerationStop::StopToken(2)));
    assert_eq!(before.sampled_draws, 1);
    assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
    let (_, input) = proposal(&mut owner, 1); approve(&mut owner, 1, &input);
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_ok());
    assert_eq!(endpoint.execution_count(), 0);
}
