#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn request(host: &FileDelivery, operation: u64) -> StopRequest {
    let state = host.inspect();
    StopRequest { operation, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch }
}

#[test]
fn terminal_stop_survives_reopening_and_only_original_endpoint_receipts_drain_its_charges() {
    let root = Directory::new();
    let mut host = create(&root);
    let (old_action, old_permit) = approved(&mut host, 1, b"reserved");
    dispatched(&mut host, 2, b"unsent");
    dispatched(&mut host, 3, b"published");
    host.publish(host.revision(), 3).unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    let stop = request(&host, 71);
    let receipt = host.request_stop(host.revision(), stop).unwrap();
    assert_eq!(receipt.cancelled(), &[1]);
    assert_eq!(receipt.refunded_units(), 16);
    assert_eq!(host.inspect().control.ledger.available, 68);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Unknown);
    let progress = host.stop_progress().unwrap();
    assert!(!progress.endpoint_fenced);
    assert!(!progress.drained());
    assert_eq!(progress.unresolved, vec![2, 3]);
    assert_eq!(host.request_stop(host.revision(), stop).unwrap(), receipt);
    assert!(host.dispatch(host.revision(), &old_permit, &old_action, snapshot()).is_err());
    assert!(host.publish(host.revision(), 2).is_err());
    assert!(host.publish(host.revision(), 3).is_err());
    assert!(host.propose(host.revision(), 4, spec(&host, b"new"), snapshot()).is_err());
    let durable = FileDelivery::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(durable.stop.as_ref(), Some(&receipt));
    assert_eq!(durable.control.ledger.charged, 32);
    assert_eq!(durable.payload, b"published");
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(host.stop_receipt(), Some(&receipt));
    assert!(!host.clock_ready());
    assert!(host.inspect().dispatcher_epoch > receipt.dispatcher_epoch());
    assert_eq!(host.request_stop(host.revision(), stop).unwrap(), receipt);
    let before = host.inspect();
    assert_eq!(host.progress_stop(host.revision(), ElapsedTick(0)).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(host.inspect(), before);
    let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(sweep.outcomes.len(), 2);
    assert_eq!(sweep.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted {
        reason: NonExecutionReason::Sealed })));
    assert_eq!(sweep.outcomes[&3], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"published");
    assert!(host.propose(host.revision(), 4, spec(&host, b"still stopped"), snapshot()).is_err());
    let repeated = host.progress_stop(host.revision(), ElapsedTick(3)).unwrap();
    assert!(repeated.outcomes.is_empty());
    assert!(repeated.progress.drained());
    assert_eq!(repeated.progress.charged_units, 16);
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(host.stop_receipt(), Some(&receipt));
    host.observe_time(host.revision(), ElapsedTick(4)).unwrap();
    assert!(host.propose(host.revision(), 4, spec(&host, b"no resurrection"), snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn stop_requires_the_exact_original_predecessor_but_not_a_clock_or_a_new_review() {
    let root = Directory::new();
    let mut host = FileDelivery::create(root.store(), profile()).unwrap();
    let initial = host.inspect();
    assert!(!host.clock_ready());
    assert_eq!(host.progress_stop(host.revision(), ElapsedTick(1)).unwrap_err(), JournalError::Contract(Error::Incomplete));
    let valid = request(&host, 81);
    for invalid in [StopRequest { operation: 0, ..valid },
        StopRequest { expected_control_sequence: 1, ..valid },
        StopRequest { expected_authority_epoch: 1, ..valid }] {
        assert!(host.request_stop(host.revision(), invalid).is_err());
        assert_eq!(host.inspect(), initial);
    }
    let receipt = host.request_stop(host.revision(), valid).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(receipt.refunded_units(), 0);
    assert_eq!(host.request_stop(host.revision(), valid).unwrap(), receipt);
    let before = host.inspect();
    assert_eq!(host.request_stop(host.revision(), StopRequest { expected_control_sequence: 1, ..valid }).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(host.request_stop(host.revision(), StopRequest { operation: 82, ..valid }).unwrap_err(),
        JournalError::Contract(Error::Duplicate));
    assert_eq!(host.inspect(), before);
    host.fence(host.revision()).unwrap();
    let sweep = host.progress_stop(host.revision(), ElapsedTick(0)).unwrap();
    assert!(sweep.progress.drained());
    assert!(sweep.outcomes.is_empty());
    assert_eq!(host.stop_receipt(), Some(&receipt));
    assert_eq!(host.propose(host.revision(), 1, spec(&host, b"forbidden"), snapshot()).unwrap_err(),
        JournalError::Contract(Error::WrongState));
}

#[test]
fn stopping_can_drain_a_younger_live_request_without_refunding_an_expired_retention_obligation() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatched(&mut host, 1, b"too old");
    host.observe_time(host.revision(), ElapsedTick(500)).unwrap();
    let mut later = spec(&host, b"still live");
    later.deadline = ElapsedTick(2000);
    let action = host.propose(host.revision(), 2, later, snapshot()).unwrap();
    host.review(host.revision(), review(2, 102, Verdict::Allow)).unwrap();
    let permit = host.authorize(host.revision(), 2, snapshot()).unwrap();
    host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
    host.request_stop(host.revision(), request(&host, 91)).unwrap();
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    let sweep = host.progress_stop(host.revision(), ElapsedTick(1001)).unwrap();
    assert_eq!(sweep.outcomes.len(), 2);
    assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
    assert_eq!(sweep.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted {
        reason: NonExecutionReason::Sealed })));
    assert!(!sweep.progress.drained());
    assert!(sweep.progress.endpoint_fenced);
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().executions, 0);
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    let sweep = host.progress_stop(host.revision(), ElapsedTick(1002)).unwrap();
    assert_eq!(sweep.outcomes.len(), 1);
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.propose(host.revision(), 3, spec(&host, b"forbidden"), snapshot()).is_err());
}

