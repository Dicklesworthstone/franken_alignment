use super::*;
use super::super::shutdown::{DomainDrain, PoolStopRequest};
use crate::action::consequence::delivery::fleet::{FleetDeliveryKnowledge, FleetScope};

fn stop_all(pool: &mut FundingPool) -> super::super::shutdown::PoolStopSweep {
    pool.request_stop_all(PoolStopRequest { operation: 700, expected_revision: pool.revision() }).unwrap()
}

#[test]
fn whole_pool_shutdown_settles_original_endpoints_and_never_refunds_execution() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoints = BTreeMap::from([(5, endpoint(5)), (6, endpoint(6)), (7, endpoint(7))]);
    for (id, total) in [(5, 40), (6, 30), (7, 20)] {
        fund(&mut pool, id, total, endpoints.get_mut(&id).unwrap());
    }
    let executed = send(&mut pool, 5, 1, 15);
    let receipt = endpoints.get_mut(&5).unwrap().deliver(&executed).unwrap();
    pool.domain(5).unwrap().accept_receipt(receipt).unwrap();
    let (pending, permit) = authorize(&mut pool, 5, 2, 10);
    let delayed = send(&mut pool, 6, 1, 20);
    assert_eq!(balances(&pool), (10, 45, 10, 35));
    let stopped = stop_all(&mut pool);
    assert_eq!(stopped.domains.len(), 3);
    assert!(stopped.domains.values().all(Result::is_ok));
    assert!(pool.inspect().unwrap().admission_closed);
    assert_eq!(pool.domain(5).unwrap().dispatch(&permit, &pending, &snapshot()).unwrap_err(), Error::WrongState);
    let mut unused = endpoint(8);
    assert_eq!(pool.fund_domain(pool.revision(), config(8, 1), &mut unused), Err(Error::WrongState));
    let sweep = pool.progress_stop_all(&mut endpoints).unwrap();
    assert!(sweep.drained());
    assert_eq!(balances(&pool), (85, 0, 0, 15));
    assert!(endpoints.get_mut(&6).unwrap().deliver(&delayed).is_err());
    assert_eq!(endpoints[&5].execution_count(), 1);
    assert_eq!(endpoints[&6].execution_count(), 0);
    let again = pool.progress_stop_all(&mut endpoints).unwrap();
    assert!(again.drained());
    for domain in again.domains.values() {
        assert!(matches!(domain, DomainDrain::Stopped { returned: Ok(receipt), .. } if receipt.units == 0));
    }
    assert_eq!(balances(&pool), (85, 0, 0, 15));
}

#[test]
fn missing_endpoint_does_not_hide_a_domain_or_rollback_other_settlement() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut first = endpoint(5);
    let mut second = endpoint(6);
    fund(&mut pool, 5, 40, &mut first);
    fund(&mut pool, 6, 30, &mut second);
    let _first = send(&mut pool, 5, 1, 15);
    let _second = send(&mut pool, 6, 1, 20);
    stop_all(&mut pool);
    let mut endpoints = BTreeMap::from([(5, first)]);
    let partial = pool.progress_stop_all(&mut endpoints).unwrap();
    assert!(!partial.drained());
    assert_eq!(partial.domains.len(), 2);
    assert!(matches!(&partial.domains[&6], DomainDrain::Stopped {
        endpoint: Err(Error::Missing), returned: Ok(receipt), ..
    } if receipt.units == 10));
    assert_eq!(balances(&pool), (80, 0, 0, 20));
    endpoints.insert(6, second);
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(balances(&pool), (100, 0, 0, 0));
}

#[test]
fn foreign_endpoint_is_not_a_receipt_and_does_not_block_a_different_domain() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut first = endpoint(5);
    let mut second = endpoint(6);
    fund(&mut pool, 5, 40, &mut first);
    fund(&mut pool, 6, 30, &mut second);
    let delayed = send(&mut pool, 5, 1, 15);
    let _second = send(&mut pool, 6, 1, 20);
    stop_all(&mut pool);
    let mut endpoints = BTreeMap::from([(5, endpoint(7)), (6, second)]);
    let partial = pool.progress_stop_all(&mut endpoints).unwrap();
    assert!(matches!(&partial.domains[&5], DomainDrain::Stopped { endpoint: Err(Error::Binding), .. }));
    assert!(matches!(&partial.domains[&6], DomainDrain::Stopped { endpoint: Ok(sweep), .. }
        if sweep.progress.drained()));
    assert_eq!(balances(&pool), (85, 0, 0, 15));
    // The actual endpoint was never fenced by a foreign endpoint's failure.
    let receipt = first.deliver(&delayed).unwrap();
    pool.domain(5).unwrap().accept_receipt(receipt).unwrap();
    endpoints.insert(5, first);
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(balances(&pool), (85, 0, 0, 15));
}

