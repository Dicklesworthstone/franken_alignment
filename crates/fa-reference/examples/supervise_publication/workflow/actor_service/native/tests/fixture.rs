use super::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{ByteBpe, TokenBytes, Merge};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderProfile, DecoderShape, DecoderIdentity};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceSnapshot, EvidenceIdentity, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(pub PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-native-{}-{}-{}", std::process::id(),
            crate::workflow::clock().0, NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("native fixture cleanup: {error}"); } }
}
pub(super) fn configured(root: &Root) -> Config {
    let mut config = Config::decode(include_bytes!("../../../../../../fixtures/supervised_publication.json")).unwrap();
    config.store = root.0.join("store"); config.profile.delivery.initial_payload.clear();
    config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, config.profile.delivery.scope, 1048576).unwrap();
    config.timing.runtime_ms = 15000; config.timing.poll_ms = 1; config.timing.cleanup_ms = 2000;
    config.programs = ["alpha", "beta"].into_iter().map(|name| (name.to_owned(),
        HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap())).collect();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { scope: config.profile.delivery.scope, source: 51, generation: 1 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    std::fs::write(root.0.join("evidence.json"), evidence.encode()).unwrap();
    config
}
pub(super) fn profiles(root: &Root, config: &Config) -> (Profile, PeerProfile) {
    let (socket, _client) = UnixStream::pair().unwrap(); let id = PeerCredentials::observe(&socket).unwrap();
    let s = config.profile.delivery.scope;
    let scope = format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority);
    let identity = format!(r#"{{"uid":{},"gid":{},"pid":{}}}"#, id.uid(), id.gid(), id.pid());
    let actor = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":91,
      "scope":{scope},"socket":"{}/actor.sock","supervisor":{identity},"actor":{identity},
      "candidate_limit":4,"connection_limit":4,"exchange_limit":4096,"runtime_ms":15000,"poll_ms":1,"reply_ms":20}}"#, root.0.display());
    let reviewer = format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{scope},"reviewer_id":77,
      "socket_directory":"{}","supervisor":{identity},"reviewer":{identity},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#, root.0.display());
    (Profile::decode(actor.as_bytes()).unwrap(), PeerProfile::decode(reviewer.as_bytes()).unwrap())
}
pub(super) fn inputs(root: &Root, config: &Config) -> Inputs {
    write_inputs(root);
    recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap()
}
pub(super) fn write_inputs(root: &Root) {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 259, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap();
    let mut embeddings: Vec<f32> = (0..259).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[130] = 0.0; embeddings[131] = 1.0;
    let mut head = vec![0.0_f32; 518]; head[130] = 1.0; head[513] = 1.0;
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
        let start = payload.len(); for value in values { payload.extend_from_slice(&value.to_le_bytes()); }
        fields.push(format!(r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#, payload.len()));
    }
    let mut header = format!("{{{}}}", fields.join(",")); while header.len() % 8 != 0 { header.push(' '); }
    let mut weights = (header.len() as u64).to_le_bytes().to_vec();
    weights.extend_from_slice(header.as_bytes()); weights.extend(payload);
    std::fs::write(root.0.join("model.safetensors"), weights).unwrap();
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Content(b"ab".to_vec()), TokenBytes::Content("é".as_bytes().to_vec())]);
    let tokenizer = ByteBpe::new(profile, vocabulary, vec![Merge { left: 97, right: 98, result: 257 },
        Merge { left: 195, right: 169, result: 258 }]).unwrap();
    std::fs::write(root.0.join("tokenizer.bbpe"), tokenizer.to_bytes().unwrap()).unwrap();
    for (name, text) in [
        ("model.json", r#"{"model_type":"llama","vocab_size":259,"hidden_size":2,"intermediate_size":2,
          "num_hidden_layers":1,"num_attention_heads":1,"num_key_value_heads":1,"max_position_embeddings":16,
          "rms_norm_eps":0.00001,"rope_theta":10000.0,"tie_word_embeddings":false}"#),
        ("monitor.json", r#"{"schema":"fa.decoder-monitor/1","generation":1,
          "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
          "budget":{"encoded_bytes":10000,"probe_coordinates":10000},
          "layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":10000,"probe_coordinates":10000},
          "probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":3.0}]}]}"#),
        ("sampling.json", r#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":259,
          "temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#),
        ("prompt.txt", "ab"),
        ("recipe.json", r#"{"schema":"fa.native-text-service/1",
          "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
          "context":16,"cache_stream":5,"generation":7,
          "stream":{"id":9,"generation":1,"max_messages":4,"max_message_bytes":64,"max_total_bytes":256},
          "prefix_controls":[256],"stop_tokens":[256],"max_new_tokens":2,"max_output_bytes":4,
          "scalar_products":1099511627776,"sampling_entries":16777216,
          "model_config":"model.json","weights":"model.safetensors","monitor":"monitor.json",
          "sampling":"sampling.json","tokenizer":"tokenizer.bbpe","prompt":"prompt.txt"}"#),
    ] { std::fs::write(root.0.join(name), text).unwrap(); }
}
