//! Native Store barriers on the new sealed terminal consumer of original replay.
//! Original numerical steps, pending intent and endpoint history are retained;
//! helper ballots remain deterministic fixture controls, not model evaluation.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo,
    observed::shutdown::recovery::FileStoppedRecovery};
use std::cell::Cell;

#[test]
fn every_terminal_storage_barrier_returns_no_candidate_and_retry_uses_only_the_visible_original_cut() {
    for executed in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let config = config(false, 1);
            let (mut host, reviewer) = owner(&root, &config);
            step(&mut host).unwrap(); step(&mut host).unwrap();
            let (action, inputs, automatic, offer) = prepared(&mut host);
            let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &offer).unwrap();
            host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
            if executed {
                let receipt = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
                assert!(matches!(receipt.outcome, EndpointOutcome::Executed { .. }));
            }
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, 1);
            host.begin_learned_step(host.revision(), numerical.actor_revision, numerical.position).unwrap();
            let pending = host.learned_generation_inspection().unwrap().pending;
            assert!(pending.is_some());
            let actor = host.machine.broker.retained_actor_state();
            let tokens = actor.tokens().to_vec(); let cache = actor.cache().to_vec(); let sampler = actor.sampler().to_vec();
            let before = host.inspect(); assert_eq!(before.control.ledger.charged, 16);
            let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap(); drop(host);
            let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
            replay_all(&mut recovery, 1);
            let stop = recovery.stop_request(701).unwrap();
            assert_eq!(stop.expected_control_sequence, before.control.sequence);
            assert_eq!(stop.expected_authority_epoch, before.control.ledger.epoch);
            recovery.fail_once(barrier); let calls = Cell::new(0);
            let failure = recovery.finish_stopped(stop, || { calls.set(calls.get() + 1); ElapsedTick(3) }).unwrap_err();
            let JournalError::Io(failure) = failure else { panic!("selected original Store barrier"); };
            assert_eq!(failure.operation, barrier); assert_eq!(calls.get(), 1);
            assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
            // The existing deterministic Rename injection is before rename;
            // only the DirectorySync injection exposes the complete replacement.
            let visible = barrier == JournalIo::DirectorySync;
            let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
            assert_eq!(disk.revision, before.revision + if visible { 3 } else { 0 });
            assert_eq!(disk.executions, u64::from(executed));
            assert_eq!(disk.stop.is_some(), visible);
            if !visible {
                assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
                assert_eq!(disk, before);
            }
            let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
            replay_all(&mut recovery, 1); // Also proves the failed consumer released its original lock.
            let machine = recovery.verified_guarded_machine().unwrap();
            assert_eq!(machine.broker.hosted_learned_generation().unwrap(), numerical);
            assert_eq!(machine.pending_learned_step(), pending);
            assert_eq!(machine.broker.retained_actor_state().tokens(), tokens);
            assert_eq!(machine.broker.retained_actor_state().cache(), cache);
            assert_eq!(machine.broker.retained_actor_state().sampler(), sampler);
            assert_eq!(recovery.stop_request(701).unwrap(), stop);
            let recovered = recovery.finish_stopped(stop, || {
                assert!(!visible, "a visible completed stop cannot read time or repeat its terminal events");
                calls.set(calls.get() + 1); ElapsedTick(4)
            }).unwrap();
            assert_eq!(matches!(recovered, FileStoppedRecovery::AlreadyDrained { .. }), visible);
            assert_eq!(calls.get(), if visible { 1 } else { 2 });
            assert_eq!(recovered.snapshot().revision, before.revision + 3);
            assert!(recovered.progress().drained());
            assert_eq!(recovered.snapshot().executions, u64::from(executed));
            assert_eq!(recovered.progress().charged_units, if executed { 16 } else { 0 });
            assert_eq!(recovered.progress().reserved_units, 0);
            assert!(!root.store().join("delivery.pending").exists());
            let completed = std::fs::read(root.store().join("delivery.bin")).unwrap();
            let mut retry = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
            replay_all(&mut retry, 1);
            assert_eq!(retry.verified_guarded_machine().unwrap().broker.hosted_learned_generation().unwrap(), numerical);
            assert_eq!(retry.verified_guarded_machine().unwrap().pending_learned_step(), pending);
            retry.fail_once(JournalIo::Stage); // Any replacement, even identical bytes, must now fail.
            let again = retry.finish_stopped(stop, || panic!("completed retry never needs a clock")).unwrap();
            assert!(matches!(again, FileStoppedRecovery::AlreadyDrained { .. }));
            assert_eq!(again.snapshot(), recovered.snapshot()); assert_eq!(again.progress(), recovered.progress());
            assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), completed);
        }
    }
}
