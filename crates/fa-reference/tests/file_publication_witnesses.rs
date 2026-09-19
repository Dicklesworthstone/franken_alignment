//! Durable witness requirements through real congress, two-key and file paths.
//! These bounded fixtures do not establish adapter authenticity or crash proofs.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationEvidence, FilePublicationInputs, FileWitnessInput,
};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart, MAX_SUBMITTED_BYTES};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, SnapshotEntry, WitnessRequest};
use fa_reference::witness::refinement::RefinementBudget;
use fa_reference::{Error, Snapshot};

fn limits() -> PublicationLimits {
    PublicationLimits { bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 } }
}
fn create(root: &Directory, p: FileOversightProfile, limits: PublicationLimits) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_publication_validation(root.store(), p, limits).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn observations(inputs: &CommitteeInput, revision: u64, keys: &[u64], closed: bool) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 };
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    if closed {
        frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
        frontiers.record_close(marker).unwrap();
    }
    let structured = FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(marker)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationInputs::new(Some(structured), Some(inputs.views()["alpha"].actual_input().clone()))
}
fn evidence(inputs: &CommitteeInput) -> FilePublicationEvidence {
    FilePublicationEvidence::new(observations(inputs, 10, &[0, 2, 4], true), vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 }, WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 },
    ]).unwrap()
}
fn bind(host: &mut FileOversight, id: u64, inputs: &CommitteeInput) -> FilePublicationEvidence {
    let evidence = evidence(inputs);
    host.bind_publication_evidence(host.revision(), id, evidence.clone()).unwrap();
    host.record_publication_inputs(host.revision(), id, 0, Some(evidence.original().clone())).unwrap();
    evidence
}
fn ready(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64) -> Keys {
    let (action, inputs) = fixture::reviewed(host, id, b"visible");
    bind(host, id, &inputs);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now.0 + 30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn newer_unrelated_evidence_publishes_once_and_replays_the_same_actual_result() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root, profile(), limits());
    assert!(host.publication_guard_required());
    let keys = ready(&mut host, &reviewer, 1);
    let current = observations(&keys.inputs, 11, &[0, 2, 4, 99], true);
    host.record_publication_inputs(host.revision(), 1, 1, Some(current)).unwrap();
    fixture::dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn every_negative_dependency_is_rechecked_after_dispatch_and_rejection_is_terminal() {
    for keys_now in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 4, 7], vec![0, 2, 3, 4]] {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root, profile(), limits());
        let keys = ready(&mut host, &reviewer, 1);
        fixture::dispatch(&mut host, &keys);
        host.record_publication_inputs(host.revision(), 1, 1,
            Some(observations(&keys.inputs, 11, &keys_now, true))).unwrap();
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
        assert_eq!(result.outcome, sealed());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.record_publication_inputs(host.revision(), 1, 2,
            Some(observations(&keys.inputs, 12, &[0, 2, 4], true))).unwrap();
        let retry = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(retry.outcome, sealed());
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
    }
}

