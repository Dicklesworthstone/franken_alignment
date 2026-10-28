//! Real numerical histories, complete policy corpus, no recovery authority.
//! Synthetic model fixtures exercise boundaries, not detector quality.
use super::*;
use super::super::super::recovery::policy_replay as replay;
use fa_reference::action::consequence::{
    gate::containment::session::policy::{Policy, Predicate},
    policy_campaign::ReplayLimits,
};

const SIMPLE: &str = r#"{"op":"payload_at_most","maximum":512}"#;

fn request(nodes: &str, cases: usize, input_bytes: usize, output_bytes: usize) -> String {
    format!(
        r#"{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":{cases},"input_bytes":{input_bytes},"output_bytes":{output_bytes}}},"candidate":{{"generation":2,"nodes":[{nodes}]}}}}"#
    )
}
fn candidate(case: &Case) -> PathBuf {
    let path = case.root.0.join("candidate-policy.json");
    fs::write(&path, request(SIMPLE, 64, 1_048_576, 1_048_576)).unwrap();
    path
}
fn replay_args(case: &Case) -> Vec<String> {
    let mut args = vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        "--resume".into(),
        "--policy-replay".into(),
        candidate(case).display().to_string(),
        "--history-anchor".into(),
        case.anchor.display().to_string(),
    ];
    if let Some(path) = &case.topology {
        args.extend(["--topology-profile".into(), path.display().to_string()]);
    }
    args
}
fn run(args: &[String]) -> Vec<u8> {
    let mut output = Vec::new();
    replay::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO).unwrap();
    output
}
fn rejected(case: &Case, args: &[String]) -> String {
    let before = case.files();
    let mut output = Vec::new();
    let error = replay::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO)
        .unwrap_err();
    assert!(output.is_empty(), "{error}");
    case.unchanged(&before);
    error
}
fn value(document: &Json) -> &Json {
    document.get("knowledge").unwrap().get("value").unwrap()
}
fn policy() -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(512)]).unwrap()
}
fn limits() -> ReplayLimits {
    ReplayLimits { cases: 64, input_bytes: 1_048_576 }
}

#[test]
fn complete_corpus_reports_actual_learned_denials_across_all_original_guarded_modes() {
    for mode in [None, Some(false), Some(true)] {
        for mediated in [false, true] {
            let case = Case::new(mode, mediated);
            let before = case.files();
            let args = replay_args(&case);
            let bytes = run(&args);
            let document = json(&bytes);
            assert_eq!(text(&document, "schema_version"), "fa.policy-replay/1");
            let report = value(&document);
            assert_eq!(text(report, "replay_grade"), "decision_replay");
            assert_eq!(text(report, "authority"), "not_issued");
            assert_eq!(report.get("policy_changed").unwrap().as_bool(), Some(false));
            assert_eq!(report.get("helpers_rerun").unwrap().as_bool(), Some(false));
            assert_eq!(report.get("live_evidence_read").unwrap().as_bool(), Some(false));
            assert_eq!(report.get("requires_shadow").unwrap().as_bool(), Some(false));
            assert_eq!(report.get("case_count").unwrap().as_u64(), Some(1));
            let cut = report.get("cut").unwrap();
            assert_eq!(cut.get("journal_revision").unwrap().as_u64(), Some(case.before.revision));
            for field in ["executions", "charged", "reserved"] {
                assert_eq!(cut.get(field).unwrap().as_u64(), Some(0));
            }
            let changed = array(report, "newly_reviewable");
            assert_eq!(changed.len(), 1);
            assert_eq!(text(&changed[0], "kind"), "proposal");
            assert_eq!(changed[0].get("attempt").unwrap().as_u64(), Some(case.attempt));
            assert!(array(report, "newly_blocked").is_empty());
            assert_eq!(text(&array(report, "cases")[0], "delta"), "newly_reviewable");
            assert_eq!(run(&args), bytes);
            // This is a corpus command, not a receipt for the recipe request.
            // A different nonzero external ID must not select away old cases.
            let original = fs::read_to_string(&case.recipe).unwrap();
            assert!(original.contains("\"request\":901"));
            fs::write(
                &case.recipe,
                original.replacen("\"request\":901", "\"request\":902", 1),
            ).unwrap();
            assert_eq!(run(&args), bytes);
            fs::write(&case.recipe, original).unwrap();
            case.unchanged(&before);
        }
    }
}

#[test]
fn missing_observations_remain_shadow_required_even_when_an_any_branch_passes() {
    let case = Case::new(Some(true), true);
    let args = replay_args(&case);
    let before = case.files();
    let nodes = format!(
        r#"{SIMPLE},{{"op":"absent","key":999}},{{"op":"any","children":[0,1]}}"#
    );
    fs::write(&args[5], request(&nodes, 64, 1_048_576, 1_048_576)).unwrap();
    let document = json(&run(&args));
    let report = value(&document);
    assert_eq!(report.get("requires_shadow").unwrap().as_bool(), Some(true));
    assert!(array(report, "newly_reviewable").is_empty());
    let retained = &array(report, "cases")[0];
    assert_eq!(text(retained, "delta"), "requires_shadow");
    let candidate = retained.get("candidate").unwrap();
    assert_eq!(text(candidate, "kind"), "unknown");
    assert_eq!(text(candidate, "reason_code"), "MissingRequiredObservation");
    assert_eq!(candidate.get("missing_nodes").unwrap(), &json(b"[1]"));
    case.unchanged(&before);
}

