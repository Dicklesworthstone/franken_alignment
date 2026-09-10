//! Human-key revocation, input changes and recovery do not rewind real effects.
//! These are public in-memory protocol cases, not OS crash or identity tests.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanPermit, HumanRequest, HumanReviewer, HumanReviewPolicy, MAX_HUMAN_INPUT_BYTES, MAX_HUMAN_REQUESTS};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart, MAX_SUBMITTED_BYTES};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64, version: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: version, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: epoch, deadline: ElapsedTick(100), units: 16,
    }
}
fn policy(generation: u64) -> Policy {
    Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap()
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn fixture(limit: usize) -> (OversightBroker, PublicationEndpoint, CommitteeContract, HumanReviewer) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"v1".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope: spec(0, 1).scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1, tokenizer_generation: 1,
            state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: policy(1),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[spec(0, 1).target.unwrap()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(spec(0, 1).target.unwrap(), b"old".to_vec(), 200, 16).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    let reviewer = broker.enable_human_review(HumanReviewPolicy { reviewer_id: 90, max_validity_ticks: 20, max_requests: limit }).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let ack = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(ack).unwrap();
    (broker, endpoint, contracts, reviewer)
}
fn input(action: &FrozenAction, contracts: &CommitteeContract, padding: usize) -> CommitteeInput {
    let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(action);
    let end = bytes.len();
    bytes.extend_from_slice(helper.question());
    let question_end = bytes.len();
    let mut parts = vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: question_end }, kind: PartKind::Question },
    ];
    if padding > 0 {
        bytes.resize(question_end + padding, b'x');
        parts.push(SubmittedPart { span: ByteSpan { start: question_end, end: bytes.len() }, kind: PartKind::Prompt });
    }
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), parts, vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), manifest)])).unwrap()
}
fn review(broker: &mut OversightBroker, id: u64, round: u64, inputs: &CommitteeInput) {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let vote = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", vote, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    let completed = session.finish(now).unwrap();
    broker.apply_review(completed, Some(inputs), &snapshot()).unwrap();
}
fn prepare(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, version: u64) -> (FrozenAction, CommitteeInput) {
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch, version), &snapshot()).unwrap();
    let inputs = input(&proposal.action, contracts, 0);
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    review(broker, id, id + 10, &inputs);
    (proposal.action, inputs)
}
fn keys(broker: &mut OversightBroker, reviewer: &HumanReviewer, id: u64, inputs: &CommitteeInput) -> (Permit, HumanRequest, HumanPermit) {
    let automatic = broker.authorize(id, Some(inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(id + 100, id, Some(inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, broker.inspect().ledger.elapsed.unwrap()).unwrap();
    (automatic, request, human)
}

#[test]
fn bulk_withdrawal_revokes_outstanding_keys_but_not_a_dispatched_effect() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (first, first_input) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, used_request, human) = keys(&mut broker, &reviewer, 1, &first_input);
    let message = broker.dispatch_with_human(&automatic, &human, &first, Some(&first_input), &snapshot()).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let (_, pending_input) = prepare(&mut broker, &contracts, 2, 1);
    let pending = broker.request_human_approval(102, 2, Some(&pending_input), ElapsedTick(10)).unwrap();
    let (third, third_input) = prepare(&mut broker, &contracts, 3, 1);
    let (third_automatic, _, third_human) = keys(&mut broker, &reviewer, 3, &third_input);
    let before = broker.inspect();
    let withdrawal = reviewer.revoke_all(ElapsedTick(1)).unwrap();
    assert_eq!(withdrawal.requests, vec![102, 103]);
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(reviewer.revoke(&used_request, ElapsedTick(1)), Err(Error::WrongState));
    assert_eq!(reviewer.approve(&pending, ElapsedTick(1)).unwrap_err(), Error::WrongState);
    assert_eq!(broker.dispatch_with_human(&third_automatic, &third_human, &third, Some(&third_input), &snapshot()).unwrap_err(), Error::WrongState);
    assert!(reviewer.revoke_all(ElapsedTick(1)).unwrap().requests.is_empty());
    broker.cancel(2).unwrap();
    broker.cancel(3).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.inspect().ledger.available, 84);
}

