//! Single-request recovery must not lose an acknowledged Fence when startup
//! refuses before the request cycle. Real native and human review then use the
//! retained successor to continue and close the same finite learned stream.
#![cfg(target_os = "linux")]
use super::*;
use super::super::peer_intake as peers;
use super::super::super::{anchors, operator as live_operator, run_with_actor_services};
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    observed::reviewer::wire::{ReviewDecision, ReviewPacket},
};
use std::path::{Path, PathBuf};

fn first_message(root: &Root) -> (PathBuf, FileDeliverySnapshot) {
    let (mut config, loaded, reviewer, driver, mut wire, limit) = super::prepared(root);
    let (mut driver, history) =
        review_current(&mut config, &loaded, &reviewer, driver, &limit);
    let human = approve(&mut driver, &mut config, &reviewer, 1);
    publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    let input = root.0.join("single-first.anchor");
    let before = {
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (4, 2));
        assert_eq!(host.inspect().executions, 1);
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &input).unwrap();
        host.inspect()
    };
    drop(history);
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    (input, before)
}

fn operator(root: &Root, actor: &Profile) -> live_operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let path = root.0.join("single-operator.json");
    fs::write(&path, format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}/peers/single-operator.sock","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":1,"connection_limit":1,"exchange_limit":2,"runtime_ms":60000,"poll_ms":1,"inspection_queries":1,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none"}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        root.0.display(), identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(), identity.pid().unwrap())).unwrap();
    live_operator::Selection::read(&path, 91).unwrap()
}

