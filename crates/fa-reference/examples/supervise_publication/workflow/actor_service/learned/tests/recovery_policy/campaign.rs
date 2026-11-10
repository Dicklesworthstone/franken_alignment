//! Independent campaign inventory through the original learned recovery owner.
//! Numerical fixtures and real campaign transitions exercise custody, not the
//! statistical quality of a policy or independent authentication of operators.
use super::*;
use fa_reference::action::consequence::{
    delivery::persistent::{
        JournalError,
        observed::{
            governance::campaigns::FilePolicyGovernor,
            guarded::FileCampaignRequirement,
        },
    },
    oversight::policy_governance::{CampaignDisposition, MAX_POLICY_CAMPAIGNS},
    policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayCaseId},
};
use fa_reference::Error;

fn guard() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 8,
    }
}

fn campaign_json(expected: FileCampaignRequirement) -> String {
    format!(r#"{{"cases":{},"input_bytes":{},"max_campaigns":{}}}"#,
        expected.limits.cases, expected.limits.input_bytes, expected.max_campaigns)
}

fn wrapped(inner: &str, generation: u64, maximum: usize,
    expected: FileCampaignRequirement) -> String
{
    format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{},"campaigns":{},"recipe":{inner}}}"#,
        policy_json(generation, maximum), campaign_json(expected))
}

fn campaign_recipe(root: &Root, original: &Path, generation: u64, maximum: usize,
    expected: FileCampaignRequirement) -> PathBuf
{
    let path = root.0.join(format!("campaign-{generation}-{maximum}-{}-{}-{}-{}",
        expected.limits.cases, expected.limits.input_bytes, expected.max_campaigns,
        original.file_name().unwrap().to_str().unwrap()));
    fs::write(&path, wrapped(&fs::read_to_string(original).unwrap(),
        generation, maximum, expected)).unwrap();
    path
}

fn floor(host: &FileOversight) -> FileRecoveryFloor {
    let before = host.inspect();
    FileRecoveryFloor { journal_revision: before.revision,
        control_sequence: before.control.sequence, authority_epoch: before.control.ledger.epoch }
}

// The corpus is a real receipt-confirmed learned publication with both original
// helper judgments and human approval. No hand-picked replay archive is supplied.
fn confirmed(root: &Root) -> (Config, PathBuf, Loaded, FileSupervisedDriver, FilePolicyGovernor) {
    let mut config = configured(root);
    let path = native::write(root);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (mut host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let expected = guard();
    let governor = host.enable_policy_campaigns(host.revision(),
        expected.limits, expected.max_campaigns).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut config, &loaded,
        &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, loaded.request));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    drop(history);
    drop(wire);
    (config, path, loaded, driver, governor)
}

#[test]
fn a_promoted_campaign_reopens_with_its_exact_guards_and_continues_original_numerics() {
    let root = Root::new();
    let (mut config, original, _, mut driver, governor) = confirmed(&root);
    let expected = policy(2, 256);
    let (view, numerical, minimum, archive, old_key) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let before = host.inspect();
        let update = PolicyUpdate::new(901, before.control.sequence,
            before.control.ledger.epoch, expected.clone()).unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        assert!(review.report().cases().iter().any(|case|
            matches!(case.id(), ReplayCaseId::Proposal(_))));
        assert!(review.report().cases().iter().any(|case|
            matches!(case.id(), ReplayCaseId::Review { .. })));
        assert!(!review.report().requires_shadow());
        assert!(review.report().newly_reviewable().is_empty());
        let revision = host.revision();
        let key = governor.approve(&mut host, revision, &review, false).unwrap();
        let revision = host.revision();
        let receipt = host.promote_policy_campaign(revision, &key).unwrap();
        assert_eq!(receipt.update().policy(), &expected);
        assert_eq!(host.policy_campaign(901).unwrap().observed_disposition(),
            CampaignDisposition::Promoted);
        assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
        let archive = root.0.join("promoted.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
        (host.inspect(), host.learned_generation_inspection().unwrap().numerical,
            floor(&host), archive, key)
    };
    assert_eq!(cleanup(driver, 1000, 1), 0);
    // The real fixture consumed the two-token prompt and sampled A/stop. Its
    // continuation therefore advances a sampled position, not another prefill.
    assert_eq!(numerical.position, 4);
    assert_eq!(numerical.sampled_draws, 2);
    let archive_bytes = fs::read(&archive).unwrap();
    let selected_path = campaign_recipe(&root, &original, 2, 256, guard());
    let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
    assert_eq!(loaded.expected_policy, expected);
    assert_eq!(loaded.expected_campaigns, Some(guard()));
    bind_anchor(&root, &config, &mut loaded, &archive);
    loaded.floor = minimum;
    let (mut host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    assert_eq!(host.current_policy().unwrap(), &expected);
    assert!(host.policy_campaigns_required());
    assert_eq!(host.revision(), view.revision + 1);
    assert_eq!(host.policy_campaign(901).unwrap().observed_disposition(),
        CampaignDisposition::Promoted);
    assert_eq!(host.inspect().executions, view.executions);
    assert_eq!(host.inspect().control.ledger.charged, view.control.ledger.charged);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    assert_eq!(host.promote_policy_campaign(host.revision(), &old_key),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);

    let continuation = host.prepare_learned_text_stream_continuation(1, 2).unwrap();
    fresh(&mut host, &mut config, 1001);
    host.resume_learned_generation(host.revision(), numerical.actor_revision,
        numerical.position).unwrap();
    host.begin_learned_text_stream_continuation(host.revision(), continuation).unwrap();
    host.advance_learned_generation(host.revision(), numerical.actor_revision,
        numerical.position).unwrap().unwrap();
    let after = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(after.position, numerical.position + 1);
    assert_eq!(after.sampled_draws, numerical.sampled_draws + 1);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, view.control.ledger.charged);
    assert_eq!(host.current_policy().unwrap(), &expected);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(fs::read(&archive).unwrap(), archive_bytes);
}

