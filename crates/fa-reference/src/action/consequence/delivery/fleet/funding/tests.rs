use super::*;
use crate::action::{ActionState, ResolvedTarget, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::reducer::Caps;
use crate::round::Verdict;

fn scope(authority: u64) -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority, purpose: Purpose::Effect }
}
fn target(authority: u64) -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: authority + 100, contract_version: 1,
        expected_version: 1, generation: 1 }
}
fn config(authority: u64, total: u64) -> ControllerConfig {
    ControllerConfig {
        scope: scope(authority), total, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy {
            generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy {
                cohort: "reference".to_owned(), weight: 1,
            })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[target(authority)]).unwrap(),
    }
}
fn endpoint(authority: u64) -> PublicationEndpoint {
    let mut endpoint = PublicationEndpoint::new(target(authority), b"initial".to_vec(), 1000, 8).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    endpoint
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn action(authority: u64, units: u64) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(authority), target: Some(target(authority)),
        payload: vec![9], required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units }
}
fn fund(pool: &mut FundingPool, authority: u64, total: u64, endpoint: &mut PublicationEndpoint) {
    pool.fund_domain(pool.revision(), config(authority, total), endpoint).unwrap();
    let mut domain = pool.domain(authority).unwrap();
    domain.observe_time(ElapsedTick(1)).unwrap();
    domain.confirm_endpoint_fence(endpoint).unwrap();
}
fn authorize(pool: &mut FundingPool, authority: u64, id: u64, units: u64) -> (FrozenAction, Permit) {
    let mut domain = pool.domain(authority).unwrap();
    let proposal = domain.propose(id, action(authority, units), &snapshot()).unwrap();
    let mut session = domain.begin_review(id, id + 100, [9; 32], &snapshot()).unwrap();
    let salt = b"reference-salt";
    let commitment = session.commitment("reviewer", Verdict::Allow, salt).unwrap();
    session.commit("reviewer", commitment).unwrap();
    session.open_reveals().unwrap();
    session.reveal("reviewer", Verdict::Allow, salt).unwrap();
    domain.apply_review(session.finish().unwrap(), &snapshot()).unwrap();
    let permit = domain.authorize(id, &snapshot()).unwrap();
    (proposal.action, permit)
}
fn send(pool: &mut FundingPool, authority: u64, id: u64, units: u64) -> DispatchEnvelope {
    let (action, permit) = authorize(pool, authority, id, units);
    pool.domain(authority).unwrap().dispatch(&permit, &action, &snapshot()).unwrap()
}
fn stop(pool: &mut FundingPool, authority: u64) -> StopReceipt {
    let mut domain = pool.domain(authority).unwrap();
    let before = domain.broker().inspect();
    domain.request_stop(StopRequest { operation: 99, expected_control_sequence: before.sequence,
        expected_authority_epoch: before.ledger.epoch }).unwrap()
}
fn balances(pool: &FundingPool) -> (u64, u64, u64, u64) {
    let inspection = pool.inspect().unwrap();
    assert!(inspection.conserved());
    // An independent u128 sum avoids repeating the implementation's checked-u64 fold.
    assert_eq!(u128::from(inspection.total), u128::from(inspection.unallocated)
        + u128::from(inspection.available_in_domains) + u128::from(inspection.reserved)
        + u128::from(inspection.charged));
    (inspection.unallocated, inspection.available_in_domains, inspection.reserved, inspection.charged)
}

#[test]
fn allocations_share_one_budget_and_failed_funding_does_not_attach_an_endpoint() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut first = endpoint(5);
    let mut second = endpoint(6);
    fund(&mut pool, 5, 6, &mut first);
    let before = pool.inspect().unwrap();
    assert_eq!(pool.fund_domain(pool.revision(), config(6, 5), &mut second), Err(Error::Limit));
    assert_eq!(pool.inspect().unwrap(), before);
    // The exact endpoint rejected above is still usable with backed funding.
    fund(&mut pool, 6, 4, &mut second);
    assert_eq!(balances(&pool), (0, 10, 0, 0));
    let _first = authorize(&mut pool, 5, 1, 6);
    let _second = authorize(&mut pool, 6, 1, 4);
    assert_eq!(balances(&pool), (0, 0, 10, 0));
    assert_eq!(pool.collect_returned(pool.revision(), 5), Err(Error::WrongState));
    assert_eq!(balances(&pool), (0, 0, 10, 0));
}

