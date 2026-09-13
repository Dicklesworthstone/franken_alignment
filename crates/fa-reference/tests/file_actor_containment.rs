//! Original containment semantics persisted with the full-input/two-key history.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::containment::*;
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, MAX_CHECKPOINTS};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::Error;

fn changed(host: &FileOversight, operation: u64) -> FileStateUpdate {
    let state = host.actor_snapshot().unwrap();
    FileStateUpdate { operation, expected_actor_revision: state.actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        state: ActorState::new(state.state.profile(), vec![8, 9, u32::MAX], vec![0, 128, 255], vec![42, 0, 255], 3).unwrap() }
}
fn reset_request(host: &FileOversight, operation: u64) -> FileResetRequest {
    FileResetRequest { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        binding: ReviewBinding { round: 10000 + operation, evidence_root: ROOT, reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}
fn checkpoint(host: &mut FileOversight) -> FileCheckpoint {
    host.capture_actor_checkpoint(host.revision(), 77, host.actor_snapshot().unwrap().actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}

#[test]
fn original_state_checkpoint_and_reset_survive_all_owner_loss_and_exact_retries() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let initial = host.actor_snapshot().unwrap();
    let saved = checkpoint(&mut host);
    let update = changed(&host, 1);
    let stored = host.record_actor_state(host.revision(), update.clone()).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().state, update.state);
    let request = reset_request(&host, 1);
    let receipt = host.reset_actor(host.revision(), &saved, request.clone()).unwrap();
    assert!(receipt.restored); assert_eq!(receipt.incident_count, 1);
    assert_eq!(host.actor_snapshot().unwrap().state, initial.state);
    let before = host.inspect();
    assert_eq!(host.record_actor_state(0, update.clone()).unwrap(), stored);
    assert_eq!(host.capture_actor_checkpoint(0, 77, 0, 0).unwrap().info(), saved.info());
    assert_eq!(host.reset_actor(0, &saved, request.clone()).unwrap(), receipt);
    assert_eq!(host.inspect(), before);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let recovered = host.actor_snapshot().unwrap();
    assert_eq!(recovered.state, initial.state); assert_eq!(recovered.incident_count, 1);
    assert_eq!(recovered.actor_revision, receipt.actor_revision);
    assert!(!host.clock_ready());
    assert_eq!(host.reset_actor(0, &saved, request.clone()), Err(JournalError::Contract(Error::Binding)));
    let saved = host.actor_checkpoint(77).unwrap(); let before = host.inspect();
    assert_eq!(host.reset_actor(0, &saved, request).unwrap(), receipt);
    assert_eq!(host.actor_reset_receipt(1).unwrap(), &receipt);
    assert_eq!(host.record_actor_state(0, update).unwrap(), stored);
    assert_eq!(host.inspect(), before);
}

#[test]
fn reset_releases_only_undispatched_reservations_and_never_reissues_either_key() {
    for phase in 0..3 {
        let root = Directory::new(); let (mut host, reviewer) = create(&root);
        let saved = checkpoint(&mut host);
        let keys = ready(&mut host, &reviewer, 1, b"before reset");
        if phase > 0 { dispatch(&mut host, &keys); }
        if phase == 2 { host.publish(host.revision(), 1).unwrap(); }
        let update = changed(&host, 1); host.record_actor_state(host.revision(), update).unwrap();
        let request = reset_request(&host, 1);
        let receipt = host.reset_actor(host.revision(), &saved, request).unwrap();
        assert_eq!(receipt.refunded_units, if phase == 0 { 16 } else { 0 });
        let ledger = host.inspect().control.ledger;
        assert_eq!(ledger.reserved, 0); assert_eq!(ledger.charged, if phase == 0 { 0 } else { 16 });
        assert_eq!(ledger.stages[&1], if phase == 0 { ActionState::Cancelled } else { ActionState::Dispatching });
        assert_eq!(host.human_status(1001).unwrap().disposition,
            if phase == 0 { HumanDisposition::Revoked } else { HumanDisposition::Consumed });
        assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        drop(keys); drop(reviewer); drop(host);
        let (mut host, reviewer) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if phase == 1 {
            assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), 1).unwrap();
        } else if phase == 2 {
            assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
        let keys = ready(&mut host, &reviewer, 2, b"fresh reset");
        dispatch(&mut host, &keys); host.publish(host.revision(), 2).unwrap();
        host.reconcile(host.revision(), 2).unwrap();
        assert_eq!(host.inspect().payload, b"fresh reset");
        assert_eq!(host.inspect().executions, if phase == 2 { 2 } else { 1 });
        let ledger = host.inspect().control.ledger;
        assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
    }
}

