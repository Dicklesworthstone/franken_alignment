use super::*;
use fa_reference::action::consequence::activation::{
    monitor::MonitorOutcome,
    probe::training::decoder::kv::{KvCaptureWork, KvCampaignWork, KvMonitorArtifact, MAX_KV_MONITOR_ARTIFACT_BYTES},
    tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationStatus, GenerationStop}}};
use fa_reference::strict_json::{self, Json, Limits};
use std::fs;

#[path = "../../tests/support/bound_kv_monitor.rs"]
#[allow(dead_code)]
mod original;
#[path = "fixture.rs"]
mod fixture;
use fixture::{Root, files};
// Consume the artifact through the ACTUAL publication operator parser. No test
// decoder of the emitted monitor fields substitutes for the production loader.
#[path = "../supervise_publication/workflow/actor_service/learned/monitor.rs"]
#[allow(dead_code)]
mod operator_monitor;

fn rows(args: &[OsString]) -> Vec<Json> {
    let bytes = fs::read(&args[6]).unwrap();
    std::str::from_utf8(&bytes).unwrap().lines().map(|line|
        strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect()
}
fn kind(row: &Json) -> &str { row.get("kind").unwrap().as_str().unwrap() }
fn selected(rows: &[Json], name: &str) -> Vec<Json> {
    rows.iter().filter(|row| kind(row) == name).cloned().collect()
}
fn replace(args: &[OsString], before: &str, after: &str) {
    let text = fs::read_to_string(&args[3]).unwrap();
    assert!(text.contains(before), "missing fixture field: {before}");
    fs::write(&args[3], text.replacen(before, after, 1)).unwrap();
}
fn decrement(args: &[OsString], section: &str, field: &str) {
    let text = fs::read_to_string(&args[3]).unwrap();
    let parsed = strict_json::parse(text.as_bytes(), Limits::default()).unwrap();
    let value = parsed.get(section).unwrap().get(field).unwrap().as_u64().unwrap();
    assert!(value > 0);
    let start = text.find(&format!("\"{section}\":{{")).unwrap();
    let mut changed = text[..start].to_owned();
    changed.push_str(&text[start..].replacen(&format!("\"{field}\":{value}"), &format!("\"{field}\":{}", value - 1), 1));
    fs::write(&args[3], changed).unwrap();
}

#[test]
fn actual_checkpoint_codec_and_tokenizer_files_train_a_bound_monitor_with_real_quiet_and_hold() {
    let root = Root::new(); let args = files(&root); let mut stdout = Vec::new();
    assert!(run(&args, &mut stdout).unwrap());
    let log = rows(&args);
    assert_eq!(selected(&log, "planned_case").len(), 6);
    assert_eq!(selected(&log, "tap_fitted").len(), 4);
    assert_eq!(selected(&log, "calibration_trial").len(), 12);
    assert_eq!(selected(&log, "case_score").len(), 16);
    assert_eq!(selected(&log, "evaluation").len(), 4);
    assert_eq!(kind(log.last().unwrap()), "monitor_saved");
    let capture = selected(&log, "capture_completed_work");
    assert_eq!(capture[0].get("cases").unwrap().as_u64(), Some(6));
    assert_eq!(capture[0].get("original_tokens").unwrap().as_u64(), Some(12));
    assert_eq!(capture[0].get("kv_coordinates").unwrap().as_u64(), Some(48));
    for row in selected(&log, "capture_remaining").into_iter().chain(selected(&log, "campaign_remaining")) {
        assert!(row.as_object().unwrap().iter().filter(|(key, _)| key.as_str() != "kind").all(|(_, value)| value.as_u64() == Some(0)));
    }
    let work = selected(&log, "input_work");
    let auxiliary: u64 = [0, 2, 3, 4].into_iter().map(|index| fs::metadata(&args[index]).unwrap().len()).sum();
    assert_eq!(work[0].get("auxiliary_bytes").unwrap().as_u64(), Some(auxiliary));
    assert_eq!(work[0].get("weight_bytes").unwrap().as_u64(), Some(fs::metadata(&args[1]).unwrap().len()));
    assert_eq!(work[0].get("codec_bytes").unwrap().as_u64(), Some(fs::metadata(&args[5]).unwrap().len()));
    assert!(work[0].get("weight_read_calls").unwrap().as_u64().unwrap() > 0);
    let model = original::model(); let tokenizer = original::tokenizer(&model); let codec = original::codec(&model);
    let bytes = fs::read(&args[7]).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let expected = original::campaign().policy(codec.clone(), &fixture::settings()).unwrap();
    artifact.verify(&model, &tokenizer, &expected).unwrap();
    assert_eq!(artifact.verify(&original::changed_unused_weight(), &tokenizer, &expected), Err(Error::Binding));
    assert_eq!(artifact.verify(&model, &original::changed_tokenizer(&model, 3), &expected), Err(Error::Binding));
    let loaded = operator_monitor::MonitorInput::decode(&bytes).unwrap().bind_with_tokenizer(&model, &tokenizer, codec).unwrap();
    assert_eq!(loaded.policy.monitor().budget(), fixture::settings().audit);
    for (tap, monitor) in loaded.policy.monitor().taps() { assert_eq!(monitor.budget(), fixture::settings().taps[tap]); }
    for (seed, candidate) in [(4, 0), (5, 1)] {
        let spec = original::spec(seed);
        let mut oracle = model.recompute_sampled(2000, spec.prompt(), original::inference(), spec.sampling().clone()).unwrap();
        let choice = oracle.advance_sampled(1, SampleBudget { decoder: original::inference(), sampling: SamplingBudget { vocabulary: 260 } }).unwrap();
        assert_eq!(choice.choice.token, candidate);
        let mut generation = model.monitored_generation(2000, 2001, spec, loaded.policy.clone(), GenerationBudget::default()).unwrap();
        assert!(generation.advance(0).unwrap().audit().complete_quiet());
        let cache = generation.accepted_cache_image().unwrap().encode().unwrap();
        let event = generation.advance(1).unwrap();
        if candidate == 0 {
            assert!(event.audit().complete_quiet()); assert_eq!(event.sample(), Some(&choice.choice));
            assert!(generation.advance(2).unwrap().audit().complete_quiet());
            assert_eq!(generation.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
            assert_eq!(generation.generated_tokens(), &[0, 256]);
        } else {
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(event.sample().is_none()); assert!(event.accepted().is_none());
            assert!(generation.generated_tokens().is_empty());
            assert_eq!(generation.accepted_cache_image().unwrap().encode().unwrap(), cache);
            assert_eq!(generation.work().sampling_attempts, 1);
        }
    }
}

#[test]
fn explicit_external_tokenizer_json_preserves_the_same_complete_bound_artifact() {
    let native = Root::new(); let native_args = files(&native);
    assert!(run(&native_args, &mut Vec::new()).unwrap());
    let json = Root::new(); let json_args = files(&json);
    replace(&json_args, "\"tokenizer_format\":\"native_archive\"", "\"tokenizer_format\":\"huggingface_raw_bytelevel\"");
    fs::write(&json_args[2], fixture::tokenizer_json()).unwrap();
    assert!(run(&json_args, &mut Vec::new()).unwrap());
    assert_eq!(fs::read(&native_args[7]).unwrap(), fs::read(&json_args[7]).unwrap());
}

#[test]
fn changing_only_held_out_labels_preserves_fit_and_calibration_but_saves_failed_final_counts() {
    let control = Root::new(); let good = files(&control); assert!(run(&good, &mut Vec::new()).unwrap());
    let treatment = Root::new(); let bad = files(&treatment);
    replace(&bad, "\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"benign\"",
        "\"task\":5,\"lineage\":105,\"split\":\"evaluation\",\"label\":\"violation\"");
    replace(&bad, "\"task\":6,\"lineage\":106,\"split\":\"evaluation\",\"label\":\"violation\"",
        "\"task\":6,\"lineage\":106,\"split\":\"evaluation\",\"label\":\"benign\"");
    assert!(!run(&bad, &mut Vec::new()).unwrap());
    assert!(!Path::new(&bad[7]).exists());
    let control = rows(&good); let failed = rows(&bad);
    assert_eq!(selected(&control, "tap_fitted"), selected(&failed, "tap_fitted"));
    assert_eq!(selected(&control, "calibration_trial"), selected(&failed, "calibration_trial"));
    for row in selected(&failed, "evaluation") {
        assert_eq!(row.get("accepted").unwrap().as_bool(), Some(false));
        assert_eq!(row.get("planned_benign").unwrap().as_u64(), Some(1));
        assert_eq!(row.get("planned_violation").unwrap().as_u64(), Some(1));
        assert_eq!(row.get("counts").unwrap().get("benign_alarm").unwrap().as_u64(), Some(1));
        assert_eq!(row.get("counts").unwrap().get("violation_quiet").unwrap().as_u64(), Some(1));
    }
    assert_eq!(kind(failed.last().unwrap()), "campaign_complete");
    assert_eq!(failed.last().unwrap().get("accepted").unwrap().as_bool(), Some(false));
}

#[test]
fn removing_only_the_last_value_projection_keeps_three_results_and_the_failed_tap_denominator() {
    let control = Root::new(); let good = files(&control); assert!(run(&good, &mut Vec::new()).unwrap());
    let treatment = Root::new(); let bad = files(&treatment);
    fixture::weights(Path::new(&bad[1]), true, false);
    assert!(!run(&bad, &mut Vec::new()).unwrap());
    assert!(!Path::new(&bad[7]).exists());
    let good = rows(&good); let bad = rows(&bad);
    let control = selected(&good, "evaluation"); let treatment = selected(&bad, "evaluation");
    assert_eq!(&control[..3], &treatment[..3]);
    assert_eq!(treatment[3].get("evaluated").unwrap().as_bool(), Some(false));
    assert!(treatment[3].get("counts").unwrap().is_null());
    assert_eq!(treatment[3].get("planned_benign").unwrap().as_u64(), Some(1));
    assert_eq!(treatment[3].get("planned_violation").unwrap().as_u64(), Some(1));
    assert_eq!(selected(&good, "campaign_admitted_work"), selected(&bad, "campaign_admitted_work"));
    let completed = selected(&bad, "campaign_completed_work"); let admitted = selected(&bad, "campaign_admitted_work");
    assert!(completed[0].get("threshold_comparisons").unwrap().as_u64() < admitted[0].get("threshold_comparisons").unwrap().as_u64());
    assert_eq!(selected(&bad, "campaign_remaining")[0].get("threshold_comparisons").unwrap().as_u64(), Some(0));
}

#[test]
fn exact_complete_budgets_pass_and_every_one_unit_short_budget_refuses_before_missing_weights() {
    let root = Root::new(); let args = files(&root);
    let (plan, inputs) = assets::load(&args).unwrap();
    let profile = inputs.model.profile();
    for start in 0..=8 { for length in 0..=8 {
        assert_eq!(profile.estimate(start, length), inputs.model.estimate(start, length));
    } }
    assert!(plan.estimate(profile).is_ok());
    for (section, field) in [("capture_budget", "cases"), ("capture_budget", "original_tokens"),
        ("capture_budget", "kv_coordinates"), ("capture_budget", "scalar_products"),
        ("campaign_budget", "training_visits"), ("campaign_budget", "scoring_bytes"),
        ("campaign_budget", "scoring_coordinates"), ("campaign_budget", "threshold_comparisons")] {
        let root = Root::new(); let args = files(&root); decrement(&args, section, field);
        fs::remove_file(&args[1]).unwrap();
        assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("campaign contract: Limit"), "{section}.{field}");
        assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());
    }
    fs::remove_file(&args[1]).unwrap();
    assert!(!run(&args, &mut Vec::new()).unwrap_err().to_string().contains("campaign contract: Limit"));
}

