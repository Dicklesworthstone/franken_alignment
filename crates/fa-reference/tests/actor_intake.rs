//! Actor proposals are data; only the existing supervisor can admit an effect.
#[path = "support/actor_gateway.rs"]
mod support;

use support::{fixture, proposal, review, snapshot};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, IntakeLimits, Knowledge, UnknownReason};
use fa_reference::action::{ActionState, ElapsedTick, Purpose};
use fa_reference::Error;

fn outcome(status: Knowledge<ActorOutcome>) -> ActorOutcome {
    match status { Knowledge::Known { value, .. } => value, other => panic!("not terminal: {other:?}") }
}

#[test]
fn exact_retries_share_one_scoped_attempt_and_fifo_is_not_key_order() {
    let (port, mut supervisor, endpoint) = fixture(IntakeLimits::default());
    let first = port.submit(90, &proposal()).unwrap();
    let duplicate = port.clone().submit(90, &proposal()).unwrap();
    port.submit(1, &proposal()).unwrap();
    assert_eq!(port.poll(&first), port.poll(&duplicate));
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 90);
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 1);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let frozen = supervisor.action(90).unwrap();
    assert_eq!((frozen.spec().scope.tenant, frozen.spec().scope.principal, frozen.spec().scope.run), (1, 2, 3));
    assert_eq!(frozen.spec().scope.purpose, Purpose::Effect);
    assert!(!frozen.spec().required_witnesses.is_empty());
    assert_eq!(supervisor.broker().inspect().ledger.stages.len(), 2);
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 0);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn every_execution_field_is_part_of_idempotency_identity() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    port.submit(7, &proposal()).unwrap();
    let original = proposal();
    for index in 0..9 {
        let mut changed = original.clone();
        match index {
            0 => changed.payload.push(0), 1 => changed.units += 1,
            2 => changed.deadline.0 += 1, 3 => changed.expected_policy_epoch += 1,
            4 => changed.target.adapter += 1, 5 => changed.target.object += 1,
            6 => changed.target.contract_version += 1, 7 => changed.target.expected_version += 1,
            _ => changed.target.generation += 1,
        }
        assert_eq!(port.submit(7, &changed).unwrap_err(), ActorError::IdempotencyConflict);
    }
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.stages.len(), 1);
}

#[test]
fn epoch_changes_and_policy_refusals_do_not_leak_private_diagnostics() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let stale = port.submit(1, &proposal()).unwrap();
    supervisor.broker_mut().revoke_epoch().unwrap();
    let refused = supervisor.accept_next(&snapshot()).unwrap().unwrap();
    assert_eq!(refused.result.unwrap_err(), Error::Stale);
    assert_eq!(outcome(port.poll(&stale)), ActorOutcome::NotAdmitted);
    assert!(supervisor.broker().inspect().ledger.stages.is_empty());
    let mut current = proposal(); current.expected_policy_epoch = 1;
    let denied = port.submit(2, &current).unwrap();
    let mut forbidden = snapshot(); forbidden.values.insert(7, vec![8]);
    let processed = supervisor.accept_next(&forbidden).unwrap().unwrap().result.unwrap().unwrap();
    assert_eq!(processed.state, ActionState::Denied);
    assert_eq!(outcome(port.poll(&denied)), ActorOutcome::Denied);
    let debug = format!("{port:?} {denied:?} {:?}", port.poll(&denied));
    for secret in ["secret-helper", "secret-detector-question", "ExactValue", "policy", "votes", "secret-cohort"] {
        assert!(!debug.contains(secret), "leaked {secret}");
    }
}

#[test]
fn successful_review_and_reservation_still_project_as_pending() {
    let (port, mut supervisor, endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(5, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 5, 11);
    let id = supervisor.attempt(5).unwrap();
    let _permit = supervisor.broker_mut().authorize(id, Some(&inputs), &snapshot()).unwrap();
    supervisor.synchronize().unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 5 });
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 16);
    assert_eq!(endpoint.execution_count(), 0);
    port.cancel(&ticket).unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 5 });
    supervisor.synchronize().unwrap();
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::CancelledBeforeDispatch);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
}

#[test]
fn foreign_tickets_are_withheld_before_request_existence_is_considered() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let (other, _other_supervisor, _) = fixture(IntakeLimits::default());
    let foreign_existing = other.submit(1, &proposal()).unwrap();
    let foreign_missing = other.submit(2, &proposal()).unwrap();
    let own = port.submit(1, &proposal()).unwrap();
    assert_eq!(port.poll(&foreign_existing), port.poll(&foreign_missing));
    assert!(matches!(port.poll(&foreign_existing), Knowledge::Withheld { .. }));
    assert_eq!(port.cancel(&foreign_existing), Err(ActorError::Withheld));
    assert_eq!(port.cancel(&foreign_missing), Err(ActorError::Withheld));
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    assert_eq!(port.poll(&own), Knowledge::Pending { request: 1 });
}

#[test]
fn cancelled_queue_entries_remain_idempotency_tombstones() {
    let (port, mut supervisor, _) = fixture(IntakeLimits { requests: 1, payload_bytes: 7 });
    let ticket = port.submit(99, &proposal()).unwrap();
    port.cancel(&ticket).unwrap();
    let processed = supervisor.accept_next(&snapshot()).unwrap().unwrap();
    assert!(processed.attempt.is_none());
    assert!(processed.result.unwrap().is_none());
    assert!(supervisor.broker().inspect().ledger.stages.is_empty());
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::CancelledBeforeDispatch);
    port.submit(99, &proposal()).unwrap();
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(port.submit(100, &proposal()).unwrap_err(), ActorError::Capacity);
}

#[test]
fn aggregate_byte_bound_and_malformed_proposals_refuse_without_queueing() {
    let (port, mut supervisor, _) = fixture(IntakeLimits { requests: 4, payload_bytes: 14 });
    assert_eq!(port.submit(0, &proposal()).unwrap_err(), ActorError::MalformedProposal);
    let mut bad = proposal(); bad.target.generation = 0;
    assert_eq!(port.submit(1, &bad).unwrap_err(), ActorError::MalformedProposal);
    bad = proposal(); bad.units = 1;
    assert_eq!(port.submit(1, &bad).unwrap_err(), ActorError::Capacity);
    port.submit(1, &proposal()).unwrap(); port.submit(2, &proposal()).unwrap();
    let mut over = proposal(); over.payload = vec![1];
    assert_eq!(port.submit(3, &over).unwrap_err(), ActorError::Capacity);
    for _ in 0..2 { let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap(); }
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn dropping_supervisor_downgrades_unfinished_work_but_preserves_terminal_history() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let cancelled = port.submit(1, &proposal()).unwrap(); port.cancel(&cancelled).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let pending = port.submit(2, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    supervisor.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    drop(supervisor);
    assert_eq!(outcome(port.poll(&cancelled)), ActorOutcome::CancelledBeforeDispatch);
    assert_eq!(port.poll(&pending), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    assert_eq!(port.clone().submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
}