#[test]
fn stale_scope_duplicate_invalid_config_and_capacity_refusals_are_atomic() {
    let mut pool = FundingPool::new(1, 900, 10, 1).unwrap();
    let mut first = endpoint(5);
    let before = pool.inspect().unwrap();
    assert_eq!(pool.fund_domain(1, config(5, 6), &mut first), Err(Error::Stale));
    for mutate in [0, 1, 2, 3] {
        let mut invalid = config(5, 6);
        match mutate {
            0 => invalid.scope.tenant = 2,
            1 => invalid.scope.authority = 900,
            2 => invalid.scope.purpose = Purpose::Experiment,
            _ => invalid.max_attempts = 0,
        }
        assert!(pool.fund_domain(0, invalid, &mut first).is_err());
        assert_eq!(pool.inspect().unwrap(), before);
    }
    fund(&mut pool, 5, 6, &mut first);
    let mut second = endpoint(6);
    let before = pool.inspect().unwrap();
    assert_eq!(pool.fund_domain(pool.revision(), config(5, 1), &mut second), Err(Error::Duplicate));
    assert_eq!(pool.fund_domain(pool.revision(), config(6, 1), &mut second), Err(Error::Limit));
    assert_eq!(pool.inspect().unwrap(), before);
    stop(&mut pool, 5);
    pool.collect_returned(pool.revision(), 5).unwrap();
    assert_eq!(pool.fund_domain(pool.revision(), config(5, 1), &mut second), Err(Error::Duplicate));
    assert_eq!(pool.fund_domain(pool.revision(), config(6, 1), &mut second), Err(Error::Limit));
}

#[test]
fn returned_reservations_can_fund_another_domain_but_old_permits_cannot_spend() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut first = endpoint(5);
    fund(&mut pool, 5, 6, &mut first);
    let (original, permit) = authorize(&mut pool, 5, 1, 4);
    assert_eq!(balances(&pool), (4, 2, 4, 0));
    let stopped = stop(&mut pool, 5);
    assert_eq!(stopped.refunded_units(), 4);
    let returned = pool.collect_returned(pool.revision(), 5).unwrap();
    assert_eq!(returned.units, 6);
    assert_eq!(balances(&pool), (10, 0, 0, 0));
    let revision = pool.revision();
    assert_eq!(pool.collect_returned(revision, 5).unwrap().units, 0);
    assert_eq!(pool.revision(), revision);
    assert!(pool.domain(5).unwrap().dispatch(&permit, &original, &snapshot()).is_err());
    assert!(pool.domain(5).unwrap().propose(2, action(5, 1), &snapshot()).is_err());
    let mut second = endpoint(6);
    fund(&mut pool, 6, 10, &mut second);
    let sent = send(&mut pool, 6, 1, 10);
    let receipt = second.deliver(&sent).unwrap();
    assert!(pool.domain(6).unwrap().accept_receipt(receipt).unwrap());
    assert_eq!(balances(&pool), (0, 0, 0, 10));
}

#[test]
fn stopping_without_endpoint_ack_does_not_reclaim_a_delayed_effect() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 60, &mut endpoint);
    let delayed = send(&mut pool, 5, 1, 30);
    pool.domain(5).unwrap().acknowledgment_lost(1).unwrap();
    stop(&mut pool, 5);
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, 30);
    assert_eq!(balances(&pool), (70, 0, 0, 30));
    // Local stop was NOT an endpoint fence. The delayed message may still execute.
    let receipt = endpoint.deliver(&delayed).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    let copy = receipt.clone();
    assert!(pool.domain(5).unwrap().accept_receipt(receipt).unwrap());
    assert!(!pool.domain(5).unwrap().accept_receipt(copy).unwrap());
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, 0);
    assert_eq!(balances(&pool), (70, 0, 0, 30));
}

