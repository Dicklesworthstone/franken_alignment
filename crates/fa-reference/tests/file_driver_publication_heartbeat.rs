//! Concrete feed heartbeats through the original helper-socket supervised driver.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod files;
#[path = "support/file_publication_capture.rs"] mod capture;
use files::driver::{Rig, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::heartbeat::FileHeartbeatDriverReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::{CheckedPublication, PublicationBasis};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::PublicationHeartbeatFile;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;
use std::cell::Cell;
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

const FEED: u64 = 41;
fn pulse(generation: u64, produced: u64) -> PublicationHeartbeat {
    PublicationHeartbeat { source: FEED, clock_domain: profile().delivery.clock_domain,
        generation, through: 0, produced_at: ElapsedTick(produced) }
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
    let revision = host.revision();
    host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
    (PublicationInputFile::new(&path, capture::SOURCE).unwrap(), path)
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn publication(report: FileHeartbeatDriverReport<FileDriverEvent>) -> CheckedPublication {
    match report.publication.evidence.result.unwrap() {
        FileDriverEvent::PublicationChecked { publication, source_failure: None, .. } => publication,
        other => panic!("expected publication with unchanged committee evidence: {other:?}"),
    }
}
struct Live {
    rig: Rig,
    witness: PublicationInputFile,
    witness_path: PathBuf,
    heartbeat: PublicationHeartbeatFile,
    heartbeat_path: PathBuf,
    human: FileHumanPermit,
}
impl Live {
    fn new() -> Self {
        let mut rig = Rig::new(); enable(&mut rig);
        let heartbeat_path = rig.root.0.join("heartbeat.bin");
        write(&heartbeat_path, &pulse(1, 1).to_bytes().unwrap());
        let heartbeat = PublicationHeartbeatFile::new(&heartbeat_path, FEED).unwrap();
        let _ticket = rig.submit(1); rig.reviewed(1);
        let inputs = rig.inputs.clone().unwrap(); let (witness, witness_path) = bind(&mut rig, &inputs);
        let human = rig.human(1001, 31);
        Self { rig, witness, witness_path, heartbeat, heartbeat_path, human }
    }
    fn step(&mut self, now: u64, use_human: bool) -> FileHeartbeatDriverReport<FileDriverEvent> {
        let inputs = self.rig.inputs.clone();
        self.rig.driver.step_with_publication_heartbeat(&self.witness, &self.heartbeat, || ElapsedTick(now),
            |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }),
            use_human.then_some(&self.human), None)
    }
    fn sent() -> Self {
        let mut live = Self::new();
        let report = live.step(1, true);
        assert_eq!(report.heartbeats.len(), 2);
        assert!(report.heartbeats.iter().all(|entry| entry.as_ref().unwrap().eligibility.is_ok()));
        assert_eq!(report.publication.reads.len(), 2);
        assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
        live
    }
}