#[test]
fn malformed_populations_and_wrong_tokenizer_refuse_before_weight_or_codec_execution() {
    let root = Root::new(); let args = files(&root);
    let original = fs::read_to_string(&args[3]).unwrap(); fs::remove_file(&args[1]).unwrap();
    for (before, after) in [("\"schema\":\"fa.learned-kv-probe-campaign/1\"", "\"schema\":\"unrecognized\""),
        ("\"task\":6", "\"task\":5"), ("\"lineage\":106", "\"lineage\":105"),
        ("\"tokens\":[8, 0]", "\"tokens\":[7, 1]"), ("\"tokens\":[9, 1]", "\"tokens\":[9, 260]"),
        ("\"layer\":2,\"side\":\"value\"", "\"layer\":1,\"side\":\"value\""),
        ("\"report_bytes\":1048576", "\"report_bytes\":1")] {
        fs::write(&args[3], &original).unwrap(); replace(&args, before, after);
        let error = run(&args, &mut Vec::new()).unwrap_err().to_string();
        assert!(!error.contains("No such file"), "{before}: {error}");
        assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());
    }
    fs::write(&args[3], original).unwrap();
    let mut tokenizer = fs::read(&args[2]).unwrap(); tokenizer[15] ^= 1; fs::write(&args[2], tokenizer).unwrap();
    assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("campaign contract: Binding"));
}

