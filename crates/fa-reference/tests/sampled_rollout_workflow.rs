//! Real checkpoint-file loading and fixed held-out campaign integration.
#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod training;
use fa_reference::action::consequence::activation::monitor::RefinementBudget;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::evaluation::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::evaluation::plan::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::evaluation::files::*;
use fa_reference::action::consequence::activation::probe::training::decoder::MonitorExport;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const PLAN: &[u8] = include_bytes!("fixtures/sampled_rollout.json");
fn config(threshold: f32) -> Vec<u8> {
    let layer = |id| format!(r#"{{"layer":{id},"levels":[23],"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"probes":[{{"id":{id},"generation":1,"weights":[1,0],"bias":0,"threshold":{threshold}}}]}}"#);
    format!(r#"{{"schema":"fa.decoder-monitor/1","generation":1,"identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"layers":[{},{}]}}"#,
        layer(1), layer(2)).into_bytes()
}
fn changed(from: &str, to: &str) -> Vec<u8> {
    let original = std::str::from_utf8(PLAN).unwrap(); let changed = original.replace(from, to);
    assert_ne!(changed, original); changed.into_bytes()
}
fn report(threshold: f32) -> RolloutReport {
    RolloutPlan::decode(PLAN).unwrap().bind(training::model(), &config(threshold)).unwrap().run().unwrap()
}
fn rows(bytes: &[u8]) -> Vec<Json> {
    std::str::from_utf8(bytes).unwrap().lines().map(|line|
        strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect()
}

#[test]
fn strict_plan_retains_exact_seed_and_admits_the_entire_original_profile_once() {
    let plan = RolloutPlan::decode(&changed("\"seed\": 7", "\"seed\": 18446744073709551615")).unwrap();
    assert_eq!(plan.cases()[1].seed, u64::MAX);
    let mut prepared = plan.bind(training::model(), &config(0.0)).unwrap();
    assert!(!prepared.started()); let before = prepared.remaining_budget(); let work = prepared.planned_work();
    let report = prepared.run().unwrap(); assert!(report.accepted());
    assert!(prepared.started()); assert_eq!(prepared.remaining_budget().cases, before.cases - work.cases);
    let after = prepared.remaining_budget();
    assert_eq!(prepared.run().unwrap_err(), Error::WrongState); assert_eq!(prepared.remaining_budget(), after);
}

#[test]
fn plan_rejects_unknown_duplicate_malformed_and_out_of_range_inputs() {
    for (from, to) in [
        ("\"schema\":", "\"extra\":0,\"schema\":"),
        ("\"seed\": 7", "\"seed\":7,\"seed\":8"),
        ("\"task\": 20", "\"task\": 10"),
        ("\"lineage\": 2000", "\"lineage\": 1000"),
        ("\"random_stream\": 12", "\"random_stream\": 11"),
        ("[1,1]", "[0,0]"), ("[0,0]", "[0,2]"),
        ("\"stop_tokens\": []", "\"stop_tokens\":[1,1]"),
        ("\"effect_patterns\": [[1]]", "\"effect_patterns\":[[1],[1]]"),
        ("\"minimum_benign\": 1", "\"minimum_benign\": 0"),
        ("\"max_new_tokens\": 2", "\"max_new_tokens\": 8"),
        ("\"seed\": 7", "\"seed\": -1"),
    ] { assert!(RolloutPlan::decode(&changed(from, to)).is_err(), "{to}"); }
    let oversized = vec![b' '; MAX_ROLLOUT_PLAN_BYTES + 1];
    assert_eq!(RolloutPlan::decode(&oversized).unwrap_err(), RolloutPlanError::Limit);
}

#[test]
fn model_binding_and_insufficient_plan_budget_refuse_before_running() {
    for (from, to, expected) in [
        ("\"model\": 2", "\"model\": 3", Error::Binding),
        ("\"context\": 8", "\"context\": 7", Error::Binding),
        ("\"scalar_products\": 1000000", "\"scalar_products\": 0", Error::Limit),
        ("\"sampling_entries\": 10000", "\"sampling_entries\": 0", Error::Limit),
    ] {
        let plan = RolloutPlan::decode(&changed(from, to)).unwrap();
        assert_eq!(plan.bind(training::model(), &config(0.0)).unwrap_err(), RolloutBuildError::Contract(expected));
    }
}

#[test]
fn reporting_preserves_case_denominators_stopping_details_and_actual_baselines() {
    let result = report(0.0); let mut output = Vec::new(); result.write_ndjson(&mut output).unwrap();
    let rows = rows(&output); assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].get("kind").unwrap().as_str(), Some("sampled_protocol"));
    assert_eq!(rows[0].get("sampling").unwrap().get("top_k").unwrap().as_u64(), Some(1));
    assert_eq!(rows[1].get("paired_positions").unwrap().as_u64(), Some(4));
    assert_eq!(rows[2].get("baseline_tokens").unwrap().as_array().unwrap().len(), 2);
    assert_eq!(rows[2].get("released_tokens").unwrap().as_array().unwrap().len(), 0);
    assert_eq!(rows[2].get("monitored_end").unwrap().get("outcome").unwrap().as_str(), Some("Alarm"));
    assert_eq!(rows[2].get("monitored_end").unwrap().get("unreviewed_layers").unwrap().as_u64(), Some(1));
    assert_eq!(rows[3].get("planned_cases").unwrap().as_u64(), Some(2));
    assert_eq!(rows[3].get("accepted").unwrap().as_bool(), Some(true));
    assert_eq!(rows[3].get("permission").unwrap().as_str(), Some("not_issued"));
}

