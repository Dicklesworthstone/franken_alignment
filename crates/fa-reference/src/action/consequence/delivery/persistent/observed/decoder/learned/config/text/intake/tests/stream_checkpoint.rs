//! Real numerical histories and all original canonical replacement barriers.
//! Synthetic controls establish neither restart fidelity nor detector quality.
use super::*;
use super::stream::{stream_config, stream_delivery_profile, stream_profile};
use crate::action::consequence::activation::consistency::{
    BinaryForecast, ErrorBudget, ForecastRegistration,
};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use crate::action::consequence::activation::tensor::kv::{
    decoder::monitoring::restart::KvRestartBudget,
    experiment::KvSide,
    model::MAX_MODEL_KV_VALUES,
};
use crate::action::consequence::delivery::persistent::{
    RecoveryReserve, MAX_JOURNAL_BYTES,
    codec::shared::{Reader, Writer},
    governance::PolicyUpdate,
    observed::{
        Event, Machine, journal,
        consistency::{FileConsistencyConfig, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        containment::FileResetRequest,
        decoder::{DecoderEvent, learned::{LearnedEvent, bind_history,
            checkpoint::{self, CheckpointEvent, FileLearnedResetIntent,
                FileLearnedStreamCheckpoint}}},
    },
};
use crate::action::consequence::gate::{ReviewBinding,
    containment::session::policy::{Policy, Predicate}};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use std::rc::Rc;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];