#[test]
fn every_campaign_guard_field_and_absence_are_exact_before_staging_cleanup() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let mut config = configured(&root);
        let path = native::write(&root);
        if let Some(owned) = mode {
            super::super::forecast::install(&root, &path, owned, 60_000);
        }
        evidence(&root, &config, 1, true, false);
        let original = recipe::load(&path, &config, false).unwrap();
        let (mut host, _, _, _) = recovery::create_selected(&config, &original).unwrap();
        let expected = guard();
        let _governor = host.enable_policy_campaigns(host.revision(),
            expected.limits, expected.max_campaigns).unwrap();
        fresh(&mut host, &mut config, 1000);
        let before = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(before.position, 0);
        assert_eq!(before.sampled_draws, 0);
        host.advance_learned_generation(host.revision(), before.actor_revision,
            before.position).unwrap().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 1);
        assert_eq!(numerical.sampled_draws, 0);
        let view = host.inspect();
        let minimum = floor(&host);
        let archive = root.0.join("campaign.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"retain unmatched campaign staging").unwrap();
        drop(host);
        let mut mismatches = vec![path.clone(), selected(&root, &path, 1, 512)];
        for changed in [
            FileCampaignRequirement { limits: ReplayLimits { cases: 17, ..expected.limits }, ..expected },
            FileCampaignRequirement { limits: ReplayLimits { input_bytes: 1_048_577, ..expected.limits }, ..expected },
            FileCampaignRequirement { max_campaigns: 9, ..expected },
        ] {
            mismatches.push(campaign_recipe(&root, &path, 1, 512, changed));
        }
        for mismatch in mismatches {
            let mut wrong = recipe::load(&mismatch, &config, false).unwrap();
            bind_anchor(&root, &config, &mut wrong, &archive);
            wrong.floor = minimum;
            let error = recovery::open_selected(&config, &wrong).err().unwrap();
            assert!(error.contains("Binding"), "{error}");
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
            assert_eq!(fs::read(&pending).unwrap(), b"retain unmatched campaign staging");
        }
        let selected_path = campaign_recipe(&root, &path, 1, 512, expected);
        let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
        assert_eq!(loaded.generation, original.generation);
        bind_anchor(&root, &config, &mut loaded, &archive);
        loaded.floor = minimum;
        let (mut host, _, observer, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert!(host.policy_campaigns_required());
        assert_eq!(observer.is_some(), mode.is_some());
        assert_eq!(host.revision(), view.revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(!pending.exists());
        fresh(&mut host, &mut config, 1001);
        host.resume_learned_generation(host.revision(), numerical.actor_revision,
            numerical.position).unwrap();
        // Finish the original xy prefill. A raw/owned forecast is required
        // before the following output sample, so this grants no forecast waiver.
        host.advance_learned_generation(host.revision(), numerical.actor_revision,
            numerical.position).unwrap().unwrap();
        let after = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(after.position, numerical.position + 1);
        assert_eq!(after.sampled_draws, numerical.sampled_draws);
    }
}

#[test]
fn a_selected_campaign_guard_cannot_upgrade_an_unguarded_history() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    let mut original = recipe::load(&path, &config, false).unwrap();
    let (host, _, _, _) = recovery::create_selected(&config, &original).unwrap();
    let archive = root.0.join("unguarded.anchor");
    anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"unselected original inventory").unwrap();
    drop(host);
    let selected_path = campaign_recipe(&root, &path, 1, 512, guard());
    let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
    bind_anchor(&root, &config, &mut loaded, &archive);
    let error = recovery::open_selected(&config, &loaded).err().unwrap();
    assert!(error.contains("Binding"), "{error}");
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"unselected original inventory");
    bind_anchor(&root, &config, &mut original, &archive);
    let (host, _, _, _) = recovery::open_selected(&config, &original).unwrap();
    assert!(!host.policy_campaigns_required());
    assert_eq!(host.current_policy().unwrap(), &config.profile.delivery.policy);
}

