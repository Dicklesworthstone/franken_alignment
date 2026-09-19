//! Concrete change feeds and original two-key effects share one final sink cut.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::completion::CapturedCompletionKeys;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;
use std::cell::Cell;
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind};

const FEED: u64 = 41;
fn batch(generation: u64, after: u64, changes: &[WitnessChange]) -> PublicationFeedBatch {
    let heartbeat = PublicationHeartbeat { source: FEED, clock_domain: profile().delivery.clock_domain,
        generation, through: after + changes.len() as u64, produced_at: ElapsedTick(1) };
    PublicationFeedBatch::new(heartbeat, after, changes.iter().enumerate().map(|(i, change)| {
        PublicationChange { source: FEED, sequence: after + i as u64 + 1, change: *change }
    }).collect()).unwrap()
}
fn write(root: &Directory, batch: &PublicationFeedBatch) {
    let path = root.0.join("feed.next");
    std::fs::write(&path, batch.to_bytes().unwrap()).unwrap();
    std::fs::rename(path, root.0.join("feed.bin")).unwrap();
}
fn reader(root: &Directory) -> PublicationFeedFile {
    PublicationFeedFile::new(root.0.join("feed.bin"), FEED).unwrap()
}
fn setup(root: &Directory, p: FileOversightProfile) -> (FileOversight, Keys) {
    let changes = PublicationChangePolicy { source: FEED, after: 0,
        lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } };
    let freshness = PublicationFreshnessPolicy { clock_domain: p.delivery.clock_domain, max_age_ticks: 4 };
    let (mut host, reviewer) = FileOversight::create_with_publication_change_freshness(
        root.store(), p, fixture::limits(), changes, freshness).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    write(root, &batch(1, 0, &[]));
    host.refresh_publication_feed(host.revision(), &reader(root), || ElapsedTick(1)).unwrap().unwrap();
    let keys = fixture::source_keys(&mut host, &reviewer, root, 1);
    (host, keys)
}
fn keys(k: &Keys) -> CapturedCompletionKeys<'_> {
    CapturedCompletionKeys { automatic: &k.automatic, human: &k.human, credential: None }
}
fn evidence(k: &Keys) -> Result<DriverEvidence, Error> {
    Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(k.inputs.clone()) })
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn unpublished(root: &Directory, expected_revision: u64) {
    let visible = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(visible.revision, expected_revision);
    assert_eq!(visible.executions, 0);
    assert_eq!(visible.control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(visible.control.ledger.reserved, 16);
    assert_eq!(visible.control.ledger.charged, 0);
}

#[test]
fn catch_up_then_two_independent_captures_publish_and_settle_once() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile());
    write(&root, &batch(2, 0, &[WitnessChange::All]));
    let before = host.revision(); let mut calls = 0;
    let report = host.complete_publication_from_feed(before, keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| {
            calls += 1; unpublished(&root, before + 2); evidence(&k)
        });
    assert_eq!(calls, 2); assert_eq!(report.reads.len(), 2);
    assert_eq!(report.completion.reads.len(), 2);
    assert_eq!(report.committed.len(), 2);
    assert_eq!(report.committed[0].changes.len(), 1);
    assert!(report.committed[1].changes.is_empty());
    let result = report.completion.result.unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.revision(), before + 13);
    assert_eq!(host.publication_change_status().unwrap().through, 1);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    let again = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
        &reader(&root), || panic!("settled keys cannot read time"), |_, _| panic!("no repeated acquisition"));
    assert_eq!(again.completion.result, Err(JournalError::Contract(Error::WrongState)));
    assert!(again.reads.is_empty()); assert!(again.committed.is_empty());
}

