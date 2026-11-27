//! Ordinary continuation owns its complete grammar and independent archive
//! custody before any live source, clock, listener or recovery Fence.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;

fn arguments() -> Vec<String> {
    [
        "serve-native-generated",
        "missing-config",
        "missing-sequence",
        "missing-reviewer",
        "--continue-generation",
        "--history-anchor",
        "original-input",
        "--retain-history-anchor",
        "new-output",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn ordinary_continuation_owns_its_unary_mode_before_all_pair_parsers() {
    let base = arguments();
    for extras in [
        vec![],
        vec!["--publication-profile", "witness"],
        vec!["--operator-profile", "operator", "--tower-generation", "2"],
        vec![
            "--publication-profile", "witness",
            "--operator-profile", "operator", "--tower-generation", "2",
        ],
    ] {
        let mut args = base.clone();
        args.extend(extras.into_iter().map(str::to_owned));
        for last in [false, true] {
            let mut selected = args.clone();
            if last {
                let flag = selected.remove(4);
                selected.push(flag);
            }
            assert!(andon::take_options(&selected, None).unwrap().1.is_none());
            let (remaining, continuing) =
                take_unaccounted_continuation(&selected, None).unwrap();
            assert!(continuing);
            assert!(!remaining.iter().any(|arg| arg == "--continue-generation"));
            assert_eq!(remaining.len() + 1, selected.len());
            let missing = Config::read(Path::new("missing-config")).err().unwrap();
            assert_eq!(
                command_with_clock(
                    &selected,
                    None,
                    || panic!("missing configuration acquired a clock"),
                    &mut Vec::new(),
                )
                .unwrap_err(),
                missing,
            );
        }
    }
    let unselected = base[..4].to_vec();
    assert_eq!(
        take_unaccounted_continuation(&unselected, None).unwrap(),
        (unselected, false)
    );
}

#[test]
fn ordinary_continuation_refuses_missing_custody_and_mixed_modes_before_input_reads() {
    let base = arguments();
    let mut refused = Vec::new();
    for flag in ["--history-anchor", "--retain-history-anchor"] {
        let mut args = base.clone();
        let index = args.iter().position(|arg| arg == flag).unwrap();
        args.drain(index..index + 2);
        refused.push(args);
    }
    for extra in [
        vec!["--continue-generation"],
        vec!["--after-message", "91"],
        vec!["--finish-after-message", "91"],
        vec!["--governor-profile", "governor"],
        vec!["--recovery-requirements", "requirements"],
        vec!["--precheck-profile", "precheck"],
        vec!["--topology-profile", "topology"],
        vec!["--requests", "91,92"],
        vec!["--history-anchor", "replacement"],
        vec!["--operator-profile", "operator"],
        vec!["--tower-generation", "1"],
        vec!["unselected-mode-value"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        refused.push(args);
    }
    for args in refused {
        let mut output = Vec::new();
        assert_eq!(
            command_with_clock(
                &args,
                None,
                || panic!("invalid continuation acquired a clock"),
                &mut output,
            ),
            Err(USAGE.to_owned()),
            "accepted {args:?}",
        );
        assert!(output.is_empty());
    }
    assert_eq!(
        command_with_clock(
            &base,
            Some(Path::new("unselected-qualification")),
            || panic!("unsupported activation acquired a clock"),
            &mut Vec::new(),
        ),
        Err(USAGE.to_owned())
    );
}

#[test]
fn ordinary_continuation_accepts_only_a_message_as_its_first_remaining_release() {
    let base = Path::new("/operator/native");
    for count in [1, MAX_ENTRIES] {
        let selected = (0..count)
            .map(|index| entry("recipe.json", &format!("actor-{index}.json"), "message"))
            .collect::<Vec<_>>();
        assert_eq!(
            decode_mode(
                document(&selected).as_bytes(),
                base,
                Mode::ContinueGeneration,
            )
            .unwrap()
            .len(),
            count,
        );
    }
    for selected in [
        vec![],
        vec![entry("recipe.json", "actor.json", "finish")],
        vec![entry("recipe.json", "actor.json", "message"); MAX_ENTRIES + 1],
    ] {
        assert!(decode_mode(
            document(&selected).as_bytes(), base, Mode::ContinueGeneration,
        ).is_err());
    }
    let complete = document(&[
        entry("recipe.json", "actor-1.json", "message"),
        entry("recipe.json", "actor-2.json", "finish"),
    ]);
    assert_eq!(
        decode_mode(complete.as_bytes(), base, Mode::ContinueGeneration).unwrap().len(),
        2,
    );
}

#[test]
fn ordinary_continuation_loader_retains_original_models_and_independent_archive_bytes() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let original = &entries[0].inputs;
    let selection = crate::workflow::actor_service::native::bootstrap::selection(
        &config, original, None,
    ).unwrap();
    let (host, human) = crate::workflow::actor_service::native::bootstrap::prepare(
        &config, original, selection, false,
    ).unwrap();
    let input = root.0.join("original.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &input).unwrap();
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    drop(human);
    drop(host);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    ).unwrap();
    let path = root.0.join("sequence.json");
    std::fs::write(&path, document(&[
        entry("recipe.json", "actor-0.json", "message"),
        entry("second.json", "actor-1.json", "message"),
        entry("second.json", "actor-2.json", "finish"),
    ])).unwrap();
    let mut args = arguments();
    args[6] = input.display().to_string();
    args[8] = root.0.join("continued.anchor").display().to_string();
    let (remaining, _) = take_unaccounted_continuation(&args, None).unwrap();
    let options = anchors::take_options(&remaining).unwrap().1;
    assert!(options.is_recovery_pair());
    let (loaded, history) = load_with_complete_services(
        &path, &config, &peers, Mode::ContinueGeneration,
        None, None, Some(options), None, None, None, &[],
    ).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].inputs.generation, 7);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert_eq!(loaded[0].inputs.decoder, original.decoder);
    assert_eq!(loaded[0].inputs.request, original.request);
    let history = history.unwrap();
    assert!(history.input().is_some());
    assert!(!history.output_path().unwrap().exists());
    assert_eq!(std::fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert!(loaded.iter().all(|entry| !entry.actor.socket.exists()));

    // A direct loader cannot omit the pair and proceed on an unanchored mode.
    assert_eq!(
        load_with_complete_services(
            Path::new("missing-sequence"), &config, &peers, Mode::ContinueGeneration,
            None, None, None, None, None, None, &[],
        ).err().unwrap(),
        USAGE,
    );
}
