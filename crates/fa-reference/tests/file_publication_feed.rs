//! Actual file transport through the original review, two-key and receipt paths.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod capture;
use capture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::FileCaptureError;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{
    PublicationFeedBatch, PublicationFeedFile, PublicationFeedReport, MAX_FEED_BYTES, MAX_FEED_RECORDS,
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationEvidence, FilePublicationInputs};
use fa_reference::action::consequence::delivery::publication_gate::changes::{ChangeRouting, PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::witness::DomainProjection;
use fa_reference::product_frontier::ProjectionKey;
use fa_reference::{Error, Snapshot};
use std::panic::{AssertUnwindSafe, catch_unwind};

const FEED: u64 = 41;
fn policy() -> PublicationChangePolicy {
    PublicationChangePolicy { source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } }
}
fn freshness() -> PublicationFreshnessPolicy {
    PublicationFreshnessPolicy { clock_domain: profile().delivery.clock_domain, max_age_ticks: 3 }
}
fn domain() -> DomainProjection {
    DomainProjection::new(40, 1, ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 })
}
fn pulse(generation: u64, through: u64, tick: u64) -> PublicationHeartbeat {
    PublicationHeartbeat { source: FEED, clock_domain: freshness().clock_domain,
        generation, through, produced_at: ElapsedTick(tick) }
}
fn batch(generation: u64, after: u64, keys: &[u64], tick: u64) -> PublicationFeedBatch {
    PublicationFeedBatch::new(pulse(generation, after + keys.len() as u64, tick), after,
        keys.iter().enumerate().map(|(i, key)| PublicationChange { source: FEED, sequence: after + i as u64 + 1,
            change: WitnessChange::Key { domain: domain(), key: *key } }).collect()).unwrap()
}
fn reader(root: &Directory) -> PublicationFeedFile {
    PublicationFeedFile::new(root.0.join("feed.bin"), FEED).unwrap()
}
fn write(root: &Directory, batch: &PublicationFeedBatch) {
    let next = root.0.join("feed.next");
    std::fs::write(&next, batch.to_bytes().unwrap()).unwrap();
    std::fs::rename(next, root.0.join("feed.bin")).unwrap();
}
fn refresh(host: &mut FileOversight, root: &Directory, tick: u64) -> PublicationFeedReport {
    host.refresh_publication_feed(host.revision(), &reader(root), || ElapsedTick(tick)).unwrap().unwrap()
}
fn configured(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), policy(), freshness()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    write(root, &batch(1, 0, &[], 1));
    assert_eq!(refresh(&mut host, root, 1).freshness.eligibility, Ok(()));
    (host, reviewer)
}
fn prepared(root: &Directory) -> (FileOversight, Keys) {
    let (mut host, reviewer) = configured(root);
    let (action, inputs) = capture::reviewed(&mut host, 1, b"visible");
    let original = capture::observations(&inputs, 1, &[0, 2, 4]);
    // Structured-only judgments retain precise routing; opaque cases use the
    // existing source fixture separately and must conservatively recapture.
    let original = FilePublicationInputs::new(original.structured().cloned(), None);
    host.bind_publication_evidence(host.revision(), 1,
        FilePublicationEvidence::new(original.clone(), capture::requests()).unwrap()).unwrap();
    host.record_publication_inputs(host.revision(), 1, 0, Some(original)).unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    (host, Keys { action, inputs, automatic, human, request })
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn packet_round_trip_preserves_all_native_notices_and_rejects_incomplete_framing() {
    let changes = [WitnessChange::Key { domain: domain(), key: u64::MAX },
        WitnessChange::Range { domain: domain(), start: 0, end: u64::MAX },
        WitnessChange::Domain { domain: domain() }, WitnessChange::All];
    let records = changes.into_iter().enumerate().map(|(i, change)| PublicationChange {
        source: FEED, sequence: i as u64 + 1, change,
    }).collect();
    let original = PublicationFeedBatch::new(pulse(1, 4, 1), 0, records).unwrap();
    let bytes = original.to_bytes().unwrap();
    assert_eq!(PublicationFeedBatch::from_bytes(&bytes).unwrap(), original);
    for end in 0..bytes.len() { assert!(PublicationFeedBatch::from_bytes(&bytes[..end]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(PublicationFeedBatch::from_bytes(&trailing).is_err());
    let mut version = bytes; version[7] = b'2';
    assert_eq!(PublicationFeedBatch::from_bytes(&version), Err(Error::Binding));
    let mut records = original.records().to_vec(); records[1].sequence += 1;
    assert_eq!(PublicationFeedBatch::new(pulse(1, 4, 1), 0, records), Err(Error::Incomplete));
    let mut records = original.records().to_vec(); records[0].source += 1;
    assert_eq!(PublicationFeedBatch::new(pulse(1, 4, 1), 0, records), Err(Error::Binding));
}

#[test]
fn bounded_file_and_sequence_limits_have_positive_neighbors() {
    let root = Directory::new();
    let keys = vec![u64::MAX; MAX_FEED_RECORDS];
    let full = batch(1, u64::MAX - MAX_FEED_RECORDS as u64, &keys, 1);
    write(&root, &full);
    assert_eq!(reader(&root).read_batch().unwrap(), full);
    assert!(full.to_bytes().unwrap().len() <= MAX_FEED_BYTES);
    let records = vec![PublicationChange { source: FEED, sequence: 1, change: WitnessChange::All }; MAX_FEED_RECORDS + 1];
    assert_eq!(PublicationFeedBatch::new(pulse(1, 0, 1), 0, records), Err(Error::Limit));
    let empty = PublicationFeedBatch::new(pulse(1, u64::MAX, 1), u64::MAX, vec![]).unwrap();
    assert_eq!(PublicationFeedBatch::from_bytes(&empty.to_bytes().unwrap()).unwrap(), empty);
    assert!(PublicationFeedBatch::new(pulse(1, 0, 1), u64::MAX, vec![]).is_err());
    std::fs::write(root.0.join("feed.bin"), vec![0; MAX_FEED_BYTES + 1]).unwrap();
    assert_eq!(reader(&root).read_batch(), Err(FileCaptureError::Data(Error::Limit)));
    std::fs::rename(root.0.join("feed.bin"), root.0.join("other.bin")).unwrap();
    std::os::unix::fs::symlink(root.0.join("other.bin"), root.0.join("feed.bin")).unwrap();
    assert_eq!(reader(&root).read_batch(), Err(FileCaptureError::Data(Error::Binding)));
}

#[test]
fn concrete_catchup_preserves_unrelated_review_and_original_two_key_publication() {
    let root = Directory::new(); let (mut host, keys) = prepared(&root);
    let revision = host.revision(); let input_revision = host.publication_input_revision(1).unwrap();
    write(&root, &batch(2, 0, &[98, 99], 1));
    let report = refresh(&mut host, &root, 1);
    assert_eq!(host.revision(), revision + 4); // withdrawal, two notices, heartbeat
    assert_eq!(report.changes.len(), 2);
    assert!(report.changes.iter().all(|change| change.routing == ChangeRouting::Indexed && change.affected.is_empty()));
    assert_eq!(report.status.through, 2); assert_eq!(report.freshness.eligibility, Ok(()));
    assert_eq!(host.publication_input_revision(1).unwrap(), input_revision);
    capture::dispatch(&mut host, &keys);
    let publication = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1), Ok(Reconciliation::Resolved(publication.outcome)));
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn an_actual_relevant_record_after_dispatch_seals_without_refunding_until_receipt() {
    let root = Directory::new(); let (mut host, keys) = prepared(&root);
    capture::dispatch(&mut host, &keys);
    write(&root, &batch(2, 0, &[1], 1));
    assert_eq!(refresh(&mut host, &root, 1).changes[0].affected, vec![1]);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete)); assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    std::fs::remove_file(root.0.join("feed.bin")).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(2)).unwrap().outcome, sealed());
}

