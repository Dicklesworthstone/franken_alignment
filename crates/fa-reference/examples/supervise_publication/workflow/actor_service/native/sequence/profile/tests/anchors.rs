//! History selection freezes private original archives before numerical recipes
//! and reserves every future endpoint in the same invocation's namespace.
use super::*;
use super::super::super::anchors as custody;
use std::{fs, os::unix::fs::PermissionsExt};

fn custody_options(input: Option<&Path>, output: Option<&Path>) -> custody::Options {
    let mut args = [
        "serve-native-generated", "config", "sequence", "reviewer",
    ].map(str::to_owned).to_vec();
    for (flag, path) in [
        ("--history-anchor", input),
        ("--retain-history-anchor", output),
    ] {
        if let Some(path) = path {
            args.extend([flag.to_owned(), path.to_str().unwrap().to_owned()]);
        }
    }
    custody::take_options(&args).unwrap().1
}

#[test]
fn resident_history_grammar_composes_explicit_recovery_operator_and_qualification_before_io() {
    let base = [
        "serve-native-generated", "/missing/config", "/missing/sequence", "/missing/reviewer",
    ].map(str::to_owned).to_vec();
    let pairs = [
        ["--history-anchor", "/private/old.anchor"],
        ["--publication-profile", "/operator/witness.json"],
        ["--operator-profile", "/operator/tower.json"],
        ["--retain-history-anchor", "/private/new.anchor"],
        ["--after-message", "92"],
        ["--tower-generation", "41"],
    ];
    for reversed in [false, true] {
        let mut pairs = pairs;
        if reversed { pairs.reverse(); }
        let mut args = base.clone();
        args.extend(pairs.into_iter().flatten().map(str::to_owned));
        let (remaining, history) = custody::take_options(&args).unwrap();
        let capsule = Some(Path::new("/private/qualification.json"));
        let (remaining, operator) = take_operator(&remaining, capsule).unwrap();
        let (publication, after) = options(&remaining, capsule).unwrap();
        assert_eq!(operator, Some((PathBuf::from("/operator/tower.json"), 41)));
        assert_eq!(publication, Some(PathBuf::from("/operator/witness.json")));
        assert_eq!(after, Some(92));
        history.check_mode(after).unwrap();
        assert!(history.is_selected());
    }
    let mut retained_creation = base.clone();
    retained_creation.extend(["--retain-history-anchor".into(), "/private/new.anchor".into()]);
    let (remaining, history) = custody::take_options(&retained_creation).unwrap();
    assert_eq!(remaining, base);
    history.check_mode(None).unwrap();

    let mut rejected = Vec::new();
    for extra in [
        vec!["--history-anchor", "/private/old.anchor"],
        vec!["--history-anchor"],
        vec!["--history-anchor", "--after-message"],
        vec!["--retain-history-anchor", ""],
        vec!["--retain-history-anchor", "./archive"],
        vec!["--retain-history-anchor", "../archive"],
        vec!["--retain-history-anchor", "/private/a", "--retain-history-anchor", "/private/b"],
        vec!["--history-anchor", "/private/a", "--history-anchor", "/private/b", "--after-message", "92"],
        vec!["--history-anchor", "/private/a", "--after-message", "0"],
        vec!["--history-anchor", "/private/a", "--after-message", "092"],
        vec!["--retain-history-anchor", "/private/a", "--requests", "91,92"],
        vec!["--retain-history-anchor", "/private/a", "--tower-generation", "41"],
        vec!["--retain-history-anchor", "/private/a", "--finish-after-message", "92"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        rejected.push(args);
    }
    for args in rejected {
        let error = command_with_clock(
            &args, None, || panic!("invalid history grammar acquired time"), &mut Vec::new(),
        ).unwrap_err();
        assert!(error == USAGE || error.contains("history"), "{error}");
    }
}

fn invalid_recipe_plan(root: &Root, entries: &[Entry]) -> PathBuf {
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let plan = root.0.join("sequence.json");
    fs::write(
        &plan,
        document(&[
            entry("invalid-recipe.json", "actor-0.json", "message"),
            entry("invalid-recipe.json", "actor-1.json", "message"),
            entry("invalid-recipe.json", "actor-2.json", "finish"),
        ]),
    ).unwrap();
    fs::write(
        root.0.join("invalid-recipe.json"),
        r#"{"schema":"history-must-be-admitted-before-this-recipe"}"#,
    ).unwrap();
    plan
}

#[test]
fn native_history_reserves_source_store_all_future_endpoints_and_capsule_before_recipes() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let plan = invalid_recipe_plan(&root, &entries);
    let selected_operator = operator_tests::operator_selection(&root, &entries[0].actor);
    let capsule = root.0.join("qualification.json");
    let check_socket = root.0.join("last-check.sock");
    let check_path = root.0.join("last-check.json");
    fs::write(&check_path, format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":1,"disclosure_id":81,"query_limit":1,"exchange_limit":16,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
        check_socket.display(),
    )).unwrap();
    fs::write(&plan,
        r#"{"schema":"fa.native-publication-sequence/2","entries":[
        {"recipe":"invalid-recipe.json","actor_profile":"actor-0.json","release":"message","precheck_profile":null},
        {"recipe":"invalid-recipe.json","actor_profile":"actor-1.json","release":"message","precheck_profile":"last-check.json"},
        {"recipe":"invalid-recipe.json","actor_profile":"actor-2.json","release":"finish","precheck_profile":null}]}"#,
    ).unwrap();
    fs::create_dir(root.0.join("separate")).unwrap();
    std::os::unix::fs::symlink(&root.0, root.0.join("alias")).unwrap();
    let source = fs::read(config.source.path()).unwrap();
    for forbidden in [
        config.source.path().to_owned(),
        config.store.clone(),
        entries[2].actor.socket.clone(),
        peers.socket(entries[2].actor.request),
        crate::workflow::control::socket_path(&peers, entries[2].actor.request),
        selected_operator.settings.socket.clone(),
        check_socket,
        capsule.clone(),
        root.0.join("alias/actor-2.sock"),
    ] {
        for as_input in [false, true] {
            let history = if as_input {
                custody_options(Some(&forbidden), None)
            } else {
                custody_options(None, Some(&forbidden))
            };
            let error = load_with_history(
                &plan, &config, &peers, Some(90), Some(&selected_operator),
                Some(&capsule), Some(history),
            ).err().unwrap();
            assert!(error.contains("native history custody must be distinct"), "{error}");
            assert_eq!(fs::read(config.source.path()).unwrap(), source);
            assert!(!config.store.exists());
            assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
        }
    }
    // A separate, new custody destination reaches the original sentinel recipe
    // refusal, proving that aliases did not merely fail another prerequisite.
    let output = root.0.join("separate/new.anchor");
    let error = load_with_history(
        &plan, &config, &peers, Some(90), Some(&selected_operator), Some(&capsule),
        Some(custody_options(None, Some(&output))),
    ).err().unwrap();
    assert_eq!(error, "unsupported native service recipe");
    assert!(!output.exists());
    assert!(!config.store.exists());
}

