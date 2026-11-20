//! Governed creation owns its complete grammar before selected inputs, clocks,
//! listeners or authority. Loader tests exercise original finite model fixtures;
//! declared peer policies alone do not demonstrate authenticated live roles.
use super::super::super::{anchors as history_custody, governor as governance};
use super::super::{USAGE, command_with_clock, load_with_services, options, take_operator};
use super::*;
use crate::workflow::actor_service::operator as live_operator;
use std::{fs, os::unix::fs::DirBuilderExt};

const GOVERNOR_INVALID: &str = "native governance requires new serve-native-generated creation with paired --operator-profile/--tower-generation and independent --governor-profile; recovery and credibility activation need their own governed recovery contract";

fn base(root: &Root) -> Vec<String> {
    vec![
        "serve-native-generated".into(),
        root.0.join("missing-config.json").display().to_string(),
        root.0.join("missing-sequence.json").display().to_string(),
        root.0.join("missing-reviewer.json").display().to_string(),
    ]
}

fn governed(root: &Root) -> Vec<String> {
    let mut args = base(root);
    args.extend([
        "--governor-profile".into(),
        root.0.join("governor.json").display().to_string(),
        "--operator-profile".into(),
        root.0.join("operator.json").display().to_string(),
        "--tower-generation".into(),
        "41".into(),
    ]);
    args
}

fn command_failure(args: &[String], credibility: Option<&Path>) -> String {
    let mut output = Vec::new();
    let result = command_with_clock(
        args,
        credibility,
        || panic!("argument/input refusal acquired a clock: {args:?}"),
        &mut output,
    );
    assert!(
        output.is_empty(),
        "argument/input refusal wrote output: {args:?}"
    );
    result
        .err()
        .expect("selected grammar or missing original input must refuse")
}

#[test]
fn native_governor_creation_admits_complete_option_orderings_before_original_input_intake() {
    let root = Root::new();
    let base = base(&root);
    let missing_config = Config::read(Path::new(&base[1])).err().unwrap();
    let governor_path = root.0.join("governor.json");
    let operator_path = root.0.join("operator.json");
    let publication_path = root.0.join("publication.json");
    let retained_path = root.0.join("retained.anchor");
    for generation in [1, u64::MAX] {
        for (publication, retained) in [(false, false), (true, false), (false, true), (true, true)]
        {
            let mut pairs = vec![
                ("--governor-profile", governor_path.display().to_string()),
                ("--operator-profile", operator_path.display().to_string()),
                ("--tower-generation", generation.to_string()),
            ];
            if publication {
                pairs.push((
                    "--publication-profile",
                    publication_path.display().to_string(),
                ));
            }
            if retained {
                pairs.push((
                    "--retain-history-anchor",
                    retained_path.display().to_string(),
                ));
            }
            for shift in 0..pairs.len() {
                for reversed in [false, true] {
                    let mut ordered = pairs.clone();
                    ordered.rotate_left(shift);
                    if reversed {
                        ordered.reverse();
                    }
                    let mut args = base.clone();
                    let mut expected = base.clone();
                    for (flag, value) in ordered {
                        args.extend([flag.to_owned(), value.clone()]);
                        if flag != "--governor-profile" {
                            expected.extend([flag.to_owned(), value]);
                        }
                    }
                    let (remaining, selected) = governance::take_option(&args, None).unwrap();
                    assert_eq!(remaining, expected);
                    assert_eq!(selected, Some(governor_path.clone()));
                    let (remaining, history) = history_custody::take_options(&remaining).unwrap();
                    assert_eq!(history.is_selected(), retained);
                    history.check_mode(None).unwrap();
                    let (remaining, operator) = take_operator(&remaining, None).unwrap();
                    assert_eq!(operator, Some((operator_path.clone(), generation)));
                    assert_eq!(
                        options(&remaining, None).unwrap(),
                        (publication.then(|| publication_path.clone()), None)
                    );
                    // Accepted selection reaches the original configuration
                    // reader, rather than merely returning a different grammar error.
                    assert_eq!(command_failure(&args, None), missing_config);
                    assert!(!retained_path.exists());
                    assert!(!governor_path.exists());
                }
            }
        }
    }
}

#[test]
fn native_governor_option_is_a_noop_for_unselected_original_creation_and_recovery_grammars() {
    let root = Root::new();
    let base = base(&root);
    let missing_config = Config::read(Path::new(&base[1])).err().unwrap();
    for suffix in [
        vec![],
        vec!["--publication-profile", "/missing/publication"],
        vec![
            "--operator-profile",
            "/missing/operator",
            "--tower-generation",
            "41",
        ],
        vec![
            "--after-message",
            "90",
            "--retain-history-anchor",
            "/missing/retained.anchor",
        ],
    ] {
        let mut args = base.clone();
        args.extend(suffix.into_iter().map(str::to_owned));
        let capsule = args
            .iter()
            .any(|arg| arg == "--after-message")
            .then(|| Path::new("/missing/capsule"));
        assert_eq!(
            governance::take_option(&args, capsule).unwrap(),
            (args.clone(), None)
        );
        assert_eq!(command_failure(&args, capsule), missing_config);
    }
    let mut unknown = base.clone();
    unknown.extend(["--unknown", "value"].map(str::to_owned));
    assert_eq!(
        governance::take_option(&unknown, None).unwrap(),
        (unknown.clone(), None)
    );
    assert_eq!(command_failure(&unknown, None), USAGE);
    assert_eq!(
        governance::take_option(&[], None).unwrap(),
        (Vec::new(), None)
    );
    assert_eq!(command_failure(&[], None), USAGE);
}