#[test]
fn incident_escalation_and_target_narrowing_do_not_rewind_on_reopening() {
    let root = Directory::new(); let (mut host, _) = create(&root); checkpoint(&mut host);
    let initial = host.actor_snapshot().unwrap().state;
    for incident in 1..=3 {
        let saved = host.actor_checkpoint(77).unwrap();
        let update = changed(&host, incident);
        host.record_actor_state(host.revision(), update.clone()).unwrap();
        let mut request = reset_request(&host, incident);
        if incident == 2 { request.retained_targets.clear(); }
        let receipt = host.reset_actor(host.revision(), &saved, request.clone()).unwrap();
        assert_eq!(receipt.incident_count, incident);
        assert_eq!(receipt.restored, incident < 3);
        assert_eq!(host.actor_snapshot().unwrap().state, if incident < 3 { initial.clone() } else { update.state });
        if incident >= 2 { assert!(!receipt.ceiling.contains(profile().delivery.target)); }
        let floor = receipt.revocation_floor;
        drop(host);
        (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(host.actor_snapshot().unwrap().incident_count, incident);
        assert!(host.inspect().control.ledger.epoch > floor);
        assert_eq!(host.inspect().control.suspended, incident == 3);
        let saved = host.actor_checkpoint(77).unwrap(); let revision = host.revision();
        assert_eq!(host.reset_actor(0, &saved, request).unwrap(), receipt);
        assert_eq!(host.revision(), revision);
    }
    assert!(host.record_actor_state(host.revision(), changed(&host, 9)).is_err());
    assert!(host.capture_actor_checkpoint(host.revision(), 78, host.actor_snapshot().unwrap().actor_revision,
        host.inspect().control.ledger.epoch).is_err());
}

#[test]
fn stale_epochs_foreign_checkpoints_and_conflicting_operations_preserve_current_state() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let saved = checkpoint(&mut host); let pending = changed(&host, 1);
    let other = Directory::new(); let (mut foreign, _) = create(&other); let foreign_checkpoint = checkpoint(&mut foreign);
    let before = host.inspect();
    assert_eq!(host.reset_actor(host.revision(), &foreign_checkpoint, reset_request(&host, 1)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let before = host.inspect();
    assert_eq!(host.record_actor_state(host.revision(), pending), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    let update = changed(&host, 1); host.record_actor_state(host.revision(), update.clone()).unwrap();
    let mut conflict = update; conflict.expected_authority_epoch += 1;
    assert_eq!(host.record_actor_state(0, conflict), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.reset_actor(host.revision(), &saved, reset_request(&host, 1)), Err(JournalError::Contract(Error::Binding)));
    let saved = host.actor_checkpoint(77).unwrap(); let request = reset_request(&host, 1);
    host.reset_actor(host.revision(), &saved, request.clone()).unwrap();
    let mut conflict = request; conflict.binding.evidence_root[0] ^= 1;
    let before = host.inspect();
    assert_eq!(host.reset_actor(0, &saved, conflict), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
}

#[test]
fn original_restart_qualification_and_terminal_stop_cannot_be_bypassed() {
    let root = Directory::new(); let mut p = profile();
    let original = p.delivery.actor.clone(); let mut grade = original.profile(); grade.grade = RestartGrade::AuditOnly;
    p.delivery.actor = ActorState::new(grade, original.tokens().to_vec(), original.cache().to_vec(), original.sampler().to_vec(), original.next_position()).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    assert_eq!(host.capture_actor_checkpoint(0, 1, 0, 0).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(host.revision(), 0);
    let root = Directory::new(); let (mut host, _) = create(&root); let saved = checkpoint(&mut host);
    let mut update = changed(&host, 1); let mut profile = update.state.profile(); profile.model_generation += 1;
    update.state = ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap();
    assert_eq!(host.record_actor_state(host.revision(), update), Err(JournalError::Contract(Error::Binding)));
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 1, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }).unwrap();
    assert!(host.reset_actor(host.revision(), &saved, reset_request(&host, 1)).is_err());
    assert!(host.record_actor_state(host.revision(), changed(&host, 2)).is_err());
    assert!(host.inspect().control.suspended);
}

#[test]
fn retained_update_and_native_checkpoint_limits_survive_recovery_without_eviction() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let first = changed(&host, 1);
    for id in 1..=MAX_FILE_STATE_UPDATES as u64 {
        let update = changed(&host, id); host.record_actor_state(host.revision(), update).unwrap();
    }
    assert_eq!(host.record_actor_state(host.revision(), changed(&host, 999)).unwrap_err(), JournalError::Contract(Error::Limit));
    let snapshot = host.actor_snapshot().unwrap();
    for id in 1..=MAX_CHECKPOINTS as u64 {
        host.capture_actor_checkpoint(host.revision(), id, snapshot.actor_revision, 0).unwrap();
    }
    assert_eq!(host.capture_actor_checkpoint(host.revision(), 999, snapshot.actor_revision, 0).unwrap_err(), JournalError::Contract(Error::Limit));
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let revision = host.revision();
    assert_eq!(host.record_actor_state(0, first).unwrap().actor_revision, 1);
    host.capture_actor_checkpoint(0, 1, snapshot.actor_revision, 0).unwrap();
    assert_eq!(host.revision(), revision);
    assert_eq!(host.actor_snapshot().unwrap(), snapshot);
    assert_eq!(host.record_actor_state(host.revision(), changed(&host, 999)).unwrap_err(), JournalError::Contract(Error::Limit));
}
