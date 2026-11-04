//! Explicit recovery policy selection through real learned journal transitions.
//! Tiny model fixtures exercise custody and exact binding, not policy quality,
//! independent operator authentication or deployment effectiveness.
use super::*;
use super::continuation as native;
use super::super::{anchors, sequence as resident, topology};
use fa_reference::action::consequence::{
    delivery::persistent::{
        governance::PolicyUpdate,
        observed::guarded::FileRecoveryFloor,
    },
    gate::containment::session::policy::{Policy, Predicate},
    policy_campaign::ReplayLimits,
};
use std::path::{Path, PathBuf};

fn policy(generation: u64, payload_limit: usize) -> Policy {
    Policy::new(generation, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(payload_limit),
        Predicate::UnitsAtMost(512),
        Predicate::All(vec![0, 1, 2]),
    ]).unwrap()
}

fn policy_json(generation: u64, payload_limit: usize) -> String {
    format!(r#"{{"generation":{generation},"nodes":[
        {{"op":"exact_value","key":7,"value_hex":"6f6b"}},
        {{"op":"payload_at_most","maximum":{payload_limit}}},
        {{"op":"units_at_most","maximum":512}},
        {{"op":"all","children":[0,1,2]}}]}}"#)
}

fn envelope(inner: &str, generation: u64, payload_limit: usize) -> String {
    format!(r#"{{"schema":"fa.learned-recovery-policy/1","effective_policy":{},"recipe":{inner}}}"#,
        policy_json(generation, payload_limit))
}

fn selected(root: &Root, original: &Path, generation: u64, payload_limit: usize) -> PathBuf {
    let path = root.0.join(format!("policy-{generation}-{payload_limit}.json"));
    fs::write(&path, envelope(&fs::read_to_string(original).unwrap(), generation, payload_limit)).unwrap();
    path
}

fn bind_anchor(root: &Root, config: &Config, loaded: &mut Loaded, archive: &Path) {
    let args = vec![
        "create-learned-generated".into(),
        root.0.join("config.json").display().to_string(),
        root.0.join("recipe.json").display().to_string(),
        "--resume".into(), "--history-anchor".into(), archive.display().to_string(),
    ];
    let (_, options) = anchors::take_options(&args).unwrap();
    options.admit(config, loaded, true).unwrap();
}

fn fresh(host: &mut FileOversight, config: &mut Config, tick: u64) {
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(tick)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
}

fn replace(host: &mut FileOversight, expected: &Policy) {
    let before = host.inspect();
    let update = PolicyUpdate::new(801, before.control.sequence,
        before.control.ledger.epoch, expected.clone()).unwrap();
    let receipt = host.replace_policy(host.revision(), &update).unwrap();
    assert_eq!(receipt.update().policy(), expected);
    assert_eq!(host.current_policy().unwrap(), expected);
    assert!(host.inspect().control.sequence > before.control.sequence);
}

#[test]
fn native_policy_replacement_recovers_exactly_and_continues_original_numerics() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let mut config = configured(&root);
        let path = native::write(&root);
        if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
        evidence(&root, &config, 1, true, false);
        let mut legacy = recipe::load(&path, &config, false).unwrap();
        assert_eq!(legacy.expected_policy, config.profile.delivery.policy);
        let (mut host, _old_human, _old_forecast, _old_identity) =
            recovery::create_selected(&config, &legacy).unwrap();
        fresh(&mut host, &mut config, 1000);
        let before = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), before.actor_revision,
            before.position).unwrap().unwrap();
        assert!(host.learned_generation_inspection().unwrap().numerical.position > before.position);
        // Select and retain the exact new policy while the original owner is
        // healthy. Recovery never reads the journal to manufacture its expectation.
        let expected = policy(2, 256);
        let selected_path = selected(&root, &path, 2, 256);
        replace(&mut host, &expected);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let view = host.inspect();
        let floor = FileRecoveryFloor { journal_revision: view.revision,
            control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch };
        let archive = root.0.join("policy-history.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
        let archive_bytes = fs::read(&archive).unwrap();
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"preserve until exact policy verification").unwrap();
        drop(host);

        bind_anchor(&root, &config, &mut legacy, &archive);
        legacy.floor = floor;
        let failure = recovery::open_selected(&config, &legacy).err().unwrap();
        assert!(failure.contains("Binding"), "{failure}");
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&pending).unwrap(), b"preserve until exact policy verification");

        // Full predicate equality matters independently of the generation.
        // A future generation is no more acceptable than a stale bootstrap.
        for (generation, maximum) in [(2, 257), (3, 256)] {
            let wrong_path = selected(&root, &path, generation, maximum);
            let mut wrong = recipe::load(&wrong_path, &config, false).unwrap();
            bind_anchor(&root, &config, &mut wrong, &archive);
            wrong.floor = floor;
            let failure = recovery::open_selected(&config, &wrong).err().unwrap();
            assert!(failure.contains("Binding"), "{failure}");
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
            assert_eq!(fs::read(&pending).unwrap(), b"preserve until exact policy verification");
        }

        let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
        assert_eq!(loaded.generation, legacy.generation);
        assert_eq!(loaded.expected_policy, expected);
        bind_anchor(&root, &config, &mut loaded, &archive);
        loaded.floor = floor;
        let (mut host, _human, forecast, _identity) =
            recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.current_policy().unwrap(), &expected);
        assert_eq!(host.revision(), view.revision + 1);
        assert_eq!(host.inspect().executions, view.executions);
        assert_eq!(host.inspect().control.ledger.charged, view.control.ledger.charged);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready());
        assert_eq!(forecast.is_some(), mode.is_some());
        assert!(!pending.exists());

        fresh(&mut host, &mut config, 1001);
        host.resume_learned_generation(host.revision(), numerical.actor_revision,
            numerical.position).unwrap();
        host.advance_learned_generation(host.revision(), numerical.actor_revision,
            numerical.position).unwrap().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position,
            numerical.position + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws,
            numerical.sampled_draws);
        assert_eq!(host.current_policy().unwrap(), &expected);
        assert_eq!(fs::read(&archive).unwrap(), archive_bytes);
    }
}