#[test]
fn withdrawal_requires_fresh_congress_context_before_new_human_approval() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, _, old) = keys(&mut broker, &reviewer, 1, &inputs);
    reviewer.revoke_all(ElapsedTick(1)).unwrap();
    assert_eq!(broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(10)).unwrap_err(), Error::Duplicate);
    review(&mut broker, 1, 12, &inputs);
    let request = broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let fresh = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    assert_eq!(broker.dispatch_with_human(&automatic, &old, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    let message = broker.dispatch_with_human(&automatic, &fresh, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(broker.inspect().ledger.charged, 16);
}

#[test]
fn dispatcher_recovery_preserves_unspent_key_and_sealing_never_reissues_a_spent_key() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 1, &inputs);
    let fence = broker.restart_dispatcher().unwrap();
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(broker.inspect().ledger.reserved, 16);
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    let fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let query = broker.status_query(1).unwrap();
    let prevented = endpoint.seal_unexecuted(&query).unwrap();
    broker.accept_receipt(prevented).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(endpoint.deliver(&message).unwrap_err(), Error::Stale);
    assert!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).is_err());
    let (fresh, fresh_input) = prepare(&mut broker, &contracts, 2, 1);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 2, &fresh_input);
    let message = broker.dispatch_with_human(&automatic, &human, &fresh, Some(&fresh_input), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn identical_recapture_and_reapproval_do_not_resurrect_the_old_human_key() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, old_request, human) = keys(&mut broker, &reviewer, 1, &inputs);
    broker.inputs_unavailable(1, 1).unwrap();
    assert!(broker.dispatch_with_human(&automatic, &human, &action, None, &snapshot()).is_err());
    broker.record_inputs(1, 2, inputs.clone()).unwrap();
    review(&mut broker, 1, 12, &inputs);
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(old_request.input_revision(), 1);
    let request = broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    assert_eq!(request.input_revision(), 3);
    let fresh = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = broker.dispatch_with_human(&automatic, &fresh, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn reset_preserves_two_key_mode_and_unknown_receipts_while_fencing_old_keys() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let (first, first_input) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 1, &first_input);
    let message = broker.dispatch_with_human(&automatic, &human, &first, Some(&first_input), &snapshot()).unwrap();
    let executed = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let (second, second_input) = prepare(&mut broker, &contracts, 2, 2);
    let (old_automatic, _, old_human) = keys(&mut broker, &reviewer, 2, &second_input);
    let before = broker.inspect();
    broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: before.sequence, expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[endpoint.target()]).unwrap(),
    }).unwrap();
    assert!(broker.human_review_required());
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert!(broker.dispatch_with_human(&old_automatic, &old_human, &second, Some(&second_input), &snapshot()).is_err());
    reviewer.revoke_all(ElapsedTick(1)).unwrap();
    broker.accept_receipt(executed).unwrap();
    let (fresh, fresh_input) = prepare(&mut broker, &contracts, 3, 2);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 3, &fresh_input);
    assert_eq!(broker.dispatch(&automatic, &fresh, Some(&fresh_input), &snapshot()).unwrap_err(), Error::Incomplete);
    let message = broker.dispatch_with_human(&automatic, &human, &fresh, Some(&fresh_input), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 2);
    assert_eq!(broker.inspect().ledger.charged, 32);
}

