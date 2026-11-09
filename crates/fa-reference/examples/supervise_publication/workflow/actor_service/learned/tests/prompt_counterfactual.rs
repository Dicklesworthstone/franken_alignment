//! Real checkpoint intake, sampled source effects and retained failure denominators.
#[path = "prompt_counterfactual/fixture.rs"]
mod fixture;
use super::{Root, configured};
use super::super::{prompt_counterfactual as command, recipe, recovery};
use fixture::{Case, at, budget, integer, original_plan, parse, set, snapshot};
use fa_reference::strict_json::Json;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    experiment::prompt::campaign::PromptCampaignBudget,
    sampling::monitored::{GenerationSpec, GenerationStatus, GenerationStop},
};
use std::{fs, io::{self, Write}, time::Duration};

fn text<'a>(value: &'a Json, keys: &[&str]) -> &'a str { at(value, keys).as_str().unwrap() }
fn number(value: &Json, keys: &[&str]) -> u64 { text(value, keys).parse().unwrap() }
fn value(bytes: &[u8]) -> Json {
    let output = parse(bytes);
    assert_eq!(text(&output, &["schema"]), "fa.learned-prompt-counterfactual-result/1");
    assert_eq!(text(&output, &["purpose"]), "experiment");
    assert_eq!(text(&output, &["effect_authority"]), "none");
    assert_eq!(text(&output, &["knowledge", "state"]), "known");
    assert_eq!(at(&output, &["knowledge", "basis", "authenticated_model"]).as_bool(), Some(false));
    at(&output, &["knowledge", "value"]).clone()
}
fn run(case: &Case) -> Json {
    let mut output = Vec::new();
    command::command_with_elapsed(&case.args(), &mut output, &mut || Duration::ZERO).unwrap();
    value(&output)
}

