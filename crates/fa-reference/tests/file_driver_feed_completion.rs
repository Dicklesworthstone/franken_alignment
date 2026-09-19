//! Actual helper sockets, feed files and the original supervised completion owner.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/file_publication_capture.rs"] mod capture;
use driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::completion::FileFeedCompletionReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;
use std::cell::Cell;
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

const FEED: u64 = 41;
fn batch(generation: u64, after: u64, produced: u64, changes: &[WitnessChange]) -> PublicationFeedBatch {
    PublicationFeedBatch::new(PublicationHeartbeat { source: FEED, clock_domain: profile().delivery.clock_domain,
        generation, through: after + changes.len() as u64, produced_at: ElapsedTick(produced) }, after,
        changes.iter().enumerate().map(|(i, change)| PublicationChange {
            source: FEED, sequence: after + i as u64 + 1, change: *change,
        }).collect()).unwrap()
}
fn write(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, bytes).unwrap(); std::fs::rename(pending, path).unwrap();
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
struct Live {
    rig: Rig,
    witness: PublicationInputFile,
    witness_path: PathBuf,
    feed: PublicationFeedFile,
    feed_path: PathBuf,
    human: FileHumanPermit,
}
impl Live {
    fn new() -> Self {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            host.enable_publication_validation(revision, capture::limits()).unwrap();
            let revision = host.revision(); host.enable_publication_changes(revision, PublicationChangePolicy {
                source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
            }).unwrap();
            let revision = host.revision(); host.enable_publication_change_freshness(revision, PublicationFreshnessPolicy {
                clock_domain: profile().delivery.clock_domain, max_age_ticks: 4,
            }).unwrap();
        }
        let feed_path = rig.root.0.join("feed.bin"); write(&feed_path, &batch(1, 0, 1, &[]).to_bytes().unwrap());
        let feed = PublicationFeedFile::new(&feed_path, FEED).unwrap();
        let _ticket = rig.submit(1); rig.reviewed(1);
        let witness_path = rig.root.0.join("witness.bin");
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let original = capture::packet(1, host.request_action(1).unwrap(), rig.inputs.as_ref().unwrap(), 1, &[0, 2, 4]);
            write(&witness_path, &original.to_bytes().unwrap());
            let revision = host.revision(); host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
        }
        let witness = PublicationInputFile::new(&witness_path, capture::SOURCE).unwrap();
        let human = rig.human(1001, 31);
        Self { rig, witness, witness_path, feed, feed_path, human }
    }
    fn complete(&mut self, now: u64) -> FileFeedCompletionReport {
        let inputs = self.rig.inputs.clone();
        self.rig.driver.complete_with_publication_feed(&self.witness, &self.feed, || ElapsedTick(now),
            |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), &self.human, None)
    }
}

#[test]
fn normal_job_acquires_three_feeds_and_finishes_with_no_deferred_accounting() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let store = live.rig.root.store();
    let mut calls = 0;
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1;
        let visible = FileOversight::read_publication(&store, &profile()).unwrap();
        assert_eq!(visible.executions, 0); assert_eq!(visible.control.ledger.charged, 0);
        if calls > 1 { assert_eq!(visible.control.ledger.reserved, 16); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(calls, 3); assert_eq!(report.authorization_feeds.len(), 1);
    assert_eq!(report.authorization_reads.len(), 1); assert_eq!(report.completion.reads.len(), 2);
    assert_eq!(report.completion.committed.len(), 2); assert_eq!(report.completion.completion.reads.len(), 2);
    assert_eq!(report.completion.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    {
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.request_status(1).unwrap().disposition,
            FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Confirmed });
        assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 1);
        assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap(), host.inspect());
    }
    std::fs::remove_file(live.feed_path).unwrap(); std::fs::remove_file(live.witness_path).unwrap();
    let idle = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2),
        |_, _| panic!("settled job has no evidence or accounting obligation"), None, None);
    assert!(idle.feeds.is_empty()); assert!(idle.publication.reads.is_empty());
    assert!(matches!(idle.publication.evidence.result, Ok(FileDriverEvent::Idle)));
}