fn start(root: &Directory, config: &FileLearnedConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_learned_text_stream(
        root.store(), stream_delivery_profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 1);
    host
}
fn capture(host: &mut FileOversight) -> FileLearnedStreamCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
fn budget(host: &FileOversight) -> KvRestartBudget {
    KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() }
}
fn instruction(host: &FileOversight, operation: u64) -> FileResetRequest {
    let state = host.inspect();
    FileResetRequest { operation, expected_control_sequence: state.control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: state.control.ledger.epoch,
        binding: ReviewBinding { round: operation, reducer_generation: 1,
            evidence_root: [8; 32] }, retained_targets: vec![state.target] }
}
fn frontier(host: &mut FileOversight, operation: u64) -> u64 {
    // A real governance transition orders the checkpoint before this frontier.
    // The incident's occurrence remains an independent supervisor declaration;
    // token steps alone do not advance the original control sequence.
    let current = host.inspect().control;
    let next = Policy::new(host.current_policy().unwrap().generation() + 1,
        vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap();
    host.replace_policy(host.revision(), &PolicyUpdate::new(operation,
        current.sequence, current.ledger.epoch, next).unwrap()).unwrap();
    host.inspect().control.sequence
}
fn prepared(root: &Directory, config: &FileLearnedConfig)
    -> (FileOversight, FileLearnedStreamCheckpoint, u64)
{
    let mut host = start(root, config);
    let saved = capture(&mut host);
    step(&mut host);
    let first_incident = frontier(&mut host, 800);
    assert!(saved.info().control_sequence < first_incident);
    (host, saved, first_incident)
}
fn bytes(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
fn events(host: &FileOversight, config: &FileLearnedConfig) -> Vec<Event> {
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes(host)).unwrap();
    bind_history(&mut events, config).unwrap();
    events
}
fn replay(host: &FileOversight, config: &FileLearnedConfig, events: &[Event])
    -> Result<Machine, Error>
{
    let encoded = journal::encode(&host.profile, host.store.identity(), events)?;
    let mut decoded = journal::decode(&host.profile, host.store.identity(), &encoded)?;
    bind_history(&mut decoded, config)?;
    Machine::replay(&host.profile, &decoded)
}
fn no_publication(host: &FileOversight) {
    let view = host.stream_snapshot().unwrap();
    assert_eq!(view.confirmed.message_count(), 0);
    assert_eq!(view.published.message_count(), 0);
    assert!(!view.confirmed.finished() && !view.published.finished());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 1000);
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn stream_capture_storage_faults_return_no_handle_and_recover_only_the_visible_cut() {
    for fault in std::iter::once(None).chain(BARRIERS.into_iter().map(Some)) {
        let root = Directory::new();
        let config = stream_config();
        let mut host = start(&root, &config);
        let before = host.learned_generation_inspection().unwrap().numerical;
        let epoch = host.inspect().control.ledger.epoch;
        if let Some(barrier) = fault { host.store.fail_once(barrier); }
        let result = host.capture_learned_stream_checkpoint(host.revision(), 1,
            before.actor_revision, epoch);
        if let Some(barrier) = fault {
            let JournalError::Io(failure) = result.unwrap_err() else {
                panic!("selected capture storage fault expected");
            };
            assert_eq!(failure.operation, barrier);
            assert_eq!(failure.replacement_may_be_visible,
                matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
            assert_eq!(host.learned_stream_checkpoint(1).err(), Some(JournalError::Unavailable));
            assert!(host.capture_learned_stream_checkpoint(host.revision(), 1,
                before.actor_revision, epoch).is_err());
        } else {
            let saved = result.unwrap();
            assert_eq!(saved.info().position, before.position);
            host.check_learned_stream_checkpoint(&saved).unwrap();
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
        }
        drop(host);
        let (host, _) = FileOversight::open_with_learned_text_stream(
            root.store(), stream_delivery_profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert!(host.learned_generation_inspection().unwrap().paused);
        let visible = fault.is_none() || fault == Some(JournalIo::DirectorySync);
        assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, usize::from(visible));
        if visible {
            assert_eq!(host.learned_stream_checkpoint(1).unwrap().info().position, 1);
            assert!(host.learned_checkpoint(1).is_err());
        } else {
            assert_eq!(host.learned_stream_checkpoint(1).err(), Some(Error::Missing.into()));
        }
        no_publication(&host);
    }
}

#[test]
fn no_stream_restart_work_precedes_acknowledgment_of_its_fixed_intent() {
    for fault in std::iter::once(None).chain(BARRIERS.into_iter().map(Some)) {
        let root = Directory::new();
        let config = stream_config();
        let (mut host, saved, first_incident) = prepared(&root, &config);
        let before = host.learned_generation_inspection().unwrap().numerical;
        let request = instruction(&host, 900); let allowance = budget(&host);
        let expected = FileLearnedResetIntent::for_recovery(saved.id(), request.clone(), allowance).unwrap();
        if let Some(barrier) = fault { host.store.fail_once(barrier); }
        let result = host.begin_learned_stream_reset(host.revision(), &saved,
            request, allowance, first_incident);
        if let Some(barrier) = fault {
            let JournalError::Io(failure) = result.unwrap_err() else {
                panic!("selected intent storage fault expected");
            };
            assert_eq!(failure.operation, barrier);
            assert!(host.complete_learned_stream_reset(host.revision(), 900).is_err());
        } else {
            result.unwrap();
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
            assert!(!host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        }
        drop(host);
        if fault.is_none() || fault == Some(JournalIo::DirectorySync) {
            let canonical = std::fs::read(root.store().join("delivery.bin")).unwrap();
            let mut run = FileOversight::begin_open_with_learned_generation(root.store(),
                stream_delivery_profile(), &config).unwrap();
            while run.progress().replayed_events < run.progress().total_events {
                run.advance(run.progress().replayed_events, 1).unwrap();
            }
            // A legacy recovery instruction has no stream kind or incident
            // binding. It must not recover a completion before the new Fence.
            assert!(run.finish_pending_reset(&expected).is_err());
            assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
        }
        let (mut host, _) = FileOversight::open_with_learned_text_stream(
            root.store(), stream_delivery_profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if fault.is_none() || fault == Some(JournalIo::DirectorySync) {
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert!(host.complete_learned_stream_reset(host.revision(), 900).is_err());
            assert!(host.complete_learned_reset(host.revision(), 900).is_err());
            assert!(host.resume_learned_generation(host.revision(), before.actor_revision,
                before.position).is_err());
        } else {
            assert!(host.pending_learned_reset().unwrap().is_none());
            let saved = host.learned_stream_checkpoint(1).unwrap();
            let request = instruction(&host, 900);
            let receipt = host.reset_learned_stream_checkpoint(host.revision(), &saved,
                request, allowance, first_incident).unwrap().unwrap();
            assert!(receipt.control.restored);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        }
        no_publication(&host);
    }
}

#[test]
fn stream_completion_faults_preserve_either_the_original_intent_or_one_actual_reset() {
    for fault in std::iter::once(None).chain(BARRIERS.into_iter().map(Some)) {
        let root = Directory::new();
        let config = stream_config();
        let (mut host, saved, first_incident) = prepared(&root, &config);
        let before = host.learned_generation_inspection().unwrap().numerical;
        let request = instruction(&host, 900); let allowance = budget(&host);
        host.begin_learned_stream_reset(host.revision(), &saved, request,
            allowance, first_incident).unwrap();
        if let Some(barrier) = fault { host.store.fail_once(barrier); }
        let result = host.complete_learned_stream_reset(host.revision(), 900);
        if let Some(barrier) = fault {
            let JournalError::Io(failure) = result.unwrap_err() else {
                panic!("selected completion storage fault expected");
            };
            assert_eq!(failure.operation, barrier);
            assert_eq!(host.learned_reset_result(900).err(), Some(JournalError::Unavailable));
            assert_eq!(host.pending_learned_reset().err(), Some(JournalError::Unavailable));
            assert!(host.complete_learned_stream_reset(host.revision(), 900).is_err());
        } else {
            assert!(result.unwrap().unwrap().control.restored);
        }
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_text_stream(
            root.store(), stream_delivery_profile(), &config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let restored = fault.is_none() || fault == Some(JournalIo::DirectorySync);
        if restored {
            assert!(host.pending_learned_reset().unwrap().is_none());
            let receipt = host.learned_reset_result(900).unwrap().unwrap();
            assert!(receipt.control.restored);
            let after = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(after.position, 1);
            assert_eq!(after.cumulative_work, before.cumulative_work);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
            assert_eq!(host.complete_learned_stream_reset(0, 900).unwrap().unwrap().control,
                receipt.control);
            assert!(host.complete_learned_reset(0, 900).is_err());
            host.resume_learned_generation(host.revision(), after.actor_revision, after.position).unwrap();
            step(&mut host);
        } else {
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
            assert!(host.complete_learned_stream_reset(host.revision(), 900).is_err());
            assert!(host.resume_learned_generation(host.revision(), before.actor_revision,
                before.position).is_err());
        }
        no_publication(&host);
    }
}

#[test]
fn stream_witnesses_bind_exact_capture_reset_intent_and_ordered_incident_frontier() {
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first_incident) = prepared(&root, &config);
    let later_frontier = frontier(&mut host, 801);
    assert!(first_incident < later_frontier);
    let request = instruction(&host, 900); let allowance = budget(&host);
    host.reset_learned_stream_checkpoint(host.revision(), &saved, request,
        allowance, first_incident).unwrap().unwrap();
    let canonical = bytes(&host);
    let original = events(&host, &config);
    replay(&host, &config, &original).unwrap();
    for reset in [false, true] {
        let index = original.iter().position(|event| matches!(event,
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::StreamCapture { .. }))) if !reset)
            || matches!(event, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::StreamReset { .. }))) if reset)).unwrap();
        let witness = match &original[index] {
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::StreamCapture { witness, .. }
                | CheckpointEvent::StreamReset { witness, .. }))) => witness.to_vec(),
            _ => unreachable!(),
        };
        assert_eq!(&witness[..8], if reset { b"FALSRS\0\x01" } else { b"FALSCP\0\x01" });
        for offset in [0, 8, witness.len() / 2, witness.len() - 1] {
            let mut altered = original.clone();
            let mut changed = witness.clone(); changed[offset] ^= 1;
            match &mut altered[index] {
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                    CheckpointEvent::StreamCapture { witness, .. }
                    | CheckpointEvent::StreamReset { witness, .. }))) => *witness = changed.into(),
                _ => unreachable!(),
            }
            assert_eq!(replay(&host, &config, &altered).err(), Some(Error::Binding));
        }
    }
    for change_incident in [false, true] {
        let mut altered = original.clone();
        for event in &mut altered {
            if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::BeginStreamReset { intent, first_incident_sequence }))) = event
            {
                if change_incident { *first_incident_sequence = later_frontier; }
                else { Rc::make_mut(intent).budget.audit.monitoring.refinements -= 1; }
            }
        }
        // Both frontiers are ordered and both quiet-audit ceilings could fit.
        // The original completion witness still binds the exact chosen input.
        assert_eq!(replay(&host, &config, &altered).err(), Some(Error::Binding));
    }
    let mut missing = original.clone();
    missing.retain(|event| !matches!(event, Event::Decoder(DecoderEvent::Learned(
        LearnedEvent::Checkpoint(CheckpointEvent::BeginStreamReset { .. })))));
    assert!(replay(&host, &config, &missing).is_err());
    for reset in [false, true] {
        let mut wrong_kind = original.clone();
        for event in &mut wrong_kind {
            if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value))) = event {
                *value = match &*value {
                    CheckpointEvent::StreamCapture { checkpoint, actor_revision, epoch, witness } if !reset =>
                        CheckpointEvent::Capture { checkpoint: *checkpoint,
                            actor_revision: *actor_revision, epoch: *epoch, witness: Rc::clone(witness) },
                    CheckpointEvent::StreamReset { operation, witness } if reset =>
                        CheckpointEvent::Reset { operation: *operation, witness: Rc::clone(witness) },
                    other => other.clone(),
                };
            }
        }
        assert!(replay(&host, &config, &wrong_kind).is_err());
    }
    assert_eq!(bytes(&host), canonical);
    no_publication(&host);
}

