//! Original learned requests, full guarded replay, and capability-free experiments.
//! Synthetic numerical fixtures exercise execution paths, not detector quality.
use super::super::{
    anchors as history, recovery::diagnosis as operator, topology as source_custody,
};
use super::mediated_topology as topology_fixture;
use super::*;
use fa_reference::action::consequence::experiment::InterventionScope;
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

#[path = "diagnosis/fixture.rs"]
mod cases;
use cases::Case;

fn json(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, Limits::default()).unwrap()
}
fn text<'a>(value: &'a Json, key: &str) -> &'a str {
    value.get(key).unwrap().as_str().unwrap()
}
fn array<'a>(value: &'a Json, key: &str) -> &'a [Json] {
    let Json::Array(values) = value.get(key).unwrap() else {
        panic!("array {key}");
    };
    values
}
fn invoke(args: &[String]) -> Vec<u8> {
    let mut output = Vec::new();
    operator::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO).unwrap();
    output
}
fn refused(case: &Case, args: &[String]) -> String {
    let before = case.files();
    let mut output = Vec::new();
    let error = operator::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO)
        .unwrap_err();
    assert!(output.is_empty(), "{error}");
    case.unchanged(&before);
    error
}
fn menu(case: &Case, stop: Option<usize>) -> PathBuf {
    let stop = stop
        .map(|count| format!(",\"stop_after_cases\":{count}"))
        .unwrap_or_default();
    let path = case.root.0.join("search.json");
    fs::write(
        &path,
        format!(
            r#"{{"schema_version":"fa.proposal-search/1",
        "scope":{{"id":81,"payload":true,"target":false,"units":false,"keys":[]}},
        "budget":{{"cases":2,"retained_edit_bytes":0}},
        "candidates":[{{"kind":"payload","bytes_hex":""}}]{stop}}}"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn actual_command_explains_unreviewed_denials_and_payload_experiments_across_all_original_cursors()
{
    for mode in [None, Some(false), Some(true)] {
        for mediated in [false, true] {
            let case = Case::new(mode, mediated);
            let before = case.files();
            let args = case.args();
            let baseline = json(&invoke(&args));
            assert_eq!(text(&baseline, "schema_version"), "fa.proposal-diagnosis/1");
            assert_eq!(text(&baseline, "request"), "901");
            assert_eq!(text(&baseline, "attempt"), case.attempt.to_string());
            assert_eq!(
                text(&baseline, "journal_revision"),
                case.before.revision.to_string()
            );
            assert_eq!(text(&baseline, "admission_state"), "Denied");
            assert_eq!(text(&baseline, "ending_state"), "Denied");
            assert_eq!(text(&baseline, "baseline"), "violated");
            assert_eq!(text(&baseline, "counterfactual"), "violated");
            assert_eq!(text(&baseline, "permission"), "not_issued");
            assert_eq!(
                text(&baseline, "next_requirement"),
                "exact_policy_violation"
            );
            assert_eq!(text(&baseline, "executions_at_capture"), "0");
            assert!(baseline.get("witness_count").unwrap().as_u64().unwrap() > 0);
            assert!(!array(baseline.get("predicate_tree").unwrap(), "nodes").is_empty());

            let payload = case.root.0.join("empty-payload.bin");
            fs::write(&payload, []).unwrap();
            let mut changed = args.clone();
            changed.extend(["--payload".into(), payload.display().to_string()]);
            let repaired = json(&invoke(&changed));
            assert_eq!(text(&repaired, "baseline"), "violated");
            assert_eq!(text(&repaired, "counterfactual"), "satisfied");
            assert_eq!(
                text(&repaired, "next_requirement"),
                "fresh_independent_review"
            );
            assert_eq!(text(&repaired, "purpose"), "experiment");
            assert_eq!(text(&repaired, "permission"), "not_issued");
            assert_eq!(text(&repaired, "admission_state"), "Denied");
            assert_eq!(text(&repaired, "ending_state"), "Denied");
            assert_eq!(text(&repaired, "executions_at_capture"), "0");
            assert_eq!(json(&invoke(&args)), baseline);
            case.unchanged(&before);
        }
    }
}

#[test]
fn original_repair_search_keeps_untouched_failure_and_complete_or_cancelled_cases() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, false);
        let before = case.files();
        let mut args = case.args();
        args.extend(["--search".into(), menu(&case, None).display().to_string()]);
        let complete = json(&invoke(&args));
        assert_eq!(
            text(&complete, "schema_version"),
            "fa.proposal-search-result/1"
        );
        assert_eq!(text(&complete, "status"), "complete");
        assert_eq!(text(&complete, "permission"), "not_issued");
        assert_eq!(array(&complete, "cases").len(), 2);
        let first = array(&complete, "cases")[0].get("result").unwrap();
        let second = array(&complete, "cases")[1].get("result").unwrap();
        assert_eq!(text(first, "counterfactual"), "violated");
        assert_eq!(text(second, "counterfactual"), "satisfied");
        let repairs = array(&complete, "repairs");
        assert_eq!(repairs.len(), 1);
        assert_eq!(repairs[0].get("mask").unwrap().as_u64(), Some(1));
        assert_eq!(text(&repairs[0], "minimality"), "established_within_menu");
        assert_eq!(
            text(&repairs[0], "next_requirement"),
            "fresh_independent_review"
        );
        assert!(array(&complete, "unreported_masks").is_empty());

        menu(&case, Some(1));
        let mut output = Vec::new();
        let error =
            operator::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                .unwrap_err();
        assert!(error.contains("incomplete"));
        let partial = json(&output);
        assert_eq!(text(&partial, "status"), "cancelled");
        assert_eq!(array(&partial, "cases").len(), 1);
        assert_eq!(array(&partial, "unreported_masks")[0].as_u64(), Some(1));
        assert!(array(&partial, "repairs").is_empty());
        assert_eq!(text(&partial, "permission"), "not_issued");
        case.unchanged(&before);
    }
}

