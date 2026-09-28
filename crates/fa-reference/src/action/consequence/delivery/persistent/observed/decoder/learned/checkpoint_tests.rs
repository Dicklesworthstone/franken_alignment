//! Real original numerics and canonical files; synthetic weights are controls.
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;
use super::*;
use super::checkpoint::CheckpointEvent;
use crate::action::ElapsedTick;
use crate::action::consequence::gate::containment::MAX_CHECKPOINTS;

fn capture(host: &mut FileOversight, id: u64) -> checkpoint::FileLearnedCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), id, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}

#[test]
fn paired_learned_capture_is_reconstructed_without_importing_saved_numerical_state() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let before = host.learned_generation_inspection().unwrap().numerical;
    let actor = host.actor_snapshot().unwrap();
    let checkpoint = capture(&mut host, 1);
    assert_eq!(checkpoint.info().position, 1);
    assert_eq!(checkpoint.info().stream, 21);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(host.actor_snapshot().unwrap(), actor);
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
    assert!(host.learned_recovery_usage().unwrap().checkpoint_bytes > 0);
    host.check_learned_checkpoint(&checkpoint).unwrap();
    let replay = read_machine(&host, &config);
    assert_eq!(replay.learned_checkpoint_info(1).unwrap(), *checkpoint.info());
    let usage = host.learned_recovery_usage().unwrap();
    drop(host);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert!(host.check_learned_checkpoint(&checkpoint).is_err());
    let recovered = host.learned_checkpoint(1).unwrap();
    host.check_learned_checkpoint(&recovered).unwrap();
    assert_eq!(recovered.info(), checkpoint.info());
    assert!(host.human_status(1).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn capture_retries_preserve_the_original_cut_and_foreign_handles_refuse() {
    let root = Directory::new(); let other_root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut other, _) = owner(&other_root, &config);
    step(&mut host).unwrap(); step(&mut other).unwrap();
    let saved = capture(&mut host, 1); let foreign = capture(&mut other, 1);
    assert_eq!(saved.info(), foreign.info());
    assert_eq!(host.check_learned_checkpoint(&foreign).err(), Some(JournalError::Contract(Error::Binding)));
    let usage = host.learned_recovery_usage().unwrap();
    step(&mut host).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let again = host.capture_learned_checkpoint(0, 1, saved.info().actor_revision, saved.info().authority_epoch).unwrap();
    assert_eq!(again.info(), saved.info());
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
    assert_eq!(host.capture_learned_checkpoint(host.revision(), 1, before.numerical.actor_revision,
        before.numerical.actor_revision).err(), Some(JournalError::Contract(Error::Binding)));
    assert!(host.capture_learned_checkpoint(host.revision() - 1, 2, before.numerical.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
}

#[test]
fn checkpoint_cannot_abandon_pending_inference_or_upgrade_a_recovered_pause() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    assert!(host.capture_learned_checkpoint(host.revision(), 1,
        host.actor_snapshot().unwrap().actor_revision, 0).is_err());
    step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    assert_eq!(host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision, 0).err(),
        Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).err(), Some(JournalError::Contract(Error::Incomplete)));
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    capture(&mut host, 1);
}

#[test]
fn corrupt_checkpoint_witness_refuses_original_replay_even_with_valid_journal_framing() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); capture(&mut host, 1);
    let original = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &original).unwrap();
    bind_history(&mut events, &config).unwrap();
    Machine::replay(&host.profile, &events).unwrap();
    let index = events.iter().position(|event| matches!(event,
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(_))))).unwrap();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
        CheckpointEvent::Capture { witness, .. }))) = &events[index] else { unreachable!() };
    let expected = witness.to_vec();
    for offset in [0, 8, 32, expected.len() - 1] {
        let mut changed = events.clone(); let mut bytes = expected.clone(); bytes[offset] ^= 1;
        let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::Capture { witness, .. }))) = &mut changed[index] else { unreachable!() };
        *witness = bytes.into();
        let encoded = journal::encode(&host.profile, host.store.identity(), &changed).unwrap();
        let mut decoded = journal::decode(&host.profile, host.store.identity(), &encoded).unwrap();
        bind_history(&mut decoded, &config).unwrap();
        assert_eq!(Machine::replay(&host.profile, &decoded).err(), Some(Error::Binding));
    }
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), original);
}

#[test]
fn checkpoint_inventory_limit_refuses_without_recapturing_or_corrupting_the_source() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    for id in 1..=MAX_CHECKPOINTS as u64 { capture(&mut host, id); }
    let before = host.learned_generation_inspection().unwrap();
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.checkpoints, MAX_CHECKPOINTS);
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    assert_eq!(host.capture_learned_checkpoint(host.revision(), MAX_CHECKPOINTS as u64 + 1,
        before.numerical.actor_revision, host.inspect().control.ledger.epoch).err(),
        Some(JournalError::Contract(Error::Limit)));
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
    step(&mut host).unwrap();
}

#[test]
fn capturing_a_pair_keeps_original_two_key_and_final_publication_guards() {
    use crate::action::consequence::delivery::EndpointOutcome;
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host, 1);
    let (action, input, automatic, request) = prepared(&mut host);
    assert!(host.publish(host.revision(), 1).is_err());
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    let published = host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
    host.check_learned_checkpoint(&checkpoint).unwrap();
}

#[test]
fn capture_storage_failure_returns_no_handle_and_reopens_only_the_canonical_pair() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        let epoch = host.inspect().control.ledger.epoch;
        host.store.fail_once(barrier);
        let error = host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision, epoch).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected selected storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_checkpoint(1).err(), Some(JournalError::Unavailable));
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        if barrier == JournalIo::DirectorySync {
            assert_eq!(host.learned_checkpoint(1).unwrap().info().position, n.position);
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
        } else {
            assert_eq!(host.learned_checkpoint(1).err(), Some(JournalError::Contract(Error::Missing)));
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[path = "reset_tests.rs"]
mod reset;

#[path = "checkpoint_tests/pending_recovery.rs"]
mod pending_recovery;
