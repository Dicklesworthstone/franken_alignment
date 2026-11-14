//! Independently anchored ordinary recovery keeps the original finite stream.
//! Each singleton release still uses a fresh authenticated actor, real native
//! review and independent human approval, with no governor or new generation.
use super::*;
use crate::workflow::actor_service::learned::anchors;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{Reconciliation, observed::driver::FileDriverEvent},
};

fn inputs(root: &Root, config: &Config, paths: &[PathBuf], entries: &[resident::Entry])
    -> Vec<String>
{
    assert_eq!(paths.len(), entries.len());
    let mut selected = Vec::new();
    for (path, entry) in paths.iter().zip(entries) {
        let inputs = super::super::mediated_topology::command_inputs(
            root, config, path, &entry.actor,
        );
        let actor = root.0.join(format!("remaining-actor-{}.json", entry.loaded.request));
        fs::rename(&inputs[5], &actor).unwrap();
        selected.push(format!(r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
            quote(path.to_str().unwrap()), quote(actor.to_str().unwrap()),
            if entry.release == LearnedTextRelease::Finish { "finish" } else { "message" }));
    }
    let sequence = root.0.join("remaining-sequence.json");
    fs::write(&sequence, format!(r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
        selected.join(","))).unwrap();
    vec![
        "serve-learned-generated".into(),
        root.0.join("command-host.json").display().to_string(),
        sequence.display().to_string(),
        root.0.join("command-reviewer.json").display().to_string(),
        "--retain-history-anchor".into(), root.0.join("final.anchor").display().to_string(),
    ]
}

fn recovery_args(mut args: Vec<String>, anchor: &Path) -> Vec<String> {
    args.extend([
        "--after-message".into(), "1".into(),
        "--history-anchor".into(), anchor.display().to_string(),
    ]);
    args
}

fn remove_pair(args: &mut Vec<String>, option: &str) {
    let at = args.iter().position(|arg| arg == option).unwrap();
    args.drain(at..at + 2);
}

fn replace_value(args: &mut [String], option: &str, value: impl Into<String>) {
    let at = args.iter().position(|arg| arg == option).unwrap();
    args[at + 1] = value.into();
}

// This original history really published Message1. Its remaining suffix must
// retain those sampled draws, receipt, audience prefix and charged liability.
fn prepared(root: &Root)
    -> (Config, Vec<PathBuf>, Vec<resident::Entry>, PathBuf, FileDeliverySnapshot)
{
    prepared_with_unknown(root, false)
}

fn prepared_with_unknown(root: &Root, published_unknown: bool)
    -> (Config, Vec<PathBuf>, Vec<resident::Entry>, PathBuf, FileDeliverySnapshot)
{
    let (mut config, paths, entries) = selected(root, None, None);
    let (host, reviewer, _, _) =
        recovery::create_selected(&config, &entries[0].loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let limit = deadline();
    let first = &entries[0].loaded;
    let mut control = Control::new(&config, first.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, first, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) =
        native::review_current(&mut config, first, &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    if published_unknown {
        assert!(matches!(
            driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), Some(&human),
            ).result.unwrap(),
            FileDriverEvent::Dispatched { .. },
        ));
        // Query the genuinely queued envelope before its publication, then let
        // that same envelope execute without reconciling the original receipt.
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            assert_eq!(host.reconcile(revision, 1).unwrap(), Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        }
        let captured = refresh(&mut driver, &mut config, &limit, &mut || ElapsedTick(1000)).unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            let publication = host.publish_checked(
                revision, 1, Some(history.input()), captured.snapshot().clone(), ElapsedTick(1000),
            ).unwrap();
            assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
            let stream = host.stream_snapshot().unwrap();
            assert_eq!(stream.pending, Some(1));
            assert_eq!(stream.confirmed.messages().count(), 0);
        }
        assert!(matches!(response(&mut wire, 1).result.unwrap(), Knowledge::Unknown { .. }));
    } else {
        native::publish(&mut driver, &mut config, &human);
        assert!(executed(&mut wire, 1));
    }
    let input = root.0.join("original-message.anchor");
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
    (config, paths, entries, input, before)
}

#[test]
fn ordinary_anchored_singleton_message_and_finish_preserve_the_original_stream() {
    singleton_sequence(ReviewDecision::Approve);
}

#[test]
fn ordinary_singleton_finish_requires_fresh_human_approval() {
    singleton_sequence(ReviewDecision::Reject);
}

