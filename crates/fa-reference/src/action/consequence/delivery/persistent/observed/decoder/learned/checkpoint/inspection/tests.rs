//! Real original replay and filesystem cuts; no supplied reset receipts.
use super::*;
use super::read_fixture::*;
use super::super::{FileLearnedConfig, FileOversightProfile, JournalIo, storage};
use super::super::super::super::{BaseEvent, containment::FileResetRequest};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
    decoder::monitoring::restart::KvRestartBudget};
use crate::action::consequence::gate::ReviewBinding;
use std::path::Path;

fn capture(host: &mut FileOversight) -> FileLearnedCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
fn instruction(host: &FileOversight, operation: u64) -> FileResetRequest {
    let state = host.inspect().control;
    FileResetRequest { operation, expected_control_sequence: state.sequence,
        expected_actor_revision: host.learned_generation_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round: operation, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target] }
}
fn budget() -> KvRestartBudget {
    // The fixture constructs this original policy; no configuration importer or
    // nonexistent public mutable/configuration accessor is used.
    let original = source(&model(), false, 1);
    KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: original.policy.allowance() }
}
fn bytes(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join(storage::CANONICAL)).unwrap() }
fn staged(root: &Directory) -> Option<Vec<u8>> {
    match std::fs::read(root.store().join("delivery.pending")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("reading actual staged bytes: {error}"),
    }
}
fn read(root: &Directory, config: &FileLearnedConfig, operation: u64) -> FileLearnedResetSnapshot {
    FileOversight::read_learned_reset_result(root.store(), &profile(), config, operation).unwrap()
}

#[test]
fn reads_original_completed_reset_while_writer_lock_and_live_state_are_unchanged() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host); step(&mut host).unwrap();
    assert!(matches!(read(&root, &config, 900).record(), FileLearnedResetRecord::NotRecorded));
    let control = instruction(&host, 900); let allowance = budget();
    let expected = host.reset_learned_checkpoint(host.revision(), &checkpoint, control.clone(), allowance).unwrap().unwrap();
    let before = bytes(&root); let n = host.learned_generation_inspection().unwrap();
    let rights = host.inspect(); let usage = host.learned_recovery_usage().unwrap();
    let observed = read(&root, &config, 900);
    assert_eq!(observed.operation(), 900); assert_eq!(observed.journal_revision(), host.revision());
    let FileLearnedResetRecord::Completed { intent, result: Ok(actual) } = observed.record() else {
        panic!("missing original completion: {observed:?}");
    };
    assert_eq!(intent.control(), &control); assert_eq!(intent.checkpoint(), checkpoint.id());
    assert_eq!(intent.budget(), allowance); assert!(actual.control.restored);
    assert_eq!(actual.control.incident_count, expected.control.incident_count);
    assert_eq!(actual.control.sequence, expected.control.sequence);
    assert_eq!(actual.control.revocation_floor, expected.control.revocation_floor);
    assert_eq!(actual.position, expected.position); assert_eq!(actual.sampled_draws, expected.sampled_draws);
    assert_eq!(actual.restart.historical_work(), expected.restart.historical_work());
    assert_eq!(actual.restart.historical_telemetry(), expected.restart.historical_telemetry());
    assert_eq!(actual.restart.kv().restoration().source, expected.restart.kv().restoration().source);
    assert_eq!(host.learned_generation_inspection().unwrap(), n); assert_eq!(host.inspect(), rights);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage); assert_eq!(bytes(&root), before);
    assert_eq!(read_machine(&host, &config).broker.hosted_learned_generation().unwrap(), n.numerical);
    host.complete_learned_reset(0, 900).unwrap().unwrap();
    assert_eq!(bytes(&root), before);
}

#[test]
fn pending_read_never_completes_or_interrupts_original_intent_and_later_fence_is_visible() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host);
    let control = instruction(&host, 900);
    host.begin_learned_reset(host.revision(), &checkpoint, control.clone(), budget()).unwrap();
    let before = bytes(&root); let usage = host.learned_recovery_usage().unwrap();
    let intent_revision = host.revision();
    let observed = read(&root, &config, 900);
    let FileLearnedResetRecord::Pending(pending) = observed.record() else { panic!("{observed:?}"); };
    assert_eq!(pending.intent.control(), &control); assert!(!pending.interrupted);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage); assert_eq!(bytes(&root), before);
    assert!(matches!(read(&root, &config, 901).record(), FileLearnedResetRecord::NotRecorded));
    drop(host);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let fenced = bytes(&root);
    let observed = read(&root, &config, 900);
    let FileLearnedResetRecord::Pending(pending) = observed.record() else { panic!("{observed:?}"); };
    assert!(pending.interrupted); assert_eq!(pending.intent.control(), &control);
    assert_eq!(observed.journal_revision(), intent_revision + 1);
    assert_eq!(observed.journal_revision(), host.revision()); assert_eq!(bytes(&root), fenced);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
}

