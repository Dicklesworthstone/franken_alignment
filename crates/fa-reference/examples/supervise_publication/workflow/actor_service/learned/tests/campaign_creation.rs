//! Explicit campaign provisioning reaches the original native bootstrap and
//! real publication gates. Recovery-only descriptions never create authority.
use super::*;
use super::continuation as native;
use super::super::{anchors, campaigns};
use fa_reference::action::consequence::{
    delivery::{persistent::{governance::PolicyUpdate, observed::{
        guarded::FileCampaignRequirement,
        decoder::learned::sidecar::FileLearnedSidecarFinish,
        reviewer::wire::ReviewDecision,
    }}, stream::ReleaseFrame},
    gate::containment::session::policy::{Policy, Predicate},
    policy_campaign::ReplayLimits,
};
use std::path::PathBuf;

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement { limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 8 }
}
fn profile(root: &Root) -> PathBuf {
    let path = root.0.join("campaign-profile.json");
    fs::write(&path, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap();
    path
}

#[test]
fn explicitly_campaign_guarded_creation_generates_reviews_and_publishes_original_text() {
    let root = Root::new();
    let mut config = configured(&root);
    let path = native::write(&root);
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    campaigns::admit(&config, &mut loaded, Some(&profile(&root)), false).unwrap();
    assert_eq!(loaded.expected_campaigns, Some(requirement()));
    assert_eq!(loaded.creation_campaigns, Some(requirement()));
    let (mut host, reviewer, observer, _) = recovery::create_selected(&config, &loaded).unwrap();
    assert!(observer.is_none());
    assert!(host.policy_campaigns_required());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
    let state = host.inspect();
    let update = PolicyUpdate::new(81, state.control.sequence, state.control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(512)]).unwrap()).unwrap();
    assert_eq!(host.replace_policy(host.revision(), &update),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect(), state);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, loaded.request));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(driver.supervisor().host().unwrap().stream_snapshot().unwrap()
        .confirmed.messages().collect::<Vec<_>>(), ["A"]);
    let archive = driver.supervisor().host().unwrap().history_anchor().unwrap();
    drop(history);
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);

    let mut recovered = recipe::load(&path, &config, false).unwrap();
    recovered.recovery_anchor = Some(archive);
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(recovery::open_selected(&config, &recovered).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    recovered.expected_campaigns = Some(requirement());
    let (host, _, _, _) = recovery::open_selected(&config, &recovered).unwrap();
    assert!(host.policy_campaigns_required());
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
}

#[test]
fn first_image_campaign_inventory_composes_with_both_original_predictor_roles() {
    for owned in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let path = native::write(&root);
        super::forecast::install(&root, &path, owned, 60_000);
        let mut loaded = recipe::load(&path, &config, false).unwrap();
        campaigns::admit(&config, &mut loaded, Some(&profile(&root)), false).unwrap();
        let (host, _, observer, _) = recovery::create_selected(&config, &loaded).unwrap();
        assert!(observer.is_some());
        assert!(host.policy_campaigns_required());
        assert!(host.action_consistency_required());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
        let archive = host.history_anchor().unwrap();
        let before = host.inspect();
        drop(host);
        loaded.creation_campaigns = None;
        loaded.recovery_anchor = Some(archive);
        let (host, _, recovered_observer, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert!(recovered_observer.is_some());
        assert!(host.policy_campaigns_required());
        assert_eq!(host.revision(), before.revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
    }
}

#[test]
fn campaign_creation_selection_is_explicit_bounded_and_cannot_upgrade_recovery() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    let selected = profile(&root);
    let bytes = fs::read_to_string(&selected).unwrap();
    assert_eq!(campaigns::decode(bytes.as_bytes()).unwrap(), requirement());
    for invalid in [
        bytes.replace("\"cases\":16", "\"cases\":0"),
        bytes.replace("\"input_bytes\":1048576", "\"input_bytes\":33554433"),
        bytes.replace("\"max_campaigns\":8", "\"max_campaigns\":65"),
        bytes.replace("\"max_campaigns\":8", "\"max_campaigns\":8,\"approve\":true"),
        bytes.replace("\"cases\":16", "\"cases\":16,\"cases\":16"),
        bytes.replace("\"cases\":16", "\"cases\":\"16\""),
    ] { assert!(campaigns::decode(invalid.as_bytes()).is_err()); }
    let mut loaded = recipe::load(&path, &config, false).unwrap();
    assert!(campaigns::admit(&config, &mut loaded, Some(&selected), true).is_err());
    assert_eq!(loaded.expected_campaigns, None);
    loaded.expected_campaigns = Some(requirement());
    assert!(recovery::create_selected(&config, &loaded).is_err());
    assert!(!config.store.exists());
    let changed = FileCampaignRequirement { max_campaigns: 9, ..requirement() };
    loaded.expected_campaigns = Some(changed);
    assert!(campaigns::admit(&config, &mut loaded, Some(&selected), false).is_err());
    assert!(!config.store.exists());
    loaded.expected_campaigns = Some(requirement());
    campaigns::admit(&config, &mut loaded, Some(&selected), false).unwrap();
    let (host, _, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    assert!(host.policy_campaigns_required());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    let archive = root.0.join("created-campaign.anchor");
    anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
    assert!(archive.is_file());
}

#[test]
fn campaign_profile_cli_creates_guarded_history_and_uses_native_and_authenticated_human_keys() {
    let root = Root::new();
    let config_path = super::functional_creation::config_file(&root, "functional_restart");
    let config = Config::read_learned_generation(&config_path).unwrap();
    let recipe_path = fixture::write(&root, b"allow");
    let campaign_path = profile(&root);
    evidence(&root, &config, 1, true, false);
    let (peer_path, peers) = super::functional_creation::peers_file(&root, &config);
    let anchor_path = root.0.join("campaign-created.anchor");
    let args = vec!["create-learned-generated".into(), config_path.display().to_string(),
        recipe_path.display().to_string(), peer_path.display().to_string(),
        "--campaign-profile".into(), campaign_path.display().to_string(),
        "--retain-history-anchor".into(), anchor_path.display().to_string()];
    let mut human = None;
    let result = super::super::command_with_clock(&args, None, || {
        if human.is_none() && peers.socket(1).exists() {
            human = Some(super::peer_intake::human(peers.clone(), 1, ReviewDecision::Approve));
        }
        ElapsedTick(1000)
    });
    let packet = human.expect("guarded creation reached the original independent human endpoint")
        .join().unwrap();
    result.unwrap();
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    assert_eq!(packet.views().len(), 2);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    let mut loaded = recipe::load(&recipe_path, &config, false).unwrap();
    loaded.recovery_anchor = Some(anchors::load(&anchor_path).unwrap());
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(recovery::open_selected(&config, &loaded).is_err(),
        "the actual CLI-created campaign guard must be independently required on recovery");
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    loaded.expected_campaigns = Some(requirement());
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    assert!(host.policy_campaigns_required());
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"A");
    assert_eq!(host.inspect().control.ledger.charged, packet.action().spec().units);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    let FileLearnedSidecarFinish::Applied { receipt, archive, .. } =
        host.learned_sidecar_outcome(1).unwrap().result else { panic!("missing original native review"); };
    assert!(receipt.is_ok());
    assert_eq!(archive.inputs.action(), packet.action());
    assert_eq!(archive.commit_times.len(), 2); assert_eq!(archive.reveal_times.len(), 2);
}

