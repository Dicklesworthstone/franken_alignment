//! File encodings of the independent original synthetic GQA fixture.
use super::original;
use fa_reference::action::consequence::activation::{
    monitor::learned::{LearnedMonitorBudget, model::LearnedAuditBudget},
    probe::training::{CaseLabel, DataSplit, decoder::kv::KvMonitorSettings},
    tensor::kv::{experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
        replay::archive::MAX_FIT_ARCHIVE_BYTES}}};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(pub PathBuf);
impl Root {
    pub fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-train-kv-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("K/V command fixture cleanup: {error}"); } }
}

pub(super) fn settings() -> KvMonitorSettings {
    let mut settings = original::settings(&original::model());
    let budget = LearnedMonitorBudget { encoded_bytes: 65536, probe_coordinates: 10000,
        reconstruction_products: 1000000, materialized_values: 512, refinements: 16 };
    for (index, value) in settings.taps.values_mut().enumerate() {
        *value = budget;
        value.probe_coordinates += index; // Preserve distinct real per-tap caps.
    }
    settings.audit = LearnedAuditBudget { rows: 4, monitoring: budget };
    settings.preparation.compression.source_values = 1024;
    settings.preparation.compression.encoded_bytes = 65536;
    settings.preparation.compression.work_units = 1000000;
    settings.preparation.source_check.source_values = 1024;
    settings.preparation.source_check.encoded_bytes = 65536;
    settings.preparation.source_check.reconstruction_products = 1000000;
    settings.inference.scalar_products = 1000000;
    settings
}
fn budget(b: LearnedMonitorBudget) -> String {
    format!(r#"{{"encoded_bytes":{},"probe_coordinates":{},"reconstruction_products":{},"materialized_values":{},"refinements":{}}}"#,
        b.encoded_bytes, b.probe_coordinates, b.reconstruction_products, b.materialized_values, b.refinements)
}
pub(super) fn campaign_json() -> String {
    let model = original::model(); let cases = original::cases(); let policies = original::policies();
    let corpus = original::trained::capture(&model, &cases);
    let capture = corpus.work(); let campaign = corpus.estimate_campaign(&policies).unwrap();
    let settings = settings();
    let cases = cases.iter().map(|case| {
        let split = match case.split { DataSplit::Training => "training", DataSplit::Calibration => "calibration", DataSplit::Evaluation => "evaluation" };
        let label = match case.label { CaseLabel::Benign => "benign", CaseLabel::Violation => "violation" };
        format!(r#"{{"task":{},"lineage":{},"split":"{split}","label":"{label}","tokens":{:?}}}"#,
            case.origin.task, case.origin.lineage, case.tokens)
    }).collect::<Vec<_>>().join(",");
    let taps = policies.iter().map(|(tap, p)| {
        let side = match tap.side { KvSide::Key => "key", KvSide::Value => "value" };
        format!(r#"{{"layer":{},"side":"{side}","fit":{{"id":{},"generation":1,"epochs":64,"learning_rate":0.25,"l2":0.01,"scale_floor":0.001}},"calibration":{{"id":{},"generation":1,"thresholds":[-10.0,0.0,10.0],"calibration":{{"min_violation_alarms":1,"max_benign_holds":0}},"evaluation":{{"min_violation_alarms":1,"max_benign_holds":0}}}},"monitor_budget":{}}}"#,
            tap.layer, p.fit.id(), p.calibration.id(), budget(settings.taps[tap]))
    }).collect::<Vec<_>>().join(",");
    format!(r#"{{"schema":"fa.learned-kv-probe-campaign/1","id":1,"generation":1,"identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},"context":8,"tokenizer_format":"native_archive","input_limits":{{"auxiliary_bytes":1048576,"weight_bytes":1048576,"weight_read_calls":10000,"codec_bytes":1048576,"codec_read_calls":10000}},"output_limits":{{"report_bytes":1048576,"monitor_bytes":2097152}},"capture_budget":{{"cases":{},"original_tokens":{},"kv_coordinates":{},"scalar_products":{}}},"campaign_budget":{{"training_visits":{},"scoring_bytes":{},"scoring_coordinates":{},"threshold_comparisons":{}}},"monitor":{{"retention":"all","inference_products":1000000,"preparation":{{"compression":{{"source_values":1024,"encoded_bytes":65536,"work_units":1000000}},"source_check":{{"source_values":1024,"encoded_bytes":65536,"reconstruction_products":1000000}}}},"audit":{{"rows":4,"budget":{}}}}},"cases":[{cases}],"taps":[{taps}]}}"#,
        capture.cases, capture.original_tokens, capture.kv_coordinates, capture.scalar_products,
        campaign.training_visits, campaign.scoring_bytes, campaign.scoring_coordinates, campaign.threshold_comparisons,
        budget(settings.audit.monitoring))
}