#[test]
fn rolling_overlap_is_exactly_once_and_rewritten_notices_quarantine_the_owner() {
    let root = Directory::new(); let (mut host, _) = configured(&root);
    write(&root, &batch(2, 0, &[10, 11], 1)); refresh(&mut host, &root, 1);
    let before = host.revision();
    assert!(refresh(&mut host, &root, 1).changes.is_empty());
    assert_eq!(host.revision(), before + 2); // no duplicate notification
    write(&root, &batch(3, 1, &[11, 12], 2));
    let report = refresh(&mut host, &root, 2);
    assert_eq!(report.before, 2); assert_eq!(report.changes.len(), 1); assert_eq!(report.status.through, 3);
    let before = host.revision();
    write(&root, &batch(4, 1, &[999, 12], 2));
    assert_eq!(host.refresh_publication_feed(before, &reader(&root), || ElapsedTick(2)), Err(JournalError::Contract(Error::Binding)));
    assert!(host.storage_failure().is_some()); assert_eq!(host.revision(), before + 1);
    assert_eq!(host.publication_change_status(), Err(JournalError::Unavailable));
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 3);
    assert!(host.publication_change_freshness().unwrap().eligibility.is_err());
}

#[test]
fn too_new_window_never_fills_a_gap_and_full_repair_withdraws_interim_inputs() {
    let root = Directory::new(); let (mut host, keys) = prepared(&root);
    write(&root, &batch(2, 2, &[99], 1));
    let gap = refresh(&mut host, &root, 1);
    assert!(gap.changes.is_empty()); assert_eq!(gap.status.through, 0); assert_eq!(gap.status.observed_through, 3);
    assert_eq!(gap.freshness.eligibility, Err(Error::Incomplete));
    let original = host.retained_publication_evidence(1).unwrap().original().clone();
    host.record_publication_inputs(host.revision(), 1, host.publication_input_revision(1).unwrap(), Some(original)).unwrap();
    write(&root, &batch(2, 0, &[97, 98, 99], 1));
    let repaired = refresh(&mut host, &root, 1);
    assert_eq!(repaired.freshness.eligibility, Ok(()));
    assert!(repaired.changes.iter().all(|change| change.routing == ChangeRouting::RecoveringTail && change.affected == vec![1]));
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect().control.ledger.reserved, 16); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn read_loss_and_stalled_windows_cannot_renew_a_lease() {
    let root = Directory::new(); let (mut host, keys) = prepared(&root);
    std::fs::remove_file(root.0.join("feed.bin")).unwrap();
    assert!(matches!(host.refresh_publication_feed(host.revision(), &reader(&root), || panic!("failed read must not sample time")), Ok(Err(FileCaptureError::Io(_)))));
    assert!(host.publication_change_freshness().unwrap().eligibility.is_err());
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete)));
    write(&root, &batch(1, 0, &[], 1));
    assert_eq!(refresh(&mut host, &root, 4).freshness.eligibility, Err(Error::Stale));
    write(&root, &batch(2, 0, &[], 4));
    assert_eq!(refresh(&mut host, &root, 4).freshness.eligibility, Ok(()));
    capture::dispatch(&mut host, &keys);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Dispatching);
}

