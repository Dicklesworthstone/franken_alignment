//! Evidence-loss recording is restrictive work, not inference or source recovery.
use super::*;

#[test]
fn interrupted_source_does_not_prevent_durable_loss_or_erase_an_unknown_charge() {
    for recovered_clock in [false, true] {
        let (root, mut host, reviewer) = ready();
        let request = activation(&host, 20, 2);
        host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
        let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if recovered_clock { host.fence(host.revision()).unwrap(); }
        // Same private latch set BEFORE the evidence reader is entered. A
        // caught reader unwind leaves it set without acknowledging a capture.
        host.source_interrupted = true;
        let lost = withdrawal(&host, 21);
        let revision = host.revision();
        let receipt = host.withdraw_credibility(revision, lost.clone()).unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert!(host.source_interrupted);
        assert_eq!(host.clock_ready(), !recovered_clock);
        assert!(host.storage_failure().is_none());
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        assert_eq!(host.inspect().control.ledger.charged, 5);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(FileOversight::read_publication(&root.0, &profile()).unwrap(), host.inspect());
        let before = host.inspect(); let bytes = root.bytes();
        assert_eq!(host.withdraw_credibility(0, lost).unwrap(), receipt);
        assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
        let next = activation(&host, 22, 3);
        assert_eq!(host.activate_credibility(host.revision(), next, &contracts(1)), Err(Error::Incomplete.into()));
        assert_eq!(host.propose(host.revision(), 11, spec(&host), snapshot()), Err(Error::Incomplete.into()));
        assert!(host.publish_checked(host.revision(), 10, Some(&inputs), snapshot(), ElapsedTick(2)).is_err());
        assert_eq!(host.inspect().control.ledger.charged, 5);
        drop(host);
        let (mut host, _) = FileOversight::open(&root.0, profile()).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        assert_eq!(host.inspect().control.ledger.charged, 5);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.reconcile(host.revision(), 10).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.charged, 5);
        host.seal_unexecuted(host.revision(), 10).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn interrupted_source_does_not_relax_withdrawal_identity_or_predecessor_checks() {
    let (root, mut host, _) = ready();
    let request = activation(&host, 20, 2);
    host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
    host.source_interrupted = true;
    let before = host.inspect(); let bytes = root.bytes();
    let mut invalid = withdrawal(&host, 21); invalid.operation = 0;
    assert_eq!(host.withdraw_credibility(host.revision(), invalid), Err(Error::InvalidInput.into()));
    let mut stale = withdrawal(&host, 21); stale.expected_epoch += 1;
    assert_eq!(host.withdraw_credibility(host.revision(), stale), Err(Error::Stale.into()));
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert!(host.storage_failure().is_none());
    assert!(host.source_interrupted);
    let lost = withdrawal(&host, 21);
    host.withdraw_credibility(host.revision(), lost).unwrap();
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
}

#[test]
fn recording_boundaries_preserve_actual_commit_and_require_explicit_reactivation() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Boundary { Acknowledged, BeforeTransaction, AfterTransaction }
    for boundary in [Boundary::Acknowledged, Boundary::BeforeTransaction, Boundary::AfterTransaction] {
        let (root, mut host, reviewer) = ready();
        let request = activation(&host, 20, 2);
        host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
        let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let lost = withdrawal(&host, 21);
        let before = host.inspect(); let bytes = root.bytes(); let revision = host.revision();
        // The production method enters this same private recording boundary
        // after its caller/operation preflight. Only the interruption point is
        // injected; the transaction, canonical file and broker are ORIGINAL.
        let outcome = catch_unwind(AssertUnwindSafe(|| record_withdrawal(&mut host, |owner| {
            if boundary == Boundary::BeforeTransaction { panic!("before withdrawal recording"); }
            owner.transact(revision, Event::Credibility(CredibilityEvent::WithdrawHeldOut(lost.clone())))?;
            if boundary == Boundary::AfterTransaction { panic!("after withdrawal replacement"); }
            owner.credibility_withdrawal(lost.operation).cloned()
        })));
        let committed = boundary != Boundary::BeforeTransaction;
        assert_eq!(host.revision(), revision + u64::from(committed));
        assert_eq!(root.bytes() != bytes, committed);
        if boundary == Boundary::Acknowledged {
            let receipt = outcome.unwrap().unwrap();
            assert_eq!(&receipt, host.credibility_withdrawal(lost.operation).unwrap());
            assert!(host.storage_failure().is_none());
            assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        } else {
            assert!(outcome.is_err());
            let failure = host.storage_failure().unwrap();
            assert_eq!(failure.operation, JournalIo::Stage);
            assert_eq!(failure.replacement_may_be_visible, committed);
            assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
            assert_eq!(host.withdraw_credibility(revision, lost.clone()), Err(JournalError::Unavailable));
            assert_eq!(host.propose(host.revision(), 11, spec(&host), snapshot()), Err(JournalError::Unavailable));
            assert!(!host.clock_ready());
        }
        if !committed { assert_eq!(host.inspect(), before); }
        assert_eq!(host.inspect().control.ledger.charged, 5);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(FileOversight::read_publication(&root.0, &profile()).unwrap(), host.inspect());
        drop(host);
        let (mut host, reviewer) = FileOversight::open(&root.0, profile()).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        assert_eq!(host.inspect().control.ledger.charged, 5);
        if committed {
            let receipt = host.credibility_withdrawal(lost.operation).unwrap().clone();
            let before = host.inspect(); let bytes = root.bytes();
            assert_eq!(host.withdraw_credibility(0, lost).unwrap(), receipt);
            assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
        } else {
            assert_eq!(host.credibility_withdrawal(lost.operation), Err(Error::Missing.into()));
        }
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.reconcile(host.revision(), 10).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.charged, 5);
        host.seal_unexecuted(host.revision(), 10).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
        let next = activation(&host, 22, 3);
        host.activate_credibility(host.revision(), next, &contracts(1)).unwrap();
        complete(&mut host, &reviewer, 11);
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, 5);
    }
}

#[test]
fn unwind_after_storage_error_preserves_each_original_barrier_diagnostic() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let (root, mut host, _) = ready();
        let request = activation(&host, 20, 2);
        host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
        let lost = withdrawal(&host, 21); let before = host.inspect(); let revision = host.revision();
        host.store.fail_once(stage);
        let mut original = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| record_withdrawal(&mut host, |owner| {
            let error = match owner.transact(revision, Event::Credibility(CredibilityEvent::WithdrawHeldOut(lost))) {
                Err(error) => error,
                Ok(_) => panic!("expected injected storage failure"),
            };
            let JournalError::Io(failure) = error else { panic!("expected injected storage error"); };
            assert_eq!(failure.operation, stage);
            original = Some(failure);
            panic!("unwind after original storage failure");
        })));
        assert!(outcome.is_err());
        assert!(original.is_some());
        assert_eq!(host.storage_failure(), original.as_ref());
        assert_eq!(host.inspect(), before);
        assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
        let disk = FileOversight::read_publication(&root.0, &profile()).unwrap();
        assert_eq!(disk.executions, 0);
        assert_eq!(disk.revision, revision + u64::from(stage == JournalIo::DirectorySync));
        drop(host);
        let (mut host, reviewer) = FileOversight::open(&root.0, profile()).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let next = activation(&host, 22, 3);
        host.activate_credibility(host.revision(), next, &contracts(1)).unwrap();
        complete(&mut host, &reviewer, 11);
        assert_eq!(host.inspect().executions, 1);
    }
}
