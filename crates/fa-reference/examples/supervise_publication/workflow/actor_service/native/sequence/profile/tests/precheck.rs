//! Complete resident disclosure admission precedes every numerical asset read.
use super::*;
use crate::workflow::actor_service::precheck::Settings;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_PRECHECK_QUERIES;
use std::fs;

fn checked_entry(recipe: &str, actor: &str, release: &str, precheck: &str) -> String {
    format!(
        r#"{{"recipe":"{recipe}","actor_profile":"{actor}","release":"{release}","precheck_profile":{precheck}}}"#
    )
}

fn checked_document(entries: &[String]) -> String {
    format!(
        r#"{{"schema":"fa.native-publication-sequence/2","entries":[{}]}}"#,
        entries.join(",")
    )
}

fn settings(socket: &Path, id: u64, queries: usize) -> String {
    format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":{id},"query_limit":{queries},"exchange_limit":16,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
        socket.display()
    )
}

fn selection(socket: &Path, id: u64, queries: usize) -> exact_precheck::Selection {
    exact_precheck::Selection::new(
        Settings::decode(settings(socket, id, queries).as_bytes()).unwrap(),
    )
    .unwrap()
}

#[test]
fn native_sequence_v2_requires_explicit_release_disclosures_without_reinterpreting_v1() {
    let base = Path::new("/operator/native");
    let first = checked_entry(
        "first.json",
        "actor-1.json",
        "message",
        r#""precheck-1.json""#,
    );
    let second = checked_entry("second.json", "actor-2.json", "message", "null");
    let finish = checked_entry(
        "second.json",
        "actor-3.json",
        "finish",
        r#""/operator/finish-precheck.json""#,
    );
    let valid = checked_document(&[first.clone(), second.clone(), finish]);
    let parsed = decode(valid.as_bytes(), base).unwrap();
    assert_eq!(parsed[0].precheck, Some(base.join("precheck-1.json")));
    assert!(parsed[1].precheck.is_none());
    assert_eq!(
        parsed[2].precheck,
        Some(PathBuf::from("/operator/finish-precheck.json"))
    );
    assert!(matches!(parsed[2].release, Release::Finish));
    let original = document(&[
        entry("first.json", "actor-1.json", "message"),
        entry("second.json", "actor-2.json", "message"),
    ]);
    assert!(
        decode(original.as_bytes(), base)
            .unwrap()
            .iter()
            .all(|entry| entry.precheck.is_none())
    );
    for invalid in [
        valid.replace("sequence/2", "sequence/1"),
        valid.replace("sequence/2", "sequence/3"),
        original.replace("sequence/1", "sequence/2"),
        valid.replacen(",\"precheck_profile\":null", "", 1),
        checked_document(&[first, second.replace("null", "true")]),
        valid.replace("precheck-1.json", "../precheck.json"),
        valid.replace("precheck-1.json", ""),
        valid.replacen(
            r#""precheck_profile":null"#,
            r#""precheck_profile":null,"precheck_profile":null"#,
            1,
        ),
    ] {
        assert_ne!(
            invalid, valid,
            "a mutation must change the selected grammar"
        );
        assert!(
            decode(invalid.as_bytes(), base).is_err(),
            "accepted {invalid}"
        );
    }
    let finish = checked_document(&[checked_entry(
        "second.json",
        "actor-3.json",
        "finish",
        "null",
    )]);
    assert!(decode(finish.as_bytes(), base).is_err());
    assert!(
        decode_selected(finish.as_bytes(), base, Some(92)).unwrap()[0]
            .precheck
            .is_none()
    );
}

#[test]
fn native_sequence_v2_loader_freezes_every_disclosure_before_first_model_input() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let plan = root.0.join("sequence.json");
    fs::write(
        root.0.join("check-first.json"),
        settings(&root.0.join("check-first.sock"), 81, 1),
    )
    .unwrap();
    fs::write(root.0.join("check-next.json"), "{}").unwrap();
    fs::write(
        &plan,
        checked_document(&[
            checked_entry(
                "missing-model-recipe.json",
                "actor-0.json",
                "message",
                r#""check-first.json""#,
            ),
            checked_entry(
                "missing-model-recipe.json",
                "actor-1.json",
                "message",
                r#""check-next.json""#,
            ),
        ]),
    )
    .unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(
        error.contains("missing precheck field schema"),
        "late disclosure must precede missing recipe: {error}"
    );
    fs::write(
        root.0.join("check-next.json"),
        settings(&entries[0].actor.socket, 82, 1),
    )
    .unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(
        error.contains("conflicts"),
        "future alias must precede missing recipe: {error}"
    );
    fs::write(
        root.0.join("check-next.json"),
        settings(&root.0.join("check-next.sock"), 82, 1),
    )
    .unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(
        !error.contains("precheck") && !error.contains("conflicts"),
        "valid disclosure must reach original recipe admission: {error}"
    );
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}

