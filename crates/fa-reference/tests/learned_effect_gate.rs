//! Real learned-K/V inference composed with original reference congress/permits.
//! The memory endpoint and synthetic helper are controls, not deployment proof.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow, action_frame,
    decoder_monitoring::{DecoderBindingLimits, LearnedDecoderBindingLimits},
    human::{HumanDisposition, HumanReviewer, HumanReviewPolicy},
    learned_source::{LearnedEvidenceLimits, LearnedObservation, LearnedSourceConfig, ObservedLearnedGeneration},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget,
};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};

fn run(mode: u8) -> ObservedLearnedGeneration {
    let model = fixture::model();
    model.observed_learned_generation(LearnedSourceConfig {
        stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default(),
    }).unwrap()
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn actor(tokens: &[u32]) -> ActorState {
    // Deliberately AuditOnly: this test does not qualify external actor cache or
    // sampler bytes for restart. The live source independently owns real KV.
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::AuditOnly }, tokens.to_vec(), vec![0], vec![0], tokens.len() as u64).unwrap()
}
fn owner(tokens: &[u32], source: Option<LearnedObservation>, limits: LearnedDecoderBindingLimits,
    human: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>)
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
    if let Some(source) = source { owner.enable_learned_decoder_monitoring(source, limits).unwrap(); }
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