#[test]
fn final_catch_up_and_witness_capture_preserve_positive_and_phantom_outcomes() {
    for inserted in [99, 1, 3, 7] {
        let mut live = Live::new(); let inputs = live.rig.inputs.clone();
        let action = live.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let next = capture::packet(1, &action, inputs.as_ref().unwrap(), 2, &[0, 2, 4, inserted]);
        let mut calls = 0;
        let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 2 { write(&live.feed_path, &batch(2, 0, 1, &[WitnessChange::All]).to_bytes().unwrap()); }
            if calls == 3 { write(&live.witness_path, &next.to_bytes().unwrap()); }
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, &live.human, None);
        assert_eq!(calls, 3); assert_eq!(report.completion.committed[1].changes.len(), 1);
        let result = report.completion.completion.result.unwrap(); let allowed = inserted == 99;
        assert_eq!(result.basis, if allowed { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(result.outcome, if allowed { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(allowed));
        assert_eq!(host.inspect().control.ledger.charged, if allowed { 16 } else { 0 });
    }
}

#[test]
fn first_completion_feed_failure_reuses_the_existing_automatic_permit_on_retry() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1; std::fs::remove_file(&live.feed_path).unwrap();
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(calls, 1); assert_eq!(report.authorization_feeds.len(), 1);
    assert_eq!(report.completion.reads, vec![Err(FileCaptureError::Io(ErrorKind::NotFound))]);
    assert!(report.completion.committed.is_empty());
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    {
        let host = live.rig.driver.supervisor().host().unwrap();
        assert!(host.storage_failure().is_none()); assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    }
    write(&live.feed_path, &batch(1, 0, 1, &[]).to_bytes().unwrap());
    let retry = live.complete(2);
    assert!(retry.authorization_feeds.is_empty()); assert!(retry.authorization_reads.is_empty());
    assert_eq!(retry.completion.reads.len(), 2);
    assert_eq!(retry.completion.completion.result.unwrap().basis, PublicationBasis::Revalidated);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn second_completion_feed_loss_finishes_with_native_nonexecution_and_no_committee_drift() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1; if calls == 2 { std::fs::remove_file(&live.feed_path).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(calls, 3); assert_eq!(report.completion.committed.len(), 1);
    assert_eq!(report.completion.reads[1], Err(FileCaptureError::Io(ErrorKind::NotFound)));
    assert_eq!(report.completion.completion.evidence_failure, None);
    assert_eq!(report.completion.completion.result.unwrap().outcome, sealed());
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn missing_authorization_window_can_be_repaired_without_replacing_the_review() {
    let mut live = Live::new();
    write(&live.feed_path, &batch(2, 1, 1, &[WitnessChange::All]).to_bytes().unwrap());
    let report = live.complete(2);
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert!(report.completion.reads.is_empty());
    assert!(!report.authorization_feeds[0].as_ref().unwrap().status.complete());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    write(&live.feed_path, &batch(2, 0, 1, &[WitnessChange::All; 2]).to_bytes().unwrap());
    let report = live.complete(2);
    assert_eq!(report.authorization_feeds[0].as_ref().unwrap().changes.len(), 2);
    assert_eq!(report.completion.completion.result.unwrap().basis, PublicationBasis::Revalidated);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
}

#[test]
fn history_conflict_between_completion_reads_never_returns_installed_feed_reports() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone();
    let original = capture::observations(inputs.as_ref().unwrap(), 1, &[0, 2, 4]);
    let domain = original.structured().unwrap().snapshot().domain_input().domain();
    let mut calls = 0;
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1;
        let notice = if calls == 1 { WitnessChange::All } else { WitnessChange::Key { domain, key: 99 } };
        write(&live.feed_path, &batch(calls + 1, 0, 1, &[notice]).to_bytes().unwrap());
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(calls, 2); assert_eq!(report.authorization_feeds.len(), 1);
    assert!(report.completion.committed.is_empty()); assert_eq!(report.completion.reads.len(), 2);
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let host = live.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(FileOversight::read_publication(live.rig.root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn final_expiry_or_changed_policy_is_not_overridden_by_identical_witness_files() {
    for expiry in [false, true] {
        let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let now = Cell::new(2); let mut calls = 0;
        let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(now.get()), |_, _| {
            calls += 1; let mut state = snapshot();
            if calls == 3 {
                if expiry { now.set(5); } else { state.values.insert(7, b"changed".to_vec()); }
            }
            Ok(DriverEvidence { snapshot: state, inputs: inputs.clone() })
        }, &live.human, None);
        assert_eq!(report.completion.committed[1].freshness.eligibility, Ok(()));
        let result = report.completion.completion.result.unwrap();
        assert_eq!(result.basis, PublicationBasis::Rejected(if expiry { Error::Stale } else { Error::Binding }));
        assert_eq!(result.outcome, sealed()); assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    }
}

#[test]
fn caught_final_provider_unwind_cannot_automatically_redispatch() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| live.rig.driver.complete_with_publication_feed(
        &live.witness, &live.feed, || ElapsedTick(2), |_, _| {
            calls += 1; assert!(calls != 3, "interrupted final provider");
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, &live.human, None))).is_err());
    assert_eq!(calls, 3); assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let retry = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed,
        || panic!("unavailable owner"), |_, _| panic!("no redispatch"), &live.human, None);
    assert_eq!(retry.completion.completion.result, Err(JournalError::Unavailable));
    assert!(retry.authorization_feeds.is_empty()); assert!(retry.completion.reads.is_empty());
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(FileOversight::read_publication(live.rig.root.store(), &profile()).unwrap(), host.inspect());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 16);
}