fn encoded(event: &CheckpointEvent) -> Vec<u8> {
    let mut writer = Writer::new(MAX_JOURNAL_BYTES);
    checkpoint::write(&mut writer, event).unwrap();
    writer.finish()
}

#[test]
fn stream_tags_are_disjoint_and_legacy_checkpoint_wire_bytes_remain_identical() {
    let witness: Rc<[u8]> = Rc::from(&[0xaa, 0xbb][..]);
    let capture = CheckpointEvent::Capture { checkpoint: 7, actor_revision: 8,
        epoch: 9, witness: Rc::clone(&witness) };
    let mut old_capture = vec![0];
    for scalar in [7_u64, 8, 9] { old_capture.extend_from_slice(&scalar.to_be_bytes()); }
    old_capture.extend_from_slice(&[0, 0, 0, 2, 0xaa, 0xbb]);
    assert_eq!(encoded(&capture), old_capture);
    let reset = CheckpointEvent::Reset { operation: 12, witness: Rc::clone(&witness) };
    let mut old_reset = vec![2]; old_reset.extend_from_slice(&12_u64.to_be_bytes());
    old_reset.extend_from_slice(&[0, 0, 0, 2, 0xaa, 0xbb]);
    assert_eq!(encoded(&reset), old_reset);
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first_incident) = prepared(&root, &config);
    let mut zero = budget(&host); zero.cache_values = 0;
    zero.audit.preparation.compression.source_values = 0;
    zero.audit.preparation.compression.encoded_bytes = 0;
    zero.audit.preparation.compression.work_units = 0;
    zero.audit.preparation.source_check.source_values = 0;
    zero.audit.preparation.source_check.encoded_bytes = 0;
    zero.audit.preparation.source_check.reconstruction_products = 0;
    zero.audit.monitoring.encoded_bytes = 0;
    zero.audit.monitoring.probe_coordinates = 0;
    zero.audit.monitoring.reconstruction_products = 0;
    zero.audit.monitoring.materialized_values = 0;
    zero.audit.monitoring.refinements = 0;
    // Fixed legacy wire fixture: zero audit allowance is only encoded here;
    // it is not presented as an executable successful reset.
    let legacy_intent = FileLearnedResetIntent { checkpoint: 1,
        control: FileResetRequest { operation: 2, expected_control_sequence: 3,
            expected_actor_revision: 4, expected_authority_epoch: 5,
            binding: ReviewBinding { round: 6, reducer_generation: 7, evidence_root: [8; 32] },
            retained_targets: Vec::new() }, budget: zero };
    let mut old_intent = vec![1];
    for scalar in [1_u64, 2, 3, 4, 5, 6, 7] {
        old_intent.extend_from_slice(&scalar.to_be_bytes());
    }
    old_intent.extend_from_slice(&[8; 32]);
    old_intent.extend_from_slice(&[0; 64]);
    assert_eq!(encoded(&CheckpointEvent::BeginReset(Rc::new(legacy_intent))), old_intent);
    let request = instruction(&host, 900); let allowance = budget(&host);
    host.reset_learned_stream_checkpoint(host.revision(), &saved, request,
        allowance, first_incident).unwrap().unwrap();
    for event in events(&host, &config) {
        if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(event))) = event {
            let tag = match &event {
                CheckpointEvent::StreamCapture { .. } => 3,
                CheckpointEvent::BeginStreamReset { .. } => 4,
                CheckpointEvent::StreamReset { .. } => 5,
                _ => panic!("stream history must retain its distinct typed event"),
            };
            let bytes = encoded(&event);
            assert_eq!(bytes[0], tag);
            let mut reader = Reader::new(&bytes);
            let roundtrip = checkpoint::read(&mut reader).unwrap(); reader.end().unwrap();
            assert_eq!(encoded(&roundtrip), bytes);
            for end in 0..bytes.len() {
                assert!(checkpoint::read(&mut Reader::new(&bytes[..end])).is_err());
            }
            if let CheckpointEvent::BeginStreamReset { intent, .. } = event {
                let legacy = encoded(&CheckpointEvent::BeginReset(Rc::clone(&intent)));
                assert_eq!(legacy[0], 1);
                // This is the unchanged legacy input encoding; no incident field
                // is smuggled into old tag1 or its original public intent shape.
                let mut reader = Reader::new(&legacy);
                let CheckpointEvent::BeginReset(roundtrip) = checkpoint::read(&mut reader).unwrap()
                    else { panic!("legacy intent kind must survive codec roundtrip"); };
                reader.end().unwrap(); assert_eq!(&*roundtrip, &*intent);
            }
        }
    }
    no_publication(&host);
}

