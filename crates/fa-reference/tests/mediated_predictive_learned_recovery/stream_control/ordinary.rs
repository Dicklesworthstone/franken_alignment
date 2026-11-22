//! The ordinary guarded owner uses the same sealed paired control transaction.
//! These real local journals and declared graphs do not certify host isolation.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::learned::FileMediatedLearnedRecovery;

struct OrdinaryHistory {
    root: Directory,
    profile: FileOversightProfile,
    config: FileLearnedConfig,
    expected: FileMediatedRequirements,
    saved: TopologyCheckpoint,
    capture: FileLearnedStreamCheckpointRequest,
}

impl OrdinaryHistory {
    fn new(reserved: bool) -> Self {
        let root = Directory::new();
        let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
        let config = FileLearnedConfig::new_text_stream(model.clone(),
            numerical::tokenizer(&model), source(&model),
            LearnedDecoderBindingLimits::default(), stream_profile()).unwrap()
            .with_required_sidecar().unwrap();
        assert!(config.required_pre_output_forecast().is_none());
        assert!(config.required_owned_pre_output_forecast().is_none());
        let mut profile = profile();
        profile.delivery.initial_payload.clear();
        profile.delivery.total = 4096;
        profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
        let (mut host, roles) = if reserved {
            FileOversight::create_mediated_guarded_with_learned_generation_reserved(
                root.store(), profile.clone(), &guards(&config), None, graph(1, false),
                None, config.clone(), RecoveryReserve::terminal())
        } else {
            FileOversight::create_mediated_guarded_with_learned_generation(
                root.store(), profile.clone(), &guards(&config), None, graph(1, false),
                None, config.clone())
        }.unwrap();
        assert!(roles.consistency_observer.is_none());
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let directory = root.store().parent().unwrap().to_owned();
        let document = document(1, false);
        let loaded = LoadedAuthorityTopology::from_json_bytes(document.as_bytes(), INVENTORY).unwrap();
        assert_eq!(loaded.graph(), &graph(1, false));
        std::fs::write(directory.join("topology.json"), &document).unwrap();
        std::fs::write(directory.join("inventory.json"), INVENTORY).unwrap();
        let mut bound = roles.topology_observer.bind_topology_source(
            &host, host.revision(), directory.join("topology.json"),
            directory.join("inventory.json"), loaded).unwrap();
        let revision = host.revision();
        let refreshed = bound.refresh(&mut host, revision, 1).unwrap();
        assert!(refreshed.refusal.is_none());
        assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
        assert!(step(&mut host).sample().is_none());
        assert!(step(&mut host).sample().is_none());
        let source = FileDriverTopology::new(bound);
        let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let control = host.inspect().control;
        assert_eq!((numerical.position, numerical.sampled_draws), (2, 0));
        let capture = FileLearnedStreamCheckpointRequest::new(host.revision(),
            FileLearnedCheckpointInfo {
                checkpoint: 71, actor_revision: numerical.actor_revision,
                authority_epoch: control.ledger.epoch, control_sequence: control.sequence,
                position: numerical.position, stream: 21, sampled_draws: 0,
            }).unwrap();
        let expected = FileMediatedRequirements {
            oversight: FileRecoveryRequirements {
                guards: guards(&config), effective_policy: profile.delivery.policy.clone(),
                credential_epoch: None, minimum: saved.minimum(),
            },
            topology: FileTopologyRequirement {
                initial: graph(1, false), current: graph(1, false), available: saved.available(),
            },
            prediction: None,
            evaluation: None,
        };
        // Successful offline control must retain source custody without reading
        // either document again or manufacturing fresh source-check allowance.
        std::fs::remove_file(directory.join("topology.json")).unwrap();
        std::fs::remove_file(directory.join("inventory.json")).unwrap();
        drop(source);
        drop(host);
        Self { root, profile, config, expected, saved, capture }
    }

