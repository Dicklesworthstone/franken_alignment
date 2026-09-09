//! Boundary controls for the endpoint's restricted view and byte budget.

use fa_reference::action::consequence::delivery::{DeliveryBroker, PublicationEndpoint};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::{Error, Snapshot};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

const SECRET: &[u8] = b"INTERNAL_POLICY_WITNESS_MUST_STAY_INSIDE_THE_CONTROLLER";

fn fixture(retention: u64) -> (DeliveryBroker, PublicationEndpoint, FrozenAction, Permit, Snapshot) {
    let target = ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
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
        scope, total: 10, max_attempts: 4, actor, suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: SECRET.to_vec() }]).unwrap(),
        congress, narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(target, Vec::new(), retention, 4).unwrap();
    let mut broker = DeliveryBroker::new(config, &mut endpoint).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, SECRET.to_vec())]) };
    let proposal = broker.propose(1, ActionSpec {
        version: VERSION, scope, target: Some(target), payload: b"public".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 6,
    }, &snapshot).unwrap();
    let mut session = broker.begin_review(1, 1, [1; 32], &snapshot).unwrap();
    let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", commitment).unwrap();
    session.open_reveals().unwrap();
    session.reveal("helper", Verdict::Allow, b"salt").unwrap();
    broker.apply_review(session.finish().unwrap(), &snapshot).unwrap();
    let permit = broker.authorize(1, &snapshot).unwrap();
    (broker, endpoint, proposal.action, permit, snapshot)
}

#[test]
fn endpoint_requests_queries_and_receipts_exclude_internal_witness_bytes() {
    let (mut broker, mut endpoint, action, permit, snapshot) = fixture(200);
    let secret_rendering = format!("{:?}", SECRET);
    assert!(format!("{action:?}").contains(&secret_rendering));
    let message = broker.dispatch(&permit, &action, &snapshot).unwrap();
    assert_eq!(message.request().payload(), b"public");
    assert_eq!(message.request().scope(), action.spec().scope);
    assert_eq!(message.request().target(), action.spec().target.unwrap());
    assert_eq!(message.request().units(), 6);
    assert_eq!(message.request().policy_epoch(), 0);
    assert_eq!(message.request().deadline(), ElapsedTick(100));
    assert!(!format!("{message:?}").contains(&secret_rendering));
    assert!(!format!("{:?}", broker.status_query(1).unwrap()).contains(&secret_rendering));
    let receipt = endpoint.deliver(&message).unwrap();
    assert!(!format!("{receipt:?}").contains(&secret_rendering));
    assert!(!format!("{endpoint:?}").contains(&secret_rendering));
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.charged, 6);
    assert!(format!("{:?}", broker.controller().review_receipts()).contains(&secret_rendering));
}

#[test]
fn overflowing_retention_refuses_before_consuming_the_authorized_permit() {
    let (mut broker, endpoint, action, permit, snapshot) = fixture(u64::MAX);
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot).unwrap_err(), Error::Overflow);
    assert_eq!(broker.inspect(), before);
    assert_eq!(endpoint.execution_count(), 0);
    assert!(broker.pending_reconciliation().unwrap().is_empty());
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 10);
}

#[test]
fn same_content_receipts_from_different_endpoint_instances_are_not_equal() {
    let (mut first, mut left, action, permit, snapshot) = fixture(200);
    let (mut second, mut right, other, other_permit, other_snapshot) = fixture(200);
    let left_message = first.dispatch(&permit, &action, &snapshot).unwrap();
    let right_message = second.dispatch(&other_permit, &other, &other_snapshot).unwrap();
    let left_receipt = left.deliver(&left_message).unwrap();
    let right_receipt = right.deliver(&right_message).unwrap();
    assert_eq!(left_receipt.request(), right_receipt.request());
    assert_eq!(left_receipt.outcome(), right_receipt.outcome());
    assert_ne!(left_receipt, right_receipt);
    assert_eq!(second.accept_receipt(left_receipt), Err(Error::Binding));
    second.accept_receipt(right_receipt).unwrap();
}
