//! Notification failures must not consume the pre-reserved recovery tail.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;

fn policy() -> PublicationChangePolicy {
    PublicationChangePolicy { source: 77, after: 0, lookup: RoutingBudget { steps: 4096, bytes: 1_048_576 } }
}

#[test]
fn failed_notice_at_ordinary_capacity_closes_admission_but_original_recovery_still_fences() {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.events = 40;
    let (mut host, reviewer) = FileOversight::create_with_publication_validation(root.store(), p.clone(), limits()).unwrap();
    host.enable_publication_changes(host.revision(), policy()).unwrap();
    // Change-profile installation is bootstrap, not work that prevents reserves.
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let before = host.inspect();
    assert_eq!(host.record_publication_change(host.revision(), PublicationChange {
        source: 77, sequence: 1, change: WitnessChange::All,
    }), Err(JournalError::Contract(Error::Limit)));
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()), Err(JournalError::Unavailable));
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
    drop(reviewer); drop(host);
    let (host, _) = FileOversight::open(root.store(), p).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
    assert!(!host.clock_ready());
}

#[test]
fn invalid_bootstrap_and_post_proposal_activation_never_change_the_acknowledged_cut() {
    let root = Directory::new(); let (mut host, _) = source_host(&root);
    let before = host.inspect();
    assert_eq!(host.enable_publication_changes(host.revision(), PublicationChangePolicy { source: 0, ..policy() }),
        Err(JournalError::Contract(Error::InvalidInput)));
    assert_eq!(host.inspect(), before);
    host.propose(host.revision(), 1, spec(&host, b"before gate"), snapshot()).unwrap();
    let before = host.inspect();
    assert_eq!(host.enable_publication_changes(host.revision(), policy()), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.inspect(), before);
    assert!(host.storage_failure().is_none());
}
