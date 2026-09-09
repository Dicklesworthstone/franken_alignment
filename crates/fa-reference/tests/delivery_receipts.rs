//! Public-API delivery and reconciliation through exact policy and congress.
//! The endpoint is a separate in-memory state machine, not a real remote API.

use fa_reference::action::consequence::delivery::{
    DeliveryBroker, DispatchEnvelope, EndpointOutcome, EndpointStatus, NonExecutionReason,
    PublicationEndpoint,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::{Error, Snapshot};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

fn target() -> ResolvedTarget {
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

fn setup() -> (DeliveryBroker, PublicationEndpoint) {
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
        scope: spec(target(), 0).scope, total: 12, max_attempts: 16, actor,
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(target(), b"old".to_vec(), 200, 16).unwrap();
    let mut broker = DeliveryBroker::new(config, &mut endpoint).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    (broker, endpoint)
}

fn dispatch(broker: &mut DeliveryBroker, id: u64, target: ResolvedTarget) -> DispatchEnvelope {
    let proposal = broker.propose(id, spec(target, broker.inspect().ledger.epoch), &snapshot()).unwrap();
    let mut session = broker.begin_review(id, id, [1; 32], &snapshot()).unwrap();
    let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", commitment).unwrap();
    session.open_reveals().unwrap();
    session.reveal("helper", Verdict::Allow, b"salt").unwrap();
    broker.apply_review(session.finish().unwrap(), &snapshot()).unwrap();
    let permit = broker.authorize(id, &snapshot()).unwrap();
    broker.dispatch(&permit, &proposal.action, &snapshot()).unwrap()
}

#[test]
fn keyed_publication_and_duplicate_acknowledgments_charge_once() {
    let (mut broker, mut endpoint) = setup();
    let message = dispatch(&mut broker, 1, target());
    assert_eq!(broker.inspect().ledger.charged, 3);
    assert_eq!(endpoint.payload(), b"old");
    let receipt = endpoint.deliver(&message).unwrap();
    assert_eq!(endpoint.payload(), b"new");
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(endpoint.deliver(&message).unwrap(), receipt);
    assert_eq!(endpoint.execution_count(), 1);
    assert!(broker.accept_receipt(receipt.clone()).unwrap());
    assert!(!broker.accept_receipt(receipt).unwrap());
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.available, 9);
}

#[test]
fn lost_execution_acknowledgment_is_recovered_by_status_not_reexecution() {
    let (mut broker, mut endpoint) = setup();
    let message = dispatch(&mut broker, 1, target());
    let _lost = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    assert_eq!(broker.cancel(1), Err(Error::WrongState));
    let EndpointStatus::Resolved(receipt) = endpoint.status(&broker.status_query(1).unwrap()).unwrap() else {
        panic!("retained execution must be queryable");
    };
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.charged, 3);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn missing_status_does_not_refund_but_atomic_seal_blocks_delayed_publication() {
    let (mut broker, mut endpoint) = setup();
    let delayed = dispatch(&mut broker, 1, target());
    broker.acknowledgment_lost(1).unwrap();
    let query = broker.status_query(1).unwrap();
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    assert_eq!(broker.inspect().ledger.available, 9);
    let proof = endpoint.seal_unexecuted(&query).unwrap();
    assert_eq!(proof.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert!(broker.accept_receipt(proof.clone()).unwrap());
    assert_eq!(endpoint.deliver(&delayed).unwrap(), proof);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(endpoint.payload(), b"old");
    assert_eq!(broker.inspect().ledger.available, 12);
    assert!(!broker.accept_receipt(proof).unwrap());
    let fresh = dispatch(&mut broker, 2, endpoint.target());
    broker.accept_receipt(endpoint.deliver(&fresh).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn execution_winning_the_seal_race_cannot_be_refunded() {
    let (mut broker, mut endpoint) = setup();
    let message = dispatch(&mut broker, 1, target());
    endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let receipt = endpoint.seal_unexecuted(&broker.status_query(1).unwrap()).unwrap();
    assert!(matches!(receipt.outcome(), EndpointOutcome::Executed { .. }));
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.available, 9);
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn remote_version_precondition_is_checked_at_the_effect_boundary() {
    let (mut broker, mut endpoint) = setup();
    let first = dispatch(&mut broker, 1, target());
    let second = dispatch(&mut broker, 2, target());
    broker.accept_receipt(endpoint.deliver(&first).unwrap()).unwrap();
    let refused = endpoint.deliver(&second).unwrap();
    assert_eq!(refused.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::VersionConflict });
    broker.accept_receipt(refused).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.available, 9);
    assert_eq!(broker.inspect().ledger.stages[&2], ActionState::ConfirmedNotExecuted);
}

#[test]
fn retention_loss_is_unknown_while_an_already_retained_receipt_remains_evidence() {
    let (mut broker, mut endpoint) = setup();
    let message = dispatch(&mut broker, 1, target());
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    endpoint.observe_time(message.retained_until()).unwrap();
    let query = broker.status_query(1).unwrap();
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::RetentionExpired);
    assert_eq!(endpoint.seal_unexecuted(&query), Err(Error::Stale));
    assert_eq!(endpoint.deliver(&message), Err(Error::Stale));
    assert_eq!(broker.inspect().ledger.available, 9);
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn foreign_receipts_queries_and_messages_cannot_cross_endpoints() {
    let (mut first, mut first_endpoint) = setup();
    let (mut second, mut second_endpoint) = setup();
    let message = dispatch(&mut first, 1, target());
    let other = dispatch(&mut second, 1, target());
    let receipt = first_endpoint.deliver(&message).unwrap();
    let before = second.inspect();
    assert_eq!(second.accept_receipt(receipt), Err(Error::Binding));
    assert_eq!(second_endpoint.deliver(&message), Err(Error::Binding));
    assert_eq!(second_endpoint.status(&first.status_query(1).unwrap()), Err(Error::Binding));
    assert_eq!(second.inspect(), before);
    second.accept_receipt(second_endpoint.deliver(&other).unwrap()).unwrap();
}