#[test]
fn independent_floors_policy_predictor_and_exact_request_refuse_without_diagnostic_output() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, false);
        let before = case.files();
        let original = fs::read_to_string(&case.recipe).unwrap();
        for (old, new) in [
            ("\"request\":901".to_owned(), "\"request\":902".to_owned()),
            (
                "\"journal_revision\":0".to_owned(),
                format!("\"journal_revision\":{}", case.before.revision + 1),
            ),
            (
                "\"control_sequence\":0".to_owned(),
                format!("\"control_sequence\":{}", case.before.control.sequence + 1),
            ),
            (
                "\"authority_epoch\":0".to_owned(),
                format!(
                    "\"authority_epoch\":{}",
                    case.before.control.ledger.epoch + 1
                ),
            ),
        ] {
            assert!(original.contains(&old));
            let variant = case.root.0.join("different-recipe.json");
            fs::write(&variant, original.replacen(&old, &new, 1)).unwrap();
            let mut args = case.args();
            args[2] = variant.display().to_string();
            refused(&case, &args);
        }
        let config = fs::read_to_string(&case.config_path).unwrap();
        let changed = case.root.0.join("different-policy.json");
        fs::write(
            &changed,
            config.replacen("\"maximum\": 1", "\"maximum\": 2", 1),
        )
        .unwrap();
        let mut args = case.args();
        args[1] = changed.display().to_string();
        refused(&case, &args);
        let mut args = case.args();
        args.extend(["--finish-after-message".into(), "900".into()]);
        refused(&case, &args);
        if mode.is_some() {
            let predictor = case.root.0.join("forecast.json");
            let saved = fs::read_to_string(&predictor).unwrap();
            assert!(saved.contains("\"max_prediction_age_ticks\":60000"));
            fs::write(
                &predictor,
                saved.replacen(
                    "\"max_prediction_age_ticks\":60000",
                    "\"max_prediction_age_ticks\":60001",
                    1,
                ),
            )
            .unwrap();
            refused(&case, &case.args());
            fs::write(&predictor, saved).unwrap();
        }
        assert_eq!(text(&json(&invoke(&case.args())), "baseline"), "violated");
        case.unchanged(&before);
    }
}

