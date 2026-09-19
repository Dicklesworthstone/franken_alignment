//! Real helper sockets and the original supervised phases consume actual files.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod files;
#[path = "support/file_publication_capture.rs"] mod capture;
use files::driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::FilePublicationDriverReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::{CheckedPublication, PublicationBasis};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureError, FileCaptureIdentity, FilePublicationCapture, PublicationInputFile,
};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

fn identity(generation: u64) -> FileCaptureIdentity { FileCaptureIdentity { source: capture::SOURCE, generation } }
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn write(path: &Path, packet: &FilePublicationCapture) {
    let pending = path.with_extension("pending");
    std::fs::write(&pending, packet.to_bytes().unwrap()).unwrap();
    std::fs::rename(pending, path).unwrap();
}
fn enable(rig: &mut Rig) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    host.enable_publication_validation(revision, capture::limits()).unwrap();
}
fn bind(rig: &mut Rig, inputs: &CommitteeInput) -> (PublicationInputFile, PathBuf) {
    let path = rig.root.0.join("publication-input.bin");
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let original = capture::packet(1, host.request_action(1).unwrap(), inputs, 1, &[0, 2, 4]);
    write(&path, &original);
    let revision = host.revision();
    host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
    (PublicationInputFile::new(&path, capture::SOURCE).unwrap(), path)
}
fn ready() -> (Rig, PublicationInputFile, PathBuf, FileHumanPermit) {
    let mut rig = Rig::new();
    enable(&mut rig);
    let _ticket = rig.submit(1);
    rig.reviewed(1);
    let inputs = rig.inputs.clone().unwrap();
    let (source, path) = bind(&mut rig, &inputs);
    let key = rig.human(1001, 31);
    (rig, source, path, key)
}
fn step(rig: &mut Rig, source: &PublicationInputFile, human: Option<&FileHumanPermit>)
    -> FilePublicationDriverReport<FileDriverEvent>
{
    let inputs = rig.inputs.clone();
    rig.driver.step_with_publication_source(source, || ElapsedTick(1),
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), human, None)
}
fn sent() -> (Rig, PublicationInputFile, PathBuf) {
    let (mut rig, source, path, human) = ready();
    let report = step(&mut rig, &source, Some(&human));
    assert_eq!(report.reads, vec![Ok(identity(1)); 2]);
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Dispatched { request: 1, attempt: 1 })));
    (rig, source, path)
}
fn publication(report: FilePublicationDriverReport<FileDriverEvent>) -> (CheckedPublication, Option<Error>) {
    match report.evidence.result.unwrap() {
        FileDriverEvent::PublicationChecked { publication, source_failure, .. } => (publication, source_failure),
        other => panic!("expected original publication result, got {other:?}"),
    }
}
fn refresh(rig: &mut Rig, source: &PublicationInputFile) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    host.refresh_publication_from_file(revision, 1, source).unwrap().unwrap();
}

