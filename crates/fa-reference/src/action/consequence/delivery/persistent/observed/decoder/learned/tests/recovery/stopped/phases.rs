//! Original sampled numerics, two-key effects, terminal reducers and Store barriers.
//! Fixed helper ballots and small model coefficients are not detector qualification.
use super::*;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, machine::Transition};
use std::{fs, panic::{AssertUnwindSafe, catch_unwind}};

fn build(root: &Directory, config: &FileLearnedConfig, mode: u64, events: usize) -> FileOversight {
    let mut p = profile(); p.delivery.limits.events = events;
    let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config.clone()).unwrap();
    step(&mut host).unwrap(); step(&mut host).unwrap();
    let (action, inputs, automatic, offer) = prepared(&mut host);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &offer).unwrap();
    if mode > 0 {
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    }
    if mode == 2 {
        host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    }
    // An unfinished numerical intent must survive both terminal phases verbatim.
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    host
}
fn open(root: &Directory, p: &FileOversightProfile, config: &FileLearnedConfig) -> FileLearnedRecovery {
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), p.clone(), config).unwrap();
    replay_all(&mut run, 1); run
}
fn bytes(root: &Directory) -> Vec<u8> { fs::read(root.store().join("delivery.bin")).unwrap() }

#[test]
fn learned_phases_preserve_numerics_and_match_original_terminal_bytes_for_all_liabilities() {
    for mode in 0..4 {
        let root = Directory::new(); let config = config(false, 1);
        let host = build(&root, &config, mode, 64); let p = host.profile.clone();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let pending = host.learned_generation_inspection().unwrap().pending;
        let actor = host.machine.broker.retained_actor_state();
        let state = (actor.tokens().to_vec(), actor.cache().to_vec(), actor.sampler().to_vec());
        let before = host.inspect();
        let stop = StopRequest { operation: 701, expected_control_sequence: before.control.sequence,
            expected_authority_epoch: before.control.ledger.epoch };
        let mut events = host.events.clone(); let mut oracle = Machine::replay(&p, &events).unwrap();
        for event in [BaseEvent::Stop(stop), BaseEvent::Fence] {
            let event = Event::Core(event); oracle.apply(&event).unwrap(); events.push(event);
        }
        let expected_stop = journal::encode(&p, host.store.identity(), &events).unwrap();
        let stopped_snapshot = oracle.snapshot(events.len());
        let tick = ElapsedTick(if mode == 3 { 2000 } else { 3 });
        let event = Event::Core(BaseEvent::StopProgress(tick));
        let Transition::StopProgressed(sweep) = oracle.apply(&event).unwrap() else { panic!("original sweep"); };
        events.push(event);
        let expected_drain = journal::encode(&p, host.store.identity(), &events).unwrap();
        let final_snapshot = oracle.snapshot(events.len()); drop(host);
        let stopped = open(&root, &p, &config).finish_admission_stop(stop).unwrap();
        assert_eq!(stopped.snapshot(), &stopped_snapshot);
        assert_eq!(stopped.appended_events(), 2); assert_eq!(bytes(&root), expected_stop);
        assert_eq!(stopped.progress().charged_units, if mode > 0 { 16 } else { 0 });
        let drained = open(&root, &p, &config).finish_stopped_drain(stop, || Ok(tick)).unwrap();
        assert_eq!(drained.stopped(), &FileStoppedRecovery::Advanced { snapshot: final_snapshot, sweep });
        assert_eq!(bytes(&root), expected_drain);
        assert_eq!(drained.stopped().progress().drained(), mode != 3);
        assert_eq!(drained.stopped().progress().charged_units, if mode >= 2 { 16 } else { 0 });
        let run = open(&root, &p, &config); let machine = run.verified_guarded_machine().unwrap();
        assert_eq!(machine.broker.hosted_learned_generation().unwrap(), numerical);
        assert_eq!(machine.pending_learned_step(), pending);
        assert_eq!(machine.broker.retained_actor_state().tokens(), state.0);
        assert_eq!(machine.broker.retained_actor_state().cache(), state.1);
        assert_eq!(machine.broker.retained_actor_state().sampler(), state.2);
        assert_eq!(drained.history_anchor(), &FileHistoryAnchor::from_acknowledged(
            expected_drain_revision(&run), expected_drain));
    }
}
fn expected_drain_revision(run: &FileLearnedRecovery) -> usize { run.progress().total_events }

#[test]
fn learned_two_phase_shutdown_uses_exact_reserve_and_completed_retries_are_clock_free() {
    let root = Directory::new(); let config = config(false, 1);
    let mut host = build(&root, &config, 1, 48); let p = host.profile.clone();
    while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    }
    let n = host.learned_generation_inspection().unwrap().numerical; drop(host);
    let run = open(&root, &p, &config); let stop = run.stop_request(701).unwrap();
    let stopped = run.finish_admission_stop(stop).unwrap(); assert_eq!(stopped.snapshot().revision, 47);
    let drained = open(&root, &p, &config).finish_stopped_drain(stop, || Ok(ElapsedTick(3))).unwrap();
    assert_eq!(drained.stopped().snapshot().revision, 48); assert!(drained.stopped().progress().drained());
    let canonical = bytes(&root);
    let run = open(&root, &p, &config); run.fail_once(JournalIo::Stage);
    assert_eq!(run.finish_admission_stop(stop).unwrap().appended_events(), 0);
    let run = open(&root, &p, &config); run.fail_once(JournalIo::Stage);
    let again = run.finish_stopped_drain(stop, || panic!("completed drain has no clock")).unwrap();
    assert_eq!(again.history_anchor(), drained.history_anchor());
    let run = open(&root, &p, &config); run.fail_once(JournalIo::Stage);
    assert_eq!(run.verified_guarded_machine().unwrap().broker.hosted_learned_generation().unwrap(), n);
    assert_eq!(run.try_finish_stopped_with_anchor(stop, || panic!("completed combined retry")).unwrap()
        .history_anchor(), drained.history_anchor());
    assert_eq!(bytes(&root), canonical);
}

