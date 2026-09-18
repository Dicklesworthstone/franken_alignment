//! Known missing capture must not fall back to the last quiet acknowledged image.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_consistency.rs"] mod prediction;
use ordinary::{Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::Error;

#[test]
fn capture_loss_at_exact_event_capacity_disables_the_owner_without_inventing_a_commit() {
    for capacity in [4, 5] {
        let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = capacity;
        let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
        let observer = host.enable_action_consistency(0, prediction::configuration()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        prediction::forecast(&mut host, &observer, 1, 1, -1.0);
        host.propose(host.revision(), 1, ordinary::spec(&host, b"ordinary"), snapshot()).unwrap();
        let quiet = host.action_consistency_snapshot().unwrap();
        assert_eq!(quiet.evidence.samples(), 1); assert!(!quiet.coverage_lost);
        assert!(quiet.pending_attempt.is_none()); assert_eq!(host.revision(), 4);
        let before = host.inspect();
        let result = observer.unavailable(&mut host, 4);
        if capacity == 4 {
            assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
            assert!(host.storage_failure().is_some()); assert!(!host.clock_ready());
            assert_eq!(host.action_consistency_snapshot(), Err(JournalError::Unavailable));
            assert_eq!(host.inspect(), before);
            assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
            assert_eq!(host.observe_time(4, ElapsedTick(2)), Err(JournalError::Unavailable));
        } else {
            result.unwrap(); assert!(host.storage_failure().is_none());
            assert_eq!(host.revision(), 5);
            let lost = host.action_consistency_snapshot().unwrap();
            assert!(lost.coverage_lost); assert_eq!(lost.evidence, quiet.evidence);
        }
    }
}

#[test]
fn foreign_or_stale_loss_does_not_disable_the_valid_two_key_control() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_action_consistency(0, prediction::configuration()).unwrap();
    let (mut second, _) = FileOversight::create(other.store(), profile()).unwrap();
    let foreign = second.enable_action_consistency(0, prediction::configuration()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    prediction::forecast(&mut host, &observer, 1, 1, -1.0);
    let keys = ordinary::ready(&mut host, &human, 1, b"ordinary");
    let before = host.inspect(); let revision = host.revision();
    assert_eq!(foreign.unavailable(&mut host, revision), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(observer.unavailable(&mut host, revision - 1), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    ordinary::dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn staged_loss_failure_retains_dispatched_liability_and_cannot_reuse_old_eligibility() {
    let root = Directory::new();
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_action_consistency(0, prediction::configuration()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    prediction::forecast(&mut host, &observer, 1, 1, -1.0);
    let keys = ordinary::ready(&mut host, &human, 1, b"ordinary"); ordinary::dispatch(&mut host, &keys);
    let before = host.inspect(); let revision = host.revision();
    std::fs::write(root.store().join("delivery.pending"), b"retain ambiguous evidence").unwrap();
    assert!(observer.unavailable(&mut host, revision).is_err());
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
    assert_eq!(before.control.ledger.charged, 16);
    assert_eq!(host.publish_checked(revision, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)),
        Err(JournalError::Unavailable));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"retain ambiguous evidence");
    drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert_eq!(recovered.reconcile(recovered.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    recovered.seal_unexecuted(recovered.revision(), 1).unwrap();
    assert_eq!(recovered.inspect().control.ledger.charged, 0);
}