#[test]
fn native_sequence_v2_loads_message_and_finish_disclosures_and_explicit_null() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    for (index, id) in [(0, 81), (2, 83)] {
        fs::write(
            root.0.join(format!("check-{index}.json")),
            settings(&root.0.join(format!("check-{index}.sock")), id, 1),
        )
        .unwrap();
    }
    let path = root.0.join("sequence.json");
    fs::write(
        &path,
        checked_document(&[
            checked_entry(
                "recipe.json",
                "actor-0.json",
                "message",
                r#""check-0.json""#,
            ),
            checked_entry("second.json", "actor-1.json", "message", "null"),
            checked_entry("second.json", "actor-2.json", "finish", r#""check-2.json""#),
        ]),
    )
    .unwrap();
    let loaded = load(&path, &config, &peers).unwrap();
    assert!(
        loaded[0].precheck.is_some()
            && loaded[1].precheck.is_none()
            && loaded[2].precheck.is_some()
    );
    assert_eq!(loaded[1].inputs.generation, loaded[2].inputs.generation);
    assert!(matches!(loaded[2].release, Release::Finish));
    for (index, id) in [(0, 81), (2, 83)] {
        let retained = &loaded[index].precheck.as_ref().unwrap().settings;
        assert_eq!(
            retained.socket_path(),
            root.0.join(format!("check-{index}.sock"))
        );
        let duplicate =
            Settings::decode(settings(&root.0.join("probe.sock"), id, 1).as_bytes()).unwrap();
        assert!(
            Settings::check_schedule([retained, &duplicate])
                .unwrap_err()
                .contains("distinct disclosure IDs")
        );
        fs::write(root.0.join(format!("check-{index}.json")), "{}").unwrap();
    }
    // Admission owns the original selections. Later file edits neither reopen
    // them at a handoff nor change their individual or aggregate allowance.
    validate(&config, &loaded, &peers).unwrap();
    assert!(
        load(&path, &config, &peers)
            .err()
            .unwrap()
            .contains("missing precheck field schema")
    );
    for (queries, accepted) in [
        (MAX_PRECHECK_QUERIES - 2, true),
        (MAX_PRECHECK_QUERIES - 1, false),
    ] {
        let remaining =
            Settings::decode(settings(&root.0.join("remaining.sock"), 1000, queries).as_bytes())
                .unwrap();
        let result = Settings::check_schedule(
            loaded
                .iter()
                .filter_map(|entry| entry.precheck.as_ref().map(|selected| &selected.settings))
                .chain([&remaining]),
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "retained disclosure query allowance changed"
        );
    }
    // Explicit nulls select no profile reads, including paths selected by a
    // previous manifest. The ordinary native recipes still load successfully.
    fs::write(
        &path,
        checked_document(&[
            checked_entry("recipe.json", "actor-0.json", "message", "null"),
            checked_entry("second.json", "actor-1.json", "message", "null"),
            checked_entry("second.json", "actor-2.json", "finish", "null"),
        ]),
    )
    .unwrap();
    let nulls = load(&path, &config, &peers).unwrap();
    assert!(nulls.iter().all(|entry| entry.precheck.is_none()));
    assert_eq!(nulls[0].inputs.decoder, loaded[0].inputs.decoder);
    assert_eq!(nulls[1].inputs.generation, loaded[1].inputs.generation);
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
    assert!(!config.store.exists());
}