fn assert_quiescent(root: &Root) {
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

fn refused(
    root: &Root, recipe_path: &Path, input: &Path, output: &Path,
    previous: u64, finishing: bool, listener_failure: bool,
) -> FileDeliverySnapshot {
    let config = configured(root);
    let mut loaded = recipe::load(recipe_path, &config, true).unwrap();
    loaded.recovery_anchor = Some(anchors::load(input).unwrap());
    loaded.anchor_output = Some(output.to_owned());
    let before = FileOversight::read_publication_with_learned_generation(
        &config.store, &config.profile, &loaded.generation,
    ).unwrap();
    loaded.floor.journal_revision = before.revision;
    loaded.floor.control_sequence = before.control.sequence;
    loaded.floor.authority_epoch = before.control.ledger.epoch;
    let input_bytes = fs::read(input).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let actor = peers::actor_profile(root, &config, loaded.request);
    let reviewers = fixture::peers(root, &config);
    let selected_operator = listener_failure.then(|| operator(root, &actor));
    let store = config.store.clone();
    let profile = config.profile.clone();
    let generation = loaded.generation.clone();
    let removed = [
        "evidence.json", "native-roster.json", "helper-model.json",
        "helper-weights.safetensors", "helper-monitor.json",
        "helper-tokenizer.bbpe", "helper-sampling.json",
    ].into_iter().map(|name| {
        let path = root.0.join(name);
        let bytes = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        (path, bytes)
    }).collect::<Vec<_>>();
    let predecessor = if listener_failure { previous } else { 999 };
    let mut calls = 0;
    let result = run_with_actor_services(
        config, loaded, &reviewers, true,
        finishing.then_some(predecessor), (!finishing).then_some(predecessor),
        None, Some(&actor), None, selected_operator.as_ref(), || {
            // Static path/custody checks already succeeded. Force the actual
            // listener bind after recovery to lose this independently held path.
            if calls == 0 {
                if let Some(operator) = &selected_operator {
                    fs::write(&operator.settings.socket, b"single-request listener collision").unwrap();
                }
            }
            calls += 1;
            ElapsedTick(1000)
        },
    );
    let error = result.err().expect("startup must refuse before the live request cycle");
    if !listener_failure {
        assert!(error.contains("Missing"), "{error}");
    }
    let after = FileOversight::read_publication_with_learned_generation(
        &store, &profile, &generation,
    ).unwrap();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_ne!(fs::read(store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(anchors::load(output).unwrap().revision(), after.revision);
    assert_eq!(fs::read(input).unwrap(), input_bytes);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!root.0.join("native-roster.json").exists());
    if let Some(operator) = &selected_operator {
        assert_eq!(fs::read(&operator.settings.socket).unwrap(), b"single-request listener collision");
        fs::remove_file(&operator.settings.socket).unwrap();
    }
    assert_quiescent(root);
    for (path, bytes) in removed {
        fs::write(path, bytes).unwrap();
    }
    after
}

fn complete(
    root: &Root, recipe_path: &Path, input: &Path, output: &Path,
    previous: u64, finishing: bool,
) -> (FileDeliverySnapshot, ReviewPacket) {
    let config = configured(root);
    let mut loaded = recipe::load(recipe_path, &config, true).unwrap();
    loaded.recovery_anchor = Some(anchors::load(input).unwrap());
    loaded.anchor_output = Some(output.to_owned());
    let request = loaded.request;
    let actor = peers::actor_profile(root, &config, request);
    let reviewers = fixture::peers(root, &config);
    let release = if finishing { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let store = config.store.clone();
    let profile = config.profile.clone();
    let generation = loaded.generation.clone();
    let input_bytes = fs::read(input).unwrap();
    let mut actor_peer = None;
    let mut human_peer = None;
    let result = run_with_actor_services(
        config, loaded, &reviewers, true,
        finishing.then_some(previous), (!finishing).then_some(previous),
        None, Some(&actor), None, None, || {
            if actor.socket.exists() && actor_peer.is_none() {
                actor_peer = Some(peers::actor(
                    actor.clone(), release, ElapsedTick(50_000), peers::Behavior::Observe,
                ));
            }
            if reviewers.socket(request).exists() && human_peer.is_none() {
                human_peer = Some(peers::human(reviewers.clone(), request, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        },
    ).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let response = actor_peer.expect("original actor was admitted").join().unwrap();
    assert!(matches!(response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let packet = human_peer.expect("fresh human review was required").join().unwrap();
    assert_eq!(packet.views().len(), 2);
    let after = FileOversight::read_publication_with_learned_generation(
        &store, &profile, &generation,
    ).unwrap();
    assert_eq!(anchors::load(output).unwrap().revision(), after.revision);
    assert_eq!(fs::read(input).unwrap(), input_bytes);
    assert_quiescent(root);
    (after, packet)
}

#[test]
fn missing_single_request_predecessors_keep_the_fence_for_corrected_message_and_finish() {
    corrected_single_requests(false);
}

#[test]
fn late_single_operator_bind_failures_keep_the_fence_for_corrected_message_and_finish() {
    corrected_single_requests(true);
}

fn corrected_single_requests(listener_failure: bool) {
    let root = Root::new();
    let (input, before) = first_message(&root);
    let original = fs::read(&input).unwrap();
    let config = configured(&root);
    let (message_path, _) = next(&root, &config, 2);
    let (finish_path, _) = next(&root, &config, 3);
    let refused_message = root.0.join("single-refused-message.anchor");
    let message_anchor = root.0.join("single-message.anchor");
    let refused_finish = root.0.join("single-refused-finish.anchor");
    let final_anchor = root.0.join("single-finished.anchor");

    let failure = refused(&root, &message_path, &input, &refused_message,
        1, false, listener_failure);
    assert_eq!(failure.executions, 1);
    assert_eq!(failure.control.ledger.epoch, before.control.ledger.epoch + 1);
    let (message, message_packet) =
        complete(&root, &message_path, &refused_message, &message_anchor, 1, false);
    assert_eq!((message.executions, message.payload.as_slice()), (2, b"AA".as_slice()));
    assert_eq!(message.control.ledger.epoch, before.control.ledger.epoch + 2);
    assert_eq!(message.control.ledger.charged,
        before.control.ledger.charged + message_packet.action().spec().units);
    let frame = ReleaseFrame::decode(&message_packet.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(frame.message(), Some("A"));

    let failure = refused(&root, &finish_path, &message_anchor, &refused_finish,
        2, true, listener_failure);
    assert_eq!(failure.executions, 2);
    assert_eq!(failure.control.ledger.epoch, before.control.ledger.epoch + 3);
    let (finished, finish_packet) =
        complete(&root, &finish_path, &refused_finish, &final_anchor, 2, true);
    assert_eq!((finished.executions, finished.payload.as_slice()), (3, b"AA".as_slice()));
    assert_eq!(finished.control.ledger.epoch, before.control.ledger.epoch + 4);
    assert_eq!(finished.control.ledger.charged,
        message.control.ledger.charged + finish_packet.action().spec().units);
    assert_eq!(finished.control.ledger.reserved, 0);
    assert_eq!(finished.control.ledger.available + finished.control.ledger.charged,
        config.profile.delivery.total);
    assert_ne!(message_packet.binding().session, finish_packet.binding().session);
    assert_eq!(message_packet.action().spec().policy_epoch, before.control.ledger.epoch + 2);
    assert_eq!(finish_packet.action().spec().policy_epoch, before.control.ledger.epoch + 4);
    let frame = ReleaseFrame::decode(&finish_packet.action().spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert_eq!(frame.message(), None);
    assert_eq!(fs::read(&input).unwrap(), original);

    let mut loaded = recipe::load(&finish_path, &config, false).unwrap();
    loaded.recovery_anchor = Some(anchors::load(&final_anchor).unwrap());
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.pending, None);
    assert_quiescent(&root);
}
