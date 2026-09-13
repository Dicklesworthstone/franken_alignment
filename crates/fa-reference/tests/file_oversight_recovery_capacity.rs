//! Recovery reserve on the full-input/mandatory-human-key publication path.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::Error;

fn stop(host: &FileOversight) -> StopRequest {
    let state = host.inspect().control;
    StopRequest { operation: 1, expected_control_sequence: state.sequence, expected_authority_epoch: state.ledger.epoch }
}
fn fill(host: &mut FileOversight) {
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let before = host.inspect();
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(1)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before);
}

#[test]
fn full_journal_recovery_retains_consumed_keys_and_settles_all_original_obligations() {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.events = 64;
    let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let first = ready(&mut host, &reviewer, 1, b"executed");
    dispatch(&mut host, &first); host.publish(host.revision(), 1).unwrap();
    let second = ready(&mut host, &reviewer, 2, b"missing");
    dispatch(&mut host, &second);
    let third = ready(&mut host, &reviewer, 3, b"reserved");
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    fill(&mut host);
    assert_eq!(host.revision(), 61);
    assert_eq!(host.journal_capacity().unwrap().remaining().events, 3);
    drop(reviewer); drop(host);
    let (mut host, role) = FileOversight::open(root.store(), p).unwrap();
    drop(role);
    assert_eq!(host.revision(), 62);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(host.human_status(1002).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(host.human_status(1003).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert!(!host.clock_ready());
    assert!(host.dispatch(host.revision(), &third.automatic, &third.human, &third.action, &third.inputs, snapshot()).is_err());
    host.request_stop(host.revision(), stop(&host)).unwrap();
    let swept = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(swept.progress.drained());
    assert_eq!(swept.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(swept.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    assert_eq!(host.revision(), 64);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"executed");
}

#[test]
fn human_approval_and_first_execution_cannot_spend_emergency_capacity() {
    for dispatched in [false, true] {
        let root = Directory::new();
        let mut p = profile(); p.delivery.limits.events = 32;
        let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
        host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (action, input) = reviewed(&mut host, 1, b"must not leak");
        let auto = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(31)).unwrap();
        if dispatched {
            let revision = host.revision();
            let human = reviewer.approve(&mut host, revision, &request).unwrap();
            host.dispatch(host.revision(), &auto, &human, &action, &input, snapshot()).unwrap();
        }
        fill(&mut host);
        let before = host.inspect();
        if dispatched {
            assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Limit)));
        } else {
            let revision = host.revision();
            assert_eq!(reviewer.approve(&mut host, revision, &request).unwrap_err(), JournalError::Contract(Error::Limit));
            assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
        }
        assert_eq!(host.inspect(), before);
        host.request_stop(host.revision(), stop(&host)).unwrap();
        assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn publication_guard_and_reserve_compose_without_a_weaker_send_path() {
    for guard_first in [false, true] {
        let root = Directory::new();
        let mut p = profile(); p.delivery.limits.events = 32;
        let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
        if guard_first { host.enable_publication_guard(host.revision()).unwrap(); }
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        if !guard_first { host.enable_publication_guard(host.revision()).unwrap(); }
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let keys = ready(&mut host, &reviewer, 1, b"guarded"); dispatch(&mut host, &keys);
        assert!(host.publish(host.revision(), 1).is_err());
        let receipt = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        fill(&mut host);
        assert_eq!(host.publish_checked(host.revision(), 1, None, snapshot(), ElapsedTick(1)).unwrap_err(), JournalError::Contract(Error::Limit));
        host.request_stop(host.revision(), stop(&host)).unwrap();
        let drained = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
        assert!(drained.progress.drained());
        assert_eq!(drained.progress.charged_units, 16);
        assert!(host.publication_guard_required());
        assert_eq!(host.inspect().executions, 1);
    }
}

#[test]
fn oversight_byte_limit_counts_its_outer_framing_and_exact_fifty_byte_recovery_tail() {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.events = 64; p.delivery.limits.bytes = 8192;
    let (mut host, role) = FileOversight::create(root.store(), p.clone()).unwrap();
    let initial = host.journal_capacity().unwrap().used().bytes;
    let reserve = RecoveryReserve { events: 3, bytes: 8192 - initial - 18 - 28 };
    host.enable_recovery_reserve(0, reserve).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let ordinary = host.journal_capacity().unwrap();
    assert_eq!(ordinary.ordinary_remaining().bytes, 0);
    assert!(ordinary.ordinary_remaining().events > 0);
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(1)), Err(JournalError::Contract(Error::Limit)));
    drop(host); drop(role);
    let (mut host, _) = FileOversight::open(root.store(), p).unwrap();
    host.request_stop(host.revision(), stop(&host)).unwrap();
    assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    assert_eq!(host.journal_capacity().unwrap().used().bytes - ordinary.used().bytes, 50);
    assert_eq!(host.journal_capacity().unwrap().used().events - ordinary.used().events, 3);
}