#[test]
fn original_argv_rejects_incompatible_selectors_and_missing_values_before_input_or_clock() {
    let base: Vec<String> = [
        "create-learned-generated",
        "/missing/config",
        "/missing/recipe",
        "--resume",
        "--explain-proposal",
        "--history-anchor",
        "/missing/history",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut cases = Vec::new();
    for flag in [
        "--retain-history-anchor",
        "--retain-topology-checkpoint",
        "--actor-profile",
        "--reconcile-receipts",
        "--receipts-only",
        "--serve-receipts",
        "--recover-receipt-checkpoint",
        "--audit-review",
        "--rotate-credential",
        "--recover-stop",
        "--continue-generation",
        "--after-message",
        "--credibility-activation",
        "--explain-proposal",
    ] {
        let mut args = base.clone();
        args.push(flag.into());
        cases.push(args);
    }
    for pair in [
        ["--history-anchor", "/duplicate/input"],
        ["--payload", "--explain-proposal"],
        ["--topology-profile", "--history-anchor"],
        ["--finish-after-message", "--explain-proposal"],
        ["--finish-after-message", "0"],
        ["--finish-after-message", "18446744073709551616"],
    ] {
        let mut args = base.clone();
        args.extend(pair.into_iter().map(str::to_owned));
        cases.push(args);
    }
    let mut both = base.clone();
    both.extend(
        ["--payload", "/payload", "--search", "/menu"]
            .into_iter()
            .map(str::to_owned),
    );
    cases.push(both);
    cases.push(base[..5].to_vec());
    let mut missing = base.clone();
    missing.pop();
    cases.push(missing);
    let mut missing_base = base.clone();
    missing_base[2] = "--explain-proposal".into();
    cases.push(missing_base);
    let mut no_resume = base.clone();
    no_resume[3] = "/reviewer".into();
    cases.push(no_resume);
    for args in cases {
        let mut output = Vec::new();
        let error = operator::command_with_elapsed(&args, None, &mut output, &mut || {
            panic!("closed admission precedes Config")
        })
        .unwrap_err();
        assert!(error.starts_with("create-learned-generated"), "{error}");
        assert!(output.is_empty());
        assert!(
            command_with_clock(&args, None, || panic!(
                "diagnosis cannot read a logical clock"
            ))
            .is_err()
        );
    }
    let mut output = Vec::new();
    assert!(
        operator::command_with_elapsed(
            &base,
            Some(Path::new("/missing/capsule")),
            &mut output,
            &mut || panic!("credibility is not diagnosis")
        )
        .is_err()
    );
    assert!(output.is_empty());
}

#[test]
fn replay_ready_and_postfinalization_deadlines_withhold_both_diagnosis_and_search() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, false);
        let before = case.files();
        for searching in [false, true] {
            let mut args = case.args();
            if searching {
                args.extend(["--search".into(), menu(&case, None).display().to_string()]);
            }
            let mut checks = 0;
            operator::command_with_elapsed(&args, None, &mut Vec::new(), &mut || {
                checks += 1;
                Duration::ZERO
            })
            .unwrap();
            assert!(checks > 12);
            // Includes admission, first replay, an interior original event,
            // Ready, canonical confirmation, evaluation and pre-output cuts.
            for expire_at in [
                0,
                3,
                5,
                checks / 2,
                checks - 4,
                checks - 3,
                checks - 2,
                checks - 1,
            ] {
                let mut calls = 0;
                let mut output = Vec::new();
                let error = operator::command_with_elapsed(&args, None, &mut output, &mut || {
                    let current = calls;
                    calls += 1;
                    if current == expire_at {
                        Duration::from_millis(case.config.timing.runtime_ms)
                    } else {
                        Duration::ZERO
                    }
                })
                .unwrap_err();
                assert!(error.contains("deadline"), "{error}");
                assert!(output.is_empty());
                case.unchanged(&before);
            }
        }
        let scope = InterventionScope::new(case.loaded.request, false, false, false, &[]).unwrap();
        let mut calls = 0;
        operator::read_with_elapsed(&case.config, &case.loaded, None, scope.clone(), &mut || {
            calls += 1;
            Duration::ZERO
        })
        .unwrap();
        // Exact replay-only boundaries establish Ready independently of the
        // number of source/recipe ingestion or search checkpoints above.
        for expire_at in [calls - 2, calls - 1] {
            let mut at = 0;
            assert!(
                operator::read_with_elapsed(
                    &case.config,
                    &case.loaded,
                    None,
                    scope.clone(),
                    &mut || {
                        let current = at;
                        at += 1;
                        if current == expire_at {
                            Duration::from_millis(case.config.timing.runtime_ms)
                        } else {
                            Duration::ZERO
                        }
                    }
                )
                .unwrap_err()
                .contains("deadline")
            );
            case.unchanged(&before);
        }
    }
}

