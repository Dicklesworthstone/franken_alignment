//! Actual evidence files feeding the ORIGINAL leased gate and durable host.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::source::{FileSourcePolicy, FileSourceError};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;
use std::path::PathBuf;
use std::rc::Rc;

fn policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 901, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(20).unwrap() }
}
fn image(generation: u64, epoch: u64, complete: bool, context: &[u8]) -> EvidenceSnapshot {
    let mut state = snapshot(); state.semantic_epoch = epoch; state.complete = complete;
    EvidenceSnapshot::new(EvidenceIdentity { source: 901, generation, scope: profile().delivery.scope },
        state, MEMBERS.into_iter().map(|name| (name.to_owned(), context.to_vec())).collect()).unwrap()
}
fn write(root: &Directory, capture: &EvidenceSnapshot) {
    let staged = root.0.join("evidence.new");
    std::fs::write(&staged, capture.encode()).unwrap();
    std::fs::rename(staged, path(root)).unwrap();
}
fn path(root: &Directory) -> PathBuf { root.0.join("evidence.json") }
fn reader(root: &Directory) -> FileEvidenceSource {
    FileEvidenceSource::new(path(root), 901, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
fn setup(root: &Directory, p: FileSourcePolicy) -> (FileOversight, FileHumanReviewer, FileEvidenceSource, Rc<EvidenceSnapshot>) {
    let (mut host, reviewer) = create(root);
    host.enable_file_source(host.revision(), p).unwrap();
    write(root, &image(4, 1, true, b"versioned exact worker context"));
    let mut source = reader(root);
    let capture = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    (host, reviewer, source, capture)
}
fn source_keys(host: &mut FileOversight, reviewer: &FileHumanReviewer, capture: &EvidenceSnapshot, id: u64) -> Keys {
    let action = host.propose(host.revision(), id, spec(host, b"published"), capture.snapshot().clone()).unwrap();
    let inputs = capture.inputs_for(&action, &profile().committee).unwrap();
    review_existing(host, id, id + 100, &inputs);
    let automatic = host.authorize(host.revision(), id, &inputs, capture.snapshot().clone()).unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}

#[test]
fn original_live_gate_blocks_uncaptured_state_and_accepts_renewed_exact_file_evidence() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    host.enable_file_source(host.revision(), policy()).unwrap();
    assert!(host.publication_guard_required());
    assert_eq!(host.propose(host.revision(), 1, spec(&host, b"unobserved"), snapshot()).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    write(&root, &image(4, 1, true, b"private assigned contexts"));
    let mut source = reader(&root);
    let captured = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let keys = source_keys(&mut host, &reviewer, &captured, 1);
    let input_revision = host.input_revision(1).unwrap();
    let previous = host.file_source_status().unwrap();
    assert_eq!(previous.capture.retained_events, 1);
    assert!(previous.capture.closed.is_some());
    let repeated = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
    assert_eq!(repeated, captured);
    let renewed = host.file_source_status().unwrap();
    assert_eq!(renewed.capture.retained_events, 2);
    assert_ne!(renewed.capture.closed, previous.capture.closed);
    assert_eq!(host.input_revision(1).unwrap(), input_revision);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 1).unwrap_err(), JournalError::Contract(Error::Incomplete));
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    std::fs::remove_file(path(&root)).unwrap();
    assert!(matches!(host.refresh_file_source(host.revision(), &mut source, ElapsedTick(4)),
        Err(FileSourceError::Read { withdrawal: None, .. })));
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn an_expired_source_cannot_be_renewed_by_advancing_the_clock_or_reusing_a_complete_flag() {
    let root = Directory::new();
    let mut p = policy(); p.freshness = StateFreshness::new(3).unwrap();
    let (mut host, reviewer, mut source, capture) = setup(&root, p);
    let keys = source_keys(&mut host, &reviewer, &capture, 1);
    host.observe_time(host.revision(), ElapsedTick(4)).unwrap();
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap_err(),
        JournalError::Contract(Error::Stale));
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().executions, 0);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(4)).unwrap();
    dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(5)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn source_loss_revokes_old_review_basis_without_refunding_the_original_reservation() {
    let root = Directory::new();
    let (mut host, reviewer, mut source, capture) = setup(&root, policy());
    let keys = source_keys(&mut host, &reviewer, &capture, 1);
    std::fs::remove_file(path(&root)).unwrap();
    let before = host.input_revision(1).unwrap();
    assert!(matches!(host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)),
        Err(FileSourceError::Read { withdrawal: None, .. })));
    assert!(host.file_source_status().unwrap().capture.closed.is_none());
    assert_eq!(host.input_revision(1).unwrap(), before + 1);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert!(host.propose(host.revision(), 2, spec(&host, b"cached"), snapshot()).is_err());
    write(&root, &capture);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    host.cancel(host.revision(), 1).unwrap();
    let fresh = source_keys(&mut host, &reviewer, &capture, 2);
    dispatch(&mut host, &fresh);
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&fresh.inputs), snapshot(), ElapsedTick(3)).unwrap().basis,
        PublicationBasis::Revalidated);
}