#[test]
fn policy_replacement_requires_both_new_keys_and_retains_historical_human_basis() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (old_action, old_inputs) = prepare(&mut broker, &contracts, 1, 1);
    let (old_automatic, request, old_human) = keys(&mut broker, &reviewer, 1, &old_inputs);
    let before = broker.inspect();
    broker.replace_policy(before.sequence, before.ledger.epoch, policy(2)).unwrap();
    assert!(broker.dispatch_with_human(&old_automatic, &old_human, &old_action, Some(&old_inputs), &snapshot()).is_err());
    let retained = broker.human_request(request.id()).unwrap();
    assert_eq!(retained.action(), &old_action);
    assert_eq!(retained.policy_generation(), 1);
    assert_eq!(reviewer.approve(&retained, ElapsedTick(1)).unwrap_err(), Error::WrongState);
    let (fresh, inputs) = prepare(&mut broker, &contracts, 2, 1);
    let (automatic, request, human) = keys(&mut broker, &reviewer, 2, &inputs);
    assert_eq!(request.policy_generation(), 2);
    assert_eq!(fresh.spec().policy_epoch, 1);
    let message = broker.dispatch_with_human(&automatic, &human, &fresh, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn lifetime_request_quota_does_not_block_cancellation_or_terminal_reconciliation() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(1);
    let (first, first_input) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 1, &first_input);
    let message = broker.dispatch_with_human(&automatic, &human, &first, Some(&first_input), &snapshot()).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let (_, other_input) = prepare(&mut broker, &contracts, 2, 1);
    let _reserved = broker.authorize(2, Some(&other_input), &snapshot()).unwrap();
    assert_eq!(broker.request_human_approval(102, 2, Some(&other_input), ElapsedTick(10)).unwrap_err(), Error::Limit);
    let before = broker.inspect();
    assert_eq!(before.ledger.reserved, 16);
    assert_eq!(before.ledger.charged, 16);
    let query = broker.status_query(1).unwrap();
    let receipt = endpoint.seal_unexecuted(&query).unwrap();
    broker.accept_receipt(receipt).unwrap();
    broker.cancel(2).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
    assert!(endpoint.deliver(&message).is_ok());
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn retained_view_limit_refuses_before_a_new_request_is_inserted() {
    let (mut broker, _, contracts, reviewer) = fixture(MAX_HUMAN_REQUESTS);
    let proposal = broker.propose(1, spec(0, 1), &snapshot()).unwrap();
    let padding = MAX_SUBMITTED_BYTES - action_frame(&proposal.action).len() - contracts.members()["helper"].question().len();
    let inputs = input(&proposal.action, &contracts, padding);
    broker.record_inputs(1, 0, inputs.clone()).unwrap();
    let capacity = MAX_HUMAN_INPUT_BYTES / inputs.logical_bytes();
    assert!(capacity > 0 && capacity < MAX_HUMAN_REQUESTS);
    for index in 0..capacity {
        review(&mut broker, 1, index as u64 + 1, &inputs);
        broker.request_human_approval(index as u64 + 1_000, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    }
    review(&mut broker, 1, capacity as u64 + 1, &inputs);
    let before = broker.inspect();
    let next = capacity as u64 + 1_000;
    assert_eq!(broker.request_human_approval(next, 1, Some(&inputs), ElapsedTick(10)).unwrap_err(), Error::Limit);
    assert_eq!(broker.human_status(next), Err(Error::Missing));
    assert_eq!(broker.inspect(), before);
    assert_eq!(reviewer.revoke_all(ElapsedTick(1)).unwrap().requests.len(), capacity);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn changed_execution_fields_do_not_spend_the_matching_original_keys() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let (automatic, _, human) = keys(&mut broker, &reviewer, 1, &inputs);
    let before = broker.inspect();
    for field in 0..5 {
        let mut spec = action.spec().clone();
        match field {
            0 => spec.payload.push(b'!'),
            1 => spec.target.as_mut().unwrap().expected_version += 1,
            2 => spec.units += 1,
            3 => spec.deadline.0 += 1,
            _ => spec.scope.branch += 1,
        }
        let altered = FrozenAction::freeze(spec).unwrap();
        assert_eq!(broker.dispatch_with_human(&automatic, &human, &altered, Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
        assert_eq!(broker.inspect(), before);
        assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
    }
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn future_human_approval_waits_for_the_controllers_own_clock_observation() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(3)).unwrap();
    let before = broker.inspect();
    assert_eq!(reviewer.revoke(&request, ElapsedTick(2)), Err(Error::Stale));
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before);
    broker.observe_time(ElapsedTick(3)).unwrap();
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(broker.human_status(101).unwrap().finished_at, Some(ElapsedTick(3)));
}

#[test]
fn request_bounds_and_expired_issuance_cannot_extend_a_human_deadline() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture(16);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, 1);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    for (id, expiry, expected) in [(0, 10, Error::InvalidInput), (101, 1, Error::Stale), (101, 22, Error::Limit), (101, 101, Error::InvalidInput)] {
        assert_eq!(broker.request_human_approval(id, 1, Some(&inputs), ElapsedTick(expiry)).unwrap_err(), expected);
    }
    let expired = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(2)).unwrap();
    assert_eq!(reviewer.approve(&expired, ElapsedTick(2)).unwrap_err(), Error::Stale);
    assert_eq!(reviewer.approve(&expired, ElapsedTick(1)).unwrap_err(), Error::Stale);
    broker.observe_time(ElapsedTick(2)).unwrap();
    review(&mut broker, 1, 12, &inputs);
    let request = broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(5)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(2)).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}
