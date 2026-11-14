//! A real recovered resident suffix: independently promoted history, a new
//! governor process, paid Tower promotion, and fresh native/human Finish gates.
use super::*;
use crate::workflow::actor_service::learned::anchors;
use fa_reference::action::consequence::{
    delivery::persistent::{
        governance::PolicyUpdate,
        observed::guarded::FileCampaignRequirement,
    },
    policy_campaign::ReplayLimits,
};

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 4,
    }
}

fn policy_document(generation: u64, maximum: usize) -> String {
    format!(r#"{{"generation":{generation},"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":{maximum}}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}"#)
}

fn recovery_document(inner: &str, generation: u64, maximum: usize) -> String {
    format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"recipe":{inner}}}"#,
        policy_document(generation, maximum))
}

fn remove_pair(args: &mut Vec<String>, option: &str) {
    let at = args.iter().position(|arg| arg == option).unwrap();
    args.drain(at..at + 2);
}

fn recovery_args(mut args: Vec<String>, anchor: &Path) -> Vec<String> {
    remove_pair(&mut args, "--campaign-profile");
    args.extend([
        "--after-message".into(), "1".into(),
        "--history-anchor".into(), anchor.display().to_string(),
    ]);
    args
}

// The first history is genuinely published and promoted. Its next Message has
// to resume the original two sampled draws, not create another prompt/session.
fn prepared(root: &Root)
    -> (Config, Vec<PathBuf>, Vec<resident::Entry>, PathBuf, FileDeliverySnapshot)
{
    let (mut config, paths, mut entries) = selected(root, None, None);
    entries[0].loaded.expected_campaigns = Some(requirement());
    entries[0].loaded.creation_campaigns = Some(requirement());
    let ((host, reviewer, _, _), governor) =
        recovery::create_governed(&config, &entries[0].loaded).unwrap();
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
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    let input = root.0.join("promoted-message.anchor");
    let before = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let control = host.inspect().control;
        let update = PolicyUpdate::new(900, control.sequence, control.ledger.epoch,
            policy()).unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        let revision = host.revision();
        let approval = governor.approve(&mut host, revision, &review, false).unwrap();
        let revision = host.revision();
        host.promote_policy_campaign(revision, &approval).unwrap();
        assert_eq!(host.current_policy().unwrap(), &policy());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (4, 2));
        assert_eq!(host.inspect().executions, 1);
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &input).unwrap();
        host.inspect()
    };
    drop(history);
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    for (path, entry) in paths.iter().zip(&mut entries).skip(1) {
        let original = fs::read_to_string(path).unwrap();
        fs::write(path, recovery_document(&original, 2, 256)).unwrap();
        entry.loaded.expected_policy = policy();
        entry.loaded.expected_campaigns = Some(requirement());
    }
    (config, paths, entries, input, before)
}

#[test]
fn recovered_governor_preflights_every_complete_policy_and_campaign_before_assets() {
    for variation in 0..8 {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, None, None);
        selections(&root, &entries[0].actor, std::process::id().checked_add(10_000).unwrap());
        let originals = paths.iter().map(|path| fs::read_to_string(path).unwrap()).collect::<Vec<_>>();
        for (path, original) in paths.iter().zip(&originals) {
            fs::write(path, recovery_document(original, 2, 256)).unwrap();
        }
        let mut args = recovery_args(inputs(&root, &config, &paths, &entries, None),
            &root.0.join("not-yet-read.anchor"));
        let expected = match variation {
            0 => {
                remove_pair(&mut args, "--history-anchor");
                "recovery with --history-anchor"
            }
            1 => {
                fs::write(&paths[2], &originals[2]).unwrap();
                "explicit full effective policy and exact original campaign limits"
            }
            2 => {
                fs::write(&paths[2], format!(
                    r#"{{"schema":"fa.learned-recovery-policy/1","effective_policy":{},"recipe":{}}}"#,
                    policy_document(2, 256), originals[2])).unwrap();
                "explicit full effective policy and exact original campaign limits"
            }
            3 => {
                let document = recovery_document(&originals[2], 2, 256)
                    .replace("\"cases\":16", "\"cases\":17");
                fs::write(&paths[2], document).unwrap();
                "explicit full effective policy and exact original campaign limits"
            }
            4 => {
                let document = recovery_document(&originals[2], 2, 256)
                    .replace("\"input_bytes\":1048576", "\"input_bytes\":1048577");
                fs::write(&paths[2], document).unwrap();
                "explicit full effective policy and exact original campaign limits"
            }
            5 => {
                let document = recovery_document(&originals[2], 2, 256)
                    .replace("\"max_campaigns\":4", "\"max_campaigns\":5");
                fs::write(&paths[2], document).unwrap();
                "explicit full effective policy and exact original campaign limits"
            }
            6 => {
                fs::write(&paths[2], recovery_document(&originals[2], 2, 255)).unwrap();
                "same complete effective policy"
            }
            7 => {
                fs::write(&paths[2], recovery_document(&originals[2], 3, 256)).unwrap();
                "same complete effective policy"
            }
            _ => unreachable!(),
        };
        fs::remove_file(root.0.join("weights.safetensors")).unwrap();
        fs::remove_file(root.0.join("native-roster.json")).unwrap();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let error = resident::command_with_clock(&args, None,
            || panic!("invalid recovered selection reached time")).unwrap_err();
        assert!(error.contains(expected), "variation {variation}: {error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("final.anchor").exists());
        quiescent(&root);
    }
}

