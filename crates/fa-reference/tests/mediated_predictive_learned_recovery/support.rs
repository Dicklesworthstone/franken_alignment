//! Original decoder, predictor and native congress controls; no imported votes.
use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-mediated-predictive-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.store().join("delivery.bin")).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("mediated predictive cleanup: {error}");
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Mode { Raw, Owned }
#[derive(Clone, Copy, Debug)]
pub(super) struct Forecast { pub frame: FrameIdentity, pub probability: BinaryForecast }
impl Mode {
    pub(super) fn forecast(self, host: &mut FileOversight, observer: &FileConsistencyObserver,
        request: u64) -> Result<Result<Forecast, Error>, JournalError>
    {
        let n = host.learned_generation_inspection()?.numerical;
        let revision = host.revision();
        match self {
            Self::Raw => observer.forecast_hosted_request(host, revision, request, n.actor_revision)
                .map(|result| result.map(|prediction| Forecast {
                    frame: prediction.observation().frame(), probability: prediction.forecast() })),
            Self::Owned => observer.forecast_owned_learned_request(host, revision, request, n.actor_revision)
                .map(|result| result.and_then(|report| report.prediction().map(|prediction| Forecast {
                    frame: prediction.observation().frame(), probability: prediction.forecast() }))),
        }
    }
}

pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy {
                cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}

pub(super) fn source(model: &DecoderModel) -> LearnedTextConfig {
    let mut source = numerical::config(model);
    // The original tokenizer merges PP, leaving one actual prefill token after
    // our checkpoint. A fresh accepted token can establish successor evidence.
    source.prompt = "PPP".to_owned();
    assert_eq!(numerical::tokenizer(model).encode(source.prompt.as_bytes(), source.tokenization)
        .unwrap().tokens(), &[numerical::MERGED_PROMPT, numerical::PROMPT]);
    source
}

fn predictor(model: &DecoderModel, mode: Mode) -> FileConsistencyConfig {
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let profile = match mode {
        Mode::Raw => model.residual_contract(1).unwrap().profile(),
        Mode::Owned => model.cache_profile().layers()[&1].values().profile(),
    };
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile, weights: vec![1.0, 0.0, 0.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap()
}

pub(super) fn recipe(model: &DecoderModel, source: LearnedTextConfig,
    mode: Mode, successors: bool) -> FileLearnedConfig
{
    recipe_with_lifetime(model, source, mode, successors, LearnedMonitorBudget::default())
}

pub(super) fn recipe_with_lifetime(model: &DecoderModel, source: LearnedTextConfig,
    mode: Mode, successors: bool, lifetime: LearnedMonitorBudget) -> FileLearnedConfig
{
    let config = FileLearnedConfig::new_text(model.clone(), numerical::tokenizer(model), source,
        LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap();
    let config = match mode {
        Mode::Raw => config.with_required_pre_output_forecast(predictor(model, mode)
            .with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap(),
        Mode::Owned => config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
            predictor(model, mode), 1, KvSide::Value, LearnedMonitorBudget::default(), lifetime,
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
            .with_pre_output_forecast().unwrap()).unwrap(),
    };
    if successors { config.with_forecast_reset_successors().unwrap() } else { config }
}

pub(super) fn prediction(config: &FileLearnedConfig) -> FileMediatedPredictor {
    if let Some(raw) = config.required_pre_output_forecast() {
        FileMediatedPredictor::Raw(raw.clone())
    } else {
        FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone())
    }
}
pub(super) fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    let mut guards = custody::guards();
    guards.stream = config.text_stream_profile();
    guards.source = config.required_policy_source();
    guards
}
pub(super) fn create(root: &Directory, config: &FileLearnedConfig,
    evaluation: Option<EvaluationProtocol>) -> (FileOversight, FileMediatedRoles)
{
    let (mut host, roles) = FileOversight::create_predictive_mediated_guarded_with_learned_generation(
        root.store(), profile(), &guards(config), None, graph(1, false),
        prediction(config), evaluation, config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}

pub(super) fn step(host: &mut FileOversight) -> Rc<GenerationEvent> {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap()
}

pub(super) fn checkpoint_before_prompt_end(host: &mut FileOversight) -> FileLearnedCheckpoint {
    assert!(step(host).sample().is_none());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.position, 1);
    let saved = host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    assert!(step(host).sample().is_none());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 2);
    saved
}

pub(super) fn begin_reset(host: &mut FileOversight, saved: &FileLearnedCheckpoint,
    audit: fa_reference::action::consequence::activation::tensor::kv::decoder::monitoring::LearnedDecoderAllowance)
    -> FileLearnedResetIntent
{
    let n = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
        operation: 900, expected_control_sequence: control.sequence,
        expected_actor_revision: n.actor_revision, expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit }).unwrap();
    host.begin_learned_reset(host.revision(), saved, intent.control().clone(), intent.budget()).unwrap();
    intent
}

pub(super) fn requirements(host: &FileOversight, config: &FileLearnedConfig,
    evaluation: Option<EvaluationProtocol>) -> FilePredictiveMediatedRequirements
{
    let control = host.inspect().control;
    let topology = host.mediation_snapshot().unwrap();
    FilePredictiveMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(config),
            effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: host.revision(),
                control_sequence: control.sequence, authority_epoch: control.ledger.epoch } },
        topology: FileTopologyRequirement { initial: graph(1, false),
            current: topology.graph, available: topology.available },
        prediction: prediction(config), evaluation,
    }
}
pub(super) fn drive(run: &mut FilePredictiveMediatedLearnedRecovery,
    quantum: usize) -> Result<(), JournalError>
{
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum)?;
        assert!(after.replayed_events - before.replayed_events <= quantum);
    }
    Ok(())
}
pub(super) fn recover(root: &Directory, config: &FileLearnedConfig,
    expected: &FilePredictiveMediatedRequirements, intent: &FileLearnedResetIntent)
    -> Result<(FileOversight, FileMediatedRoles), JournalError>
{
    let mut run = FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
        root.store(), profile(), expected, config)?;
    drive(&mut run, 3)?;
    run.finish_pending_reset(intent)
}