    fn required(&self, saved: &TopologyCheckpoint) -> FileMediatedRequirements {
        let mut expected = self.expected.clone();
        expected.oversight.minimum = saved.minimum();
        expected.topology.available = saved.available();
        expected
    }

    fn begin(&self, expected: &FileMediatedRequirements, saved: &TopologyCheckpoint)
        -> Result<FileMediatedLearnedRecovery, JournalError>
    {
        FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            self.root.store(), self.profile.clone(), expected, &self.config,
            saved.history_anchor())
    }

    fn ready(&self, saved: &TopologyCheckpoint, request: FileMediatedStreamControlRequest)
        -> Result<FileMediatedStreamControlRecovery, JournalError>
    {
        let mut run = self.begin(&self.required(saved), saved)?
            .require_stream_control_checkpoint(saved, request)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 2)?;
        }
        Ok(run)
    }

    fn apply(&self, saved: &TopologyCheckpoint, request: FileMediatedStreamControlRequest)
        -> Result<FileMediatedStreamControlHistory, JournalError>
    {
        self.ready(saved, request)?.finish_stream_control()
    }

    fn complete(&self, saved: &TopologyCheckpoint, intent: &FileLearnedResetIntent,
        first: u64) -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        let mut run = self.begin(&self.required(saved), saved)?
            .require_stream_reset_checkpoint(saved, intent, first)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 2)?;
        }
        run.finish_pending_stream_reset()
    }

    fn assert_no_source_refresh(&self) {
        let directory = self.root.store().parent().unwrap().to_owned();
        assert!(!directory.join("topology.json").exists());
        assert!(!directory.join("inventory.json").exists());
    }
}

#[test]
fn ordinary_pair_captures_narrows_and_begins_reset_without_prediction_or_new_source_work() {
    let h = OrdinaryHistory::new(true);
    let capture = FileMediatedStreamControlRequest::Capture(h.capture);
    let captured = h.apply(&h.saved, capture.clone()).unwrap();
    assert_eq!(captured.control().appended_events(), 1);
    assert_eq!(captured.control().snapshot().revision, h.saved.history_anchor().revision() + 1);
    assert_eq!(captured.topology_checkpoint().history_anchor(), captured.control().history_anchor());
    unchanged_source(&h.saved, captured.topology_checkpoint());
    let bytes = h.root.bytes();
    let retry = h.apply(&h.saved, capture).unwrap();
    assert_eq!(retry.control().appended_events(), 0);
    assert_eq!(retry.control().receipt(), captured.control().receipt());
    assert_eq!(retry.topology_checkpoint(), captured.topology_checkpoint());
    assert_eq!(h.root.bytes(), bytes);

    let request = narrow(captured.control(), h.capture.checkpoint());
    let selection = FileMediatedStreamControlRequest::Narrow(request.clone());
    let narrowed = h.apply(captured.topology_checkpoint(), selection.clone()).unwrap();
    let first = match narrowed.control().receipt() {
        FileLearnedStreamControlReceipt::Narrowed { request: actual, receipt } => {
            assert_eq!(actual, &request);
            assert_eq!(receipt.incident_count, 0);
            assert_eq!(receipt.sequence, h.capture.checkpoint().control_sequence + 1);
            assert_eq!(receipt.revocation_floor, h.capture.checkpoint().authority_epoch + 1);
            receipt.sequence
        },
        _ => panic!("ordinary native narrowing receipt"),
    };
    assert_eq!(narrowed.control().appended_events(), 1);
    assert_eq!(narrowed.control().numerical(), captured.control().numerical());
    assert_eq!(narrowed.control().recovery_usage(), captured.control().recovery_usage());
    assert_eq!(narrowed.control().snapshot().control.ledger.charged, 0);
    unchanged_source(&h.saved, narrowed.topology_checkpoint());
    let bytes = h.root.bytes();
    let retry = h.apply(captured.topology_checkpoint(), selection).unwrap();
    assert_eq!(retry.control().appended_events(), 0);
    assert_eq!(retry.control().receipt(), narrowed.control().receipt());
    assert_eq!(retry.topology_checkpoint(), narrowed.topology_checkpoint());
    assert_eq!(h.root.bytes(), bytes);

    let intent = reset(narrowed.control());
    let selection = FileMediatedStreamControlRequest::BeginReset {
        intent: intent.clone(), first_incident_sequence: first,
    };
    let begun = h.apply(narrowed.topology_checkpoint(), selection.clone()).unwrap();
    assert_eq!(begun.control().appended_events(), 1);
    assert_eq!(begun.control().numerical(), narrowed.control().numerical());
    assert_eq!(begun.control().recovery_usage().restart_attempts, 0);
    unchanged_source(&h.saved, begun.topology_checkpoint());
    h.assert_no_source_refresh();
    let bytes = h.root.bytes();
    let retry = h.apply(narrowed.topology_checkpoint(), selection).unwrap();
    assert_eq!(retry.control().appended_events(), 0);
    assert_eq!(retry.topology_checkpoint(), begun.topology_checkpoint());
    assert_eq!(h.root.bytes(), bytes);

    let (host, roles) = h.complete(begun.topology_checkpoint(), &intent, first).unwrap();
    assert!(roles.consistency_observer.is_none());
    assert!(host.action_consistency_snapshot().is_err());
    let result = host.learned_reset_result(900).unwrap().unwrap();
    assert!(result.control.restored);
    assert_eq!(result.resumed_stream, Some(22));
    assert!(result.restored_forecast_audit.is_none());
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.inspect().executions, 0);
    h.assert_no_source_refresh();
}

