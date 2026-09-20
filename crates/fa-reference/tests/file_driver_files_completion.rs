//! Real helper-socket jobs with concrete native source capture at every boundary.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod files;
#[path = "support/file_publication_capture.rs"] mod capture;
use files::driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::completion::files::FileFilesCompletionReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::{CheckedPublication, PublicationBasis};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationHeartbeat, PublicationFreshnessPolicy};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

const FEED: u64 = 41;
fn write(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, bytes).unwrap(); std::fs::rename(pending, path).unwrap();
}
fn write_feed(path: &Path, generation: u64, through: u64, now: u64) {
    let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: FEED,
        clock_domain: profile().delivery.clock_domain, generation, through, produced_at: ElapsedTick(now) },
        0, (1..=through).map(|sequence| PublicationChange { source: FEED, sequence, change: WitnessChange::All }).collect()).unwrap();
    write(path, &batch.to_bytes().unwrap());
}
fn outcome(report: FileFilesCompletionReport) -> CheckedPublication {
    report.completion.completion.completion.result.unwrap()
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
struct Live {
    file: files::FileRig,
    witness: PublicationInputFile,
    feed: Option<PublicationFeedFile>,
    human: FileHumanPermit,
}
impl Live {
    fn new(with_feed: bool) -> Self {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            host.enable_publication_validation(revision, capture::limits()).unwrap();
            if with_feed {
                let revision = host.revision();
                host.enable_publication_changes(revision, PublicationChangePolicy {
                    source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
                }).unwrap();
                let revision = host.revision();
                host.enable_publication_change_freshness(revision, PublicationFreshnessPolicy {
                    clock_domain: profile().delivery.clock_domain, max_age_ticks: 20,
                }).unwrap();
            }
        }
        let feed = with_feed.then(|| {
            let path = rig.root.0.join("feed.bin"); write_feed(&path, 1, 0, 1);
            PublicationFeedFile::new(path, FEED).unwrap()
        });
        let path = rig.root.0.join("policy.json"); files::replace(&path, &files::document(1));
        let mut source = FileEvidenceSource::new(&path, files::SOURCE, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            host.enable_file_source(revision, FileSourcePolicy {
                source: StateSource { scope: profile().delivery.scope, source: files::SOURCE, generation: 1 },
                limits: StateLimits::default(), freshness: StateFreshness::new(3).unwrap(),
            }).unwrap();
            let revision = host.revision(); host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
        }
        let proposal = rig.proposal(); let revision = rig.driver.supervisor().host().unwrap().revision();
        rig.driver.supervisor_mut().set_snapshot(revision, Some(files::document(1).snapshot().clone())).unwrap();
        let _ticket = rig.port.submit(1, &proposal).unwrap();
        let mut file = files::FileRig { rig, source, path }; files::reviewed(&mut file);
        let witness_path = file.rig.root.0.join("witness.bin");
        {
            let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
            let action = host.request_action(1).unwrap();
            let inputs = files::document(1).inputs_for(action, &profile().committee).unwrap();
            let original = capture::packet(1, action, &inputs, 1, &[0, 2, 4]);
            write(&witness_path, &original.to_bytes().unwrap());
            let revision = host.revision(); host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
        }
        let human = files::human(&mut file, 1);
        Self { file, witness: PublicationInputFile::new(witness_path, capture::SOURCE).unwrap(), feed, human }
    }
    fn complete(&mut self, now: u64) -> FileFilesCompletionReport {
        self.file.rig.driver.complete_from_files_with_publication(&mut self.file.source,
            &self.witness, self.feed.as_ref(), || ElapsedTick(now), &self.human, None)
    }
}

#[test]
fn real_file_job_renews_native_leases_three_times_and_finishes_without_a_settlement_turn() {
    for with_feed in [false, true] {
        let mut live = Live::new(with_feed); let baseline = files::capture_count(&live.file);
        let report = live.complete(5);
        assert_eq!(report.authorization_observations, vec![Ok(files::document(1).identity())]);
        assert_eq!(report.authorization_source_updates, vec![Ok(files::document(1).identity())]);
        assert_eq!(report.authorization_feeds.len(), usize::from(with_feed));
        assert_eq!(report.completion.observations, vec![Ok(files::document(1).identity()); 2]);
        assert_eq!(report.completion.committed_source_updates, vec![Ok(files::document(1).identity()); 2]);
        assert_eq!(report.completion.completion.committed.len(), if with_feed { 2 } else { 0 });
        assert_eq!(outcome(report).outcome, EndpointOutcome::Executed { resulting_version: 2 });
        assert_eq!(files::capture_count(&live.file), baseline + 3);
        assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::Idle);
        let host = live.file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.request_status(1).unwrap().disposition,
            FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Confirmed });
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(FileOversight::read_publication(live.file.rig.root.store(), &profile()).unwrap(), host.inspect());
        std::fs::remove_file(&live.file.path).unwrap();
        let reads = live.file.source.status().read_attempts;
        let idle = live.file.rig.driver.step_from_file(&mut live.file.source, || ElapsedTick(6), None);
        assert!(matches!(idle.result, Ok(FileDriverEvent::Idle)));
        assert_eq!(live.file.source.status().read_attempts, reads);
    }
}

#[test]
fn final_native_source_loss_seals_and_closes_the_job_without_a_callback_fallback() {
    let mut live = Live::new(false); let path = live.file.path.clone(); let mut calls = 0;
    let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
        &live.witness, None, || {
            calls += 1; if calls == 5 { std::fs::remove_file(&path).unwrap(); }
            ElapsedTick(2)
        }, &live.human, None);
    assert_eq!(calls, 6);
    assert_eq!(report.completion.observations[1], Err(EvidenceError::Io(std::io::ErrorKind::NotFound)));
    assert_eq!(report.completion.committed_source_updates[1], Err(Error::Incomplete));
    assert_eq!(outcome(report).outcome, sealed());
    assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::Idle);
    let host = live.file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.request_status(1).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::ConfirmedNotExecuted });
}