#[test]
fn durable_generation_and_semantic_floors_survive_fresh_readers_and_incomplete_newer_files() {
    let root = Directory::new();
    let (host, _, _, _) = setup(&root, policy());
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let status = host.file_source_status().unwrap();
    assert_eq!(status.producer.unwrap().generation, 4);
    assert!(status.capture.closed.is_none());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.propose(host.revision(), 1, spec(&host, b"saved"), snapshot()).is_err());
    for (candidate, error) in [
        (image(3, 1, true, b"older"), Error::Stale),
        (image(4, 1, true, b"substituted at same version"), Error::Binding),
        (image(5, 2, false, b"incomplete new image"), Error::Incomplete),
        (image(4, 1, true, b"versioned exact worker context"), Error::Stale),
        (image(6, 1, true, b"semantic rollback"), Error::Stale),
    ] {
        write(&root, &candidate);
        let mut fresh_reader = reader(&root);
        let revision = host.revision();
        assert_eq!(host.refresh_file_source(revision, &mut fresh_reader, ElapsedTick(2)).unwrap_err(),
            FileSourceError::Refused(error));
        assert_eq!(host.revision(), revision + 1);
        assert!(host.file_source_status().unwrap().capture.closed.is_none());
    }
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 5);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.file_source_status().unwrap().semantic_epoch, Some(2));
    write(&root, &image(6, 2, true, b"complete current image"));
    let mut source = reader(&root);
    let capture = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(3)).unwrap();
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 6);
    assert!(host.propose(host.revision(), 1, spec(&host, b"fresh"), capture.snapshot().clone()).is_ok());
}

#[test]
fn no_direct_input_call_can_substitute_contexts_from_another_source() {
    let root = Directory::new();
    let (mut host, _, _, capture) = setup(&root, policy());
    let action = host.propose(host.revision(), 1, spec(&host, b"same action"), snapshot()).unwrap();
    let forged = inputs(&action, b"unobserved helper bytes");
    let before = host.revision();
    assert_eq!(host.record_inputs(before, 1, 0, forged).unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(host.revision(), before);
    let actual = capture.inputs_for(&action, &profile().committee).unwrap();
    review_existing(&mut host, 1, 101, &actual);
    assert!(host.authorize(host.revision(), 1, &actual, snapshot()).is_ok());
}

#[test]
fn post_dispatch_source_change_seals_before_publication_but_only_receipts_release_charges() {
    for already_published in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer, mut source, capture) = setup(&root, policy());
        let keys = source_keys(&mut host, &reviewer, &capture, 1);
        dispatch(&mut host, &keys);
        if already_published {
            host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        }
        write(&root, &image(5, 1, true, b"new helper context"));
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, 16);
        let expected = if already_published { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } };
        assert_eq!(result.outcome, expected);
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().executions, u64::from(already_published));
        assert_eq!(host.inspect().control.ledger.charged, if already_published { 16 } else { 0 });
    }
}

