//! Explicit capsule selection is recovery-only and cannot occupy a source,
//! store or future transport name before numerical input loading.
use super::*;

#[test]
fn resident_native_creation_refuses_requalification_before_configuration_or_clock_reads() {
    for operator in [false, true] {
        let mut args = [
            "serve-native-generated", "/missing/config", "/missing/sequence", "/missing/reviewer",
        ].map(str::to_owned).to_vec();
        if operator {
            args.extend([
                "--operator-profile", "/missing/operator", "--tower-generation", "1",
            ].map(str::to_owned));
        }
        assert_eq!(
            command_with_clock(
                &args,
                Some(Path::new("/missing/capsule")),
                || panic!("creation acquired a clock before refusing activation"),
                &mut Vec::new(),
            ),
            Err(USAGE.to_owned()),
        );
    }
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let store = config.store.clone();
    let actors = entries.iter().map(|entry| entry.actor.socket.clone()).collect::<Vec<_>>();
    let result = crate::workflow::actor_service::native::sequence::run_with_qualification(
        config, entries, &peers, None, (None, None), Some(&root.0.join("capsule")),
        || panic!("direct creation acquired time"), &mut Vec::new(),
    );
    assert!(result.unwrap_err().contains("explicit --after-message recovery"));
    assert!(!store.exists());
    assert!(actors.iter().all(|path| !path.exists()));
}

#[test]
fn resident_qualification_reserves_every_future_endpoint_before_loading_a_recipe() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let plan = root.0.join("sequence.json");
    std::fs::write(
        &plan,
        document(&[
            entry("missing-recipe.json", "actor-0.json", "message"),
            entry("missing-recipe.json", "actor-1.json", "message"),
            entry("missing-recipe.json", "actor-2.json", "finish"),
        ]),
    ).unwrap();
    let operator = operator_tests::operator_selection(&root, &entries[0].actor);
    for capsule in [
        config.source.path().to_owned(),
        config.store.join("delivery.bin"),
        entries[2].actor.socket.clone(),
        peers.socket(entries[2].actor.request),
        crate::workflow::control::socket_path(&peers, entries[2].actor.request),
        operator.settings.socket.clone(),
    ] {
        let error = load_with_qualification(
            &plan, &config, &peers, Some(90), Some(&operator), Some(&capsule),
        ).err().unwrap();
        assert!(error.contains("native credibility input"), "{error}");
    }

    let real = root.0.join("real");
    std::fs::create_dir(&real).unwrap();
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let mut later = entries[2].actor.clone();
    later.socket = real.join("later.sock");
    save_actor(&root.0.join("actor-2.json"), &later);
    let error = load_with_qualification(
        &plan, &config, &peers, Some(90), None, Some(&alias.join("later.sock")),
    ).err().unwrap();
    assert!(error.contains("scheduled transport endpoint"), "{error}");
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}

#[test]
fn resident_qualification_admission_keeps_the_capsule_for_the_original_controlled_reader() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    ).unwrap();
    let plan = root.0.join("sequence.json");
    std::fs::write(
        &plan,
        document(&[
            entry("recipe.json", "actor-0.json", "message"),
            entry("second.json", "actor-1.json", "message"),
            entry("second.json", "actor-2.json", "finish"),
        ]),
    ).unwrap();
    let missing = root.0.join("independent-qualification.bin");
    let loaded = load_with_qualification(
        &plan, &config, &peers, Some(90), None, Some(&missing),
    ).unwrap();
    assert_eq!(loaded.len(), 3);
    assert!(!missing.exists());
    assert!(!config.store.exists());

    let selected = exact_precheck::Selection::new(
        crate::workflow::actor_service::precheck::Settings::decode(
            format!(
                r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":2,"disclosure_id":81,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":1000}}"#,
                missing.display(),
            ).as_bytes(),
        ).unwrap(),
    ).unwrap();
    let error = super::super::super::requalification::check_paths(
        &config, &[(&entries[0].actor, Some(&selected))], &peers, None, Some(&missing),
    ).unwrap_err();
    assert!(error.contains("scheduled precheck endpoint"), "{error}");
}
