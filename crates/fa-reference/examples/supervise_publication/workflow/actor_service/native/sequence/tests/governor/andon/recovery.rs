//! Complete governor and evaluation recovery select the same native replay.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;
use crate::workflow::actor_service::native::sequence::evaluation::Recovery;
use std::sync::mpsc;

struct Pending {
    config: Config,
    entries: Vec<Entry>,
    reviewer: PeerProfile,
    reader: Reader,
    accounting: andon_settings::Settings,
    governor: live_governor::Settings,
    operator: operator::Selection,
    requirements: FileRecoveryRequirements,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
}

fn pending(root: &Root, steps: u64, joint: bool) -> Pending {
    let (mut config, entries, reviewer) = selected(root, true);
    let reader = Reader::new(&config, &entries[0]);
    let accounting = settings(joint);
    let (governor, operator) = selections(
        root, &entries[0].actor, std::process::id().checked_add(100_000).unwrap(),
    );
    let selected = bootstrap::selection(&config, &entries[0].inputs, None).unwrap();
    let (mut host, human, old_governor, evaluator) = bootstrap::prepare_andon_selected(
        &config, &entries[0].inputs, selected, &accounting,
        Some(governor.campaign_requirement()),
    ).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        7, numerical.actor_revision, numerical.position, entries[0].inputs.request.clone(),
    ).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for completed in 0..steps {
        host.advance_decoder_text(host.revision(), 7, completed).unwrap();
    }
    let before = host.inspect();
    assert_eq!(before.executions, 0);
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, steps);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, steps - 2);
    let anchor = root.0.join("original-governed-accounted.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    let requirements = FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(entries[0].inputs.stream), decoder: Some(entries[0].inputs.decoder.clone()),
            decoder_stop: None, source: Some(config.source_policy), identity: None,
            campaigns: Some(governor.campaign_requirement()), credential: None,
        },
        effective_policy: config.profile.delivery.policy.clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: before.revision, control_sequence: before.control.sequence,
            authority_epoch: before.control.ledger.epoch,
        },
    };
    drop(evaluator);
    drop(old_governor);
    drop(human);
    drop(host);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Pending { config, entries, reviewer, reader, accounting, governor, operator,
        requirements, anchor, before }
}

fn recovery(root: &Root, pending: &Pending, floor: FileRecoveryFloor) -> (Recovery, anchors::Selection) {
    let path = root.0.join("evaluation-floors.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch,
    )).unwrap();
    let selected = Recovery::read(&path, &pending.config).unwrap();
    let args = [
        "serve-native-generated".into(), "config".into(), "sequence".into(), "reviewer".into(),
        "--history-anchor".into(), pending.anchor.display().to_string(),
        "--retain-history-anchor".into(), root.0.join("recovered.anchor").display().to_string(),
    ];
    let history = anchors::take_options(&args).unwrap().1.admit_mode(
        &pending.config,
        &pending.entries.iter().map(|entry| (&entry.actor, entry.precheck.as_ref())).collect::<Vec<_>>(),
        &pending.reviewer, Some(&pending.operator), Mode::ContinueGeneration, None,
    ).unwrap();
    (selected, history)
}

