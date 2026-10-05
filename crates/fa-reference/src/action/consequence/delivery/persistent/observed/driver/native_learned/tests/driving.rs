//! Real native models and policy-file observations in one original effect loop.
//! Positive/negative controls share the same original decoder and request owner.
use super::*;
use crate::action::consequence::delivery::{EndpointOutcome, persistent::Reconciliation};
use crate::action::consequence::delivery::persistent::observed::FileHumanPermit;
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot,
    FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use std::cell::Cell;
use std::path::{Path, PathBuf};

fn live(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>)
    -> Result<FileNativeDriverEvent, JournalError>
{
    run.step(|| ElapsedTick(1), || Ok(snapshot()), human)
}
fn complete_live(run: &mut FileNativeSupervisedDriver) {
    for _ in 0..crate::action::consequence::oversight::learned_host::sidecar::workers::MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { return; }
        assert!(matches!(live(run, None).unwrap(), FileNativeDriverEvent::Review(_)));
    }
    panic!("live native congress exceeded its original frozen poll allowance");
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer) -> FileHumanPermit {
    let request = run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); reviewer.approve(&mut host, revision, &request).unwrap()
}
fn policy(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 99, generation, scope: profile().delivery.scope },
        snapshot(), BTreeMap::from([("alpha".to_owned(), Vec::new()), ("beta".to_owned(), Vec::new())])).unwrap()
}
fn replace(path: &Path, observation: &EvidenceSnapshot) {
    let next = path.with_extension("next");
    std::fs::write(&next, observation.encode()).unwrap();
    std::fs::rename(next, path).unwrap();
}
fn policy_file(root: &Directory) -> (PathBuf, FileEvidenceSource) {
    let path = root.store().parent().unwrap().join("native-policy");
    replace(&path, &policy(1));
    let source = FileEvidenceSource::new(&path, 99, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    (path, source)
}

#[test]
fn original_native_refinement_and_real_policy_file_drive_both_keys_without_handoff_or_packets() {
    let root = Directory::new(); let (port, ticket, reviewer, mut run) = start(&root, 1, b"allow");
    let (path, mut source) = policy_file(&root);
    assert_eq!(run.next_review_deadline(), Some(ElapsedTick(10)));
    assert!(run.request_human_approval(1000, ElapsedTick(80), ElapsedTick(1)).is_err());
    // Mixing a manual original quantum with the live loop must not leave its
    // evidence adapter stuck on the coarse input after acknowledged refinement.
    for _ in 0..64 {
        if run.progress().input_revision > 1 { break; }
        run.advance(run.review().revision(), ElapsedTick(1), snapshot()).unwrap();
    }
    assert!(run.progress().input_revision > 1);
    assert_eq!(run.review().status(), NativeReviewStatus::Running);
    for _ in 0..crate::action::consequence::oversight::learned_host::sidecar::workers::MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert_eq!(report.observations, vec![Ok(policy(1).identity())]);
        assert!(report.source_updates.is_empty());
        assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().history().len() > 1);
    assert!(native_records(run.review()).any(|r| r.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    assert_eq!(run.next_review_deadline(), None);
    let before = costs(run.review());
    let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(report.observations.len(), 1);
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::AwaitingHuman { request: 41 })));
    assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let human = approve(&mut run, &reviewer);
    let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), Some(&human));
    assert_eq!(report.observations.len(), 2);
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::Dispatched { request: 41, .. })));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(report.observations.len(), 1);
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::PublicationChecked {
        publication, source_failure: None, .. }) if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    let reads = source.status().read_attempts;
    std::fs::remove_file(path).unwrap();
    let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert!(report.observations.is_empty() && report.source_updates.is_empty());
    assert_eq!(source.status().read_attempts, reads);
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. })));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(costs(run.review()), before);
    let parts = run.into_handoff().unwrap();
    assert_eq!(costs(&parts.review), before);
    let published = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
    assert_eq!(published.payload, b"visible"); assert_eq!(published.executions, 1);
    assert_eq!(published.control.ledger.charged, 16);
}

