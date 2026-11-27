//! Original native policy, numerical and evaluator custody compose in one service.
#[path = "andon/recovery.rs"]
mod recovery;

use super::*;
use super::super::andon_tests::{self as accounting_tests, EvaluatorProcess};
use crate::workflow::actor_service::{andon_settings, evaluator};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    evaluation::andon::FileAndonRecoveryRequirements,
};
use fa_reference::action::consequence::gate::containment::session::policy::Policy;

fn settings(joint: bool) -> andon_settings::Settings {
    if joint {
        andon_settings::Settings::decode(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/fixtures/joint_native_andon_operator.json",
        ))).unwrap()
    } else {
        accounting_tests::settings()
    }
}

fn candidate_policy() -> Policy {
    crate::config::policy_from_json(json(br#"{"generation":2,"nodes":[{"op":"exact_value","key":7,"value_hex":"6f6b"},{"op":"payload_at_most","maximum":256},{"op":"units_at_most","maximum":512},{"op":"all","children":[0,1,2]}]}"#)).unwrap()
}

#[test]
fn original_independent_evaluator_survives_native_policy_promotion_and_finish() {
    for joint in [false, true] {
        for checked in [false, true] {
            evaluated_sequence(ReviewDecision::Approve, joint, checked);
        }
    }
}

#[test]
fn policy_and_evaluation_cannot_replace_the_next_native_human_key() {
    evaluated_sequence(ReviewDecision::Reject, true, true);
}

fn evaluated_sequence(decision: ReviewDecision, jointly: bool, checked: bool) {
    let root = Root::new();
    let (mut config, mut entries, reviewer) = selected(&root, true);
    config.programs = helper_programs(&root);
    let reader = Reader::new(&config, &entries[0]);
    let publication = checked.then(|| checked::publication(&root, &reader));
    let accounting = settings(jointly);
    let peer_store = config.store.clone();
    let peer_profile = config.profile.clone();
    let barrier = barrier(&root, &mut entries[1]);
    let mut child = GovernorChild::start(&root);
    assert_ne!(child.0.id(), std::process::id());
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 4);
    let evaluator = peer.selected.clone();
    let (history, anchor) = accounting_tests::history(
        &root, &config, &entries, &reviewer, Some(&selected_operator),
    );
    let source_policy = config.source_policy;
    let mut output = GovernedOutput::new(&entries);
    output.change = FreshChange::Exact;
    let release_actor = output.released.clone();
    let refreshed = output.refreshed.clone();
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [
            Some(ReviewDecision::Approve),
            None,
            Some(ReviewDecision::Approve),
        ],
    );
    let human_ready = Arc::new(AtomicBool::new(false));
    let release_human = human_ready.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let operator_settings = selected_operator.settings.clone();
    let next_human = reviewer.socket(REQUESTS[1]);
    let scope = entries[0].actor.scope;
    let fresh_source = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope,
            source: 51,
            generation: 2,
        },
        Snapshot {
            semantic_epoch: 2,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    format!("review this fixture: {member}; {FRESH_SOURCE}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode();
    let operator_peer = thread::spawn(move || {
        // Decoder assets use local Rc custody. Reconstruct the same immutable
        // recipe inside this peer instead of sending its reader across threads.
        let inputs = recipe::load(
            &directory.join("recipe.json"),
            peer_profile.delivery.scope.tenant,
            peer_profile.delivery.limits.bytes,
        )
        .unwrap();
        let peer_reader = Reader {
            store: peer_store,
            profile: peer_profile,
            decoder: inputs.decoder,
            tokenizer: inputs.tokenizer,
            stream: inputs.stream,
        };
        wait(&barrier);
        let before = peer_reader.publication();
        assert_eq!(before.executions, 1);
        assert_eq!(before.control.ledger.epoch, 0);
        assert_eq!(before.payload, b"A");
        let numerical = peer_reader.progress(8);
        assert_eq!(
            (
                numerical.numerical.numerical.position,
                numerical.numerical.numerical.sampled_draws
            ),
            (8, 4)
        );
        let retained = fs::read(&source).unwrap();
        assert_ne!(retained, fresh_source);
        fs::remove_file(&source).unwrap();
        let mut socket = operator_tests::connect(&operator_settings);
        let mut request = TowerRequest::new(
            "native-campaign-promotion",
            Address::parse(&format!(
                "fa://{}/run/{}@{GENERATION}",
                scope.tenant, scope.run
            ))
            .unwrap(),
            Verb::Next,
        );
        request.arguments = ReadArguments::PolicyPromotionNext {
            campaign: CAMPAIGN,
            operation: OPERATION,
        };
        let next = encode_request(&request).unwrap();
        let unavailable = operator_exchange(&mut socket, &next);
        assert_eq!(
            field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported")
        );
        write_complete(
            &directory.join("ready"),
            b"second original generation is complete",
        );
        wait(&directory.join("approved"));
        let approved = peer_reader.publication();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(approved.control.ledger.epoch, 0);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let result = operator_exchange(&mut socket, &command);
        operator_known(&result, "policy-promotion");
        let after = peer_reader.publication();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.ledger.epoch, 1);
        assert_eq!(
            (after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged)
        );
        assert!(
            !source.exists(),
            "promotion must not acquire current source"
        );
        let (accepted, receipt) = peer.operation("label", REQUESTS[0]);
        assert!(accepted, "{}", String::from_utf8_lossy(&receipt));
        assert_eq!(field(&json(&receipt), "status").as_str(), Some("applied"));
        assert!(!source.exists(), "old-policy evaluation must not refresh the source");
        assert_eq!(peer_reader.publication().executions, 1);

        let started = Instant::now();
        while !refreshed.load(Ordering::Acquire) {
            assert!(
                started.elapsed() < Duration::from_secs(25),
                "promotion failed to refresh the ready source reference"
            );
            pause(1);
        }
        assert!(
            !barrier.exists(),
            "the old exact-predicate disclosure survived the epoch change"
        );
        write_complete(&source, fresh_source);
        release_actor.store(true, Ordering::Release);
        wait(&next_human);
        let awaiting = peer_reader.publication();
        assert_eq!(
            awaiting.executions, 1,
            "governor approval is not a human effect key"
        );
        let (accepted, receipt) = peer.operation("label", REQUESTS[1]);
        assert!(accepted, "{}", String::from_utf8_lossy(&receipt));
        assert_eq!(field(&json(&receipt), "status").as_str(), Some("applied"));
        assert_eq!(peer_reader.publication().executions, 1);
        peer.finish();
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), result);
        assert_eq!(
            fs::read(directory.join("store/delivery.bin")).unwrap(),
            canonical,
            "promotion retry disturbed the fresh review"
        );
        release_human.store(true, Ordering::Release);
    });
    let result = run_accounted_selected(
        config,
        entries,
        &reviewer,
        publication.as_ref().map(|(profile, _)| profile),
        (Mode::Create, Some(&selected_operator)),
        &history,
        andon::Selection {
            settings: &accounting, evaluator: &evaluator, recovery: None, paths: None,
        },
        Some(&governor),
        None,
        || {
            if human_ready.load(Ordering::Acquire) {
                humans.decisions[1] = Some(decision);
            }
            humans.poll();
            ElapsedTick(1000)
        },
        &mut output,
    );
    operator_peer.join().unwrap();
    child.finish();
    let references = output
        .documents
        .iter()
        .map(|document| {
            let Command::Submit { request, proposal } = decode_command(document).unwrap() else {
                unreachable!()
            };
            (request, proposal)
        })
        .collect::<Vec<_>>();
    let old = references
        .iter()
        .find(|(request, proposal)| *request == REQUESTS[1] && proposal.expected_policy_epoch == 0)
        .unwrap();
    let fresh = references
        .iter()
        .find(|(request, proposal)| *request == REQUESTS[1] && proposal.expected_policy_epoch == 1)
        .unwrap();
    assert_eq!(old.1.payload, fresh.1.payload);
    assert_eq!(
        (old.1.target, old.1.units, old.1.deadline),
        (fresh.1.target, fresh.1.units, fresh.1.deadline)
    );
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]));
    let first = packets[0].as_ref().unwrap();
    assert_eq!(first.action().spec().policy_epoch, 0);
    let second = packets[1].as_ref().unwrap();
    assert_ne!(first.binding().session, second.binding().session);
    assert_eq!(second.action().spec().policy_epoch, 1);
    assert_eq!(second.policy_generation(), 2);
    assert_eq!(second.views().len(), 2);
    let second_frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
    assert_eq!(second_frame.prior_messages(), &["A"]);
    assert_eq!(second_frame.message(), Some("A"));
    let stream = reader.stream();
    if decision == ReviewDecision::Approve {
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(replies.len(), 3);
        assert!(replies.iter().all(executed));
        assert_eq!(
            (
                stream.publication.executions,
                stream.publication.payload.as_slice()
            ),
            (3, b"AA".as_slice())
        );
        assert!(stream.confirmed.finished());
        assert_eq!(
            stream.publication.control.ledger.charged,
            packets
                .iter()
                .map(|packet| packet.as_ref().unwrap().action().spec().units)
                .sum::<u64>()
        );
        let finish = packets[2].as_ref().unwrap();
        assert_eq!(finish.action().spec().policy_epoch, 1);
        assert_eq!(finish.policy_generation(), 2);
        assert!(
            ReleaseFrame::decode(&finish.action().spec().payload)
                .unwrap()
                .is_finish()
        );
    } else {
        assert!(result.is_err());
        assert_eq!(replies.len(), 2);
        assert!(!executed(&replies[1]));
        assert!(packets[2].is_none());
        assert_eq!(
            (
                stream.publication.executions,
                stream.publication.payload.as_slice()
            ),
            (1, b"A".as_slice())
        );
        assert!(!stream.confirmed.finished());
        assert_eq!(
            stream.publication.control.ledger.charged,
            first.action().spec().units
        );
    }
    assert_eq!(stream.publication.control.ledger.epoch, 1);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.available + stream.publication.control.ledger.charged,
        reader.profile.delivery.total
    );
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.pending.is_none());
    let numerical = reader.progress(8);
    assert_eq!(
        (
            numerical.numerical.numerical.position,
            numerical.numerical.numerical.sampled_draws
        ),
        (8, 4),
        "policy change must not regenerate a ready Message or replenish work"
    );
    reader.no_generation(9);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    assert!(!evaluator.socket_path().exists());
    accounting_tests::retained(&anchor, &reader);
    let base = FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(reader.stream), decoder: Some(reader.decoder.clone()),
            decoder_stop: None, source: Some(source_policy), identity: None,
            campaigns: Some(governor.campaign_requirement()), credential: None,
        },
        effective_policy: candidate_policy(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: stream.publication.revision,
            control_sequence: stream.publication.control.sequence,
            authority_epoch: stream.publication.control.ledger.epoch,
        },
    };
    let mut expected = FileAndonRecoveryRequirements::new(
        base, accounting.config.clone(), accounting.windows.clone(), 0,
    ).unwrap();
    if let Some(policy) = accounting.joint {
        expected = expected.require_joint(policy).unwrap();
    }
    if let Some((publication, _)) = &publication {
        expected = expected.require_publication(publication.limits).unwrap();
    }
    let retained = FileHistoryAnchor::read_trusted(
        &mut std::io::Cursor::new(fs::read(anchor).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    let snapshot = FileOversight::read_andon_guarded(
        &reader.store, &reader.profile, &expected, &accounting.protocol, &retained,
    ).unwrap();
    assert_eq!(snapshot.credibility.report.retained_cases,
        if decision == ReviewDecision::Approve { 3 } else { 2 });
    assert_eq!(snapshot.credibility.report.benign_origins, 1);
    assert_eq!(snapshot.credibility.report.policy_generation, 2);
    assert_eq!(snapshot.accounting.windows()[&1].false_stops, 0);
    closed(&profiles, &reviewer);
}


#[test]
fn governor_and_evaluator_credentials_and_selected_custody_are_independent_before_work() {
    for defect in ["same peer", "same socket", "archive", "contract", "missing governor", "missing operator"] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let governor_pid = std::process::id().checked_add(10_000).unwrap();
        let (mut governor, selected_operator) = selections(&root, &entries[0].actor, governor_pid);
        let evaluator_pid = std::process::id().checked_add(20_000).unwrap();
        let mut evaluator = evaluator::Settings::decode(
            accounting_tests::peer_profile(&root, &entries[0].actor, evaluator_pid, 4).as_bytes(),
        ).unwrap();
        let accounting = settings(false);
        let paths = andon::Paths {
            settings: root.0.join("selected-andon.json"),
            evaluator: root.0.join("selected-evaluator.json"),
            recovery: None,
            mode: Mode::Create,
        };
        let (history, anchor) = accounting_tests::history(
            &root, &config, &entries, &reviewer, Some(&selected_operator),
        );
        match defect {
            "same peer" => evaluator.evaluator.pid = Some(governor_pid),
            "same socket" => evaluator.socket = governor.socket.clone(),
            "archive" => governor.socket = anchor.clone(),
            "contract" => governor.socket = paths.settings.clone(),
            _ => {}
        }
        let store = config.store.clone();
        let source_path = config.source.path().to_path_buf();
        let source = fs::read(&source_path).unwrap();
        let mut output = Vec::new();
        let result = run_accounted_selected(
            config, entries, &reviewer, None,
            (Mode::Create, (defect != "missing operator").then_some(&selected_operator)),
            &history,
            andon::Selection {
                settings: &accounting, evaluator: &evaluator, recovery: None, paths: Some(&paths),
            },
            (defect != "missing governor").then_some(&governor), None,
            || panic!("invalid evaluator/governor selection reached a clock"), &mut output,
        );
        assert!(result.is_err(), "{defect}");
        assert!(output.is_empty() && !store.exists() && !anchor.exists());
        assert_eq!(fs::read(source_path).unwrap(), source);
        assert!(!evaluator.socket_path().exists());
        assert!(!governor.socket.exists());
        assert!(!selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn evaluated_governance_grammar_keeps_both_recovery_contracts_and_original_modes_explicit() {
    let base = [
        "serve-native-generated", "config", "sequence", "reviewer",
        "--andon", "andon", "--evaluator-profile", "evaluator",
        "--retain-history-anchor", "new-anchor",
        "--operator-profile", "operator", "--tower-generation", "61",
        "--governor-profile", "governor",
    ].map(str::to_owned).to_vec();
    let (_, created) = andon::take_options(&base, None).unwrap();
    assert_eq!(created.unwrap().mode, Mode::Create);
    for mode in [vec!["--continue-generation"], vec!["--after-message", "91"]] {
        let mut recovered = base.clone();
        recovered.extend(mode.iter().map(|word| (*word).to_owned()));
        recovered.extend([
            "--history-anchor", "old-anchor", "--evaluation-recovery", "evaluation-floors",
            "--recovery-requirements", "policy-and-guard-requirements",
        ].map(str::to_owned));
        assert!(andon::take_options(&recovered, None).unwrap().1.unwrap().mode.is_recovery());
        for missing in ["--evaluation-recovery", "--recovery-requirements", "--history-anchor"] {
            let mut incomplete = recovered.clone();
            let position = incomplete.iter().position(|word| word == missing).unwrap();
            incomplete.drain(position..position + 2);
            assert!(andon::take_options(&incomplete, None).is_err(), "{missing}");
        }
        let mut mixed = recovered.clone();
        mixed.extend(["--after-message".into(), "92".into()]);
        assert!(andon::take_options(&mixed, None).is_err());
    }
}

#[test]
fn original_stop_retires_both_governance_and_evaluator_custody_before_more_native_work() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let reader = Reader::new(&config, &entries[0]);
    let barrier = barrier(&root, &mut entries[1]);
    let child = GovernorChild::start(&root);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let evaluator = evaluator::Settings::decode(
        accounting_tests::peer_profile(
            &root, &entries[0].actor, std::process::id().checked_add(200_000).unwrap(), 1,
        ).as_bytes(),
    ).unwrap();
    let accounting = settings(false);
    let (history, anchor) = accounting_tests::history(
        &root, &config, &entries, &reviewer, Some(&selected_operator),
    );
    let mut output = GovernedOutput::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let mut controller = None;
    let result = run_accounted_selected(
        config,
        entries,
        &reviewer,
        None,
        (Mode::Create, Some(&selected_operator)),
        &history,
        andon::Selection {
            settings: &accounting, evaluator: &evaluator, recovery: None, paths: None,
        },
        Some(&governor),
        None,
        || {
            humans.poll();
            if controller.is_none() && barrier.exists() {
                assert!(governor.socket.exists());
                assert!(evaluator.socket_path().exists());
                assert!(selected_operator.settings.socket.exists());
                assert_eq!(reader.publication().executions, 1);
                controller = Some(stopping(reviewer.clone()));
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(result.is_err());
    let receipt = controller
        .expect("completed native draft must reach its original actor boundary")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, REQUESTS[0]);
    let packets = humans.join();
    let replies = output.join();
    assert_eq!(replies.len(), 1);
    assert!(executed(&replies[0]));
    assert!(packets[1].is_none() && packets[2].is_none());
    let after = reader.publication();
    assert_eq!(after.stop.unwrap().request().operation, REQUESTS[0]);
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"A");
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    assert!(!evaluator.socket_path().exists());
    accounting_tests::retained(&anchor, &reader);
    closed(&profiles, &reviewer);
}
