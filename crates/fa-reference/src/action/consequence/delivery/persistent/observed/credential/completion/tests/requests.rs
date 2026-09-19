//! Lost-acknowledgment recovery through durable external request identities.
use super::*;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;

const REQUEST: u64 = 700;
impl Ready {
    fn complete_request(&mut self, revision: u64, request: u64, observed: Snapshot,
        tick: u64, credentialed: bool) -> Result<EndpointOutcome, JournalError>
    {
        self.host.complete_request_publication(revision, request, CheckedCompletion {
            automatic: &self.automatic, human: &self.human, action: &self.action,
            current: &self.inputs, snapshot: observed, now: ElapsedTick(tick),
        }, credentialed.then_some(&self.credential))
    }
}

#[test]
fn external_request_completes_once_and_exact_terminal_retry_needs_no_new_observation() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    let revision = r.host.revision();
    let expected = EndpointOutcome::Executed { resulting_version: 2 };
    assert_eq!(r.complete_request(revision, REQUEST, snapshot(), 2, true), Ok(expected));
    assert_eq!(r.host.revision(), revision + 4);
    let completed = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(r.host.request_status(REQUEST).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Confirmed });
    // Deliberately stale revision, unavailable observation, elapsed deadline and
    // no live credential: this is a lookup of completed work, not new admission.
    assert_eq!(r.complete_request(0, REQUEST, Snapshot::default(), u64::MAX, false), Ok(expected));
    assert_eq!(r.host.inspect(), completed); assert_eq!(r.bytes(), bytes);
    assert_eq!(completed.executions, 1); assert_eq!(completed.control.ledger.charged, 16);
}

#[test]
fn request_identity_and_entire_action_bind_before_receipt_retry_or_dispatch() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    let mut spec = r.action.spec().clone(); spec.required_witnesses.clear();
    r.host.submit_request(r.host.revision(), REQUEST + 1, spec, snapshot()).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.complete_request(r.host.revision(), REQUEST + 1, snapshot(), 2, true),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(r.complete_request(r.host.revision(), 999, snapshot(), 2, true),
        Err(JournalError::Contract(Error::Missing)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    let expected = r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true).unwrap();
    let after = r.host.inspect(); let bytes = r.bytes();
    let mut changed = r.action.spec().clone(); changed.payload = b"different retry".to_vec();
    let changed = FrozenAction::freeze(changed).unwrap();
    assert_eq!(r.host.complete_request_publication(0, REQUEST, CheckedCompletion {
        automatic: &r.automatic, human: &r.human, action: &changed, current: &r.inputs,
        snapshot: Snapshot::default(), now: ElapsedTick(2),
    }, None), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(r.host.request_resolution(REQUEST + 1), Ok(None));
    assert_eq!(r.host.inspect(), after); assert_eq!(r.bytes(), bytes);
}

#[test]
fn request_wrapper_never_downgrades_mandatory_credentials() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, false),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert!(r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true).is_ok());
}

#[test]
fn later_credential_revocation_and_source_loss_cannot_erase_an_accepted_execution() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    let expected = r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true).unwrap();
    r.host.revoke_credential_guard(r.host.revision(), CredentialRevocationRequest {
        operation: 33, expected_generation: 1,
    }).unwrap();
    r.host.source_interrupted = true;
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(r.complete_request(0, REQUEST, Snapshot::default(), u64::MAX, true), Ok(expected));
    assert!(r.host.source_interrupted);
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
}

#[test]
fn retained_outcome_survives_restart_but_old_keys_never_become_live_again() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    let expected = r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true).unwrap();
    let Ready { host, automatic, human, action, inputs, credential, .. } = r; drop(host);
    let recovered = FileOversight::open_reconciled_publication(root.store(), profile(), ElapsedTick(3)).unwrap();
    let mut host = recovered.owner; let before = host.inspect();
    assert_eq!(host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(host.complete_request_publication(0, REQUEST, CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &inputs,
        snapshot: Snapshot::default(), now: ElapsedTick(3),
    }, Some(&credential)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    assert_eq!(before.executions, 1); assert_eq!(before.control.ledger.charged, 16);
}

#[test]
fn dispatch_without_accepted_receipt_never_becomes_a_completion_retry() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    r.host.dispatch(r.host.revision(), &r.automatic, &r.human, &r.action, &r.inputs, snapshot()).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert_eq!(r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true),
        Err(JournalError::Contract(Error::WrongState)));
    r.host.cancel_request(r.host.revision(), REQUEST).unwrap();
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    assert_eq!(before.control.ledger.charged, 16); assert_eq!(before.executions, 0);
    // The actual endpoint can subsequently establish nonexecution. Only its
    // original accepted receipt makes the durable request terminal and refunds.
    let outcomes = r.host.reconcile_publications_at(r.host.revision(), ElapsedTick(101)).unwrap();
    let Reconciliation::Resolved(expected @ EndpointOutcome::NotExecuted { .. }) = outcomes[&1].as_ref().unwrap() else {
        panic!("native expired-request nonexecution receipt");
    };
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(*expected)));
    let after = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(after.control.ledger.available, 100); assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(r.complete_request(0, REQUEST, Snapshot::default(), 0, false), Ok(*expected));
    assert_eq!(r.host.inspect(), after); assert_eq!(r.bytes(), bytes);
}

