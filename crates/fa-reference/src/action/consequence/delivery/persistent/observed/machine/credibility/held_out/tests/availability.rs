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