#[test]
fn finite_corpus_input_output_and_expected_epoch_fail_without_partial_reports() {
    let case = Case::new(None, false);
    let args = replay_args(&case);
    let before = case.files();
    let good = request(SIMPLE, 64, 1_048_576, 1_048_576);
    for bad in [
        request(SIMPLE, 0, 1_048_576, 1_048_576),
        request(SIMPLE, 64, 1, 1_048_576),
        request(SIMPLE, 64, 1_048_576, 1),
        good.replace("\"expected_policy_generation\":1", "\"expected_policy_generation\":0"),
        good.replace("fa.policy-replay-request/1", "fa.policy-replay-request/2"),
    ] {
        fs::write(&args[5], bad).unwrap();
        rejected(&case, &args);
    }
    fs::write(&args[5], good).unwrap();
    assert_eq!(value(&json(&run(&args))).get("case_count").unwrap().as_u64(), Some(1));
    case.unchanged(&before);
}

#[test]
fn closed_original_argv_refuses_mutating_modes_duplicate_selectors_and_missing_values() {
    let base: Vec<String> = [
        "create-learned-generated", "/missing/config", "/missing/recipe", "--resume",
        "--policy-replay", "/missing/candidate", "--history-anchor", "/missing/history",
    ].into_iter().map(str::to_owned).collect();
    let mut variants = Vec::new();
    for flag in [
        "--retain-history-anchor", "--retain-topology-checkpoint", "--actor-profile",
        "--reconcile-receipts", "--receipts-only", "--serve-receipts",
        "--recover-receipt-checkpoint", "--audit-review", "--rotate-credential",
        "--recover-stop", "--stop-admission", "--drain-stopped", "--abandon-unknown",
        "--continue-generation", "--after-message", "--finish-after-message",
        "--credibility-activation", "--explain-proposal", "--payload", "--search",
    ] {
        let mut args = base.clone();
        args.push(flag.into());
        variants.push(args);
    }
    for pair in [
        ["--history-anchor", "/duplicate/anchor"],
        ["--policy-replay", "/duplicate/candidate"],
        ["--topology-profile", "--history-anchor"],
        ["--topology-profile", "relative/path"],
    ] {
        let mut args = base.clone();
        args.extend(pair.into_iter().map(str::to_owned));
        variants.push(args);
    }
    let mut duplicate = base.clone();
    duplicate.extend(["--topology-profile", "/one", "--topology-profile", "/two"]
        .into_iter().map(str::to_owned));
    variants.push(duplicate);
    variants.push(base[..6].to_vec());
    variants.push(base[..7].to_vec());
    let mut missing = base.clone();
    missing[5] = "--history-anchor".into();
    variants.push(missing);
    let mut live = base.clone();
    live[3] = "/reviewer".into();
    variants.push(live);
    for args in variants {
        let mut output = Vec::new();
        let error = replay::command_with_elapsed(
            &args, None, &mut output, &mut || panic!("closed argv before Config"),
        ).unwrap_err();
        assert!(error.starts_with("create-learned-generated"), "{error}");
        assert!(output.is_empty());
        assert!(command_with_clock(&args, None, || panic!("no logical clock")).is_err());
    }
    let mut output = Vec::new();
    assert!(replay::command_with_elapsed(
        &base, Some(Path::new("/missing/capsule")), &mut output,
        &mut || panic!("historical replay cannot activate credibility"),
    ).is_err());
    assert!(output.is_empty());
}

#[test]
fn admission_every_replay_event_ready_canonical_confirmation_and_output_share_one_deadline() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, true);
        let args = replay_args(&case);
        let before = case.files();
        let mut checks = 0;
        replay::command_with_elapsed(&args, None, &mut Vec::new(), &mut || {
            checks += 1;
            Duration::ZERO
        }).unwrap();
        assert!(checks > 12);
        for expire_at in [0, 3, 5, checks / 2, checks - 3, checks - 2, checks - 1] {
            let mut calls = 0;
            let mut output = Vec::new();
            let error = replay::command_with_elapsed(&args, None, &mut output, &mut || {
                let current = calls;
                calls += 1;
                if current == expire_at {
                    Duration::from_millis(case.config.timing.runtime_ms)
                } else {
                    Duration::ZERO
                }
            }).unwrap_err();
            assert!(error.contains("deadline"), "{error}");
            assert!(output.is_empty());
            case.unchanged(&before);
        }
        // Establish Ready and post-canonical cuts independently of admission.
        let mut calls = 0;
        replay::read_with_elapsed(&case.config, &case.loaded, policy(), limits(), &mut || {
            calls += 1;
            Duration::ZERO
        }).unwrap();
        for expire_at in [calls - 2, calls - 1] {
            let mut at = 0;
            assert!(replay::read_with_elapsed(
                &case.config, &case.loaded, policy(), limits(), &mut || {
                    let current = at;
                    at += 1;
                    if current == expire_at {
                        Duration::from_millis(case.config.timing.runtime_ms)
                    } else {
                        Duration::ZERO
                    }
                },
            ).unwrap_err().contains("deadline"));
            case.unchanged(&before);
        }
    }
}