#[test]
fn notifications_between_dispatch_and_publication_cannot_stale_the_capture_revision() {
    for rows in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 3, 4], vec![0, 2, 4, 7], vec![0, 2, 4, 99]] {
        let root = Directory::new(); let (mut host, k) = setup(&root, profile());
        let mut calls = 0;
        let report = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
            &reader(&root), || ElapsedTick(2), |_, _| {
                calls += 1;
                if calls == 1 { write(&root, &batch(2, 0, &[WitnessChange::All])); }
                if calls == 2 { fixture::replace_source(&root, &fixture::packet(1, &k.action, &k.inputs, 2, &rows)); }
                evidence(&k)
            });
        assert_eq!(calls, 2); assert_eq!(report.committed[1].changes.len(), 1);
        assert_eq!(report.completion.evidence_failure, None);
        let result = report.completion.result.unwrap(); let allowed = rows.contains(&99);
        assert_eq!(result.basis, if allowed { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(result.outcome, if allowed { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(host.inspect().executions, u64::from(allowed));
        assert_eq!(host.inspect().control.ledger.charged, if allowed { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.reconcile(host.revision(), 1), Ok(Reconciliation::Resolved(result.outcome)));
    }
}

#[test]
fn overlap_is_checked_against_first_staged_read_not_just_the_old_live_history() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile());
    write(&root, &batch(2, 0, &[WitnessChange::All]));
    let original = fixture::observations(&k.inputs, 1, &[0, 2, 4]);
    let domain = original.structured().unwrap().snapshot().domain_input().domain();
    let before = host.revision(); let mut calls = 0;
    let report = host.complete_publication_from_feed(before, keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| {
            calls += 1;
            write(&root, &batch(3, 0, &[WitnessChange::Key { domain, key: 99 }]));
            evidence(&k)
        });
    assert_eq!(calls, 1); assert_eq!(report.reads.len(), 2);
    assert!(report.reads.iter().all(Result::is_ok));
    assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert!(report.committed.is_empty()); assert!(host.storage_failure().is_some());
    unpublished(&root, before + 2);
    assert_eq!(host.publication_change_status(), Err(JournalError::Unavailable));
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 0);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn first_feed_loss_keeps_both_original_keys_retryable_without_a_second_reservation() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile());
    std::fs::remove_file(root.0.join("feed.bin")).unwrap(); let before = host.revision();
    let report = host.complete_publication_from_feed(before, keys(&k), &fixture::source(&root),
        &reader(&root), || panic!("failed feed cannot sample time"), |_, _| panic!("failed first feed precedes provider"));
    assert_eq!(report.reads, vec![Err(FileCaptureError::Io(ErrorKind::NotFound))]);
    assert!(report.committed.is_empty()); assert!(report.completion.reads.is_empty());
    assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert!(host.storage_failure().is_none()); unpublished(&root, before + 2);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    write(&root, &batch(1, 0, &[]));
    let retried = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| evidence(&k));
    assert_eq!(retried.completion.result.unwrap().basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 1);
}