fn singleton_sequence(decision: ReviewDecision) {
    let root = Root::new();
    let (config, paths, entries, input, before) = prepared(&root);
    let input_bytes = fs::read(&input).unwrap();
    let message_anchor = root.0.join("singleton-message.anchor");
    let mut message_args = recovery_args(
        inputs(&root, &config, &paths[1..2], &entries[1..2]), &input,
    );
    replace_value(&mut message_args, "--retain-history-anchor", message_anchor.display().to_string());
    let mut message_peers = Peers::new(&root, &config, &entries);
    resident::command_with_clock(&message_args, None, || {
        message_peers.poll(
            [None, Some(Actor::Observe), None],
            [None, Some(ReviewDecision::Approve), None],
        );
        ElapsedTick(1000)
    }).unwrap();
    assert_eq!(message_peers.seen_actor, [false, true, false]);
    assert_eq!(message_peers.seen_human, [false, true, false]);
    let (message_responses, message_packets) = message_peers.join();
    assert!(confirmed(message_responses[1].as_ref().unwrap()));
    let message = message_packets[1].as_ref().unwrap();
    assert_eq!(message.views().len(), 2);
    assert_eq!(message.policy_generation(), 1);
    assert_eq!(message.action().spec().policy_epoch, before.control.ledger.epoch + 1);
    let frame = ReleaseFrame::decode(&message.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(frame.message(), Some("A"));
    let mut reader = Reader::new(&root, &paths[1]);
    let state = reader.snapshot();
    assert_eq!((state.executions, state.payload.as_slice()), (2, b"AA".as_slice()));
    assert_eq!(state.control.ledger.charged,
        before.control.ledger.charged + message.action().spec().units);
    assert_eq!(anchors::load(&message_anchor).unwrap().revision(), state.revision);
    let message_archive = fs::read(&message_anchor).unwrap();
    quiescent(&root);

    // The original two-Message/four-draw allowance is exhausted. A third
    // Message must refuse without recording its intent or consuming more draws;
    // the acknowledged recovery Fence still has to be retained for Finish.
    let attempted = resident::Entry {
        loaded: recipe::load(&paths[2], &config, true).unwrap(),
        actor: entries[2].actor.clone(),
        precheck: None,
        release: LearnedTextRelease::Message,
    };
    let exhausted_anchor = root.0.join("exhausted-message.anchor");
    let mut exhausted_args = recovery_args(
        inputs(&root, &config, &paths[2..], &[attempted]), &message_anchor,
    );
    replace_value(&mut exhausted_args, "--after-message", "2");
    replace_value(&mut exhausted_args, "--retain-history-anchor", exhausted_anchor.display().to_string());
    assert!(resident::command_with_clock(&exhausted_args, None, || ElapsedTick(1000)).is_err());
    let exhausted = reader.snapshot();
    assert_eq!(exhausted.revision, state.revision + 1);
    assert_eq!(exhausted.control.ledger.epoch, state.control.ledger.epoch + 1);
    assert_eq!(exhausted.executions, state.executions);
    assert_eq!(exhausted.control.ledger.charged, state.control.ledger.charged);
    assert_eq!(exhausted.payload, state.payload);
    assert!(!exhausted.control.ledger.stages.contains_key(&3));
    assert_eq!(anchors::load(&exhausted_anchor).unwrap().revision(), exhausted.revision);
    quiescent(&root);

    // Finish is a separate invocation with a new recovery Fence and human key.
    // It names the exact original confirmed Message2 and cannot sample again.
    let finish_anchor = root.0.join("singleton-finish.anchor");
    let mut finish_args = recovery_args(
        inputs(&root, &config, &paths[2..], &entries[2..]), &exhausted_anchor,
    );
    replace_value(&mut finish_args, "--after-message", "2");
    replace_value(&mut finish_args, "--retain-history-anchor", finish_anchor.display().to_string());
    let mut finish_peers = Peers::new(&root, &config, &entries);
    let finished = resident::command_with_clock(&finish_args, None, || {
        finish_peers.poll(
            [None, None, Some(Actor::Observe)],
            [None, None, Some(decision)],
        );
        ElapsedTick(1000)
    });
    assert_eq!(finished.is_ok(), decision == ReviewDecision::Approve, "{finished:?}");
    assert_eq!(finish_peers.seen_actor, [false, false, true]);
    assert_eq!(finish_peers.seen_human, [false, false, true]);
    let (finish_responses, finish_packets) = finish_peers.join();
    assert_eq!(confirmed(finish_responses[2].as_ref().unwrap()), decision == ReviewDecision::Approve);
    let finish = finish_packets[2].as_ref().unwrap();
    assert_eq!(finish.views().len(), 2);
    assert_eq!(finish.policy_generation(), 1);
    assert_eq!(finish.action().spec().policy_epoch, before.control.ledger.epoch + 3);
    assert_ne!(message.binding().session, finish.binding().session);
    let frame = ReleaseFrame::decode(&finish.action().spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert_eq!(frame.message(), None);
    let state = reader.snapshot();
    assert_eq!(state.executions, if decision == ReviewDecision::Approve { 3 } else { 2 });
    assert_eq!(state.payload, b"AA");
    assert_eq!(state.control.ledger.epoch, before.control.ledger.epoch + 3);
    assert_eq!(state.control.ledger.charged,
        before.control.ledger.charged + message.action().spec().units
            + if decision == ReviewDecision::Approve { finish.action().spec().units } else { 0 });
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.available + state.control.ledger.charged,
        reader.config.profile.delivery.total);
    let saved = anchors::load(&finish_anchor).unwrap();
    assert_eq!(saved.revision(), state.revision);
    assert_eq!(fs::read(&input).unwrap(), input_bytes);
    assert_eq!(fs::read(&message_anchor).unwrap(), message_archive);
    reader.loaded.recovery_anchor = Some(saved);
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    assert_eq!(host.current_policy().unwrap(), &reader.loaded.expected_policy);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(stream.confirmed.finished(), decision == ReviewDecision::Approve);
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.pending, None);
    quiescent(&root);
}

