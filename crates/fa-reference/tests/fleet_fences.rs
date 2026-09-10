//! Actual reference permits and endpoint outcomes, not asserted fence reports.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{DeliveryBroker, EndpointOutcome, PublicationEndpoint};
use fa_reference::action::consequence::delivery::fleet::{FleetCoordinator, FleetDeliveryKnowledge, FleetScope, FencePropagation};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

fn spec(domain: u64, tenant: u64, epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION, scope: Scope { tenant, principal: domain, run: 1, branch: 1, authority: domain, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: domain, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 16,
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) } }
fn fixture(domain: u64, tenant: u64) -> (DeliveryBroker, PublicationEndpoint) {
    let action = spec(domain, tenant, 0);
    let config = ControllerConfig {
        scope: action.scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[action.target.unwrap()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(action.target.unwrap(), b"old".to_vec(), 200, 16).unwrap();
    let mut broker = DeliveryBroker::new(config, &mut endpoint).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    (broker, endpoint)
}
fn fleet() -> FleetCoordinator { FleetCoordinator::new(4, 8, 100).unwrap() }
fn join(broker: &mut DeliveryBroker, fleet: &mut FleetCoordinator, domain: u64, until: u64) {
    let revision = fleet.revision(); broker.join_fleet(fleet, domain, revision, ElapsedTick(until)).unwrap();
}
fn ready(broker: &mut DeliveryBroker, domain: u64, tenant: u64, attempt: u64) -> (FrozenAction, Permit) {
    let action = broker.propose(attempt, spec(domain, tenant, broker.inspect().ledger.epoch), &snapshot()).unwrap().action;
    let mut session = broker.begin_review(attempt, attempt, [1; 32], &snapshot()).unwrap();
    let vote = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", vote).unwrap(); session.open_reveals().unwrap();
    session.reveal("helper", Verdict::Allow, b"salt").unwrap();
    broker.apply_review(session.finish().unwrap(), &snapshot()).unwrap();
    let permit = broker.authorize(attempt, &snapshot()).unwrap(); (action, permit)
}
fn conserve(broker: &DeliveryBroker) {
    let ledger = broker.inspect().ledger; assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
}

#[test]
fn issue_is_not_install_and_post_issue_admission_is_reconciled_not_hidden() {
    let mut fleet = fleet(); let (mut a, mut endpoint) = fixture(1, 1); let (mut b, _) = fixture(2, 2);
    join(&mut a, &mut fleet, 1, 50); join(&mut b, &mut fleet, 2, 50);
    let (action, permit) = ready(&mut a, 1, 1, 1);
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let initial = fleet.report(10).unwrap();
    assert!(matches!(initial.domains[&1].propagation, FencePropagation::AwaitingAcknowledgment { .. }));
    assert_eq!(initial.domains[&1].deliveries, FleetDeliveryKnowledge::Unobserved);
    let message = a.dispatch(&permit, &action, &snapshot()).unwrap();
    a.acknowledgment_lost(1).unwrap();
    let prior = a.inspect();
    let ack = a.install_fleet_fence(&command, prior.sequence, prior.ledger.epoch).unwrap();
    assert_eq!(ack.transition().refunded_units, 0);
    assert_eq!(a.inspect().ledger.stages[&1], ActionState::Unknown);
    fleet.acknowledge(ack).unwrap();
    fleet.observe_domain(a.fleet_observation().unwrap()).unwrap();
    let report = fleet.report(10).unwrap();
    match &report.domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue, unresolved, post_issue_admissions_complete, .. } => {
            assert!(*post_issue_admissions_complete); assert_eq!(post_issue.len(), 1); assert_eq!(unresolved.len(), 1);
            assert!(post_issue[0].admitted_order > command.issued_order()); assert_eq!(post_issue[0].outcome, None);
        }
        _ => panic!("domain was observed"),
    }
    assert!(matches!(report.domains[&2].propagation, FencePropagation::AwaitingAcknowledgment { .. }));
    // Local admission has stopped, but this previously admitted message still exists.
    a.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    fleet.observe_domain(a.fleet_observation().unwrap()).unwrap();
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue, unresolved, .. } => {
            assert!(unresolved.is_empty()); assert_eq!(post_issue[0].state, ActionState::Confirmed);
            assert_eq!(post_issue[0].outcome, Some(EndpointOutcome::Executed { resulting_version: 2 }));
        }
        _ => panic!("retained domain evidence"),
    }
    conserve(&a); conserve(&b);
}

