use super::*;
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::monitor::learned::{LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvRow};
use crate::action::consequence::activation::tensor::kv::{decoder::{DecoderBudget, DecoderIdentity,
    DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}},
    experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, helper_workers::HelperLimits,
    human::HumanReviewPolicy, learned_source::LearnedSourceConfig,
    learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
    sidecar::{SidecarCongressBudget, SidecarIdentity, probe_helper::ProbeHelperBudget}};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-durable-probes-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("durable probe cleanup: {error}"); }
    }
}

pub(super) fn config() -> FileLearnedConfig {
    // Same history-sensitive decoder and quiet policy as learned_probe_review.
    // Its sampler emits off-axis token 2 after prompt 0; the codec was fitted
    // only on 0 and 1, so the separate helper's first-coordinate probe MUST
    // request a retained residual before certifying the quiet second position.
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![1.0, 0.0, 0.0, 1.0],
        keys: vec![1.0, 0.0, 0.0, 1.0], values: vec![1.0, 0.0, 0.0, 1.0],
        attention_output: vec![0.25, 0.0, 0.0, 0.25], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    let model = DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let probe = LinearProbe::new(1, 1, tensor.profile(), &vec![0.0; tensor.dimensions()], 0.0, 1.0).unwrap();
            taps.insert(KvTap { layer: *layer, side }, LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps,
        LearnedAuditBudget { rows: 4, ..LearnedAuditBudget::default() }).unwrap();
    let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap();
    let source = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap()
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
            congress: CongressPolicy { generation: 1,
                members: ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
                    MemberPolicy { cohort: member.to_owned(), weight: 1 })).collect(),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1,
                b"registered numerical probes; uncertain means abstain".to_vec()).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 100, max_requests: 8 },
    }
}
pub(super) fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) fn owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config.clone()).unwrap();
    (host, reviewer)
}
pub(super) fn step(host: &mut FileOversight) {
    let state = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), state.actor_revision, state.position).unwrap().unwrap();
}
pub(super) fn propose(host: &mut FileOversight) -> (FrozenAction, FileLearnedSidecar) {
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let evidence = host.machine.broker.learned_decoder_evidence(1).unwrap().unwrap();
    let request = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
        priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default() };
    let revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let sidecar = host.begin_learned_sidecar_plan(host.revision(), 1, revision, request).unwrap();
    (action, sidecar)
}
fn probes(source: &CheckedLearnedKv, mode: u8) -> BTreeMap<KvRow, Vec<LinearProbe>> {
    let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
    rows.into_iter().map(|row| {
        let (frame, heads, channels) = source.row_shape(row).unwrap();
        let mut weights = vec![0.0; heads * channels];
        let threshold = if mode > 0 && row.layer == 1 && row.side == KvSide::Value {
            weights[usize::from(mode == 2)] = 1.0; 0.5
        } else { 1.0 };
        (row, vec![LinearProbe::new(1, 1, frame.profile, &weights, 0.0, threshold).unwrap()])
    }).collect()
}
pub(super) fn members(host: &FileOversight, sidecar: &FileLearnedSidecar, mode: u8) -> BTreeMap<String, ProbeReviewMember> {
    let original = host.checked_learned_sidecar(sidecar).unwrap();
    ["alpha", "beta"].into_iter().enumerate().map(|(index, member)| (member.to_owned(), ProbeReviewMember {
        probes: probes(original.source(), mode), salts: (0..5).map(|round| vec![16 + (index * 5 + round) as u8; 32]).collect(),
    })).collect()
}
pub(super) fn schedule() -> LearnedWorkerSchedule {
    LearnedWorkerSchedule { rounds: (0..5).map(|i| LearnedWorkerRound { round: 101 + i, evidence_root: [7; 32],
        window: ReviewWindow { commit_by: ElapsedTick(10 + 15 * i), reveal_by: ElapsedTick(15 + 15 * i) } }).collect(),
        helpers: HelperLimits::default(), polls: 64 }
}
pub(super) fn limits() -> ProbeReviewLimits {
    ProbeReviewLimits { evaluations: 10, per_evaluation: ProbeHelperBudget::default() }
}
pub(super) fn poll(host: &mut FileOversight, run: &mut FileLearnedProbeReview, now: u64) -> Result<FileLearnedProbeStatus, JournalError> {
    run.advance(host, run.revision(), ElapsedTick(now), snapshot())
}
pub(super) fn drive(host: &mut FileOversight, run: &mut FileLearnedProbeReview) -> FileLearnedProbeStatus {
    for _ in 0..64 {
        if run.status() != FileLearnedProbeStatus::Running { return run.status(); }
        poll(host, run, 1).unwrap();
    }
    panic!("durable computed review exceeded its fixed schedule");
}
