use fa_reference::action::consequence::activation::probe::training::interchange::files::*;
use fa_reference::action::consequence::activation::probe::training::decoder::plan::{ProbeRunPlan, MAX_CAMPAIGN_PLAN_BYTES};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::CheckpointFileLimits;
use fa_reference::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const PLAN: &[u8] = include_bytes!("fixtures/decoder_probe_files.json");
const CONFIG: &[u8] = include_bytes!("fixtures/decoder_llama_config.json");
const WEIGHTS: &[u8] = include_bytes!("fixtures/decoder_mixed.safetensors");
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-probe-files-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn inputs(&self, plan: &[u8]) -> (PathBuf, PathBuf, PathBuf) {
        let c = self.0.join("config.json"); let w = self.0.join("weights.safetensors"); let p = self.0.join("campaign.json");
        fs::write(&c, CONFIG).unwrap(); fs::write(&w, WEIGHTS).unwrap(); fs::write(&p, plan).unwrap(); (c, w, p)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("probe file cleanup failed: {error}"); } }
}

#[test]
fn independently_loaded_files_match_in_memory_capture_training_and_all_thresholds() {
    let directory = Directory::new(); let (c, w, p) = directory.inputs(PLAN);
    let limits = TrainingFileLimits { plan_bytes: PLAN.len(),
        checkpoint: CheckpointFileLimits { config_bytes: CONFIG.len(), weight_bytes: WEIGHTS.len() } };
    let (model, mut plan) = load_training_inputs(c, w, p, limits).unwrap();
    let mut expected_plan = ProbeRunPlan::from_json(PLAN).unwrap();
    let (expected_model, _) = DecoderModel::from_llama_safetensors(expected_plan.identity(), expected_plan.context(), CONFIG, WEIGHTS).unwrap();
    assert_eq!(plan.estimate(&model).unwrap(), expected_plan.estimate(&expected_model).unwrap());
    drop(directory);
    let actual = plan.run(&model).unwrap(); let expected = expected_plan.run(&expected_model).unwrap();
    assert!(actual.accepted()); assert_eq!(actual.accepted(), expected.accepted());
    for (id, layer) in actual.layers() {
        let other = &expected.layers()[id];
        assert_eq!(layer.calibration().fitted().weights(), other.calibration().fitted().weights());
        assert_eq!(layer.calibration().fitted().bias().to_bits(), other.calibration().fitted().bias().to_bits());
        assert_eq!(layer.calibration().trials(), other.calibration().trials());
        assert_eq!(layer.evaluation().unwrap().counts(), other.evaluation().unwrap().counts());
    }
    let remaining = plan.remaining_campaign();
    assert_eq!(plan.run(&model).unwrap_err(), Error::Limit);
    assert_eq!(plan.remaining_campaign(), remaining);
}

#[test]
fn malformed_plan_precedes_all_configuration_and_weight_access() {
    let directory = Directory::new(); let path = directory.0.join("bad.json"); fs::write(&path, b"{}").unwrap();
    assert!(matches!(load_training_inputs(directory.0.join("missing-config"), directory.0.join("missing-weights"),
        &path, TrainingFileLimits::default()), Err(TrainingFileError::Plan(_))));
}

#[test]
fn insufficient_complete_campaign_budget_is_rejected_before_capture() {
    let changed = std::str::from_utf8(PLAN).unwrap().replace("\"training_visits\": 1000000", "\"training_visits\": 0");
    assert_ne!(changed.as_bytes(), PLAN);
    let directory = Directory::new(); let (c, w, p) = directory.inputs(changed.as_bytes());
    assert!(matches!(load_training_inputs(c, w, &p, TrainingFileLimits::default()), Err(TrainingFileError::Admission(Error::Limit))));
    let mut plan = read_training_plan(p, MAX_CAMPAIGN_PLAN_BYTES).unwrap();
    let (model, _) = DecoderModel::from_llama_safetensors(plan.identity(), plan.context(), CONFIG, WEIGHTS).unwrap();
    let capture = plan.remaining_capture(); let training = plan.remaining_campaign();
    assert_eq!(plan.run(&model).unwrap_err(), Error::Limit);
    assert_eq!(plan.remaining_capture(), capture); assert_eq!(plan.remaining_campaign(), training);
}

#[test]
fn plan_file_limits_nonregular_inputs_and_selected_symlinks_refuse() {
    let directory = Directory::new(); let (_, _, p) = directory.inputs(PLAN);
    assert!(read_training_plan(&p, PLAN.len()).is_ok());
    assert!(matches!(read_training_plan(&p, PLAN.len() - 1), Err(TrainingFileError::Limit)));
    assert!(matches!(read_training_plan(&p, MAX_CAMPAIGN_PLAN_BYTES + 1), Err(TrainingFileError::Limit)));
    assert!(matches!(read_training_plan(&directory.0, PLAN.len()), Err(TrainingFileError::NotRegular)));
    #[cfg(unix)] {
        let alias = directory.0.join("alias.json"); std::os::unix::fs::symlink(&p, &alias).unwrap();
        assert!(matches!(read_training_plan(alias, PLAN.len()), Err(TrainingFileError::NotRegular)));
    }
    fs::write(&p, &PLAN[..PLAN.len() / 2]).unwrap();
    assert!(matches!(read_training_plan(p, PLAN.len()), Err(TrainingFileError::Plan(_))));
}