#[test]
fn live_native_deny_and_numerical_alarm_do_not_become_publication_permissions() {
    for (mode, spelling) in [(0, b"deny".as_slice()), (2, b"allow".as_slice())] {
        let root = Directory::new(); let (_, _, _, mut run) = start(&root, mode, spelling);
        complete_live(&mut run);
        assert_eq!(run.review().status(), NativeReviewStatus::Finished);
        assert_ne!(run.supervisor().host().unwrap().inspect().control.decisions.get(&1), Some(&Consequence::Continue));
        assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
        assert!(matches!(run.step(|| panic!("no terminal clock"), || panic!("no terminal source"), None).unwrap(),
            FileNativeDriverEvent::Driver(FileDriverEvent::Idle)));
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn actor_cancellation_precedes_all_live_callbacks_and_preserves_completed_native_work() {
    let root = Directory::new(); let (port, ticket, _, mut run) = start(&root, 0, b"allow");
    live(&mut run, None).unwrap(); live(&mut run, None).unwrap();
    let before = costs(run.review()); assert!(before.values().any(|work| work.0 > 0));
    let reservation = run.review().reservation();
    port.cancel(&ticket).unwrap();
    assert!(matches!(run.step(|| panic!("cancel before clock"), || panic!("cancel before policy"), None).unwrap(),
        FileNativeDriverEvent::Review(FileNativeDriverProgress { status: NativeReviewStatus::Cancelled, .. })));
    assert_eq!(costs(run.review()), before); assert_eq!(run.review().reservation(), reservation);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn lost_original_source_refuses_before_file_io_or_further_scoring_in_review_and_ready() {
    for ready in [false, true] {
        let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, b"allow");
        if ready { complete_live(&mut run); } else { live(&mut run, None).unwrap(); live(&mut run, None).unwrap(); }
        let before = costs(run.review()); let (_, mut source) = policy_file(&root);
        { let mut host = run.supervisor_mut().host_mut().unwrap(); step(&mut host); }
        let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert!(report.result.is_err());
        assert!(report.observations.is_empty() && report.source_updates.is_empty());
        assert_eq!(source.status().read_attempts, 0);
        assert_eq!(costs(run.review()), before);
        assert_ne!(run.review().status(), NativeReviewStatus::Running);
        let host = run.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn invalid_policy_file_never_silently_drops_helper_context_or_reuses_old_native_evidence() {
    for defect in 0..5 {
        let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, b"allow");
        let (path, mut source) = policy_file(&root);
        run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap();
        run.step_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap();
        let before = costs(run.review()); assert!(before.values().any(|work| work.0 > 0));
        let observation = policy(2); let mut identity = observation.identity();
        let mut state = observation.snapshot().clone(); let mut contexts = observation.contexts().clone();
        match defect {
            0 => { contexts.insert("alpha".to_owned(), b"unreviewed helper input".to_vec()); }
            1 => { contexts.remove("beta"); }
            2 => { identity.scope.principal += 1; }
            3 => { state.complete = false; }
            _ => { std::fs::remove_file(&path).unwrap(); }
        }
        if defect != 4 { replace(&path, &EvidenceSnapshot::new(identity, state, contexts).unwrap()); }
        let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert!(report.result.is_err()); assert_eq!(report.observations.len(), 1);
        assert!(report.observations[0].is_err() && report.source_updates.is_empty());
        assert_eq!(run.review().status(), NativeReviewStatus::Cancelled);
        assert_eq!(costs(run.review()), before);
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn policy_loss_after_reservation_preserves_the_original_charge_until_explicit_cancellation() {
    let root = Directory::new(); let (port, ticket, reviewer, mut run) = start(&root, 0, b"allow");
    complete_live(&mut run); let human = approve(&mut run, &reviewer);
    let before = costs(run.review()); let calls = Cell::new(0);
    let result = run.step(|| ElapsedTick(1), || {
        calls.set(calls.get() + 1);
        if calls.get() == 1 { Ok(snapshot()) } else { Err(Error::Missing) }
    }, Some(&human));
    assert!(matches!(result, Err(JournalError::Contract(Error::Missing))));
    assert_eq!(calls.get(), 2);
    assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 84);
    assert_eq!(costs(run.review()), before);
    run.cancel(run.review().revision()).unwrap();
    assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
}

#[test]
fn source_loss_at_first_publication_uses_original_nonexecution_and_reconciliation() {
    let root = Directory::new(); let (port, ticket, reviewer, mut run) = start(&root, 0, b"allow");
    complete_live(&mut run); let human = approve(&mut run, &reviewer);
    assert!(matches!(live(&mut run, Some(&human)).unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::Dispatched { .. })));
    { let mut host = run.supervisor_mut().host_mut().unwrap(); step(&mut host); }
    let (_, mut source) = policy_file(&root);
    let report = run.step_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert!(report.observations.is_empty()); assert_eq!(source.status().read_attempts, 0);
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::PublicationChecked {
        publication, source_failure: Some(_), .. }) if !matches!(publication.outcome, EndpointOutcome::Executed { .. })));
    assert!(matches!(run.step(|| ElapsedTick(1), || panic!("settlement has no source"), None).unwrap(),
        FileNativeDriverEvent::Driver(FileDriverEvent::Reconciled { .. })));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    let host = run.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn every_observation_unwind_cut_retires_native_custody_without_fabricating_request_cancellation() {
    for cut in 0..3 {
        let root = Directory::new(); let (port, ticket, _, mut run) = start(&root, 0, b"allow");
        live(&mut run, None).unwrap(); live(&mut run, None).unwrap();
        let before = costs(run.review()); let reservation = run.review().reservation(); let clocks = Cell::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.step(|| {
                clocks.set(clocks.get() + 1);
                if (cut == 0 && clocks.get() == 1) || (cut == 2 && clocks.get() == 2) { panic!("clock interrupted"); }
                ElapsedTick(1)
            }, || { if cut == 1 { panic!("policy interrupted"); } Ok(snapshot()) }, None)
        }));
        assert!(result.is_err());
        assert_eq!(run.review().status(), NativeReviewStatus::Cancelled);
        assert_eq!(costs(run.review()), before); assert_eq!(run.review().reservation(), reservation);
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 41 }));
        assert!(matches!(run.step(|| panic!("no retry clock"), || panic!("no retry scoring"), None).unwrap(),
            FileNativeDriverEvent::Driver(FileDriverEvent::Idle)));
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    }
}