#[test]
fn incomplete_or_failed_learned_replay_cannot_reach_a_terminal_consumer() {
    for failed in [false, true] { for drain in [false, true] {
        let root = Directory::new(); let config = config(false, 1);
        let host = build(&root, &config, 1, 64); let p = host.profile.clone();
        let state = host.inspect(); let stop = StopRequest { operation: 701,
            expected_control_sequence: state.control.sequence, expected_authority_epoch: state.control.ledger.epoch };
        let mut events = host.events.clone();
        if failed {
            let event = events.iter_mut().find(|event| matches!(event,
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { .. })))).unwrap();
            let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = event else { unreachable!() };
            let mut changed = witness.to_vec(); changed[0] ^= 1; *witness = changed.into();
            host.store.replace(&journal::encode(&p, host.store.identity(), &events).unwrap()).unwrap();
        }
        drop(host); let canonical = bytes(&root);
        let staged = root.store().join("delivery.pending"); fs::write(&staged, b"preserve").unwrap();
        let mut run = FileOversight::begin_open_with_learned_generation(root.store(), p, &config).unwrap();
        if failed { assert!(run.advance(0, events.len()).is_err()); }
        let error = if drain { run.finish_stopped_drain(stop, || panic!("not verified")).unwrap_err() }
            else { run.finish_admission_stop(stop).unwrap_err() };
        assert_eq!(error, JournalError::Contract(if failed { Error::Binding } else { Error::Incomplete }));
        assert_eq!(bytes(&root), canonical); assert_eq!(fs::read(staged).unwrap(), b"preserve");
    }}
}

#[test]
fn learned_active_drain_and_failed_or_panicking_clocks_leave_canonical_and_staging_untouched() {
    for drain in [false, true] { for panic_clock in [false, true] {
        let root = Directory::new(); let config = config(false, 1);
        let host = build(&root, &config, 1, 64); let p = host.profile.clone(); drop(host);
        let run = open(&root, &p, &config); let stop = run.stop_request(701).unwrap();
        if drain {
            let original = bytes(&root);
            assert_eq!(run.finish_stopped_drain(stop, || panic!("active domain")).unwrap_err(),
                JournalError::Contract(Error::WrongState));
            assert_eq!(bytes(&root), original);
            open(&root, &p, &config).finish_admission_stop(stop).unwrap();
        } else { drop(run); }
        let canonical = bytes(&root); let staged = root.store().join("delivery.pending");
        fs::write(&staged, b"preserve").unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let run = open(&root, &p, &config);
            let clock = || { if panic_clock { panic!("failed terminal clock"); } Err(JournalError::Unavailable) };
            if drain { run.finish_stopped_drain(stop, clock) }
            else { run.try_finish_stopped_with_anchor(stop, clock) }
        }));
        if panic_clock { assert!(result.is_err()); } else { assert_eq!(result.unwrap().unwrap_err(), JournalError::Unavailable); }
        assert_eq!(bytes(&root), canonical); assert_eq!(fs::read(staged).unwrap(), b"preserve");
        let run = open(&root, &p, &config);
        let history = if drain { run.finish_stopped_drain(stop, || Ok(ElapsedTick(3))) }
            else { run.try_finish_stopped_with_anchor(stop, || Ok(ElapsedTick(3))) }.unwrap();
        assert!(history.stopped().progress().drained());
    }}
}

#[test]
fn both_learned_terminal_phases_preserve_real_write_ambiguity_and_exact_retry_identity() {
    for drain in [false, true] { for barrier in [JournalIo::Stage, JournalIo::Write,
        JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let host = build(&root, &config, 2, 64); let p = host.profile.clone();
        let n = host.learned_generation_inspection().unwrap().numerical; drop(host);
        let run = open(&root, &p, &config); let stop = run.stop_request(701).unwrap();
        if drain { run.finish_admission_stop(stop).unwrap(); } else { drop(run); }
        let run = open(&root, &p, &config); let before = run.progress().total_events as u64;
        run.fail_once(barrier);
        let failure = if drain { run.finish_stopped_drain(stop, || Ok(ElapsedTick(3))).unwrap_err() }
            else { run.finish_admission_stop(stop).unwrap_err() };
        assert!(matches!(failure, JournalError::Io(ref e) if e.operation == barrier));
        let run = open(&root, &p, &config);
        assert_eq!(run.verified_guarded_machine().unwrap().broker.hosted_learned_generation().unwrap(), n);
        let visible = barrier == JournalIo::DirectorySync; let growth = if drain { 1 } else { 2 };
        assert_eq!(run.progress().total_events as u64, before + if visible { growth } else { 0 });
        if drain {
            let history = run.finish_stopped_drain(stop, || {
                assert!(!visible, "visible drained retry must not reacquire time"); Ok(ElapsedTick(3))
            }).unwrap();
            assert_eq!(history.stopped().snapshot().revision, before + 1);
            assert_eq!(history.stopped().progress().charged_units, 16);
        } else {
            let stopped = run.finish_admission_stop(stop).unwrap();
            assert_eq!(stopped.appended_events(), if visible { 0 } else { 2 });
            assert_eq!(stopped.snapshot().revision, before + 2);
            assert_eq!(stopped.progress().charged_units, 16);
        }
    }}
}
