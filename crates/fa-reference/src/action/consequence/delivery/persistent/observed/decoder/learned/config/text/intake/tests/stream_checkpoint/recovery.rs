//! Recovery completes the original stream intent and Fence as one stored image.
//! Real decoder/audit work is replayed; injected storage failures are the original Store seams.
use super::*;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::{Event as BaseEvent,
    observed::decoder::learned::{FileLearnedRecovery, FileLearnedRecoveryStatus}};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}
fn ready(root: &Directory, config: &FileLearnedConfig) -> FileLearnedRecovery {
    ready_with_profile(root, stream_delivery_profile(), config)
}
fn ready_with_profile(root: &Directory, profile: FileOversightProfile,
    config: &FileLearnedConfig) -> FileLearnedRecovery
{
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), profile, config).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 3).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}
fn begin(host: &mut FileOversight, saved: &FileLearnedStreamCheckpoint,
    first_incident: u64, allowance: KvRestartBudget) -> FileLearnedResetIntent
{
    let expected = FileLearnedResetIntent::for_recovery(saved.id(), instruction(host, 900), allowance).unwrap();
    host.begin_learned_stream_reset(host.revision(), saved, expected.control().clone(),
        expected.budget(), first_incident).unwrap();
    let pending = host.pending_learned_reset().unwrap().unwrap();
    assert_eq!(pending.intent, expected); assert!(!pending.interrupted);
    expected
}
fn checkpoint_event(value: CheckpointEvent) -> Event {
    Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value)))
}

#[test]
fn recovery_storage_barriers_acknowledge_both_stream_completion_and_fence_or_neither() {
    for fault in std::iter::once(None).chain(BARRIERS.into_iter().map(Some)) {
        let root = Directory::new(); let config = stream_config();
        let (mut host, saved, first) = prepared(&root, &config);
        let spent = host.learned_generation_inspection().unwrap().numerical;
        let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
        let cut = host.revision(); let old = bytes(&host);
        let identity = host.store.identity().to_path_buf();
        let mut complete = events(&host, &config);
        let mut control = replay(&host, &config, &complete).unwrap();
        let completion = control.prepare_learned_stream_reset(900).unwrap();
        complete.push(checkpoint_event(completion));
        control.apply(&Event::Core(BaseEvent::Fence)).unwrap();
        complete.push(Event::Core(BaseEvent::Fence));
        let expected_snapshot = control.snapshot(complete.len());
        let expected_numerical = control.broker.hosted_learned_generation().unwrap();
        let expected_usage = control.broker.hosted_learned_recovery_usage().unwrap();
        let new = journal::encode(&host.profile, &identity, &complete).unwrap();
        assert_eq!(expected_numerical.cumulative_work, spent.cumulative_work);
        assert_eq!(expected_numerical.availability, LearnedAvailability::Empty);
        drop(host);
        let run = ready(&root, &config);
        assert_eq!(canonical(&root), old);
        if let Some(barrier) = fault { run.fail_once(barrier); }
        let result = run.finish_pending_stream_reset(&expected, first);
        if let Some(barrier) = fault {
            let JournalError::Io(failure) = result.unwrap_err() else {
                panic!("selected combined replacement storage fault expected");
            };
            assert_eq!(failure.operation, barrier);
            assert_eq!(failure.replacement_may_be_visible,
                matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        } else {
            let (host, _) = result.unwrap();
            assert_eq!(host.inspect(), expected_snapshot);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, expected_numerical);
            assert!(host.check_learned_stream_checkpoint(&saved).is_err());
            no_publication(&host);
        }
        // Rename is conservatively uncertain, but this injection precedes the
        // actual rename. Only DirectorySync has made the whole new image visible.
        let visible = fault.is_none() || fault == Some(JournalIo::DirectorySync);
        assert_eq!(canonical(&root), if visible { new.clone() } else { old.clone() });
        let mut observed = journal::decode(&stream_delivery_profile(), &identity, &canonical(&root)).unwrap();
        bind_history(&mut observed, &config).unwrap();
        let machine = Machine::replay(&stream_delivery_profile(), &observed).unwrap();
        assert_eq!(observed.len() as u64, cut + if visible { 2 } else { 0 });
        assert_eq!(machine.learned_reset_result_optional(900).unwrap().is_some(), visible);
        if visible {
            assert!(matches!(observed.last(), Some(Event::Core(BaseEvent::Fence))));
            assert!(matches!(&observed[observed.len() - 2], Event::Decoder(DecoderEvent::Learned(
                LearnedEvent::Checkpoint(CheckpointEvent::StreamReset { operation: 900, .. })))));
            assert!(!machine.clock_ready);
        } else {
            assert_eq!(machine.pending_learned_reset().unwrap().intent, expected);
            assert!(!machine.pending_learned_reset().unwrap().interrupted);
        }
        // The consumed failed candidate released its lock. Retry either does
        // the one outstanding reset or only fences an already completed reset.
        let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
        assert_eq!(host.revision(), cut + if visible { 3 } else { 2 });
        assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work,
            spent.cumulative_work);
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
        no_publication(&host);
    }
}