#[test]
fn recovery_selection_cannot_bootstrap_a_different_policy_in_any_creator() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let config = configured(&root);
        let path = native::write(&root);
        if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
        let selected_path = selected(&root, &path, 2, 256);
        let loaded = recipe::load(&selected_path, &config, false).unwrap();
        let error = recovery::create_selected(&config, &loaded).err().unwrap();
        assert!(error.contains("bootstrap policy"), "{error}");
        if mode.is_none() {
            assert!(recovery::create(&config, &loaded).err().unwrap().contains("bootstrap policy"));
        }
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);

        let bootstrap = selected(&root, &path, 1, 512);
        let loaded = recipe::load(&bootstrap, &config, false).unwrap();
        assert_eq!(loaded.expected_policy, config.profile.delivery.policy);
        let (host, _human, _forecast, _identity) =
            recovery::create_selected(&config, &loaded).unwrap();
        assert_eq!(host.current_policy().unwrap(), &config.profile.delivery.policy);
    }

    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, path, mut original) = super::mediated_topology::selected(&root, mode, false);
        let selected_path = selected(&root, &path, 2, 256);
        let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
        loaded.topology = original.topology.take();
        loaded.anchor_output = original.anchor_output.take();
        fs::remove_file(root.0.join("live-1.json")).unwrap();
        fs::remove_file(root.0.join("inventory-1.json")).unwrap();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        let error = topology::start(&config, &loaded, false, &mut control, &deadline(),
            &mut || panic!("policy creation mismatch precedes source admission and clock"))
            .err().unwrap();
        assert!(error.contains("bootstrap policy"), "{error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("source-1.bin").exists());
        assert!(!root.0.join("history-1.bin").exists());
    }
}

#[test]
fn expected_policy_does_not_discover_or_admit_an_unselected_campaign_guard() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    let original = recipe::load(&path, &config, false).unwrap();
    let (mut host, _human, _forecast, _identity) =
        recovery::create_selected(&config, &original).unwrap();
    let expected = policy(2, 256);
    replace(&mut host, &expected);
    let _governor = host.enable_policy_campaigns(host.revision(),
        ReplayLimits { cases: 4, input_bytes: 1_048_576 }, 4).unwrap();
    let archive = root.0.join("campaign-history.anchor");
    anchors::export_acknowledged(&host.history_anchor().unwrap(), &archive).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"unselected campaign custody").unwrap();
    drop(host);

    let selected_path = selected(&root, &path, 2, 256);
    let mut loaded = recipe::load(&selected_path, &config, false).unwrap();
    bind_anchor(&root, &config, &mut loaded, &archive);
    assert_eq!(loaded.expected_policy, expected);
    assert!(recovery::open_selected(&config, &loaded).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"unselected campaign custody");
}