#[test]
fn ordinary_singleton_admission_requires_anchor_and_explicit_finish_predecessor_before_assets() {
    for variation in 0..7 {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, None, None);
        let (index, count) = if variation == 3 {
            (1, 2)
        } else if matches!(variation, 0 | 1 | 6) {
            (1, 1)
        } else {
            (2, 1)
        };
        if variation == 3 {
            entries[1].release = LearnedTextRelease::Finish;
        }
        let mut args = recovery_args(
            inputs(&root, &config, &paths[index..index + count], &entries[index..index + count]),
            &root.0.join("not-yet-read.anchor"),
        );
        let expected = match variation {
            0 => {
                remove_pair(&mut args, "--after-message");
                remove_pair(&mut args, "--history-anchor");
                "a learned sequence requires 2..=16 entries"
            }
            1 => {
                remove_pair(&mut args, "--history-anchor");
                "a learned sequence requires 2..=16 entries"
            }
            2 => {
                remove_pair(&mut args, "--after-message");
                args.push("--continue-generation".into());
                "sequence entries must be Messages followed by an optional final Finish"
            }
            3 => "sequence entries must be Messages followed by an optional final Finish",
            4 => {
                replace_value(&mut args, "--after-message", "0");
                "serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE"
            }
            5 => {
                args.extend([
                    "--checkpoint-before-generation".into(),
                    root.0.join("not-yet-read-checkpoint.json").display().to_string(),
                ]);
                "a remaining Finish cannot select a checkpoint before generation"
            }
            6 => {
                remove_pair(&mut args, "--after-message");
                remove_pair(&mut args, "--history-anchor");
                args.push("--continue-generation".into());
                "a learned sequence requires 2..=16 entries"
            }
            _ => unreachable!(),
        };
        fs::remove_file(root.0.join("weights.safetensors")).unwrap();
        fs::remove_file(root.0.join("native-roster.json")).unwrap();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let error = resident::command_with_clock(&args, None,
            || panic!("invalid singleton selection reached time")).unwrap_err();
        assert!(error.contains(expected), "variation {variation}: {error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("final.anchor").exists());
        quiescent(&root);
    }
}

#[test]
fn direct_ordinary_singleton_cannot_infer_an_anchor_or_create_a_history() {
    for continuing in [false, true] {
        let root = Root::new();
        let (config, _paths, mut entries) = selected(&root, None, None);
        let reviewer = fixture::peers(&root, &config);
        let store = config.store.clone();
        let entry = entries.remove(1);
        let result = resident::run(config, vec![entry], &reviewer,
            continuing, continuing.then_some(1), None,
            || panic!("unanchored singleton reached time"));
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("unanchored singleton admitted"),
        };
        assert!(error.contains("a learned sequence requires 2..=16"), "{error}");
        assert!(!store.exists());
        quiescent(&root);
    }
}

#[test]
fn ordinary_singleton_finish_cannot_confirm_an_unknown_predecessor() {
    let root = Root::new();
    let (config, paths, entries, input, before) = prepared_with_unknown(&root, true);
    assert_eq!(before.executions, 1);
    assert_eq!(before.control.ledger.stages[&1], ActionState::Unknown);
    assert!(before.control.ledger.charged > 0);
    let input_bytes = fs::read(&input).unwrap();
    let args = recovery_args(
        inputs(&root, &config, &paths[2..], &entries[2..]), &input,
    );
    let reader = Reader::new(&root, &paths[2]);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let error = resident::command_with_clock(&args, None, || ElapsedTick(1000)).unwrap_err();
    assert!(error.contains("learned finish requires"), "{error}");
    let state = reader.snapshot();
    assert_eq!(state.revision, before.revision + 1);
    assert_eq!(state.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(state.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(state.executions, before.executions);
    assert_eq!(state.payload, before.payload);
    assert!(!state.control.ledger.stages.contains_key(&3));
    assert_eq!(anchors::load(&root.0.join("final.anchor")).unwrap().revision(), state.revision);
    assert_eq!(fs::read(&input).unwrap(), input_bytes);
    assert!(!root.0.join("evidence.json").exists());
    quiescent(&root);
}