#[test]
fn only_original_nonexecution_evidence_returns_later_unknown_charges() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 60, &mut endpoint);
    let delayed = send(&mut pool, 5, 1, 30);
    stop(&mut pool, 5);
    let old_revision = pool.revision();
    assert_eq!(pool.collect_returned(old_revision, 5).unwrap().units, 30);
    assert_eq!(pool.collect_returned(old_revision, 5), Err(Error::Stale));
    let sweep = pool.domain(5).unwrap().progress_stop(&mut endpoint).unwrap();
    assert!(sweep.progress.drained());
    assert!(matches!(&sweep.outcomes[&1], Ok(EndpointStatus::Resolved(receipt))
        if receipt.outcome() == (EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, 30);
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, 0);
    assert_eq!(balances(&pool), (100, 0, 0, 0));
    assert!(endpoint.deliver(&delayed).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn expired_retention_and_irrecoverable_effects_remain_parent_liabilities() {
    let mut pool = FundingPool::new(1, 900, 100, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 60, &mut endpoint);
    let _delayed = send(&mut pool, 5, 1, 30);
    stop(&mut pool, 5);
    pool.collect_returned(pool.revision(), 5).unwrap();
    endpoint.observe_time(ElapsedTick(1001)).unwrap();
    let sweep = pool.domain(5).unwrap().progress_stop(&mut endpoint).unwrap();
    assert_eq!(sweep.outcomes[&1], Ok(EndpointStatus::RetentionExpired));
    assert!(!sweep.progress.drained());
    pool.domain(5).unwrap().abandon_unknown(1).unwrap();
    assert_eq!(pool.domain(5).unwrap().broker().inspect().ledger.stages[&1], ActionState::IrrecoverablyUnknown);
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, 0);
    assert_eq!(balances(&pool), (70, 0, 0, 30));
}

#[test]
fn permits_and_receipts_cannot_move_between_funded_children() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut first = endpoint(5);
    let mut second = endpoint(6);
    fund(&mut pool, 5, 6, &mut first);
    fund(&mut pool, 6, 4, &mut second);
    let (action, permit) = authorize(&mut pool, 5, 1, 3);
    let before = pool.inspect().unwrap();
    assert_eq!(pool.domain(6).unwrap().dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Binding);
    assert_eq!(pool.inspect().unwrap(), before);
    let sent = pool.domain(5).unwrap().dispatch(&permit, &action, &snapshot()).unwrap();
    let receipt = first.deliver(&sent).unwrap();
    assert_eq!(pool.domain(6).unwrap().accept_receipt(receipt.clone()), Err(Error::Binding));
    assert!(pool.domain(5).unwrap().accept_receipt(receipt).unwrap());
    assert_eq!(balances(&pool), (0, 7, 0, 3));
}

#[test]
fn dropping_a_domain_borrow_does_not_return_live_rights() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut endpoint = endpoint(5);
    fund(&mut pool, 5, 6, &mut endpoint);
    { let _domain = pool.domain(5).unwrap(); }
    assert_eq!(balances(&pool), (4, 6, 0, 0));
    assert_eq!(pool.collect_returned(pool.revision(), 5), Err(Error::WrongState));
    assert_eq!(pool.domain(77).unwrap_err(), Error::Missing);
}

#[test]
fn maximum_integer_budget_conserves_through_allocation_and_return() {
    let mut pool = FundingPool::new(1, 900, u64::MAX, 4).unwrap();
    let mut first = endpoint(5);
    let mut second = endpoint(6);
    fund(&mut pool, 5, u64::MAX - 1, &mut first);
    fund(&mut pool, 6, 1, &mut second);
    assert_eq!(balances(&pool), (0, u64::MAX, 0, 0));
    stop(&mut pool, 5);
    assert_eq!(pool.collect_returned(pool.revision(), 5).unwrap().units, u64::MAX - 1);
    stop(&mut pool, 6);
    assert_eq!(pool.collect_returned(pool.revision(), 6).unwrap().units, 1);
    assert_eq!(balances(&pool), (u64::MAX, 0, 0, 0));
}

#[test]
fn revision_overflow_cannot_publish_partial_funding_or_a_refund() {
    let mut pool = FundingPool::new(1, 900, 10, 4).unwrap();
    let mut endpoint = endpoint(5);
    pool.revision = u64::MAX;
    let before = pool.inspect().unwrap();
    assert_eq!(pool.fund_domain(u64::MAX, config(5, 6), &mut endpoint), Err(Error::Overflow));
    assert_eq!(pool.inspect().unwrap(), before);
    pool.revision = 0;
    fund(&mut pool, 5, 6, &mut endpoint);
    stop(&mut pool, 5);
    pool.revision = u64::MAX;
    let before = pool.inspect().unwrap();
    assert_eq!(pool.collect_returned(u64::MAX, 5), Err(Error::Overflow));
    assert_eq!(pool.inspect().unwrap(), before);
}
