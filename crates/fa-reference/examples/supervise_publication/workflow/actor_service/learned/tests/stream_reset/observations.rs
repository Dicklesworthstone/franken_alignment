//! Restored functional histories remain observable and stoppable through the
//! actual operators. No read is a fresh owner, renewed evidence or restart claim.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::FileLearnedSidecarFinish;
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};

fn completed(root: &Root) -> (Case, Vec<String>) {
    let case = super::prepared(root, false, false);
    let arguments = super::args(root, &case);
    let mut output = Vec::new();
    reset::command(&arguments, None, || ElapsedTick(1000), &mut output).unwrap();
    assert_eq!(report(&output).get("restored").and_then(Json::as_bool), Some(true));
    for name in [
        "evidence.json", "native-roster.json", "helper-model.json",
        "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::write(case.config.store.join("delivery.pending"), b"readers preserve this pending file").unwrap();
    (case, arguments)
}

fn files(case: &Case) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&case.config.store).unwrap().map(|entry| {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    }).collect()
}

fn parse(bytes: &[u8]) -> Json {
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    strict_json::parse(bytes, Limits::default()).unwrap()
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| value.get(name).unwrap())
}

fn audit_args(arguments: &[String]) -> Vec<String> {
    vec![
        "create-learned-generated".into(), arguments[1].clone(), arguments[2].clone(),
        "--audit-review".into(), "1".into(), arguments[8].clone(),
    ]
}

fn situation_args(arguments: &[String]) -> Vec<String> {
    vec![
        "situation".into(), arguments[1].clone(), "--learned-recipe".into(),
        arguments[2].clone(), "--history-anchor".into(), arguments[8].clone(),
        "--page-size".into(), "64".into(),
    ]
}

fn observation_fleet(root: &Root, arguments: &[String], learned: bool) -> Vec<String> {
    let recipe = if learned { format!("\"{}\"", arguments[2]) } else { "null".into() };
    let text = format!(
        concat!(
            "{{\"schema\":\"fa.publication-situation-fleet/1\",\"runtime_ms\":60000,",
            "\"input_bytes\":134217728,\"journal_bytes\":33554432,\"output_bytes\":4194304,",
            "\"replay_events\":4096,\"page_size\":64,\"members\":[{{\"id\":7,",
            "\"config\":\"{}\",\"learned_recipe\":{},\"history_anchor\":\"{}\"}}]}}"
        ),
        arguments[1], recipe, arguments[8],
    );
    let path = root.0.join("observation-fleet.json");
    fs::write(&path, text).unwrap();
    vec!["situation".into(), "--fleet".into(), path.display().to_string()]
}

fn check_situation(value: &Json, case: &Case, anchor_revision: u64) {
    let state = snapshot(case);
    assert_eq!(field(value, &["schema"]).as_str(), Some("fa.publication-situation/1"));
    assert_eq!(field(value, &["historical"]).as_bool(), Some(true));
    assert_eq!(field(value, &["current_eligibility"]).as_str(), Some("not_observed"));
    assert_eq!(field(value, &["knowledge", "state"]).as_str(), Some("known"));
    let basis = field(value, &["knowledge", "basis"]);
    assert_eq!(field(basis, &["origin"]).as_str(), Some("canonical_image"));
    assert_eq!(field(basis, &["journal_revision"]).as_str().unwrap(), state.revision.to_string());
    assert_eq!(field(basis, &["anchored_through"]).as_str().unwrap(), anchor_revision.to_string());
    let summary = field(value, &["knowledge", "value", "summary"]);
    assert_eq!(field(summary, &["executions"]).as_str(), Some("0"));
    assert_eq!(field(summary, &["charged_units"]).as_str().unwrap(), state.control.ledger.charged.to_string());
    assert_eq!(field(summary, &["reserved_units"]).as_str().unwrap(), state.control.ledger.reserved.to_string());
    assert_eq!(field(summary, &["attempts"]).as_str().unwrap(), state.control.ledger.stages.len().to_string());
    assert_eq!(field(value, &["knowledge", "value", "enumeration_complete"]).as_bool(), Some(true));
}