#[test]
fn final_replacement_failure_leaves_no_partially_acknowledged_notification_prefix() {
    let root = Directory::new(); let (mut host, _) = configured(&root);
    write(&root, &batch(2, 0, &[10, 11, 12], 1));
    let before = host.revision();
    let result = host.refresh_publication_feed(before, &reader(&root), || {
        std::fs::write(root.store().join("delivery.pending"), b"occupied").unwrap();
        ElapsedTick(1)
    });
    assert!(matches!(result, Err(JournalError::Io(_))));
    assert!(host.storage_failure().is_some()); assert_eq!(host.revision(), before + 1);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 0);
    assert!(host.publication_change_freshness().unwrap().eligibility.is_err());
    assert_eq!(refresh(&mut host, &root, 1).changes.len(), 3);
    assert_eq!(host.publication_change_status().unwrap().through, 3);
}

#[test]
fn post_read_clock_unwind_cannot_keep_old_feed_eligibility() {
    let root = Directory::new(); let (mut host, _) = configured(&root);
    let before = host.revision();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = host.refresh_publication_feed(before, &reader(&root), || panic!("clock failed"));
    }));
    assert!(outcome.is_err()); assert_eq!(host.revision(), before + 1);
    assert!(host.storage_failure().is_some());
    assert_eq!(host.publication_change_freshness(), Err(JournalError::Unavailable));
}

#[test]
fn full_batch_capacity_is_checked_before_any_suffix_is_published() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 8;
    let (mut host, _) = FileOversight::create_with_publication_change_freshness(root.store(), p.clone(),
        capture::limits(), policy(), freshness()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    write(&root, &batch(1, 0, &[], 1)); refresh(&mut host, &root, 1); // six events
    write(&root, &batch(2, 0, &[10, 11], 1));
    assert_eq!(host.refresh_publication_feed(host.revision(), &reader(&root), || ElapsedTick(1)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.revision(), 7); assert!(host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap().revision, 7);
}