#[test]
fn recovery_revokes_pending_approval_and_never_reissues_the_old_governor_or_key() {
    let root = Root::new();
    let (config, original, _, mut driver, governor) = confirmed(&root);
    let (review, key, view, minimum, archive) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let control = host.inspect().control;
        let update = PolicyUpdate::new(902, control.sequence, control.ledger.epoch,
            policy(2, 256)).unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        let revision = host.revision();
        let key = governor.approve(&mut host, revision, &review, false).unwrap();
        assert_eq!(host.policy_campaign(902).unwrap().observed_disposition(),
            CampaignDisposition::Approved);
        let archive = root.0.join("approved.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
        (review, key, host.inspect(), floor(&host), archive)
    };
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let selected_path = campaign_recipe(&root, &original, 1, 512, guard());
    let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
    bind_anchor(&root, &config, &mut loaded, &archive);
    loaded.floor = minimum;
    let (mut host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    assert_eq!(host.policy_campaign(902).unwrap().observed_disposition(),
        CampaignDisposition::Revoked);
    assert!(host.policy_campaigns_required());
    assert_eq!(host.current_policy().unwrap(), &config.profile.delivery.policy);
    assert_eq!(host.inspect().executions, view.executions);
    assert_eq!(host.inspect().control.ledger.charged, view.control.ledger.charged);
    let revision = host.revision();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    assert_eq!(host.promote_policy_campaign(revision, &key),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(governor.approve(&mut host, revision, &review, false).err(),
        Some(JournalError::Contract(Error::Binding)));
    let control = host.inspect().control;
    let bypass = PolicyUpdate::new(903, control.sequence, control.ledger.epoch,
        policy(2, 256)).unwrap();
    assert_eq!(host.replace_policy(revision, &bypass),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), revision);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
}

#[test]
fn recovery_campaigns_refuse_all_new_owners_before_source_or_journal_creation() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let config = configured(&root);
        let path = native::write(&root);
        if let Some(owned) = mode {
            super::super::forecast::install(&root, &path, owned, 60_000);
        }
        let selected_path = campaign_recipe(&root, &path, 1, 512, guard());
        let loaded = recipe::load(&selected_path, &config, false).unwrap();
        let error = recovery::create_selected(&config, &loaded).err().unwrap();
        assert!(error.contains("campaign guard"), "{error}");
        if mode.is_none() {
            assert!(recovery::create(&config, &loaded).err().unwrap().contains("campaign guard"));
        }
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        let original = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _) = recovery::create_selected(&config, &original).unwrap();
        assert!(!host.policy_campaigns_required());
    }
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, path, mut original) = super::super::mediated_topology::selected(&root, mode, false);
        let selected_path = campaign_recipe(&root, &path, 1, 512, guard());
        let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
        loaded.topology = original.topology.take();
        loaded.anchor_output = original.anchor_output.take();
        fs::remove_file(root.0.join("live-1.json")).unwrap();
        fs::remove_file(root.0.join("inventory-1.json")).unwrap();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        let error = topology::start(&config, &loaded, false, &mut control, &deadline(),
            &mut || panic!("campaign creation refusal precedes source admission and clock"))
            .err().unwrap();
        assert!(error.contains("campaign guard"), "{error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("source-1.bin").exists());
        assert!(!root.0.join("history-1.bin").exists());
    }
}