#[test]
fn native_governor_creation_rejects_unpaired_conflicting_and_ambiguous_options_before_io() {
    let root = Root::new();
    let base = base(&root);
    let valid = governed(&root);
    let mut standalone = base.clone();
    standalone.extend(["--governor-profile", "/missing/governor"].map(str::to_owned));
    let mut missing_value = base.clone();
    missing_value.push("--governor-profile".into());
    let mut no_operator = base.clone();
    no_operator.extend(
        [
            "--governor-profile",
            "/missing/governor",
            "--tower-generation",
            "41",
        ]
        .map(str::to_owned),
    );
    let mut rejected = vec![
        standalone,
        missing_value,
        no_operator,
        valid[..8].to_vec(),
        valid[..9].to_vec(),
    ];
    for value_index in [1, 2, 3, 5, 7, 9] {
        for value in ["", "--not-a-value"] {
            let mut args = valid.clone();
            args[value_index] = value.into();
            rejected.push(args);
        }
    }
    for (flag, value) in [
        ("--governor-profile", "/missing/second-governor"),
        ("--operator-profile", "/missing/second-operator"),
        ("--tower-generation", "42"),
        ("--publication-profile", "/missing/publication"),
        ("--retain-history-anchor", "/missing/retained.anchor"),
    ] {
        let mut duplicate = valid.clone();
        if matches!(flag, "--publication-profile" | "--retain-history-anchor") {
            duplicate.extend([flag.to_owned(), value.to_owned()]);
        }
        duplicate.extend([flag.to_owned(), value.to_owned()]);
        rejected.push(duplicate);
    }
    for flag in ["--publication-profile", "--retain-history-anchor"] {
        for value in ["", "--not-a-value"] {
            let mut args = valid.clone();
            args.extend([flag.to_owned(), value.to_owned()]);
            rejected.push(args);
        }
    }
    for flag in [
        "--after-message",
        "--history-anchor",
        "--credibility-activation",
        "--topology-profile",
        "--continue-generation",
        "--finish-after-message",
        "--native-text",
        "--precheck-profile",
        "--precheck-schedule",
        "--requests",
        "--guarded",
        "--unknown",
    ] {
        let mut args = valid.clone();
        args.extend([flag.to_owned(), "1".into()]);
        rejected.push(args);
    }
    let mut recovery = valid.clone();
    recovery.extend(
        [
            "--after-message",
            "90",
            "--history-anchor",
            "/missing/history",
        ]
        .map(str::to_owned),
    );
    rejected.push(recovery);
    let mut odd = valid.clone();
    odd.push("--publication-profile".into());
    rejected.push(odd);
    let mut wrong_command = valid.clone();
    wrong_command[0] = "serve-open".into();
    rejected.push(wrong_command);
    for args in rejected {
        assert_eq!(
            governance::take_option(&args, None),
            Err(GOVERNOR_INVALID.to_owned()),
            "{args:?}"
        );
        assert_eq!(command_failure(&args, None), GOVERNOR_INVALID, "{args:?}");
    }
    let capsule = Path::new("/missing/independent-capsule");
    assert_eq!(
        governance::take_option(&valid, Some(capsule)),
        Err(GOVERNOR_INVALID.to_owned())
    );
    assert_eq!(command_failure(&valid, Some(capsule)), GOVERNOR_INVALID);
    assert!(!root.0.join("governor.json").exists());
    assert!(!root.0.join("store").exists());
}

#[test]
fn native_governor_creation_retains_original_canonical_tower_generation_validation() {
    let root = Root::new();
    for generation in [
        "0",
        "00",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e3",
        " 1",
        "1 ",
        "18446744073709551616",
        "100000000000000000000",
    ] {
        let mut args = governed(&root);
        args[9] = generation.into();
        let (remaining, selected) = governance::take_option(&args, None).unwrap();
        assert!(selected.is_some());
        assert_eq!(take_operator(&remaining, None), Err(USAGE.to_owned()));
        assert_eq!(
            command_failure(&args, None),
            USAGE,
            "generation={generation}"
        );
    }
}

