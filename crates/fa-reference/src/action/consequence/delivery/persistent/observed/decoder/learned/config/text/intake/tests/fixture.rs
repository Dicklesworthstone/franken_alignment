//! Synthetic weights exercise actual ByteBpe, inference, learned codec and gate.
use super::*;
use crate::action::consequence::activation::monitor::learned::{LearnedMonitorBudget,
    LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, TokenBytes, TokenizationBudget};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::{decoder::{DecoderBudget, DecoderIdentity,
    DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationTelemetryBudget}}},
    experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::text::{LearnedTextConfig, LearnedTextOutputPolicy, LearnedTextCompletion}};
use crate::action::{Purpose, Scope};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-text-intake-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("learned intake cleanup: {error}"); }
    }
}

pub(super) fn recipe() -> (DecoderModel, ByteBpe, LearnedTextConfig) {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 256, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let tokenizer = ByteBpe::new(profile.clone(),
        (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect(), Vec::new()).unwrap();
    let mut embeddings = vec![0.0; 512];
    for id in 0..256 { embeddings[id * 2] = 1.0; }
    embeddings[2] = -1.0;
    let mut output = vec![0.0; 512]; output[usize::from(b'a') * 2] = 10.0;
    let model = DecoderModel::new(profile, embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![1.0, 0.0, 0.0, 1.0],
        values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
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
        LearnedAuditBudget { rows: 2, ..LearnedAuditBudget::default() }).unwrap();
    let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap();
    let source = LearnedTextConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        prompt: "p".to_owned(), tokenization: TokenizationBudget::default(), max_new_tokens: 2,
        stop_tokens: BTreeSet::new(), sampling: SamplingStart {
            policy: SamplingPolicy::new(1, 1, 256, 1.0, 1, 1.0).unwrap(), stream: 71, seed: 173 },
        policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default(),
        output: LearnedTextOutputPolicy { max_bytes: 16, completion: LearnedTextCompletion::StopOrTokenLimit } };
    (model, tokenizer, source)
}
pub(super) fn config() -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    FileLearnedConfig::new_text(model, tokenizer, source, LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap()
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
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve exact generated message?".to_vec()).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 100, max_requests: 8 },
    }
}
pub(super) fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) fn step(host: &mut FileOversight) {
    let state = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), state.actor_revision, state.position).unwrap().unwrap();
}
pub(super) fn owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 { step(&mut host); }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    (host, reviewer)
}
pub(super) fn proposal() -> LearnedTextProposal {
    LearnedTextProposal { target: profile().delivery.target, expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
pub(super) fn observe(supervisor: &mut FileActorSupervisor<FileOversight>, snapshot: Snapshot) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot)).unwrap();
}
