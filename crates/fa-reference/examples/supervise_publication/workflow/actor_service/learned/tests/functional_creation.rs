//! Real fresh learned ownership, native reviews, human review and exact recovery
//! for an independently declared functional profile. Metadata is not a model.
use super::*;
use fa_reference::action::consequence::delivery::{
    persistent::observed::{
        decoder::learned::sidecar::FileLearnedSidecarFinish,
        reviewer::wire::ReviewDecision,
    },
    stream::ReleaseFrame,
};
use fa_reference::action::consequence::gate::containment::RestartGrade;
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use std::{os::unix::net::UnixStream, path::{Path, PathBuf}};

fn config_file(root: &Root, grade: &str) -> PathBuf {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"grade\": \"audit_only\"", &format!("\"grade\": \"{grade}\""))
        .replace("\"events\": 256", "\"events\": 1024")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000")
        .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
        .replace("\"cleanup_ms\": 2000", "\"cleanup_ms\": 1000");
    let path = root.0.join("creation-config.json");
    fs::write(&path, text).unwrap();
    path
}

fn peers_file(root: &Root, config: &Config) -> (PathBuf, PeerProfile) {
    let (socket, _client) = UnixStream::pair().unwrap();
    let p = PeerCredentials::observe(&socket).unwrap();
    let s = config.profile.delivery.scope;
    let text = format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority,
        config.profile.human.reviewer_id, root.0.display(),
        p.uid(), p.gid(), p.pid(), p.uid(), p.gid(), p.pid(),
    );
    let path = root.0.join("reviewer.json");
    fs::write(&path, text).unwrap();
    let peers = PeerProfile::read(&path).unwrap();
    (path, peers)
}

#[test]
fn fresh_functional_cli_uses_actual_native_generation_and_both_publication_keys() {
    let root = Root::new();
    let config_path = config_file(&root, "functional_restart");
    assert!(Config::read(&config_path).is_err(), "ordinary operators retain audit-only admission");
    let config = Config::read_learned_generation(&config_path).unwrap();
    assert_eq!(config.profile.delivery.actor.profile().grade, RestartGrade::FunctionalRestart);
    let original_metadata = config.profile.delivery.actor.clone();
    let recipe_path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let (peer_path, peers) = peers_file(&root, &config);
    let anchor_path = root.0.join("created.anchor");
    let args = vec!["create-learned-generated".into(),
        config_path.display().to_string(), recipe_path.display().to_string(),
        peer_path.display().to_string(), "--retain-history-anchor".into(),
        anchor_path.display().to_string()];
    let mut human = None;
    let result = super::super::command_with_clock(&args, None, || {
        if human.is_none() && peers.socket(1).exists() {
            human = Some(super::peer_intake::human(peers.clone(), 1, ReviewDecision::Approve));
        }
        ElapsedTick(1000)
    });
    let packet = human.expect("actual learned review reached its independent human endpoint")
        .join().unwrap();
    result.unwrap();
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    assert_eq!(packet.views().len(), 2);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);

    let mut loaded = recipe::load(&recipe_path, &config, false).unwrap();
    let published = FileOversight::read_publication_with_learned_generation(
        &config.store, &config.profile, &loaded.generation).unwrap();
    assert_eq!(published.payload, b"A");
    assert_eq!(published.executions, 1);
    assert_eq!(published.control.ledger.charged, packet.action().spec().units);
    loaded.recovery_anchor = Some(super::super::anchors::load(&anchor_path).unwrap());
    let retained = fs::read(&anchor_path).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let actor = host.actor_snapshot().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(actor.state.profile(), original_metadata.profile());
    assert_eq!(actor.state.profile().grade, RestartGrade::FunctionalRestart);
    assert!(actor.state.next_position() > 0);
    assert_eq!(actor.state.next_position(), numerical.position);
    assert_eq!(actor.state.cache().len(), numerical.cache_bytes);
    assert_eq!(actor.state.sampler().len(), numerical.sampler_bytes);
    assert_ne!(actor.state.cache(), original_metadata.cache());
    assert_ne!(actor.state.sampler(), original_metadata.sampler());
    assert_eq!(numerical.sampled_draws, 2);
    assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
    let native = host.learned_sidecar_outcome(1).unwrap();
    let FileLearnedSidecarFinish::Applied { receipt, archive, .. } = native.result else {
        panic!("new functional owner did not use its real native review");
    };
    assert!(receipt.is_ok());
    assert_eq!(archive.inputs.action(), packet.action());
    assert_eq!(archive.commit_times.len(), 2);
    assert_eq!(archive.reveal_times.len(), 2);
    drop(host);

    // A weaker declaration cannot reopen the acknowledged functional owner or
    // replace its retained prefix. Neither read creates a substitute history.
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let audit_path = config_file(&root, "audit_only");
    let audit = Config::read(&audit_path).unwrap();
    let mut wrong = recipe::load(&recipe_path, &audit, false).unwrap();
    wrong.recovery_anchor = loaded.recovery_anchor.clone();
    assert!(recovery::open_selected(&audit, &wrong).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    assert_eq!(fs::read(&anchor_path).unwrap(), retained);
}

#[test]
fn creation_never_infers_or_admits_a_stronger_restart_grade() {
    for grade in ["bit_exact", "decision_replay", "functional", "unknown"] {
        let root = Root::new();
        let path = config_file(&root, grade);
        assert!(Config::read_learned_generation(&path).is_err());
        let args = vec!["create-learned-generated".into(), path.display().to_string(),
            root.0.join("missing-recipe.json").display().to_string(),
            root.0.join("missing-reviewer.json").display().to_string()];
        assert!(super::super::command_with_clock(&args, None, || ElapsedTick(1000)).is_err());
        assert!(!root.0.join("store").exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
    let root = Root::new();
    let path = config_file(&root, "audit_only");
    let legacy = Config::read(&path).unwrap();
    let learned = Config::read_learned_generation(&path).unwrap();
    crate::config::assert_oversight_profiles_eq(&learned.profile, &legacy.profile);
    assert_eq!(learned.profile.delivery.actor.profile().grade, RestartGrade::AuditOnly);
}

#[test]
fn functional_metadata_cannot_import_an_advanced_actor_into_a_new_native_owner() {
    let root = Root::new();
    let path = config_file(&root, "functional_restart");
    let text = fs::read_to_string(&path).unwrap()
        .replace("\"tokens\": []", "\"tokens\": [120]")
        .replace("\"next_position\": 0", "\"next_position\": 1");
    fs::write(&path, text).unwrap();
    let config = Config::read_learned_generation(&path).unwrap();
    let recipe_path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let (peer_path, _) = peers_file(&root, &config);
    let args = vec!["create-learned-generated".into(), path.display().to_string(),
        recipe_path.display().to_string(), peer_path.display().to_string()];
    assert!(super::super::command_with_clock(&args, None, || ElapsedTick(1000)).is_err());
    assert!(!config.store.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    assert!(Path::new(&args[2]).is_file());
}
