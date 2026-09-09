//! Interruption/fence/delayed-delivery campaigns through the public API.
//! All assertions are about the declared surviving-state reference profile.

use fa_reference::action::consequence::delivery::{
    DeliveryBroker, DispatchEnvelope, EndpointOutcome, EndpointStatus, PublicationEndpoint,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::{Error, Snapshot};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

fn initial_target() -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: 1, generation: 4 }
}

fn spec(target: ResolvedTarget, epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(target), payload: b"new".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: epoch, deadline: ElapsedTick(100), units: 3,
    }
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

fn fixture(limit: usize) -> (DeliveryBroker, PublicationEndpoint) {
    let actor = ActorState::new(RestartProfile {
        id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
    }, vec![1], vec![2], vec![3], 1).unwrap();
    let congress = CongressPolicy {
        generation: 1,
        members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
        caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
        continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
    };
    let config = ControllerConfig {
        scope: spec(initial_target(), 0).scope, total: 12, max_attempts: 32, actor,
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress, narrowed_targets: TargetCeiling::new(&[initial_target()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(initial_target(), b"old".to_vec(), 200, limit).unwrap();
    let mut broker = DeliveryBroker::new(config, &mut endpoint).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    (broker, endpoint)
}

fn authorize(broker: &mut DeliveryBroker, id: u64, target: ResolvedTarget) -> (FrozenAction, Permit) {
    let proposal = broker.propose(id, spec(target, broker.inspect().ledger.epoch), &snapshot()).unwrap();
    let mut session = broker.begin_review(id, id, [1; 32], &snapshot()).unwrap();
    let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", commitment).unwrap();
    session.open_reveals().unwrap();
    session.reveal("helper", Verdict::Allow, b"salt").unwrap();
    broker.apply_review(session.finish().unwrap(), &snapshot()).unwrap();
    let permit = broker.authorize(id, &snapshot()).unwrap();
    (proposal.action, permit)
}

fn dispatch(broker: &mut DeliveryBroker, id: u64, target: ResolvedTarget) -> DispatchEnvelope {
    let (action, permit) = authorize(broker, id, target);
    broker.dispatch(&permit, &action, &snapshot()).unwrap()
}

fn check_accounting(broker: &DeliveryBroker, endpoint: &PublicationEndpoint) {
    let state = broker.inspect();
    assert_eq!(state.ledger.available + state.ledger.reserved + state.ledger.charged, 12);
    assert_eq!(state.ledger.charged, 3 * endpoint.execution_count());
}

#[test]
fn all_delayed_delivery_fence_and_seal_orders_reconcile_without_double_spending() {
    let schedules = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for schedule in schedules {
        let (mut broker, mut endpoint) = fixture(16);
        let old_message = dispatch(&mut broker, 1, initial_target());
        let fence = broker.restart_dispatcher().unwrap();
        let query = broker.status_query(1).unwrap();
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
        assert_eq!(broker.inspect().ledger.available, 9);
        for operation in schedule {
            match operation {
                0 => {
                    let outcome = endpoint.deliver(&old_message);
                    if broker.fence_confirmed() {
                        assert_eq!(outcome, Err(Error::Stale));
                    } else {
                        assert!(outcome.is_ok());
                    }
                }
                1 => {
                    let acknowledgment = endpoint.install_fence(fence.clone()).unwrap();
                    broker.confirm_fence(acknowledgment).unwrap();
                }
                2 => {
                    let result = endpoint.seal_unexecuted(&query);
                    if broker.fence_confirmed() {
                        broker.accept_receipt(result.unwrap()).unwrap();
                    } else {
                        assert_eq!(result, Err(Error::Stale));
                    }
                }
                _ => unreachable!(),
            }
        }
        let receipt = endpoint.seal_unexecuted(&query).unwrap();
        broker.accept_receipt(receipt.clone()).unwrap();
        assert!(!broker.accept_receipt(receipt).unwrap());
        assert!(endpoint.execution_count() <= 1);
        assert!(broker.pending_reconciliation().unwrap().is_empty());
        check_accounting(&broker, &endpoint);
        let previous_executions = endpoint.execution_count();
        let fresh = dispatch(&mut broker, 2, endpoint.target());
        broker.accept_receipt(endpoint.deliver(&fresh).unwrap()).unwrap();
        assert_eq!(endpoint.execution_count(), previous_executions + 1);
        check_accounting(&broker, &endpoint);
    }
}

#[test]
fn interruption_before_dispatch_keeps_reservation_and_requires_fence_acknowledgment() {
    let (mut broker, mut endpoint) = fixture(16);
    let (action, permit) = authorize(&mut broker, 1, initial_target());
    let before = broker.inspect();
    let fence = broker.restart_dispatcher().unwrap();
    assert_eq!(broker.inspect(), before);
    assert!(broker.pending_reconciliation().unwrap().is_empty());
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let message = broker.dispatch(&permit, &action, &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    check_accounting(&broker, &endpoint);
}

#[test]
fn restart_after_remote_commit_recovers_receipt_without_republishing() {
    let (mut broker, mut endpoint) = fixture(16);
    let message = dispatch(&mut broker, 1, initial_target());
    endpoint.deliver(&message).unwrap();
    let fence = broker.restart_dispatcher().unwrap();
    let queries = broker.pending_reconciliation().unwrap();
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].attempt(), 1);
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let status = endpoint.status(&queries[0]).unwrap();
    broker.reconcile_status(&queries[0], status).unwrap();
    assert_eq!(endpoint.deliver(&message), Err(Error::Stale));
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    let before = broker.inspect();
    broker.restart_dispatcher().unwrap();
    assert_eq!(broker.inspect(), before);
    assert!(broker.pending_reconciliation().unwrap().is_empty());
}

#[test]
fn stale_fence_acknowledgment_and_stale_queries_cannot_end_a_new_recovery() {
    let (mut broker, mut endpoint) = fixture(16);
    dispatch(&mut broker, 1, initial_target());
    let first_fence = broker.restart_dispatcher().unwrap();
    let first_query = broker.status_query(1).unwrap();
    let first_ack = endpoint.install_fence(first_fence.clone()).unwrap();
    let second_fence = broker.restart_dispatcher().unwrap();
    assert_eq!(broker.confirm_fence(first_ack), Err(Error::Stale));
    assert!(!broker.fence_confirmed());
    broker.confirm_fence(endpoint.install_fence(second_fence).unwrap()).unwrap();
    assert!(endpoint.install_fence(first_fence).is_err());
    assert_eq!(broker.reconcile_status(&first_query, EndpointStatus::AwaitingResolution), Err(Error::Stale));
    let current = broker.status_query(1).unwrap();
    broker.accept_receipt(endpoint.seal_unexecuted(&current).unwrap()).unwrap();
    assert_eq!(broker.inspect().ledger.available, 12);
}

#[test]
fn status_responses_are_bound_to_the_requested_effect_not_just_the_endpoint() {
    let (mut broker, mut endpoint) = fixture(16);
    let first = dispatch(&mut broker, 1, initial_target());
    let second = dispatch(&mut broker, 2, initial_target());
    let first_receipt = endpoint.deliver(&first).unwrap();
    let second_query = broker.status_query(2).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.reconcile_status(&second_query, EndpointStatus::Resolved(first_receipt.clone())), Err(Error::Binding));
    assert_eq!(broker.inspect(), before);
    broker.accept_receipt(first_receipt).unwrap();
    broker.accept_receipt(endpoint.deliver(&second).unwrap()).unwrap();
    check_accounting(&broker, &endpoint);
}

#[test]
fn retention_expiry_never_refunds_and_irrecoverable_effects_stay_charged() {
    let (mut broker, mut endpoint) = fixture(16);
    let message = dispatch(&mut broker, 1, initial_target());
    let receipt = endpoint.deliver(&message).unwrap();
    endpoint.observe_time(message.retained_until()).unwrap();
    let query = broker.status_query(1).unwrap();
    broker.reconcile_status(&query, endpoint.status(&query).unwrap()).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.available, 9);
    broker.abandon_unknown(1).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.accept_receipt(receipt), Err(Error::WrongState));
    broker.restart_dispatcher().unwrap();
    assert_eq!(broker.inspect(), before);
    assert!(broker.pending_reconciliation().unwrap().is_empty());
}