#[test]
fn new_native_sequence_refuses_noninitial_first_disclosure_before_assets_clock_or_store() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let socket = root.0.join("check-first.sock");
    let document = settings(&socket, 81, 1).replacen(
        r#""expected_policy_epoch":0"#,
        r#""expected_policy_epoch":1"#,
        1,
    );
    entries[0].precheck = Some(
        exact_precheck::Selection::new(Settings::decode(document.as_bytes()).unwrap()).unwrap(),
    );
    fs::write(root.0.join("check-first.json"), document).unwrap();
    let plan = root.0.join("sequence.json");
    fs::write(
        &plan,
        checked_document(&[
            checked_entry(
                "missing-model-recipe.json",
                "actor-0.json",
                "message",
                r#""check-first.json""#,
            ),
            checked_entry(
                "missing-model-recipe.json",
                "actor-1.json",
                "message",
                "null",
            ),
            checked_entry(
                "missing-model-recipe.json",
                "actor-2.json",
                "finish",
                "null",
            ),
        ]),
    )
    .unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(
        error.contains("precheck") && error.contains("epoch"),
        "a deterministically invalid initial disclosure reached missing model assets: {error}"
    );

    // Recovery has an original owner whose current epoch is checked after
    // reconstruction. It must not be assigned the fresh owner's initial epoch.
    validate_selected(&config, &entries, &peers, Some(90)).unwrap();
    // A separately selected later release may follow an operator restriction;
    // only the first creation grant has a deterministically known epoch here.
    let first = entries[0].precheck.take();
    entries[1].precheck = first;
    validate(&config, &entries, &peers).unwrap();
    let later = entries[1].precheck.take();
    entries[0].precheck = later;

    let store = config.store.clone();
    let source_path = config.source.path().to_owned();
    let source = fs::read(&source_path).unwrap();
    let actor_sockets = entries
        .iter()
        .map(|entry| entry.actor.socket.clone())
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let result = super::super::super::run(
        config,
        entries,
        &peers,
        None,
        || panic!("invalid initial disclosure reached the first clock"),
        &mut output,
    );
    assert_eq!(result, Err(error));
    assert!(output.is_empty());
    assert_eq!(fs::read(source_path).unwrap(), source);
    assert!(
        !store.exists(),
        "an unusable first grant consumed the new authority store"
    );
    assert!(!socket.exists());
    assert!(actor_sockets.iter().all(|path| !path.exists()));
}

#[test]
fn resident_native_disclosures_share_one_finite_schedule_ceiling_and_unique_identity() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    entries.truncate(2);
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.precheck = Some(selection(
            &root.0.join(format!("check-{index}.sock")),
            81 + index as u64,
            64,
        ));
    }
    validate(&config, &entries, &peers).unwrap();
    entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 82, 65));
    assert!(
        validate(&config, &entries, &peers)
            .unwrap_err()
            .contains("aggregate")
    );
    entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 81, 64));
    assert!(
        validate(&config, &entries, &peers)
            .unwrap_err()
            .contains("distinct disclosure IDs")
    );
    for (field, value) in [("exchange_limit", 128), ("candidate_limit", 1024)] {
        let first = settings(&root.0.join("check-0.sock"), 81, 1).replace(
            &format!(
                "\"{field}\":{}",
                if field == "exchange_limit" { 16 } else { 8 }
            ),
            &format!("\"{field}\":{value}"),
        );
        entries[0].precheck = Some(
            exact_precheck::Selection::new(Settings::decode(first.as_bytes()).unwrap()).unwrap(),
        );
        entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 82, 1));
        assert!(
            validate(&config, &entries, &peers)
                .unwrap_err()
                .contains("aggregate")
        );
    }
    assert!(!config.store.exists());
}

#[test]
fn resident_native_disclosure_admission_protects_all_future_names_and_canonical_aliases() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    entries[0].precheck = Some(selection(&root.0.join("check-0.sock"), 81, 1));
    validate(&config, &entries, &peers).unwrap();
    for collision in [
        entries[2].actor.socket.clone(),
        peers.socket(entries[2].actor.request),
        crate::workflow::control::socket_path(&peers, entries[2].actor.request),
        config.source.path().to_owned(),
        config.store.join("query.sock"),
    ] {
        entries[0].precheck = Some(selection(&collision, 81, 1));
        assert!(
            validate(&config, &entries, &peers).is_err(),
            "accepted future collision {collision:?}"
        );
        assert!(!config.store.exists());
    }
    let real = root.0.join("real");
    let alias = root.0.join("alias");
    fs::create_dir(&real).unwrap();
    fs::create_dir(real.join("peers")).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    // The immediate listener directory remains a real protected directory;
    // only an ancestor aliases it, matching the original host contract.
    entries[0].precheck = Some(selection(&real.join("peers/query.sock"), 81, 1));
    entries[1].precheck = Some(selection(&alias.join("peers/other.sock"), 82, 1));
    validate(&config, &entries, &peers).unwrap();
    entries[1].precheck = Some(selection(&alias.join("peers/query.sock"), 82, 1));
    assert!(
        validate(&config, &entries, &peers)
            .unwrap_err()
            .contains("conflicts")
    );
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}

