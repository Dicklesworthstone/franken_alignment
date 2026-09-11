//! Actual local-file tests; injected errors name the barrier they precede.
//! Private envelopes isolate storage behavior. Public integration tests separately
//! exercise the real congress/permit construction path without forged messages.

use super::*;
use crate::action::{Purpose, VERSION};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-file-unit-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("test directory cleanup failed: {error}"); }
    }
}
fn limits() -> FilePublicationLimits { FilePublicationLimits { mutations: 64, bytes: 1_048_576 } }
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: 1, generation: 4 }
}
fn setup(limits: FilePublicationLimits) -> (Temp, PublicationEndpoint, FileEndpointRecovery) {
    let root = Temp::new();
    let (mut endpoint, key) = PublicationEndpoint::create_file_publication(root.0.join("endpoint"), target(),
        b"before".to_vec(), 200, 16, limits).unwrap();
    endpoint.attach(scope()).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    (root, endpoint, key)
}
fn message(endpoint: &PublicationEndpoint, id: u64) -> DispatchEnvelope {
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION, scope: scope(), target: Some(target()), payload: b"published".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 9,
    }).unwrap();
    DispatchEnvelope { binding: Rc::clone(&endpoint.binding), epoch: 0, attempt: id,
        request: PublicationRequest::from_action(&action), retained_until: ElapsedTick(200) }
}

#[test]
fn each_write_barrier_preserves_visible_outcome_and_requires_recovery() {
    for stage in [StorageStage::CreatePending, StorageStage::WritePending, StorageStage::SyncPending,
        StorageStage::Publish, StorageStage::SyncDirectory]
    {
        let (_root, mut endpoint, key) = setup(limits());
        let message = message(&endpoint, 1);
        let query = StatusQuery(message.clone());
        endpoint.file_store.as_mut().unwrap().fail_at = Some(stage);
        assert_eq!(endpoint.deliver(&message), Err(Error::Incomplete));
        let failure = endpoint.file_storage_status().unwrap().failure.unwrap();
        assert_eq!(failure.stage, stage);
        let visible = stage == StorageStage::SyncDirectory;
        assert_eq!(failure.publication_visible, visible);
        assert_eq!(endpoint.execution_count(), u64::from(visible));
        assert_eq!(endpoint.status(&query), Err(Error::Incomplete));
        assert_eq!(endpoint.seal_unexecuted(&query), Err(Error::Incomplete));
        let published = PublicationEndpoint::read_file_publication(key.directory()).unwrap();
        assert_eq!(published.execution_count, u64::from(visible));
        assert_eq!(published.payload, if visible { b"published".to_vec() } else { b"before".to_vec() });
        drop(endpoint);
        let mut recovered = key.reopen().unwrap();
        let state = recovered.file_storage_status().unwrap();
        assert!(state.failure.is_none());
        assert!(state.clock_confirmation_required);
        assert_eq!(recovered.status(&query), Err(Error::Incomplete));
        recovered.observe_time(ElapsedTick(1)).unwrap();
        let receipt = recovered.seal_unexecuted(&query).unwrap();
        assert_eq!(receipt.outcome(), if visible { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } });
        assert_eq!(recovered.deliver(&message).unwrap(), receipt);
        assert_eq!(recovered.execution_count(), u64::from(visible));
    }
}

#[test]
fn a_real_create_pending_failure_does_not_publish_or_accept_a_partial_file() {
    let (_root, mut endpoint, key) = setup(limits());
    let message = message(&endpoint, 1);
    let query = StatusQuery(message.clone());
    fs::write(key.directory().join(PENDING), b"unfinished unrelated staging bytes").unwrap();
    assert_eq!(endpoint.deliver(&message), Err(Error::Incomplete));
    assert_eq!(endpoint.file_storage_status().unwrap().failure.unwrap().stage, StorageStage::CreatePending);
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"before");
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    assert!(!key.directory().join(PENDING).exists());
    recovered.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(recovered.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    let sealed = recovered.seal_unexecuted(&query).unwrap();
    assert_eq!(recovered.deliver(&message).unwrap(), sealed);
    assert_eq!(recovered.execution_count(), 0);
}

#[test]
fn exclusive_owner_lock_is_released_by_drop_not_a_stale_lockfile_flag() {
    let (_root, endpoint, key) = setup(limits());
    assert!(matches!(key.reopen(), Err(FilePublicationError::LockUnavailable(_))));
    drop(endpoint);
    let recovered = key.reopen().unwrap();
    assert!(matches!(key.reopen(), Err(FilePublicationError::LockUnavailable(_))));
    assert_eq!(recovered.payload(), b"before");
}

#[test]
fn acknowledged_history_cannot_be_rolled_back_under_a_surviving_key() {
    let (_root, mut endpoint, key) = setup(limits());
    let before = fs::read(key.directory().join(STATE)).unwrap();
    let message = message(&endpoint, 1);
    endpoint.deliver(&message).unwrap();
    let current = fs::read(key.directory().join(STATE)).unwrap();
    drop(endpoint);
    fs::write(key.directory().join(STATE), &before).unwrap();
    assert!(matches!(key.reopen(), Err(FilePublicationError::Refused(Error::Stale))));
    // A read-only view has no independent anti-rollback anchor.
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"before");
    fs::write(key.directory().join(STATE), current).unwrap();
    assert_eq!(key.reopen().unwrap().execution_count(), 1);
}