#[test]
fn normal_supervised_dispatch_and_publication_perform_three_independent_acquisitions() {
    let (mut rig, source, path) = sent();
    let state = rig.driver.supervisor().host().unwrap().publication_source(1).unwrap().unwrap();
    assert!(!state.fresh);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 1 });
    let report = step(&mut rig, &source, None);
    assert_eq!(report.reads, vec![Ok(identity(1))]);
    assert!(report.evidence.observations.is_empty());
    assert!(report.evidence.source_updates.is_empty());
    let (result, failure) = publication(report);
    assert_eq!(failure, None);
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert!(!host.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap(), host.inspect());
    }
    std::fs::rename(&path, path.with_extension("offline")).unwrap();
    let report = rig.driver.step_with_publication_source(&source, || ElapsedTick(2),
        |_, _| panic!("reconciliation never calls a provider"), None, None);
    assert!(report.reads.is_empty());
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. })));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn publication_rereads_changed_files_and_distinguishes_unrelated_updates_from_phantoms() {
    for inserted in [99, 1, 3, 7] {
        let (mut rig, source, path) = sent();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let next = capture::packet(1, &action, rig.inputs.as_ref().unwrap(), 2, &[0, 2, 4, inserted]);
        write(&path, &next);
        let report = step(&mut rig, &source, None);
        assert_eq!(report.reads, vec![Ok(identity(2))]);
        let (result, failure) = publication(report);
        assert_eq!(failure, None);
        if inserted == 99 {
            assert_eq!(result.basis, PublicationBasis::Revalidated);
            assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        } else {
            assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
            assert_eq!(result.outcome, sealed());
        }
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(inserted == 99));
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn second_read_failure_retains_the_original_reservation_until_explicit_cancellation() {
    let (mut rig, source, path, human) = ready();
    let inputs = rig.inputs.clone();
    let mut calls = 0;
    let report = rig.driver.step_with_publication_source(&source, || ElapsedTick(1), |_, _| {
        calls += 1;
        if calls == 2 { std::fs::rename(&path, path.with_extension("offline")).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, Some(&human), None);
    assert_eq!(calls, 2);
    assert_eq!(report.reads, vec![Ok(identity(1)), Err(FileCaptureError::Io(ErrorKind::NotFound))]);
    assert!(matches!(report.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Authorized);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert!(!host.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(host.inspect().executions, 0);
    }
    rig.driver.cancel_active().unwrap();
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn lost_file_after_dispatch_seals_but_only_the_next_reconciliation_refunds() {
    let (mut rig, source, path) = sent();
    std::fs::rename(&path, path.with_extension("offline")).unwrap();
    let report = step(&mut rig, &source, None);
    assert_eq!(report.reads, vec![Err(FileCaptureError::Io(ErrorKind::NotFound))]);
    let (result, failure) = publication(report);
    assert_eq!(failure, Some(Error::Incomplete));
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete));
    assert_eq!(result.outcome, sealed());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    let report = rig.driver.step_with_publication_source(&source, || ElapsedTick(2),
        |_, _| panic!("no reacquisition during settlement"), None, None);
    assert!(report.reads.is_empty());
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(outcome), .. }) if outcome == sealed()));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn caught_committee_provider_unwind_cannot_leave_an_earlier_fresh_capture_eligible() {
    for after_dispatch in [false, true] {
        let (mut rig, source, _path, human) = ready();
        if after_dispatch {
            assert!(matches!(step(&mut rig, &source, Some(&human)).evidence.result,
                Ok(FileDriverEvent::Dispatched { .. })));
        }
        refresh(&mut rig, &source);
        let old_revision = {
            let host = rig.driver.supervisor().host().unwrap();
            assert!(host.publication_source(1).unwrap().unwrap().fresh);
            host.publication_input_revision(1).unwrap()
        };
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _ = rig.driver.step_with_publication_source(&source, || ElapsedTick(1),
                |_, _| panic!("committee capture failed before witness read"), Some(&human), None);
        }));
        assert!(unwound.is_err());
        let inputs = rig.inputs.clone().unwrap();
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(host.publication_input_revision(1).unwrap(), old_revision + 1);
        assert!(!host.publication_source(1).unwrap().unwrap().fresh);
        assert!(host.storage_failure().is_none());
        let revision = host.revision();
        if after_dispatch {
            let result = host.publish_checked(revision, 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete));
            assert_eq!(result.outcome, sealed());
            assert_eq!(host.inspect().control.ledger.charged, 16);
        } else {
            assert!(matches!(host.authorize(revision, 1, &inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
            assert_eq!(host.inspect().control.ledger.reserved, 0);
        }
        assert_eq!(host.inspect().executions, 0);
        drop(host);
        if after_dispatch { assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 }); }
    }
}

