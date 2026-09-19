//! Feed expiry through original file/committee/two-key publication and recovery.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod capture;
use capture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationFreshnessStatus, PublicationHeartbeat};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::FileCaptureError;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::{PublicationHeartbeatFile, PUBLICATION_HEARTBEAT_BYTES};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::{Error, Snapshot};
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind};

const FEED: u64 = 41;
fn policy() -> PublicationFreshnessPolicy {
    PublicationFreshnessPolicy { clock_domain: profile().delivery.clock_domain, max_age_ticks: 3 }
}
fn change_policy() -> PublicationChangePolicy {
    PublicationChangePolicy { source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } }
}
fn pulse(generation: u64, through: u64, tick: u64) -> PublicationHeartbeat {
    PublicationHeartbeat { source: FEED, clock_domain: policy().clock_domain, generation, through, produced_at: ElapsedTick(tick) }
}
fn reader(root: &Directory) -> PublicationHeartbeatFile {
    PublicationHeartbeatFile::new(root.0.join("heartbeat.bin"), FEED).unwrap()
}
fn write(root: &Directory, heartbeat: PublicationHeartbeat) {
    let next = root.0.join("heartbeat.next");
    std::fs::write(&next, heartbeat.to_bytes().unwrap()).unwrap();
    std::fs::rename(next, root.0.join("heartbeat.bin")).unwrap();
}
fn refresh(host: &mut FileOversight, root: &Directory, now: u64) -> PublicationFreshnessStatus {
    host.refresh_publication_heartbeat(host.revision(), &reader(root), || ElapsedTick(now)).unwrap().unwrap()
}
fn configured(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = capture::source_host(root);
    host.enable_publication_changes(host.revision(), change_policy()).unwrap();
    host.enable_publication_change_freshness(host.revision(), policy()).unwrap();
    (host, reviewer)
}
fn prepared(root: &Directory) -> (FileOversight, FileHumanReviewer, Keys) {
    let (mut host, reviewer) = configured(root);
    write(root, pulse(1, 0, 1));
    assert_eq!(refresh(&mut host, root, 1).eligibility, Ok(()));
    let keys = capture::source_keys(&mut host, &reviewer, root, 1);
    (host, reviewer, keys)
}
fn dispatch(host: &mut FileOversight, keys: &Keys) -> Result<(), JournalError> {
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot())
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn original_authorization_requires_a_current_feed_even_when_all_witnesses_match() {
    let root = Directory::new(); let (mut host, _) = configured(&root);
    let (action, inputs) = capture::reviewed(&mut host, 1, b"visible");
    let original = capture::packet(1, &action, &inputs, 1, &[0, 2, 4]);
    capture::replace_source(&root, &original);
    host.bind_publication_file_source(host.revision(), 1, original, capture::requests()).unwrap();
    capture::refresh(&mut host, &root, 1);
    let before = host.inspect();
    assert!(matches!(host.authorize(host.revision(), 1, &inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.inspect(), before);
    assert_eq!(before.control.ledger.reserved, 0);
    write(&root, pulse(1, 0, 1)); refresh(&mut host, &root, 1);
    capture::refresh(&mut host, &root, 1);
    assert!(host.authorize(host.revision(), 1, &inputs, snapshot()).is_ok());
    assert_eq!(host.inspect().control.ledger.reserved, 16);
}

#[test]
fn unchanged_file_cannot_renew_expiry_but_new_heartbeat_preserves_both_original_keys() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
    host.observe_time(host.revision(), ElapsedTick(4)).unwrap();
    capture::refresh(&mut host, &root, 1);
    let before = host.inspect();
    assert_eq!(dispatch(&mut host, &keys), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(refresh(&mut host, &root, 4).eligibility, Err(Error::Stale));
    assert_eq!(dispatch(&mut host, &keys), Err(JournalError::Contract(Error::Stale)));
    write(&root, pulse(2, 0, 4));
    assert_eq!(refresh(&mut host, &root, 4).eligibility, Ok(()));
    capture::refresh(&mut host, &root, 1); dispatch(&mut host, &keys).unwrap();
    capture::refresh(&mut host, &root, 1);
    let publication = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(4)).unwrap();
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1), Ok(Reconciliation::Resolved(publication.outcome)));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn expiry_after_dispatch_seals_the_original_request_and_only_its_receipt_refunds() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
    capture::refresh(&mut host, &root, 1); dispatch(&mut host, &keys).unwrap();
    capture::refresh(&mut host, &root, 1);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(4)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
    assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    write(&root, pulse(2, 0, 4)); refresh(&mut host, &root, 4);
    let retry = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(4)).unwrap();
    assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved); assert_eq!(retry.outcome, sealed());
}

