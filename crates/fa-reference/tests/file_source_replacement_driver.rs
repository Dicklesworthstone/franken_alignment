//! Existing socket-worker driver plus the original mandatory file-state gate.
#![cfg(unix)]
#[path = "support/file_evidence_driver.rs"] mod fixture;
use fixture::{FileRig, SOURCE, document, driver, replace};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileReviewLaunch;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::source::{FileSourcePolicy, FileSourceReplacement};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn configured() -> FileRig {
    let mut rig = driver::Rig::new();
    let path = rig.root.0.join("evidence.json"); replace(&path, &document(1));
    let mut source = FileEvidenceSource::new(&path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_file_source(revision, FileSourcePolicy {
            source: StateSource { scope: driver::profile().delivery.scope, source: SOURCE, generation: 1 },
            limits: StateLimits::default(), freshness: StateFreshness::new(10).unwrap(),
        }).unwrap();
        let revision = host.revision(); host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let mut file = FileRig { rig, source, path }; submit(&mut file, 1); file
}
fn submit(file: &mut FileRig, request: u64) {
    let proposal = file.rig.proposal();
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    file.rig.driver.supervisor_mut().set_snapshot(revision, Some(document(1).snapshot().clone())).unwrap();
    let _ticket = file.rig.port.submit(request, &proposal).unwrap();
}
fn request(file: &FileRig, operation: u64) -> FileSourceReplacement {
    let host = file.rig.driver.supervisor().host().unwrap();
    let generation = host.file_source_status().unwrap().capture.source.generation;
    FileSourceReplacement { operation, expected_generation: generation, next_generation: generation + 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch }
}
fn refresh(file: &mut FileRig) {
    let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); host.refresh_file_source(revision, &mut file.source, ElapsedTick(1)).unwrap();
}
fn start(file: &mut FileRig, request: u64, round: u64) {
    let launch = {
        let host = file.rig.driver.supervisor().host().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("fixture request was not admitted"); };
        let action = host.request_action(request).unwrap();
        let (workers, clients) = driver::sockets(action.spec().policy_epoch);
        file.rig.clients = clients;
        FileReviewLaunch { request, round, window: driver::helper::window(&host),
            expected_input_revision: host.input_revision(attempt).unwrap(), workers, limits: HelperLimits::default() }
    };
    file.rig.driver.start_file_review(&mut file.source, launch, || ElapsedTick(1)).unwrap();
}

#[test]
fn stale_replacement_preserves_workers_while_committed_replacement_retires_the_original_job() {
    let mut file = configured(); file.start(101);
    let valid = request(&file, 1); let deadline = file.rig.driver.next_review_deadline();
    assert!(deadline.is_some());
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    let stale = FileSourceReplacement { expected_authority_epoch: valid.expected_authority_epoch + 1, ..valid };
    assert_eq!(file.rig.driver.replace_file_source(stale), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(file.rig.driver.next_review_deadline(), deadline);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), before);
    let change = file.rig.driver.replace_file_source(valid).unwrap();
    assert_eq!(change.cancelled, vec![1]); assert_eq!(change.refunded_units, 16);
    assert_eq!(file.rig.driver.next_review_deadline(), None);
    let event = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(event.result, Ok(FileDriverEvent::Stopped { request: 1, stage: ActionState::Cancelled })));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn historical_retry_does_not_retire_a_new_helper_cohort() {
    let mut file = configured(); let original = request(&file, 1);
    let receipt = file.rig.driver.replace_file_source(original).unwrap();
    refresh(&mut file); submit(&mut file, 2); start(&mut file, 2, 202);
    let deadline = file.rig.driver.next_review_deadline(); assert!(deadline.is_some());
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(file.rig.driver.replace_file_source(original).unwrap(), receipt);
    assert_eq!(file.rig.driver.next_review_deadline(), deadline);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), before);
    assert!(matches!(file.finish(Verdict::Allow).result, Ok(FileDriverEvent::ReviewApplied { request: 2, .. })));
}

#[test]
fn pending_publication_remains_charged_until_guarded_nonexecution_is_reconciled() {
    let mut file = configured(); file.reviewed(); let human = file.human(); file.dispatch(&human);
    let replacement = request(&file, 1);
    let change = file.rig.driver.replace_file_source(replacement).unwrap();
    assert!(change.cancelled.is_empty()); assert_eq!(change.refunded_units, 0);
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 1 });
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    std::fs::remove_file(&file.path).unwrap();
    let publication = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(publication.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if matches!(publication.basis, PublicationBasis::Rejected(_))));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    let settled = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(settled.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn executed_receipt_survives_source_replacement_without_republication() {
    let mut file = configured(); file.reviewed(); let human = file.human(); file.dispatch(&human);
    let published = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(published.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.basis == PublicationBasis::Revalidated));
    let replacement = request(&file, 1);
    let change = file.rig.driver.replace_file_source(replacement).unwrap();
    assert_eq!(change.refunded_units, 0); assert!(change.cancelled.is_empty());
    std::fs::remove_file(&file.path).unwrap();
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        let historical = host.publish_checked(revision, 1, None, document(1).snapshot().clone(), ElapsedTick(1)).unwrap();
        assert_eq!(historical.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    }
    let settled = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(settled.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. })));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
