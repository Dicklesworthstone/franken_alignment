//! Test-only external artifacts; no production fitting or monitoring defaults.
use super::super::{Config, PeerProfile, clock};
use super::models;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, MAX_DECODER_PRODUCTS},
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy, replay::archive::MAX_FIT_ARCHIVE_BYTES},
};
use fa_reference::action::consequence::oversight::{
    actor_peer::PeerCredentials, policy_state::StateLimits,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, fs, path::PathBuf,
    os::unix::{fs::DirBuilderExt, net::UnixStream},
    sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(pub PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-learned-cli-{}-{}-{}",
            std::process::id(), clock().0, NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new().mode(0o750).create(path.join("peers")).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("learned command cleanup: {error}"); }
    }
}
pub(super) fn configured(root: &Root) -> Config {
    let mut config = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))).unwrap();
    config.store = root.0.join("store");
    config.profile.delivery.initial_payload.clear();
    config.source_policy.limits = StateLimits { events: 1024, retained_bytes: 1_048_576 };
    config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51,
        config.profile.delivery.scope, 1_048_576).unwrap();
    config.timing.poll_ms = 1; config.timing.runtime_ms = 60_000; config.timing.cleanup_ms = 1000;
    // The command must use the admitted native owners, never an ordinary
    // process helper fallback. This map cannot produce any permitting ballot.
    config.programs.clear();
    config
}
pub(super) fn evidence(root: &Root, config: &Config, generation: u64, allowed: bool, contexts: bool) {
    let observed = EvidenceSnapshot::new(EvidenceIdentity { scope: config.profile.delivery.scope,
        source: 51, generation },
        Snapshot { semantic_epoch: generation, complete: true,
            values: BTreeMap::from([(7, if allowed { b"ok".to_vec() } else { b"blocked".to_vec() })]) },
        ["alpha", "beta"].into_iter().map(|name|
            (name.to_owned(), if contexts { b"alternate helper content".to_vec() } else { Vec::new() })).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), observed.encode()).unwrap();
}
pub(super) fn peers(root: &Root, config: &Config) -> PeerProfile {
    let (socket, _client) = UnixStream::pair().unwrap();
    let p = PeerCredentials::observe(&socket).unwrap(); let s = config.profile.delivery.scope;
    PeerProfile::decode(format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority, config.profile.human.reviewer_id,
        root.0.display(), p.uid(), p.gid(), p.pid(), p.uid(), p.gid(), p.pid()).as_bytes()).unwrap()
}