#[test]
fn all_completion_storage_faults_are_inspectable_without_cleaning_or_promoting_staging() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let checkpoint = capture(&mut host);
        let control = instruction(&host, 900);
        host.begin_learned_reset(host.revision(), &checkpoint, control, budget()).unwrap();
        let intent_revision = host.revision();
        host.store.fail_once(barrier);
        let error = host.complete_learned_reset(host.revision(), 900).unwrap_err();
        let JournalError::Io(failure) = error else {
            panic!("expected the selected storage barrier");
        };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_reset_result(900).err(), Some(JournalError::Unavailable));
        let before = bytes(&root); let pending_bytes = staged(&root);
        let observed = read(&root, &config, 900);
        if barrier == JournalIo::DirectorySync {
            assert_eq!(observed.journal_revision(), intent_revision + 1);
            assert!(matches!(observed.record(), FileLearnedResetRecord::Completed { result: Ok(r), .. }
                if r.control.restored && r.control.incident_count == 1));
        } else {
            assert_eq!(observed.journal_revision(), intent_revision);
            assert!(matches!(observed.record(), FileLearnedResetRecord::Pending(p) if !p.interrupted));
        }
        assert_eq!(bytes(&root), before); assert_eq!(staged(&root), pending_bytes);
        // Read-only discovery has not rehabilitated the failed writer.
        assert_eq!(host.complete_learned_reset(host.revision(), 900).err(), Some(JournalError::Unavailable));
    }
}

#[test]
fn recipe_mismatch_and_corruption_anywhere_refuse_instead_of_returning_an_earlier_result() {
    let root = Directory::new(); let expected = config(false, 1);
    let (mut host, _) = owner(&root, &expected);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host);
    let control = instruction(&host, 900);
    host.reset_learned_checkpoint(host.revision(), &checkpoint, control, budget()).unwrap().unwrap();
    let original = bytes(&root);
    let changed = config(true, 1);
    assert_eq!(FileOversight::read_learned_reset_result(root.store(), &profile(), &changed, 900).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(bytes(&root), original);
    let mut events = host.events.clone();
    let mut changed_witness = false;
    for event in &mut events {
        if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = event {
            let mut forged = witness.to_vec(); *forged.last_mut().unwrap() ^= 1;
            *witness = forged.into(); changed_witness = true; break;
        }
    }
    assert!(changed_witness);
    let last = events.pop().unwrap();
    let corrupt_prefix = journal::encode_appended(&host.profile, host.store.identity(), &events, &last).unwrap();
    let invalid_suffix = journal::encode_appended(&host.profile, host.store.identity(), &host.events,
        &Event::Core(BaseEvent::Time(ElapsedTick(0)))).unwrap();
    for corrupt in [corrupt_prefix, invalid_suffix] {
        std::fs::write(root.store().join(storage::CANONICAL), &corrupt).unwrap();
        assert!(FileOversight::read_learned_reset_result(root.store(), &profile(), &expected, 900).is_err());
        assert!(FileOversight::read_learned_reset_result(root.store(), &profile(), &expected, 901).is_err());
        assert_eq!(bytes(&root), corrupt);
    }
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    assert!(matches!(read(&root, &expected, 900).record(), FileLearnedResetRecord::Completed { result: Ok(_), .. }));
    assert_eq!(FileOversight::read_learned_reset_result(Path::new("unused-invalid-operation"), &profile(), &expected, 0).err(),
        Some(JournalError::Contract(Error::InvalidInput)));
}

#[test]
fn a_full_journal_can_report_completion_without_room_for_another_recovery_fence() {
    let root = Directory::new(); let config = config(false, 1);
    let mut selected: FileOversightProfile = profile(); selected.delivery.limits.events = 7;
    let (mut host, _) = FileOversight::create(root.store(), selected.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config.clone()).unwrap();
    step(&mut host).unwrap(); let checkpoint = capture(&mut host);
    let control = instruction(&host, 900);
    host.reset_learned_checkpoint(host.revision(), &checkpoint, control, budget()).unwrap().unwrap();
    assert_eq!(host.revision(), 7); let before = bytes(&root); drop(host);
    assert!(FileOversight::open_with_learned_generation(root.store(), selected.clone(), &config).is_err());
    let observed = FileOversight::read_learned_reset_result(root.store(), &selected, &config, 900).unwrap();
    assert_eq!(observed.journal_revision(), 7);
    assert!(matches!(observed.record(), FileLearnedResetRecord::Completed { result: Ok(r), .. } if r.control.restored));
    assert_eq!(bytes(&root), before);
}

#[test]
fn recorded_native_audit_failure_is_not_converted_into_missing_or_successful_reset() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host);
    let control = instruction(&host, 900); let mut allowance = budget();
    allowance.audit.monitoring.probe_coordinates = 0;
    let error = host.reset_learned_checkpoint(host.revision(), &checkpoint, control.clone(), allowance).unwrap().unwrap_err();
    let usage = host.learned_recovery_usage().unwrap(); let before = bytes(&root);
    assert_eq!(usage.restart_attempts, 1);
    let observed = read(&root, &config, 900);
    let FileLearnedResetRecord::Completed { intent, result: Err(actual) } = observed.record() else { panic!("{observed:?}"); };
    assert_eq!(*actual, error); assert_eq!(intent.control(), &control); assert_eq!(intent.budget(), allowance);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage); assert_eq!(bytes(&root), before);
}

#[test]
fn reading_reset_history_does_not_reconcile_or_refund_an_outstanding_effect() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let checkpoint = capture(&mut host);
    let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let control = instruction(&host, 900);
    let reset = host.reset_learned_checkpoint(host.revision(), &checkpoint, control, budget()).unwrap().unwrap();
    assert_eq!(reset.control.refunded_units, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    let before = bytes(&root); let inspection = host.inspect();
    let observed = read(&root, &config, 900);
    assert!(matches!(observed.record(), FileLearnedResetRecord::Completed { result: Ok(r), .. }
        if r.control.refunded_units == 0 && r.control.cancelled.is_empty()));
    assert_eq!(host.inspect(), inspection); assert_eq!(bytes(&root), before);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
}
