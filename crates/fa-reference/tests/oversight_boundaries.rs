//! Deadline, provider-change, policy-rotation and containment controls.
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, MAX_OBSERVED_ROUNDS, ObservedReview, ObservedSession, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION, scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
fn policy(generation: u64) -> Policy { Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap() }
fn config() -> ControllerConfig {
    ControllerConfig { scope: spec(0).scope, total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1, tokenizer_generation: 1,
            state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: policy(1),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }
}
fn contracts() -> CommitteeContract {
    CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"framed-action-v1/helper".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 }, 9, b"Review".to_vec(),
    ).unwrap())])).unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) } }
fn fixture() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let contracts = contracts(); let mut endpoint = PublicationEndpoint::new(spec(0).target.unwrap(), b"old".to_vec(), 200, 8).unwrap();
    let mut broker = OversightBroker::new(config(), &mut endpoint, contracts.clone()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    let ack = endpoint.install_fence(broker.fence_request()).unwrap(); broker.confirm_fence(ack).unwrap(); (broker, endpoint, contracts)
}
fn manifest(action: &FrozenAction, contract: &HelperContract, omissions: Vec<Omission>, projection_id: u64) -> EvidenceViewManifest {
    let mut bytes = action_frame(action); let boundary = bytes.len(); bytes.extend_from_slice(contract.question()); let end = bytes.len();
    let input = ActualHelperInput::new(bytes, contract.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], omissions).unwrap();
    EvidenceViewManifest::new(input, AuthorizationProjection { projection_id, policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap()
}
fn inputs(action: &FrozenAction, contracts: &CommitteeContract) -> CommitteeInput {
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), manifest(action, &contracts.members()["helper"], Vec::new(), 9))])).unwrap()
}
fn prepare(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64) -> (FrozenAction, CommitteeInput) {
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch), &snapshot()).unwrap(); let inputs = inputs(&proposal.action, contracts);
    broker.record_inputs(id, 0, inputs.clone()).unwrap(); (proposal.action, inputs)
}
fn window() -> ReviewWindow { ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) } }
fn complete(mut session: ObservedSession, tick: u64) -> ObservedReview {
    let value = session.commitment("helper", Verdict::Allow, b"salt").unwrap(); session.commit("helper", value, ElapsedTick(tick)).unwrap();
    session.open_reveals(ElapsedTick(tick)).unwrap(); session.reveal("helper", Verdict::Allow, b"salt", ElapsedTick(tick)).unwrap(); session.finish(ElapsedTick(tick)).unwrap()
}
fn approve(broker: &mut OversightBroker, id: u64, round: u64, input: &CommitteeInput) {
    let tick = broker.inspect().ledger.elapsed.unwrap().0; let session = broker.begin_review(id, round, [1; 32], window(), &snapshot()).unwrap();
    broker.apply_review(complete(session, tick), Some(input), &snapshot()).unwrap();
}
#[test]
fn late_reveal_is_missing_even_when_its_commitment_was_timely() {
    let (mut broker, _, contracts) = fixture(); prepare(&mut broker, &contracts, 1);
    let mut session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    let value = session.commitment("helper", Verdict::Allow, b"salt").unwrap(); session.commit("helper", value, ElapsedTick(1)).unwrap(); session.open_reveals(ElapsedTick(2)).unwrap();
    assert_eq!(session.reveal("helper", Verdict::Allow, b"salt", ElapsedTick(20)), Err(Error::Stale));
    let review = session.finish(ElapsedTick(20)).unwrap(); assert_eq!(review.missing(), &["helper".to_owned()]); assert!(review.abstained().is_empty());
    assert_eq!(review.decision().consequence, Consequence::HoldEffect); broker.observe_time(ElapsedTick(20)).unwrap(); broker.apply_review(review, None, &snapshot()).unwrap();
}
#[test]
fn an_entirely_silent_round_can_close_at_the_deadline() {
    let (mut broker, _, contracts) = fixture(); prepare(&mut broker, &contracts, 1);
    let mut session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap(); assert_eq!(session.finish(ElapsedTick(19)).unwrap_err(), Error::Incomplete);
    let review = session.finish(ElapsedTick(20)).unwrap(); assert_eq!(review.decision().consequence, Consequence::HoldEffect); assert_eq!(review.missing().len(), 1);
}
#[test]
fn invalid_reveal_does_not_count_as_phase_completion() {
    let (mut broker, _, contracts) = fixture(); let (_, input) = prepare(&mut broker, &contracts, 1);
    let mut session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    let value = session.commitment("helper", Verdict::Allow, b"salt").unwrap(); session.commit("helper", value, ElapsedTick(1)).unwrap(); session.open_reveals(ElapsedTick(1)).unwrap();
    assert_eq!(session.reveal("helper", Verdict::Allow, b"wrong", ElapsedTick(2)), Err(Error::Binding)); assert_eq!(session.finish(ElapsedTick(2)).unwrap_err(), Error::Incomplete);
    session.reveal("helper", Verdict::Allow, b"salt", ElapsedTick(3)).unwrap(); let review = session.finish(ElapsedTick(3)).unwrap();
    broker.observe_time(ElapsedTick(3)).unwrap(); broker.apply_review(review, Some(&input), &snapshot()).unwrap(); broker.authorize(1, Some(&input), &snapshot()).unwrap();
}
#[test]
fn a_future_completed_review_cannot_advance_the_controller_clock() {
    let (mut broker, _, contracts) = fixture(); let (_, input) = prepare(&mut broker, &contracts, 1);
    let session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap(); let before = broker.inspect();
    assert_eq!(broker.apply_review(complete(session, 2), Some(&input), &snapshot()), Err(Error::Stale)); assert_eq!(broker.inspect(), before);
    broker.observe_time(ElapsedTick(2)).unwrap(); approve(&mut broker, 1, 12, &input);
}
#[test]
fn discarded_round_ids_are_not_reused_with_a_new_context() {
    let (mut broker, _, contracts) = fixture(); let (_, input) = prepare(&mut broker, &contracts, 1);
    let session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap(); drop(session);
    assert_eq!(broker.begin_review(1, 11, [2; 32], window(), &snapshot()).unwrap_err(), Error::Duplicate); approve(&mut broker, 1, 12, &input);
}
#[test]
fn helper_model_replacement_cannot_hide_behind_identical_submitted_bytes() {
    let (mut broker, _, contracts) = fixture(); let (action, original) = prepare(&mut broker, &contracts, 1);
    let mut profile = contracts.members()["helper"].profile_at(0); profile.model_epoch += 1;
    let changed_contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(profile, 9, b"Review".to_vec()).unwrap())])).unwrap();
    let changed = inputs(&action, &changed_contracts); assert_eq!(original.views()["helper"].submitted_bytes(), changed.views()["helper"].submitted_bytes());
    assert_eq!(broker.record_inputs(1, 1, changed), Err(Error::Binding)); assert_eq!(broker.input_revision(1), Ok(1)); approve(&mut broker, 1, 11, &original);
}
#[test]
fn declared_memory_gap_and_wrong_projection_refuse_but_closed_absence_is_usable() {
    let action = FrozenAction::freeze(spec(0)).unwrap(); let contracts = contracts(); let helper = &contracts.members()["helper"];
    for omission in [Omission::Gapped { domain_id: 1, first_missing: 1 }, Omission::Unsupported { domain_id: 1 },
        Omission::Redacted { domain_id: 1, transform_id: 1 }, Omission::ClosedAbsent { domain_id: 1, trusted_closure_marker_id: 1 }]
    {
        let closed = matches!(omission, Omission::ClosedAbsent { .. }); let view = manifest(&action, helper, vec![omission], 9);
        let result = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("helper".to_owned(), view)]));
        if closed { assert!(result.is_ok()); } else { assert_eq!(result, Err(Error::Incomplete)); }
    }
    let wrong = manifest(&action, helper, Vec::new(), 10);
    assert_eq!(CommitteeInput::capture(&action, &contracts, BTreeMap::from([("helper".to_owned(), wrong)])), Err(Error::Binding));
}
#[test]
fn policy_rotation_invalidates_old_reviews_but_allows_new_epoch_views() {
    let (mut broker, mut endpoint, contracts) = fixture(); let (_, old_input) = prepare(&mut broker, &contracts, 1);
    let session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap(); let review = complete(session, 1);
    broker.replace_policy(0, 0, policy(2)).unwrap(); assert_eq!(broker.apply_review(review, Some(&old_input), &snapshot()), Err(Error::Stale));
    let (action, input) = prepare(&mut broker, &contracts, 2); assert_eq!(input.views()["helper"].input_profile().policy_epoch, 1);
    approve(&mut broker, 2, 12, &input); let permit = broker.authorize(2, Some(&input), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap(); broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap(); assert_eq!(endpoint.execution_count(), 1);
}
#[test]
fn reset_requires_new_view_and_review_without_restoring_an_old_permit() {
    let (mut broker, mut endpoint, contracts) = fixture(); let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let (old_action, old_input) = prepare(&mut broker, &contracts, 1); approve(&mut broker, 1, 11, &old_input);
    let old_permit = broker.authorize(1, Some(&old_input), &snapshot()).unwrap(); let before = broker.inspect();
    broker.reset(ResetRequest { checkpoint, expected_control_sequence: before.sequence, expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 }, retained_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }).unwrap();
    assert!(broker.dispatch(&old_permit, &old_action, Some(&old_input), &snapshot()).is_err()); assert_eq!(broker.inspect().ledger.available, 100);
    let (action, input) = prepare(&mut broker, &contracts, 2); approve(&mut broker, 2, 12, &input);
    let permit = broker.authorize(2, Some(&input), &snapshot()).unwrap(); let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap(); broker.accept_receipt(receipt).unwrap(); assert_eq!(endpoint.execution_count(), 1);
}
#[test]
fn round_capacity_cannot_obstruct_cancellation_of_existing_reservations() {
    let (mut broker, _, contracts) = fixture(); let (_, input) = prepare(&mut broker, &contracts, 1); approve(&mut broker, 1, 1, &input);
    let _permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    for round in 2..=MAX_OBSERVED_ROUNDS { broker.begin_review(1, round as u64, [1; 32], window(), &snapshot()).unwrap(); }
    assert_eq!(broker.begin_review(1, MAX_OBSERVED_ROUNDS as u64 + 1, [1; 32], window(), &snapshot()).unwrap_err(), Error::Limit);
    broker.cancel(1).unwrap(); assert_eq!(broker.inspect().ledger.available, 100); assert_eq!(broker.inspect().ledger.reserved, 0);
}
#[test]
fn invalid_roster_does_not_consume_the_endpoint_attachment() {
    let contracts = contracts(); let wrong = CommitteeContract::new(BTreeMap::from([("other".to_owned(), contracts.members()["helper"].clone())])).unwrap();
    let mut endpoint = PublicationEndpoint::new(spec(0).target.unwrap(), Vec::new(), 200, 8).unwrap();
    assert_eq!(OversightBroker::new(config(), &mut endpoint, wrong).unwrap_err(), Error::Binding); OversightBroker::new(config(), &mut endpoint, contracts).unwrap();
}
#[test]
fn an_exact_policy_violation_still_denies_without_waiting_for_helper_inputs() {
    let (mut broker, _, _) = fixture(); let mut violation = snapshot(); violation.values.insert(7, vec![8]);
    let proposal = broker.propose(1, spec(0), &violation).unwrap(); assert_eq!(proposal.state, ActionState::Denied); assert_eq!(broker.input_revision(1), Ok(0));
    assert_eq!(broker.inspect().ledger.available, 100); assert_eq!(broker.authorize(1, None, &violation).unwrap_err(), Error::Incomplete);
}
