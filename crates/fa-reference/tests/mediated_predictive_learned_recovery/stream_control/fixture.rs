//! Real owned stream, original source custodian and independently retained pair.
use super::*;

pub(super) const INVENTORY: &[u8] = include_bytes!("../../fixtures/authority_topology_inventory.json");

pub(super) fn document(generation: u64, bypass: bool) -> String {
    let extra = if bypass { r#",{"id":3,"from":1,"to":3,"channel":"dispatch","route":"publication","provenance":103}"# } else { "" };
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
      "complete_inventory_generation":{generation},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":3,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":3,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":90}}],
      "gates":[2],"check_edge_budget":65536,"source_limits":{{"checks":32,"read_bytes":67108928}}}}"#)
}

pub(super) fn stream_profile() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }
pub(super) fn stream_recipe() -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let raw = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: model.cache_profile().layers()[&1].values().profile(),
        weights: vec![1.0, 0.0, 0.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap().with_stream_messages(stream_profile()).unwrap();
    let predictor = FileLearnedConsistencyConfig::new(raw, 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
    FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model), source(&model),
        LearnedDecoderBindingLimits::default(), stream_profile()).unwrap().with_required_sidecar().unwrap()
        .with_required_owned_pre_output_forecast(predictor).unwrap()
        .with_forecast_text_stream_continuation().unwrap()
        .with_owned_forecast_stream_restart_audit().unwrap()
}

pub(super) struct History {
    pub root: Directory,
    pub profile: FileOversightProfile,
    pub config: FileLearnedConfig,
    pub expected: FilePredictiveMediatedRequirements,
    pub saved: TopologyCheckpoint,
    pub capture: FileLearnedStreamCheckpointRequest,
}
impl History {
    pub fn new(reserved: bool) -> Self {
        let root = Directory::new(); let config = stream_recipe();
        let mut p = profile(); p.delivery.initial_payload.clear(); p.delivery.total = 4096;
        p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
        let (mut host, roles) = if reserved {
            FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                root.store(), p.clone(), &guards(&config), None, graph(1, false), prediction(&config),
                None, config.clone(), RecoveryReserve::terminal())
        } else {
            FileOversight::create_predictive_mediated_guarded_with_learned_generation(
                root.store(), p.clone(), &guards(&config), None, graph(1, false), prediction(&config),
                None, config.clone())
        }.unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let directory = root.store().parent().unwrap().to_owned();
        let document = document(1, false);
        let loaded = LoadedAuthorityTopology::from_json_bytes(document.as_bytes(), INVENTORY).unwrap();
        assert_eq!(loaded.graph(), &graph(1, false));
        std::fs::write(directory.join("topology.json"), &document).unwrap();
        std::fs::write(directory.join("inventory.json"), INVENTORY).unwrap();
        let mut bound = roles.topology_observer.bind_topology_source(&host, host.revision(),
            directory.join("topology.json"), directory.join("inventory.json"), loaded).unwrap();
        let revision = host.revision(); let refreshed = bound.refresh(&mut host, revision, 1).unwrap();
        assert!(refreshed.refusal.is_none());
        assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
        assert!(step(&mut host).sample().is_none()); assert!(step(&mut host).sample().is_none());
        let source = FileDriverTopology::new(bound);
        let saved = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let control = host.inspect().control;
        assert_eq!((numerical.position, numerical.sampled_draws), (2, 0));
        let capture = FileLearnedStreamCheckpointRequest::new(host.revision(), FileLearnedCheckpointInfo {
            checkpoint: 71, actor_revision: numerical.actor_revision, authority_epoch: control.ledger.epoch,
            control_sequence: control.sequence, position: numerical.position, stream: 21, sampled_draws: 0,
        }).unwrap();
        let mut expected = requirements(&host, &config, None);
        expected.oversight.effective_policy = p.delivery.policy.clone();
        // Offline control must not read these files or reconstruct an allowance.
        std::fs::remove_file(directory.join("topology.json")).unwrap();
        std::fs::remove_file(directory.join("inventory.json")).unwrap();
        drop(source); drop(host);
        Self { root, profile: p, config, expected, saved, capture }
    }

    pub fn required(&self, saved: &TopologyCheckpoint) -> FilePredictiveMediatedRequirements {
        let mut expected = self.expected.clone();
        expected.oversight.minimum = saved.minimum();
        expected.topology.available = saved.available();
        expected
    }

    pub fn begin(&self, expected: &FilePredictiveMediatedRequirements, saved: &TopologyCheckpoint)
        -> Result<FilePredictiveMediatedLearnedRecovery, JournalError>
    {
        FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            self.root.store(), self.profile.clone(), expected, &self.config, saved.history_anchor())?
            .require_recovery_reserve(RecoveryReserve::terminal())
    }

    pub fn ready(&self, saved: &TopologyCheckpoint, request: FileMediatedStreamControlRequest)
        -> Result<FileMediatedStreamControlRecovery, JournalError>
    {
        let mut run = self.begin(&self.required(saved), saved)?
            .require_stream_control_checkpoint(saved, request)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1)?;
        }
        Ok(run)
    }

    pub fn apply(&self, saved: &TopologyCheckpoint, request: FileMediatedStreamControlRequest)
        -> Result<FileMediatedStreamControlHistory, JournalError>
    {
        self.ready(saved, request)?.finish_stream_control()
    }

    pub fn complete(&self, saved: &TopologyCheckpoint, intent: &FileLearnedResetIntent, first: u64)
        -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        let mut run = self.begin(&self.required(saved), saved)?
            .require_stream_reset_checkpoint(saved, intent, first)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1)?;
        }
        run.finish_pending_stream_reset()
    }
}

pub(super) fn narrow(history: &FileLearnedStreamControlHistory, info: FileLearnedCheckpointInfo)
    -> FileLearnedStreamNarrowRequest
{
    let cut = history.snapshot();
    FileLearnedStreamNarrowRequest::new(cut.revision, info, FileLearnedStreamNarrowControl {
        operation: 800, expected_control_sequence: cut.control.sequence,
        expected_actor_revision: history.numerical().actor_revision, expected_authority_epoch: cut.control.ledger.epoch,
        binding: ReviewBinding { round: 800, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![cut.target, ResolvedTarget { expected_version: cut.target.expected_version + 1, ..cut.target }],
    }).unwrap()
}

pub(super) fn reset(history: &FileLearnedStreamControlHistory) -> FileLearnedResetIntent {
    let cut = history.snapshot();
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    FileLearnedResetIntent::for_recovery(71, FileResetRequest {
        operation: 900, expected_control_sequence: cut.control.sequence,
        expected_actor_revision: history.numerical().actor_revision, expected_authority_epoch: cut.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: vec![cut.target, ResolvedTarget { expected_version: cut.target.expected_version + 1, ..cut.target }],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: source(&model).policy.allowance() }).unwrap()
}

pub(super) fn unchanged_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.work(), before.work()); assert_eq!(after.last_operation(), before.last_operation());
    assert!(after.minimum().control_sequence >= before.minimum().control_sequence);
    assert!(after.minimum().authority_epoch >= before.minimum().authority_epoch);
}