#[test]
fn failed_stop_drain_returns_no_candidate_refund_and_recovery_does_not_reopen_intake() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatched(&mut host, 1, b"published");
    host.publish(host.revision(), 1).unwrap();
    dispatched(&mut host, 2, b"unsent");
    let receipt = host.request_stop(host.revision(), request(&host, 101)).unwrap();
    let before = host.inspect();
    std::fs::write(root.store().join("delivery.pending"), b"inert staging").unwrap();
    assert!(matches!(host.progress_stop(host.revision(), ElapsedTick(2)), Err(JournalError::Io(_))));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.stop_progress().unwrap_err(), JournalError::Unavailable);
    assert_eq!(host.request_stop(host.revision(), receipt.request()).unwrap_err(), JournalError::Unavailable);
    let disk = FileDelivery::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(disk.stop.as_ref(), Some(&receipt));
    assert_eq!(disk.control.ledger.charged, 32);
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(host.stop_receipt(), Some(&receipt));
    let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 1);
    assert!(host.propose(host.revision(), 3, spec(&host, b"forbidden"), snapshot()).is_err());
}

#[test]
fn terminal_stop_is_not_a_new_journal_budget_and_exhaustion_cannot_erase_it() {
    let root = Directory::new();
    let mut p = profile();
    p.limits.events = 6;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    dispatched(&mut host, 1, b"unsent");
    assert_eq!(host.revision(), 5);
    let receipt = host.request_stop(host.revision(), request(&host, 111)).unwrap();
    assert_eq!(host.revision(), 6);
    assert_eq!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap_err(), JournalError::Contract(Error::Limit));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(host);
    assert_eq!(FileDelivery::open(root.store(), p.clone()).unwrap_err(), JournalError::Contract(Error::Limit));
    let disk = FileDelivery::read_publication(root.store(), &p).unwrap();
    assert_eq!(disk.stop.as_ref(), Some(&receipt));
    assert_eq!(disk.control.ledger.charged, 16);
    assert_eq!(disk.control.ledger.stages[&1], ActionState::Unknown);
}
