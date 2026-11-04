//! Healthy original generation captures a usable prefix before any new draw;
//! selecting a checkpoint never supplies review, human approval or a reset.
use super::*;
use super::super::checkpoint_on_generation as checkpoint;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use std::path::PathBuf;

fn document(id: u64, position: u64, draws: u64) -> String {
    format!(r#"{{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"{id}","expected_position":"{position}","expected_sampled_draws":"{draws}"}}"#)
}
fn selection(id: u64, position: u64, draws: u64) -> checkpoint::Selection {
    checkpoint::Selection::decode(document(id, position, draws).as_bytes()).unwrap()
}
fn selected_file(root: &Root, id: u64, position: u64, draws: u64) -> PathBuf {
    let path = root.0.join("generation-checkpoint.json");
    fs::write(&path, document(id, position, draws)).unwrap();
    path
}
fn functional(root: &Root) -> Config {
    let path = super::functional_creation::config_file(root, "functional_restart");
    Config::read_learned_generation(&path).unwrap()
}

#[test]
fn live_cli_captures_the_original_prompt_boundary_and_still_requires_the_human_key() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let config_path = super::functional_creation::config_file(&root, "functional_restart");
        let config = Config::read_learned_generation(&config_path).unwrap();
        let recipe_path = fixture::write(&root, b"allow");
        let selected = selected_file(&root, 71, 2, 0);
        evidence(&root, &config, 1, true, false);
        let (peer_path, peers) = super::functional_creation::peers_file(&root, &config);
        let anchor = root.0.join("checkpoint-created.anchor");
        let args = vec!["create-learned-generated".into(), config_path.display().to_string(),
            recipe_path.display().to_string(), peer_path.display().to_string(),
            "--checkpoint-before-generation".into(), selected.display().to_string(),
            "--retain-history-anchor".into(), anchor.display().to_string()];
        let mut human = None;
        let result = super::super::command_with_clock(&args, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1000)
        });
        let packet = human.expect("original native reviews reached the separate human endpoint")
            .join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve);
        let mut loaded = recipe::load(&recipe_path, &config, false).unwrap();
        let published = FileOversight::read_publication_with_learned_generation(
            &config.store, &config.profile, &loaded.generation).unwrap();
        assert_eq!(published.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(published.control.ledger.charged,
            if decision == ReviewDecision::Approve { packet.action().spec().units } else { 0 });
        assert_eq!(packet.views().len(), 2);
        if decision == ReviewDecision::Approve {
            loaded.recovery_anchor = Some(super::super::anchors::load(&anchor).unwrap());
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let saved = host.learned_stream_checkpoint(71).unwrap();
            assert_eq!(saved.info().position, 2);
            assert_eq!(saved.info().sampled_draws, 0);
            assert_eq!(saved.info().authority_epoch, 0);
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, 4);
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        }
    }
}