#[test]
fn stream_reset_requires_two_ordinary_slots_and_never_spends_terminal_reserve() {
    for reserved in [false, true] {
        for slots in [0, 1, 2] {
            let root = Directory::new(); let config = stream_config();
            let mut profile = stream_delivery_profile();
            let tail = if reserved { RecoveryReserve::terminal().events } else { 0 };
            profile.delivery.limits.events = 9 + usize::from(reserved) + slots + tail;
            let (mut host, _) = if reserved {
                FileOversight::create_with_learned_text_stream_with_reserve(root.store(),
                    profile, config.clone(), RecoveryReserve::terminal())
            } else { FileOversight::create_with_learned_text_stream(root.store(), profile, config.clone()) }.unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            step(&mut host); let saved = capture(&mut host); step(&mut host);
            let first_incident = frontier(&mut host, 800);
            assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, slots);
            let before = host.learned_generation_inspection().unwrap();
            let usage = host.learned_recovery_usage().unwrap(); let canonical = bytes(&host);
            let request = instruction(&host, 900); let allowance = budget(&host);
            let result = host.begin_learned_stream_reset(host.revision(), &saved,
                request.clone(), allowance, first_incident);
            if slots < 2 {
                assert_eq!(result.err(), Some(Error::Limit.into()));
                assert!(host.pending_learned_reset().unwrap().is_none());
                assert_eq!(host.learned_generation_inspection().unwrap(), before);
                assert_eq!(host.learned_recovery_usage().unwrap(), usage);
                assert_eq!(bytes(&host), canonical); assert!(host.storage_failure().is_none());
            } else {
                result.unwrap();
                let receipt = host.complete_learned_stream_reset(host.revision(), 900).unwrap().unwrap();
                assert!(receipt.control.restored);
                assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
                assert_eq!(host.journal_capacity().unwrap().remaining().events, tail);
                let canonical = bytes(&host); let usage = host.learned_recovery_usage().unwrap();
                assert_eq!(host.reset_learned_stream_checkpoint(0, &saved, request,
                    allowance, first_incident).unwrap().unwrap().control, receipt.control);
                assert_eq!(bytes(&host), canonical);
                assert_eq!(host.learned_recovery_usage().unwrap(), usage);
                let original = events(&host, &config);
                replay(&host, &config, &original).unwrap();
                // Both byte dimensions apply to the actual complete event corpus,
                // including the fixed bootstrap and the terminal-reserve suffix.
                let mut exact = host.profile.clone();
                exact.delivery.limits.bytes = canonical.len()
                    + if reserved { RecoveryReserve::terminal().bytes } else { 0 };
                let encoded = journal::encode(&exact, host.store.identity(), &original).unwrap();
                assert_eq!(encoded.len(), canonical.len());
                journal::decode(&exact, host.store.identity(), &encoded).unwrap();
                exact.delivery.limits.bytes -= 1;
                assert_eq!(journal::encode(&exact, host.store.identity(), &original).err(), Some(Error::Limit));
            }
            no_publication(&host);
        }
    }
}

