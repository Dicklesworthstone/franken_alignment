//! Real socket review feeds source-acquired, atomically settled driver completion.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/file_publication_capture.rs"] mod capture;
use driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureIdentity, FilePublicationCapture, PublicationInputFile,
};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

fn identity(generation: u64) -> FileCaptureIdentity { FileCaptureIdentity { source: capture::SOURCE, generation } }
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn write(path: &Path, packet: &FilePublicationCapture) {
    let pending = path.with_extension("pending");
    std::fs::write(&pending, packet.to_bytes().unwrap()).unwrap();
    std::fs::rename(pending, path).unwrap();
}
fn ready() -> (Rig, PublicationInputFile, PathBuf, FileHumanPermit) {
    let mut rig = Rig::new();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_publication_validation(revision, capture::limits()).unwrap();
    }
    let _ticket = rig.submit(1);
    rig.reviewed(1);
    let path = rig.root.0.join("source-completion.bin");
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let original = capture::packet(1, host.request_action(1).unwrap(), rig.inputs.as_ref().unwrap(), 1, &[0, 2, 4]);
        write(&path, &original);
        let revision = host.revision();
        host.bind_publication_file_source(revision, 1, original, capture::requests()).unwrap();
    }
    let human = rig.human(1001, 31);
    let source = PublicationInputFile::new(&path, capture::SOURCE).unwrap();
    (rig, source, path, human)
}

#[test]
fn one_call_acquires_three_times_and_finishes_the_original_actor_request() {
    let (mut rig, source, path, human) = ready();
    let inputs = rig.inputs.clone();
    let store = rig.root.store();
    let mut calls = 0;
    let report = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
        calls += 1;
        let visible = FileOversight::read_publication(&store, &profile()).unwrap();
        assert_eq!(visible.executions, 0);
        assert_eq!(visible.control.ledger.charged, 0);
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &human, None);
    assert_eq!(calls, 3);
    assert_eq!(report.authorization_reads, vec![Ok(identity(1))]);
    assert_eq!(report.completion.reads, vec![Ok(identity(1)); 2]);
    assert_eq!(report.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.request_status(1).unwrap().disposition,
            FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Confirmed });
        assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap(), host.inspect());
    }
    std::fs::remove_file(path).unwrap();
    let idle = rig.driver.step_with_publication_source(&source, || ElapsedTick(2),
        |_, _| panic!("completed jobs need no additional evidence or settlement"), None, None);
    assert!(idle.reads.is_empty());
    assert!(matches!(idle.evidence.result, Ok(FileDriverEvent::Idle)));
}

#[test]
fn final_source_change_distinguishes_unrelated_updates_from_new_negative_dependencies() {
    for inserted in [99, 1, 3, 7] {
        let (mut rig, source, path, human) = ready();
        let inputs = rig.inputs.clone();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let next = capture::packet(1, &action, inputs.as_ref().unwrap(), 2, &[0, 2, 4, inserted]);
        let mut calls = 0;
        let report = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 3 { write(&path, &next); }
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, &human, None);
        assert_eq!(calls, 3);
        assert_eq!(report.completion.reads, vec![Ok(identity(1)), Ok(identity(2))]);
        let publication = report.completion.result.unwrap();
        let permitted = inserted == 99;
        assert_eq!(publication.basis, if permitted { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(publication.outcome, if permitted { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(permitted));
        assert_eq!(host.inspect().control.ledger.charged, if permitted { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn failed_pre_dispatch_read_retries_the_same_reservation_without_authorizing_again() {
    let (mut rig, source, path, human) = ready();
    let inputs = rig.inputs.clone();
    let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    let original = capture::packet(1, &action, inputs.as_ref().unwrap(), 1, &[0, 2, 4]);
    let mut calls = 0;
    let failed = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
        calls += 1;
        if calls == 2 { std::fs::remove_file(&path).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &human, None);
    assert_eq!(calls, 2);
    assert_eq!(failed.authorization_reads, vec![Ok(identity(1))]);
    assert_eq!(failed.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    }
    write(&path, &original);
    let mut calls = 0;
    let retried = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
        calls += 1;
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &human, None);
    assert_eq!(calls, 2);
    assert!(retried.authorization_reads.is_empty());
    assert_eq!(retried.completion.reads.len(), 2);
    assert_eq!(retried.completion.result.unwrap().basis, PublicationBasis::Revalidated);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn loss_after_staged_dispatch_closes_the_driver_with_native_nonexecution_not_an_unknown_charge() {
    let (mut rig, source, path, human) = ready();
    let inputs = rig.inputs.clone();
    let mut calls = 0;
    let report = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
        calls += 1;
        if calls == 3 { std::fs::remove_file(&path).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &human, None);
    assert_eq!(report.completion.result.unwrap().outcome, sealed());
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn caught_completion_provider_panic_retires_the_send_path_and_preserves_the_canonical_cut() {
    let (mut rig, source, _, human) = ready();
    let inputs = rig.inputs.clone();
    let mut calls = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| {
        rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
            calls += 1;
            assert!(calls != 3, "interrupted final evidence capture");
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, &human, None)
    })).is_err());
    assert_eq!(calls, 3);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert!(host.storage_failure().is_some());
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap().executions, 0);
    }
    let retry = rig.driver.complete_with_publication_source(&source,
        || panic!("unavailable owner must not sample time"), |_, _| panic!("unavailable owner must not repeat capture"), &human, None);
    assert!(retry.authorization_reads.is_empty());
    assert!(retry.completion.reads.is_empty());
    assert_eq!(retry.completion.result, Err(JournalError::Unavailable));
}

#[test]
fn a_foreign_human_key_cannot_trigger_capture_or_reservation_and_the_original_key_can() {
    let (mut rig, source, _, human) = ready();
    let (_other, _other_source, _other_path, foreign) = ready();
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let refused = rig.driver.complete_with_publication_source(&source,
        || panic!("foreign human cannot sample a clock"), |_, _| panic!("foreign human cannot capture"), &foreign, None);
    assert_eq!(refused.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert!(refused.authorization_reads.is_empty());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    let inputs = rig.inputs.clone();
    let permitted = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2),
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), &human, None);
    assert_eq!(permitted.completion.result.unwrap().basis, PublicationBasis::Revalidated);
}

#[test]
fn the_second_policy_snapshot_is_checked_even_when_both_witness_file_reads_are_identical() {
    let (mut rig, source, _, human) = ready();
    let inputs = rig.inputs.clone();
    let mut calls = 0;
    let report = rig.driver.complete_with_publication_source(&source, || ElapsedTick(2), |_, _| {
        calls += 1;
        let mut state = snapshot();
        if calls == 3 { state.values.insert(7, b"changed policy input".to_vec()); }
        Ok(DriverEvidence { snapshot: state, inputs: inputs.clone() })
    }, &human, None);
    assert_eq!(report.completion.reads, vec![Ok(identity(1)); 2]);
    let publication = report.completion.result.unwrap();
    assert_eq!(publication.basis, PublicationBasis::Rejected(Error::Binding));
    assert_eq!(publication.outcome, sealed());
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}