pub(super) fn files(root: &Root) -> Vec<OsString> {
    let args: Vec<_> = ["config.json", "weights.safetensors", "tokenizer.bbpe", "campaign.json",
        "codec-binding.json", "codec.fakv", "report.ndjson", "monitor.fakvmon"]
        .into_iter().map(|name| root.0.join(name).into_os_string()).collect();
    fs::write(&args[0], br#"{"model_type":"llama","vocab_size":260,"hidden_size":4,"intermediate_size":4,"num_hidden_layers":2,"num_attention_heads":2,"num_key_value_heads":1,"max_position_embeddings":8,"rms_norm_eps":0.00001,"rope_theta":10000.0}"#).unwrap();
    weights(Path::new(&args[1]), false, false);
    fs::write(&args[2], original::tokenizer(&original::model()).to_bytes().unwrap()).unwrap();
    fs::write(&args[3], campaign_json()).unwrap();
    let model = original::model();
    let source = model.recompute(1000, &[0, 1], original::inference()).unwrap().cache_image().unwrap();
    let policy = LearnedKvPolicy::new(1, 1, 1, 8).unwrap();
    let allowance = FitBudget { source_values: 64, parameter_values: 64, scratch_values: 16, work_units: 10000 };
    let (_, checkpoint) = LearnedKvCodec::fit_with_checkpoint(policy, &BTreeMap::from([(1010, source)]), allowance).unwrap();
    let binding = checkpoint.binding();
    let descriptor = binding.sources[&1010].encode().unwrap().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    fs::write(&args[4], format!(r#"{{"schema":"fa.learned-fit-binding/1","policy":{{"id":1,"generation":1,"rank":1,"sweeps":8}},"budget":{{"source_values":64,"parameter_values":64,"scratch_values":16,"work_units":10000}},"sources":[{{"origin":1010,"descriptor_hex":"{descriptor}"}}]}}"#)).unwrap();
    fs::write(&args[5], checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap()).unwrap();
    args
}

/// Source-derived arrays encode the same model as original::model(). The final
/// artifact is verified against that independent direct constructor in tests.
pub(super) fn weights(path: &Path, missing_value_signal: bool, overflow: bool) {
    let mut embedding = [-1.0_f32, 0.0, 0.0, 0.0].repeat(260);
    embedding[4] = 1.0; embedding[4 * 4 + 2] = 1.0; embedding[5 * 4 + 2] = -1.0;
    let mut output = vec![0.0; 260 * 4];
    output[2] = 1.0; output[4 + 2] = -1.0; output[256 * 4] = -0.5;
    if overflow { output[0] = f32::MAX; }
    let mut tensors: BTreeMap<String, (Vec<usize>, Vec<f32>)> = BTreeMap::from([
        ("model.embed_tokens.weight".into(), (vec![260, 4], embedding)),
        ("model.norm.weight".into(), (vec![4], vec![1.0; 4])),
        ("lm_head.weight".into(), (vec![260, 4], output)),
    ]);
    for layer in 0..2 {
        for name in ["input_layernorm", "post_attention_layernorm"] {
            tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![4], vec![1.0; 4]));
        }
        for name in ["self_attn.q_proj", "self_attn.o_proj", "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj"] {
            tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![4, 4], vec![0.0; 16]));
        }
        for name in ["self_attn.k_proj", "self_attn.v_proj"] {
            let values = if layer == 1 && name == "self_attn.v_proj" && missing_value_signal { vec![0.0; 8] }
                else { vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] };
            tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![2, 4], values));
        }
    }
    let mut header = String::from("{"); let mut body = Vec::new();
    for (index, (name, (shape, values))) in tensors.into_iter().enumerate() {
        if index != 0 { header.push(','); }
        let start = body.len(); for value in values { body.extend_from_slice(&value.to_le_bytes()); }
        write!(header, "\"{name}\":{{\"dtype\":\"F32\",\"shape\":{shape:?},\"data_offsets\":[{start},{}]}}", body.len()).unwrap();
    }
    header.push('}'); while !header.len().is_multiple_of(8) { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec(); bytes.extend_from_slice(header.as_bytes()); bytes.extend_from_slice(&body);
    fs::write(path, bytes).unwrap();
}

pub(super) fn tokenizer_json() -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    let mut next = 256_u32;
    let mut vocabulary = Vec::new();
    for byte in 0_u32..256 {
        let code = if (33..=126).contains(&byte) || (161..=172).contains(&byte) || (174..=255).contains(&byte) { byte }
            else { let value = next; next += 1; value };
        vocabulary.push(format!("{}:{byte}", quote(&char::from_u32(code).unwrap().to_string())));
    }
    vocabulary.extend(["\"<|stop|>\":256".to_owned(), "\"ab\":257".to_owned(), "\"bc\":258".to_owned(), "\"<|reserved|>\":259".to_owned()]);
    let specials = [(256, "<|stop|>"), (259, "<|reserved|>")].into_iter().map(|(id, text)|
        format!(r#"{{"id":{id},"content":"{text}","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}}"#))
        .collect::<Vec<_>>().join(",");
    let byte_level = r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#;
    format!(r#"{{"version":"1.0","truncation":null,"padding":null,"added_tokens":[{specials}],"normalizer":null,"pre_tokenizer":{byte_level},"post_processor":null,"decoder":{byte_level},"model":{{"type":"BPE","dropout":null,"unk_token":null,"continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":false,"vocab":{{{}}},"merges":[["a","b"],["b","c"]]}}}}"#,
        vocabulary.join(","))
}
