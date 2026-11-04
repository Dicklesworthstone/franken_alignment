//! Operator selection reaches the original immutable predictive stream recipe.
//! Synthetic model/probability fixtures test custody, not empirical calibration.
use super::*;
use super::super::super::checkpoint_on_generation;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;

#[test]
fn owned_prompt_reset_is_explicit_and_disabled_selection_keeps_the_legacy_recipe() {
    let root = Root::new();
    let config_path = super::super::functional_creation::config_file(&root, "functional_restart");
    let config = Config::read_learned_generation(&config_path).unwrap();
    let path = fixture::write(&root, b"allow");
    let file = install(&root, &path, true, 60_000);
    let original = fs::read_to_string(&file).unwrap();
    let legacy = recipe::load(&path, &config, false).unwrap();
    fs::write(&file, original.replacen(r#""schema":"fa.learned-forecast/1""#,
        r#""schema":"fa.learned-forecast/2","reset":"disabled""#, 1)).unwrap();
    let disabled = recipe::load(&path, &config, false).unwrap();
    assert_eq!(disabled.generation, legacy.generation);
    assert!(!disabled.generation.requires_owned_forecast_stream_restart_audit());
    let selected = fs::read_to_string(&file).unwrap().replace(
        r#""reset":"disabled""#, r#""reset":"owned_prompt_audit""#);
    fs::write(&file, selected).unwrap();
    let active = recipe::load(&path, &config, false).unwrap();
    assert!(active.generation.requires_owned_forecast_stream_restart_audit());
    assert!(active.generation.follows_forecast_reset_successors());
    assert!(active.generation.requires_forecast_text_stream_continuation());
    assert_ne!(active.generation, legacy.generation);
    let (host, _, _, _) = recovery::create_selected(&config, &active).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    drop(host);
    assert!(recovery::open_selected(&config, &legacy).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    let (host, _, observer, _) = recovery::open_selected(&config, &active).unwrap();
    assert!(observer.is_some());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
}

#[test]
fn prompt_reset_schema_and_source_near_misses_refuse_before_model_intake() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    let file = install_restart(&root, &path, 60_000);
    let valid = fs::read_to_string(&file).unwrap();
    recipe::load(&path, &config, false).unwrap();
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    for changed in [
        valid.replace("fa.learned-forecast/2", "fa.learned-forecast/1"),
        valid.replace("fa.learned-forecast/2", "fa.learned-forecast/3"),
        valid.replace(r#""reset":"owned_prompt_audit","#, ""),
        valid.replace(r#""reset":"owned_prompt_audit""#, r#""reset":"automatic""#),
        valid.replace(r#""reset":"owned_prompt_audit""#, r#""reset":true"#),
        valid.replace(r#""reset":"owned_prompt_audit""#,
            r#""reset":"owned_prompt_audit","reset":"disabled""#),
        valid.replace(r#""kind":"owned_learned_kv""#, r#""kind":"raw_residual""#),
    ] {
        fs::write(&file, changed).unwrap();
        let failure = recipe::load(&path, &config, false).err().unwrap();
        assert!(!failure.contains("No such file"), "reached the missing model: {failure}");
        assert!(!config.store.exists());
    }
}

#[test]
fn owned_live_checkpoint_precedes_forecast_and_preserves_the_separate_human_key() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let config_path = super::super::functional_creation::config_file(&root, "functional_restart");
        let config = Config::read_learned_generation(&config_path).unwrap();
        let path = fixture::write(&root, b"allow");
        install_restart(&root, &path, 60_000);
        let checkpoint = root.0.join("owned-prompt-checkpoint.json");
        fs::write(&checkpoint, r#"{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"71","expected_position":"2","expected_sampled_draws":"0"}"#).unwrap();
        evidence(&root, &config, 1, true, false);
        let (peer_path, peers) = super::super::functional_creation::peers_file(&root, &config);
        let anchor = root.0.join("owned-prompt.anchor");
        let args = vec!["create-learned-generated".into(), config_path.display().to_string(),
            path.display().to_string(), peer_path.display().to_string(),
            "--checkpoint-before-generation".into(), checkpoint.display().to_string(),
            "--retain-history-anchor".into(), anchor.display().to_string()];
        let mut human = None;
        let result = super::super::super::command_with_clock(&args, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1000)
        });
        human.expect("native review reached independent human custody").join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve);
        let mut loaded = recipe::load(&path, &config, false).unwrap();
        let published = FileOversight::read_publication_with_learned_generation(
            &config.store, &config.profile, &loaded.generation).unwrap();
        assert_eq!(published.executions, u64::from(decision == ReviewDecision::Approve));
        if decision == ReviewDecision::Approve {
            loaded.recovery_anchor = Some(super::super::super::anchors::load(&anchor).unwrap());
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let saved = host.learned_stream_checkpoint(71).unwrap();
            assert_eq!(saved.info().position, 2);
            assert_eq!(saved.info().sampled_draws, 0);
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        }
    }
}

#[test]
fn owned_live_checkpoint_rejects_sampling_history_before_creating_the_store() {
    let root = Root::new();
    let config_path = super::super::functional_creation::config_file(&root, "functional_restart");
    let config = Config::read_learned_generation(&config_path).unwrap();
    let path = fixture::write(&root, b"allow");
    install_restart(&root, &path, 60_000);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.checkpoint_on_generation = Some(checkpoint_on_generation::Selection::decode(
        br#"{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"1","expected_position":"3","expected_sampled_draws":"1"}"#).unwrap());
    let peers = fixture::peers(&root, &config);
    assert!(run(config, loaded, &peers, false, || ElapsedTick(1000)).is_err());
    assert!(!root.0.join("store").exists());
}