#[test]
fn public_command_runs_real_paired_prompt_effect_and_sham_without_touching_a_live_store() {
    let root = Root::new(); let mut case = Case::new(&root);
    let config = configured(&root);
    let loaded = recipe::load(&root.0.join("recipe.json"), &config, false).unwrap();
    let (host, _) = recovery::create(&config, &loaded).unwrap();
    let revision = host.revision();
    for sham in [false, true] {
        set(&mut case.plan, &["edit", "replacement"], parse(if sham { b"[121]" } else { b"[65]" }));
        case.write();
        let before = snapshot(&root.0);
        let mut output = Vec::new();
        crate::command_with_output(case.args(), &mut output).unwrap();
        let result = value(&output);
        assert_eq!(text(&result, &["status", "kind"]), "finished");
        assert_eq!(number(&result, &["denominator", "recorded_trials"]), 3);
        assert_eq!(number(&result, &["denominator", "observed_pairs"]), 3);
        assert_eq!(number(&result, &["denominator", "unstarted_trials"]), 0);
        assert_eq!(at(&result, &["seeds"]).as_array().unwrap()[2].as_str(), Some("18446744073709551615"));
        let plan = original_plan(&root, sham);
        let model = fixture::model(&root);
        for (row, seed) in at(&result, &["trials"]).as_array().unwrap().iter().zip([0, 7, u64::MAX]) {
            assert_eq!(number(row, &["seed"]), seed);
            for (name, spec) in [("baseline", &plan.config().original), ("treated", plan.treated_spec())] {
                let mut sampling = spec.sampling().clone(); sampling.seed = seed;
                let spec = GenerationSpec::new(spec.prompt().to_vec(), spec.max_new_tokens(),
                    spec.stop_tokens().clone(), sampling).unwrap();
                let mut oracle = model.monitored_generation_with_telemetry(5, 201, spec,
                    plan.config().policy.clone(), plan.config().generation, plan.config().telemetry).unwrap();
                oracle.run_to_stop().unwrap();
                let observed = oracle.generated_tokens().contains(&65);
                assert_eq!(text(row, &["outcomes", name, "kind"]),
                    if observed { "present" } else { "absent_within_completed_horizon" });
                assert_eq!(number(row, &["comparison", name, "numerical", "admitted_tokens"]), oracle.work().admitted_tokens);
                assert_eq!(number(row, &["comparison", name, "numerical", "sampling_attempts"]), oracle.work().sampling_attempts);
                assert_eq!(number(row, &["comparison", name, "numerical", "reserved_decoder_products"]), oracle.work().reserved_decoder_products);
                assert_eq!(number(row, &["comparison", name, "telemetry", "monitor_probe_coordinates"]), oracle.telemetry_work().monitor_probe_coordinates);
                assert!(oracle.work().accepted_decoder.attention_products > 0);
                assert_eq!(oracle.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
            }
            assert_eq!(text(row, &["outcomes", "baseline", "kind"]), "present");
            assert_eq!(text(row, &["outcomes", "treated", "kind"]),
                if sham { "present" } else { "absent_within_completed_horizon" });
            if sham { assert_eq!(at(row, &["comparison", "baseline"]), at(row, &["comparison", "treated"])); }
        }
        assert_eq!(snapshot(&root.0), before);
        assert_eq!(host.revision(), revision);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn explicit_checkpoint_source_uses_the_same_original_numerical_campaign() {
    let root = Root::new(); let mut case = Case::new(&root);
    let direct = run(&case);
    let path = root.0.join("checkpoint-source.json");
    fs::write(&path, format!(r#"{{"schema":"fa.checkpoint-weights/1","base":{{"kind":"single","path":"{}/weights.safetensors"}},"adapter":null,"limits":{{"weight_bytes":1048576,"weight_read_calls":4096}}}}"#, root.0.display())).unwrap();
    set(&mut case.plan, &["files", "weights"], parse(format!(r#"{{"format":"checkpoint_source","path":"{}"}}"#, path.display()).as_bytes()));
    case.write();
    let before = snapshot(&root.0);
    let selected = run(&case);
    assert_eq!(at(&selected, &["trials"]), at(&direct, &["trials"]));
    assert_eq!(at(&selected, &["outcome_table"]), at(&direct, &["outcome_table"]));
    assert!(number(&selected, &["input", "asset_bytes"]) > number(&direct, &["input", "asset_bytes"]));
    assert_eq!(snapshot(&root.0), before);
    assert!(!root.0.join("store").exists());
}

#[test]
fn complete_argv_and_fixed_schedule_refuse_live_options_unknown_fields_and_ambiguous_trials() {
    let root = Root::new(); let mut case = Case::new(&root);
    for extra in [vec!["--credibility-activation", "/missing/activation"],
        vec!["--history-anchor", "/missing/anchor"], vec!["--continue-generation"],
        vec!["--actor-profile", "/missing/actor"], vec!["--retain-history-anchor", "/missing/output"]] {
        let mut args = case.args(); args.extend(extra.into_iter().map(str::to_owned));
        let mut output = Vec::new();
        let error = crate::command_with_output(args, &mut output).unwrap_err();
        assert_eq!(error, "compare-learned-prompt PLAN"); assert!(output.is_empty());
    }
    let original = case.plan.clone();
    for change in 0..7 {
        case.plan = original.clone();
        match change {
            0 => set(&mut case.plan, &["schedule", "seeds"], parse(b"[7,7]")),
            1 => set(&mut case.plan, &["schedule", "seeds"], parse(b"[]")),
            2 => set(&mut case.plan, &["schedule", "outcome_tokens"], parse(b"[]")),
            3 => set(&mut case.plan, &["campaign"], integer(0)),
            4 => set(&mut case.plan, &["schedule", "seeds"], parse(b"[1.5]")),
            5 => set(&mut case.plan, &["schedule", "seeds"], parse(b"[18446744073709551616]")),
            _ => { let Json::Object(fields) = &mut case.plan else { unreachable!() };
                fields.insert("store".into(), Json::String(root.0.join("store").display().to_string())); },
        }
        case.write();
        assert!(command::command_with_elapsed(&case.args(), &mut Vec::new(),
            &mut || panic!("invalid frozen plan reached runtime")).is_err());
    }
    case.plan = original;
    set(&mut case.plan, &["schedule", "outcome_tokens"], parse(b"[999]")); case.write();
    assert!(command::command_with_elapsed(&case.args(), &mut Vec::new(), &mut || Duration::ZERO).is_err());
    assert!(!root.0.join("store").exists());
}

#[test]
fn exact_prompt_span_and_independent_fit_lineage_are_checked_before_weight_intake() {
    let root = Root::new(); let mut case = Case::new(&root);
    let original = case.plan.clone();
    fs::rename(root.0.join("weights.safetensors"), root.0.join("kept-weights")).unwrap();
    for change in 0..3 {
        case.plan = original.clone();
        match change {
            0 => set(&mut case.plan, &["edit", "expected"], parse(b"[120]")),
            1 => set(&mut case.plan, &["model", "evaluation_origin"], integer(101)),
            _ => set(&mut case.plan, &["model", "stream"], integer(11)),
        }
        case.write(); let before = snapshot(&root.0);
        let mut output = Vec::new();
        let error = command::command_with_elapsed(&case.args(), &mut output, &mut || Duration::ZERO).unwrap_err();
        assert!(error.contains(if change == 0 { "expected token span" } else { "fit inventory" }), "{error}");
        assert!(output.is_empty()); assert_eq!(snapshot(&root.0), before);
    }
    fs::rename(root.0.join("kept-weights"), root.0.join("weights.safetensors")).unwrap();
    case.plan = original; case.write();
    assert_eq!(number(&run(&case), &["denominator", "observed_pairs"]), 3);
}

#[test]
fn complete_pair_schedule_capacity_and_aggregate_asset_bytes_have_real_boundaries() {
    let root = Root::new(); let mut case = Case::new(&root);
    let exact = original_plan(&root, false).estimate_campaign(3, 1).unwrap();
    set(&mut case.plan, &["budget"], budget(exact)); case.write();
    let admitted = run(&case);
    assert_eq!(number(&admitted, &["denominator", "recorded_trials"]), 3);
    for short in [PromptCampaignBudget { trials: exact.trials - 1, ..exact },
        PromptCampaignBudget { positions: exact.positions - 1, ..exact },
        PromptCampaignBudget { decoder_products: exact.decoder_products - 1, ..exact },
        PromptCampaignBudget { vocabulary_scores: exact.vocabulary_scores - 1, ..exact },
        PromptCampaignBudget { outcome_comparisons: exact.outcome_comparisons - 1, ..exact }] {
        set(&mut case.plan, &["budget"], budget(short)); case.write();
        let before = snapshot(&root.0); let mut output = Vec::new();
        let error = command::command_with_elapsed(&case.args(), &mut output, &mut || Duration::ZERO).unwrap_err();
        assert!(error.contains("Limit"), "{error}"); assert!(output.is_empty());
        assert_eq!(snapshot(&root.0), before);
    }
    set(&mut case.plan, &["budget"], budget(exact));
    // The plan's own bytes consume the same asset allowance as all its files.
    set(&mut case.plan, &["asset_bytes"], integer(1)); case.write();
    let error = command::command_with_elapsed(&case.args(), &mut Vec::new(), &mut || Duration::ZERO).unwrap_err();
    assert!(error.contains("asset limit"), "{error}");
    assert!(!root.0.join("store").exists());
}

#[test]
fn held_failed_and_admission_failed_trials_preserve_denominators_without_false_absence() {
    let root = Root::new(); let mut case = Case::new(&root);
    let original = case.plan.clone();
    let monitor_path = root.0.join("learned-monitor.json");
    let quiet = fs::read(&monitor_path).unwrap();
    for mode in 0..3 {
        case.plan = original.clone(); fs::write(&monitor_path, &quiet).unwrap();
        match mode {
            0 => fs::write(&monitor_path, super::fixture::monitoring(-100.0)).unwrap(),
            1 => set(&mut case.plan, &["generation", "telemetry", "source_check_values"], integer(0)),
            _ => set(&mut case.plan, &["generation", "decoder_products"], integer(0)),
        }
        case.write();
        let result = run(&case);
        assert_eq!(text(&result, &["status", "kind"]), "finished");
        assert_eq!(number(&result, &["denominator", "declared_trials"]), 3);
        assert_eq!(number(&result, &["denominator", "recorded_trials"]), 3);
        for row in at(&result, &["trials"]).as_array().unwrap() {
            if mode == 2 {
                assert_eq!(text(row, &["disposition", "kind"]), "admission_failed");
                assert_eq!(text(row, &["disposition", "error"]), "limit");
                assert!(at(row, &["outcomes"]).is_null() && at(row, &["comparison"]).is_null());
            } else {
                for arm in ["baseline", "treated"] {
                    assert_eq!(text(row, &["outcomes", arm, "kind"]), "censored");
                    assert_eq!(text(row, &["comparison", arm, "status", "kind"]), if mode == 0 { "held" } else { "failed" });
                    assert_eq!(number(row, &["comparison", arm, "numerical", "sampling_attempts"]), 0);
                    assert_eq!(number(row, &["comparison", arm, "attempted_calls"]), 1);
                    if mode == 1 {
                        assert_eq!(text(row, &["comparison", arm, "status", "error"]), "limit");
                        assert_eq!(at(row, &["comparison", arm, "all_attempted_telemetry_reported"]).as_bool(), Some(false));
                    }
                }
            }
        }
        assert_eq!(number(&result, &["denominator", "admission_failures"]), if mode == 2 { 3 } else { 0 });
        assert_eq!(number(&result, &["denominator", "observed_pairs"]), if mode == 2 { 0 } else { 3 });
    }
}

#[test]
fn every_deadline_boundary_preserves_partial_work_and_unstarted_seeds_without_late_inference() {
    let root = Root::new(); let case = Case::new(&root);
    let mut calls = 0;
    command::command_with_elapsed(&case.args(), &mut Vec::new(), &mut || { calls += 1; Duration::ZERO }).unwrap();
    let mut partial = false; let mut completed = false; let mut before_inference = false;
    for cut in 1..=calls {
        let before = snapshot(&root.0); let mut observed = 0; let mut output = Vec::new();
        let result = command::command_with_elapsed(&case.args(), &mut output, &mut || {
            observed += 1;
            if observed >= cut { Duration::from_millis(60_000) } else { Duration::ZERO }
        });
        assert!(result.is_err(), "deadline boundary {cut} returned success");
        if output.is_empty() { before_inference = true; } else {
            let result = value(&output);
            let declared = number(&result, &["denominator", "declared_trials"]);
            let started = number(&result, &["denominator", "started_trials"]);
            let recorded = number(&result, &["denominator", "recorded_trials"]);
            assert_eq!(declared, started + number(&result, &["denominator", "unstarted_trials"]));
            assert_eq!(started, recorded + number(&result, &["denominator", "unrecorded_started_trials"]));
            match text(&result, &["status", "kind"]) {
                "cancelled" => {
                    assert_eq!(number(&result, &["cancelled_at_ms"]), 60_000);
                    assert_eq!(at(&result, &["deadline_reached"]).as_bool(), Some(true));
                    for row in at(&result, &["trials"]).as_array().unwrap() {
                        if text(row, &["disposition", "kind"]) == "interrupted" {
                            assert!(at(row, &["outcomes"]).is_null());
                            assert!(number(row, &["comparison", "baseline", "numerical", "admitted_tokens"]) > 0);
                            partial = true;
                        }
                    }
                }
                "finished" => { completed = true; assert_eq!(recorded, declared); }
                other => panic!("unexpected deadline outcome {other}"),
            }
        }
        assert_eq!(snapshot(&root.0), before);
    }
    assert!(before_inference && partial && completed);
}

#[test]
fn lost_result_does_not_repeat_completed_numerical_work_or_create_any_artifact() {
    struct LostReport { calls: usize }
    impl Write for LostReport {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1; Err(io::Error::other("lost experiment report"))
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Root::new(); let case = Case::new(&root); let before = snapshot(&root.0);
    let mut expected_calls = 0;
    command::command_with_elapsed(&case.args(), &mut Vec::new(), &mut || { expected_calls += 1; Duration::ZERO }).unwrap();
    let mut output = LostReport { calls: 0 }; let mut observed_calls = 0;
    let error = command::command_with_elapsed(&case.args(), &mut output,
        &mut || { observed_calls += 1; Duration::ZERO }).unwrap_err();
    assert!(error.contains("lost experiment report") && error.contains("no inference retry"), "{error}");
    assert_eq!(output.calls, 1);
    assert!(observed_calls <= expected_calls);
    assert_eq!(snapshot(&root.0), before);
}