#[test]
fn failed_child_stop_cannot_leave_any_parent_admission_path_open() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoints = BTreeMap::from([(5, endpoint(5)), (6, endpoint(6))]);
    fund(&mut pool, 5, 40, endpoints.get_mut(&5).unwrap());
    fund(&mut pool, 6, 30, endpoints.get_mut(&6).unwrap());
    let (pending, permit) = authorize(&mut pool, 5, 1, 10);
    // Only a unit test can exhaust the original dispatcher's private counter.
    pool.allocations.get_mut(&5).unwrap().broker.epoch = u64::MAX;
    let stopped = stop_all(&mut pool);
    assert_eq!(stopped.domains[&5], Err(Error::Overflow));
    assert!(stopped.domains[&6].is_ok());
    assert!(pool.domain(5).unwrap().broker().stop_receipt().is_none());
    assert_eq!(pool.domain(5).unwrap().dispatch(&permit, &pending, &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(pool.domain(5).unwrap().authorize(1, &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(pool.domain(5).unwrap().propose(2, action(5, 1), &snapshot()).unwrap_err(), Error::WrongState);
    assert!(matches!(pool.domain(5).unwrap().begin_review(1, 300, [9; 32], &snapshot()), Err(Error::WrongState)));
    let partial = pool.progress_stop_all(&mut endpoints).unwrap();
    assert_eq!(partial.domains[&5], DomainDrain::StopFailed(Error::Overflow));
    assert_eq!(balances(&pool), (60, 30, 10, 0));
    // Demonstrate retry of the SAME operation, without reopening the pool.
    pool.allocations.get_mut(&5).unwrap().broker.epoch = 0;
    let retry = pool.request_stop_all(stopped.request).unwrap();
    assert!(retry.domains.values().all(Result::is_ok));
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(balances(&pool), (100, 0, 0, 0));
}

#[test]
fn stop_identity_retries_survive_funding_revision_changes_without_new_floors() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 40, &mut endpoint);
    let before = pool.inspect().unwrap();
    assert_eq!(pool.request_stop_all(PoolStopRequest { operation: 0, expected_revision: pool.revision() }), Err(Error::InvalidInput));
    assert_eq!(pool.request_stop_all(PoolStopRequest { operation: 700, expected_revision: 0 }), Err(Error::Stale));
    assert_eq!(pool.inspect().unwrap(), before);
    let initial = stop_all(&mut pool);
    pool.collect_returned(pool.revision(), 5).unwrap();
    assert_ne!(pool.revision(), initial.request.expected_revision);
    let retry = pool.request_stop_all(initial.request).unwrap();
    assert_eq!(retry.domains, initial.domains);
    assert_eq!(pool.stop_request(), Some(initial.request));
    assert_eq!(pool.request_stop_all(PoolStopRequest { expected_revision: pool.revision(), ..initial.request }), Err(Error::Binding));
    assert_eq!(pool.request_stop_all(PoolStopRequest { operation: 701, ..initial.request }), Err(Error::Duplicate));
    assert!(pool.inspect().unwrap().admission_closed);
}

#[test]
fn funding_revision_exhaustion_cannot_keep_admission_open_or_hide_collection_failure() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 10, &mut endpoint);
    let delayed = send(&mut pool, 5, 1, 5);
    pool.revision = u64::MAX;
    assert!(stop_all(&mut pool).domains[&5].is_ok());
    let mut endpoints = BTreeMap::from([(5, endpoint)]);
    let sweep = pool.progress_stop_all(&mut endpoints).unwrap();
    assert!(matches!(&sweep.domains[&5], DomainDrain::Stopped {
        endpoint: Ok(sweep), returned: Err(Error::Overflow), ..
    } if sweep.progress.drained()));
    assert!(!sweep.drained());
    assert_eq!(balances(&pool), (0, 10, 0, 0));
    assert!(pool.inspect().unwrap().admission_closed);
    assert!(endpoints.get_mut(&5).unwrap().deliver(&delayed).is_err());
}