#[test]
fn root_audit_and_situation_observe_the_actual_completed_reset_without_recovery() {
    let root = Root::new();
    let (case, arguments) = completed(&root);
    let before = files(&case);
    let state = snapshot(&case);
    let anchor = custody::load(Path::new(&arguments[8])).unwrap();
    assert_eq!(anchor.revision(), state.revision);
    let original = FileOversight::read_learned_sidecar_outcome(
        &case.config.store, &case.config.profile, &case.loaded.generation, 1,
    ).unwrap();
    let FileLearnedSidecarFinish::Applied { receipt, archive, .. } = original.result else {
        panic!("fixture must retain its original applied native review");
    };
    assert!(receipt.is_ok());

    let audit = audit_args(&arguments);
    // This root selector writes learned reports to stdout. Exercise its real
    // route, then capture the same command parser's immutable report directly.
    crate::command_with_output(audit.clone(), &mut Vec::new()).unwrap();
    let mut output = Vec::new();
    super::super::super::audit::command(&audit, &mut output).unwrap();
    let value = parse(&output);
    assert_eq!(field(&value, &["schema"]).as_str(), Some("fa.review-audit/1"));
    assert_eq!(field(&value, &["status"]).as_str(), Some("complete"));
    assert_eq!(field(&value, &["authority"]).as_str(), Some("none"));
    assert_eq!(field(&value, &["knowledge", "kind"]).as_str(), Some("known"));
    let basis = field(&value, &["knowledge", "basis"]);
    assert_eq!(field(basis, &["authentic_origin"]).as_str(), Some("unknown"));
    assert_eq!(field(basis, &["independent_history_through"]).as_str().unwrap(), anchor.revision().to_string());
    assert_eq!(field(basis, &["verified_journal_revision"]).as_str().unwrap(), state.revision.to_string());
    assert_eq!(field(basis, &["unanchored_suffix_events"]).as_str(), Some("0"));
    let review = field(&value, &["knowledge", "value"]);
    assert_eq!(field(review, &["round"]).as_str(), Some("1"));
    assert_eq!(field(review, &["input_revision"]).as_str().unwrap(), archive.input_revision.to_string());
    assert_eq!(field(review, &["commit_count"]).as_str(), Some("2"));
    assert_eq!(field(review, &["reveal_count"]).as_str(), Some("2"));
    assert_eq!(field(review, &["application", "status"]).as_str(), Some("applied"));
    assert_eq!(field(review, &["historical_effects", "executions_at_journal_cut"]).as_str(), Some("0"));

    let mut output = Vec::new();
    crate::command_with_output(situation_args(&arguments), &mut output).unwrap();
    check_situation(&parse(&output), &case, anchor.revision());
    let mut output = Vec::new();
    crate::command_with_output(observation_fleet(&root, &arguments, true), &mut output).unwrap();
    let fleet = parse(&output);
    assert_eq!(field(&fleet, &["schema"]).as_str(), Some("fa.publication-situation-fleet-page/1"));
    assert_eq!(field(&fleet, &["member"]).as_str(), Some("7"));
    assert_eq!(field(&fleet, &["member_state"]).as_str(), Some("known"));
    assert_eq!(field(&fleet, &["all_members_known"]).as_bool(), Some(true));
    assert_eq!(field(&fleet, &["coverage", "known_members"]).as_str(), Some("1"));
    assert_eq!(field(&fleet, &["coverage", "unknown_members"]).as_str(), Some("0"));
    assert_eq!(field(&fleet, &["enumeration_complete"]).as_bool(), Some(true));
    assert_eq!(field(&fleet, &["atomic_cut"]).as_bool(), Some(false));
    check_situation(field(&fleet, &["situation"]), &case, anchor.revision());
    assert_eq!(files(&case), before);
    assert_eq!(snapshot(&case), state);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn mismatched_profiles_never_become_known_and_ordinary_observation_stays_audit_only() {
    let root = Root::new();
    let (case, arguments) = completed(&root);
    let before = files(&case);
    let original = fs::read_to_string(&arguments[1]).unwrap();
    let audit = audit_args(&arguments);
    let situation = situation_args(&arguments);
    for changed in [
        original.replace("\"functional_restart\"", "\"audit_only\""),
        original.replace("\"model_generation\": 1", "\"model_generation\": 2"),
        original.replace("\"functional_restart\"", "\"bit_exact\""),
    ] {
        assert_ne!(changed, original);
        fs::write(&arguments[1], changed).unwrap();
        let mut output = Vec::new();
        assert!(crate::command_with_output(audit.clone(), &mut output).is_err());
        assert!(output.is_empty());
        assert!(super::super::super::audit::command(&audit, &mut output).is_err());
        assert!(output.is_empty());
        assert!(crate::command_with_output(situation.clone(), &mut output).is_err());
        assert!(output.is_empty());
        let error = crate::command_with_output(
            observation_fleet(&root, &arguments, true), &mut output,
        ).unwrap_err();
        assert!(error.contains("unknown members"), "{error}");
        let fleet = parse(&output);
        assert_eq!(field(&fleet, &["member_state"]).as_str(), Some("unknown"));
        assert_eq!(field(&fleet, &["coverage", "known_members"]).as_str(), Some("0"));
        assert_eq!(field(&fleet, &["coverage", "unknown_members"]).as_str(), Some("1"));
        assert_eq!(field(&fleet, &["all_members_known"]).as_bool(), Some(false));
        assert!(matches!(field(&fleet, &["situation"]), Json::Null));
        assert_eq!(files(&case), before);
    }
    fs::write(&arguments[1], original).unwrap();
    let mut output = Vec::new();
    let error = crate::command_with_output(
        vec!["situation".into(), arguments[1].clone()], &mut output,
    ).unwrap_err();
    assert!(error.contains("audit_only"), "{error}");
    assert!(output.is_empty());
    let error = crate::command_with_output(
        observation_fleet(&root, &arguments, false), &mut output,
    ).unwrap_err();
    assert!(error.contains("unknown members"), "{error}");
    let fleet = parse(&output);
    assert_eq!(field(&fleet, &["member_state"]).as_str(), Some("unknown"));
    assert_eq!(field(&fleet, &["all_members_known"]).as_bool(), Some(false));
    assert!(matches!(field(&fleet, &["situation"]), Json::Null));
    assert_eq!(files(&case), before);
}

fn terminal_fleet(root: &Root, arguments: &[String], learned: bool) -> PathBuf {
    if learned {
        fs::set_permissions(&arguments[2], fs::Permissions::from_mode(0o600)).unwrap();
    }
    let recipe = if learned { format!(",\"learned_recipe\":\"{}\"", arguments[2]) } else { String::new() };
    let text = format!(
        concat!(
            "{{\"schema_version\":\"fa.shutdown-fleet/3\",\"plan_archive\":null,\"operation\":\"901\",",
            "\"coordinator\":\"{}\",\"minimum_revision\":\"0\",\"max_visits\":64,",
            "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":134217728,",
            "\"runtime_ms\":60000,\"replay_events\":1,\"members\":[{{\"id\":\"7\",",
            "\"config\":\"{}\",\"history_anchor\":\"{}\"{}}}]}}"
        ),
        root.0.join("coordinator").display(), arguments[1], arguments[8], recipe,
    );
    let path = root.0.join("terminal-fleet.json");
    fs::write(&path, text).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

#[test]
fn a_declared_learned_fleet_can_stop_and_passively_reobserve_the_original_functional_member() {
    let root = Root::new();
    let (case, arguments) = completed(&root);
    fs::set_permissions(&arguments[1], fs::Permissions::from_mode(0o600)).unwrap();
    let manifest = terminal_fleet(&root, &arguments, true);
    let before = snapshot(&case);
    let input_anchor = fs::read(&arguments[8]).unwrap();
    let retained = root.0.join("coordinator.anchor");
    let mut output = Vec::new();
    crate::command_with_output(vec![
        "shutdown-fleet".into(), "create".into(), manifest.display().to_string(),
        "--admission-only".into(), "--retain-coordinator-anchor".into(),
        retained.display().to_string(),
    ], &mut output).unwrap();
    let report = parse(&output);
    assert_eq!(field(&report, &["schema_version"]).as_str(), Some("fa.shutdown-fleet-result/3"));
    assert_eq!(field(&report, &["all_observed_stopped"]).as_bool(), Some(true));
    let members = field(&report, &["members"]).as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(field(&members[0], &["domain"]).as_str(), Some("7"));
    assert_eq!(field(&members[0], &["result"]).as_str(), Some("observed"));
    assert_eq!(field(&members[0], &["stop_operation"]).as_str(), Some("901"));
    assert!(matches!(field(&members[0], &["acquired_tick"]), Json::Null));
    let stopped = snapshot(&case);
    assert_eq!(stopped.stop.as_ref().unwrap().request().operation, 901);
    assert_eq!(stopped.revision, before.revision + 2);
    assert_eq!(stopped.executions, before.executions);
    assert_eq!(stopped.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(stopped.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(fs::read(&arguments[8]).unwrap(), input_anchor);
    let reset = FileOversight::read_learned_reset_result(
        &case.config.store, &case.config.profile, &case.loaded.generation, 900,
    ).unwrap();
    assert!(matches!(reset.record(), FileLearnedResetRecord::Completed { result: Ok(receipt), .. }
        if receipt.control.restored && receipt.position == 1 && receipt.sampled_draws == 0));

    fs::write(case.config.store.join("delivery.pending"), b"passive fleet must preserve this file").unwrap();
    let member_files = files(&case);
    let coordinator = root.0.join("coordinator/delivery.bin");
    let coordinator_bytes = fs::read(&coordinator).unwrap();
    let next = root.0.join("next-coordinator.anchor");
    let mut output = Vec::new();
    crate::command_with_output(vec![
        "shutdown-fleet".into(), "resume".into(), manifest.display().to_string(),
        "--admission-only".into(), "--coordinator-anchor".into(), retained.display().to_string(),
        "--retain-coordinator-anchor".into(), next.display().to_string(),
    ], &mut output).unwrap();
    assert_eq!(field(&parse(&output), &["all_observed_stopped"]).as_bool(), Some(true));
    assert_eq!(files(&case), member_files);
    assert_eq!(fs::read(coordinator).unwrap(), coordinator_bytes);
    assert_eq!(fs::read(retained).unwrap(), fs::read(next).unwrap());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn terminal_fleet_requires_the_original_declared_grade_and_an_explicit_learned_recipe() {
    let root = Root::new();
    let (case, arguments) = completed(&root);
    fs::set_permissions(&arguments[1], fs::Permissions::from_mode(0o600)).unwrap();
    let before = files(&case);
    let original = fs::read_to_string(&arguments[1]).unwrap();
    let manifest = terminal_fleet(&root, &arguments, true);
    for changed in [
        original.replace("\"functional_restart\"", "\"audit_only\""),
        original.replace("\"model_generation\": 1", "\"model_generation\": 2"),
    ] {
        assert_ne!(changed, original);
        fs::write(&arguments[1], changed).unwrap();
        let mut output = Vec::new();
        assert!(crate::command_with_output(vec![
            "shutdown-fleet".into(), "create".into(), manifest.display().to_string(),
            "--admission-only".into(),
        ], &mut output).is_err());
        assert!(output.is_empty());
        assert!(!root.0.join("coordinator").exists());
        assert_eq!(files(&case), before);
    }
    fs::write(&arguments[1], original).unwrap();
    let manifest = terminal_fleet(&root, &arguments, false);
    let mut output = Vec::new();
    let error = crate::command_with_output(vec![
        "shutdown-fleet".into(), "create".into(), manifest.display().to_string(),
        "--admission-only".into(),
    ], &mut output).unwrap_err();
    assert!(error.contains("audit_only"), "{error}");
    assert!(output.is_empty());
    assert!(!root.0.join("coordinator").exists());
    assert_eq!(files(&case), before);
}