#[test]
fn a_changed_canonical_cut_is_preserved_before_and_after_private_stream_preparation() {
    for after_preparation in [false, true] {
        let root = Directory::new(); let config = stream_config();
        let (mut host, saved, first) = prepared(&root, &config);
        let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
        let before = bytes(&host); let identity = host.store.identity().to_path_buf();
        let mut successor = events(&host, &config);
        successor.push(Event::Core(BaseEvent::Time(ElapsedTick(2))));
        let successor = journal::encode(&host.profile, &identity, &successor).unwrap();
        drop(host);
        let run = ready(&root, &config);
        let run = if after_preparation {
            run.prepare_pending_stream_reset(&expected, first).unwrap()
        } else { run };
        assert_eq!(canonical(&root), before);
        std::fs::write(root.store().join("delivery.bin"), &successor).unwrap();
        let result = if after_preparation { run.finish() }
            else { run.finish_pending_stream_reset(&expected, first) };
        assert_eq!(result.err(), Some(Error::Binding.into()));
        assert_eq!(canonical(&root), successor);
        // The genuine Time successor retains the original control predecessor.
        let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        no_publication(&host);
    }
}

#[test]
fn a_stream_recovery_expectation_cannot_invent_change_or_complete_a_partial_intent() {
    let root = Directory::new(); let config = stream_config();
    let (host, saved, first) = prepared(&root, &config);
    let expected = FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 900), budget(&host)).unwrap();
    let no_intent = bytes(&host); drop(host);
    assert_eq!(ready(&root, &config).finish_pending_stream_reset(&expected, first).err(),
        Some(Error::Missing.into()));
    assert_eq!(canonical(&root), no_intent);
    // Partial replay cannot manufacture the absent intent or insert a Fence.
    let mut run = FileOversight::begin_open_with_learned_generation(
        root.store(), stream_delivery_profile(), &config).unwrap();
    assert_eq!(run.progress().replayed_events, 0);
    run.advance(0, 1).unwrap();
    assert_eq!(run.finish_pending_stream_reset(&expected, first).err(), Some(Error::Incomplete.into()));
    assert_eq!(canonical(&root), no_intent);
    let root = Directory::new();
    let (mut host, saved, first) = prepared(&root, &config);
    let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
    let before = bytes(&host); drop(host);
    let mut wrong = Vec::new();
    wrong.push(FileLearnedResetIntent::for_recovery(2, expected.control().clone(), expected.budget()).unwrap());
    for field in 0..5 {
        let mut control = expected.control().clone();
        match field {
            0 => control.operation += 1,
            1 => control.expected_control_sequence += 1,
            2 => control.expected_actor_revision += 1,
            3 => control.expected_authority_epoch += 1,
            _ => control.binding.evidence_root[0] ^= 1,
        }
        wrong.push(FileLearnedResetIntent::for_recovery(saved.id(), control, expected.budget()).unwrap());
    }
    let mut reduced = expected.budget(); reduced.audit.monitoring.refinements -= 1;
    wrong.push(FileLearnedResetIntent::for_recovery(saved.id(), expected.control().clone(), reduced).unwrap());
    for changed in wrong {
        assert_eq!(ready(&root, &config).finish_pending_stream_reset(&changed, first).err(),
            Some(Error::Binding.into()));
        assert_eq!(canonical(&root), before);
    }
    assert_eq!(ready(&root, &config).finish_pending_stream_reset(&expected, 0).err(),
        Some(Error::InvalidInput.into()));
    assert_eq!(ready(&root, &config).finish_pending_stream_reset(&expected, first + 1).err(),
        Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let early = FileOversight::begin_open_with_learned_generation(
        root.store(), stream_delivery_profile(), &config).unwrap();
    assert_eq!(early.finish_pending_stream_reset(&expected, first).err(), Some(Error::Incomplete.into()));
    assert_eq!(canonical(&root), before);
    let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    drop(host);
    // An exact completed expectation cannot bypass a DIFFERENT current pending
    // operation. A retry fence would otherwise interrupt that real instruction.
    let root = Directory::new();
    let (mut host, saved, first) = prepared(&root, &config);
    let allowance = budget(&host); let completed = begin(&mut host, &saved, first, allowance);
    let receipt = host.complete_learned_stream_reset(host.revision(), 900).unwrap().unwrap();
    assert!(receipt.control.restored);
    step(&mut host);
    let pending = FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 901), budget(&host)).unwrap();
    host.begin_learned_stream_reset(host.revision(), &saved, pending.control().clone(),
        pending.budget(), first).unwrap();
    let before = bytes(&host); drop(host);
    assert_eq!(ready(&root, &config).finish_pending_stream_reset(&completed, first).err(),
        Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let (host, _) = ready(&root, &config).finish_pending_stream_reset(&pending, first).unwrap();
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap().control, receipt.control);
    assert!(host.learned_reset_result(901).unwrap().unwrap().control.restored);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 2);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 2);
    assert!(host.pending_learned_reset().unwrap().is_none());
    no_publication(&host);
}