#[test]
fn empty_pool_can_close_but_an_unstopped_pool_cannot_drain() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut endpoints = BTreeMap::new();
    assert_eq!(pool.progress_stop_all(&mut endpoints), Err(Error::Incomplete));
    let stopped = stop_all(&mut pool);
    assert!(stopped.domains.is_empty());
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(pool.fund_domain(pool.revision(), config(5, 1), &mut endpoint(5)), Err(Error::WrongState));
}

#[test]
fn fleet_fences_preserve_shared_funding_and_are_not_parent_refunds() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoints = BTreeMap::from([(5, endpoint(5)), (6, endpoint(6))]);
    let mut fleet = FleetCoordinator::new(4, 4, 100).unwrap();
    for (id, total) in [(5, 40), (6, 30)] {
        fund(&mut pool, id, total, endpoints.get_mut(&id).unwrap());
        let revision = fleet.revision();
        pool.domain(id).unwrap().join_fleet(&mut fleet, id, revision, ElapsedTick(90)).unwrap();
    }
    let _sent = send(&mut pool, 5, 1, 15);
    let (pending, permit) = authorize(&mut pool, 6, 1, 10);
    let fence = fleet.issue_fence(700, fleet.revision(), FleetScope::All).unwrap();
    for id in [5, 6] {
        let mut domain = pool.domain(id).unwrap();
        let before = domain.broker().inspect();
        let ack = domain.install_fleet_fence(&fence, before.sequence, before.ledger.epoch).unwrap();
        fleet.acknowledge(ack).unwrap();
        fleet.observe_domain(domain.broker().fleet_observation().unwrap()).unwrap();
    }
    assert_eq!(balances(&pool), (30, 55, 0, 15));
    assert_eq!(pool.collect_returned(pool.revision(), 5), Err(Error::WrongState));
    assert!(pool.domain(6).unwrap().dispatch(&permit, &pending, &snapshot()).is_err());
    let report = fleet.report(700).unwrap();
    assert!(matches!(&report.domains[&5].deliveries,
        FleetDeliveryKnowledge::Observed { unresolved, .. } if unresolved.len() == 1));
    stop_all(&mut pool);
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(balances(&pool), (100, 0, 0, 0));
}

#[test]
fn enrolled_funding_cannot_dispatch_after_shared_lease_expiry_with_an_old_clock() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 40, &mut endpoint);
    let mut fleet = FleetCoordinator::new(4, 4, 100).unwrap();
    pool.domain(5).unwrap().join_fleet(&mut fleet, 5, 0, ElapsedTick(10)).unwrap();
    let (pending, permit) = authorize(&mut pool, 5, 1, 10);
    fleet.observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(pool.domain(5).unwrap().dispatch(&permit, &pending, &snapshot()).unwrap_err(), Error::Stale);
    pool.domain(5).unwrap().observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(pool.domain(5).unwrap().dispatch(&permit, &pending, &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(balances(&pool), (60, 30, 10, 0));
    stop_all(&mut pool);
    endpoint.observe_time(ElapsedTick(10)).unwrap();
    let mut endpoints = BTreeMap::from([(5, endpoint)]);
    assert!(pool.progress_stop_all(&mut endpoints).unwrap().drained());
    assert_eq!(balances(&pool), (100, 0, 0, 0));
}

#[test]
fn a_completed_positive_review_cannot_bypass_a_failed_child_stop() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 40, &mut endpoint);
    let review = {
        let mut domain = pool.domain(5).unwrap();
        domain.propose(1, action(5, 10), &snapshot()).unwrap();
        let mut session = domain.begin_review(1, 100, [9; 32], &snapshot()).unwrap();
        let salt = b"reference-salt";
        let commitment = session.commitment("reviewer", Verdict::Allow, salt).unwrap();
        session.commit("reviewer", commitment).unwrap();
        session.open_reveals().unwrap();
        session.reveal("reviewer", Verdict::Allow, salt).unwrap();
        session.finish().unwrap()
    };
    pool.allocations.get_mut(&5).unwrap().broker.epoch = u64::MAX;
    assert_eq!(stop_all(&mut pool).domains[&5], Err(Error::Overflow));
    let before = pool.inspect().unwrap();
    assert_eq!(pool.domain(5).unwrap().apply_review(review, &snapshot()).unwrap_err(), Error::WrongState);
    let mut fleet = FleetCoordinator::new(4, 4, 100).unwrap();
    assert_eq!(pool.domain(5).unwrap().join_fleet(&mut fleet, 5, 0, ElapsedTick(90)), Err(Error::WrongState));
    assert_eq!(pool.inspect().unwrap(), before);
}
