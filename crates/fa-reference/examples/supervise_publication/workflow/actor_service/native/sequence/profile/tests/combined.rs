//! Combined operator/disclosure admission must protect both original endpoints.
use super::*;
use crate::workflow::actor_service::precheck::Settings as PrecheckSettings;
use std::fs;

#[test]
fn resident_native_operator_and_disclosure_admission_refuses_aliases_before_assets_or_clock() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let mut selected_operator = operator_tests::operator_selection(&root, &entries[0].actor);
    let real = root.0.join("real");
    let alias = root.0.join("alias");
    fs::create_dir(&real).unwrap();
    fs::create_dir(real.join("peers")).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    selected_operator.settings.socket = real.join("peers/operator.sock");
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    let settings = |socket: &Path| {
        format!(
            r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":82,"query_limit":1,"exchange_limit":16,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
            socket.display()
        )
    };
    let manifest = |first: &str, later: &str| {
        format!(
            r#"{{"schema":"fa.native-publication-sequence/2","entries":[{{"recipe":"{first}","actor_profile":"actor-0.json","release":"message","precheck_profile":null}},{{"recipe":"{later}","actor_profile":"actor-1.json","release":"message","precheck_profile":"precheck.json"}},{{"recipe":"{later}","actor_profile":"actor-2.json","release":"finish","precheck_profile":null}}]}}"#
        )
    };
    let precheck = root.0.join("precheck.json");
    let plan = root.0.join("sequence.json");
    let permitted_socket = alias.join("peers/precheck.sock");
    fs::write(&precheck, settings(&permitted_socket)).unwrap();
    fs::write(&plan, manifest("recipe.json", "second.json")).unwrap();

    // The same alias parent is permitted when the final endpoint is distinct.
    // Load real original assets, preserving one selected late disclosure.
    let mut loaded = load_with_operator(
        &plan,
        &config,
        &peers,
        None,
        Some(&selected_operator),
    )
    .unwrap();
    assert_eq!(loaded.len(), 3);
    assert!(loaded[0].precheck.is_none());
    assert!(loaded[1].precheck.is_some());
    assert!(loaded[2].precheck.is_none());
    assert_eq!(loaded[0].inputs.decoder, entries[0].inputs.decoder);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert_eq!(loaded[1].inputs.request, loaded[2].inputs.request);
    let source_path = config.source.path().to_owned();
    let source = fs::read(&source_path).unwrap();
    let store = config.store.clone();
    let actor_sockets = loaded
        .iter()
        .map(|entry| entry.actor.socket.clone())
        .collect::<Vec<_>>();
    assert!(!store.exists());
    assert!(!selected_operator.settings.socket.exists());
    assert!(!permitted_socket.exists());

    // Change only the late disclosure endpoint to the operator's canonical
    // alias. Deliberately absent model inputs make admission order observable.
    let conflicting_socket = alias.join("peers/operator.sock");
    assert_ne!(conflicting_socket, selected_operator.settings.socket);
    let conflicting_profile = settings(&conflicting_socket);
    fs::write(&precheck, &conflicting_profile).unwrap();
    fs::write(
        &plan,
        manifest("missing-first-recipe.json", "missing-later-recipe.json"),
    )
    .unwrap();
    let error = load_with_operator(
        &plan,
        &config,
        &peers,
        None,
        Some(&selected_operator),
    )
    .err()
    .unwrap();
    assert_eq!(
        error,
        "native precheck and operator sockets must be distinct",
        "operator disclosure alias must refuse before the missing recipe"
    );

    // Direct callers must carry the selected operator into the same complete
    // admission check, before clocks, recovery, source or durable setup.
    loaded[1].precheck = Some(
        exact_precheck::Selection::new(
            PrecheckSettings::decode(conflicting_profile.as_bytes()).unwrap(),
        )
        .unwrap(),
    );
    let mut output = Vec::new();
    let result = super::super::super::run_with_operator(
        config,
        loaded,
        &peers,
        None,
        (None, Some(&selected_operator)),
        || panic!("combined endpoint collision reached the first clock"),
        &mut output,
    );
    assert_eq!(
        result,
        Err("native precheck and operator sockets must be distinct".into())
    );
    assert!(output.is_empty());
    assert_eq!(fs::read(source_path).unwrap(), source);
    assert!(!store.exists());
    assert!(!selected_operator.settings.socket.exists());
    assert!(!permitted_socket.exists());
    assert!(!conflicting_socket.exists());
    assert!(actor_sockets.iter().all(|socket| !socket.exists()));
}