#[test]
fn legacy_and_stream_recovery_kinds_remain_disjoint_for_pending_and_completed_operations() {
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first) = prepared(&root, &config);
    let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
    let before = bytes(&host); drop(host);
    assert_eq!(ready(&root, &config).finish_pending_reset(&expected).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
    let completed = bytes(&host); drop(host);
    assert_eq!(ready(&root, &config).finish_pending_reset(&expected).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), completed);
    let root = Directory::new(); let config = super::super::config();
    let raw_profile = profile();
    let (mut host, _) = FileOversight::create_with_learned_text(
        root.store(), raw_profile.clone(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    let expected = FileLearnedResetIntent::for_recovery(1, instruction(&host, 900), budget(&host)).unwrap();
    host.begin_learned_reset(host.revision(), &saved, expected.control().clone(), expected.budget()).unwrap();
    let before = bytes(&host); drop(host);
    assert_eq!(ready_with_profile(&root, raw_profile.clone(), &config)
        .finish_pending_stream_reset(&expected, 1).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let (host, _) = ready_with_profile(&root, raw_profile.clone(), &config).finish_pending_reset(&expected).unwrap();
    let completed = bytes(&host); drop(host);
    assert_eq!(ready_with_profile(&root, raw_profile, &config)
        .finish_pending_stream_reset(&expected, 1).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), completed);
}

#[test]
fn a_pending_stream_reset_cannot_cross_stop_fence_or_default_recovery() {
    for cutoff in 0..3 {
        let root = Directory::new(); let config = stream_config();
        let (mut host, saved, first) = prepared(&root, &config);
        let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
        if cutoff == 0 { host.fence(host.revision()).unwrap(); }
        if cutoff == 1 {
            let control = host.inspect().control;
            host.request_stop(host.revision(), StopRequest { operation: 700,
                expected_control_sequence: control.sequence,
                expected_authority_epoch: control.ledger.epoch }).unwrap();
        }
        drop(host);
        if cutoff == 2 {
            let (host, _) = FileOversight::open_with_learned_text_stream(
                root.store(), stream_delivery_profile(), &config).unwrap();
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
        }
        let before = canonical(&root);
        assert_eq!(ready(&root, &config).finish_pending_stream_reset(&expected, first).err(),
            Some(Error::WrongState.into()));
        assert_eq!(canonical(&root), before);
    }
}

#[test]
fn pending_stream_recovery_needs_one_ordinary_slot_and_separate_fence_capacity() {
    for (reserved, slots, consume_last_ordinary) in [
        (false, 1, false), (false, 2, false), (true, 1, false), (true, 1, true),
    ] {
        let root = Directory::new(); let config = stream_config();
        let mut profile = stream_delivery_profile();
        let reserve = if reserved { RecoveryReserve::terminal().events } else { 0 };
        profile.delivery.limits.events = 10 + usize::from(reserved) + slots + reserve;
        let (mut host, _) = if reserved {
            FileOversight::create_with_learned_text_stream_with_reserve(root.store(),
                profile.clone(), config.clone(), RecoveryReserve::terminal())
        } else {
            FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), config.clone())
        }.unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        step(&mut host); let saved = capture(&mut host); step(&mut host);
        let first = frontier(&mut host, 800);
        let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
        assert_eq!(host.revision(), 10 + u64::from(reserved));
        if consume_last_ordinary { host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); }
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events,
            slots - usize::from(consume_last_ordinary));
        let cut = host.revision(); let before = bytes(&host); drop(host);
        let result = ready_with_profile(&root, profile.clone(), &config)
            .finish_pending_stream_reset(&expected, first);
        if consume_last_ordinary || (!reserved && slots == 1) {
            assert_eq!(result.err(), Some(Error::Limit.into()));
            assert_eq!(canonical(&root), before);
            let run = ready_with_profile(&root, profile, &config);
            assert_eq!(run.verified_guarded_machine().unwrap().broker
                .hosted_learned_recovery_usage().unwrap().restart_attempts, 0);
        } else {
            let (host, _) = result.unwrap();
            assert_eq!(host.revision(), cut + 2);
            assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
            assert_eq!(host.journal_capacity().unwrap().remaining().events,
                if reserved { reserve - 1 } else { 0 });
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
            no_publication(&host);
        }
    }
}