#[test]
fn endpoint_visible_publication_is_not_reported_as_accepted_until_reconciliation() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    r.host.dispatch(r.host.revision(), &r.automatic, &r.human, &r.action, &r.inputs, snapshot()).unwrap();
    let expected = r.host.publish_checked_with_credential(r.host.revision(), 1, Some(&r.inputs),
        snapshot(), ElapsedTick(2), &r.credential).unwrap().outcome;
    assert_eq!(r.host.inspect().executions, 1);
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert_eq!(r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true),
        Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(r.host.reconcile(r.host.revision(), 1), Ok(Reconciliation::Resolved(expected)));
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(r.host.inspect().executions, 1);
}

#[test]
fn expired_receipt_retention_preserves_uncertainty_whether_or_not_endpoint_executed() {
    for published in [false, true] {
        let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
        r.host.dispatch(r.host.revision(), &r.automatic, &r.human, &r.action, &r.inputs, snapshot()).unwrap();
        if published {
            r.host.publish_checked_with_credential(r.host.revision(), 1, Some(&r.inputs),
                snapshot(), ElapsedTick(2), &r.credential).unwrap();
        }
        drop(r);
        let recovered = FileOversight::open_reconciled_publication(root.store(), profile(), ElapsedTick(1002)).unwrap();
        assert_eq!(recovered.outcomes[&1], Ok(Reconciliation::RetentionExpired));
        assert_eq!(recovered.owner.request_resolution(REQUEST), Ok(None));
        let after = recovered.owner.inspect();
        assert_eq!(after.executions, u64::from(published));
        assert_eq!(after.control.ledger.available, 84); assert_eq!(after.control.ledger.charged, 16);
    }
}

#[test]
fn cancelled_and_refused_requests_have_no_fabricated_endpoint_receipt() {
    let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
    r.host.cancel_request(r.host.revision(), REQUEST).unwrap();
    assert_eq!(r.host.request_resolution(REQUEST), Ok(None));
    assert_eq!(r.host.request_status(REQUEST).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Cancelled });
    let mut foreign = r.action.spec().clone(); foreign.required_witnesses.clear();
    foreign.target.as_mut().unwrap().object += 1;
    let status = r.host.submit_request(r.host.revision(), REQUEST + 1, foreign, snapshot()).unwrap();
    assert!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)));
    assert_eq!(r.host.request_resolution(REQUEST + 1), Ok(None));
    assert_eq!(r.host.request_resolution(999), Err(JournalError::Contract(Error::Missing)));
    assert_eq!(r.host.inspect().control.ledger.available, 100);
    assert_eq!(r.host.inspect().executions, 0);
}

#[test]
fn foreign_live_keys_cannot_borrow_a_terminal_requests_receipt() {
    let a = Directory::new(); let b = Directory::new();
    let mut r = ready_request(&a, profile(), Some(REQUEST));
    let foreign = ready_request(&b, profile(), Some(REQUEST));
    let expected = r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    for (automatic, human) in [(&foreign.automatic, &r.human), (&r.automatic, &foreign.human)] {
        assert_eq!(r.host.complete_request_publication(0, REQUEST, CheckedCompletion {
            automatic, human, action: &r.action, current: &r.inputs,
            snapshot: Snapshot::default(), now: ElapsedTick(2),
        }, None), Err(JournalError::Contract(Error::Binding)));
    }
    assert_eq!(r.host.request_resolution(REQUEST), Ok(Some(expected)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
}

#[test]
fn ambiguous_acknowledgment_refuses_live_request_lookup_until_actual_cut_is_recovered() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut r = ready_request(&root, profile(), Some(REQUEST));
        r.host.store.fail_once(barrier);
        assert!(matches!(r.complete_request(r.host.revision(), REQUEST, snapshot(), 2, true), Err(JournalError::Io(_))));
        assert_eq!(r.host.request_resolution(REQUEST), Err(JournalError::Unavailable));
        assert_eq!(r.complete_request(0, REQUEST, Snapshot::default(), 2, false), Err(JournalError::Unavailable));
        drop(r);
        let recovered = FileOversight::open_reconciled_publication(root.store(), profile(), ElapsedTick(3)).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(recovered.owner.request_resolution(REQUEST),
            Ok(visible.then_some(EndpointOutcome::Executed { resulting_version: 2 })));
        let after = recovered.owner.inspect();
        assert_eq!(after.executions, u64::from(visible));
        assert_eq!(after.control.ledger.charged, if visible { 16 } else { 0 });
        assert_eq!(after.control.ledger.available, if visible { 84 } else { 100 });
    }
}