#[test]
fn expired_action_can_be_terminally_rejected_inside_the_longer_status_window() {
    let (mut broker, mut endpoint) = fixture(16);
    let message = dispatch(&mut broker, 1, initial_target());
    endpoint.observe_time(ElapsedTick(100)).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    assert!(matches!(receipt.outcome(), EndpointOutcome::NotExecuted { .. }));
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(broker.inspect().ledger.available, 12);
}

#[test]
fn pending_capacity_is_reserved_before_permit_consumption_and_does_not_block_reconciliation() {
    let (mut broker, mut endpoint) = fixture(1);
    let pending = dispatch(&mut broker, 1, initial_target());
    let (action, permit) = authorize(&mut broker, 2, initial_target());
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Limit);
    assert_eq!(broker.inspect(), before);
    broker.cancel(2).unwrap();
    let fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let query = broker.status_query(1).unwrap();
    broker.accept_receipt(endpoint.seal_unexecuted(&query).unwrap()).unwrap();
    assert_eq!(endpoint.deliver(&pending), Err(Error::Stale));
    assert_eq!(broker.inspect().ledger.available, 12);
}

#[test]
fn policy_rotation_and_actor_reset_do_not_erase_a_remote_execution() {
    let (mut broker, mut endpoint) = fixture(16);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let message = dispatch(&mut broker, 1, initial_target());
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let state = broker.inspect();
    let policy = Policy::new(2, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap();
    broker.replace_policy(state.sequence, state.ledger.epoch, policy).unwrap();
    let reset = broker.reset(ResetRequest {
        checkpoint,
        expected_control_sequence: broker.inspect().sequence,
        expected_actor_revision: broker.controller().actor_revision(),
        binding: ReviewBinding { round: 100, evidence_root: [2; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[endpoint.target()]).unwrap(),
    }).unwrap();
    assert!(reset.restored);
    assert_eq!(reset.refunded_units, 0);
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.controller().policy().generation(), 2);
    assert_eq!(broker.inspect().ledger.charged, 3);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    let fresh = dispatch(&mut broker, 2, endpoint.target());
    broker.accept_receipt(endpoint.deliver(&fresh).unwrap()).unwrap();
    check_accounting(&broker, &endpoint);
}

#[test]
fn byte_budget_and_registered_target_refuse_before_creating_an_attempt() {
    let (mut broker, _endpoint) = fixture(16);
    let mut underfunded = spec(initial_target(), 0);
    underfunded.units = 2;
    assert_eq!(broker.propose(1, underfunded, &snapshot()), Err(Error::Limit));
    let mut foreign = initial_target();
    foreign.generation += 1;
    assert_eq!(broker.propose(1, spec(foreign, 0), &snapshot()), Err(Error::Binding));
    assert!(broker.inspect().ledger.stages.is_empty());
    authorize(&mut broker, 1, initial_target());
    assert_eq!(broker.inspect().ledger.reserved, 3);
}