fn ordinary_operator(root: &Root, actor: &Profile) -> learned_operator::Selection {
    let identity = actor.supervisor;
    let principal = format!(r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        identity.uid(), identity.gid(), identity.pid().unwrap());
    let path = root.0.join("ordinary-operator.json");
    fs::write(&path, format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{},"socket":{},"supervisor":{principal},"operator":{principal},"candidate_limit":1,"connection_limit":1,"exchange_limit":2,"runtime_ms":60000,"poll_ms":1,"inspection_queries":1,"explanation_queries":0,"explanation_replay_events":4096,"explanation_disclosure":"policy_and_selected_witnesses"}}"#,
        scope_json(actor), quote(root.0.join("peers/ordinary-operator.sock").to_str().unwrap()))).unwrap();
    learned_operator::Selection::read(&path, GENERATION).unwrap()
}

#[test]
fn ordinary_anchored_startup_failures_retain_the_fence_and_allow_corrected_continuation() {
    for listener_failure in [false, true] {
        let root = Root::new();
        let (config, paths, mut entries, input, before) = prepared(&root);
        let before_anchor = fs::read(&input).unwrap();
        let output = root.0.join("refused-startup.anchor");
        let first = &mut entries[1].loaded;
        first.recovery_anchor = Some(anchors::load(&input).unwrap());
        first.anchor_output = Some(output.clone());
        first.floor.journal_revision = before.revision;
        first.floor.control_sequence = before.control.sequence;
        first.floor.authority_epoch = before.control.ledger.epoch;
        let operator = listener_failure.then(|| ordinary_operator(&root, &entries[1].actor));
        let peers = fixture::peers(&root, &config);
        let reader = Reader::new(&root, &paths[1]);
        let actors = entries.iter().skip(1).map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let mut clients = Peers::new(&root, &config, &entries);
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
        let mut calls = 0;
        let result = resident::run_with_services(
            config, entries.split_off(1), &peers, true,
            Some(if listener_failure { 1 } else { 999 }), None,
            operator.as_ref(), None, || {
                // All role/path preflight has already succeeded. This competing
                // file then makes the actual post-Fence listener bind fail.
                if calls == 0 {
                    if let Some(operator) = &operator {
                        fs::write(&operator.settings.socket, b"independent listener collision").unwrap();
                    }
                }
                calls += 1;
                ElapsedTick(1000)
            },
        );
        assert!(result.is_err(), "invalid original predecessor or listener cannot start");
        let state = reader.snapshot();
        assert_eq!(state.revision, before.revision + 1);
        assert_eq!(state.control.ledger.epoch, before.control.ledger.epoch + 1);
        assert_eq!(state.executions, before.executions);
        assert_eq!(state.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(state.payload, before.payload);
        let saved = anchors::load(&output).unwrap();
        assert_eq!(saved.revision(), state.revision);
        assert_eq!(fs::read(&input).unwrap(), before_anchor);
        assert!(!root.0.join("evidence.json").exists());
        assert!(!root.0.join("native-roster.json").exists());
        if let Some(operator) = &operator {
            assert_eq!(fs::read(&operator.settings.socket).unwrap(), b"independent listener collision");
            fs::remove_file(&operator.settings.socket).unwrap();
        }
        quiescent(&root);

        // The acknowledged successor is usable, not merely an emitted filename.
        // Correct only the failed startup input, restore independently provisioned
        // evidence, then complete the original Message2 and Finish through both keys.
        for (path, bytes) in removed {
            fs::write(path, bytes).unwrap();
        }
        let config = configured(&root);
        let mut corrected = paths[1..].iter().zip(actors).enumerate().map(|(index, (path, actor))| {
            resident::Entry {
                loaded: recipe::load(path, &config, true).unwrap(),
                actor,
                precheck: None,
                release: if index == 0 { LearnedTextRelease::Message } else { LearnedTextRelease::Finish },
            }
        }).collect::<Vec<_>>();
        corrected[0].loaded.recovery_anchor = Some(saved);
        corrected[0].loaded.anchor_output = Some(root.0.join("corrected-final.anchor"));
        let result = resident::run_with_services(
            config, corrected, &peers, true, Some(1), None, None, None, || {
                clients.poll(
                    [None, Some(Actor::Observe), Some(Actor::Observe)],
                    [None, Some(ReviewDecision::Approve), Some(ReviewDecision::Approve)],
                );
                ElapsedTick(1000)
            },
        ).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 2);
        assert!(result.responses.iter().all(confirmed));
        let (responses, packets) = clients.join();
        assert!(responses[0].is_none() && packets[0].is_none());
        assert!(responses[1..].iter().all(|response| confirmed(response.as_ref().unwrap())));
        let state = reader.snapshot();
        assert_eq!((state.executions, state.payload.as_slice()), (3, b"AA".as_slice()));
        assert_eq!(state.control.ledger.epoch, before.control.ledger.epoch + 2);
        assert_eq!(fs::read(&input).unwrap(), before_anchor);
        assert_eq!(
            anchors::load(&root.0.join("corrected-final.anchor")).unwrap().revision(),
            state.revision,
        );
        quiescent(&root);
    }
}
