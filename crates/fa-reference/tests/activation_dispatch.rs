//! Actual codec/probe results through the existing reference effect path.
//! Supplied host tensors, independent votes and endpoint remain model inputs.

use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ObservedReview, ObservedSession, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 42, model_generation: 1, tap: 3, layout_generation: 1 }
}
fn source(sequence: u64, position: u64, value: f32) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: 9, sequence, position }, &[value]).unwrap()
}
fn monitor(bytes: usize) -> RefinementMonitor {
    RefinementMonitor::new(vec![LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 0.0).unwrap()],
        vec![0, 8, 23], RefinementBudget { encoded_bytes: bytes, probe_coordinates: 100 }).unwrap()
}
fn actor(tokens: Vec<u32>) -> ActorState {
    let position = tokens.len() as u64;
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
        tokens, vec![2], vec![3], position).unwrap()
}
fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 6, object: 7, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn fixture(bytes: usize, captures: usize) -> (OversightBroker, PublicationEndpoint) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 11, profile_bytes: b"frame-v1".to_vec(), tokenizer_epoch: 1,
            policy_epoch: 0, model_epoch: 1 }, 9, b"Review".to_vec()).unwrap())])).unwrap();
    let config = ControllerConfig { scope: spec(0).scope, total: 100, max_attempts: 8, actor: actor(vec![1]),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() };
    let mut endpoint = PublicationEndpoint::new(spec(0).target.unwrap(), b"old".to_vec(), 200, 8).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts).unwrap();
    broker.enable_activation_tripwire(monitor(bytes), 9, captures).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    (broker, endpoint)
}
fn prepare(broker: &mut OversightBroker, id: u64) -> (FrozenAction, CommitteeInput) {
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch), &snapshot()).unwrap();
    let action = proposal.action;
    let helper = &broker.contracts().members()["helper"];
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: boundary, end }, kind: PartKind::Question },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 9,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, broker.contracts(), BTreeMap::from([("helper".to_owned(), manifest)])).unwrap();
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    (action, inputs)
}
fn session(broker: &mut OversightBroker, id: u64, round: u64) -> ObservedSession {
    broker.begin_review(id, round, [1; 32], ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) }, &snapshot()).unwrap()
}
fn finish(mut session: ObservedSession, verdict: Verdict) -> ObservedReview {
    let commitment = session.commitment("helper", verdict, b"salt").unwrap();
    session.commit("helper", commitment, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("helper", verdict, b"salt", ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}
fn approve(broker: &mut OversightBroker, id: u64, round: u64, inputs: &CommitteeInput) {
    let review = finish(session(broker, id, round), Verdict::Allow);
    broker.apply_review(review, Some(inputs), &snapshot()).unwrap();
}
fn capture(broker: &mut OversightBroker, id: u64, sequence: u64, position: u64, value: f32) -> MonitorOutcome {
    broker.record_activation(id, broker.input_revision(id).unwrap(), broker.actor_revision(),
        &source(sequence, position, value)).unwrap().outcome()
}

#[test]
fn quiet_observation_needs_independent_review_before_real_reference_publication() {
    let (mut broker, mut endpoint) = fixture(4096, 8);
    let (action, inputs) = prepare(&mut broker, 1);
    let review = finish(session(&mut broker, 1, 11), Verdict::Allow);
    assert_eq!(broker.apply_review(review, Some(&inputs), &snapshot()), Err(Error::Incomplete));
    assert_eq!(capture(&mut broker, 1, 1, 0, -1.5), MonitorOutcome::NoAlarm);
    assert_eq!(broker.authorize(1, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    approve(&mut broker, 1, 12, &inputs);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"publish");
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn rare_alarm_budget_exhaustion_and_exact_equality_are_never_approval() {
    for (bytes, value, expected) in [(4096, f32::from_bits(1), MonitorOutcome::Alarm),
        (80, -f32::from_bits(1), MonitorOutcome::BudgetExhausted),
        (4096, 0.0, MonitorOutcome::AtThreshold)]
    {
        let (mut broker, endpoint) = fixture(bytes, 8);
        let (_, inputs) = prepare(&mut broker, 1);
        assert_eq!(capture(&mut broker, 1, 1, 0, value), expected);
        let review = finish(session(&mut broker, 1, 11), Verdict::Allow);
        assert_eq!(broker.apply_review(review, Some(&inputs), &snapshot()), Err(Error::Incomplete));
        assert_eq!(broker.inspect().ledger.available, 100);
        assert_eq!(endpoint.execution_count(), 0);
        broker.cancel(1).unwrap();
    }
}

#[test]
fn refreshed_capture_invalidates_completed_review_even_with_identical_helper_bytes() {
    let (mut broker, _) = fixture(4096, 8);
    let (_, inputs) = prepare(&mut broker, 1);
    capture(&mut broker, 1, 1, 0, -1.5);
    let old = finish(session(&mut broker, 1, 11), Verdict::Allow);
    let revision = broker.input_revision(1).unwrap();
    capture(&mut broker, 1, 2, 0, -1.75);
    assert_eq!(broker.input_revision(1).unwrap(), revision + 1);
    assert_eq!(broker.apply_review(old, Some(&inputs), &snapshot()), Err(Error::Stale));
    approve(&mut broker, 1, 12, &inputs);
    broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
}

#[test]
fn capacity_failure_invalidates_old_quiet_but_does_not_refund_a_reservation() {
    let (mut broker, endpoint) = fixture(4096, 1);
    let (action, inputs) = prepare(&mut broker, 1);
    capture(&mut broker, 1, 1, 0, -1.0);
    approve(&mut broker, 1, 11, &inputs);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let revision = broker.input_revision(1).unwrap();
    assert_eq!(broker.record_activation(1, revision, 0, &source(2, 0, 1.0)), Err(Error::Limit));
    assert_eq!(broker.input_revision(1).unwrap(), revision + 1);
    assert!(broker.activation_report(1).unwrap().is_none());
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect().ledger.reserved, 16);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn capture_outage_cannot_refund_or_block_reconciliation_of_a_sent_effect() {
    let (mut broker, mut endpoint) = fixture(4096, 8);
    let (action, inputs) = prepare(&mut broker, 1);
    capture(&mut broker, 1, 1, 0, -1.0); approve(&mut broker, 1, 11, &inputs);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    broker.activation_unavailable(1, broker.input_revision(1).unwrap()).unwrap();
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.cancel(1), Err(Error::WrongState));
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.charged, 16);
}

#[test]
fn human_key_cannot_bypass_changed_actor_state_or_activation_basis() {
    let (mut broker, mut endpoint) = fixture(4096, 8);
    let reviewer = broker.enable_human_review(HumanReviewPolicy { reviewer_id: 1,
        max_validity_ticks: 50, max_requests: 8 }).unwrap();
    let (action, inputs) = prepare(&mut broker, 1);
    capture(&mut broker, 1, 1, 0, -1.0); approve(&mut broker, 1, 11, &inputs);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(40)).unwrap();
    let old_key = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    broker.replace_actor_state(0, actor(vec![1, 2])).unwrap();
    assert_eq!(broker.dispatch_with_human(&permit, &old_key, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    capture(&mut broker, 1, 2, 1, -1.0); approve(&mut broker, 1, 12, &inputs);
    assert_eq!(broker.dispatch_with_human(&permit, &old_key, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    let request = broker.request_human_approval(2, 1, Some(&inputs), ElapsedTick(40)).unwrap();
    let key = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = broker.dispatch_with_human(&permit, &key, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(broker.human_status(2).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn stale_revision_wrong_profile_stream_and_future_position_refuse_before_admission() {
    let (mut broker, _) = fixture(4096, 8);
    prepare(&mut broker, 1);
    let original = broker.input_revision(1).unwrap();
    let base = source(1, 0, -1.0).identity();
    for field in 0..4 {
        let mut changed = base;
        match field { 0 => changed.profile.tenant += 1, 1 => changed.profile.tap += 1,
            2 => changed.stream += 1, _ => changed.position += 1 }
        let value = SourceFrame::capture(changed, &[-1.0]).unwrap();
        assert_eq!(broker.record_activation(1, original, 0, &value), Err(Error::Binding));
        assert_eq!(broker.input_revision(1).unwrap(), original);
    }
    assert_eq!(broker.record_activation(1, original, 1, &source(1, 0, -1.0)), Err(Error::Stale));
    capture(&mut broker, 1, 1, 0, -1.0);
    assert_eq!(broker.record_activation(1, broker.input_revision(1).unwrap(), 0, &source(1, 0, -1.0)), Err(Error::Stale));
    assert_eq!(broker.enable_activation_tripwire(monitor(9999), 9, 8), Err(Error::Duplicate));
}

#[test]
fn reset_preserves_tripwire_and_requires_new_capture_review_and_permit() {
    let (mut broker, mut endpoint) = fixture(4096, 8);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let (old_action, old_inputs) = prepare(&mut broker, 1);
    capture(&mut broker, 1, 1, 0, -1.0); approve(&mut broker, 1, 11, &old_inputs);
    let old_permit = broker.authorize(1, Some(&old_inputs), &snapshot()).unwrap();
    let before = broker.inspect();
    broker.reset(ResetRequest { checkpoint, expected_control_sequence: before.sequence,
        expected_actor_revision: 0, binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }).unwrap();
    assert!(broker.activation_tripwire_required());
    assert!(broker.dispatch(&old_permit, &old_action, Some(&old_inputs), &snapshot()).is_err());
    let (action, inputs) = prepare(&mut broker, 2);
    capture(&mut broker, 2, 2, 0, -1.0); approve(&mut broker, 2, 12, &inputs);
    let permit = broker.authorize(2, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn restrictive_reviews_remain_applicable_without_an_activation_capture() {
    let (mut broker, _) = fixture(4096, 8);
    prepare(&mut broker, 1);
    let review = finish(session(&mut broker, 1, 11), Verdict::Hold);
    let mut unavailable = snapshot(); unavailable.complete = false;
    broker.apply_review(review, None, &unavailable).unwrap();
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
}
