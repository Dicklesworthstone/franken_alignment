//! Concrete acquisition through the original durable two-key publication owner.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureError, FilePublicationCapture, PublicationInputFile, MAX_CAPTURE_BYTES,
};
use fa_reference::{Error, Snapshot};

fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn each_authority_boundary_needs_a_new_read_even_when_the_file_is_unchanged() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    let before = host.inspect();
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect(), before);
    refresh(&mut host, &root, 1);
    dispatch(&mut host, &keys);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    refresh(&mut host, &root, 1);
    let published = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(published.basis, PublicationBasis::Revalidated);
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn a_dispatch_capture_cannot_be_reused_for_first_publication() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    refresh(&mut host, &root, 1);
    dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete));
    assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    refresh(&mut host, &root, 1);
    let retry = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
    assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(retry.outcome, sealed());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn live_file_drift_after_dispatch_preserves_unrelated_updates_and_rejects_phantoms() {
    for inserted in [99, 1, 3, 7] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        let keys = source_keys(&mut host, &reviewer, &root, 1);
        refresh(&mut host, &root, 1);
        dispatch(&mut host, &keys);
        let next = packet(1, &keys.action, &keys.inputs, 2, &[0, 2, 4, inserted]);
        replace_source(&root, &next);
        refresh(&mut host, &root, 1);
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        if inserted == 99 {
            assert_eq!(result.basis, PublicationBasis::Revalidated);
            assert_eq!(host.inspect().executions, 1);
        } else {
            assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
            assert_eq!(result.outcome, sealed());
            assert_eq!(host.inspect().executions, 0);
        }
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn failed_open_withdraws_old_inputs_without_spending_an_unspent_effect_key() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    refresh(&mut host, &root, 1);
    let revision = host.publication_input_revision(1).unwrap();
    std::fs::rename(root.0.join("witness-input.bin"), root.0.join("offline.bin")).unwrap();
    assert!(matches!(host.refresh_publication_from_file(host.revision(), 1, &source(&root)),
        Ok(Err(FileCaptureError::Io(_)))));
    assert_eq!(host.publication_input_revision(1).unwrap(), revision + 1);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    std::fs::rename(root.0.join("offline.bin"), root.0.join("witness-input.bin")).unwrap();
    refresh(&mut host, &root, 1);
    dispatch(&mut host, &keys);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn rollback_and_same_generation_equivocation_cannot_revive_an_old_quiet_capture() {
    for rollback in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        let keys = source_keys(&mut host, &reviewer, &root, 1);
        let newer = packet(1, &keys.action, &keys.inputs, 2, &[0, 2, 4, 99]);
        replace_source(&root, &newer);
        refresh(&mut host, &root, 1);
        let bad = packet(1, &keys.action, &keys.inputs, if rollback { 1 } else { 2 }, &[0, 2, 4]);
        replace_source(&root, &bad);
        assert_eq!(host.refresh_publication_from_file(host.revision(), 1, &source(&root)),
            Err(JournalError::Contract(if rollback { Error::Stale } else { Error::Binding })));
        assert!(host.storage_failure().is_some());
        assert_eq!(host.publication_source(1), Err(JournalError::Unavailable));
        assert_eq!(host.inspect().executions, 0);
        drop(reviewer); drop(host);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let retained = host.publication_source(1).unwrap().unwrap();
        assert_eq!(retained.generation, 2);
        assert!(!retained.fresh);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn source_packets_cannot_be_substituted_across_attempts_or_actions() {
    for wrong_attempt in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        let keys = source_keys(&mut host, &reviewer, &root, 1);
        let action = if wrong_attempt { keys.action.clone() } else {
            host.propose(host.revision(), 2, spec(&host, b"different"), snapshot()).unwrap()
        };
        let bad = packet(if wrong_attempt { 2 } else { 1 }, &action, &keys.inputs, 2, &[0, 2, 4]);
        replace_source(&root, &bad);
        assert_eq!(host.refresh_publication_from_file(host.revision(), 1, &source(&root)),
            Err(JournalError::Contract(Error::Binding)));
        assert!(host.storage_failure().is_some());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn cached_positive_inputs_cannot_bypass_a_source_bound_owner() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    let cached = observations(&keys.inputs, 1, &[0, 2, 4]);
    let before = host.inspect();
    assert_eq!(host.record_publication_inputs(host.revision(), 1, host.publication_input_revision(1).unwrap(), Some(cached)),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    assert!(host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
}

#[test]
fn original_bindings_and_execution_receipts_replay_without_any_source_file() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    let original = host.retained_publication_evidence(1).unwrap().clone();
    refresh(&mut host, &root, 1);
    dispatch(&mut host, &keys);
    refresh(&mut host, &root, 1);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    std::fs::rename(root.0.join("witness-input.bin"), root.0.join("offline.bin")).unwrap();
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open_with_publication_validation(root.store(), profile(), limits()).unwrap();
    assert_eq!(host.retained_publication_evidence(1).unwrap(), &original);
    assert_eq!(host.publication_source(1).unwrap().unwrap().source, SOURCE);
    let result = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(40)).unwrap();
    assert_eq!(result.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn reader_rejects_malformed_oversized_symlink_and_wrong_producer_files() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    let original = packet(1, &keys.action, &keys.inputs, 1, &[0, 2, 4]);
    let bytes = original.to_bytes().unwrap();
    assert_eq!(FilePublicationCapture::from_bytes(&bytes).unwrap(), original);
    for length in 0..bytes.len() { assert!(FilePublicationCapture::from_bytes(&bytes[..length]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(FilePublicationCapture::from_bytes(&trailing).is_err());
    let mut version = bytes.clone(); version[7] = b'2';
    assert_eq!(FilePublicationCapture::from_bytes(&version), Err(Error::Binding));
    let path = root.0.join("witness-input.bin");
    std::fs::write(&path, b"malformed").unwrap();
    assert!(source(&root).read_capture().is_err());
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_CAPTURE_BYTES as u64 + 1).unwrap(); drop(file);
    assert_eq!(source(&root).read_capture(), Err(FileCaptureError::Data(Error::Limit)));
    replace_source(&root, &original);
    let alias = root.0.join("alias.bin");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert_eq!(PublicationInputFile::new(alias, SOURCE).unwrap().read_capture(), Err(FileCaptureError::Data(Error::Binding)));
    assert_eq!(PublicationInputFile::new(path, SOURCE + 1).unwrap().read_capture(), Err(FileCaptureError::Data(Error::Binding)));
}
