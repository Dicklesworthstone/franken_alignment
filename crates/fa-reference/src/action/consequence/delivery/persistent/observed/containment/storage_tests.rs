//! Reuse the original five storage barriers and full two-key fixture.
use super::*;
use crate::action::consequence::delivery::persistent::observed::containment::{FileResetRequest, FileStateUpdate};
use crate::action::consequence::gate::ReviewBinding;

fn changed(host: &FileOversight) -> FileStateUpdate {
    let actor = host.actor_snapshot().unwrap();
    FileStateUpdate { operation: 1, expected_actor_revision: actor.actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        state: ActorState::new(actor.state.profile(), vec![8, 9], vec![255, 0], vec![128, 1], 2).unwrap() }
}
fn reset_request(host: &FileOversight) -> FileResetRequest {
    FileResetRequest { operation: 1, expected_control_sequence: host.inspect().control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        binding: ReviewBinding { round: 10001, evidence_root: [42; 32], reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}

#[test]
fn interrupted_state_write_exposes_no_candidate_and_reopening_keeps_the_exact_canonical_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _) = create(&root);
        let before = host.actor_snapshot().unwrap(); let update = changed(&host);
        host.store.fail_once(barrier);
        check_failure(host.record_actor_state(host.revision(), update.clone()).unwrap_err(), barrier);
        assert_eq!(host.actor_snapshot(), Err(JournalError::Unavailable));
        assert_eq!(host.record_actor_state(0, update.clone()), Err(JournalError::Unavailable));
        let disk = canonical(&host); let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.broker.retained_actor_state(), if visible { &update.state } else { &before.state });
        assert_eq!(disk.broker.actor_revision(), u64::from(visible));
        drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(host.actor_snapshot().unwrap().actor_revision, u64::from(visible));
        let revision = host.revision();
        if visible {
            assert_eq!(host.record_actor_state(0, update).unwrap().actor_revision, 1);
        } else {
            // The missed operation belongs to the OLD epoch; never silently
            // rebase its write after the independent recovery fence.
            assert_eq!(host.record_actor_state(revision, update), Err(JournalError::Contract(Error::Stale)));
        }
        assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn unacknowledged_checkpoint_capture_is_recovered_only_if_its_original_event_became_visible() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _) = create(&root);
        host.store.fail_once(barrier);
        check_failure(host.capture_actor_checkpoint(host.revision(), 77, 0, 0).unwrap_err(), barrier);
        assert_eq!(host.actor_checkpoint(77).unwrap_err(), JournalError::Unavailable);
        let visible = barrier == JournalIo::DirectorySync;
        drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let revision = host.revision();
        if visible {
            let recovered = host.actor_checkpoint(77).unwrap();
            assert_eq!(recovered.info().actor_revision, 0);
            assert_eq!(host.capture_actor_checkpoint(0, 77, 0, 0).unwrap().info(), recovered.info());
        } else {
            assert_eq!(host.actor_checkpoint(77).unwrap_err(), JournalError::Contract(Error::Missing));
            assert_eq!(host.capture_actor_checkpoint(revision, 77, 0, 0).unwrap_err(), JournalError::Contract(Error::Stale));
        }
        assert_eq!(host.revision(), revision);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
    }
}

#[test]
fn reset_incident_memory_human_withdrawal_and_refund_publish_together_or_are_not_acknowledged() {
    for barrier in BARRIERS {
        for sent in [false, true] {
            let root = Directory::new(); let (mut host, reviewer) = create(&root);
            let initial = host.actor_snapshot().unwrap();
            let saved = host.capture_actor_checkpoint(host.revision(), 77, 0, 0).unwrap();
            let (action, inputs, automatic, request) = prepared(&mut host);
            let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
            if sent { host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap(); }
            let update = changed(&host); host.record_actor_state(host.revision(), update.clone()).unwrap();
            let request = reset_request(&host); let before = host.inspect();
            host.store.fail_once(barrier);
            check_failure(host.reset_actor(host.revision(), &saved, request.clone()).unwrap_err(), barrier);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.actor_snapshot(), Err(JournalError::Unavailable));
            assert_eq!(host.actor_reset_receipt(1).unwrap_err(), JournalError::Unavailable);
            let disk = canonical(&host); let visible = barrier == JournalIo::DirectorySync;
            assert_eq!(disk.broker.incident_count(), u64::from(visible));
            assert_eq!(disk.broker.retained_actor_state(), if visible { &initial.state } else { &update.state });
            assert_eq!(disk.broker.inspect().ledger.charged, if sent { 16 } else { 0 });
            assert_eq!(disk.broker.inspect().ledger.reserved, if !sent && !visible { 16 } else { 0 });
            assert_eq!(disk.broker.human_status(1001).unwrap().disposition,
                if sent { HumanDisposition::Consumed } else if visible { HumanDisposition::Revoked } else { HumanDisposition::Approved });
            drop(host);
            let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
            assert_eq!(host.actor_snapshot().unwrap().incident_count, u64::from(visible));
            let saved = host.actor_checkpoint(77).unwrap(); let revision = host.revision();
            if visible { assert_eq!(host.reset_actor(0, &saved, request).unwrap().incident_count, 1); }
            else { assert_eq!(host.reset_actor(revision, &saved, request), Err(JournalError::Contract(Error::Stale))); }
            assert_eq!(host.revision(), revision);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert_eq!(host.inspect().control.ledger.charged, if sent { 16 } else { 0 });
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            if sent {
                assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
                assert_eq!(host.inspect().control.ledger.charged, 16);
                host.seal_unexecuted(host.revision(), 1).unwrap();
            }
            assert_eq!(host.inspect().control.ledger.available, 100);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn interrupted_observation_does_not_block_reset_or_reopen_source_eligibility() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let saved = host.capture_actor_checkpoint(host.revision(), 77, 0, 0).unwrap();
    let update = changed(&host); host.record_actor_state(host.revision(), update).unwrap();
    // This is the actual fail-closed latch consumed by transact(), not a mock
    // predicate or a new source implementation. No successful observation is forged.
    host.source_interrupted = true;
    let request = reset_request(&host);
    let receipt = host.reset_actor(host.revision(), &saved, request).unwrap();
    assert!(receipt.restored); assert_eq!(receipt.incident_count, 1);
    assert!(host.source_interrupted);
    let actor = host.actor_snapshot().unwrap();
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"blocked".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 };
    assert_eq!(host.propose(host.revision(), 1, spec, snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(host.actor_snapshot().unwrap(), actor);
    assert_eq!(host.inspect().executions, 0);
}