#[test]
fn a_resident_successor_cannot_select_another_effective_policy() {
    let root = Root::new();
    let config = configured(&root);
    let first_path = native::write(&root);
    let first = recipe::load(&first_path, &config, true).unwrap();
    let (second_path, _second) = native::next(&root, &config, 2);
    let changed = selected(&root, &second_path, 2, 256);
    let second = recipe::load(&changed, &config, true).unwrap();
    assert_eq!(first.generation, second.generation);
    let entries = [first, second].into_iter().map(|loaded| {
        let mut actor = super::peer_intake::actor_profile(&root, &config, loaded.request);
        actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
        resident::Entry { loaded, actor, release: LearnedTextRelease::Message }
    }).collect();
    let peers = fixture::peers(&root, &config);
    let store = config.store.clone();
    let error = resident::run(config, entries, &peers, false, None, None,
        || panic!("resident policy mismatch precedes time and owner creation")).err().unwrap();
    assert!(error.contains("effective policy"), "{error}");
    assert!(!store.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn the_policy_envelope_preserves_legacy_versions_and_composed_numerical_contracts() {
    for (version, mode) in [(1, None), (2, None), (3, None), (4, None),
        (5, Some(false)), (5, Some(true)), (6, None)] {
        let root = Root::new();
        let config = configured(&root);
        let path = if matches!(version, 1 | 6) {
            fixture::write(&root, b"allow")
        } else { native::write(&root) };
        if matches!(version, 2 | 3) {
            fs::write(&path, fs::read_to_string(&path).unwrap().replacen(
                "fa.learned-publication/4", &format!("fa.learned-publication/{version}"), 1)).unwrap();
        }
        if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
        if version == 6 {
            fs::write(&path, super::processes::process_recipe(&root, "allow")).unwrap();
        }
        let original = recipe::load(&path, &config, false).unwrap();
        let wrapped = selected(&root, &path, 2, 256);
        let loaded = recipe::load(&wrapped, &config, false).unwrap();
        assert_eq!(loaded.generation, original.generation);
        assert_eq!(loaded.expected_policy, policy(2, 256));
        assert_eq!(original.expected_policy, config.profile.delivery.policy);
        assert_eq!(loaded.request, original.request);
        assert_eq!(loaded.rounds, original.rounds);
        assert_eq!(loaded.process.is_some(), original.process.is_some());
        assert!(!config.store.exists());
    }

    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    super::identity_tests::provision(&root, &path, true);
    fs::write(root.0.join("publication.json"), format!(r#"{{
        "schema":"fa.supervised-witnesses/1","source":91,
        "original":"{0}/missing-original.bin","current":"{0}/missing-current.bin",
        "limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},
        "requests":[{{"kind":"absent_key","key":1}}]}}"#, root.0.display())).unwrap();
    fs::write(root.0.join("credential.json"), format!(r#"{{
        "schema":"fa.learned-file-credential/1",
        "policy":{{"family":"publication","route":"adapter:file-oversight",
            "credential":"broker-token","profile_generation":1}},
        "epoch":{{"generation":1,"revoked":false}},
        "inventory":"{0}/missing-inventory.json",
        "broker_credential":"{0}/missing-broker.bin",
        "provider_credential":"{0}/missing-provider.bin"}}"#, root.0.display())).unwrap();
    let inner = fs::read_to_string(&path).unwrap();
    let composed = format!(r#"{{"schema":"fa.learned-credential-publication/1",
        "credential":"{0}/credential.json","recipe":{{
            "schema":"fa.learned-checked-publication/1",
            "publication":"{0}/publication.json","recipe":{inner}}}}}"#, root.0.display());
    fs::write(&path, &composed).unwrap();
    let original = recipe::load(&path, &config, false).unwrap();
    let wrapped = selected(&root, &path, 2, 256);
    let loaded = recipe::load(&wrapped, &config, false).unwrap();
    assert_eq!(loaded.generation, original.generation);
    assert_eq!(loaded.expected_policy, policy(2, 256));
    assert!(loaded.credential.is_some());
    assert!(loaded.publication.is_some());
    assert!(loaded.identity.is_some());
    assert!(!config.store.exists());
}

#[test]
fn malformed_or_reordered_policy_envelopes_refuse_before_model_or_store_access() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    let original = fs::read_to_string(&path).unwrap();
    let valid = envelope(&original, 2, 256);
    let invalid_path = root.0.join("invalid-policy-recipe.json");
    fs::write(&invalid_path, &valid).unwrap();
    assert_eq!(recipe::load(&invalid_path, &config, false).unwrap().expected_policy,
        policy(2, 256));
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    let bad = [
        envelope(&valid, 2, 256),
        format!(r#"{{"schema":"fa.learned-credential-publication/1",
            "credential":"/missing/profile","recipe":{valid}}}"#),
        valid.replacen("fa.learned-recovery-policy/1", "fa.learned-recovery-policy/2", 1),
        valid.replacen("\"effective_policy\":", "\"skip_guard\":true,\"effective_policy\":", 1),
        valid.replacen("\"effective_policy\":", "\"effective_policy\":null,\"effective_policy\":", 1),
        valid.replacen("\"generation\":2", "\"generation\":0", 1),
        valid.replacen("\"children\":[0,1,2]", "\"children\":[0,1,99]", 1),
        valid.replacen("\"effective_policy\":", "\"unexpected_policy\":", 1),
        original.replacen("\"schema\":", &format!("\"effective_policy\":{},\"schema\":",
            policy_json(2, 256)), 1),
        format!("{}{valid}", " ".repeat(65_536)),
    ];
    for document in bad {
        fs::write(&invalid_path, document).unwrap();
        let error = recipe::load(&invalid_path, &config, false).err().unwrap();
        assert!(!error.contains("No such file"), "invalid policy reached model intake: {error}");
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
