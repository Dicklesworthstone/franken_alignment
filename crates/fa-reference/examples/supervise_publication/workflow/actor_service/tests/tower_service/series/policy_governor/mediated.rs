//! Separately authenticated governance on the SAME mediated owner and source.
//! Real sockets and native effects establish enforcement; helper votes, custody
//! documents and explicit human/governor decisions remain controlled fixtures.
use super::*;
use crate::publication::PublicationProfile;
use crate::recover_stop::guarded::SubmitProfile;
use crate::workflow::actor_service::topology::recovery as topology_recovery;
use crate::workflow::continuation::{
    creation::topology::Selection as CreationSelection,
    guarded::mediated::profile::Selection as RecoverySelection,
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
};
use std::io::Cursor;

#[path = "mediated/support.rs"]
mod support;
#[path = "mediated/preflight.rs"]
mod preflight;
use support::*;

// The existing unmediated two-epoch regression uses the same independently
// provisioned worker fixture rather than silently accepting an old binding.
pub(super) fn governed_config(root: &Directory) -> Config {
    write_complete(&root.0.join("helper-epoch"), b"0");
    mediated_config(root)
}

#[test]
fn governed_mediated_creation_promotes_then_publishes_with_fresh_plain_and_checked_reviews() {
    for checked in [false, true] {
        governed_pair(checked, false, false);
    }
}

#[test]
fn governed_mediated_recovery_preserves_original_custody_and_publishes_after_promotion() {
    for checked in [false, true] {
        governed_pair(checked, true, false);
    }
}

#[test]
fn promotion_cannot_bypass_the_original_final_source_check_in_created_or_recovered_services() {
    for recovered in [false, true] {
        for checked in [false, true] {
            governed_pair(checked, recovered, true);
        }
    }
}