pub(super) fn write(root: &Root, spelling: &[u8]) -> PathBuf {
    let model = models::actor(&root.0);
    // Constant V rows reconstruct exactly; the original rotating K rows remain compressed.
    let source = model.recompute(11, &[120, 120, 120, 120], DecoderBudget { scalar_products: 100_000 })
        .unwrap().cache_image().unwrap();
    let policy = LearnedKvPolicy::new(1, 1, 1, 8).unwrap();
    let budget = FitBudget { source_values: 64, parameter_values: 64, scratch_values: 16, work_units: 10_000 };
    let (_codec, checkpoint) = LearnedKvCodec::fit_with_checkpoint(policy,
        &BTreeMap::from([(101, source)]), budget).unwrap();
    // Retain the expected inventory from ORIGINAL training at this independent
    // provisioning boundary. The command never derives it from an intake archive.
    let binding = checkpoint.binding();
    let descriptor = binding.sources[&101].encode().unwrap().iter()
        .map(|byte| format!("{byte:02x}")).collect::<String>();
    fs::write(root.0.join("fit-binding.json"), format!(r#"{{"schema":"fa.learned-fit-binding/1","policy":{{"id":1,"generation":1,"rank":1,"sweeps":8}},"budget":{{"source_values":64,"parameter_values":64,"scratch_values":16,"work_units":10000}},"sources":[{{"origin":101,"descriptor_hex":"{descriptor}"}}]}}"#)).unwrap();
    fs::write(root.0.join("fit.fakv"), checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap()).unwrap();
    fs::write(root.0.join("learned-monitor.json"), monitoring(100.0)).unwrap();
    let stop = models::native(&root.0, spelling);
    let r = root.0.display();
    let members = ["alpha", "beta"].into_iter().enumerate().map(|(index, name)| {
        let stream = 71 + index;
        format!(r#""{name}":{{"model":{{"identity":{{"tenant":1,"model":31,"model_generation":1,"tokenizer_generation":1,"profile_generation":1}},"context":4096,"stream":{stream}}},"files":{{"model_config":"{r}/helper-model.json","weights":"{r}/helper-weights.safetensors","monitor":"{r}/helper-monitor.json","sampling":"{r}/helper-sampling.json","tokenizer":{{"format":"native_archive","path":"{r}/helper-tokenizer.bbpe"}}}},"text":{{"max_new_tokens":2,"max_output_bytes":128,"stop_tokens":[{stop}],"tokenization":{{"input_bytes":65536,"pair_lookups":196608,"heap_pops":196608}},"scalar_products":{MAX_DECODER_PRODUCTS},"sampling_entries":4096}}}}"#)
    }).collect::<Vec<_>>().join(",");
    fs::write(root.0.join("native-roster.json"),
        format!(r#"{{"schema":"fa.learned-native-roster/1","members":{{{members}}}}}"#)).unwrap();
    let path = root.0.join("recipe.json");
    fs::write(&path, recipe(root)).unwrap();
    path
}
pub(super) fn monitoring(threshold: f32) -> String {
    let budget = r#"{"encoded_bytes":65536,"probe_coordinates":10000,"reconstruction_products":1000000,"materialized_values":256,"refinements":64}"#;
    let taps = ["key", "value"].into_iter().map(|side|
        format!(r#"{{"layer":1,"side":"{side}","budget":{budget},"probes":[{{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":{threshold}}}]}}"#))
        .collect::<Vec<_>>().join(",");
    format!(r#"{{"schema":"fa.learned-kv-monitor/1","retention":"all","inference_products":1000000,"preparation":{{"compression":{{"source_values":1000,"encoded_bytes":65536,"work_units":1000000}},"source_check":{{"source_values":1000,"encoded_bytes":65536,"reconstruction_products":1000000}}}},"audit":{{"rows":2,"budget":{budget}}},"taps":[{taps}]}}"#)
}
pub(super) fn recipe(root: &Root) -> String {
    let r = root.0.display();
    let products = MAX_DECODER_PRODUCTS * 2;
    format!(r#"{{"schema":"fa.learned-publication/1","request":1,"ttl_ms":60000,"asset_bytes":4194304,"model":{{"identity":{{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1}},"context":16,"stream":5,"evaluation_origin":201,"monitor_generation":1}},"publication_stream":{{"id":9,"generation":1,"max_messages":1,"max_message_bytes":64,"max_total_bytes":64}},"binding":{{"token_ids":4096,"score_words":10000,"encoded_bytes":1048576}},"recovery_floor":{{"journal_revision":0,"control_sequence":0,"authority_epoch":0}},"files":{{"model_config":"{r}/model.json","weights":"{r}/weights.safetensors","tokenizer":{{"format":"huggingface_raw_bytelevel","path":"{r}/tokenizer.json"}},"sampling":"{r}/sampling.json","prompt":"{r}/prompt.txt","fit_binding":"{r}/fit-binding.json","fit_archive":"{r}/fit.fakv","monitor":"{r}/learned-monitor.json","native_roster":"{r}/native-roster.json"}},"text":{{"max_new_tokens":2,"max_output_bytes":64,"stop_tokens":[256],"completion":"stop_required","tokenization":{{"input_bytes":65536,"pair_lookups":196608,"heap_pops":196608}},"decoder_products":1000000,"vocabulary_scores":1024,"telemetry":{{"compression_source_values":1000000,"compression_encoded_bytes":1000000,"compression_work_units":1000000,"source_check_values":1000000,"source_check_encoded_bytes":1000000,"source_check_reconstruction_products":1000000,"monitor_encoded_bytes":1000000,"monitor_probe_coordinates":1000000,"monitor_reconstruction_products":1000000,"monitor_materialized_values":1000000,"monitor_refinements":1000}}}},"sidecar":{{"identity":{{"object_id":1001,"generation":1,"transform_id":7}},"budget":{{"rounds":1,"residual_bytes":65536,"committee_bytes":1048576}}}},"native_review":{{"polls":4096,"probe_coordinates":10000,"reconstruction_products":1000000,"evaluations":2,"scalar_products":{products},"sampling_entries":8192}}}}"#)
}
