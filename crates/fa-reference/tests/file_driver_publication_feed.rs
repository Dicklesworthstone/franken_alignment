//! Real helper-socket jobs catch up without manual notification delivery.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod files;
#[path = "support/file_publication_capture.rs"] mod capture;
use files::driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::feed::FileFeedDriverReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::{CheckedPublication, PublicationBasis};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{PublicationInputFile, FileCaptureError};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::action::consequence::oversight::{CommitteeInput, supervised::DriverEvidence};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::witness::DomainProjection;
use fa_reference::product_frontier::ProjectionKey;
use fa_reference::Error;
use std::cell::Cell;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::panic::{AssertUnwindSafe, catch_unwind};

const FEED: u64 = 41;
fn batch(generation: u64, after: u64, keys: &[u64], tick: u64) -> PublicationFeedBatch {
    let domain = DomainProjection::new(40, 1, ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 });
    PublicationFeedBatch::new(PublicationHeartbeat { source: FEED,
        clock_domain: profile().delivery.clock_domain, generation,
        through: after + keys.len() as u64, produced_at: ElapsedTick(tick) }, after,
        keys.iter().enumerate().map(|(i, key)| PublicationChange { source: FEED,
            sequence: after + i as u64 + 1, change: WitnessChange::Key { domain, key: *key } }).collect()).unwrap()
}
fn write(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, bytes).unwrap(); std::fs::rename(pending, path).unwrap();
}
fn enable(rig: &mut Rig) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); host.enable_publication_validation(revision, capture::limits()).unwrap();
    let revision = host.revision(); host.enable_publication_changes(revision, PublicationChangePolicy {
        source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
    }).unwrap();
    let revision = host.revision(); host.enable_publication_change_freshness(revision, PublicationFreshnessPolicy {
        clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
    }).unwrap();
}
fn bind(rig: &mut Rig, inputs: &CommitteeInput) -> (PublicationInputFile, PathBuf) {
    let path = rig.root.0.join("witness.bin");
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let original = capture::packet(1, host.request_action(1).unwrap(), inputs, 1, &[0, 2, 4]);
    write(&path, &original.to_bytes().unwrap());
    let revision = host.revision(); host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
    (PublicationInputFile::new(&path, capture::SOURCE).unwrap(), path)
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn publication(report: FileFeedDriverReport<FileDriverEvent>) -> CheckedPublication {
    match report.publication.evidence.result.unwrap() {
        FileDriverEvent::PublicationChecked { publication, source_failure: None, .. } => publication,
        other => panic!("expected original checked publication: {other:?}"),
    }
}
struct Live {
    rig: Rig, witness: PublicationInputFile, witness_path: PathBuf,
    feed: PublicationFeedFile, feed_path: PathBuf, human: FileHumanPermit,
}
impl Live {
    fn new() -> Self {
        let mut rig = Rig::new(); enable(&mut rig);
        let feed_path = rig.root.0.join("feed.bin"); write(&feed_path, &batch(1, 0, &[99], 1).to_bytes().unwrap());
        let feed = PublicationFeedFile::new(&feed_path, FEED).unwrap();
        let _ticket = rig.submit(1); rig.reviewed(1);
        let inputs = rig.inputs.clone().unwrap(); let (witness, witness_path) = bind(&mut rig, &inputs);
        let human = rig.human(1001, 31);
        Self { rig, witness, witness_path, feed, feed_path, human }
    }
    fn step(&mut self, now: u64, human: bool) -> FileFeedDriverReport<FileDriverEvent> {
        let inputs = self.rig.inputs.clone();
        self.rig.driver.step_with_publication_feed(&self.witness, &self.feed, || ElapsedTick(now),
            |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), human.then_some(&self.human), None)
    }
    fn sent() -> Self {
        let mut live = Self::new();
        let report = live.step(1, true);
        assert_eq!(report.feeds.len(), 2); assert_eq!(report.publication.reads.len(), 2);
        assert_eq!(report.feeds[0].as_ref().unwrap().changes.len(), 1);
        assert_eq!(report.feeds[0].as_ref().unwrap().changes[0].affected, vec![1]);
        assert!(report.feeds[1].as_ref().unwrap().changes.is_empty());
        assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
        live
    }
}

