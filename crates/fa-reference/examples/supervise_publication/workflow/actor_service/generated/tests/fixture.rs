//! Synthetic weights and helper decisions; real constructors, files and sockets.
use super::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, tokenizer::{ByteBpe, TokenBytes, TokenizationBudget}, text::TextGenerationRequest,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(in crate::workflow::actor_service::generated) struct Root(pub PathBuf);
impl Root {
    pub(in crate::workflow::actor_service::generated) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-native-pub-{}-{}-{}",
            std::process::id(), clock().0, NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new().mode(0o750).create(path.join("peers")).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("native publication cleanup: {error}"); } }
}
pub(in crate::workflow::actor_service::generated) fn configured(root: &Root) -> Config {
    let mut c = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))).unwrap();
    c.store = root.0.join("store");
    c.profile.delivery.initial_payload.clear();
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1_048_576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15_000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap();
        (name.to_owned(), program)
    }).collect();
    let observed = EvidenceSnapshot::new(EvidenceIdentity { scope: c.profile.delivery.scope, source: 51, generation: 1 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), format!("review this fixture: {name}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), observed.encode()).unwrap();
    c
}
pub(in crate::workflow::actor_service::generated) fn peers(root: &Root, config: &Config) -> PeerProfile {
    let (socket, _client) = UnixStream::pair().unwrap();
    let p = PeerCredentials::observe(&socket).unwrap(); let s = config.profile.delivery.scope;
    PeerProfile::decode(format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority, config.profile.human.reviewer_id,
        root.0.display(), p.uid(), p.gid(), p.pid(), p.uid(), p.gid(), p.pid()).as_bytes()).unwrap()
}
pub(in crate::workflow::actor_service::generated) fn model_profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 257,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap()
}
pub(in crate::workflow::actor_service::generated) fn tokenizer() -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.push(TokenBytes::Control);
    ByteBpe::new(model_profile(), vocabulary, Vec::new()).unwrap()
}
pub(in crate::workflow::actor_service::generated) fn weights() -> Vec<u8> {
    let mut embeddings: Vec<f32> = (0..257).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[65 * 2] = 0.0; embeddings[65 * 2 + 1] = 1.0;
    let mut head = vec![0.0_f32; 514]; head[65 * 2] = 1.0; head[256 * 2 + 1] = 1.0;
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![257, 2], embeddings)),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0_f32; 2])),
        ("lm_head.weight".to_owned(), (vec![257, 2], head)),
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
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes()); bytes.extend(payload); bytes
}
pub(in crate::workflow::actor_service::generated) fn monitor(held: bool) -> Vec<u8> {
    format!(r#"{{"schema":"fa.decoder-monitor/1","generation":1,"identity":{{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1}},"budget":{{"encoded_bytes":10000,"probe_coordinates":10000}},"layers":[{{"layer":1,"levels":[23],"budget":{{"encoded_bytes":10000,"probe_coordinates":10000}},"probes":[{{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":{}}}]}}]}}"#,
        if held { -3.0 } else { 3.0 }).into_bytes()
}
pub(in crate::workflow::actor_service::generated) fn sampling() -> Vec<u8> {
    br#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":257,"temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#.to_vec()
}
pub(in crate::workflow::actor_service::generated) fn loaded(held: bool) -> Loaded {
    Loaded { request: 1, generation: 7, ttl_ms: 100000,
        decoder: FileDecoderConfig::new(model_profile(), weights(), monitor(held), sampling(),
            5, DecoderBindingLimits::default()).unwrap(), tokenizer: tokenizer(),
        stream: StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
        text: TextGenerationRequest { prompt: b"x".to_vec(), prefix_controls: vec![256],
            max_new_tokens: 2, stop_tokens: vec![256], max_output_bytes: 64,
            tokenization: TokenizationBudget::default(), generation: GenerationBudget {
                scalar_products: 1_000_000, sampling_entries: 1024 } } }
}
pub(in crate::workflow::actor_service::generated) fn write_recipe(root: &Root) -> PathBuf {
    fs::write(root.0.join("model.json"), br#"{"model_type":"llama","vocab_size":257,"hidden_size":2,"intermediate_size":2,"num_hidden_layers":1,"num_attention_heads":1,"num_key_value_heads":1,"max_position_embeddings":16,"rms_norm_eps":0.00001,"rope_theta":10000.0}"#).unwrap();
    fs::write(root.0.join("weights.safetensors"), weights()).unwrap();
    fs::write(root.0.join("monitor.json"), monitor(false)).unwrap();
    fs::write(root.0.join("sampling.json"), sampling()).unwrap();
    fs::write(root.0.join("tokenizer.bbpe"), tokenizer().to_bytes().unwrap()).unwrap();
    fs::write(root.0.join("prompt.txt"), b"x").unwrap();
    let path = root.0.join("recipe.json"); let r = root.0.display();
    let limits = DecoderBindingLimits::default();
    let token_ids = limits.token_ids; let score_words = limits.score_words;
    fs::write(&path, format!(r#"{{"schema":"fa.generated-publication/1","request":1,"generation":7,"ttl_ms":100000,"model":{{"identity":{{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1}},"context":16,"stream":5}},"publication_stream":{{"id":9,"generation":1,"max_messages":4,"max_message_bytes":64,"max_total_bytes":256}},"binding":{{"token_ids":{token_ids},"score_words":{score_words}}},"files":{{"model_config":"{r}/model.json","weights":"{r}/weights.safetensors","monitor":"{r}/monitor.json","sampling":"{r}/sampling.json","tokenizer":"{r}/tokenizer.bbpe","prompt":"{r}/prompt.txt"}},"text":{{"prefix_controls":[256],"stop_tokens":[256],"max_new_tokens":2,"max_output_bytes":64,"scalar_products":1000000,"sampling_entries":1024,"tokenization":{{"input_bytes":65536,"pair_lookups":196608,"heap_pops":196608}}}}}}"#)).unwrap();
    path
}