#[test]
fn producer_head_exposes_a_missing_tail_without_filling_it_or_reactivating_saved_evidence() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
    write(&root, pulse(2, 2, 1));
    assert_eq!(refresh(&mut host, &root, 1).eligibility, Err(Error::Incomplete));
    let status = host.publication_change_status().unwrap();
    assert_eq!(status.through, 0); assert_eq!(status.observed_through, 2);
    for sequence in [1, 2] {
        host.record_publication_change(host.revision(), PublicationChange { source: FEED, sequence, change: WitnessChange::All }).unwrap();
    }
    assert!(host.publication_change_status().unwrap().complete());
    assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
    capture::refresh(&mut host, &root, 1);
    assert_eq!(dispatch(&mut host, &keys), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(refresh(&mut host, &root, 1).eligibility, Ok(()));
    capture::refresh(&mut host, &root, 1); dispatch(&mut host, &keys).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn recovery_preserves_configuration_but_an_unexpired_saved_heartbeat_is_not_a_new_read() {
    let root = Directory::new(); let (mut host, reviewer) = configured(&root);
    write(&root, pulse(1, 0, 1)); let old = refresh(&mut host, &root, 1);
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let retained = host.publication_change_freshness().unwrap();
    assert_eq!(retained.policy, policy()); assert_eq!(retained.heartbeat, old.heartbeat);
    assert_eq!(retained.eligibility, Err(Error::Stale));
    assert!(!host.clock_ready());
    let new = refresh(&mut host, &root, 2);
    assert_eq!(new.eligibility, Ok(())); assert_ne!(new.acquired_epoch, old.acquired_epoch);
    assert_eq!(refresh(&mut host, &root, 4).eligibility, Err(Error::Stale));
}

#[test]
fn same_generation_conflict_survives_restart_and_cannot_use_the_old_quiet_file() {
    let root = Directory::new(); let (mut host, reviewer) = configured(&root);
    write(&root, pulse(5, 0, 1)); refresh(&mut host, &root, 1);
    write(&root, pulse(5, 0, 2));
    assert_eq!(refresh(&mut host, &root, 2).eligibility, Err(Error::Binding));
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    write(&root, pulse(5, 0, 1));
    assert_eq!(refresh(&mut host, &root, 2).eligibility, Err(Error::Binding));
    write(&root, pulse(6, 0, 2));
    assert_eq!(refresh(&mut host, &root, 2).eligibility, Ok(()));
}

#[test]
fn actual_read_loss_withdraws_eligibility_without_spending_or_refunding_keys() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
    let generation = host.publication_change_freshness().unwrap().heartbeat;
    std::fs::remove_file(root.0.join("heartbeat.bin")).unwrap();
    assert_eq!(host.refresh_publication_heartbeat(host.revision(), &reader(&root), || panic!("failed read needs no clock")),
        Ok(Err(FileCaptureError::Io(ErrorKind::NotFound))));
    assert_eq!(host.publication_change_freshness().unwrap().heartbeat, generation);
    assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
    capture::refresh(&mut host, &root, 1);
    assert_eq!(dispatch(&mut host, &keys), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
}

#[test]
fn post_read_clock_unwind_and_failed_installation_leave_the_live_owner_unavailable() {
    for unwind in [false, true] {
        let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
        let before = host.revision();
        let result = catch_unwind(AssertUnwindSafe(|| host.refresh_publication_heartbeat(before, &reader(&root), || {
            if unwind { panic!("clock source failed after actual heartbeat read"); }
            std::fs::write(root.store().join("delivery.pending"), b"inert staging collision").unwrap();
            ElapsedTick(1)
        })));
        if unwind { assert!(result.is_err()); }
        else { assert!(matches!(result.unwrap(), Err(JournalError::Io(_)))); }
        assert!(host.storage_failure().is_some());
        assert_eq!(host.revision(), before + 1);
        assert_eq!(host.publication_change_freshness(), Err(JournalError::Unavailable));
        assert_eq!(dispatch(&mut host, &keys), Err(JournalError::Unavailable));
        let actual = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(actual.executions, 0); assert_eq!(actual.control.ledger.reserved, 16);
        assert_eq!(actual.revision, before + 1);
    }
}

#[test]
fn fixed_packet_bounds_source_selection_symlinks_and_bootstrap_clock_are_checked() {
    let root = Directory::new(); let (mut host, _) = capture::source_host(&root);
    host.enable_publication_changes(host.revision(), change_policy()).unwrap();
    let before = host.inspect();
    assert_eq!(host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
        clock_domain: policy().clock_domain + 1, ..policy()
    }), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    host.enable_publication_change_freshness(host.revision(), policy()).unwrap();
    let h = pulse(1, 0, 1); let bytes = h.to_bytes().unwrap();
    assert_eq!(bytes.len(), PUBLICATION_HEARTBEAT_BYTES);
    assert_eq!(PublicationHeartbeat::from_bytes(&bytes), Ok(h));
    for length in 0..bytes.len() { assert!(PublicationHeartbeat::from_bytes(&bytes[..length]).is_err()); }
    let mut version = bytes.clone(); version[7] = b'2';
    assert_eq!(PublicationHeartbeat::from_bytes(&version), Err(Error::Binding));
    let mut trailing = bytes; trailing.push(0);
    assert_eq!(PublicationHeartbeat::from_bytes(&trailing), Err(Error::Limit));
    write(&root, h);
    let foreign = PublicationHeartbeatFile::new(root.0.join("heartbeat.bin"), FEED + 1).unwrap();
    let before = host.inspect();
    assert_eq!(host.refresh_publication_heartbeat(host.revision(), &foreign, || panic!("foreign source")), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    let link = root.0.join("heartbeat-link");
    std::os::unix::fs::symlink(root.0.join("heartbeat.bin"), &link).unwrap();
    assert_eq!(PublicationHeartbeatFile::new(link, FEED).unwrap().read_heartbeat(), Err(FileCaptureError::Data(Error::Binding)));
    write(&root, PublicationHeartbeat { clock_domain: policy().clock_domain + 1, ..h });
    assert_eq!(refresh(&mut host, &root, 1).eligibility, Err(Error::Binding));
}

#[test]
fn heartbeat_work_cannot_consume_the_reserved_recovery_tail() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 8;
    let (mut host, _) = FileOversight::create_with_publication_validation(root.store(), p.clone(), capture::limits()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_publication_changes(host.revision(), change_policy()).unwrap();
    host.enable_publication_change_freshness(host.revision(), policy()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let before = host.inspect();
    assert_eq!(host.refresh_publication_heartbeat(host.revision(), &reader(&root), || panic!("no capacity")), Err(JournalError::Contract(Error::Limit)));
    assert!(host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
}

#[test]
fn already_executed_receipts_win_over_feed_loss_and_lease_expiry() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root);
    capture::refresh(&mut host, &root, 1); dispatch(&mut host, &keys).unwrap();
    capture::refresh(&mut host, &root, 1);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    host.publication_changes_unavailable(host.revision(), FEED).unwrap();
    let result = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(4)).unwrap();
    assert_eq!(result.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 1);
}
