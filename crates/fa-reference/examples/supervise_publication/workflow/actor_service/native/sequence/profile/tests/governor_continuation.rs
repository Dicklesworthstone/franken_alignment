//! Governed interrupted generation carries its exact mode through all parsers
//! and retains the complete selected recovery contracts before numerical work.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;

fn arguments(root: &Root) -> Vec<String> {
    let mut args = governed(root);
    args.extend([
        "--continue-generation".into(),
        "--history-anchor".into(), root.0.join("original.anchor").display().to_string(),
        "--retain-history-anchor".into(), root.0.join("continued.anchor").display().to_string(),
        "--recovery-requirements".into(), root.0.join("requirements.json").display().to_string(),
    ]);
    args
}

#[test]
fn governed_continuation_accepts_complete_original_recovery_selection_in_any_order() {
    let root = Root::new();
    let args = arguments(&root);
    let missing = Config::read(Path::new(&args[1])).err().unwrap();
    for publication in [false, true] {
        let mut selected = args.clone();
        if publication {
            selected.extend(["--publication-profile".into(), root.0.join("witness.json").display().to_string()]);
        }
        let mode = selected.iter().position(|arg| arg == "--continue-generation").unwrap();
        selected.remove(mode);
        let pairs = selected[4..].chunks_exact(2).map(|pair| pair.to_vec()).collect::<Vec<_>>();
        for shift in 0..pairs.len() {
            let mut ordered = pairs.clone();
            ordered.rotate_left(shift);
            for first in [false, true] {
                let mut args = selected[..4].to_vec();
                if first { args.push("--continue-generation".into()); }
                args.extend(ordered.iter().flatten().cloned());
                if !first { args.push("--continue-generation".into()); }
                let (remaining, continuing) = take_unaccounted_continuation(&args, None).unwrap();
                assert!(continuing);
                let (remaining, governor, recovery) =
                    governance::take_recovery_options(&remaining, None, continuing).unwrap();
                assert_eq!(governor, Some(root.0.join("governor.json")));
                assert_eq!(recovery, Some(root.0.join("requirements.json")));
                let (remaining, history) = history_custody::take_options(&remaining).unwrap();
                assert!(history.is_recovery_pair());
                history.check_selected_mode(Mode::ContinueGeneration).unwrap();
                let (remaining, operator) = take_operator(&remaining, None).unwrap();
                assert_eq!(operator, Some((root.0.join("operator.json"), 41)));
                assert_eq!(options(&remaining, None).unwrap().1, None);
                assert_eq!(command_failure(&args, None), missing);
            }
        }
    }
    assert!(!root.0.join("store").exists());
}

#[test]
fn governed_continuation_refuses_missing_or_mixed_contracts_before_input_reads() {
    let root = Root::new();
    let base = arguments(&root);
    let mut cases = Vec::new();
    for flag in [
        "--governor-profile", "--operator-profile", "--tower-generation",
        "--history-anchor", "--retain-history-anchor", "--recovery-requirements",
    ] {
        let mut missing = base.clone();
        let at = missing.iter().position(|arg| arg == flag).unwrap();
        missing.drain(at..at + 2);
        cases.push(missing);
        let mut duplicate = base.clone();
        duplicate.extend([flag.into(), "duplicate".into()]);
        cases.push(duplicate);
    }
    for extra in [
        vec!["--continue-generation"],
        vec!["--after-message", "91"],
        vec!["--finish-after-message", "91"],
        vec!["--topology-profile", "topology"],
        vec!["--evaluation-recovery", "evaluation"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        cases.push(args);
    }
    let missing_config = Config::read(Path::new(&base[1])).err().unwrap();
    for args in cases {
        let error = command_failure(&args, None);
        assert_ne!(error, missing_config, "invalid selection reached configuration: {args:?}");
    }
    assert_eq!(command_failure(&base, Some(Path::new("qualification"))), USAGE);
}

#[test]
fn governed_continuation_loads_the_original_assets_and_exact_independent_history() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    let (governor, operator) = selections(&root, &entries[0].actor);
    let original = &entries[0].inputs;
    let selected = crate::workflow::actor_service::native::bootstrap::selection(
        &config, original, None,
    ).unwrap();
    let (host, human, role) = crate::workflow::actor_service::native::bootstrap::prepare_governed(
        &config, original, selected, governor.campaign_requirement(),
    ).unwrap();
    let input = root.0.join("original.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &input).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    drop(role);
    drop(human);
    drop(host);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    ).unwrap();
    let path = root.0.join("sequence.json");
    fs::write(&path, document(&[
        entry("recipe.json", "actor-0.json", "message"),
        entry("second.json", "actor-1.json", "message"),
        entry("second.json", "actor-2.json", "finish"),
    ])).unwrap();
    let args = arguments(&root);
    let (remaining, continuing) = take_unaccounted_continuation(&args, None).unwrap();
    let (remaining, _, recovery) =
        governance::take_recovery_options(&remaining, None, continuing).unwrap();
    let history = history_custody::take_options(&remaining).unwrap().1;
    let (loaded, retained) = load_with_complete_services(
        &path, &config, &peers, Mode::ContinueGeneration,
        Some(&operator), None, Some(history), Some(&governor),
        recovery.as_deref(), None, &[],
    ).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].inputs.generation, 7);
    assert_eq!(loaded[0].inputs.decoder, original.decoder);
    assert_eq!(loaded[0].inputs.request, original.request);
    assert_eq!(loaded[1].inputs.generation, 8);
    assert!(matches!(loaded[2].release, Release::Finish));
    let retained = retained.unwrap();
    assert!(retained.input().is_some());
    assert!(!retained.output_path().unwrap().exists());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert!(loaded.iter().all(|entry| !entry.actor.socket.exists()));
    assert!(!governor.socket.exists());
}