#[test]
fn native_capture_exhaustion_is_persisted_and_cannot_be_reset_by_reopening_or_new_readers() {
    let root = Directory::new();
    let mut p = policy(); p.limits.events = 1;
    let (mut host, _, mut source, _) = setup(&root, p);
    let before = host.revision();
    assert_eq!(host.refresh_file_source(before, &mut source, ElapsedTick(2)).unwrap_err(), FileSourceError::Refused(Error::Limit));
    assert_eq!(host.revision(), before + 1);
    assert_eq!(host.file_source_status().unwrap().capture.fault, Some(Error::Limit));
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.file_source_status().unwrap().capture.fault, Some(Error::Limit));
    let mut fresh = reader(&root);
    assert!(host.refresh_file_source(host.revision(), &mut fresh, ElapsedTick(3)).is_err());
    assert_eq!(host.enable_file_source(host.revision(), policy()).unwrap_err(), JournalError::Contract(Error::Duplicate));
    assert!(host.propose(host.revision(), 1, spec(&host, b"no allowance reset"), snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn failed_source_storage_and_failed_withdrawal_return_no_current_capture_or_new_authority() {
    for source_missing in [false, true] {
        let root = Directory::new();
        let (mut host, _, mut source, _) = setup(&root, policy());
        if source_missing { std::fs::remove_file(path(&root)).unwrap(); }
        else { write(&root, &image(5, 1, true, b"new observation")); }
        std::fs::write(root.store().join("delivery.pending"), b"occupied staging").unwrap();
        let before = host.file_source_status().unwrap();
        let error = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap_err();
        if source_missing {
            assert!(matches!(error, FileSourceError::Read { withdrawal: Some(JournalError::Io(_)), .. }));
        } else { assert!(matches!(error, FileSourceError::Journal(JournalError::Io(_)))); }
        assert!(host.storage_failure().is_some());
        let mut after = host.file_source_status().unwrap();
        assert!(after.interrupted); after.interrupted = false;
        assert_eq!(after, before);
        assert!(host.propose(host.revision(), 1, spec(&host, b"unavailable"), snapshot()).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(host.file_source_status().unwrap().capture.closed.is_none());
        assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 4);
        write(&root, &image(5, 1, true, b"recovered source"));
        let mut fresh = reader(&root);
        let observed = host.refresh_file_source(host.revision(), &mut fresh, ElapsedTick(3)).unwrap();
        assert!(host.propose(host.revision(), 1, spec(&host, b"after recovery"), observed.snapshot().clone()).is_ok());
    }
}

#[test]
fn source_binding_is_preproposal_and_cannot_be_installed_as_a_late_weaker_fallback() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    host.propose(host.revision(), 1, spec(&host, b"legacy"), snapshot()).unwrap();
    assert_eq!(host.enable_file_source(host.revision(), policy()).unwrap_err(), JournalError::Contract(Error::WrongState));
    assert!(!host.file_source_required());
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let mut invalid = policy(); invalid.source.scope.authority += 1;
    assert_eq!(host.enable_file_source(host.revision(), invalid).unwrap_err(), JournalError::Contract(Error::Binding));
    assert!(!host.publication_guard_required());
    host.enable_file_source(host.revision(), policy()).unwrap();
    assert_eq!(host.enable_file_source(host.revision(), policy()).unwrap_err(), JournalError::Contract(Error::Duplicate));
}

#[test]
fn prewrite_capacity_failure_cannot_leave_old_grants_usable_and_recovery_must_withdraw_first() {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.bytes = 65_536;
    let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
    host.enable_file_source(host.revision(), policy()).unwrap();
    write(&root, &image(4, 1, true, b"small initial context"));
    let mut source = reader(&root);
    let captured = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let keys = source_keys(&mut host, &reviewer, &captured, 1);
    write(&root, &image(5, 1, true, &vec![7; 20_000]));
    let revision = host.revision();
    assert_eq!(host.refresh_file_source(revision, &mut source, ElapsedTick(2)).unwrap_err(),
        FileSourceError::Journal(JournalError::Contract(Error::Limit)));
    assert_eq!(host.revision(), revision);
    assert!(host.storage_failure().is_none());
    assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    write(&root, &image(5, 1, true, b"small new context"));
    let mut fresh_reader = reader(&root);
    let before = host.revision();
    let current = host.refresh_file_source(before, &mut fresh_reader, ElapsedTick(2)).unwrap();
    assert_eq!(host.revision(), before + 2, "withdrawal must commit before a new source observation");
    assert!(!host.file_source_status().unwrap().interrupted);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    host.cancel(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    let fresh_keys = source_keys(&mut host, &reviewer, &current, 2);
    dispatch(&mut host, &fresh_keys);
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&fresh_keys.inputs), snapshot(), ElapsedTick(3)).unwrap().basis,
        PublicationBasis::Revalidated);
}