fn prediction_config(owned: bool) -> FileConsistencyConfig {
    let (model, _, _) = recipe();
    let neutral = BinaryForecast::new(32768, 32768).unwrap();
    let capture = if owned { model.cache_profile().layers()[&1].values().profile() }
        else { model.residual_contract(1).unwrap().profile() };
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: capture, weights: vec![0.0; 2],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"a".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_stream_messages(stream_profile()).unwrap()
}

fn required_predictor(owned: bool) -> FileLearnedConfig {
    let prediction = prediction_config(owned);
    if owned {
        let prediction = FileLearnedConsistencyConfig::new(prediction, 1, KvSide::Value,
            LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
            .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
        stream_config().with_required_owned_pre_output_forecast(prediction).unwrap()
    } else {
        stream_config().with_required_pre_output_forecast(
            prediction.with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap()
    }
}

#[test]
fn stream_checkpoint_kind_cannot_downgrade_legacy_or_required_raw_and_owned_predictors() {
    let root = Directory::new(); let config = stream_config();
    let (mut host, saved, first_incident) = prepared(&root, &config);
    let before = bytes(&host); let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.learned_checkpoint(saved.id()).is_err());
    assert!(host.capture_learned_checkpoint(host.revision(), 2, n.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    let request = instruction(&host, 900); let allowance = budget(&host);
    host.begin_learned_stream_reset(host.revision(), &saved, request,
        allowance, first_incident).unwrap();
    let pending_bytes = bytes(&host); let usage = host.learned_recovery_usage().unwrap();
    assert!(host.complete_learned_reset(host.revision(), 900).is_err());
    assert!(host.prepare_learned_reset_completion(host.revision(), 900).is_err());
    assert_eq!(bytes(&host), pending_bytes);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert!(host.complete_learned_stream_reset(host.revision(), 900).unwrap().unwrap().control.restored);
    assert_ne!(bytes(&host), before);
    no_publication(&host);
    let root = Directory::new(); let raw = super::config();
    let (mut raw_host, _) = FileOversight::create_with_learned_text(root.store(), profile(), raw).unwrap();
    raw_host.observe_time(raw_host.revision(), ElapsedTick(1)).unwrap(); step(&mut raw_host);
    let n = raw_host.learned_generation_inspection().unwrap().numerical;
    let legacy = raw_host.capture_learned_checkpoint(raw_host.revision(), 1,
        n.actor_revision, raw_host.inspect().control.ledger.epoch).unwrap();
    let canonical = bytes(&raw_host);
    assert!(raw_host.learned_stream_checkpoint(legacy.id()).is_err());
    assert!(raw_host.capture_learned_stream_checkpoint(raw_host.revision(), 2,
        n.actor_revision, raw_host.inspect().control.ledger.epoch).is_err());
    assert_eq!(bytes(&raw_host), canonical);
    // Optional supplied-frame prediction may be installed after a quiet capture
    // while the original control sequence is still zero. That old checkpoint
    // cannot subsequently reset or silently retire the new predictor lifetime.
    let root = Directory::new(); let config = stream_config();
    let mut host = start(&root, &config); let saved = capture(&mut host);
    assert_eq!(host.inspect().control.sequence, 0);
    let prediction = prediction_config(false);
    assert!(!prediction.requires_pre_output_forecast());
    host.enable_action_consistency(host.revision(), prediction).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let canonical = bytes(&host); let usage = host.learned_recovery_usage().unwrap();
    let forecast = host.action_consistency_snapshot().unwrap();
    assert!(host.capture_learned_stream_checkpoint(host.revision(), 2, n.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    assert_eq!(bytes(&host), canonical);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.action_consistency_snapshot().unwrap(), forecast);
    let first_incident = frontier(&mut host, 800);
    let request = instruction(&host, 900); let allowance = budget(&host);
    let canonical = bytes(&host); let forecast = host.action_consistency_snapshot().unwrap();
    assert!(host.begin_learned_stream_reset(host.revision(), &saved, request.clone(),
        allowance, first_incident).is_err());
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved, request,
        allowance, first_incident).is_err());
    assert_eq!(bytes(&host), canonical);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.action_consistency_snapshot().unwrap(), forecast);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.storage_failure().is_none());
    no_publication(&host);
    for owned in [false, true] {
        let root = Directory::new(); let config = required_predictor(owned);
        let (mut host, _, _) = FileOversight::create_with_pre_output_forecast(
            root.store(), stream_delivery_profile(), config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        assert!(host.action_consistency_required());
        let n = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(n.availability, LearnedAvailability::Ready);
        let canonical = bytes(&host); let usage = host.learned_recovery_usage().unwrap();
        let forecast = host.action_consistency_snapshot().unwrap();
        assert!(host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
            host.inspect().control.ledger.epoch).is_err());
        assert_eq!(bytes(&host), canonical);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.action_consistency_snapshot().unwrap(), forecast);
        no_publication(&host);
    }
}