#[test]
fn new_policy_generation_after_dispatch_preserves_history_but_not_old_approval() {
    let mut live = Live::new(false); let path = live.file.path.clone(); let mut calls = 0;
    let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
        &live.witness, None, || {
            calls += 1; if calls == 5 { files::replace(&path, &files::document(2)); }
            ElapsedTick(2)
        }, &live.human, None);
    assert_eq!(report.completion.observations, vec![Ok(files::document(1).identity()), Ok(files::document(2).identity())]);
    let result = outcome(report);
    assert!(matches!(result.basis, PublicationBasis::Rejected(_))); assert_eq!(result.outcome, sealed());
    assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::Idle);
    let host = live.file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.file_source_status().unwrap().producer, Some(files::document(2).identity()));
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn feed_loss_before_completion_retries_the_same_automatic_permit_without_reauthorization() {
    let mut live = Live::new(true); let path = live.file.rig.root.0.join("feed.bin"); let mut calls = 0;
    let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
        &live.witness, live.feed.as_ref(), || {
            calls += 1; if calls == 3 { std::fs::remove_file(&path).unwrap(); }
            ElapsedTick(2)
        }, &live.human, None);
    assert_eq!(report.authorization_observations.len(), 1);
    assert!(report.completion.observations.is_empty());
    assert_eq!(report.completion.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    assert_eq!(live.file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    write_feed(&path, 2, 0, 2);
    let retry = live.complete(2);
    assert!(retry.authorization_observations.is_empty()); assert!(retry.authorization_reads.is_empty());
    assert_eq!(retry.completion.observations.len(), 2);
    assert_eq!(outcome(retry).outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(live.file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn staged_feed_change_and_witness_capture_distinguish_phantoms_from_unrelated_values() {
    for inserted in [1, 99] {
        let mut live = Live::new(true); let root = live.file.rig.root.0.clone();
        let action = live.file.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let inputs = files::document(1).inputs_for(&action, &profile().committee).unwrap();
        let next = capture::packet(1, &action, &inputs, 2, &[0, 2, 4, inserted]).to_bytes().unwrap();
        let mut calls = 0;
        let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
            &live.witness, live.feed.as_ref(), || {
                calls += 1;
                if calls == 6 { write_feed(&root.join("feed.bin"), 2, 1, 2); write(&root.join("witness.bin"), &next); }
                ElapsedTick(2)
            }, &live.human, None);
        assert_eq!(report.completion.completion.committed.len(), 2);
        assert_eq!(report.completion.completion.committed[1].changes.len(), 1);
        let result = outcome(report);
        assert_eq!(result.outcome, if inserted == 99 { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(live.file.rig.driver.supervisor().host().unwrap().inspect().executions, u64::from(inserted == 99));
    }
}

#[test]
fn final_native_lease_expiry_has_a_permitted_neighbor_and_cannot_be_retimestamped() {
    for final_tick in [7, 8] {
        let mut live = Live::new(false); let mut calls = 0;
        let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
            &live.witness, None, || {
                calls += 1; ElapsedTick(if calls == 6 { final_tick } else { 5 })
            }, &live.human, None);
        assert_eq!(calls, 6); assert_eq!(report.completion.observations.len(), 2);
        let result = outcome(report);
        assert_eq!(result.outcome, if final_tick == 7 { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::Idle);
    }
}

#[test]
fn failed_final_write_and_caught_clock_unwind_retire_the_send_phase() {
    for panic in [false, true] {
        let mut live = Live::new(false); let store = live.file.rig.root.store(); let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
                &live.witness, None, || {
                    calls += 1;
                    assert!(!(panic && calls == 5), "clock interrupted before final native source read");
                    if !panic && calls == 6 { std::fs::write(store.join("delivery.pending"), b"blocked").unwrap(); }
                    ElapsedTick(2)
                }, &live.human, None)
        }));
        if panic { assert!(result.is_err()); } else {
            let report = result.unwrap();
            assert!(matches!(report.completion.completion.completion.result, Err(JournalError::Io(_))));
            assert_eq!(report.authorization_source_updates.len(), 1);
            assert!(report.completion.committed_source_updates.is_empty());
        }
        assert_eq!(live.file.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
        let host = live.file.rig.driver.supervisor().host().unwrap();
        assert!(host.storage_failure().is_some()); assert!(host.file_source_status().unwrap().interrupted);
        assert_eq!(host.inspect().control.ledger.reserved, 16); assert_eq!(host.inspect().executions, 0);
        assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap(), host.inspect());
        let retry = live.complete(2);
        assert!(retry.authorization_observations.is_empty()); assert!(retry.completion.observations.is_empty());
        assert_eq!(retry.completion.completion.completion.result, Err(JournalError::Unavailable));
    }
}

#[test]
fn foreign_human_cannot_trigger_any_reader_and_the_original_human_can_complete() {
    let mut live = Live::new(false); let other = Live::new(false);
    let before = live.file.rig.driver.supervisor().host().unwrap().inspect();
    let reads = live.file.source.status().read_attempts;
    let report = live.file.rig.driver.complete_from_files_with_publication(&mut live.file.source,
        &live.witness, None, || panic!("foreign key cannot run a capture clock"), &other.human, None);
    assert_eq!(report.completion.completion.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert!(report.authorization_observations.is_empty()); assert_eq!(live.file.source.status().read_attempts, reads);
    assert_eq!(live.file.rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(outcome(live.complete(2)).basis, PublicationBasis::Revalidated);
}