fn selections(root: &Root, actor: &Profile) -> (governance::Settings, operator::Selection) {
    let scope = actor.scope;
    let supervisor = actor.supervisor;
    let pid = supervisor.pid().unwrap();
    let governor_pid = if pid == u32::MAX { pid - 1 } else { pid + 1 };
    let scope = format!(
        r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority
    );
    let identity = format!(
        r#"{{"uid":{},"gid":{},"pid":{pid}}}"#,
        supervisor.uid(),
        supervisor.gid()
    );
    let governor = governance::Settings::decode(format!(r#"{{"schema":"fa.policy-governor-service/1","scope":{scope},"socket":"{}/governor.sock","supervisor":{identity},"governor":{{"uid":{},"gid":{},"pid":{governor_pid}}},"candidate_limit":4,"connection_limit":4,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"campaign_limit":1,"report_bytes":65536,"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"numerical_replay":{{"max_replay_events":4096,"events":8192}}}}"#,
        root.0.display(), supervisor.uid(), supervisor.gid()).as_bytes()).unwrap();
    let operator = live_operator::Settings::decode(format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{scope},"socket":"{}/operator.sock","supervisor":{identity},"operator":{identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"inspection_queries":0,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none","policy_promotion":{{"queries":2,"max_replay_events":4096,"numerical_replay_events":4096,"authority":"promote_approved_policy"}}}}"#,
        root.0.display()).as_bytes()).unwrap();
    (
        governor,
        operator::Selection::new_governed(operator, 41).unwrap(),
    )
}

#[test]
fn native_governor_loader_admits_original_assets_and_refuses_unsupported_modes_and_future_custody_aliases()
 {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let (governor, chosen) = selections(&root, &entries[0].actor);
    let plain = super::operator_tests::operator_selection(&root, &entries[0].actor);
    let missing = root.0.join("missing-sequence.json");
    for (selected_governor, selected_operator, after, credibility) in [
        (Some(&governor), None, None, None),
        (Some(&governor), Some(&plain), None, None),
        (None, Some(&chosen), None, None),
        (Some(&governor), Some(&chosen), Some(90), None),
        (
            Some(&governor),
            Some(&chosen),
            None,
            Some(Path::new("/missing/capsule")),
        ),
    ] {
        let error = load_with_services(
            &missing,
            &config,
            &peers,
            after,
            selected_operator,
            credibility,
            None,
            selected_governor,
        )
        .err()
        .unwrap();
        assert!(error.contains("native governance"), "{error}");
        assert!(!missing.exists());
        assert!(!config.store.exists());
    }
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let text = fs::read_to_string(root.0.join("recipe.json"))
        .unwrap()
        .replace("\"prompt.txt\"", "\"real/inputs/prompt.txt\"");
    fs::write(root.0.join("recipe.json"), &text).unwrap();
    fs::write(
        root.0.join("second.json"),
        text.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(root.0.join("real"))
        .unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(root.0.join("real/inputs"))
        .unwrap();
    std::os::unix::fs::symlink(root.0.join("real"), root.0.join("alias")).unwrap();
    let prompt = fs::read(root.0.join("prompt.txt")).unwrap();
    let model = fs::read(root.0.join("model.json")).unwrap();
    fs::remove_file(root.0.join("model.json")).unwrap();
    let plan = root.0.join("sequence.json");
    fs::write(
        &plan,
        document(&[
            entry("recipe.json", "actor-0.json", "message"),
            entry("second.json", "actor-1.json", "message"),
            entry("second.json", "actor-2.json", "finish"),
        ]),
    )
    .unwrap();
    for socket in [
        root.0.join("real/inputs/prompt.txt"),
        root.0.join("alias/inputs/prompt.txt"),
    ] {
        let mut conflicting = governor.clone();
        conflicting.socket = socket;
        let error = load_with_services(
            &plan,
            &config,
            &peers,
            None,
            Some(&chosen),
            None,
            None,
            Some(&conflicting),
        )
        .err()
        .unwrap();
        assert!(
            error.contains(
                "governor socket conflicts with original source or retained checkpoint custody"
            ),
            "future asset custody lost to missing model: {error}"
        );
    }
    fs::write(root.0.join("model.json"), model).unwrap();
    fs::write(root.0.join("real/inputs/prompt.txt"), prompt).unwrap();
    let retained = root.0.join("retained.anchor");
    let mut args = base(&root);
    args.extend([
        "--retain-history-anchor".into(),
        retained.display().to_string(),
    ]);
    let (_, history) = history_custody::take_options(&args).unwrap();
    let (loaded, history) = load_with_services(
        &plan,
        &config,
        &peers,
        None,
        Some(&chosen),
        None,
        Some(history),
        Some(&governor),
    )
    .unwrap();
    assert_eq!(loaded.len(), 3);
    assert!(history.is_some());
    assert_eq!(loaded[0].inputs.decoder, entries[0].inputs.decoder);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert_eq!(loaded[1].inputs.request, loaded[2].inputs.request);
    assert!(!retained.exists());
    assert!(!config.store.exists());
    assert!(!governor.socket.exists());
    assert!(!chosen.settings.socket.exists());
    assert!(loaded.iter().all(|entry| !entry.actor.socket.exists()));
}