#[test]
fn every_truncation_trailing_bytes_and_invalid_replay_order_refuse() {
    let (_root, mut endpoint, key) = setup(limits());
    endpoint.deliver(&message(&endpoint, 1)).unwrap();
    let bytes = fs::read(key.directory().join(STATE)).unwrap();
    for end in 0..bytes.len() { assert!(codec::decode_file(&bytes[..end], Rc::new(())).is_err(), "end={end}"); }
    let mut extra = bytes;
    extra.push(0);
    assert!(codec::decode_file(&extra, Rc::new(())).is_err());
    let initial = &endpoint.file_store.as_ref().unwrap().recovery.initial;
    for events in [
        vec![Operation::ObserveTime(ElapsedTick(2)), Operation::ObserveTime(ElapsedTick(1))],
        vec![Operation::Attach(scope()), Operation::Attach(scope())],
        vec![Operation::ObserveTime(ElapsedTick(2)), Operation::ObserveTime(ElapsedTick(2))],
    ] {
        let events: Vec<_> = events.iter().map(|event| codec::encode_operation(event).unwrap()).collect();
        let bytes = codec::encode_file(initial, &events, limits()).unwrap();
        assert!(codec::decode_file(&bytes, Rc::new(())).is_err());
    }
}

#[test]
fn mutation_quota_survives_recovery_but_does_not_charge_idempotent_receipts() {
    let (_root, mut endpoint, key) = setup(FilePublicationLimits { mutations: 3, ..limits() });
    let message = message(&endpoint, 1);
    let query = StatusQuery(message.clone());
    let receipt = endpoint.deliver(&message).unwrap();
    let before = fs::read(key.directory().join(STATE)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap(), receipt);
    assert_eq!(endpoint.seal_unexecuted(&query).unwrap(), receipt);
    assert_eq!(fs::read(key.directory().join(STATE)).unwrap(), before);
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(recovered.status(&query).unwrap(), EndpointStatus::Resolved(receipt));
    assert_eq!(recovered.observe_time(ElapsedTick(2)), Err(Error::Limit));
    assert!(recovered.file_storage_status().unwrap().clock_confirmation_required);
    assert_eq!(recovered.status(&query), Err(Error::Incomplete));
    assert_eq!(recovered.observe_time(ElapsedTick(1)), Err(Error::Stale));
    assert_eq!(fs::read(key.directory().join(STATE)).unwrap(), before);
}

#[test]
fn second_key_expiry_and_terminal_nonexecution_survive_serialization() {
    let (_root, mut endpoint, key) = setup(limits());
    let mut message = message(&endpoint, 1);
    let approval = DispatchApproval::new(7, 8, ElapsedTick(1), ElapsedTick(5)).unwrap();
    message.request.approval = Some(approval);
    endpoint.observe_time(ElapsedTick(5)).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(5)).unwrap();
    let recovered_receipt = recovered.deliver(&message).unwrap();
    assert_eq!(recovered_receipt, receipt);
    assert_eq!(recovered_receipt.request().approval(), Some(approval));
    assert_eq!(recovered.observe_time(ElapsedTick(4)), Err(Error::Stale));
    recovered.observe_time(ElapsedTick(200)).unwrap();
    assert_eq!(recovered.status(&StatusQuery(message)).unwrap(), EndpointStatus::RetentionExpired);
}

#[test]
fn existing_paths_and_symlinked_state_refuse_without_overwriting_targets() {
    let (_root, endpoint, key) = setup(limits());
    let before = fs::read(key.directory().join(STATE)).unwrap();
    assert!(PublicationEndpoint::create_file_publication(key.directory(), target(), Vec::new(), 10, 4, limits()).is_err());
    assert_eq!(fs::read(key.directory().join(STATE)).unwrap(), before);
    assert_eq!(fs::metadata(key.directory()).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(fs::metadata(key.directory().join(STATE)).unwrap().permissions().mode() & 0o077, 0);
    drop(endpoint);
    let original = key.directory().join("original.bin");
    fs::rename(key.directory().join(STATE), &original).unwrap();
    symlink(&original, key.directory().join(STATE)).unwrap();
    assert!(key.reopen().is_err());
    assert!(PublicationEndpoint::read_file_publication(key.directory()).is_err());
    assert_eq!(fs::read(original).unwrap(), before);
}

#[test]
fn a_failed_clock_write_cannot_be_forgotten_by_reopening_an_older_file() {
    let (_root, mut endpoint, key) = setup(limits());
    let mut message = message(&endpoint, 1);
    message.request.approval = Some(DispatchApproval::new(7, 8, ElapsedTick(1), ElapsedTick(10)).unwrap());
    endpoint.file_store.as_mut().unwrap().fail_at = Some(StorageStage::WritePending);
    assert_eq!(endpoint.observe_time(ElapsedTick(20)), Err(Error::Incomplete));
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().observed_time, Some(ElapsedTick(1)));
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    assert_eq!(recovered.observe_time(ElapsedTick(1)), Err(Error::Stale));
    assert_eq!(recovered.deliver(&message), Err(Error::Incomplete));
    recovered.observe_time(ElapsedTick(20)).unwrap();
    let receipt = recovered.deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(recovered.execution_count(), 0);
}