#[test]
fn scoped_halt_cancels_reservations_once_and_does_not_stop_other_tenants() {
    let mut fleet = fleet(); let (mut a, _) = fixture(1, 1); let (mut b, mut other_endpoint) = fixture(2, 2);
    join(&mut a, &mut fleet, 1, 50); join(&mut b, &mut fleet, 2, 50);
    let (action, permit) = ready(&mut a, 1, 1, 1); let (other, other_permit) = ready(&mut b, 2, 2, 1);
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::Tenant(1)).unwrap();
    assert_eq!(command.domains().collect::<Vec<_>>(), vec![1]);
    let before = a.inspect();
    let ack = a.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(ack.transition().cancelled, vec![1]); assert_eq!(ack.transition().refunded_units, 16);
    let after = a.inspect(); assert!(after.suspended); assert_eq!(after.ledger.available, 100);
    assert_eq!(after.sequence, before.sequence + 1); assert_eq!(after.ledger.epoch, before.ledger.epoch + 1);
    assert_eq!(a.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap().transition(), ack.transition());
    assert_eq!(a.inspect(), after);
    assert_eq!(a.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(a.propose(2, spec(1, 1, after.ledger.epoch), &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(b.install_fleet_fence(&command, 1, 0).unwrap_err(), Error::Binding);
    let message = b.dispatch(&other_permit, &other, &snapshot()).unwrap();
    b.accept_receipt(other_endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(other_endpoint.execution_count(), 1); conserve(&a); conserve(&b);
}

#[test]
fn lost_ack_and_pre_stop_telemetry_never_imply_complete_propagation() {
    let mut fleet = fleet(); let (mut broker, _) = fixture(1, 1); join(&mut broker, &mut fleet, 1, 50);
    ready(&mut broker, 1, 1, 1);
    let old_snapshot = broker.fleet_observation().unwrap();
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let before = broker.inspect();
    let ack = broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    fleet.observe_domain(old_snapshot.clone()).unwrap();
    assert!(matches!(fleet.report(10).unwrap().domains[&1].propagation, FencePropagation::AwaitingAcknowledgment { .. }));
    assert_eq!(fleet.acknowledge(ack.clone()), Ok(true)); assert_eq!(fleet.acknowledge(ack), Ok(false));
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue_admissions_complete, .. } => assert!(!post_issue_admissions_complete),
        _ => panic!("observed old prefix"),
    }
    let current = broker.fleet_observation().unwrap();
    assert_eq!(fleet.observe_domain(current.clone()), Ok(true)); assert_eq!(fleet.observe_domain(current), Ok(false));
    assert_eq!(fleet.observe_domain(old_snapshot), Err(Error::Stale));
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue_admissions_complete, post_issue, .. } => {
            assert!(*post_issue_admissions_complete); assert!(post_issue.is_empty());
        }
        _ => panic!("observed closed admission prefix"),
    }
}

#[test]
fn lease_expiry_is_enforced_even_with_an_old_local_clock_and_no_ack() {
    let mut fleet = fleet(); let (mut broker, _) = fixture(1, 1); join(&mut broker, &mut fleet, 1, 10);
    let (action, permit) = ready(&mut broker, 1, 1, 1);
    fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    fleet.observe_time(ElapsedTick(10)).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.observe_time(ElapsedTick(9)), Err(Error::Stale));
    broker.observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Stale);
    let report = fleet.report(10).unwrap();
    assert_eq!(report.domains[&1].propagation, FencePropagation::LeaseExpired { at: ElapsedTick(10) });
    assert_eq!(report.domains[&1].deliveries, FleetDeliveryKnowledge::Unobserved);
    fleet.observe_domain(broker.fleet_observation().unwrap()).unwrap();
    match &fleet.report(10).unwrap().domains[&1].deliveries {
        FleetDeliveryKnowledge::Observed { post_issue_admissions_complete, .. } => assert!(*post_issue_admissions_complete),
        _ => panic!("observed after enforced expiry"),
    }
    assert_eq!(broker.inspect().ledger.reserved, 16); // Lease expiry is not a cancellation receipt.
    broker.cancel(1).unwrap(); assert_eq!(broker.inspect().ledger.available, 100); conserve(&broker);
}