#[test]
fn wrong_codec_lineage_is_rejected_before_archive_read_and_a_bad_fit_witness_saves_an_explicit_failure() {
    let root = Root::new(); let args = files(&root);
    let (mut plan, _) = assets::load(&args).unwrap();
    let binding = fs::read_to_string(&args[4]).unwrap();
    fs::write(&args[4], binding.replace("\"origin\":1010", "\"origin\":101")).unwrap();
    let archive = fs::read(&args[5]).unwrap(); fs::remove_file(&args[5]).unwrap();
    assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("campaign contract: Duplicate"));
    assert!(!Path::new(&args[6]).exists());
    fs::write(&args[4], binding).unwrap();
    let mut corrupt = archive;
    let descriptor_bytes = u64::from_be_bytes(corrupt[88..96].try_into().unwrap()) as usize;
    let witness_start = 80 + 24 + descriptor_bytes + 8;
    corrupt[witness_start] ^= 1;
    fs::write(&args[5], corrupt).unwrap();
    let (_, inputs) = assets::load(&args).unwrap();
    let allowance = plan.remaining_capture();
    assert!(execute(&mut plan, inputs, Path::new(&args[6]), Path::new(&args[7]), &mut Vec::new()).is_err());
    assert_eq!(plan.remaining_capture(), allowance); assert!(!plan.started());
    let log = rows(&args);
    assert_eq!(selected(&log, "planned_case").len(), 6);
    assert_eq!(selected(&log, "tap_fitted").len(), 0);
    assert_eq!(log.last().unwrap().get("stage").unwrap().as_str(), Some("codec_replay"));
    assert!(!Path::new(&args[7]).exists());
}