#[test]
fn missing_evidence_refuses_dispatch_without_spending_either_key_then_fresh_input_can_proceed() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root, profile(), limits());
    let keys = ready(&mut host, &reviewer, 1);
    host.record_publication_inputs(host.revision(), 1, 1, None).unwrap();
    let before = host.inspect();
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    host.record_publication_inputs(host.revision(), 1, 2,
        Some(observations(&keys.inputs, 11, &[0, 2, 4], true))).unwrap();
    fixture::dispatch(&mut host, &keys);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn frontier_loss_or_opaque_drift_after_dispatch_seals_even_with_unchanged_committee_inputs() {
    for opaque_drift in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root, profile(), limits());
        let keys = ready(&mut host, &reviewer, 1);
        fixture::dispatch(&mut host, &keys);
        let mut current = observations(&keys.inputs, 11, &[0, 2, 4], opaque_drift);
        if opaque_drift {
            let old = current.opaque().unwrap();
            let mut p = old.input_profile().clone(); p.model_epoch += 1;
            let changed = ActualHelperInput::new(old.submitted_bytes().to_vec(), p,
                old.ordered_parts().to_vec(), old.omissions().to_vec()).unwrap();
            current = FilePublicationInputs::new(current.structured().cloned(), Some(changed));
        }
        host.record_publication_inputs(host.revision(), 1, 1, Some(current)).unwrap();
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, PublicationBasis::Rejected(if opaque_drift { Error::Stale } else { Error::Incomplete }));
        assert_eq!(result.outcome, sealed());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generic_reopen_preserves_gate_and_original_binding_but_never_restores_sendable_authority() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root, profile(), limits());
    let keys = ready(&mut host, &reviewer, 1);
    let original = host.retained_publication_evidence(1).unwrap().clone();
    fixture::dispatch(&mut host, &keys);
    host.record_publication_inputs(host.revision(), 1, 1, None).unwrap();
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
    assert_eq!(host.retained_publication_evidence(1).unwrap(), &original);
    assert_eq!(host.publication_input_revision(1).unwrap(), 2);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).is_err());
    let (_, current) = fixture::reviewed(&mut host, 2, b"new attempt");
    assert_eq!(host.publication_input_revision(2).unwrap(), 0);
    assert!(matches!(host.authorize(host.revision(), 2, &current, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
    bind(&mut host, 2, &current);
    assert!(host.authorize(host.revision(), 2, &current, snapshot()).is_ok());
}

#[test]
fn pinned_open_rejects_downgrade_without_publishing_a_recovery_transition() {
    let root = Directory::new();
    let (host, reviewer) = create(&root, profile(), limits());
    let before = host.inspect();
    drop(reviewer); drop(host);
    let changed = PublicationLimits { validation: RefinementBudget { steps: 0, value_bytes: 0 }, ..limits() };
    assert!(matches!(FileOversight::open_with_publication_validation(root.store(), profile(), changed),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    let (host, _) = FileOversight::open_with_publication_validation(root.store(), profile(), limits()).unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
    assert!(host.revision() > before.revision);
}

#[test]
fn failed_current_input_update_cannot_fall_back_to_an_old_permitting_image() {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.bytes = 65_536;
    let (mut host, reviewer) = create(&root, p.clone(), limits());
    let keys = ready(&mut host, &reviewer, 1);
    fixture::dispatch(&mut host, &keys);
    let before = host.inspect();
    let original = evidence(&keys.inputs);
    let large = ActualHelperInput::new(vec![b'Q'; MAX_SUBMITTED_BYTES],
        original.original().opaque().unwrap().input_profile().clone(), vec![SubmittedPart {
            span: ByteSpan { start: 0, end: MAX_SUBMITTED_BYTES }, kind: PartKind::Question,
        }], vec![]).unwrap();
    let changed = FilePublicationInputs::new(original.original().structured().cloned(), Some(large));
    assert_eq!(host.record_publication_inputs(host.revision(), 1, 1, Some(changed)), Err(JournalError::Contract(Error::Limit)));
    assert!(host.storage_failure().is_some());
    assert_eq!(host.inspect(), before);
    assert_eq!(host.publication_validation(1), Err(JournalError::Unavailable));
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)), Err(JournalError::Unavailable));
    assert_eq!(FileOversight::read_publication(root.store(), &p).unwrap(), before);
    drop(reviewer); drop(host);
    let (mut recovered, _) = FileOversight::open_with_publication_validation(root.store(), p, limits()).unwrap();
    assert_eq!(recovered.inspect().executions, 0);
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert!(recovered.publish_checked(recovered.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).is_err());
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
}

#[test]
fn stale_writers_and_binding_replacement_do_not_advance_the_journal() {
    let root = Directory::new();
    let (mut host, _) = create(&root, profile(), limits());
    let (_, inputs) = fixture::reviewed(&mut host, 1, b"reviewed");
    let original = bind(&mut host, 1, &inputs);
    let before = host.inspect();
    let narrower = FilePublicationEvidence::new(original.original().clone(), vec![]).unwrap();
    assert_eq!(host.bind_publication_evidence(host.revision(), 1, narrower), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.record_publication_inputs(host.revision(), 1, 0, None), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    assert!(host.storage_failure().is_none());
    assert_eq!(host.retained_publication_evidence(1).unwrap(), &original);
    assert!(host.authorize(host.revision(), 1, &inputs, snapshot()).is_ok());
}

#[test]
fn executed_outcomes_survive_recovery_and_evidence_loss_without_a_refund() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root, profile(), limits());
    let keys = ready(&mut host, &reviewer, 1);
    fixture::dispatch(&mut host, &keys);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    host.record_publication_inputs(host.revision(), 1, 1, None).unwrap();
    host.inputs_unavailable(host.revision(), 1, host.input_revision(1).unwrap()).unwrap();
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open_with_publication_validation(root.store(), profile(), limits()).unwrap();
    let result = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(40)).unwrap();
    assert_eq!(result.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}