#[test]
fn a_failed_stream_restart_audit_is_acknowledged_once_and_cannot_refund_its_work() {
    for fail in [false, true] {
        let root = Directory::new(); let config = stream_config();
        let (mut host, saved, first_incident) = prepared(&root, &config);
        let request = instruction(&host, 900); let mut allowance = budget(&host);
        if fail { allowance.audit.monitoring.probe_coordinates = 0; }
        let result = host.reset_learned_stream_checkpoint(host.revision(), &saved,
            request.clone(), allowance, first_incident).unwrap();
        assert_eq!(result.is_err(), fail);
        let usage = host.learned_recovery_usage().unwrap();
        assert_eq!(usage.restart_attempts, 1);
        assert!(usage.reserved_cache_values > 0);
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.host_failure.is_some(), fail);
        let canonical = bytes(&host);
        let retry = host.reset_learned_stream_checkpoint(0, &saved,
            request, allowance, first_incident).unwrap();
        match (&result, &retry) {
            (Ok(first), Ok(again)) => assert_eq!(first.control, again.control),
            (Err(first), Err(again)) => assert_eq!(first, again),
            _ => panic!("exact operation must retain its original acknowledged outcome"),
        }
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(bytes(&host), canonical);
        replay(&host, &config, &events(&host, &config)).unwrap();
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_text_stream(
            root.store(), stream_delivery_profile(), &config).unwrap();
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        if fail {
            assert!(host.learned_reset_result(900).unwrap().is_err());
            assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
        } else {
            host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
            step(&mut host);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.availability,
                LearnedAvailability::Ready);
        }
        no_publication(&host);
    }
}

mod recovery;
