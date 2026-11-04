//! Original owned-prompt narrowing, exact durable retries and Store boundaries.
use super::*;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::delivery::persistent::{RecoveryReserve, codec::shared::Writer,
    observed::decoder::learned::{FileLearnedStreamControlReceipt, FileLearnedStreamNarrowControl,
        FileLearnedStreamNarrowRequest, checkpoint::{CheckpointEvent, write_narrow_request}}};
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;

fn stream_profile() -> StreamProfile { StreamProfile::new(1, 1, 4, 16, 64).unwrap() }
fn predictor(required: bool) -> FileLearnedConsistencyConfig {
    let raw = super::predictor(true).consistency().clone().with_stream_messages(stream_profile()).unwrap();
    let owned = FileLearnedConsistencyConfig::new(raw, 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap();
    if required { owned.with_pre_output_forecast().unwrap() } else { owned }
}
fn delivery_profile() -> FileOversightProfile {
    let mut profile = profile(); profile.delivery.initial_payload.clear(); profile
}
fn generation() -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(), stream_profile()).unwrap().with_required_sidecar().unwrap()
        .with_required_owned_pre_output_forecast(predictor(true)).unwrap()
        .with_forecast_text_stream_continuation().unwrap()
        .with_owned_forecast_stream_restart_audit().unwrap()
}
fn prepared(root: &Directory) -> (FileOversight, FileLearnedConfig, FileLearnedStreamNarrowRequest) {
    let generation = generation();
    let (mut host, _, _) = FileOversight::create_with_pre_output_forecast_with_reserve(
        root.store(), delivery_profile(), generation.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    let state = host.inspect();
    let request = FileLearnedStreamNarrowRequest::new(host.revision(), *saved.info(), FileLearnedStreamNarrowControl {
        operation: 800, expected_control_sequence: state.control.sequence,
        expected_actor_revision: n.actor_revision, expected_authority_epoch: state.control.ledger.epoch,
        binding: ReviewBinding { round: 800, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![state.target, ResolvedTarget { expected_version: state.target.expected_version + 1,
            ..state.target }],
    }).unwrap();
    (host, generation, request)
}
fn recovery(root: &Directory, generation: &FileLearnedConfig, required: &FileOwnedPredictiveRequirements,
    anchor: &FileHistoryAnchor, profile: FileOversightProfile) -> FileOwnedPredictiveRecovery
{
    let mut run = FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
        root.store(), profile, required, generation, anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    ready(&mut run); run
}

#[test]
fn every_live_and_offline_store_barrier_withholds_narrow_ack_and_retries_exactly_once() {
    for live in [false, true] {
        for barrier in BARRIERS {
            let root = Directory::new(); let (mut host, generation, request) = prepared(&root);
            let required = expected(&host, &generation, predictor(true)); let anchor = host.history_anchor().unwrap();
            let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
            let usage = host.learned_recovery_usage().unwrap(); let prediction = host.learned_action_consistency_snapshot().unwrap();
            let canonical = bytes(&root);
            if live {
                host.store.fail_once(barrier);
                assert!(matches!(host.narrow_learned_stream_checkpoint(&request),
                    Err(JournalError::Io(failure)) if failure.operation == barrier));
                assert_eq!(host.narrow_learned_stream_checkpoint(&request).err(), Some(JournalError::Unavailable));
                drop(host);
            } else {
                drop(host);
                let run = recovery(&root, &generation, &required, &anchor, delivery_profile());
                run.inner.fail_once(barrier);
                assert!(matches!(run.finish_narrowed_stream_checkpoint(&request),
                    Err(JournalError::Io(failure)) if failure.operation == barrier));
            }
            if barrier != JournalIo::DirectorySync { assert_eq!(bytes(&root), canonical); }
            let result = recovery(&root, &generation, &required, &anchor, delivery_profile())
                .finish_narrowed_stream_checkpoint(&request).unwrap();
            assert_eq!(result.appended_events(), usize::from(barrier != JournalIo::DirectorySync));
            assert_eq!(result.snapshot().revision, before.revision + 1);
            assert_eq!(result.snapshot().control.sequence, before.control.sequence + 1);
            assert_eq!(result.snapshot().control.ledger.epoch, before.control.ledger.epoch + 1);
            assert_eq!(result.numerical(), &numerical); assert_eq!(result.recovery_usage(), usage);
            let FileLearnedStreamControlReceipt::Narrowed { request: original, receipt } = result.receipt()
                else { panic!("native narrow receipt"); };
            assert_eq!(original, &request); assert_eq!(receipt.incident_count, 0);
            assert_eq!(receipt.refunded_units, 0); assert!(receipt.cancelled.is_empty());
            let read = FileOversight::read_predictive_guarded_anchored_with_owned_learned_consistency(
                root.store(), &delivery_profile(), &required, &generation, &anchor).unwrap();
            assert_eq!(&read.journal.publication, result.snapshot());
            assert_eq!(read.journal.consistency.work, prediction.work);
            assert_eq!(read.journal.consistency.retained_source_bytes, prediction.retained_source_bytes);
            assert_eq!(read.journal.consistency.consistency.evidence, prediction.consistency.evidence);
            assert!(!read.journal.consistency.consistency.coverage_lost);
            let canonical = bytes(&root);
            let repeated = recovery(&root, &generation, &required, &anchor, delivery_profile())
                .finish_narrowed_stream_checkpoint(&request).unwrap();
            assert_eq!(repeated.appended_events(), 0); assert_eq!(repeated.receipt(), result.receipt());
            assert_eq!(bytes(&root), canonical);
        }
    }
}

#[test]
fn wrong_capture_cut_predecessor_audience_and_changed_retry_never_narrow() {
    for field in 0..13 {
        let root = Directory::new(); let (mut host, generation, request) = prepared(&root);
        let required = expected(&host, &generation, predictor(true)); let anchor = host.history_anchor().unwrap();
        let before = host.inspect(); let canonical = bytes(&root);
        let mut revision = request.revision(); let mut checkpoint = request.checkpoint();
        let mut control = request.control().clone();
        match field {
            0 => revision += 1, 1 => checkpoint.actor_revision += 1, 2 => checkpoint.authority_epoch += 1,
            3 => checkpoint.control_sequence += 1, 4 => checkpoint.stream += 1, 5 => checkpoint.position += 1,
            6 => checkpoint.sampled_draws += 1, 7 => checkpoint.checkpoint += 1,
            8 => control.expected_control_sequence += 1, 9 => control.expected_actor_revision += 1,
            10 => control.expected_authority_epoch += 1, 11 => control.retained_targets[0].object += 1,
            _ => control.retained_targets[0].generation += 1,
        }
        let changed = FileLearnedStreamNarrowRequest::new(revision, checkpoint, control).unwrap();
        assert!(host.narrow_learned_stream_checkpoint(&changed).is_err());
        assert_eq!(host.inspect(), before); assert_eq!(bytes(&root), canonical); drop(host);
        assert!(recovery(&root, &generation, &required, &anchor, delivery_profile())
            .finish_narrowed_stream_checkpoint(&changed).is_err());
        assert_eq!(bytes(&root), canonical);
        let result = recovery(&root, &generation, &required, &anchor, delivery_profile())
            .finish_narrowed_stream_checkpoint(&request).unwrap();
        assert_eq!(result.appended_events(), 1);
        let canonical = bytes(&root);
        assert!(recovery(&root, &generation, &required, &anchor, delivery_profile())
            .finish_narrowed_stream_checkpoint(&changed).is_err());
        assert_eq!(bytes(&root), canonical);
    }
}

#[test]
fn ordinary_event_and_byte_limits_cannot_spend_the_terminal_reserve_for_narrowing() {
    for limited_bytes in [false, true] {
        for ordinary in [0, 1] {
            let root = Directory::new(); let (host, generation, request) = prepared(&root);
            let required = expected(&host, &generation, predictor(true));
            let events = host.events.clone(); let mut profile = delivery_profile();
            if limited_bytes {
                profile.delivery.limits.bytes = bytes(&root).len() + RecoveryReserve::terminal().bytes
                    + ordinary * 65_536;
            } else {
                profile.delivery.limits.events = events.len() + ordinary + RecoveryReserve::terminal().events;
            }
            let canonical = journal::encode(&profile, host.store.identity(), &events).unwrap();
            let anchor = FileHistoryAnchor::from_acknowledged(events.len(), canonical.clone());
            drop(host); std::fs::write(root.store().join(storage::CANONICAL), &canonical).unwrap();
            let result = recovery(&root, &generation, &required, &anchor, profile)
                .finish_narrowed_stream_checkpoint(&request);
            if ordinary == 0 {
                assert_eq!(result.err(), Some(Error::Limit.into())); assert_eq!(bytes(&root), canonical);
            } else {
                assert_eq!(result.unwrap().appended_events(), 1);
            }
        }
    }
}

#[test]
fn journal_index_and_original_narrow_witness_are_checked_independently_on_replay() {
    let root = Directory::new(); let (mut host, generation, request) = prepared(&root);
    host.narrow_learned_stream_checkpoint(&request).unwrap();
    let index = request.revision() as usize;
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(CheckpointEvent::StreamNarrow {
        request: encoded, .. }))) = &mut events[index] else { panic!("narrow event"); };
    *encoded = std::rc::Rc::new(FileLearnedStreamNarrowRequest::new(request.revision() + 1,
        request.checkpoint(), request.control().clone()).unwrap());
    assert_eq!(journal::encode(&host.profile, host.store.identity(), &events), Err(Error::Binding));
    // The decoder independently rejects a moved expected-prefix coordinate,
    // before canonical re-encoding or a private numerical replay can begin.
    let mut writer = Writer::new(65_536); write_narrow_request(&mut writer, &request).unwrap();
    let request_bytes = writer.finish(); let mut canonical = bytes(&root);
    let offset = canonical.windows(request_bytes.len()).position(|span| span == request_bytes).unwrap();
    canonical[offset..offset + 8].copy_from_slice(&(request.revision() + 1).to_be_bytes());
    assert!(journal::decode(&host.profile, host.store.identity(), &canonical).is_err());
    for changed_request in [false, true] {
        let mut events = host.events.clone();
        let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(CheckpointEvent::StreamNarrow {
            request: selected, witness }))) = &mut events[index] else { panic!("narrow event"); };
        if changed_request {
            let mut checkpoint = request.checkpoint(); checkpoint.position += 1;
            *selected = std::rc::Rc::new(FileLearnedStreamNarrowRequest::new(request.revision(),
                checkpoint, request.control().clone()).unwrap());
        } else {
            let mut altered = witness.to_vec(); *altered.last_mut().unwrap() ^= 1; *witness = altered.into();
        }
        let encoded = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
        let mut decoded = journal::decode(&host.profile, host.store.identity(), &encoded).unwrap();
        bind_owned_history(&mut decoded, &generation, &predictor(true)).unwrap();
        assert_eq!(Machine::replay(&host.profile, &decoded).err(), Some(Error::Binding));
    }
    independent(&host, &generation, &predictor(true));
}

