//! Actual bounded generation and original publication owners, not imported state.
use super::*;
use fa_reference::action::{FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{stream::StreamProfile, persistent::{
    FileDeliveryProfile, FilePermit, JournalLimits, RecoveryReserve,
    observed::{FileHumanReviewer, FileHumanRequest, guarded::FileGuardSet},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput,
    HelperContract, ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy};
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
    learned::FileLearnedConsistencyConfig,
};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-situation-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = std::fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("learned situation cleanup: {error}"); }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lane { Plain, Raw, Owned }

pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 4096, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "reference".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: Vec::new(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn stream() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }
pub(super) fn recipe(lane: Lane, changed_prompt: bool) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let mut source = numerical::config(&model);
    if changed_prompt { source.prompt = "PPP".to_owned(); }
    let config = FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model), source,
        LearnedDecoderBindingLimits::default(), stream()).unwrap();
    if lane == Lane::Plain { return config; }
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let capture = if lane == Lane::Raw { model.residual_contract(1).unwrap().profile() }
        else { model.cache_profile().layers()[&1].values().profile() };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap().with_stream_messages(stream()).unwrap();
    let config = config.with_required_sidecar().unwrap();
    match lane {
        Lane::Raw => config.with_required_pre_output_forecast(prediction
            .with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Owned => config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
            prediction, 1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
            .with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Plain => unreachable!(),
    }
}
pub(super) fn owner(root: &Directory, recipe: &FileLearnedConfig)
    -> (FileOversight, FileHumanReviewer, Option<FileConsistencyObserver>)
{
    let guards = FileGuardSet { stream: Some(stream()), decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None };
    let (mut host, human, observer) = if let Some(prediction) = recipe.required_owned_pre_output_forecast() {
        let (host, roles) = FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
            root.store(), profile(), &guards, None, prediction.clone(), None, recipe.clone(),
            RecoveryReserve::terminal()).unwrap();
        (host, roles.oversight.human, Some(roles.consistency_observer))
    } else if let Some(prediction) = recipe.required_pre_output_forecast() {
        let (host, roles) = FileOversight::create_predictive_guarded_with_learned_generation_reserved(
            root.store(), profile(), &guards, None, prediction.clone(), None, recipe.clone(),
            RecoveryReserve::terminal()).unwrap();
        (host, roles.oversight.human, Some(roles.consistency_observer))
    } else {
        let (host, human) = FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe.clone()).unwrap();
        (host, human, None)
    };
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, human, observer)
}
pub(super) fn step(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
pub(super) fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) fn generate(host: &mut FileOversight) {
    for _ in 0..4 {
        if !host.learned_generation_inspection().unwrap().numerical.status.is_active() { break; }
        step(host);
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
}
pub(super) fn message(host: &mut FileOversight, attempt: u64) -> FrozenAction {
    host.propose_learned_text_stream_message(host.revision(), attempt, ElapsedTick(100), Vec::new(), snapshot()).unwrap()
}
pub(super) fn dispatch(host: &mut FileOversight, reviewer: &FileHumanReviewer, attempt: u64,
    action: &FrozenAction) -> CommitteeInput
{
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(action, &contracts, BTreeMap::from([("reviewer".into(), view)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    host.commit_review(host.revision(), round, "reviewer",
        commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic: FilePermit = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request: FileHumanRequest = host.request_human_approval(host.revision(), 1000 + attempt,
        attempt, &inputs, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, action, &inputs, snapshot()).unwrap(); inputs
}
