//! Actual immutable-version file reads, not caller-supplied cached evidence.
#![cfg(unix)]
#[path = "support/file_evidence_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase, FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileSourceReviewError;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceSnapshot};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::io;

fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn file_capture_flows_through_real_helpers_independent_human_and_guarded_publication() {
    let mut file = FileRig::new();
    file.reviewed();
    assert_eq!(file.source.status().read_attempts, 2);
    for (member, client) in &file.rig.clients {
        let input = client.input().unwrap();
        let bytes = input.actual_input().submitted_bytes();
        let (own, other) = if member == "alpha" { (ALPHA, BETA) } else { (BETA, ALPHA) };
        assert!(bytes.windows(own.len()).any(|window| window == own));
        assert!(!bytes.windows(other.len()).any(|window| window == other));
        assert!(!bytes.windows(PRIVATE.len()).any(|window| window == PRIVATE));
    }
    let key = file.human();
    assert_eq!(file.source.status().read_attempts, 3);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    file.dispatch(&key);
    assert_eq!(file.source.status().read_attempts, 5);
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert_eq!(report.observations, vec![Ok(document(1).identity())]);
    assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, source_failure: None, .. })
        if publication.basis == PublicationBasis::Revalidated && publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(file.source.status().read_attempts, 6);
    std::fs::remove_file(&file.path).unwrap();
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(report.observations.is_empty());
    assert!(matches!(report.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. })));
    assert_eq!(file.source.status().read_attempts, 6);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().payload, b"publication");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn human_offer_never_uses_a_cached_view_after_source_loss_or_change() {
    for mode in 0..3 {
        let mut file = FileRig::new();
        file.reviewed();
        match mode {
            0 => std::fs::remove_file(&file.path).unwrap(),
            1 => replace(&file.path, &document(2)),
            _ => {
                let next = document(2);
                let mut snapshot = next.snapshot().clone(); snapshot.complete = false;
                let incomplete = EvidenceSnapshot::new(next.identity(), snapshot, next.contexts().clone()).unwrap();
                replace(&file.path, &incomplete);
            }
        }
        let report = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(31), || ElapsedTick(1));
        assert_eq!(report.observations.len(), 1);
        if mode == 1 { assert_eq!(report.observations, vec![Ok(document(2).identity())]); }
        assert!(matches!(report.result, Err(JournalError::Contract(Error::Incomplete | Error::Stale))));
        let host = file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.human_status(1001).unwrap_err(), JournalError::Contract(Error::Missing));
        assert_eq!(host.input_revision(1).unwrap(), 2);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn failed_second_file_read_retains_the_original_automatic_reservation() {
    let mut file = FileRig::new();
    file.reviewed();
    let key = file.human();
    let path = file.path.clone();
    let mut calls = 0;
    let report = file.rig.driver.step_from_file(&mut file.source, || {
        calls += 1;
        // The first provider read just completed; reserve still uses that valid
        // sample, then the ORIGINAL driver must perform its second actual read.
        if calls == 2 { std::fs::remove_file(&path).unwrap(); }
        ElapsedTick(1)
    }, Some(&key));
    assert_eq!(report.observations, vec![Ok(document(1).identity()), Err(EvidenceError::Io(io::ErrorKind::NotFound))]);
    assert!(matches!(report.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    {
        let host = file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.inspect().executions, 0);
    }
    let reads = file.source.status().read_attempts;
    file.rig.driver.cancel_active().unwrap();
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn actual_file_drift_after_dispatch_seals_and_reconciliation_does_not_reread() {
    for mode in 0..3 {
        let mut file = FileRig::new();
        file.reviewed();
        let key = file.human(); file.dispatch(&key);
        match mode {
            0 => std::fs::remove_file(&file.path).unwrap(),
            1 => replace(&file.path, &document(2)),
            _ => {
                let prior = document(1);
                let mut snapshot = prior.snapshot().clone(); snapshot.values.insert(7, b"substituted".to_vec());
                let changed = EvidenceSnapshot::new(prior.identity(), snapshot, prior.contexts().clone()).unwrap();
                replace(&file.path, &changed);
            }
        }
        let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
        assert_eq!(report.observations.len(), 1);
        if mode == 2 { assert_eq!(report.observations, vec![Err(EvidenceError::Data(Error::Binding))]); }
        assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
            if publication.outcome == sealed() && matches!(publication.basis, PublicationBasis::Rejected(_))));
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        let reads = file.source.status().read_attempts;
        let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
        assert!(report.observations.is_empty());
        assert_eq!(file.source.status().read_attempts, reads);
        assert!(matches!(report.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(outcome), .. }) if outcome == sealed()));
        let host = file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().payload, b"initial");
    }
}

