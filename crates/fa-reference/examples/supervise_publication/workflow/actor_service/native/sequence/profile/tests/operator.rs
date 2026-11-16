//! Complete optional operator admission precedes native model asset loading.
use super::*;
use crate::workflow::actor_service::operator::Settings;

fn operator_selection(root: &Root, actor: &Profile) -> operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let settings = Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":4,"connection_limit":4,"exchange_limit":16,
        "runtime_ms":15000,"poll_ms":1,"inspection_queries":1,
        "explanation_queries":0,"explanation_replay_events":0,
        "explanation_disclosure":"none"}}"#,
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            root.0.display(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
        )
        .as_bytes(),
    )
    .unwrap();
    operator::Selection::new(settings, 41).unwrap()
}

#[test]
fn resident_native_operator_pair_has_one_closed_grammar_for_creation_and_recovery() {
    let base = [
        "serve-native-generated",
        "/missing/config",
        "/missing/sequence",
        "/missing/reviewer",
    ]
    .map(str::to_owned)
    .to_vec();
    for generation in [1, u64::MAX] {
        let pair = [
            "--operator-profile".into(),
            "/missing/operator".into(),
            "--tower-generation".into(),
            generation.to_string(),
        ];
        for suffix in [
            vec![],
            vec!["--publication-profile".into(), "/missing/witness".into()],
            vec!["--after-message".into(), "90".into()],
            vec![
                "--publication-profile".into(),
                "/missing/witness".into(),
                "--after-message".into(),
                "90".into(),
            ],
        ] {
            for pair_first in [false, true] {
                let mut args = base.clone();
                if pair_first {
                    args.extend(pair.clone());
                }
                args.extend(suffix.clone());
                if !pair_first {
                    args.extend(pair.clone());
                }
                let (remaining, selected) = take_operator(&args, None).unwrap();
                assert_eq!(
                    selected,
                    Some((PathBuf::from("/missing/operator"), generation))
                );
                let mut original = base.clone();
                original.extend(suffix.clone());
                assert_eq!(remaining, original);
                options(&remaining, None).unwrap();
                assert!(take_operator(&args, Some(Path::new("capsule"))).is_err());
            }
        }
    }
    let valid = [
        base.clone(),
        vec![
            "--operator-profile".into(),
            "/missing/operator".into(),
            "--tower-generation".into(),
            "41".into(),
        ],
    ]
    .concat();
    let mut rejected = vec![valid[..7].to_vec()];
    for generation in ["0", "01", "+1", " 1", "1.0", "18446744073709551616"] {
        let mut args = valid.clone();
        args[7] = generation.into();
        rejected.push(args);
    }
    for pair in [
        ["--operator-profile", "/missing/second"],
        ["--tower-generation", "42"],
        ["--native-text", "recipe"],
        ["--governor-profile", "governor"],
        ["--precheck-profile", "precheck"],
        ["--requests", "91,92"],
        ["--finish-after-message", "90"],
    ] {
        let mut args = valid.clone();
        args.extend(pair.map(str::to_owned));
        rejected.push(args);
    }
    for args in rejected {
        assert_eq!(
            command_with_clock(
                &args,
                None,
                || panic!("invalid operator grammar read time"),
                &mut Vec::new()
            ),
            Err(USAGE.into()),
        );
    }
}

#[test]
fn resident_native_operator_checks_every_future_endpoint_and_scope_before_recipe_reads() {
    for collision in 0..7 {
        let root = Root::new();
        let (config, peers, mut entries) = selected(&root);
        let mut chosen = operator_selection(&root, &entries[0].actor);
        chosen.settings.socket = match collision {
            0 => entries[1].actor.socket.clone(),
            1 => peers.socket(entries[2].actor.request),
            2 => crate::workflow::control::socket_path(&peers, entries[2].actor.request),
            3 => config.source.path().to_owned(),
            4 => {
                std::fs::create_dir(&config.store).unwrap();
                config.store.join("operator.sock")
            }
            5 => {
                let real = root.0.join("real");
                std::fs::create_dir(&real).unwrap();
                std::fs::create_dir(real.join("peers")).unwrap();
                std::os::unix::fs::symlink(&real, root.0.join("alias")).unwrap();
                entries[2].actor.socket = real.join("peers/later.sock");
                root.0.join("alias/peers/later.sock")
            }
            _ => {
                chosen.settings.scope.principal += 1;
                chosen.settings.socket.clone()
            }
        };
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
        )
        .unwrap();
        let source = std::fs::read(config.source.path()).unwrap();
        let error = load_with_operator(&plan, &config, &peers, None, Some(&chosen))
            .err()
            .unwrap();
        assert!(
            error.contains("operator"),
            "operator preflight lost to missing model: {error}"
        );
        assert_eq!(std::fs::read(config.source.path()).unwrap(), source);
        assert!(!config.store.join("delivery.bin").exists());
        assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
        let result = super::super::super::run_with_operator(
            config,
            entries,
            &peers,
            None,
            (None, Some(&chosen)),
            || panic!("invalid operator selection reached the first clock"),
            &mut Vec::new(),
        );
        assert!(result.is_err());
    }
}

#[test]
fn resident_native_operator_loader_retains_exact_assets_without_starting_any_service() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let chosen = operator_selection(&root, &entries[0].actor);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    let path = root.0.join("sequence.json");
    std::fs::write(
        &path,
        document(&[
            entry("recipe.json", "actor-0.json", "message"),
            entry("second.json", "actor-1.json", "message"),
            entry("second.json", "actor-2.json", "finish"),
        ]),
    )
    .unwrap();
    let loaded = load_with_operator(&path, &config, &peers, None, Some(&chosen)).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].inputs.decoder, entries[0].inputs.decoder);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert_eq!(loaded[1].inputs.request, loaded[2].inputs.request);
    assert!(!config.store.exists());
    assert!(!chosen.settings.socket.exists());
    assert!(loaded.iter().all(|entry| !entry.actor.socket.exists()));
}