#[test]
fn real_finish_requires_its_original_confirmed_predecessor_before_immutable_diagnosis() {
    let root = Root::new();
    let (mut config, _, reviewer, mut driver, mut wire, limit) = super::finish::confirmed(&root);
    let original = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    let recipe = root.0.join("finish-diagnosis.json");
    fs::write(
        &recipe,
        original
            .replacen("\"request\":1", "\"request\":2", 1)
            .replacen("\"object_id\":1001", "\"object_id\":1002", 1),
    )
    .unwrap();
    let loaded = recipe::load(&recipe, &config, true).unwrap();
    let mut control = Control::new(&config, 2, None).unwrap();
    assert!(
        super::super::finish::prepare(
            &mut driver,
            &reviewer,
            &mut config,
            &loaded,
            1,
            &mut control,
            &limit,
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    submit_release(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        LearnedTextRelease::Finish,
        &limit,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let anchor = root.0.join("finish.anchor");
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    history::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    drop(host);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(wire);
    cases::remove_live(&root, false);
    fs::write(
        config.store.join("delivery.pending"),
        b"preserve finish staging",
    )
    .unwrap();
    let config_path = cases::config_file(&root, false);
    crate::config::assert_oversight_profiles_eq(
        &Config::read(&config_path).unwrap().profile,
        &config.profile,
    );
    let before = cases::files(&config.store, &anchor, None);
    let base = vec![
        "create-learned-generated".into(),
        config_path.display().to_string(),
        recipe.display().to_string(),
        "--resume".into(),
        "--explain-proposal".into(),
        "--history-anchor".into(),
        anchor.display().to_string(),
    ];
    for predecessor in [None, Some(2), Some(900)] {
        let mut args = base.clone();
        if let Some(previous) = predecessor {
            args.extend(["--finish-after-message".into(), previous.to_string()]);
        }
        let mut output = Vec::new();
        assert!(
            operator::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                .is_err()
        );
        assert!(output.is_empty());
        assert_eq!(cases::files(&config.store, &anchor, None), before);
    }
    let mut args = base;
    args.extend(["--finish-after-message".into(), "1".into()]);
    let result = json(&invoke(&args));
    assert_eq!(text(&result, "request"), "2");
    assert_eq!(text(&result, "baseline"), "satisfied");
    assert_eq!(text(&result, "executions_at_capture"), "1");
    assert_eq!(text(&result, "permission"), "not_issued");
    assert_eq!(
        text(&result, "next_requirement"),
        "fresh_independent_review"
    );
    assert_eq!(cases::files(&config.store, &anchor, None), before);
}

struct FailedOutput {
    flush: bool,
    bytes: Vec<u8>,
}
impl Write for FailedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.flush {
            return Err(io::Error::other("fixture write failure"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("fixture flush failure"))
    }
}

#[test]
fn output_failures_and_malformed_suffix_never_change_original_authority_or_staging() {
    let case = Case::new(Some(true), false);
    let before = case.files();
    for flush in [false, true] {
        let mut output = FailedOutput {
            flush,
            bytes: Vec::new(),
        };
        let error =
            operator::command_with_elapsed(&case.args(), None, &mut output, &mut || Duration::ZERO)
                .unwrap_err();
        assert!(error.contains(if flush {
            "flush failed"
        } else {
            "output failed"
        }));
        assert!(error.contains("no authority or effect was changed"));
        assert_eq!(!output.bytes.is_empty(), flush);
        case.unchanged(&before);
    }
    let journal = case.config.store.join("delivery.bin");
    let original = fs::read(&journal).unwrap();
    let mut malformed = original.clone();
    malformed.push(0xff);
    fs::write(&journal, malformed).unwrap();
    refused(&case, &case.args());
    fs::write(&journal, original).unwrap();
    assert_eq!(text(&json(&invoke(&case.args())), "baseline"), "violated");
    case.unchanged(&before);
}

#[test]
fn topology_diagnosis_requires_the_complete_matching_source_cut_without_current_sources() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, true);
        let before = case.files();
        assert_eq!(text(&json(&invoke(&case.args())), "baseline"), "violated");
        case.unchanged(&before);
        let saved = topology_fixture::checkpoint(&case.root, 1);
        let pair = format!(
            r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
            case.root.0.join("initial.json").display(),
            case.root.0.join("initial-inventory.json").display()
        );
        let manual = case.root.0.join("manual-topology.json");
        fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1",
            "initial":{pair},"current":{pair},"next":{pair},"available":true,
            "operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
            saved.last_operation() + 1, saved.work().checks, saved.work().reserved_read_bytes,
            saved.last_operation())).unwrap();
        let mut manual_args = case.args();
        let at = manual_args
            .iter()
            .position(|arg| arg == "--topology-profile")
            .unwrap()
            + 1;
        manual_args[at] = manual.display().to_string();
        assert!(refused(&case, &manual_args).contains("exact v2"));
        let (host, _, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
        assert_eq!(host.revision(), case.before.revision + 1);
        let newer = case.root.0.join("newer.anchor");
        history::export_acknowledged(&host.history_anchor().unwrap(), &newer).unwrap();
        drop(host);
        let changed = case.files();
        assert!(refused(&case, &case.args()).contains("checkpoint"));
        let mut args = case.args();
        let index = args
            .iter()
            .position(|arg| arg == "--history-anchor")
            .unwrap()
            + 1;
        args[index] = newer.display().to_string();
        assert!(refused(&case, &args).contains("different cuts"));
        case.unchanged(&changed);
    }
}
