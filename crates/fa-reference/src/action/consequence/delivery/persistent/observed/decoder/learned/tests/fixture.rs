use super::super::*;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::FileHumanRequest;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy, learned_source::LearnedSourceConfig};
use crate::action::consequence::activation::monitor::learned::{LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::{decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights,
    DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}},
    experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-durable-learned-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("learned test cleanup: {error}"); } }
}
pub(super) fn model() -> DecoderModel { model_with_output(0.0) }
pub(super) fn model_with_output(output: f32) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4],
        values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![output, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
pub(super) fn source(model: &DecoderModel, alarm: bool, top_k: usize) -> LearnedSourceConfig {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 2 && side == KvSide::Value { weights[1] = 1.0; 0.5 } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side }, LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap();
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}
pub(super) fn config(alarm: bool, top_k: usize) -> FileLearnedConfig {
    let model = model(); let source = source(&model, alarm, top_k);
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
}
pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
pub(super) fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
pub(super) fn owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config.clone()).unwrap();
    (host, reviewer)
}
pub(super) fn step(host: &mut FileOversight) -> Result<Rc<GenerationEvent>, JournalError> {
    let n = host.learned_generation_inspection()?.numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position)?.map_err(Into::into)
}
pub(super) fn action_spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
pub(super) fn read_machine(host: &FileOversight, config: &FileLearnedConfig) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    bind_history(&mut events, config).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}
pub(super) fn prepared(host: &mut FileOversight) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest) {
    let action = host.propose(host.revision(), 1, action_spec(host), snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
    (action, inputs, automatic, request)
}