#[test]
fn campaign_selection_composes_with_live_actor_precheck_but_cannot_disguise_maintenance() {
    let base = vec!["create-learned-generated".into(), "config".into(),
        "recipe".into(), "reviewer".into()];
    for precheck in [false, true] {
        let mut args = base.clone();
        args.extend(["--campaign-profile".into(), "/campaign.json".into(),
            "--actor-profile".into(), "/actor.json".into()]);
        if precheck { args.extend(["--precheck-profile".into(), "/precheck.json".into()]); }
        let (remaining, selected_precheck) = super::super::precheck::take_option(&args).unwrap();
        assert_eq!(selected_precheck, precheck.then_some(std::path::Path::new("/precheck.json")));
        let (remaining, selected_campaign) = campaigns::take_option(&remaining).unwrap();
        assert_eq!(selected_campaign, Some(PathBuf::from("/campaign.json")));
        let (remaining, selected_actor) = super::super::ingress::take_option(&remaining).unwrap();
        assert_eq!(selected_actor, Some(std::path::Path::new("/actor.json")));
        assert_eq!(remaining, base);
        if precheck {
            for mode in ["--resume", "--policy-replay", "--promote-policy",
                "--recover-stop", "--begin-stream-reset", "--capture-stream-checkpoint"] {
                let mut invalid = args.clone(); invalid.extend([mode.into(), "1".into()]);
                assert!(super::super::precheck::take_option(&invalid).is_err(), "{mode}");
                assert!(super::super::command_with_clock(&invalid, None,
                    || panic!("incompatible maintenance must refuse before a clock or source")).is_err());
            }
        }
    }
    // These are valid live continuation grammars, but first-image provisioning
    // cannot use them to install a mandatory guard after work already exists.
    let root = Root::new();
    let config_path = super::functional_creation::config_file(&root, "functional_restart");
    let config = Config::read_learned_generation(&config_path).unwrap();
    let recipe_path = fixture::write(&root, b"allow");
    let (peer_path, _) = super::functional_creation::peers_file(&root, &config);
    let campaign_path = profile(&root);
    let base = vec!["create-learned-generated".into(), config_path.display().to_string(),
        recipe_path.display().to_string(), peer_path.display().to_string(),
        "--campaign-profile".into(), campaign_path.display().to_string()];
    for mode in [vec!["--continue-generation"], vec!["--after-message", "1"],
        vec!["--finish-after-message", "1"]] {
        let mut args = base.clone(); args.extend(mode.into_iter().map(str::to_owned));
        let failure = super::super::command_with_clock(&args, None,
            || panic!("creation selection cannot start recovery")).unwrap_err();
        assert!(failure.contains("only for new learned histories"), "{failure}");
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