#[test]
fn stream_recovery_checks_actual_completion_bytes_before_spending_terminal_reserve() {
    for reserved in [false, true] {
        for short in [false, true] {
            let root = Directory::new(); let config = stream_config();
            let profile = stream_delivery_profile();
            let (mut host, _) = if reserved {
                FileOversight::create_with_learned_text_stream_with_reserve(root.store(),
                    profile.clone(), config.clone(), RecoveryReserve::terminal())
            } else {
                FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), config.clone())
            }.unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            step(&mut host); let saved = capture(&mut host); step(&mut host);
            let first = frontier(&mut host, 800);
            let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
            let identity = host.store.identity().to_path_buf();
            let pending = events(&host, &config);
            let mut machine = replay(&host, &config, &pending).unwrap();
            let completed = checkpoint_event(machine.prepare_learned_stream_reset(900).unwrap());
            let mut complete = pending.clone(); complete.push(completed);
            let completion_bytes = journal::encode(&profile, &identity, &complete).unwrap().len();
            complete.push(Event::Core(BaseEvent::Fence));
            let final_bytes = journal::encode(&profile, &identity, &complete).unwrap().len();
            assert!(final_bytes > completion_bytes);
            let mut exact = profile;
            exact.delivery.limits.bytes = if reserved {
                completion_bytes + RecoveryReserve::terminal().bytes
            } else { final_bytes } - usize::from(short);
            // The complete bootstrap is re-encoded under this independently
            // selected bound. No unrelated profile is used to open old bytes.
            let original = journal::encode(&exact, &identity, &pending).unwrap();
            drop(host);
            std::fs::write(root.store().join("delivery.bin"), &original).unwrap();
            let result = ready_with_profile(&root, exact.clone(), &config)
                .finish_pending_stream_reset(&expected, first);
            if short {
                assert_eq!(result.err(), Some(Error::Limit.into()));
                assert_eq!(canonical(&root), original);
                let run = ready_with_profile(&root, exact, &config);
                assert_eq!(run.verified_guarded_machine().unwrap().broker
                    .hosted_learned_recovery_usage().unwrap().restart_attempts, 0);
            } else {
                let (host, _) = result.unwrap();
                assert_eq!(bytes(&host), journal::encode(&exact, &identity, &complete).unwrap());
                let space = host.journal_capacity().unwrap();
                assert_eq!(space.ordinary_remaining().bytes, 0);
                assert_eq!(space.remaining().bytes, if reserved {
                    RecoveryReserve::terminal().bytes - (final_bytes - completion_bytes)
                } else { 0 });
                assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
                no_publication(&host);
            }
        }
    }
}

