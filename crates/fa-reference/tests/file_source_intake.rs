//! Cold actor entry through actual registered files and the original authority.
#![cfg(unix)]
#[path = "support/file_source_intake.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceError;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::Error;
use std::fs;
use std::io::ErrorKind;

#[test]
fn cold_entry_reaches_full_input_human_approval_and_guarded_publication() {
    let mut file = cold(10, StateLimits::default());
    assert_eq!(file.source.status().read_attempts, 0);
    let proposal = file.rig.proposal();
    assert_eq!(file.rig.port.submit(1, &proposal).unwrap_err(), ActorError::Unavailable);
    let report = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1));
    assert_eq!(report.result, Ok(document(1).identity()));
    assert_eq!(report.source_updates, vec![Ok(document(1).identity())]);
    assert_eq!(report.observations, vec![Ok(document(1).identity())]);
    let ticket = file.rig.port.submit(1, &proposal).unwrap();
    assert!(matches!(file.rig.port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(file.rig.port.submit(2, &proposal).unwrap_err(), ActorError::Unavailable);
    file.reviewed();
    let key = file.human(); file.dispatch(&key);
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(file.source.status().read_attempts, 7);
    fs::remove_file(&file.path).unwrap();
    let settled = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(settled.source_updates.is_empty());
    assert!(matches!(settled.result, Ok(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. })));
    assert!(matches!(file.rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn missing_or_incomplete_replacement_withdraws_the_unused_intake_slot() {
    for missing in [true, false] {
        let mut file = cold(10, StateLimits::default());
        file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
        if missing { fs::remove_file(&file.path).unwrap(); }
        else {
            let next = document(2); let mut state = next.snapshot().clone(); state.complete = false;
            replace(&file.path, &EvidenceSnapshot::new(next.identity(), state, next.contexts().clone()).unwrap());
        }
        let report = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(2));
        assert_eq!(report.result, Err(JournalError::Contract(Error::Incomplete)));
        assert_eq!(report.source_updates.len(), 1);
        let proposal = file.rig.proposal();
        assert_eq!(file.rig.port.submit(1, &proposal).unwrap_err(), ActorError::Unavailable);
        {
            let host = file.rig.driver.supervisor().host().unwrap();
            assert!(host.inspect().control.ledger.stages.is_empty());
            assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        }
        replace(&file.path, &document(3));
        file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(3)).result.unwrap();
        assert!(file.rig.port.submit(1, &proposal).is_ok());
    }
}

#[test]
fn read_start_lease_is_checked_again_before_intake_becomes_available() {
    for completed in [4, 5] {
        let mut file = cold(3, StateLimits::default()); let mut ticks = 0;
        let report = file.rig.driver.prepare_file_intake(&mut file.source, || {
            ticks += 1; ElapsedTick(if ticks == 1 { 2 } else { completed })
        });
        assert_eq!(ticks, 2); assert_eq!(report.source_updates, vec![Ok(document(1).identity())]);
        let proposal = file.rig.proposal();
        if completed == 4 {
            report.result.unwrap(); assert!(file.rig.port.submit(1, &proposal).is_ok());
        } else {
            assert_eq!(report.result, Err(JournalError::Contract(Error::Stale)));
            assert_eq!(file.rig.port.submit(1, &proposal).unwrap_err(), ActorError::Unavailable);
            file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(5)).result.unwrap();
            assert!(file.rig.port.submit(1, &proposal).is_ok());
        }
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn a_new_reader_cannot_restore_an_older_producer_into_an_unused_slot() {
    let mut file = cold(10, StateLimits::default()); replace(&file.path, &document(2));
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    replace(&file.path, &document(1));
    file.source = FileEvidenceSource::new(&file.path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let report = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(2));
    assert_eq!(report.result, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(report.source_updates, vec![Err(FileSourceError::Refused(Error::Stale))]);
    assert_eq!(file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap().producer.unwrap().generation, 2);
    replace(&file.path, &document(3));
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(3)).result.unwrap();
    assert!(file.rig.port.submit(1, &file.rig.proposal()).is_ok());
}

#[test]
fn compound_read_and_withdrawal_failure_returns_neither_an_intake_slot_nor_candidate_identity() {
    let mut file = cold(10, StateLimits::default());
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    fs::remove_file(&file.path).unwrap();
    fs::write(file.rig.root.store().join("delivery.pending"), b"occupied").unwrap();
    let report = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(2));
    assert!(matches!(report.result, Err(JournalError::Io(_))));
    assert!(matches!(&report.source_updates[..], [Err(FileSourceError::Read {
        error: EvidenceError::Io(ErrorKind::NotFound), withdrawal: Some(JournalError::Io(_)),
    })]));
    assert_eq!(report.observations, vec![Err(EvidenceError::Io(ErrorKind::NotFound))]);
    assert_eq!(file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);
    assert!(file.rig.driver.supervisor().host().unwrap().storage_failure().is_some());
}

#[test]
fn no_source_or_terminal_stop_refuses_before_reading_and_cannot_leave_a_manual_slot_usable() {
    let mut rig = driver::Rig::new();
    let path = rig.root.0.join("evidence.json"); replace(&path, &document(1));
    let mut reader = FileEvidenceSource::new(&path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(document(1).snapshot().clone())).unwrap();
    let report = rig.driver.prepare_file_intake(&mut reader, || panic!("unconfigured intake sampled time"));
    assert_eq!(report.result, Err(JournalError::Contract(Error::WrongState)));
    assert!(report.source_updates.is_empty()); assert_eq!(reader.status().read_attempts, 0);
    assert_eq!(rig.port.submit(1, &rig.proposal()).unwrap_err(), ActorError::Unavailable);

    let mut file = cold(10, StateLimits::default());
    let control = file.rig.driver.supervisor().host().unwrap().inspect().control;
    file.rig.driver.request_stop(StopRequest { operation: 1, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }).unwrap();
    let report = file.rig.driver.prepare_file_intake(&mut file.source, || panic!("stopped intake sampled time"));
    assert_eq!(report.result, Err(JournalError::Contract(Error::WrongState)));
    assert!(report.source_updates.is_empty()); assert_eq!(file.source.status().read_attempts, 0);
}

#[test]
fn source_capture_capacity_and_host_mutation_cannot_replenish_an_unused_slot() {
    let mut file = cold(10, StateLimits { events: 1, ..StateLimits::default() });
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    let report = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(2));
    assert_eq!(report.result, Err(JournalError::Contract(Error::Limit)));
    assert_eq!(report.source_updates, vec![Err(FileSourceError::Refused(Error::Limit))]);
    assert_eq!(file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);

    let mut file = cold(10, StateLimits::default());
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    drop(file.rig.driver.supervisor_mut().host_mut().unwrap());
    assert_eq!(file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(), ActorError::Unavailable);
    file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1)).result.unwrap();
    assert!(file.rig.port.submit(1, &file.rig.proposal()).is_ok());
}