#[test]
fn accounted_continuation_manifest_keeps_explicit_disclosures_at_the_recovered_epoch() {
    use super::super::super::evaluation::Recovery;
    use crate::workflow::actor_service::learned::anchors as archive;
    use crate::workflow::actor_service::{andon_settings, evaluator};

    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    for (index, id) in [(0, 81), (2, 83)] {
        let selected = settings(&root.0.join(format!("check-{index}.sock")), id, 1)
            .replacen("\"expected_policy_epoch\":0", "\"expected_policy_epoch\":1", 1);
        fs::write(root.0.join(format!("check-{index}.json")), selected).unwrap();
    }
    let path = root.0.join("sequence.json");
    let manifest = checked_document(&[
        checked_entry("recipe.json", "actor-0.json", "message", r#""check-0.json""#),
        checked_entry("second.json", "actor-1.json", "message", "null"),
        checked_entry("second.json", "actor-2.json", "finish", r#""check-2.json""#),
    ]);
    fs::write(&path, &manifest).unwrap();
    let accounting = andon_settings::Settings::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/native_andon_operator.json"
    )))
    .unwrap();
    let selected = bootstrap::selection(&config, &entries[0].inputs, None).unwrap();
    let (host, human, evaluation_role) =
        bootstrap::prepare_andon(&config, &entries[0].inputs, selected, &accounting).unwrap();
    let before = host.inspect();
    let input = root.0.join("original.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &input).unwrap();
    drop(evaluation_role);
    drop(human);
    drop(host);
    let requirements_path = root.0.join("evaluation-recovery.json");
    fs::write(
        &requirements_path,
        format!(
            r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
            before.revision, before.control.sequence, before.control.ledger.epoch,
        ),
    )
    .unwrap();
    let recovery = Recovery::read(&requirements_path, &config).unwrap();
    let actor = &entries[0].actor;
    let scope = actor.scope;
    let server = actor.supervisor;
    let peer_pid = server.pid().unwrap().checked_add(100_000).unwrap();
    let evaluator = evaluator::Settings::decode(
        format!(
            r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}/evaluator.sock",
            "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
            "deployment":101,"evaluator":202,
            "supervisor":{{"uid":{},"gid":{},"pid":{}}},
            "peer":{{"uid":{},"gid":{},"pid":{peer_pid}}},
            "candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
            root.0.display(),
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            server.uid(),
            server.gid(),
            server.pid().unwrap(),
            server.uid(),
            server.gid(),
        )
        .as_bytes(),
    )
    .unwrap();
    let output = root.0.join("continued.anchor");
    let args = [
        "serve-native-generated".into(),
        "config".into(),
        "sequence".into(),
        "reviewer".into(),
        "--history-anchor".into(),
        input.display().to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
    ];
    let journal = fs::read(config.store.join("delivery.bin")).unwrap();
    let source = fs::read(config.source.path()).unwrap();
    let original = fs::read(&input).unwrap();
    let admit = || {
        load_with_complete_services(
            &path,
            &config,
            &peers,
            Mode::ContinueGeneration,
            None,
            None,
            Some(
                crate::workflow::actor_service::native::sequence::anchors::take_options(&args)
                    .unwrap()
                    .1,
            ),
            None,
            None,
            Some(crate::workflow::actor_service::native::sequence::andon::Selection {
                settings: &accounting,
                evaluator: &evaluator,
                recovery: Some(&recovery),
                paths: None,
            }),
            &[],
        )
    };
    // This recovery mode has after=None. Its explicit epoch-one grant must
    // survive the file loader instead of being treated as a new epoch-zero
    // owner. The actual recovery Fence and epoch match remain runtime checks.
    let (loaded, history) = admit().unwrap();
    assert_eq!(loaded.len(), 3);
    assert!(loaded[0].precheck.is_some());
    assert!(loaded[1].precheck.is_none());
    assert!(loaded[2].precheck.is_some());
    assert_eq!(loaded[0].inputs.generation, 7);
    assert_eq!(loaded[1].inputs.generation, loaded[2].inputs.generation);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert_eq!(
        history.as_ref().unwrap().input().unwrap().revision(),
        before.revision
    );

    // Every selected disclosure still parses before the first model input.
    // Accounting must never make a malformed future profile optional.
    fs::write(root.0.join("check-2.json"), "{}").unwrap();
    fs::write(
        &path,
        manifest.replace("recipe.json", "missing-model-recipe.json"),
    )
    .unwrap();
    let error = admit().err().unwrap();
    assert!(
        error.contains("missing precheck field schema"),
        "a malformed future disclosure reached the missing recipe: {error}"
    );
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), journal);
    assert_eq!(fs::read(config.source.path()).unwrap(), source);
    assert_eq!(fs::read(&input).unwrap(), original);
    assert!(!output.exists());
    assert!(!evaluator.socket_path().exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
    assert!(!root.0.join("check-0.sock").exists());
    assert!(!root.0.join("check-2.sock").exists());
}