#[test]
fn terminal_receipts_and_expiry_skip_both_providers_even_when_the_file_is_gone() {
    for terminal in 0..3 {
        let (mut rig, source, path) = sent();
        if terminal != 2 {
            refresh(&mut rig, &source);
            let inputs = rig.inputs.clone();
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.publish_checked(revision, 1, if terminal == 0 { inputs.as_ref() } else { None },
                snapshot(), ElapsedTick(1)).unwrap();
        }
        std::fs::rename(&path, path.with_extension("offline")).unwrap();
        let report = rig.driver.step_with_publication_source(&source,
            || ElapsedTick(if terminal == 2 { 31 } else { 2 }),
            |_, _| panic!("terminal or expired request must not acquire"), None, None);
        assert!(report.reads.is_empty());
        let (result, failure) = publication(report);
        assert_eq!(failure, None);
        assert_eq!(result.basis, if terminal == 2 { PublicationBasis::DeadlineElapsed } else { PublicationBasis::PreviouslyResolved });
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(terminal == 0));
        assert_eq!(host.inspect().control.ledger.charged, 16);
    }
}

#[test]
fn successful_file_read_is_not_reported_as_successful_installation_on_equivocation() {
    let (mut rig, source, path) = sent();
    let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    write(&path, &capture::packet(1, &action, rig.inputs.as_ref().unwrap(), 1, &[0, 2, 4, 99]));
    let report = step(&mut rig, &source, None);
    assert_eq!(report.reads, vec![Ok(identity(1))]);
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::PublicationUnknown {
        error: JournalError::Contract(Error::Binding), .. })));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let host = rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap().executions, 0);
}

#[test]
fn paired_file_path_renews_native_policy_leases_without_weakening_publication_requirements() {
    let mut rig = Rig::new();
    enable(&mut rig);
    let path = rig.root.0.join("evidence.json");
    files::replace(&path, &files::document(1));
    let mut source = FileEvidenceSource::new(&path, files::SOURCE, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_file_source(revision, FileSourcePolicy {
            source: StateSource { scope: profile().delivery.scope, source: files::SOURCE, generation: 1 },
            limits: StateLimits::default(), freshness: StateFreshness::new(3).unwrap(),
        }).unwrap();
        let revision = host.revision();
        host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let proposal = rig.proposal();
    let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(files::document(1).snapshot().clone())).unwrap();
    let _ticket = rig.port.submit(1, &proposal).unwrap();
    let mut file = files::FileRig { rig, source, path };
    files::reviewed(&mut file);
    let inputs = {
        let host = file.rig.driver.supervisor().host().unwrap();
        files::document(1).inputs_for(host.request_action(1).unwrap(), &profile().committee).unwrap()
    };
    let (witness, witness_path) = bind(&mut file.rig, &inputs);
    let human = files::human(&mut file, 4);
    let baseline = files::capture_count(&file);
    let report = file.rig.driver.step_from_files_with_publication_source(&mut file.source,
        &witness, || ElapsedTick(6), Some(&human), None);
    assert_eq!(report.reads, vec![Ok(identity(1)); 2]);
    assert_eq!(report.evidence.observations, vec![Ok(files::document(1).identity()); 2]);
    assert_eq!(report.evidence.source_updates, vec![Ok(files::document(1).identity()); 2]);
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(files::capture_count(&file), baseline + 2);
    let report = file.rig.driver.step_from_files_with_publication_source(&mut file.source,
        &witness, || ElapsedTick(8), None, None);
    assert_eq!(report.reads, vec![Ok(identity(1))]);
    assert_eq!(report.evidence.source_updates, vec![Ok(files::document(1).identity())]);
    let (result, failure) = publication(report);
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(failure, None);
    assert_eq!(files::capture_count(&file), baseline + 3);
    std::fs::rename(&file.path, file.path.with_extension("offline")).unwrap();
    std::fs::rename(&witness_path, witness_path.with_extension("offline")).unwrap();
    let reads = file.source.status().read_attempts;
    let report = file.rig.driver.step_from_files_with_publication_source(&mut file.source,
        &witness, || ElapsedTick(12), None, None);
    assert!(report.reads.is_empty());
    assert!(report.evidence.observations.is_empty());
    assert!(report.evidence.source_updates.is_empty());
    assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    assert_eq!(file.source.status().read_attempts, reads);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}