#[test]
fn catchup_revises_the_active_slot_before_capture_and_publishes_through_original_keys() {
    let mut live = Live::sent();
    assert_eq!(live.rig.driver.supervisor().host().unwrap().publication_change_status().unwrap().through, 1);
    write(&live.feed_path, &batch(2, 0, &[99, 100], 2).to_bytes().unwrap());
    let report = live.step(2, false);
    assert_eq!(report.feeds[0].as_ref().unwrap().changes.len(), 1);
    assert_eq!(publication(report).outcome, EndpointOutcome::Executed { resulting_version: 2 });
    std::fs::remove_file(&live.feed_path).unwrap(); std::fs::remove_file(&live.witness_path).unwrap();
    let settled = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed,
        || ElapsedTick(3), |_, _| panic!("settlement does not acquire sources"), None, None);
    assert!(settled.feeds.is_empty()); assert!(settled.publication.reads.is_empty());
    assert!(matches!(settled.publication.evidence.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(live.rig.root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn post_dispatch_change_and_actual_phantom_cannot_rebase_the_reviewed_judgment() {
    let mut live = Live::sent();
    write(&live.feed_path, &batch(2, 1, &[1], 1).to_bytes().unwrap());
    let action = live.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    let changed = capture::packet(1, &action, live.rig.inputs.as_ref().unwrap(), 2, &[0, 1, 2, 4]);
    write(&live.witness_path, &changed.to_bytes().unwrap());
    let report = live.step(1, false);
    assert_eq!(report.feeds[0].as_ref().unwrap().changes[0].affected, vec![1]);
    let result = publication(report);
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    live.step(1, false);
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn a_lost_second_feed_read_keeps_the_same_automatic_permit_for_real_retry() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    let report = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(1), |_, _| {
        calls += 1;
        if calls == 1 { std::fs::remove_file(&live.feed_path).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, Some(&live.human), None);
    assert_eq!(calls, 2); assert_eq!(report.feeds[1], Err(FileCaptureError::Io(ErrorKind::NotFound)));
    assert!(matches!(report.publication.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    write(&live.feed_path, &batch(2, 0, &[99], 2).to_bytes().unwrap());
    let retry = live.step(2, true);
    assert_eq!(retry.feeds.len(), 1); // retained permit, no second authorization
    assert!(matches!(retry.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    let ledger = live.rig.driver.supervisor().host().unwrap().inspect().control.ledger;
    assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 0, 16));
}

#[test]
fn a_window_gap_repairs_before_capture_instead_of_reusing_an_interim_revision() {
    let mut live = Live::new();
    write(&live.feed_path, &batch(1, 1, &[100], 1).to_bytes().unwrap());
    let refused = live.step(1, true);
    assert_eq!(refused.feeds[0].as_ref().unwrap().freshness.eligibility, Err(Error::Incomplete));
    assert!(matches!(refused.publication.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    write(&live.feed_path, &batch(1, 0, &[99, 100], 1).to_bytes().unwrap());
    let repaired = live.step(1, true);
    assert_eq!(repaired.feeds[0].as_ref().unwrap().changes.len(), 2);
    assert!(matches!(repaired.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn catchup_success_is_not_permission_after_expiry_during_committee_capture() {
    let mut live = Live::sent(); let now = Cell::new(3); let inputs = live.rig.inputs.clone();
    let report = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed,
        || ElapsedTick(now.get()), |_, _| {
            now.set(4); Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, None, None);
    assert_eq!(report.feeds[0].as_ref().unwrap().freshness.eligibility, Ok(()));
    let result = publication(report);
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn committee_unwind_after_feed_installation_leaves_witness_withdrawn_and_send_retired() {
    let mut live = Live::sent();
    let failed = catch_unwind(AssertUnwindSafe(|| {
        live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(1),
            |_, _| panic!("committee acquisition failed"), None, None)
    }));
    assert!(failed.is_err()); assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let mut host = live.rig.driver.supervisor_mut().host_mut().unwrap();
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    let revision = host.revision();
    let result = host.publish_checked(revision, 1, live.rig.inputs.as_ref(), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, sealed()); assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn resolved_or_expired_publications_skip_feed_and_witness_transport() {
    for executed in [false, true] {
        let mut live = Live::sent();
        if executed {
            let mut host = live.rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.refresh_publication_from_file(revision, 1, &live.witness).unwrap().unwrap();
            let revision = host.revision(); host.publish_checked(revision, 1, live.rig.inputs.as_ref(), snapshot(), ElapsedTick(1)).unwrap();
        }
        std::fs::remove_file(&live.feed_path).unwrap(); std::fs::remove_file(&live.witness_path).unwrap();
        let report = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(31),
            |_, _| panic!("terminal evidence has precedence"), None, None);
        assert!(report.feeds.is_empty()); assert!(report.publication.reads.is_empty());
        let result = publication(report);
        assert_eq!(result.basis, if executed { PublicationBasis::PreviouslyResolved } else { PublicationBasis::DeadlineElapsed });
        assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, u64::from(executed));
    }
}

#[test]
fn three_readers_catch_up_and_preserve_independent_native_policy_leases() {
    let mut rig = Rig::new(); enable(&mut rig);
    let feed_path = rig.root.0.join("feed.bin"); write(&feed_path, &batch(1, 0, &[99], 1).to_bytes().unwrap());
    let feed = PublicationFeedFile::new(&feed_path, FEED).unwrap();
    let path = rig.root.0.join("policy.json"); files::replace(&path, &files::document(1));
    let mut source = FileEvidenceSource::new(&path, files::SOURCE, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        host.enable_file_source(revision, FileSourcePolicy { source: StateSource {
            scope: profile().delivery.scope, source: files::SOURCE, generation: 1 },
            limits: StateLimits::default(), freshness: StateFreshness::new(3).unwrap(),
        }).unwrap();
        let revision = host.revision(); host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let proposal = rig.proposal(); let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(files::document(1).snapshot().clone())).unwrap();
    let _ticket = rig.port.submit(1, &proposal).unwrap();
    let mut file = files::FileRig { rig, source, path }; files::reviewed(&mut file);
    let inputs = {
        let host = file.rig.driver.supervisor().host().unwrap();
        files::document(1).inputs_for(host.request_action(1).unwrap(), &profile().committee).unwrap()
    };
    let (witness, _) = bind(&mut file.rig, &inputs); let human = files::human(&mut file, 1);
    let baseline = files::capture_count(&file);
    let stale = file.rig.driver.step_from_files_with_publication_feed(&mut file.source, &witness, &feed,
        || ElapsedTick(4), Some(&human), None);
    assert_eq!(stale.publication.evidence.source_updates, vec![Ok(files::document(1).identity())]);
    assert!(matches!(stale.publication.evidence.result, Err(JournalError::Contract(Error::Stale))));
    write(&feed_path, &batch(2, 0, &[99, 100], 4).to_bytes().unwrap());
    let dispatch = file.rig.driver.step_from_files_with_publication_feed(&mut file.source, &witness, &feed,
        || ElapsedTick(4), Some(&human), None);
    assert_eq!(dispatch.publication.evidence.source_updates, vec![Ok(files::document(1).identity()); 2]);
    assert!(matches!(dispatch.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(files::capture_count(&file), baseline + 3);
    let published = file.rig.driver.step_from_files_with_publication_feed(&mut file.source, &witness, &feed,
        || ElapsedTick(5), None, None);
    assert_eq!(publication(published).basis, PublicationBasis::Revalidated);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages[&1], ActionState::Dispatching);
}