#[test]
fn original_floors_policy_predictor_anchor_and_entire_suffix_remain_mandatory() {
    let case = Case::new(Some(true), false);
    let args = replay_args(&case);
    let before = case.files();
    let original = fs::read_to_string(&case.recipe).unwrap();
    for (old, new) in [
        ("\"journal_revision\":0".to_owned(),
            format!("\"journal_revision\":{}", case.before.revision + 1)),
        ("\"control_sequence\":0".to_owned(),
            format!("\"control_sequence\":{}", case.before.control.sequence + 1)),
        ("\"authority_epoch\":0".to_owned(),
            format!("\"authority_epoch\":{}", case.before.control.ledger.epoch + 1)),
    ] {
        assert!(original.contains(&old));
        fs::write(&case.recipe, original.replacen(&old, &new, 1)).unwrap();
        rejected(&case, &args);
    }
    fs::write(&case.recipe, original).unwrap();
    let profile = fs::read_to_string(&case.config_path).unwrap();
    assert!(profile.contains("\"maximum\": 1"));
    fs::write(&case.config_path, profile.replacen("\"maximum\": 1", "\"maximum\": 2", 1)).unwrap();
    rejected(&case, &args);
    fs::write(&case.config_path, profile).unwrap();
    let forecast = case.root.0.join("forecast.json");
    let predictor = fs::read_to_string(&forecast).unwrap();
    assert!(predictor.contains("\"max_prediction_age_ticks\":60000"));
    fs::write(&forecast,
        predictor.replacen("\"max_prediction_age_ticks\":60000", "\"max_prediction_age_ticks\":60001", 1),
    ).unwrap();
    rejected(&case, &args);
    fs::write(&forecast, predictor).unwrap();
    let anchor = fs::read(&case.anchor).unwrap();
    fs::write(&case.anchor, b"not a retained history anchor").unwrap();
    rejected(&case, &args);
    fs::write(&case.anchor, anchor).unwrap();
    let journal = case.config.store.join("delivery.bin");
    let original = fs::read(&journal).unwrap();
    let mut malformed = original.clone();
    malformed.push(0xff);
    fs::write(&journal, malformed).unwrap();
    rejected(&case, &args);
    fs::write(&journal, original).unwrap();
    assert_eq!(array(value(&json(&run(&args))), "newly_reviewable").len(), 1);
    case.unchanged(&before);
}

#[test]
fn output_errors_and_stale_mediated_source_cuts_never_finalize_or_rebase_replay() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, true);
        let args = replay_args(&case);
        let before = case.files();
        for flush in [false, true] {
            let mut output = FailedOutput { flush, bytes: Vec::new() };
            let error = replay::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                .unwrap_err();
            assert!(error.contains(if flush { "flush failed" } else { "output failed" }));
            assert!(error.contains("no authority or effect was changed"));
            assert_eq!(!output.bytes.is_empty(), flush);
            case.unchanged(&before);
        }
        let saved = topology_fixture::checkpoint(&case.root, 1);
        let pair = format!(r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
            case.root.0.join("initial.json").display(),
            case.root.0.join("initial-inventory.json").display());
        let manual = case.root.0.join("manual-topology.json");
        fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1",
            "initial":{pair},"current":{pair},"next":{pair},"available":true,
            "operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
            saved.last_operation() + 1, saved.work().checks, saved.work().reserved_read_bytes,
            saved.last_operation())).unwrap();
        let mut manual_args = args.clone();
        let topology_at = manual_args.iter().position(|arg| arg == "--topology-profile").unwrap() + 1;
        manual_args[topology_at] = manual.display().to_string();
        assert!(rejected(&case, &manual_args).contains("exact v2"));
        // The explicit live recovery is only the causal-negative setup. It
        // appends a real Fence; the report command must not rebase old custody.
        let (host, _, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
        assert_eq!(host.revision(), case.before.revision + 1);
        let newer = case.root.0.join("newer.anchor");
        history::export_acknowledged(&host.history_anchor().unwrap(), &newer).unwrap();
        drop(host);
        let changed = case.files();
        assert!(rejected(&case, &args).contains("checkpoint"));
        let mut different_cut = args.clone();
        different_cut[7] = newer.display().to_string();
        assert!(rejected(&case, &different_cut).contains("different cuts"));
        case.unchanged(&changed);
    }
}
