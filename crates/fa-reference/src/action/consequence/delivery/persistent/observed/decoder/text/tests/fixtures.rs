use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-durable-text-{}-{tick}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("durable text cleanup: {error}"); }
    }
}
pub(super) fn model_profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 259, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap()
}
pub(super) fn tokenizer(reverse: bool) -> ByteBpe {
    use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{TokenBytes, Merge};
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Content((if reverse { b"ba" } else { b"ab" }).to_vec()),
        TokenBytes::Content("é".as_bytes().to_vec())]);
    ByteBpe::new(model_profile(), vocabulary, vec![
        Merge { left: if reverse { 98 } else { 97 }, right: if reverse { 97 } else { 98 }, result: 257 },
        Merge { left: 0xc3, right: 0xa9, result: 258 },
    ]).unwrap()
}
pub(super) fn config(threshold: f32, output_token: u32) -> FileDecoderConfig {
    // Actual weights: attention/MLP are zero, so residuals preserve embeddings.
    // A has [2,0]; every other token has [1,0]. Changing only the threshold makes
    // the SAME computed sampled token hold. No caller-supplied verdict is used.
    let mut embeddings: Vec<f32> = (0..259).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[65 * 2] = 2.0;
    let mut head = vec![0.0_f32; 259 * 2]; head[output_token as usize * 2] = 1.0;
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![259, 2], embeddings)),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0; 2])),
        ("lm_head.weight".to_owned(), (vec![259, 2], head)),
    ]);
    for suffix in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2], vec![1.0; 2]));
    }
    for suffix in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2, 2], vec![0.0; 4]));
    }
    let mut payload = Vec::new(); let mut fields = Vec::new();
    for (name, (shape, values)) in tensors {
        let start = payload.len();
        for value in values { payload.extend_from_slice(&value.to_le_bytes()); }
        fields.push(format!(r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#, payload.len()));
    }
    let mut header = format!("{{{}}}", fields.join(","));
    while header.len() % 8 != 0 { header.push(' '); }
    let mut weights = (header.len() as u64).to_le_bytes().to_vec();
    weights.extend_from_slice(header.as_bytes()); weights.extend(payload);
    let monitor = r#"{"schema":"fa.decoder-monitor/1","generation":1,
      "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
      "budget":{"encoded_bytes":10000,"probe_coordinates":10000},
      "layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":10000,"probe_coordinates":10000},
        "probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":THRESHOLD}]}]}"#
        .replace("THRESHOLD", &threshold.to_string()).into_bytes();
    let sampling = br#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":259,
      "temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#.to_vec();
    FileDecoderConfig::new(model_profile(), weights, monitor, sampling,
        5, DecoderBindingLimits::default()).unwrap()
}
pub(super) fn host_profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"durable-text-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
pub(super) fn owner_with_profile(root: &Directory, configuration: &FileDecoderConfig, profile: FileOversightProfile) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), profile).unwrap();
    host.enable_decoder(host.revision(), configuration.clone()).unwrap();
    host.enable_decoder_tokenizer(host.revision(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}
pub(super) fn owner(root: &Directory, configuration: &FileDecoderConfig) -> FileOversight {
    owner_with_profile(root, configuration, host_profile())
}
pub(super) fn request(prompt: &[u8], new: usize) -> TextGenerationRequest {
    use crate::action::consequence::activation::monitor::decoder::sampled::generation::{GenerationBudget, tokenizer::TokenizationBudget};
    TextGenerationRequest { prompt: prompt.to_vec(), prefix_controls: vec![256],
        max_new_tokens: new, stop_tokens: vec![256], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: new * 2 }
}
pub(super) fn command(host: &FileOversight, id: u64, request: TextGenerationRequest) -> FileTextGenerationCommand {
    let n = host.decoder_inspection().unwrap().numerical;
    FileTextGenerationCommand::new(id, n.actor_revision, n.position, request).unwrap()
}
pub(super) fn bytes(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