#[test]
fn ordinary_pair_preserves_independent_guards_and_event_zero_source_admission() {
    let h = OrdinaryHistory::new(true);
    let foreign = OrdinaryHistory::new(true);
    let request = FileMediatedStreamControlRequest::Capture(h.capture);
    let bytes = h.root.bytes();
    assert!(h.apply(&foreign.saved, request.clone()).is_err());
    let mut late = h.begin(&h.expected, &h.saved).unwrap();
    late.advance(0, 1).unwrap();
    assert_eq!(late.require_stream_control_checkpoint(&h.saved, request.clone()).err(),
        Some(Error::WrongState.into()));
    assert!(h.begin(&h.expected, &h.saved).unwrap()
        .require_stream_control_checkpoint(&h.saved, request.clone()).unwrap()
        .finish_stream_control().is_err());
    for field in 0..9 {
        let mut expected = h.expected.clone();
        match field {
            0 => expected.oversight.guards.identity = None,
            1 => expected.oversight.effective_policy =
                Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap(),
            2 => expected.topology.available = false,
            3 => expected.topology.current = graph(2, false),
            4 => expected.oversight.minimum.control_sequence += 1,
            5 => expected.oversight.minimum.journal_revision += 1,
            6 => expected.oversight.minimum.authority_epoch += 1,
            7 => expected.prediction = Some(stream_recipe()
                .required_owned_pre_output_forecast().unwrap().consistency().clone()),
            _ => expected.evaluation = Some(protocol()),
        }
        let result = h.begin(&expected, &h.saved).and_then(|run|
            run.require_stream_control_checkpoint(&h.saved, request.clone())).and_then(|mut run| {
                while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                    run.advance(run.progress().replayed_events, 2)?;
                }
                run.finish_stream_control()
            });
        assert!(result.is_err(), "independent requirement {field}");
        assert_eq!(h.root.bytes(), bytes);
    }
    let unreserved = OrdinaryHistory::new(false);
    let unreserved_bytes = unreserved.root.bytes();
    assert!(unreserved.apply(&unreserved.saved,
        FileMediatedStreamControlRequest::Capture(unreserved.capture)).is_err());
    assert_eq!(unreserved.root.bytes(), unreserved_bytes);
    let captured = h.apply(&h.saved, request).unwrap();
    assert_eq!(captured.control().appended_events(), 1);
    unchanged_source(&h.saved, captured.topology_checkpoint());
    h.assert_no_source_refresh();
}