#[test]
fn real_learned_source_and_original_congress_both_required_for_one_use_publication() {
    let mut run = run(0);
    run.advance(0).unwrap();
    let (mut owner, mut endpoint, _) = owner(run.accepted_tokens(), Some(run.observation()),
        LearnedDecoderBindingLimits::default(), false);
    let (action, inputs) = proposal(&mut owner, 1);
    assert!(owner.decoder_monitoring_required());
    assert!(owner.decoder_evidence(1).unwrap().is_none());
    let evidence = owner.learned_decoder_evidence(1).unwrap().unwrap();
    assert_eq!(evidence.tokens(), &[0]);
    assert!(evidence.audit().complete_quiet());
    assert!(owner.dispatched_learned_decoder_evidence(1).is_err());
    assert_eq!(owner.authorize(1, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
    assert_eq!(endpoint.execution_count(), 0);
    approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    assert!(owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    let receipt = endpoint.deliver(&message).unwrap();
    assert!(owner.accept_receipt(receipt).unwrap());
    assert_eq!(endpoint.payload(), b"visible");
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatched_learned_decoder_evidence(1).unwrap().unwrap().audit().complete_quiet());
}

#[test]
fn advancement_alarm_and_drop_block_every_remaining_permitting_phase() {
    for loss in 0..3 {
        for phase in 0..4 {
            let mut run = run(if loss == 1 { 2 } else { 0 });
            run.advance(0).unwrap();
            let (mut owner, endpoint, _) = owner(run.accepted_tokens(), Some(run.observation()),
                LearnedDecoderBindingLimits::default(), false);
            let (action, inputs) = proposal(&mut owner, 1);
            let pending = if phase == 1 { Some(review(&mut owner, 1, Verdict::Allow)) } else { None };
            if phase >= 2 { approve(&mut owner, 1, &inputs); }
            let permit = if phase == 3 { Some(owner.authorize(1, Some(&inputs), &snapshot()).unwrap()) } else { None };
            let ledger = owner.inspect();
            if loss == 2 { drop(run); } else { run.advance(1).unwrap(); }
            let expected = if loss == 0 { Error::Stale } else { Error::Incomplete };
            let error = match phase {
                0 => owner.begin_review(1, 999, [7; 32], ReviewWindow {
                    commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
                }, &snapshot()).err(),
                1 => owner.apply_review(pending.unwrap(), Some(&inputs), &snapshot()).err(),
                2 => owner.authorize(1, Some(&inputs), &snapshot()).err(),
                _ => owner.dispatch(permit.as_ref().unwrap(), &action, Some(&inputs), &snapshot()).err(),
            };
            assert_eq!(error, Some(expected), "loss {loss}, phase {phase}");
            assert_eq!(owner.inspect(), ledger);
            assert_eq!(owner.learned_decoder_evidence(1).unwrap().unwrap().tokens(), &[0]);
            assert_eq!(endpoint.execution_count(), 0);
            assert_eq!(endpoint.payload(), b"initial");
        }
    }
}

#[test]
fn empty_or_mismatched_actor_cannot_propose_but_current_exact_prefix_can() {
    let mut run = run(0);
    let (mut owner, endpoint, _) = owner(&[], Some(run.observation()), LearnedDecoderBindingLimits::default(), false);
    let before = owner.inspect();
    assert_eq!(owner.propose(1, spec(&owner), &snapshot()).err(), Some(Error::Incomplete));
    assert_eq!(owner.inspect(), before);
    run.advance(0).unwrap();
    assert_eq!(owner.propose(1, spec(&owner), &snapshot()).err(), Some(Error::Binding));
    assert_eq!(owner.inspect(), before);
    owner.replace_actor_state(owner.actor_revision(), actor(run.accepted_tokens())).unwrap();
    proposal(&mut owner, 1);
    assert_eq!(endpoint.execution_count(), 0);
    run.advance(1).unwrap();
    owner.replace_actor_state(owner.actor_revision(), actor(run.accepted_tokens())).unwrap();
    proposal(&mut owner, 2);
    assert_eq!(owner.learned_decoder_evidence(1).unwrap().unwrap().tokens(), &[0]);
    assert_eq!(owner.learned_decoder_evidence(2).unwrap().unwrap().tokens(), run.accepted_tokens());
}

#[test]
fn exact_cumulative_source_and_score_costs_do_not_refund_when_attempts_cancel() {
    let mut run = run(0);
    run.advance(0).unwrap();
    let cost = run.observation().capture(LearnedEvidenceLimits::default()).unwrap().cost();
    let exact = LearnedDecoderBindingLimits { evidence: DecoderBindingLimits {
        token_ids: cost.token_ids * 2, score_words: cost.score_words * 2,
    }, encoded_bytes: cost.encoded_bytes * 2 };
    for axis in 0..3 {
        let mut limits = exact;
        match axis {
            0 => limits.evidence.token_ids -= 1,
            1 => limits.evidence.score_words -= 1,
            _ => limits.encoded_bytes -= 1,
        }
        let (mut owner, _, _) = owner(run.accepted_tokens(), Some(run.observation()), limits, false);
        proposal(&mut owner, 1);
        owner.cancel(1).unwrap();
        let before = owner.inspect();
        assert_eq!(owner.propose(2, spec(&owner), &snapshot()).err(), Some(Error::Limit));
        assert_eq!(owner.inspect(), before);
        assert_eq!(owner.learned_decoder_binding_bytes(), Some(cost.encoded_bytes));
        assert_eq!(owner.decoder_binding_usage().unwrap().token_ids, cost.token_ids);
    }
    let (mut owner, _, _) = owner(run.accepted_tokens(), Some(run.observation()), exact, false);
    proposal(&mut owner, 1);
    owner.cancel(1).unwrap();
    proposal(&mut owner, 2);
    assert_eq!(owner.learned_decoder_binding_bytes(), Some(cost.encoded_bytes * 2));
    assert_eq!(owner.decoder_binding_usage().unwrap().score_words, cost.score_words * 2);
}

#[test]
fn two_key_dispatch_rechecks_learned_source_after_human_approval() {
    for loss in [false, true] {
        let mut run = run(0);
        run.advance(0).unwrap();
        let (mut owner, mut endpoint, reviewer) = owner(run.accepted_tokens(), Some(run.observation()),
            LearnedDecoderBindingLimits::default(), true);
        let (action, inputs) = proposal(&mut owner, 1);
        approve(&mut owner, 1, &inputs);
        let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
        assert_eq!(owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
        let request = owner.request_human_approval(10, 1, Some(&inputs), ElapsedTick(50)).unwrap();
        let human = reviewer.unwrap().approve(&request, ElapsedTick(1)).unwrap();
        if loss {
            drop(run);
            let before = owner.inspect();
            assert_eq!(owner.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
            assert_eq!(owner.inspect(), before);
            assert_eq!(owner.human_status(10).unwrap().disposition, HumanDisposition::Approved);
            assert_eq!(endpoint.execution_count(), 0);
        } else {
            let message = owner.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap();
            assert_eq!(owner.human_status(10).unwrap().disposition, HumanDisposition::Consumed);
            owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
            assert_eq!(endpoint.execution_count(), 1);
        }
    }
}

#[test]
fn post_dispatch_source_loss_does_not_erase_external_work_or_block_reconciliation() {
    let mut run = run(0);
    run.advance(0).unwrap();
    let (mut owner, mut endpoint, _) = owner(run.accepted_tokens(), Some(run.observation()),
        LearnedDecoderBindingLimits::default(), false);
    let (action, inputs) = proposal(&mut owner, 1);
    approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    drop(run);
    owner.acknowledgment_lost(1).unwrap();
    assert!(owner.cancel(1).is_err());
    assert_eq!(owner.pending_reconciliation().unwrap().len(), 1);
    let receipt = endpoint.deliver(&message).unwrap();
    assert!(owner.accept_receipt(receipt).unwrap());
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
    endpoint.deliver(&message).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert!(owner.dispatched_learned_decoder_evidence(1).unwrap().is_some());
}

#[test]
fn source_cannot_be_replaced_and_policy_epoch_still_invalidates_existing_evidence() {
    let mut run = run(0);
    run.advance(0).unwrap();
    let (mut owner, endpoint, _) = owner(run.accepted_tokens(), Some(run.observation()),
        LearnedDecoderBindingLimits::default(), false);
    assert_eq!(owner.enable_learned_decoder_monitoring(run.observation(),
        LearnedDecoderBindingLimits::default()), Err(Error::Duplicate));
    let (_, inputs) = proposal(&mut owner, 1);
    approve(&mut owner, 1, &inputs);
    owner.revoke_epoch().unwrap();
    assert_eq!(owner.authorize(1, Some(&inputs), &snapshot()).err(), Some(Error::Stale));
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn restrictive_congress_result_survives_later_source_loss() {
    let mut run = run(0);
    run.advance(0).unwrap();
    let (mut owner, endpoint, _) = owner(run.accepted_tokens(), Some(run.observation()),
        LearnedDecoderBindingLimits::default(), false);
    let (_, inputs) = proposal(&mut owner, 1);
    let held = review(&mut owner, 1, Verdict::Hold);
    drop(run);
    owner.apply_review(held, None, &snapshot()).unwrap();
    assert_ne!(owner.inspect().decisions[&1], fa_reference::action::consequence::Consequence::Continue);
    assert_eq!(owner.authorize(1, Some(&inputs), &snapshot()).err(), Some(Error::Incomplete));
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn residual_source_keeps_original_evidence_type_and_dispatch_behavior() {
    use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
    use fa_reference::action::consequence::activation::monitor::decoder::MonitoredDecoder;
    use fa_reference::action::consequence::activation::probe::LinearProbe;
    use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
    let model = fixture::model();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let mut monitors = BTreeMap::new();
    for layer in 1..=2 {
        let contract = model.residual_contract(layer).unwrap();
        let probe = LinearProbe::new(1, 1, contract.profile(), &[0.0, 0.0], 0.0, 1.0).unwrap();
        monitors.insert(layer, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap());
    }
    let mut residual = MonitoredDecoder::new(model, 21, 31, monitors, allowance).unwrap();
    residual.advance(0, 0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    let (mut owner, mut endpoint, _) = owner(&[0], None, LearnedDecoderBindingLimits::default(), false);
    owner.enable_decoder_monitoring(residual.observation(), DecoderBindingLimits::default()).unwrap();
    let learned = run(0);
    assert_eq!(owner.enable_learned_decoder_monitoring(learned.observation(),
        LearnedDecoderBindingLimits::default()), Err(Error::Duplicate));
    let (action, inputs) = proposal(&mut owner, 1);
    assert!(owner.decoder_evidence(1).unwrap().is_some());
    assert!(owner.learned_decoder_evidence(1).unwrap().is_none());
    assert!(owner.learned_decoder_binding_bytes().is_none());
    approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert!(owner.dispatched_decoder_evidence(1).unwrap().is_some());
}