#[test]
fn interrupted_publication_and_reopened_unknown_settlement_never_retry_native_models_or_send() {
    let root = Directory::new(); let (port, ticket, reviewer, mut run) = start(&root, 0, b"allow");
    complete_live(&mut run); let human = approve(&mut run, &reviewer);
    assert!(matches!(live(&mut run, Some(&human)).unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::Dispatched { .. })));
    let before = costs(run.review());
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.step(|| ElapsedTick(1), || panic!("first publication evidence interrupted"), None)
    }));
    assert!(interrupted.is_err());
    assert_eq!(run.progress().phase, FileDriverPhase::AwaitingReconciliation { request: 41 });
    assert_eq!(costs(run.review()), before);
    assert!(matches!(run.step(|| ElapsedTick(2), || panic!("no send retry"), None).unwrap(),
        FileNativeDriverEvent::Driver(FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. })));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let FileNativeDriverHandoff { driver, review } = run.into_handoff().unwrap();
    drop(driver);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let (_, mut driver) = host.into_supervised_driver(); driver.resume_reconciliation(41).unwrap();
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(3), || panic!("paused source not needed"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert_eq!(costs(&review), before);
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 84);
}

#[test]
fn elapsed_observation_deadline_does_not_backdate_a_native_vote_or_slide_its_round() {
    let root = Directory::new(); let (_, _, _, mut run) = start(&root, 1, b"allow");
    assert_eq!(run.next_review_deadline(), Some(ElapsedTick(10)));
    let clock = Cell::new(0);
    let event = run.step(|| { clock.set(clock.get() + 1); ElapsedTick(if clock.get() == 1 { 1 } else { 10 }) },
        || Ok(snapshot()), None).unwrap();
    assert!(matches!(event, FileNativeDriverEvent::Review(_)));
    if run.review().status() == NativeReviewStatus::Running {
        assert_eq!(run.next_review_deadline(), Some(ElapsedTick(15)));
        run.step(|| ElapsedTick(15), || Ok(snapshot()), None).unwrap();
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(native_records(run.review()).all(|record| record.progress.completed_probes == 0 && !record.progress.native_started));
    assert_eq!(run.progress().input_revision, 1);
    assert_ne!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
}
