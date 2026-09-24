//! Synthetic weights, real native inference and real registered source files.
use super::*;
use crate::action::{Purpose, Scope};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
    tokenizer::{ByteBpe, Merge, TokenBytes, TokenizationBudget},
};
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversightProfile, decoder::{FileDecoderConfig, text::FileTextGenerationCommand},
    source::FileSourcePolicy, stream::generated::FileTextMessageRequest,
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use crate::action::consequence::oversight::evidence_source::{EvidenceSnapshot, FileEvidenceSource};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-generated-source-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn source(&self) -> PathBuf { self.0.join("evidence.json") }
    pub(super) fn replace(&self, capture: &EvidenceSnapshot) {
        let staged = self.0.join("evidence.next");
        std::fs::write(&staged, capture.encode()).unwrap();
        std::fs::rename(staged, self.source()).unwrap();
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("generated source cleanup: {error}"); }
    }
}

pub(super) fn profile() -> FileOversightProfile {
    let target = crate::action::ResolvedTarget {
        adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1,
    };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 4096, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(4096),
            Predicate::ExactValue { key: 7, value: b"allow".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: Vec::new(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"source-intake".to_vec(),
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
pub(super) fn capture(generation: u64, complete: bool, value: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { scope: profile().delivery.scope, source: 7, generation },
        Snapshot { semantic_epoch: 1, complete, values: BTreeMap::from([(7, value.to_vec())]) },
        BTreeMap::from([("reviewer".into(), b"private reviewer context".to_vec())])).unwrap()
}
pub(super) fn source_policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 7, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(10).unwrap() }
}
fn model_profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 259, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap()
}
fn model() -> FileDecoderConfig {
    // The content embedding changes the readout to EOS on the following sample.
    // Neither an output ID nor a completed report is supplied to the generator.
    let mut embeddings: Vec<f32> = (0..259).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[65 * 2] = 0.0; embeddings[65 * 2 + 1] = 1.0;
    let mut head = vec![0.0_f32; 259 * 2]; head[65 * 2] = 1.0; head[256 * 2 + 1] = 1.0;
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![259, 2], embeddings)),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0_f32; 2])),
        ("lm_head.weight".to_owned(), (vec![259, 2], head)),
    ]);
    for name in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{name}"), (vec![2], vec![1.0; 2]));
    }
    for name in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"] {
        tensors.insert(format!("model.layers.0.{name}"), (vec![2, 2], vec![0.0; 4]));
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
    let monitor = br#"{"schema":"fa.decoder-monitor/1","generation":1,
      "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
      "budget":{"encoded_bytes":10000,"probe_coordinates":10000},
      "layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":10000,"probe_coordinates":10000},
        "probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":3.0}]}]}"#.to_vec();
    let sampling = br#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":259,
      "temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#.to_vec();
    FileDecoderConfig::new(model_profile(), weights, monitor, sampling, 5, DecoderBindingLimits::default()).unwrap()
}
fn tokenizer() -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content("é".as_bytes().to_vec())]);
    ByteBpe::new(model_profile(), vocabulary, vec![Merge { left: 97, right: 98, result: 257 },
        Merge { left: 0xc3, right: 0xa9, result: 258 }]).unwrap()
}

pub(super) struct Setup {
    pub root: Root,
    pub source: FileEvidenceSource,
    pub port: FileGeneratedTextActorPort,
    pub supervisor: FileActorSupervisor<FileOversight>,
    pub reviewer: FileHumanReviewer,
    pub input: FileTextMessageRequest,
    pub configuration: FileDecoderConfig,
    pub tokenizer: ByteBpe,
}
pub(super) fn setup() -> Setup {
    let root = Root::new(); root.replace(&capture(1, true, b"allow"));
    let configuration = model(); let tokenizer = tokenizer();
    let (mut host, reviewer) = FileOversight::create_generated_text_stream(root.store(), profile(),
        StreamProfile::new(9, 1, 4, 64, 256).unwrap(), configuration.clone(), tokenizer.clone()).unwrap();
    host.enable_file_source(host.revision(), source_policy()).unwrap();
    let mut source = FileEvidenceSource::new(root.source(), 7, profile().delivery.scope, 4096).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let request = TextGenerationRequest { prompt: b"ab".to_vec(), prefix_controls: vec![256],
        max_new_tokens: 2, stop_tokens: vec![256], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 4 };
    let command = FileTextGenerationCommand::new(7, n.actor_revision, n.position, request).unwrap();
    let generated = host.generate_decoder_text(host.revision(), command).unwrap();
    assert_eq!(generated.result().unwrap().bytes().unwrap(), b"A");
    assert_eq!(generated.result().unwrap().generation().finish(), GenerationFinish::StopToken);
    let input = FileTextMessageRequest { request: 91, generation: 7,
        generation_revision: host.decoder_generation_progress(7).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100) };
    let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    Setup { root, source, port, supervisor, reviewer, input, configuration, tokenizer }
}
pub(super) fn document(input: &FileTextMessageRequest) -> Vec<u8> {
    encode_command(&Command::Submit { request: input.request,
        proposal: FileGeneratedTextActorPort::encode_message(input).unwrap() }).unwrap()
}
pub(super) fn disk(s: &Setup) -> Vec<u8> { std::fs::read(s.root.store().join("delivery.bin")).unwrap() }