#[test]
fn a_real_certified_topology_cannot_enter_the_unmediated_prompt_narrow_profile() {
    use crate::action::consequence::delivery::persistent::observed::guarded::mediated::predictive::FileMediatedPredictor;
    use crate::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
    use crate::perimeter_inventory::topology::LoadedAuthorityTopology;
    let inventory = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
    let topology = br#"{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
      "complete_inventory_generation":1,
      "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"},
      "target":{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1},
      "inventory_scope":{"tenant":1,"principal":2,"purpose":1},"family":"file-publication",
      "nodes":[{"id":1,"kind":"actor"},{"id":2,"kind":"enforcer"},{"id":4,"kind":"sink"}],
      "edges":[{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101},
               {"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}],
      "enforcers":[{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}],
      "gates":[2],"check_edge_budget":1000,"source_limits":{"checks":32,"read_bytes":134217728}}"#;
    let graph = LoadedAuthorityTopology::from_json_bytes(topology, inventory).unwrap().graph().clone();
    let root = Directory::new(); let generation = generation();
    let guards = FileGuardSet { stream: Some(stream_profile()), decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None };
    let (mut host, roles) = FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
        root.store(), delivery_profile(), &guards, None, graph.clone(),
        FileMediatedPredictor::Owned(predictor(true)), None, generation,
        RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(roles.topology_observer.certify(&mut host, revision, epoch,
        &graph.propose_cut(&[2]).unwrap(), MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
    step(&mut host); let numerical = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_stream_checkpoint(host.revision(), 1, numerical.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    let before = host.inspect(); let canonical = bytes(&root); let topology = host.mediation_snapshot().unwrap();
    assert!(topology.accepted.is_some());
    let request = FileLearnedStreamNarrowRequest::new(host.revision(), *saved.info(), FileLearnedStreamNarrowControl {
        operation: 800, expected_control_sequence: before.control.sequence,
        expected_actor_revision: numerical.actor_revision, expected_authority_epoch: before.control.ledger.epoch,
        binding: ReviewBinding { round: 800, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![before.target],
    }).unwrap();
    assert_eq!(host.narrow_learned_stream_checkpoint(&request).err(), Some(Error::Binding.into()));
    assert_eq!(host.inspect(), before); assert_eq!(host.mediation_snapshot().unwrap(), topology);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(bytes(&root), canonical);
    // The SAME selected profile succeeds without the independent topology
    // obligation, establishing that prompt/source/capture were not the cause.
    let root = Directory::new(); let (mut host, _, request) = prepared(&root);
    assert_eq!(host.narrow_learned_stream_checkpoint(&request).unwrap().sequence, 1);
}