#[test]
fn ordinary_pair_rejects_changed_operations_and_unrelated_unexported_successors() {
    let h = OrdinaryHistory::new(true);
    let capture = FileMediatedStreamControlRequest::Capture(h.capture);
    let captured = h.apply(&h.saved, capture.clone()).unwrap();
    let bytes = h.root.bytes();
    let mut changed_info = h.capture.checkpoint();
    changed_info.position += 1;
    let changed = FileLearnedStreamCheckpointRequest::new(h.capture.revision(), changed_info).unwrap();
    assert!(h.apply(&h.saved, FileMediatedStreamControlRequest::Capture(changed)).is_err());
    assert_eq!(h.root.bytes(), bytes);

    let request = narrow(captured.control(), h.capture.checkpoint());
    let narrowed = h.apply(captured.topology_checkpoint(),
        FileMediatedStreamControlRequest::Narrow(request.clone())).unwrap();
    let bytes = h.root.bytes();
    let mut changed_control = request.control().clone();
    changed_control.binding.evidence_root[0] ^= 1;
    let changed = FileLearnedStreamNarrowRequest::new(
        request.revision(), request.checkpoint(), changed_control).unwrap();
    assert!(h.apply(captured.topology_checkpoint(),
        FileMediatedStreamControlRequest::Narrow(changed)).is_err());
    assert!(h.apply(&h.saved, capture).is_err());
    assert_eq!(h.root.bytes(), bytes);

    let intent = reset(narrowed.control());
    let first = narrowed.control().snapshot().control.sequence;
    let begun = h.apply(narrowed.topology_checkpoint(),
        FileMediatedStreamControlRequest::BeginReset {
            intent: intent.clone(), first_incident_sequence: first,
        }).unwrap();
    let bytes = h.root.bytes();
    assert!(h.apply(narrowed.topology_checkpoint(),
        FileMediatedStreamControlRequest::BeginReset {
            intent: intent.clone(), first_incident_sequence: first + 1,
        }).is_err());
    assert!(h.apply(captured.topology_checkpoint(),
        FileMediatedStreamControlRequest::Narrow(request)).is_err());
    assert_eq!(h.root.bytes(), bytes);
    assert_eq!(h.complete(begun.topology_checkpoint(), &intent, first).unwrap().0
        .actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn ordinary_pair_requires_original_canonical_storage_and_round_trips_exact_source_custody() {
    let h = OrdinaryHistory::new(true);
    let request = FileMediatedStreamControlRequest::Capture(h.capture);
    let canonical = h.root.bytes();
    let run = h.ready(&h.saved, request.clone()).unwrap();
    std::fs::write(h.root.store().join("delivery.bin"), b"changed canonical bytes").unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(Error::Binding.into()));
    std::fs::write(h.root.store().join("delivery.bin"), &canonical).unwrap();

    let run = h.ready(&h.saved, request.clone()).unwrap();
    std::fs::create_dir(h.root.store().join("delivery.pending")).unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(JournalError::InvalidFile));
    assert_eq!(h.root.bytes(), canonical);
    std::fs::remove_dir(h.root.store().join("delivery.pending")).unwrap();

    let captured = h.apply(&h.saved, request.clone()).unwrap();
    assert_eq!(captured.control().appended_events(), 1);
    let bytes = h.root.bytes();
    let retry = h.apply(&h.saved, request).unwrap();
    assert_eq!(retry.control().appended_events(), 0);
    assert_eq!(retry.topology_checkpoint(), captured.topology_checkpoint());
    assert_eq!(h.root.bytes(), bytes);
    let mut archive = Vec::new();
    captured.topology_checkpoint().write_to(&mut archive, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    let restored = TopologyCheckpoint::read_trusted(
        &mut &archive[..], MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    assert_eq!(&restored, captured.topology_checkpoint());
    unchanged_source(&h.saved, &restored);
    h.assert_no_source_refresh();
}