#[test]
fn trained_campaign_uses_original_coefficients_and_rejects_reused_tasks_from_every_split() {
    let model = training::model(); let corpus = training::capture(&model, &training::cases());
    let campaign = training::campaign(&corpus); assert!(campaign.accepted());
    let budget = RefinementBudget { encoded_bytes: 100000, probe_coordinates: 100000 };
    let settings = MonitorExport::new(1, vec![23], budget, budget).unwrap().for_campaign(&campaign);
    let plan = RolloutPlan::decode(PLAN).unwrap();
    let expected = campaign.monitor_json(&settings, 1_048_576).unwrap();
    let mut prepared = campaign.sampled_rollout(settings.clone(), &plan).unwrap();
    let report = prepared.run().unwrap(); assert!(report.accepted());
    assert_eq!(report.monitor_json().unwrap(), expected);
    for id in 1..=6 {
        let reused = changed("\"task\": 10", &format!("\"task\":{id}"));
        let reused = RolloutPlan::decode(&reused).unwrap();
        assert_eq!(campaign.sampled_rollout(settings.clone(), &reused).unwrap_err(), RolloutBuildError::Contract(Error::Duplicate));
    }
    for (from, to) in [("\"lineage\": 1000", "\"lineage\": 101"), ("[0,0]", "[0]")] {
        let reused = RolloutPlan::decode(&changed(from, to)).unwrap();
        assert_eq!(campaign.sampled_rollout(settings.clone(), &reused).unwrap_err(), RolloutBuildError::Contract(Error::Duplicate));
    }
}

#[test]
fn a_failed_fitted_layer_cannot_be_dropped_to_run_a_sampled_export() {
    let mut cases = training::cases(); cases[4].tokens = vec![1];
    let corpus = training::capture(&training::model(), &cases);
    let campaign = training::campaign(&corpus); assert!(!campaign.accepted());
    let budget = RefinementBudget { encoded_bytes: 100000, probe_coordinates: 100000 };
    let settings = MonitorExport::new(1, vec![23], budget, budget).unwrap().for_campaign(&campaign);
    assert_eq!(campaign.sampled_rollout(settings, &RolloutPlan::decode(PLAN).unwrap()).unwrap_err(),
        RolloutBuildError::Contract(Error::WrongState));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-sampled-files-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("sampled file cleanup: {error}"); } }
}
fn checkpoint(root: &Path) -> [PathBuf; 5] {
    let configuration = root.join("config.json"); let weights = root.join("weights.safetensors");
    let monitor = root.join("monitor.json"); let plan = root.join("rollout.json"); let output = root.join("accepted.json");
    fs::write(&configuration, br#"{"model_type":"llama","vocab_size":2,"hidden_size":2,"intermediate_size":2,"num_hidden_layers":2,"num_attention_heads":1,"num_key_value_heads":1,"max_position_embeddings":8,"rms_norm_eps":0.00001,"rope_theta":10000.0}"#).unwrap();
    let mut tensors: BTreeMap<String, (Vec<usize>, Vec<f32>)> = BTreeMap::from([
        ("model.embed_tokens.weight".into(), (vec![2, 2], vec![-1.0, 0.0, 1.0, 0.0])),
        ("model.norm.weight".into(), (vec![2], vec![1.0; 2])),
        ("lm_head.weight".into(), (vec![2, 2], vec![-1.0, 0.0, 1.0, 0.0])),
    ]);
    for layer in 0..2 {
        for name in ["input_layernorm", "post_attention_layernorm"] {
            tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![2], vec![1.0; 2]));
        }
        for name in ["self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj", "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj"] {
            tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![2, 2], vec![0.0; 4]));
        }
    }
    let mut header = String::from("{"); let mut body = Vec::new();
    for (index, (name, (shape, values))) in tensors.into_iter().enumerate() {
        if index != 0 { header.push(','); } let start = body.len();
        for value in values { body.extend_from_slice(&value.to_le_bytes()); }
        write!(header, "\"{name}\":{{\"dtype\":\"F32\",\"shape\":{shape:?},\"data_offsets\":[{start},{}]}}", body.len()).unwrap();
    }
    header.push('}'); while !header.len().is_multiple_of(8) { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec(); bytes.extend_from_slice(header.as_bytes()); bytes.extend_from_slice(&body);
    fs::write(&weights, bytes).unwrap(); fs::write(&monitor, config(0.0)).unwrap(); fs::write(&plan, PLAN).unwrap();
    [configuration, weights, monitor, plan, output]
}
fn files(paths: &[PathBuf; 5], out: &mut impl Write) -> Result<bool, RolloutFileError> {
    evaluate_checkpoint_files(&paths[0], &paths[1], &paths[2], &paths[3], &paths[4], out)
}