#[test]
fn failed_file_launch_withdraws_prior_review_but_does_not_burn_the_new_round() {
    let mut file = FileRig::new();
    file.reviewed();
    let action = file.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    let original = document(1).inputs_for(&action, &driver::profile().committee).unwrap();
    let released = file.rig.driver.release();
    assert!(released.children.is_none());
    file.rig.driver = FileSupervisedDriver::new(released.supervisor);
    std::fs::remove_file(&file.path).unwrap();
    let (workers, clients) = driver::sockets(0);
    file.rig.clients = clients;
    let launch = launch(&file.rig, 202, workers);
    let error = file.rig.driver.start_file_review(&mut file.source, launch, || ElapsedTick(1)).unwrap_err();
    assert!(matches!(error, FileSourceReviewError::Source { error: EvidenceError::Io(io::ErrorKind::NotFound), withdrawal: None }));
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(host.input_revision(1).unwrap(), 2);
        let revision = host.revision();
        assert!(host.authorize(revision, 1, &original, driver::snapshot()).is_err());
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
    assert!(file.rig.clients.values_mut().all(|client| client.step().is_err()));
    replace(&file.path, &document(1));
    file.start(202);
    let report = file.finish(Verdict::Allow);
    assert!(matches!(report.result, Ok(FileDriverEvent::ReviewApplied { .. })));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    assert_eq!(file.rig.driver.supervisor().host().unwrap().input_revision(1).unwrap(), 3);
}

#[test]
fn source_error_and_failed_durable_withdrawal_are_both_retained() {
    let mut file = FileRig::new();
    file.reviewed();
    let released = file.rig.driver.release();
    file.rig.driver = FileSupervisedDriver::new(released.supervisor);
    std::fs::remove_file(&file.path).unwrap();
    std::fs::write(file.rig.root.store().join("delivery.pending"), b"occupied stage").unwrap();
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    let (workers, _clients) = driver::sockets(0);
    let launch = launch(&file.rig, 202, workers);
    let error = file.rig.driver.start_file_review(&mut file.source, launch, || ElapsedTick(1)).unwrap_err();
    assert!(matches!(error, FileSourceReviewError::Source {
        error: EvidenceError::Io(io::ErrorKind::NotFound), withdrawal: Some(JournalError::Io(_)),
    }));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), before);
    assert!(file.rig.driver.supervisor().host().unwrap().storage_failure().is_some());
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(file.source.status().read_attempts, 3);
}

#[test]
fn stale_launch_and_cancelled_jobs_do_not_contact_the_file_source() {
    let mut file = FileRig::new();
    let (workers, _clients) = driver::sockets(0);
    let mut invalid = launch(&file.rig, 101, workers); invalid.expected_input_revision = 99;
    let error = file.rig.driver.start_file_review(&mut file.source, invalid, || ElapsedTick(1)).unwrap_err();
    assert!(matches!(error, FileSourceReviewError::Control(JournalError::Contract(Error::Stale))));
    assert_eq!(file.source.status().read_attempts, 0);
    let (mut workers, _clients) = driver::sockets(0); workers.remove("beta");
    let invalid = launch(&file.rig, 101, workers);
    let error = file.rig.driver.start_file_review(&mut file.source, invalid, || ElapsedTick(1)).unwrap_err();
    assert!(matches!(error, FileSourceReviewError::Control(JournalError::Contract(Error::Binding))));
    assert_eq!(file.source.status().read_attempts, 0);
    file.reviewed();
    file.rig.driver.cancel_active().unwrap();
    std::fs::remove_file(&file.path).unwrap();
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(report.observations.is_empty());
    assert!(matches!(report.result, Ok(FileDriverEvent::Idle)));
    let request = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(31), || ElapsedTick(2));
    assert!(request.observations.is_empty());
    assert!(request.result.is_err());
    assert_eq!(file.source.status().read_attempts, 2);
}

#[test]
fn human_request_expiry_is_checked_after_the_file_read_without_minting_a_key() {
    let mut file = FileRig::new();
    file.reviewed();
    let mut calls = 0;
    let report = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(31), || {
        calls += 1; ElapsedTick(if calls == 1 { 1 } else { 31 })
    });
    assert_eq!(report.observations, vec![Ok(document(1).identity())]);
    assert!(matches!(report.result, Err(JournalError::Contract(Error::Stale))));
    assert!(file.rig.driver.supervisor().host().unwrap().human_status(1001).is_err());
    // No request was created or approved, so an explicit valid first request
    // still succeeds. This is not reissuing an expired approval or new consent.
    let report = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(50), || ElapsedTick(31));
    let request = report.result.unwrap();
    assert_eq!(request.evidence().expires_at(), ElapsedTick(50));
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}