#[test]
fn failed_dispatch_never_creates_an_admission_or_consumes_the_original_permit() {
    let mut fleet = fleet(); let (mut broker, mut endpoint) = fixture(1, 1); join(&mut broker, &mut fleet, 1, 50);
    let (action, permit) = ready(&mut broker, 1, 1, 1);
    let mut changed = snapshot(); changed.values.insert(7, vec![8]);
    let before = broker.inspect();
    assert!(broker.dispatch(&permit, &action, &changed).is_err());
    assert_eq!(broker.inspect(), before); assert!(broker.fleet_observation().unwrap().dispatches().is_empty());
    let message = broker.dispatch(&permit, &action, &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(broker.fleet_observation().unwrap().dispatches().len(), 1); conserve(&broker);
}

#[test]
fn higher_floors_fence_monotonically_and_foreign_messages_cannot_supply_acks() {
    let mut fleet = fleet(); let (mut broker, _) = fixture(1, 1); join(&mut broker, &mut fleet, 1, 50);
    let first = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let second = fleet.issue_fence(11, fleet.revision(), FleetScope::All).unwrap();
    for _ in 0..3 { broker.revoke_epoch().unwrap(); }
    let before = broker.inspect();
    assert_eq!(broker.install_fleet_fence(&second, before.sequence + 1, before.ledger.epoch).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before);
    let ack = broker.install_fleet_fence(&second, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(ack.transition().revocation_floor, 4);
    assert_eq!(broker.install_fleet_fence(&first, before.sequence, before.ledger.epoch).unwrap_err(), Error::Stale);
    fleet.acknowledge(ack.clone()).unwrap();
    assert!(matches!(fleet.report(10).unwrap().domains[&1].propagation, FencePropagation::Acknowledged(_)));
    let mut foreign = FleetCoordinator::new(4, 8, 100).unwrap(); let (mut other, _) = fixture(1, 1);
    join(&mut other, &mut foreign, 1, 50);
    let alien = foreign.issue_fence(11, foreign.revision(), FleetScope::All).unwrap();
    assert_eq!(other.install_fleet_fence(&second, 0, 0).unwrap_err(), Error::Binding);
    assert_eq!(broker.install_fleet_fence(&alien, 0, 0).unwrap_err(), Error::Binding);
    assert_eq!(foreign.acknowledge(ack), Err(Error::Binding));
    assert_eq!(foreign.observe_domain(broker.fleet_observation().unwrap()), Err(Error::Binding));
}

#[test]
fn frozen_membership_and_explicit_selectors_cannot_omit_unknown_domains_silently() {
    let mut fleet = fleet(); let (mut a, _) = fixture(1, 1); let (mut duplicate, _) = fixture(1, 1);
    join(&mut a, &mut fleet, 1, 50);
    let revision = fleet.revision();
    assert_eq!(duplicate.join_fleet(&mut fleet, 2, revision, ElapsedTick(50)), Err(Error::Duplicate));
    assert!(fleet.issue_fence(10, revision, FleetScope::Domains(BTreeSet::new())).is_err());
    assert!(fleet.issue_fence(10, revision, FleetScope::Domains(BTreeSet::from([999]))).is_err());
    assert!(fleet.issue_fence(10, revision, FleetScope::Tenant(99)).is_err());
    assert_eq!(fleet.revision(), revision);
    let command = fleet.issue_fence(10, revision, FleetScope::Principal { tenant: 1, principal: 1 }).unwrap();
    assert_eq!(command.domains().collect::<Vec<_>>(), vec![1]);
    let retry = fleet.issue_fence(10, revision, FleetScope::Principal { tenant: 1, principal: 1 }).unwrap();
    assert_eq!(retry.floor(), command.floor()); assert_eq!(retry.issued_order(), command.issued_order());
    assert_eq!(fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap_err(), Error::Binding);
    let (mut late, _) = fixture(3, 3); let revision = fleet.revision();
    assert_eq!(late.join_fleet(&mut fleet, 3, revision, ElapsedTick(50)), Err(Error::WrongState));
    assert_eq!(a.join_fleet(&mut fleet, 4, revision, ElapsedTick(50)), Err(Error::Duplicate));
}

#[test]
fn endpoint_fencing_and_nonexecution_settlement_remain_available_after_fleet_stop() {
    let mut fleet = fleet(); let (mut broker, mut endpoint) = fixture(1, 1); join(&mut broker, &mut fleet, 1, 50);
    let (action, permit) = ready(&mut broker, 1, 1, 1);
    let command = fleet.issue_fence(10, fleet.revision(), FleetScope::All).unwrap();
    let message = broker.dispatch(&permit, &action, &snapshot()).unwrap();
    let before = broker.inspect(); broker.install_fleet_fence(&command, before.sequence, before.ledger.epoch).unwrap();
    let endpoint_fence = broker.restart_dispatcher().unwrap();
    let ack = endpoint.install_fence(endpoint_fence).unwrap(); broker.confirm_fence(ack).unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect().ledger.charged, 16);
    let query = broker.status_query(1).unwrap();
    let sealed = endpoint.seal_unexecuted(&query).unwrap();
    broker.accept_receipt(sealed).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100); assert_eq!(broker.inspect().ledger.charged, 0);
    assert_eq!(broker.propose(2, spec(1, 1, broker.inspect().ledger.epoch), &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(endpoint.execution_count(), 0); conserve(&broker);
}