#[test]
fn native_steps_acquire_at_authorization_dispatch_and_publication_but_not_settlement() {
    let mut live = Live::sent();
    let report = live.step(3, false);
    assert_eq!(report.heartbeats.len(), 1); assert_eq!(report.publication.reads.len(), 1);
    let result = publication(report);
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    {
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(FileOversight::read_publication(live.rig.root.store(), &profile()).unwrap(), host.inspect());
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    }
    std::fs::remove_file(&live.heartbeat_path).unwrap(); std::fs::remove_file(&live.witness_path).unwrap();
    let report = live.rig.driver.step_with_publication_heartbeat(&live.witness, &live.heartbeat,
        || ElapsedTick(4), |_, _| panic!("settlement cannot require provider availability"), None, None);
    assert!(report.heartbeats.is_empty()); assert!(report.publication.reads.is_empty());
    assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. })));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn stalled_heartbeat_refuses_without_inventing_committee_drift_or_reserving_twice() {
    let mut live = Live::new();
    let input_revision = live.rig.driver.supervisor().host().unwrap().input_revision(1).unwrap();
    let report = live.step(4, true);
    assert_eq!(report.heartbeats.len(), 1);
    assert_eq!(report.heartbeats[0].as_ref().unwrap().eligibility, Err(Error::Stale));
    assert!(matches!(report.publication.evidence.result, Err(JournalError::Contract(Error::Stale))));
    {
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.input_revision(1).unwrap(), input_revision);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    }
    write(&live.heartbeat_path, &pulse(2, 4).to_bytes().unwrap());
    let report = live.step(4, true);
    assert_eq!(report.heartbeats.len(), 2);
    assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn heartbeat_loss_between_reservation_and_dispatch_preserves_the_same_permit_for_fresh_retry() {
    let mut live = Live::new(); let inputs = live.rig.inputs.clone();
    let path = live.heartbeat_path.clone(); let mut calls = 0;
    let input_revision = live.rig.driver.supervisor().host().unwrap().input_revision(1).unwrap();
    let report = live.rig.driver.step_with_publication_heartbeat(&live.witness, &live.heartbeat,
        || ElapsedTick(1), |_, _| {
            calls += 1;
            // First heartbeat was acquired before this callback; its observation
            // can support authorization, but a second acquisition must fail.
            if calls == 1 { std::fs::remove_file(&path).unwrap(); }
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, Some(&live.human), None);
    assert_eq!(calls, 2); assert_eq!(report.heartbeats.len(), 2);
    assert_eq!(report.heartbeats[1], Err(FileCaptureError::Io(ErrorKind::NotFound)));
    assert!(matches!(report.publication.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    {
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.reserved, 16); assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.input_revision(1).unwrap(), input_revision);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    }
    write(&live.heartbeat_path, &pulse(2, 2).to_bytes().unwrap());
    let report = live.step(2, true);
    assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    let ledger = live.rig.driver.supervisor().host().unwrap().inspect().control.ledger;
    assert_eq!(ledger.available, 84); assert_eq!(ledger.reserved, 0); assert_eq!(ledger.charged, 16);
}

#[test]
fn heartbeat_can_expire_during_committee_capture_and_final_publication_rechecks_time() {
    let mut live = Live::sent(); let inputs = live.rig.inputs.clone(); let now = Cell::new(3);
    let report = live.rig.driver.step_with_publication_heartbeat(&live.witness, &live.heartbeat,
        || ElapsedTick(now.get()), |_, _| {
            now.set(4);
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, None, None);
    // The read-time status was valid. It is NOT a permit for the later cut.
    assert_eq!(report.heartbeats[0].as_ref().unwrap().eligibility, Ok(()));
    let result = publication(report);
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn lost_heartbeat_after_dispatch_seals_without_falsifying_the_committee_capture() {
    let mut live = Live::sent(); std::fs::remove_file(&live.heartbeat_path).unwrap();
    let report = live.step(2, false);
    assert_eq!(report.heartbeats, vec![Err(FileCaptureError::Io(ErrorKind::NotFound))]);
    assert_eq!(report.publication.reads.len(), 1);
    let result = publication(report);
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete)); assert_eq!(result.outcome, sealed());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    let report = live.step(2, false);
    assert!(report.heartbeats.is_empty()); assert!(report.publication.reads.is_empty());
    assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(outcome), .. }) if outcome == sealed()));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn post_read_clock_unwind_quarantines_the_owner_and_retires_the_drivers_send_phase() {
    let mut live = Live::sent(); let mut clocks = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        live.rig.driver.step_with_publication_heartbeat(&live.witness, &live.heartbeat, || {
            clocks += 1;
            if clocks == 2 { panic!("post-heartbeat-read clock failure"); }
            ElapsedTick(1)
        }, |_, _| panic!("committee must not run after failed clock"), None, None)
    }));
    assert!(result.is_err()); assert_eq!(clocks, 2);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let host = live.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(live.rig.root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn_original_receipts_and_execution_deadlines_skip_all_new_readers() {
    for executed in [false, true] {
        let mut live = Live::sent();
        if executed {
            let mut host = live.rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.refresh_publication_from_file(revision, 1, &live.witness).unwrap().unwrap();
            let revision = host.revision(); host.publish_checked(revision, 1, live.rig.inputs.as_ref(), snapshot(), ElapsedTick(1)).unwrap();
        }
        std::fs::remove_file(&live.heartbeat_path).unwrap(); std::fs::remove_file(&live.witness_path).unwrap();
        let report = live.rig.driver.step_with_publication_heartbeat(&live.witness, &live.heartbeat,
            || ElapsedTick(31), |_, _| panic!("original outcome/expiry needs no new source"), None, None);
        assert!(report.heartbeats.is_empty()); assert!(report.publication.reads.is_empty());
        let result = publication(report);
        assert_eq!(result.basis, if executed { PublicationBasis::PreviouslyResolved } else { PublicationBasis::DeadlineElapsed });
        assert_eq!(result.outcome, if executed { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed } });
    }
}

#[test]
fn_three_concrete_readers_preserve_the_independent_native_policy_source_lease() {
    let mut rig = Rig::new(); enable(&mut rig);
    let heartbeat_path = rig.root.0.join("heartbeat.bin"); write(&heartbeat_path, &pulse(1, 1).to_bytes().unwrap());
    let heartbeat = PublicationHeartbeatFile::new(&heartbeat_path, FEED).unwrap();
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
    let inputs = {
        let host = file.rig.driver.supervisor().host().unwrap();
        files::document(1).inputs_for(host.request_action(1).unwrap(), &profile().committee).unwrap()
    };
    let (witness, _) = bind(&mut file.rig, &inputs); let human = files::human(&mut file, 1);
    let baseline = files::capture_count(&file);
    let stale = file.rig.driver.step_from_files_with_publication_heartbeat(&mut file.source,
        &witness, &heartbeat, || ElapsedTick(4), Some(&human), None);
    assert_eq!(stale.publication.evidence.source_updates, vec![Ok(files::document(1).identity())]);
    assert!(matches!(stale.publication.evidence.result, Err(JournalError::Contract(Error::Stale))));
    assert_eq!(files::capture_count(&file), baseline + 1);
    write(&heartbeat_path, &pulse(2, 4).to_bytes().unwrap());
    let dispatch = file.rig.driver.step_from_files_with_publication_heartbeat(&mut file.source,
        &witness, &heartbeat, || ElapsedTick(4), Some(&human), None);
    assert_eq!(dispatch.heartbeats.len(), 2);
    assert_eq!(dispatch.publication.evidence.source_updates, vec![Ok(files::document(1).identity()); 2]);
    assert!(matches!(dispatch.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    let published = file.rig.driver.step_from_files_with_publication_heartbeat(&mut file.source,
        &witness, &heartbeat, || ElapsedTick(5), None, None);
    assert_eq!(published.publication.evidence.source_updates, vec![Ok(files::document(1).identity())]);
    assert_eq!(publication(published).basis, PublicationBasis::Revalidated);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