#[test]
fn an_admitted_failed_stream_audit_is_stored_once_and_recovery_never_refunds_it() {
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first) = prepared(&root, &config);
    let spent = host.learned_generation_inspection().unwrap().numerical.cumulative_work;
    let mut allowance = budget(&host); allowance.audit.monitoring.probe_coordinates = 0;
    let expected = begin(&mut host, &saved, first, allowance); drop(host);
    let (mut host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
    let failure = host.learned_reset_result(900).unwrap().unwrap_err();
    let usage = host.learned_recovery_usage().unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(usage.restart_attempts, 1); assert!(usage.reserved_cache_values > 0);
    assert_eq!(n.cumulative_work, spent); assert!(n.host_failure.is_some());
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    no_publication(&host);
    let cut = host.revision(); let incidents = host.actor_snapshot().unwrap().incident_count; drop(host);
    let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
    assert_eq!(host.revision(), cut + 1);
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap_err(), failure);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, incidents);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work, spent);
    no_publication(&host);
}

#[test]
fn a_missing_original_recipe_and_corrupt_reset_completion_never_create_recovery_authority() {
    for missing_recipe in [false, true] {
        let root = Directory::new(); let config = stream_config();
        let (mut host, saved, first) = prepared(&root, &config);
        let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
        let identity = host.store.identity().to_path_buf();
        let mut history = events(&host, &config);
        if missing_recipe {
            let index = history.iter().position(|event| matches!(event,
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(_))))).unwrap();
            history.remove(index);
        } else {
            let mut machine = replay(&host, &config, &history).unwrap();
            let mut completed = machine.prepare_learned_stream_reset(900).unwrap();
            let CheckpointEvent::StreamReset { witness, .. } = &mut completed else {
                panic!("original stream completion expected");
            };
            let mut corrupt = witness.to_vec(); let end = corrupt.len() - 1;
            corrupt[end] ^= 1; *witness = Rc::from(corrupt);
            history.push(checkpoint_event(completed));
            history.push(Event::Core(BaseEvent::Fence));
        }
        let bad = journal::encode(&host.profile, &identity, &history).unwrap(); drop(host);
        std::fs::write(root.store().join("delivery.bin"), &bad).unwrap();
        let opened = FileOversight::begin_open_with_learned_generation(
            root.store(), stream_delivery_profile(), &config);
        if missing_recipe {
            assert_eq!(opened.err(), Some(Error::Incomplete.into()));
        } else {
            let mut run = opened.unwrap();
            assert_eq!(run.advance(0, 4096).err(), Some(Error::Binding.into()));
            assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Failed(Error::Binding));
            assert_eq!(run.finish_pending_stream_reset(&expected, first).err(), Some(Error::Binding.into()));
        }
        assert_eq!(canonical(&root), bad);
    }
}

#[test]
fn unwinding_after_private_stream_preparation_drops_the_candidate_without_acknowledgment() {
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first) = prepared(&root, &config);
    let allowance = budget(&host); let expected = begin(&mut host, &saved, first, allowance);
    let before = bytes(&host); drop(host);
    // This exercises a caller unwind after actual original preparation, not an
    // injected panic inside the numerical audit or a bound on physical retries.
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let candidate = ready(&root, &config).prepare_pending_stream_reset(&expected, first).unwrap();
        assert_eq!(candidate.progress().status, FileLearnedRecoveryStatus::Ready);
        assert_eq!(canonical(&root), before);
        panic!("drop an unacknowledged original prepared stream reset");
    }));
    assert!(unwound.is_err()); assert_eq!(canonical(&root), before);
    let (host, _) = ready(&root, &config).finish_pending_stream_reset(&expected, first).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(!host.clock_ready()); no_publication(&host);
}