#[test]
fn second_feed_loss_seals_and_settles_without_fabricating_committee_drift() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile()); let mut calls = 0;
    let report = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 1 { std::fs::remove_file(root.0.join("feed.bin")).unwrap(); }
            evidence(&k)
        });
    assert_eq!(calls, 2); assert_eq!(report.reads[1], Err(FileCaptureError::Io(ErrorKind::NotFound)));
    assert_eq!(report.committed.len(), 1); assert_eq!(report.completion.evidence_failure, None);
    let result = report.completion.result.unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete)); assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn missing_second_window_commits_the_gap_and_cannot_publish_matching_witnesses() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile()); let mut calls = 0;
    let report = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 1 { write(&root, &batch(2, 2, &[WitnessChange::All])); }
            evidence(&k)
        });
    assert!(report.committed[1].changes.is_empty());
    assert_eq!(report.committed[1].status.through, 0); assert_eq!(report.committed[1].status.observed_through, 3);
    assert_eq!(report.completion.result.unwrap().outcome, sealed());
    assert!(!host.publication_change_status().unwrap().complete());
    write(&root, &batch(2, 0, &[WitnessChange::All; 3]));
    let repaired = host.refresh_publication_feed(host.revision(), &reader(&root), || ElapsedTick(2)).unwrap().unwrap();
    assert!(repaired.status.complete()); assert_eq!(repaired.changes.len(), 3);
    assert_eq!(host.reconcile(host.revision(), 1), Ok(Reconciliation::Resolved(sealed())));
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn final_time_rechecks_feed_expiry_after_successful_second_feed_acquisition() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile());
    let now = Cell::new(2); let mut calls = 0;
    let report = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(now.get()), |_, _| {
            calls += 1; if calls == 2 { now.set(5); } evidence(&k)
        });
    assert_eq!(report.committed[1].freshness.eligibility, Ok(()));
    assert_eq!(report.completion.result.unwrap().basis, PublicationBasis::Rejected(Error::Stale));
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn second_feed_clock_unwind_cannot_expose_staged_dispatch_or_restore_old_coverage() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile()); let before = host.revision();
    let mut clocks = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| host.complete_publication_from_feed(before, keys(&k),
        &fixture::source(&root), &reader(&root), || {
            clocks += 1; assert!(clocks != 3, "interrupted after second feed read"); ElapsedTick(2)
        }, |_, _| evidence(&k)))).is_err());
    assert_eq!(clocks, 3); assert!(host.storage_failure().is_some()); unpublished(&root, before + 2);
    drop(host); let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
    assert!(host.publication_change_freshness().unwrap().eligibility.is_err());
}

#[test]
fn failed_final_replace_returns_no_acknowledged_feed_reports_or_speculative_effect() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile()); let before = host.revision();
    let mut calls = 0;
    let report = host.complete_publication_from_feed(before, keys(&k), &fixture::source(&root),
        &reader(&root), || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 2 { std::fs::write(root.store().join("delivery.pending"), b"stage obstruction").unwrap(); }
            evidence(&k)
        });
    assert!(report.reads.iter().all(Result::is_ok)); assert_eq!(report.reads.len(), 2);
    assert!(report.committed.is_empty()); assert!(matches!(report.completion.result, Err(JournalError::Io(_))));
    assert!(host.storage_failure().is_some()); unpublished(&root, before + 2);
    drop(host); let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn fixed_capacity_refuses_before_reads_and_real_suffix_capacity_cannot_leak_partial_installation() {
    let baseline_root = Directory::new(); let (baseline, _) = setup(&baseline_root, profile());
    let before = baseline.revision();
    for extra in [11, 12] {
        let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = before as usize + extra;
        let (mut host, k) = setup(&root, p.clone());
        write(&root, &batch(2, 0, &[WitnessChange::All]));
        let report = host.complete_publication_from_feed(host.revision(), keys(&k), &fixture::source(&root),
            &reader(&root), || ElapsedTick(2), |_, _| evidence(&k));
        assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Limit)));
        assert!(report.committed.is_empty());
        assert_eq!(host.revision(), before + if extra == 11 { 0 } else { 2 });
        assert_eq!(host.storage_failure().is_some(), extra == 12);
        assert_eq!(report.reads.is_empty(), extra == 11);
        assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), host.inspect());
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
}

#[test]
fn foreign_source_or_human_is_rejected_before_durable_withdrawal_or_external_code() {
    let root = Directory::new(); let (mut host, k) = setup(&root, profile());
    let other_root = Directory::new(); let (_, other) = setup(&other_root, profile());
    let before = host.inspect();
    let wrong_source = PublicationInputFile::new(root.0.join("absent"), fixture::SOURCE + 1).unwrap();
    let report = host.complete_publication_from_feed(host.revision(), keys(&k), &wrong_source, &reader(&root),
        || panic!("wrong source"), |_, _| panic!("wrong source"));
    assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Binding)));
    let report = host.complete_publication_from_feed(host.revision(), CapturedCompletionKeys {
        automatic: &k.automatic, human: &other.human, credential: None,
    }, &fixture::source(&root), &reader(&root), || panic!("foreign human"), |_, _| panic!("foreign human"));
    assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert!(report.reads.is_empty()); assert_eq!(host.inspect(), before);
}
