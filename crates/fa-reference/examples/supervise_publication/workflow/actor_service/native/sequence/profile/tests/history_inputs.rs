//! Archive custody must remain independent of every selected command and
//! manifest input, including future paths which cannot yet be opened.
use super::*;
use super::super::super::anchors as custody;

fn selected_custody(input: Option<&Path>, output: Option<&Path>) -> custody::Options {
    let mut args = [
        "serve-native-generated", "config", "sequence", "reviewer",
    ].map(str::to_owned).to_vec();
    for (flag, path) in [
        ("--history-anchor", input),
        ("--retain-history-anchor", output),
    ] {
        if let Some(path) = path {
            args.extend([flag.to_owned(), path.to_string_lossy().into_owned()]);
        }
    }
    custody::take_options(&args).unwrap().1
}

#[test]
fn resident_custody_rejects_command_input_aliases_before_opening_any_configuration() {
    let root = Root::new();
    let config = root.0.join("config.json");
    let sequence = root.0.join("sequence.json");
    let reviewer = root.0.join("reviewer.json");
    let publication = root.0.join("publication.json");
    let operator = root.0.join("operator.json");
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&root.0, &alias).unwrap();
    let mut base = vec![
        "serve-native-generated".to_owned(),
        config.to_string_lossy().into_owned(),
        sequence.to_string_lossy().into_owned(),
        reviewer.to_string_lossy().into_owned(),
    ];
    base.extend([
        "--publication-profile".to_owned(), publication.to_string_lossy().into_owned(),
        "--operator-profile".to_owned(), operator.to_string_lossy().into_owned(),
        "--tower-generation".to_owned(), "7".to_owned(),
        "--after-message".to_owned(), "90".to_owned(),
    ]);
    for selected in [&config, &sequence, &reviewer, &publication, &operator] {
        let canonical_alias = alias.join(selected.file_name().unwrap());
        for flag in ["--history-anchor", "--retain-history-anchor"] {
            let mut args = base.clone();
            args.extend([flag.to_owned(), canonical_alias.to_string_lossy().into_owned()]);
            let mut output = Vec::new();
            let error = command_with_clock(
                &args, None, || panic!("input alias acquired time"), &mut output,
            ).unwrap_err();
            assert!(error.contains("custody aliases a selected input file"), "{error}");
            assert!(output.is_empty());
            assert!(!selected.exists());
        }
    }
    // A distinct selected output passes custody admission and reaches the
    // missing original configuration, proving the alias check caused refusal.
    let independent = root.0.join("independent.anchor");
    let mut args = base;
    args.extend([
        "--retain-history-anchor".to_owned(),
        independent.to_string_lossy().into_owned(),
    ]);
    let expected = Config::read(&config).err().unwrap();
    assert_eq!(
        command_with_clock(
            &args, None, || panic!("missing configuration acquired time"), &mut Vec::new(),
        ),
        Err(expected),
    );
    assert!(!independent.exists());
    assert!(!root.0.join("store").exists());
}

#[test]
fn resident_custody_reserves_manifest_files_before_actor_precheck_and_model_reads() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let source = std::fs::read(config.source.path()).unwrap();
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let plan = root.0.join("sequence.json");
    std::fs::write(
        &plan,
        r#"{"schema":"fa.native-publication-sequence/2","entries":[
        {"recipe":"missing-recipe.json","actor_profile":"actor-0.json","release":"message","precheck_profile":null},
        {"recipe":"missing-recipe.json","actor_profile":"actor-1.json","release":"message","precheck_profile":"future-precheck.json"},
        {"recipe":"missing-recipe.json","actor_profile":"actor-2.json","release":"finish","precheck_profile":null}]}"#,
    ).unwrap();
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&root.0, &alias).unwrap();
    let before = std::fs::read(&plan).unwrap();
    for reserved in [
        plan.clone(),
        root.0.join("missing-recipe.json"),
        root.0.join("actor-2.json"),
        root.0.join("future-precheck.json"),
        alias.join("missing-recipe.json"),
    ] {
        for as_input in [false, true] {
            let history = if as_input {
                selected_custody(Some(&reserved), None)
            } else {
                selected_custody(None, Some(&reserved))
            };
            let error = load_with_history(
                &plan, &config, &peers, Some(90), None, None, Some(history),
            ).err().unwrap();
            assert!(error.contains("custody aliases a selected input file"), "{error}");
            assert_eq!(std::fs::read(&plan).unwrap(), before);
            assert_eq!(std::fs::read(config.source.path()).unwrap(), source);
            assert!(!root.0.join("missing-recipe.json").exists());
            assert!(!root.0.join("future-precheck.json").exists());
        }
    }

    // Independent custody keeps the original model/recipe loader usable. This
    // admission alone never creates an output archive, owner or live listener.
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    ).unwrap();
    std::fs::write(
        &plan,
        document(&[
            entry("recipe.json", "actor-0.json", "message"),
            entry("second.json", "actor-1.json", "message"),
            entry("second.json", "actor-2.json", "finish"),
        ]),
    ).unwrap();
    let output = root.0.join("independent.anchor");
    let (loaded, history) = load_with_history(
        &plan, &config, &peers, None, None, None,
        Some(selected_custody(None, Some(&output))),
    ).unwrap();
    assert!(history.is_some());
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert!(!output.exists());
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
    assert_eq!(std::fs::read(config.source.path()).unwrap(), source);
}