#[test]
fn actual_checkpoint_files_produce_a_paired_report_and_the_identical_accepted_monitor() {
    let root = Directory::new(); let paths = checkpoint(&root.0); let mut output = Vec::new();
    assert!(files(&paths, &mut output).unwrap());
    assert_eq!(fs::read(&paths[4]).unwrap(), fs::read(&paths[2]).unwrap());
    let log = rows(&output); assert_eq!(log.len(), 4);
    assert_eq!(log[3].get("counts").unwrap().get("timely_alarm").unwrap().as_u64(), Some(1));
    assert_eq!(log[3].get("counts").unwrap().get("benign_complete").unwrap().as_u64(), Some(1));
}

#[test]
fn rejected_sampled_results_keep_their_counts_and_create_no_monitor() {
    let root = Directory::new(); let paths = checkpoint(&root.0); fs::write(&paths[2], config(2.0)).unwrap();
    let mut output = Vec::new(); assert!(!files(&paths, &mut output).unwrap()); assert!(!paths[4].exists());
    let log = rows(&output); assert_eq!(log[3].get("accepted").unwrap().as_bool(), Some(false));
    assert_eq!(log[3].get("counts").unwrap().get("quiet_miss").unwrap().as_u64(), Some(1));
}

#[test]
fn report_write_and_final_flush_failures_never_create_an_artifact() {
    struct Broken { flush_only: bool, written: Vec<u8> }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.flush_only { return Err(io::ErrorKind::BrokenPipe.into()); }
            self.written.extend_from_slice(bytes); Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let result = report(0.0); let root = Directory::new(); let destination = root.0.join("not-created.json");
    for flush_only in [false, true] {
        let mut sink = Broken { flush_only, written: Vec::new() };
        assert!(matches!(publish_report(&result, &destination, &mut sink),
            Err(RolloutFileError::Io { operation: "report", kind: io::ErrorKind::BrokenPipe })));
        assert!(!destination.exists());
        if flush_only { assert_eq!(rows(&sink.written).len(), 4); }
    }
    assert!(publish_report(&result, &destination, &mut Vec::new()).unwrap());
}

#[test]
fn existing_destinations_and_creation_races_cannot_overwrite_other_bytes() {
    let root = Directory::new(); let destination = root.0.join("existing.json");
    fs::write(&destination, b"preserve existing").unwrap();
    assert!(publish_report(&report(0.0), &destination, &mut Vec::new()).is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"preserve existing");
    struct Racing { path: PathBuf }
    impl Write for Racing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> { fs::write(&self.path, b"concurrent owner") }
    }
    let raced = root.0.join("raced.json");
    assert!(matches!(publish_report(&report(0.0), &raced, &mut Racing { path: raced.clone() }),
        Err(RolloutFileError::Io { operation: "create_monitor", kind: io::ErrorKind::AlreadyExists })));
    assert_eq!(fs::read(&raced).unwrap(), b"concurrent owner");
}

#[test]
fn malformed_plan_is_refused_before_checkpoint_access_or_publication() {
    let root = Directory::new(); let paths = checkpoint(&root.0);
    fs::write(&paths[3], changed("\"seed\": 7", "\"seed\": -1")).unwrap();
    fs::remove_file(&paths[0]).unwrap(); fs::remove_file(&paths[1]).unwrap();
    let mut output = Vec::new();
    assert!(matches!(files(&paths, &mut output), Err(RolloutFileError::Plan(_))));
    assert!(output.is_empty()); assert!(!paths[4].exists());
}

#[cfg(unix)]
#[test]
fn symbolic_link_inputs_and_outputs_are_not_followed() {
    use std::os::unix::fs::symlink;
    let root = Directory::new(); let mut paths = checkpoint(&root.0);
    let original = paths[3].clone(); paths[3] = root.0.join("linked-plan.json"); symlink(&original, &paths[3]).unwrap();
    assert!(matches!(files(&paths, &mut Vec::new()), Err(RolloutFileError::Io { operation: "plan", .. })));
    assert!(!paths[4].exists()); paths[3] = original;
    symlink(&paths[2], &paths[4]).unwrap();
    assert!(files(&paths, &mut Vec::new()).is_err());
    assert_eq!(fs::read(&paths[2]).unwrap(), config(0.0));
}