#[test]
fn native_history_missing_malformed_public_and_symlink_inputs_never_fall_back_to_the_store() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let plan = invalid_recipe_plan(&root, &entries);
    let malformed = root.0.join("malformed.anchor");
    fs::write(&malformed, b"not an original history archive").unwrap();
    fs::set_permissions(&malformed, fs::Permissions::from_mode(0o600)).unwrap();
    let public = root.0.join("public.anchor");
    fs::write(&public, b"not private").unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o644)).unwrap();
    let symlink = root.0.join("alias.anchor");
    std::os::unix::fs::symlink(&malformed, &symlink).unwrap();
    let output = root.0.join("new.anchor");
    let source = fs::read(config.source.path()).unwrap();
    for input in [root.0.join("missing.anchor"), malformed.clone(), public, symlink] {
        let error = load_with_history(
            &plan, &config, &peers, Some(90), None, None,
            Some(custody_options(Some(&input), Some(&output))),
        ).err().unwrap();
        assert_ne!(error, "unsupported native service recipe");
        assert_eq!(fs::read(config.source.path()).unwrap(), source);
        assert!(!config.store.exists());
        assert!(!output.exists());
        assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
    }
    assert_eq!(fs::read(&malformed).unwrap(), b"not an original history archive");

    // Existing custody is never replaced, even for a creation command with no
    // imported anchor and before the first invalid numerical recipe.
    let error = load_with_history(
        &plan, &config, &peers, None, None, None,
        Some(custody_options(None, Some(&malformed))),
    ).err().unwrap();
    assert!(error.contains("must be a new file"), "{error}");
    assert_eq!(fs::read(&malformed).unwrap(), b"not an original history archive");
}