pub(super) fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}

pub(super) fn original_samples(model: &DecoderModel, source: LearnedTextConfig) -> Vec<SampledToken> {
    let mut original = model.observed_learned_text_generation(numerical::tokenizer(model), source).unwrap();
    let mut samples = Vec::new();
    while original.status().is_active() {
        let event = original.advance(original.position()).unwrap();
        if let Some(sample) = event.sample() { samples.push(sample.clone()); }
    }
    assert_eq!(original.text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
    assert_eq!(samples.len(), 3);
    samples
}

pub(super) fn finish_samples(host: &mut FileOversight) -> Vec<SampledToken> {
    let mut samples = Vec::new();
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let event = step(host);
        if let Some(sample) = event.sample() { samples.push(sample.clone()); }
    }
    samples
}

pub(super) fn submit_generated(host: &mut FileOversight, request: u64) {
    let payload = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes().to_vec();
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload, required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 };
    host.submit_request(host.revision(), request, spec, Snapshot {
        semantic_epoch: 1, complete: true, values: BTreeMap::new() }).unwrap();
    assert_eq!(host.request_action(request).unwrap().spec().payload, b"OK");
}

pub(super) fn finish_generation(host: &mut FileOversight) -> (Vec<SampledToken>, Rc<GenerationEvent>) {
    let mut samples = Vec::new();
    let mut last = None;
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let event = step(host);
        if let Some(sample) = event.sample() { samples.push(sample.clone()); }
        last = Some(event);
    }
    (samples, last.expect("this fixture has three actual output steps"))
}

/// All disclosure rows and probes come from the actual final accepted event.
/// The public native-review adapter independently checks this exact source,
/// input, full row coverage and each original numerical probe before inference.
pub(super) fn review(host: &mut FileOversight, request: u64, round: u64,
    last: &GenerationEvent, spelling: &[u8]) -> (u64, CommitteeInput)
{
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
        else { panic!("original generated request must be admitted"); };
    let source = last.audit().source();
    let options = LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1000 + request, generation: 1, transform_id: 7 },
        priority: source.groups().collect(), budget: SidecarCongressBudget::default(),
    };
    let n = host.learned_generation_inspection().unwrap().numerical;
    let sidecar = host.begin_learned_sidecar_plan(host.revision(), attempt, n.actor_revision, options).unwrap();
    let input = host.current_learned_sidecar(&sidecar).unwrap().clone();
    let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
    let members = input.views().iter().enumerate().map(|(index, (name, view))| {
        let queries = rows.iter().map(|row| {
            let (frame, heads, channels) = source.row_shape(*row).unwrap();
            SidecarProbeQuery { row: *row, probe: LinearProbe::new(1, 1, frame.profile,
                &vec![0.0; heads * channels], 0.0, 1.0).unwrap() }
        }).collect();
        (name.clone(), NativeReviewMember {
            evaluator: native_fixture::evaluator(view.actual_input().input_profile().clone(), spelling),
            queries, salt: vec![16 + index as u8; 32],
        })
    }).collect();
    let mut run = host.begin_native_sidecar_review(host.revision(), sidecar,
        LearnedWorkerRound { round, evidence_root: [7; 32],
            window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } },
        members, NativeReviewLimits::default(), snapshot()).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.status() != NativeReviewStatus::Running { break; }
        run.advance(host, run.revision(), ElapsedTick(2), snapshot()).unwrap();
    }
    assert_eq!(run.status(), NativeReviewStatus::Finished);
    assert!(run.records().values().all(|record| record.failure.is_none()));
    assert!(run.records().values().any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    assert_eq!(run.input(), &input);
    (attempt, input)
}

pub(super) fn prepared(host: &mut FileOversight, request: u64, round: u64,
    last: &GenerationEvent) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest)
{
    let (attempt, input) = review(host, request, round, last, b"allow");
    let action = host.request_action(request).unwrap().clone();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let human = host.request_human_approval(host.revision(), 1000 + request, attempt,
        &input, ElapsedTick(30)).unwrap();
    (action, input, automatic, human)
}

pub(super) fn changed_owned(config: &FileLearnedConfig) -> FileMediatedPredictor {
    let original = config.required_owned_pre_output_forecast().unwrap();
    let lifetime = LearnedMonitorBudget { probe_coordinates: 1,
        ..LearnedMonitorBudget::default() };
    FileMediatedPredictor::Owned(FileLearnedConsistencyConfig::new(original.consistency().clone(),
        1, KvSide::Value, LearnedMonitorBudget::default(), lifetime, MAX_CHECKED_KV_BYTES).unwrap()
        .with_owned_generation().unwrap().with_pre_output_forecast().unwrap())
}

pub(super) fn changed_prediction(model: &DecoderModel, config: &FileLearnedConfig,
    mode: Mode) -> FileMediatedPredictor
{
    match mode {
        Mode::Raw => FileMediatedPredictor::Raw(predictor(model, mode)
            .with_hosted_residual(2).unwrap().with_pre_output_forecast().unwrap()),
        Mode::Owned => changed_owned(config),
    }
}