fn continued(steps: u64, approve: bool) {
    let root = Root::new();
    let pending = pending(&root, steps, true);
    let (recovery, history) = recovery(&root, &pending, pending.requirements.minimum);
    let original_anchor = fs::read(&pending.anchor).unwrap();
    let original = pending.reader.progress(7);
    let mut peer = EvaluatorProcess::start(
        &root, &pending.entries[0].actor, if approve { 6 } else { 2 },
    );
    let evaluator = peer.selected.clone();
    let requests = if approve { REQUESTS.to_vec() } else { vec![REQUESTS[0]] };
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = operator_tests::held_reviews(
        pending.reviewer.clone(), requests.clone(), offered, decisions,
    );
    let store = pending.reader.store.clone();
    let profile = pending.reader.profile.clone();
    let independent = thread::spawn(move || {
        for (index, request) in requests.into_iter().enumerate() {
            let (actual, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(actual, request);
            assert_eq!(packet.action().spec().policy_epoch, 1);
            let (accepted, response) = peer.operation("label", request);
            assert!(accepted, "{}", String::from_utf8_lossy(&response));
            assert_eq!(field(&json(&response), "status").as_str(), Some("applied"));
            assert_eq!(FileOversight::read_publication(&store, &profile).unwrap().executions, index as u64);
            release.send(Some(if approve { ReviewDecision::Approve } else { ReviewDecision::Reject })).unwrap();
        }
        peer.finish();
    });
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    let mut saw_original = false;
    let result = run_accounted_selected(
        pending.config, pending.entries, &pending.reviewer, None,
        (Mode::ContinueGeneration, Some(&pending.operator)), &history,
        andon::Selection {
            settings: &pending.accounting, evaluator: &evaluator, recovery: Some(&recovery), paths: None,
        },
        Some(&pending.governor), Some(&pending.requirements),
        || {
            if !saw_original && pending.reviewer.socket(REQUESTS[0]).exists() {
                let now = pending.reader.progress(7);
                assert_eq!(now.text.command(), original.text.command());
                assert_eq!(now.numerical.numerical.position, 4);
                assert_eq!(now.numerical.numerical.sampled_draws, 2);
                saw_original = true;
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    independent.join().unwrap();
    assert!(saw_original);
    let packets = human.join().unwrap();
    let replies = output.join();
    assert_eq!(result.is_ok(), approve, "{result:?}");
    assert_eq!(replies.len(), if approve { 3 } else { 1 });
    assert_eq!(pending.reader.publication().executions, if approve { 3 } else { 0 });
    assert_eq!(pending.reader.publication().control.ledger.epoch, 1);
    assert_eq!(pending.reader.publication().control.ledger.reserved, 0);
    assert_eq!(packets.len(), if approve { 3 } else { 1 });
    if approve {
        assert!(replies.iter().all(executed));
        assert_eq!(pending.reader.publication().payload, b"AA");
        assert!(pending.reader.stream().confirmed.finished());
        assert_eq!(pending.reader.progress(8).numerical.numerical.sampled_draws, 4);
    } else {
        assert!(!executed(&replies[0]));
        assert_eq!(pending.reader.publication().control.ledger.charged, 0);
        pending.reader.no_generation(8);
    }
    assert_eq!(fs::read(&pending.anchor).unwrap(), original_anchor);
    accounting_tests::retained(&root.0.join("recovered.anchor"), &pending.reader);
    assert!(!evaluator.socket_path().exists());
    assert!(!pending.governor.socket.exists());
    assert!(!pending.operator.settings.socket.exists());
    closed(&profiles, &pending.reviewer);
}

#[test]
fn pending_and_completed_governed_native_generation_keep_original_evaluation_and_draws() {
    for steps in [3, 4] {
        continued(steps, true);
    }
    continued(4, false);
}

#[test]
fn both_explicit_recovery_floors_and_effective_policy_bind_before_the_original_fence() {
    for defect in ["policy", "evaluation floor", "governor floor", "campaign"] {
        let root = Root::new();
        let mut pending = pending(&root, 3, false);
        let mut floor = pending.requirements.minimum;
        match defect {
            "policy" => pending.requirements.effective_policy = candidate_policy(),
            "evaluation floor" => floor.authority_epoch += 1,
            "governor floor" => pending.requirements.minimum.authority_epoch += 1,
            "campaign" => pending.requirements.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            _ => unreachable!(),
        }
        let (recovery, history) = recovery(&root, &pending, floor);
        let evaluator = evaluator::Settings::decode(
            accounting_tests::peer_profile(
                &root, &pending.entries[0].actor,
                std::process::id().checked_add(200_000).unwrap(), 2,
            ).as_bytes(),
        ).unwrap();
        let bytes = fs::read(pending.reader.store.join("delivery.bin")).unwrap();
        let profiles = pending.entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let mut output = Vec::new();
        let result = run_accounted_selected(
            pending.config, pending.entries, &pending.reviewer, None,
            (Mode::ContinueGeneration, Some(&pending.operator)), &history,
            andon::Selection {
                settings: &pending.accounting, evaluator: &evaluator,
                recovery: Some(&recovery), paths: None,
            },
            Some(&pending.governor), Some(&pending.requirements),
            || ElapsedTick(1000), &mut output,
        );
        assert!(result.is_err(), "{defect}");
        assert!(output.is_empty());
        assert_eq!(fs::read(pending.reader.store.join("delivery.bin")).unwrap(), bytes, "{defect}");
        assert_eq!(pending.reader.publication().revision, pending.before.revision);
        assert!(!evaluator.socket_path().exists());
        assert!(!pending.governor.socket.exists());
        assert!(!pending.operator.settings.socket.exists());
        closed(&profiles, &pending.reviewer);
    }
}