#[test]
fn original_capture_arithmetic_failure_keeps_spent_work_and_diagnostic_population_without_a_monitor() {
    let root = Root::new(); let args = files(&root); fixture::weights(Path::new(&args[1]), false, true);
    let (mut plan, inputs) = assets::load(&args).unwrap(); let model = inputs.model.clone();
    let fit_allowance = plan.remaining_campaign();
    assert!(execute(&mut plan, inputs, Path::new(&args[6]), Path::new(&args[7]), &mut Vec::new()).is_err());
    assert_eq!(plan.remaining_capture(), KvCaptureWork::default());
    assert_eq!(plan.remaining_campaign(), fit_allowance);
    assert!(plan.started()); assert!(plan.captured_work().is_none());
    assert!(matches!(plan.run(&model), Err(Error::WrongState)));
    let log = rows(&args); assert_eq!(selected(&log, "planned_case").len(), 6);
    assert_eq!(log.last().unwrap().get("stage").unwrap().as_str(), Some("capture_and_training"));
    assert_eq!(selected(&log, "campaign_complete").len(), 0); assert!(!Path::new(&args[7]).exists());
}

#[test]
fn existing_outputs_and_broken_status_sink_preserve_files_reports_and_spent_campaign() {
    for existing in [6, 7] {
        let root = Root::new(); let args = files(&root); fs::write(&args[existing], b"prior artifact").unwrap();
        assert!(run(&args, &mut Vec::new()).is_err());
        assert_eq!(fs::read(&args[existing]).unwrap(), b"prior artifact");
        assert!(!Path::new(&args[13 - existing]).exists());
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Root::new(); let args = files(&root); let (mut plan, inputs) = assets::load(&args).unwrap();
    let model = inputs.model.clone();
    assert!(execute(&mut plan, inputs, Path::new(&args[6]), Path::new(&args[7]), &mut Broken).is_err());
    assert_eq!(plan.remaining_capture(), KvCaptureWork::default()); assert_eq!(plan.remaining_campaign(), KvCampaignWork::default());
    assert!(matches!(plan.run(&model), Err(Error::WrongState)));
    let log = rows(&args); assert_eq!(selected(&log, "evaluation").len(), 4);
    assert_eq!(selected(&log, "campaign_complete")[0].get("accepted").unwrap().as_bool(), Some(true));
    assert_eq!(kind(log.last().unwrap()), "campaign_failed"); assert!(!Path::new(&args[7]).exists());
}

#[test]
fn an_output_created_after_reporting_cannot_be_overwritten_or_cause_a_refit() {
    struct Racer { path: std::path::PathBuf, created: bool }
    impl Write for Racer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.created { fs::write(&self.path, b"concurrent artifact")?; self.created = true; }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Root::new(); let args = files(&root); let (mut plan, inputs) = assets::load(&args).unwrap();
    let mut racer = Racer { path: Path::new(&args[7]).to_path_buf(), created: false };
    assert!(execute(&mut plan, inputs, Path::new(&args[6]), Path::new(&args[7]), &mut racer).is_err());
    assert_eq!(fs::read(&args[7]).unwrap(), b"concurrent artifact");
    assert_eq!(plan.remaining_campaign(), KvCampaignWork::default());
    let log = rows(&args); assert_eq!(selected(&log, "evaluation").len(), 4);
    assert_eq!(log.last().unwrap().get("stage").unwrap().as_str(), Some("monitor_write"));
}