fn governed_pair(checked: bool, recovered: bool, lose_source: bool) {
    let root = Directory::new();
    let (config, recovery, before_source) = if recovered {
        let (config, recovery, saved) = seed(&root, checked, false);
        (config, Some(recovery), Some(saved))
    } else {
        let config = governed_config(&root);
        evidence(&root, &config);
        (config, None, None)
    };
    let creation = (!recovered).then(|| creation_selection(&root));
    let continuation = before_source.as_ref().map(|saved| next_selection(&root, saved));
    let graph_generation = if recovered { 2 } else { 1 };
    let before_epoch = if recovered { 3 } else { 0 };
    let promoted_epoch = before_epoch + 1;
    let topology_path = root.0.join(format!("topology-{graph_generation}.json"));
    let inventory_path = root.0.join(format!("inventory-{graph_generation}.json"));
    let original_topology = fs::read(&topology_path).unwrap();
    let original_inventory = fs::read(&inventory_path).unwrap();
    let publication = checked.then(|| {
        if !recovered {
            let first = checked::capture(&config, 1, 0, b"first");
            write_complete(&root.0.join("original.bin"), &first);
            write_complete(&root.0.join("current.bin"), &first);
        }
        PublicationProfile::decode(checked::profile_document(&root).as_bytes()).unwrap()
    });
    let second_capture = checked.then(|| checked::capture(&config, 2, promoted_epoch, b"second"));
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let operator = operator_settings(&root, &config);
    let mut governor_child = GovernorChild::start(&root);
    let governor_text = governor_document(&root, &config, governor_child.0.id());
    let governor = governor::Settings::decode(governor_text.as_bytes()).unwrap();
    write_complete(&root.0.join("governor.json"), governor_text);
    assert_ne!(governor_child.0.id(), std::process::id());
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64").replace(
        "\"expected_policy_epoch\":\"0\"",
        &format!("\"expected_policy_epoch\":\"{promoted_epoch}\""),
    );
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let selected_scope = config.profile.delivery.scope;
    let profile = config.profile.clone();
    let store = config.store.clone();
    let directory = root.0.clone();
    let actor_peer = actor.clone();
    let operator_peer = operator.clone();
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let selected_inventory = inventory_path.clone();
    let (offered_send, offered_receive) = mpsc::channel();
    let (approve_send, approve_receive) = mpsc::channel();
    let fresh_reviewer = reviewer.clone();
    let second_human = std::thread::spawn(move || {
        let mut client = fresh_reviewer.connect_client_waiting(8, Duration::from_secs(10)).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap();
                    assert_eq!(packet.policy_generation(), 2);
                    assert_eq!(packet.action().spec().policy_epoch, promoted_epoch);
                    assert_eq!(packet.action().spec().payload, b"second");
                    assert_eq!(packet.views().len(), 2);
                    offered_send.send(()).unwrap();
                    approve_receive.recv_timeout(Duration::from_secs(10)).unwrap();
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    });
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&actor_peer);
        wait(&operator_peer.socket);
        let mut operator = UnixStream::connect(&operator_peer.socket).unwrap();
        operator_peer.supervisor.verify(&operator).unwrap();
        let unavailable = raw_exchange(&mut operator, &next(selected_scope));
        assert_eq!(field(field(&unavailable, "envelope"), "reason_code").as_str(), Some("CapabilityUnsupported"));
        // Recovery acknowledges the original recorded request; only creation
        // needs another first-request helper/human exchange.
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&next_control);
        let before = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!((before.executions, before.control.ledger.charged, before.control.ledger.epoch), (1, 7, before_epoch));
        let evidence_path = directory.join("evidence.json");
        let evidence_bytes = fs::read(&evidence_path).unwrap();
        let inventory_bytes = fs::read(&selected_inventory).unwrap();
        fs::remove_file(&evidence_path).unwrap();
        fs::remove_file(&selected_inventory).unwrap();
        write_complete(&directory.join("governor-review.json"), review_document(&before, selected_scope));
        wait(&directory.join("governor-approved.json"));
        let approval = governor_json(&fs::read(directory.join("governor-approved.json")).unwrap());
        governor_known(&approval, "approve");
        let approved = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!((approved.executions, approved.control.ledger.charged, approved.control.ledger.epoch), (1, 7, before_epoch));
        let saved = store_files(&store);
        let planned = raw_exchange(&mut operator, &next(selected_scope));
        let plan = field(known_operator(&planned, "policy-promotion-next"), "value");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        assert_eq!(field(plan, "candidate_policy_generation").as_str(), Some("2"));
        let command = governor::protocol::json(field(plan, "action"));
        assert_eq!(store_files(&store), saved);
        let denied = raw_exchange(&mut actor, &command);
        assert_eq!(field(field(&denied, "envelope"), "reason_code").as_str(), Some("DeniedByPolicy"));
        assert_eq!(store_files(&store), saved);
        let promoted = raw_exchange(&mut operator, &command);
        let promotion = known_operator(&promoted, "policy-promotion");
        assert_eq!(field(field(promotion, "basis"), "policy_generation").as_str(), Some("2"));
        assert_eq!(field(field(promotion, "basis"), "authority_epoch").as_str(), Some(promoted_epoch.to_string().as_str()));
        assert!(field(field(promotion, "value"), "cancelled").as_array().unwrap().is_empty());
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.sequence, approved.control.sequence + 1);
        assert_eq!((after.executions, after.control.ledger.charged, after.control.ledger.epoch), (1, 7, promoted_epoch));
        assert!(!evidence_path.exists() && !selected_inventory.exists());
        assert!(executed(&raw_exchange(&mut actor, &first)));
        write_complete(&evidence_path, evidence_bytes);
        write_complete(&selected_inventory, inventory_bytes);
        write_complete(&directory.join("helper-epoch"), promoted_epoch.to_string());
        if let Some(capture) = second_capture {
            write_complete(&directory.join("original.bin"), &capture);
            write_complete(&directory.join("current.bin"), &capture);
        }
        let pending = raw_exchange(&mut actor, &second);
        assert_eq!(field(knowledge(&pending), "state").as_str(), Some("pending"));
        offered_receive.recv_timeout(Duration::from_secs(10)).unwrap();
        let awaiting = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(awaiting.executions, 1);
        assert_eq!(awaiting.payload, b"first");
        let saved = store_files(&store);
        assert_eq!(raw_exchange(&mut operator, &command), promoted);
        assert_eq!(store_files(&store), saved, "promotion retry disturbed the fresh human review");
        approve_send.send(()).unwrap();
        if !lose_source {
            assert!(executed(&finish(&mut actor, &second_poll, &second_poll)));
        }
        // On final-source refusal the admitted request continues independently
        // of this actor connection, retaining its original native disposition.
    });
    let first_human = (!recovered).then(|| human_for(reviewer.clone(), 7));
    let store = config.store.clone();
    let profile = config.profile.clone();
    let mut observed_second_dispatch = false;
    let mut time = || {
        if !observed_second_dispatch
            && let Ok(state) = FileOversight::read_publication(&store, &profile)
            && state.executions == 1
            && state.control.ledger.charged == 14
        {
            observed_second_dispatch = true;
            if lose_source {
                fs::remove_file(&inventory_path).unwrap();
            }
        }
        ElapsedTick(if recovered { 1001 } else { 1000 })
    };
    let served = match (&recovery, &continuation) {
        (Some(recovery), Some(topology)) => topology_recovery::serve_sequence_governed(
            config, &actor, &reviewer, publication.as_ref(), None,
            recovery, topology, GENERATION, &operator, &governor, &[7, 8], &mut time,
        ),
        (None, None) => service::serve_tower_governed_with_topology(
            config, &actor, &reviewer, publication.as_ref(), &[7, 8],
            GENERATION, &operator, &governor, creation.unwrap(), &mut time,
        ),
        _ => unreachable!(),
    };
    peer.join().unwrap();
    if let Some(human) = first_human { human.join().unwrap(); }
    second_human.join().unwrap();
    assert!(governor_child.finish().success());
    assert!(observed_second_dispatch, "checked={checked}, recovered={recovered}: {served:?}");
    let disk = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(disk.executions, if lose_source { 1 } else { 2 });
    assert_eq!(disk.payload.as_slice(), if lose_source { b"first".as_slice() } else { b"second".as_slice() });
    assert_eq!(disk.target.expected_version, if lose_source { 2 } else { 3 });
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.available + disk.control.ledger.charged, profile.delivery.total);
    assert_eq!(disk.control.ledger.stages.len(), 2);
    assert_eq!(disk.control.ledger.stages[&1], ActionState::Confirmed);
    if lose_source {
        assert!(served.is_err(), "{served:?}");
        if disk.control.ledger.charged == 7 {
            assert_eq!(disk.control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
        } else {
            assert_eq!(disk.control.ledger.charged, 14);
            assert!(matches!(disk.control.ledger.stages[&2], ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
        }
    } else {
        assert!(served.is_ok(), "{served:?}");
        assert_eq!(disk.control.ledger.charged, 14);
        assert_eq!(disk.control.ledger.epoch, promoted_epoch);
        assert_eq!(disk.control.ledger.stages[&2], ActionState::Confirmed);
        assert!(disk.stop.is_none());
    }
    let saved = read_checkpoint(&root, if recovered { "source-2.bin" } else { "source-1.bin" });
    assert_eq!(saved.history_anchor().revision(), disk.revision);
    assert_eq!(saved.minimum().control_sequence, disk.control.sequence);
    assert_eq!(saved.minimum().authority_epoch, disk.control.ledger.epoch);
    // One resident initial check plus one compulsory check per fresh effect;
    // recovered request7 is a receipt, and graph rebind spends no source check.
    let checks = if recovered { 4 } else { 3 };
    assert_eq!(saved.work().checks, checks);
    assert_eq!(saved.work().reserved_read_bytes, checks * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(saved.last_operation(), if recovered { 5 } else { 3 });
    assert_eq!(saved.expected().graph().spec().generation, graph_generation);
    assert_eq!(saved.available(), !lose_source);
    assert_eq!(saved.topology_bytes(), original_topology);
    assert_eq!(saved.inventory_bytes(), original_inventory);
    if recovered {
        assert_eq!(saved.history_anchor(), &read_anchor(&root, "after.anchor"));
        assert_eq!(before_source.as_ref().unwrap().history_anchor(), &read_anchor(&root, "before.anchor"));
    }
    for socket in [actor.socket, operator.socket, governor.socket, reviewer.socket(7), reviewer.socket(8), crate::workflow::control::socket_path(&reviewer, 7), crate::workflow::control::socket_path(&reviewer, 8)] {
        assert!(!socket.exists(), "retired service left {}", socket.display());
    }
}
