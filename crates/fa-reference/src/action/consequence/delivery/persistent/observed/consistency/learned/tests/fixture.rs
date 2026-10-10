//! Actual decoder, fitted codec, source checker and original two-key publication.
//! Small synthetic weights isolate lost coordinates; this is not qualification.
use super::*;
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::probe::learned::{CheckedKvBudget, ResidualRetention};
use crate::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS},
    model::{ModelKvImage, learned::{CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy}},
};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, human::HumanReviewPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-consistency-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("learned consistency cleanup: {error}"); }
    }
}

pub(super) fn row() -> KvRow { KvRow { layer: 1, side: KvSide::Key, position: 0 } }
pub(super) fn source(value: [f32; 2], retention: ResidualRetention, tokens: usize)
    -> (CheckedLearnedKv, ModelKvImage)
{
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 1, tokenizer_generation: 1, profile_generation: 1 },
        DecoderShape { vocabulary: 4, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
    let model = DecoderModel::new(profile,
        vec![1.0, 0.0, -1.0, 0.0, value[0], value[1], 0.0, 1.0],
        vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
            keys: vec![1.0, 0.0, 0.0, 1.0], values: vec![1.0, 0.0, 0.0, 1.0],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
            gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0; 8]).unwrap();
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let positive = model.recompute(11, &[0], inference).unwrap().cache_image().unwrap();
    let negative = model.recompute(12, &[1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, positive), (102, negative)]), FitBudget::default()).unwrap();
    let original = model.recompute(17, &vec![2; tokens], inference).unwrap().cache_image().unwrap();
    let (image, report) = codec.evaluate_held_out(202, &original, CompressionBudget::default()).unwrap();
    assert!(!report.training_source_overlap);
    let checked = CheckedLearnedKv::new(image, &original, retention, CheckedKvBudget::default()).unwrap();
    (checked, original)
}

pub(super) fn actor(tokens: usize, cache: Vec<u8>) -> ActorState {
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![2; tokens], cache, vec![3], tokens as u64).unwrap()
}
pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 16, actor: actor(1, vec![2]), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::UnitsAtMost(16)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("helper".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 16, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"learned-durable".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 } }
}
pub(super) fn configuration(checked: &CheckedLearnedKv, weights: Vec<f32>, lifetime: LearnedMonitorBudget)
    -> FileLearnedConsistencyConfig
{
    let neutral = BinaryForecast::new(32768, 32768).unwrap();
    let base = super::super::super::FileConsistencyConfig::new(super::super::super::FileConsistencyParameters {
        probe_id: 1, probe_generation: 1, profile: checked.row_shape(row()).unwrap().0.profile,
        weights, bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10,
    }).unwrap();
    FileLearnedConsistencyConfig::new(base, 1, KvSide::Key, LearnedMonitorBudget::default(),
        lifetime, MAX_CHECKED_KV_BYTES).unwrap()
}
pub(super) fn owner(root: &Directory, config: FileLearnedConsistencyConfig)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver)
{
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, reviewer, observer)
}
pub(super) fn predict(host: &mut FileOversight, observer: &FileConsistencyObserver,
    source: &CheckedLearnedKv) -> Result<Result<LearnedForecastReport, Error>, JournalError>
{
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_learned_action(host, revision, 1, actor, source, row())
}
pub(super) fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
pub(super) fn proposal(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"risk".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(50), units: 16 }
}
pub(super) fn disk_sources(root: &Directory, checked: &CheckedLearnedKv) -> BTreeMap<u64, CheckedLearnedKv> {
    FileOversight::learned_consistency_recovery_requirements(root.store(), &profile()).unwrap()
        .into_iter().map(|requirement| (requirement.journal_revision, checked.clone())).collect()
}
pub(super) fn inputs(host: &FileOversight, action: &FrozenAction) -> CommitteeInput {
    let contracts = host.machine.broker.contracts(); let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let input = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), view)])).unwrap()
}
pub(super) fn review(host: &mut FileOversight, action: &FrozenAction) -> CommitteeInput {
    let input = inputs(host, action);
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, snapshot()).unwrap();
    let digest = crate::round::commitment(101, "helper", &[1; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "helper", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "helper", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap(); input
}
