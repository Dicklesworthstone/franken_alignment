//! Offline checkpoint -> original residuals -> evaluated, loadable monitor data.
//! No automatic deployment. Statistical rejection prints reports and exits 2.
#![forbid(unsafe_code)]
use fa_reference::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use fa_reference::action::consequence::activation::probe::training::calibration::ConfusionCounts;
use fa_reference::action::consequence::activation::probe::training::decoder::{DecoderCampaign, plan::ProbeRunPlan};
use fa_reference::action::consequence::activation::probe::training::interchange::files::{load_training_inputs, TrainingFileLimits};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use fa_reference::action::consequence::activation::probe::training::decoder::trajectory::plan::{TrajectoryPlan, read_trajectory_plan};
use std::ffi::OsString;
#[cfg(test)]
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "train_decoder_probes_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS CAMPAIGN_JSON NEW_MONITOR_JSON [TRAJECTORY_JSON]";
fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn counts(value: ConfusionCounts) -> String {
    format!("{{\"benign_alarm\":{},\"benign_quiet\":{},\"benign_boundary\":{},\"violation_alarm\":{},\"violation_quiet\":{},\"violation_boundary\":{}}}",
        value.benign_alarm, value.benign_quiet, value.benign_boundary,
        value.violation_alarm, value.violation_quiet, value.violation_boundary)
}
fn report(campaign: &DecoderCampaign, output: &mut impl Write) -> io::Result<()> {
    for (layer, result) in campaign.layers() {
        for trial in result.calibration().trials() {
            writeln!(output, "{{\"kind\":\"calibration_trial\",\"layer\":{layer},\"threshold\":{:.9e},\"accepted\":{},\"counts\":{}}}",
                trial.threshold(), trial.accepted(), counts(trial.counts()))?;
        }
        let selected = result.calibration().selected_threshold().map_or_else(|| "null".to_owned(), |value| format!("{value:.9e}"));
        let outcome = result.evaluation().map_or_else(|| "null".to_owned(), |value| counts(value.counts()));
        writeln!(output, "{{\"kind\":\"evaluation\",\"layer\":{layer},\"selected_threshold\":{selected},\"evaluated\":{},\"accepted\":{},\"counts\":{outcome}}}",
            result.evaluation().is_some(), result.accepted())?;
    }
    let work = campaign.completed_work();
    writeln!(output, "{{\"kind\":\"campaign_complete\",\"layers\":{},\"accepted\":{},\"training_visits\":{},\"scoring_bytes\":{},\"scoring_coordinates\":{},\"threshold_comparisons\":{},\"permission\":\"not_issued\"}}",
        campaign.layers().len(), campaign.accepted(), work.training_visits, work.scoring_bytes,
        work.scoring_coordinates, work.threshold_comparisons)?;
    output.flush()
}
fn execute(plan: &mut ProbeRunPlan, model: &DecoderModel, destination: &Path,
    output: &mut impl Write) -> Result<bool, Box<dyn std::error::Error>>
{
    execute_validated(plan, model, destination, None, output)
}
fn execute_validated(plan: &mut ProbeRunPlan, model: &DecoderModel, destination: &Path,
    trajectory: Option<TrajectoryPlan>, output: &mut impl Write) -> Result<bool, Box<dyn std::error::Error>>
{
    if let Some(validation) = &trajectory { validation.validate_profile(model.profile())?; }
    let campaign = plan.run(model).map_err(|error| invalid(format!("campaign refusal: {error:?}")))?;
    // Print all operating points and untouched evaluation counts, even when no
    // roster can be exported. A broken report sink prevents new file creation.
    report(&campaign, output)?;
    if !campaign.accepted() { return Ok(false); }
    let settings = plan.settings().for_campaign(&campaign);
    if let Some(validation) = trajectory {
        let mut prepared = validation.bind(&campaign, settings.clone())?;
        let results = prepared.run().map_err(|error| invalid(format!("trajectory refusal: {error:?}")))?;
        results.write_ndjson(output)?;
        if !results.accepted() { return Ok(false); }
        let exercised = results.monitor_json().map_err(|error| invalid(format!("trajectory export: {error:?}")))?;
        let exported = campaign.monitor_json(&settings, MAX_MONITOR_CONFIG_BYTES)
            .map_err(|error| invalid(format!("monitor export: {error:?}")))?;
        if exercised != exported.as_slice() { return Err(invalid("evaluated monitor/configuration mismatch").into()); }
    }
    let bytes = campaign.save_monitor_new(&settings, MAX_MONITOR_CONFIG_BYTES, destination)?;
    writeln!(output, "{{\"kind\":\"monitor_saved\",\"bytes\":{bytes},\"permission\":\"not_issued\"}}")?;
    output.flush()?;
    Ok(true)
}
fn run(args: &[OsString], output: &mut impl Write) -> Result<bool, Box<dyn std::error::Error>> {
    if args.len() != 4 && args.len() != 5 { return Err(invalid(USAGE).into()); }
    let trajectory = args.get(4).map(|path| read_trajectory_plan(Path::new(path))).transpose()?;
    let (model, mut plan) = load_training_inputs(&args[0], &args[1], &args[2], TrainingFileLimits::default())?;
    match trajectory {
        Some(validation) => execute_validated(&mut plan, &model, Path::new(&args[3]), Some(validation), output),
        None => execute(&mut plan, &model, Path::new(&args[3]), output),
    }
}
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).take(6).collect();
    match run(&args, &mut io::stdout().lock()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
#[path = "../tests/support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep, MonitoringStatus};
    use fa_reference::action::consequence::activation::probe::training::decoder::CaptureWork;
    use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
    use fa_reference::strict_json::{self, Json, Limits};
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    const PLAN: &[u8] = include_bytes!("../tests/fixtures/decoder_probe_campaign.json");
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("fa-probe-cli-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("probe CLI cleanup: {error}"); } }
    }
    fn rows(bytes: &[u8]) -> Vec<Json> {
        std::str::from_utf8(bytes).unwrap().lines().map(|line| strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect()
    }
    fn kind(row: &Json) -> &str { row.get("kind").unwrap().as_str().unwrap() }
    // Actual serialized F32 tensors for the same tiny separable decoder fixture.
    // This never substitutes a trained-model claim for deterministic parameters.
    fn checkpoint(directory: &Path) -> Vec<OsString> {
        let config = directory.join("config.json"); let weights = directory.join("model.safetensors");
        let plan = directory.join("campaign.json"); let output = directory.join("monitor.json");
        fs::write(&config, br#"{"model_type":"llama","vocab_size":2,"hidden_size":2,"intermediate_size":2,"num_hidden_layers":2,"num_attention_heads":1,"num_key_value_heads":1,"max_position_embeddings":8,"rms_norm_eps":0.00001,"rope_theta":10000.0}"#).unwrap();
        let mut tensors: BTreeMap<String, (Vec<usize>, Vec<f32>)> = BTreeMap::from([
            ("model.embed_tokens.weight".into(), (vec![2, 2], vec![-1.0, 0.0, 1.0, 0.0])),
            ("model.norm.weight".into(), (vec![2], vec![1.0; 2])),
            ("lm_head.weight".into(), (vec![2, 2], vec![-1.0, 0.0, 1.0, 0.0])),
        ]);
        for layer in 0..2 {
            for name in ["input_layernorm", "post_attention_layernorm"] {
                tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![2], vec![1.0; 2]));
            }
            for name in ["self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj",
                "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj"] {
                tensors.insert(format!("model.layers.{layer}.{name}.weight"), (vec![2, 2], vec![0.0; 4]));
            }
        }
        let mut header = String::from("{"); let mut body = Vec::new();
        for (i, (name, (shape, values))) in tensors.into_iter().enumerate() {
            if i != 0 { header.push(','); }
            let start = body.len();
            for value in values { body.extend_from_slice(&value.to_le_bytes()); }
            write!(header, "\"{name}\":{{\"dtype\":\"F32\",\"shape\":{shape:?},\"data_offsets\":[{start},{}]}}", body.len()).unwrap();
        }
        header.push('}'); while !header.len().is_multiple_of(8) { header.push(' '); }
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header.as_bytes()); bytes.extend_from_slice(&body);
        fs::write(&weights, bytes).unwrap(); fs::write(&plan, PLAN).unwrap();
        vec![config.into_os_string(), weights.into_os_string(), plan.into_os_string(), output.into_os_string()]
    }
    #[test]
    fn actual_checkpoint_files_train_and_export_a_monitor_with_real_quiet_and_alarm_paths() {
        let root = Directory::new(); let args = checkpoint(&root.0); let mut output = Vec::new();
        assert!(run(&args, &mut output).unwrap());
        let log = rows(&output);
        assert_eq!(log.iter().filter(|row| kind(row) == "calibration_trial").count(), 6);
        assert_eq!(log.iter().filter(|row| kind(row) == "evaluation").count(), 2);
        assert_eq!(kind(log.last().unwrap()), "monitor_saved");
        let mut guarded = MonitoredDecoder::from_json(fixture::model(), 20, &fs::read(&args[3]).unwrap()).unwrap();
        assert!(matches!(guarded.advance(0, 0, DecoderBudget { scalar_products: 68 }).unwrap(), MonitoredStep::Released(_)));
        let budget = DecoderBudget { scalar_products: guarded.estimate(1).unwrap().scalar_products().unwrap() };
        assert!(matches!(guarded.advance(1, 1, budget).unwrap(), MonitoredStep::Held(_)));
        assert_eq!(guarded.status(), MonitoringStatus::Held);
    }
    #[test]
    fn untouched_evaluation_failure_prints_counts_without_creating_any_monitor_file() {
        let root = Directory::new(); let args = checkpoint(&root.0);
        let bad = std::str::from_utf8(PLAN).unwrap().replace("\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"benign\",\"tokens\":[0]",
            "\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"benign\",\"tokens\":[1]");
        assert_ne!(bad.as_bytes(), PLAN); fs::write(&args[2], bad).unwrap();
        let mut output = Vec::new(); assert!(!run(&args, &mut output).unwrap());
        assert!(!Path::new(&args[3]).exists());
        let log = rows(&output); assert_eq!(kind(log.last().unwrap()), "campaign_complete");
        assert_eq!(log.last().unwrap().get("accepted").unwrap().as_bool(), Some(false));
        for result in log.iter().filter(|row| kind(row) == "evaluation") {
            assert_eq!(result.get("counts").unwrap().get("benign_alarm").unwrap().as_u64(), Some(1));
        }
    }
    #[test]
    fn successful_training_never_overwrites_a_prior_monitor_artifact() {
        let root = Directory::new(); let args = checkpoint(&root.0);
        fs::write(&args[3], b"existing artifact").unwrap(); let mut output = Vec::new();
        assert!(run(&args, &mut output).is_err());
        assert_eq!(fs::read(&args[3]).unwrap(), b"existing artifact");
        assert!(rows(&output).iter().all(|row| kind(row) != "monitor_saved"));
    }
    #[test]
    fn broken_report_sink_cannot_create_a_config_or_replenish_a_completed_campaign() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
        }
        let root = Directory::new(); let path = root.0.join("monitor.json");
        let mut plan = ProbeRunPlan::from_json(PLAN).unwrap();
        assert!(execute(&mut plan, &fixture::model(), &path, &mut Broken).is_err());
        assert!(!path.exists()); assert_eq!(plan.remaining_capture(), CaptureWork::default());
        assert!(plan.run(&fixture::model()).is_err());
    }
    #[test]
    fn malformed_plan_is_rejected_before_opening_missing_checkpoint_files() {
        let root = Directory::new(); let args = checkpoint(&root.0);
        fs::write(&args[2], b"{\"schema\":1,\"schema\":2}").unwrap();
        fs::remove_file(&args[0]).unwrap(); fs::remove_file(&args[1]).unwrap();
        let mut output = Vec::new();
        assert!(run(&args, &mut output).unwrap_err().to_string().contains("Syntax"));
        assert!(output.is_empty()); assert!(!Path::new(&args[3]).exists());
    }
    include!("../tests/support/decoder_trajectory_cli.rs");
}