#[test]
fn resident_successors_cannot_drop_or_change_the_independent_campaign_inventory() {
    let expected = guard();
    for next in [
        None,
        Some(FileCampaignRequirement { limits: ReplayLimits { cases: 17, ..expected.limits }, ..expected }),
        Some(FileCampaignRequirement { limits: ReplayLimits { input_bytes: 1_048_577, ..expected.limits }, ..expected }),
        Some(FileCampaignRequirement { max_campaigns: 9, ..expected }),
    ] {
        let root = Root::new();
        let config = configured(&root);
        let original = native::write(&root);
        let first_path = campaign_recipe(&root, &original, 1, 512, expected);
        let first = recipe::load(&first_path, &config, true).unwrap();
        let (path, _) = native::next(&root, &config, 2);
        let second_path = next.map_or(path.clone(), |next|
            campaign_recipe(&root, &path, 1, 512, next));
        let second = recipe::load(&second_path, &config, true).unwrap();
        assert_eq!(first.generation, second.generation);
        assert_eq!(first.expected_policy, second.expected_policy);
        let entries = [first, second].into_iter().map(|loaded| {
            let mut actor = super::super::peer_intake::actor_profile(&root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
            resident::Entry { loaded, actor, precheck: None, release: LearnedTextRelease::Message }
        }).collect();
        let peers = fixture::peers(&root, &config);
        let store = config.store.clone();
        let error = resident::run(config, entries, &peers, true, None, None,
            || panic!("resident campaign mismatch precedes time and owner admission"))
            .err().unwrap();
        assert!(error.contains("campaign guard"), "{error}");
        assert!(!store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn campaign_envelopes_admit_native_bounds_and_reject_malformed_inventory_before_assets() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    let original = fs::read_to_string(&path).unwrap();
    let selected_path = root.0.join("campaign-schema.json");
    for expected in [guard(), FileCampaignRequirement {
        limits: ReplayLimits { cases: MAX_REPLAY_CASES, input_bytes: MAX_REPLAY_INPUT_BYTES },
        max_campaigns: MAX_POLICY_CAMPAIGNS,
    }] {
        fs::write(&selected_path, wrapped(&original, 1, 512, expected)).unwrap();
        let loaded = recipe::load(&selected_path, &config, false).unwrap();
        assert_eq!(loaded.expected_campaigns, Some(expected));
        assert_eq!(loaded.expected_policy, config.profile.delivery.policy);
        assert!(!config.store.exists());
    }
    let valid = wrapped(&original, 1, 512, guard());
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    let mut invalid = vec![
        valid.replacen("fa.learned-recovery-policy/2", "fa.learned-recovery-policy/1", 1),
        valid.replacen("fa.learned-recovery-policy/2", "fa.learned-recovery-policy/3", 1),
        valid.replacen(&format!("\"campaigns\":{},", campaign_json(guard())), "", 1),
        valid.replacen(&campaign_json(guard()), "null", 1),
        valid.replacen(&campaign_json(guard()), "[]", 1),
        valid.replacen("\"campaigns\":", "\"unexpected_campaigns\":", 1),
        valid.replacen("\"cases\":16", "\"cases\":16,\"cases\":16", 1),
        valid.replacen("\"cases\":16", "\"skip_guard\":true,\"cases\":16", 1),
        valid.replacen("\"max_campaigns\":8", "\"maximum\":8", 1),
        wrapped(&valid, 1, 512, guard()),
        envelope(&valid, 1, 512),
        wrapped(&envelope(&original, 1, 512), 1, 512, guard()),
        format!(r#"{{"schema":"fa.learned-credential-publication/1","credential":"/missing/profile","recipe":{valid}}}"#),
    ];
    for (field, value, upper) in [
        ("cases", 16, MAX_REPLAY_CASES),
        ("input_bytes", 1_048_576, MAX_REPLAY_INPUT_BYTES),
        ("max_campaigns", 8, MAX_POLICY_CAMPAIGNS),
    ] {
        for bad in ["0".to_owned(), "-1".into(), "1.5".into(), "true".into(),
            "\"16\"".into(), (upper + 1).to_string(), "18446744073709551616".into()] {
            invalid.push(valid.replacen(&format!("\"{field}\":{value}"),
                &format!("\"{field}\":{bad}"), 1));
        }
    }
    for document in invalid {
        fs::write(&selected_path, document).unwrap();
        let error = recipe::load(&selected_path, &config, false).err().unwrap();
        assert!(!error.contains("No such file"), "invalid campaign guard reached assets: {error}");
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
