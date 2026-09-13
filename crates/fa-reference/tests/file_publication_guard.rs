//! First publication rechecks original evidence; settlement stays separate.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::{Error, Snapshot};

fn guarded(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = create(root);
    host.enable_publication_guard(host.revision()).unwrap();
    (host, reviewer)
}
fn sealed() -> EndpointOutcome {
    EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }
}

#[test]
fn unchanged_evidence_publishes_once_but_only_reconciliation_acknowledges_it() {
    let root = Directory::new();
    let (mut host, reviewer) = guarded(&root);
    let keys = ready(&mut host, &reviewer, 1, b"visible");
    dispatch(&mut host, &keys);
    let before = host.inspect();
    assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect(), before);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Dispatching);
    let repeat = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(3)).unwrap();
    assert_eq!(repeat.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(repeat.outcome, result.outcome);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn source_drift_after_both_keys_were_spent_seals_instead_of_rebasing_permission() {
    for mode in 0..6 {
        let root = Directory::new();
        let (mut host, reviewer) = guarded(&root);
        let keys = ready(&mut host, &reviewer, 1, b"no longer valid");
        dispatch(&mut host, &keys);
        let mut state = snapshot();
        let changed = inputs(&keys.action, b"different whole helper input");
        let mut current = Some(&keys.inputs);
        match mode {
            0 => { state.values.insert(7, b"changed".to_vec()); }
            1 => state.semantic_epoch += 1,
            2 => state.complete = false,
            3 => current = Some(&changed),
            4 => current = None,
            _ => { host.inputs_unavailable(host.revision(), 1, host.input_revision(1).unwrap()).unwrap(); }
        }
        let result = host.publish_checked(host.revision(), 1, current, state, ElapsedTick(2)).unwrap();
        assert!(matches!(result.basis, PublicationBasis::Rejected(_)), "mode {mode}: {result:?}");
        assert_eq!(result.outcome, sealed());
        assert_eq!(host.inspect().payload, b"initial");
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Dispatching);
        let retry = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(retry.outcome, sealed());
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn unrelated_state_is_reusable_but_original_absence_and_range_witnesses_are_not() {
    for inserted in [99_u64, 8, 25] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.policy = Policy::new(1, vec![
            Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
            Predicate::Absent { key: 8 }, Predicate::EmptyRange { start: 20, end: 30 },
            Predicate::All(vec![0, 1, 2]),
        ]).unwrap();
        let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
        host.enable_publication_guard(host.revision()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let keys = ready(&mut host, &reviewer, 1, b"witness-bound");
        dispatch(&mut host, &keys);
        let mut state = snapshot(); state.values.insert(inserted, b"new row".to_vec());
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), state, ElapsedTick(2)).unwrap();
        if inserted == 99 {
            assert_eq!(result.basis, PublicationBasis::Revalidated);
            assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        } else {
            assert_eq!(result.basis, PublicationBasis::Rejected(Error::Binding));
            assert_eq!(result.outcome, sealed());
        }
        assert_eq!(host.inspect().executions, u64::from(inserted == 99));
    }
}

#[test]
fn a_new_control_decision_cannot_refresh_the_old_consumed_human_context() {
    let root = Directory::new();
    let (mut host, reviewer) = guarded(&root);
    let first = ready(&mut host, &reviewer, 1, b"old context");
    dispatch(&mut host, &first);
    let second = ready(&mut host, &reviewer, 2, b"new context");
    let result = host.publish_checked(host.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
    assert_eq!(result.outcome, sealed());
    host.reconcile(host.revision(), 1).unwrap();
    dispatch(&mut host, &second);
    let result = host.publish_checked(host.revision(), 2, Some(&second.inputs), snapshot(), ElapsedTick(3)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"new context");
}

#[test]
fn first_execution_expiry_uses_original_endpoint_deadline_evidence() {
    let root = Directory::new();
    let (mut host, reviewer) = guarded(&root);
    let keys = ready(&mut host, &reviewer, 1, b"too late");
    dispatch(&mut host, &keys);
    let expiry = keys.request.evidence().expires_at();
    let result = host.publish_checked(host.revision(), 1, None, Snapshot::default(), expiry).unwrap();
    assert_eq!(result.basis, PublicationBasis::DeadlineElapsed);
    assert_eq!(result.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn prior_execution_wins_after_reopen_source_loss_and_approval_expiry() {
    let root = Directory::new();
    let (mut host, reviewer) = guarded(&root);
    let keys = ready(&mut host, &reviewer, 1, b"already executed");
    dispatch(&mut host, &keys);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    host.inputs_unavailable(host.revision(), 1, host.input_revision(1).unwrap()).unwrap();
    drop(reviewer); drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(recovered.publication_guard_required());
    assert!(!recovered.clock_ready());
    let result = recovered.publish_checked(recovered.revision(), 1, None, Snapshot::default(), ElapsedTick(40)).unwrap();
    assert_eq!(result.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    recovered.reconcile(recovered.revision(), 1).unwrap();
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().payload, b"already executed");
}

#[test]
fn guard_installation_is_irreversible_and_cannot_upgrade_an_existing_proposal() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"legacy control");
    let before = host.inspect();
    assert_eq!(host.enable_publication_guard(host.revision()), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.inspect(), before);
    dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    let root = Directory::new();
    let (mut host, _) = guarded(&root);
    let before = host.inspect();
    assert_eq!(host.enable_publication_guard(host.revision()), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.inspect(), before);
    assert!(host.publish_checked(host.revision(), 99, None, snapshot(), ElapsedTick(2)).is_err());
    assert_eq!(host.inspect(), before);
}

#[test]
fn rejected_clock_is_preflight_but_recovery_never_reconstructs_a_sendable_envelope() {
    let root = Directory::new();
    let (mut host, reviewer) = guarded(&root);
    let keys = ready(&mut host, &reviewer, 1, b"not yet sent");
    dispatch(&mut host, &keys);
    let before = host.inspect();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(0)),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.publish_checked(recovered.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)),
        Err(JournalError::Contract(Error::Missing)));
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(recovered.seal_unexecuted(recovered.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert_eq!(recovered.inspect().executions, 0);
}

#[test]
fn a_failed_canonical_write_exposes_no_publication_or_speculative_refund() {
    for valid in [true, false] {
        let root = Directory::new();
        let (mut host, reviewer) = guarded(&root);
        let keys = ready(&mut host, &reviewer, 1, b"storage test");
        dispatch(&mut host, &keys);
        let before = host.inspect();
        std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
        let current = valid.then_some(&keys.inputs);
        assert!(matches!(host.publish_checked(host.revision(), 1, current, snapshot(), ElapsedTick(2)), Err(JournalError::Io(_))));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Unavailable));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        recovered.seal_unexecuted(recovered.revision(), 1).unwrap();
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
    }
}