#[test]
fn mismatched_boundary_refuses_before_the_first_draw_and_never_captures_a_later_cut() {
    for (position, draws) in [(1, 0), (3, 0), (2, 1)] {
        let root = Root::new(); let mut config = functional(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let (_, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut control = Control::new(&config, 1, None).unwrap();
        let error = super::super::generate_with_checkpoint(&mut driver, &reviewer, &mut config,
            &mut control, &deadline(), &mut || ElapsedTick(1000),
            (None, Some(selection(1, position, draws)))).unwrap_err();
        assert!(error.contains("checkpoint"), "{error}");
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 2);
        assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert!(host.learned_stream_checkpoint(1).is_err());
        assert_eq!(host.inspect().executions, 0);
        drop(host);
        // The same actual owner can still proceed when its correct boundary is
        // explicitly selected; no failed selection spent a random draw.
        assert!(super::super::generate_with_checkpoint(&mut driver, &reviewer, &mut config,
            &mut control, &deadline(), &mut || ElapsedTick(1000),
            (None, Some(selection(1, 2, 0)))).unwrap());
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.learned_stream_checkpoint(1).unwrap().info().position, 2);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 0);
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn a_continued_message_captures_its_new_confirmed_boundary_without_refunding_work() {
    let root = Root::new(); let mut config = functional(&root);
    let path = super::continuation::write(&root);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port); let mut driver = FileSupervisedDriver::new(supervisor);
    let limit = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit, &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = super::continuation::review_current(
        &mut config, &loaded, &reviewer, driver, &limit);
    let human = super::continuation::approve(&mut driver, &mut config, &reviewer, 1);
    super::continuation::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    let before = driver.supervisor().host().unwrap().inspect();
    let command = driver.supervisor().host().unwrap()
        .prepare_learned_text_stream_continuation(1, 2).unwrap();
    assert!(super::super::continuation::prepare(&mut driver, &reviewer, &mut config,
        command, &mut control, &limit, &mut || ElapsedTick(1000)).unwrap());
    assert!(super::super::generate_with_checkpoint(&mut driver, &reviewer, &mut config,
        &mut control, &limit, &mut || ElapsedTick(1000), (None, Some(selection(2, 4, 2)))).unwrap());
    let host = driver.supervisor().host().unwrap();
    let saved = host.learned_stream_checkpoint(2).unwrap();
    assert_eq!(saved.info().position, 4); assert_eq!(saved.info().sampled_draws, 2);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    drop(host);
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(super::super::generate_with_checkpoint(&mut driver, &reviewer, &mut config,
        &mut control, &limit, &mut || ElapsedTick(1000), (None, Some(selection(2, 4, 2)))).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn expired_generation_and_invalid_profile_do_not_create_a_checkpoint_or_draw() {
    let root = Root::new(); let mut config = functional(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let (_, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(super::super::generate_with_checkpoint(&mut driver, &reviewer, &mut config,
        &mut control, &deadline(), &mut || ElapsedTick(61_001),
        (None, Some(selection(1, 2, 0)))).is_err());
    let host = driver.supervisor().host().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 0); assert_eq!(numerical.sampled_draws, 0);
    assert!(host.learned_stream_checkpoint(1).is_err());
    drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    let valid = document(1, 2, 0);
    for invalid in [valid.replace("/1", "/2"), valid.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"0\""),
        valid.replace("\"expected_position\":\"2\"", "\"expected_position\":\"02\""),
        valid.replace("\"expected_sampled_draws\":\"0\"", "\"expected_sampled_draws\":0"),
        valid.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"1\",\"checkpoint\":\"1\""),
        valid.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"1\",\"approve\":true")]
    { assert!(checkpoint::Selection::decode(invalid.as_bytes()).is_err()); }
}

#[test]
fn checkpoint_selection_refuses_unsupported_profiles_and_maintenance_before_store_creation() {
    for predictor in [None, Some(false), Some(true)] {
        let root = Root::new();
        let config = if predictor.is_some() { functional(&root) } else { configured(&root) };
        let path = super::continuation::write(&root);
        if let Some(owned) = predictor { super::forecast::install(&root, &path, owned, 60_000); }
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        loaded.checkpoint_on_generation = Some(selection(1, 2, 0));
        let peers = fixture::peers(&root, &config);
        assert!(run(config, loaded, &peers, false, || ElapsedTick(1000)).is_err());
        assert!(!root.0.join("store").exists());
    }
    let base = vec!["create-learned-generated".into(), "config".into(), "recipe".into(), "reviewer".into()];
    let mut good = base.clone(); good.extend(["--checkpoint-before-generation".into(), "/checkpoint.json".into()]);
    assert_eq!(checkpoint::take_option(&good).unwrap().0, base);
    for flag in ["--resume", "--complete-stream-reset", "--begin-stream-reset", "--recover-stop",
        "--policy-replay", "--promote-policy", "--finish-after-message", "--capture-stream-checkpoint"] {
        let mut wrong = good.clone(); wrong.extend([flag.into(), "1".into()]);
        assert!(checkpoint::take_option(&wrong).is_err(), "{flag}");
    }
    let mut duplicate = good.clone(); duplicate.extend(["--checkpoint-before-generation".into(), "/other.json".into()]);
    assert!(checkpoint::take_option(&duplicate).is_err());
    good.pop(); assert!(checkpoint::take_option(&good).is_err());
}
