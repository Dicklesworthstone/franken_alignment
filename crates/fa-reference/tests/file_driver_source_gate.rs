//! Native leased source capture at the actual driver evidence boundaries.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceError;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;
use std::fs;
use std::io::ErrorKind;

#[test]
fn file_reads_renew_original_leases_through_human_dispatch_and_first_publication() {
    let mut file = configured(1, 3, StateLimits::default()); reviewed(&mut file);
    let baseline = capture_count(&file);
    // The previous observation expires at tick 4. A NEW read permits the same
    // reviewed bytes without silently widening their original three-tick lease.
    let key = human(&mut file, 4);
    assert_eq!(capture_count(&file), baseline + 1);
    let dispatch = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(6), Some(&key));
    assert!(matches!(dispatch.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(dispatch.source_updates, vec![Ok(document(1).identity()); 2]);
    assert_eq!(capture_count(&file), baseline + 3);
    let published = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(8), None);
    assert!(matches!(published.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.basis == PublicationBasis::Revalidated));
    assert_eq!(published.source_updates, vec![Ok(document(1).identity())]);
    assert_eq!(capture_count(&file), baseline + 4);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    let reads = file.source.status().read_attempts;
    fs::remove_file(&file.path).unwrap();
    let settled = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(12), None);
    assert!(matches!(settled.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    assert!(settled.source_updates.is_empty()); assert!(settled.observations.is_empty());
    assert_eq!(file.source.status().read_attempts, reads);
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.charged, 16); assert_eq!(state.executions, 1);
}

#[test]
fn stored_callback_evidence_cannot_renew_a_source_but_a_real_file_read_can() {
    let mut file = configured(1, 3, StateLimits::default()); reviewed(&mut file);
    let key = human(&mut file, 1);
    let input = {
        let host = file.rig.driver.supervisor().host().unwrap();
        document(1).inputs_for(host.request_action(1).unwrap(), &driver::profile().committee).unwrap()
    };
    let count = capture_count(&file);
    let refused = file.rig.driver.step_with_evidence(|| ElapsedTick(4), |_, _| Ok(DriverEvidence {
        snapshot: document(1).snapshot().clone(), inputs: Some(input.clone()),
    }), Some(&key));
    assert!(matches!(refused, Err(JournalError::Contract(Error::Stale))));
    assert_eq!(capture_count(&file), count);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    // Same action, original reviewer key and same tick; only the real observation
    // path differs. Neither a new review nor a new automatic grant is fabricated.
    let accepted = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(4), Some(&key));
    assert!(matches!(accepted.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(accepted.source_updates, vec![Ok(document(1).identity()); 2]);
    assert_eq!(capture_count(&file), count + 2);
}

#[test]
fn newly_constructed_reader_cannot_bypass_the_durable_producer_floor() {
    let mut file = configured(2, 10, StateLimits::default()); reviewed(&mut file);
    let key = human(&mut file, 1);
    replace(&file.path, &document(1));
    file.source = FileEvidenceSource::new(&file.path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), Some(&key));
    assert_eq!(report.source_updates, vec![Err(FileSourceError::Refused(Error::Stale))]);
    assert!(matches!(report.result, Err(JournalError::Contract(Error::Stale))));
    let host = file.rig.driver.supervisor().host().unwrap();
    let source = host.file_source_status().unwrap();
    assert_eq!(source.producer.unwrap().generation, 2); assert_eq!(source.capture.closed, None);
    assert_eq!(host.inspect().control.ledger.reserved, 16); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
}

#[test]
fn loss_during_the_second_authorization_read_keeps_the_spent_reservation_and_withdraws_source() {
    let mut file = configured(1, 10, StateLimits::default()); reviewed(&mut file);
    let key = human(&mut file, 1); let path = file.path.clone(); let mut calls = 0;
    let report = file.rig.driver.step_from_file(&mut file.source, || {
        calls += 1;
        // Initial driver clock; first read start; first read completion; then
        // second read start, which follows original automatic authorization.
        if calls == 4 { fs::remove_file(&path).unwrap(); }
        ElapsedTick(1)
    }, Some(&key));
    assert!(matches!(report.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(report.source_updates.len(), 2);
    assert_eq!(report.source_updates[0], Ok(document(1).identity()));
    assert_eq!(report.source_updates[1], Err(FileSourceError::Read {
        error: EvidenceError::Io(ErrorKind::NotFound), withdrawal: None,
    }));
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn read_failure_and_failed_withdrawal_are_both_retained_without_candidate_evidence() {
    let mut file = configured(1, 10, StateLimits::default()); reviewed(&mut file);
    let key = human(&mut file, 1);
    fs::remove_file(&file.path).unwrap();
    fs::write(file.rig.root.store().join("delivery.pending"), b"inert staging").unwrap();
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), Some(&key));
    assert!(matches!(report.result, Err(JournalError::Io(_))));
    assert!(matches!(&report.source_updates[..], [Err(FileSourceError::Read {
        error: EvidenceError::Io(ErrorKind::NotFound), withdrawal: Some(JournalError::Io(_)),
    })]));
    assert_eq!(report.observations, vec![Err(EvidenceError::Io(ErrorKind::NotFound))]);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some()); assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.inspect().control.ledger.reserved, 16); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn source_update_at_publication_is_durable_but_does_not_reauthorize_the_consumed_keys() {
    let mut file = configured(1, 10, StateLimits::default()); reviewed(&mut file);
    let key = human(&mut file, 1); file.dispatch(&key);
    replace(&file.path, &document(2));
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert_eq!(report.source_updates, vec![Ok(document(2).identity())]);
    assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if matches!(publication.basis, PublicationBasis::Rejected(_))));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    {
        let host = file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 2);
        assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    }
    fs::remove_file(&file.path).unwrap();
    let result = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(result.source_updates.is_empty());
    assert!(matches!(result.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn time_after_a_committed_capture_cannot_extend_its_read_start_lease() {
    for complete_at in [4, 5] {
        let mut file = configured(1, 3, StateLimits::default()); reviewed(&mut file);
        let mut calls = 0;
        let report = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(30), || {
            calls += 1;
            ElapsedTick(match calls { 1 => 1, 2 => 2, _ => complete_at })
        });
        assert_eq!(report.source_updates, vec![Ok(document(1).identity())]);
        if complete_at == 4 { assert!(report.result.is_ok()); }
        else { assert!(matches!(report.result, Err(JournalError::Contract(Error::Stale)))); }
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}
