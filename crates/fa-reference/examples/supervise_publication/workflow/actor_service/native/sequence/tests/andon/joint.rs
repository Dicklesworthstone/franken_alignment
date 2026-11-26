//! Explicit joint Andon selection reaches original generated publication and
//! retains one independently authenticated evaluator and paid operator across
//! the resident sequence. Benign fixture labels do not qualify a detector.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    evaluation::andon::FileAndonRecoveryRequirements,
};
use fa_reference::action::consequence::oversight::joint_credibility::JointPromotionPolicy;
use fa_reference::tower::{ReadArguments, TowerRequest};

const PROMOTION: u64 = 501;

fn joint_settings() -> andon_settings::Settings {
    andon_settings::Settings::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/joint_native_andon_operator.json"
    )))
    .unwrap()
}

fn promotion_operator(root: &Root, actor: &Profile) -> operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let settings = crate::workflow::actor_service::operator::Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":2,"connection_limit":2,"exchange_limit":32,
        "runtime_ms":15000,"poll_ms":1,"response_bytes":65536,
        "inspection_queries":0,"explanation_queries":0,"explanation_replay_events":0,
        "explanation_disclosure":"none",
        "credibility_promotion":{{"queries":3,"max_replay_events":4096,
        "numerical_replay_events":4096,"authority":"promote_qualified_credibility"}}}}"#,
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            root.0.display(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
        )
        .as_bytes(),
    )
    .unwrap();
    operator::Selection::new(settings, 41).unwrap()
}

fn next(actor: &Profile, id: &str, operation: u64) -> TowerRequest {
    let mut query = request(actor, id, Verb::Next);
    query.arguments = ReadArguments::CredibilityPromotionNext { operation };
    query
}

fn plan<'a>(reply: &'a Json, can_promote: bool, reason: Option<&str>) -> &'a Json {
    assert_eq!(
        field(reply, &["result", "kind"]).as_str(),
        Some("credibility-promotion-next")
    );
    let known = field(reply, &["result", "value", "knowledge"]);
    assert_eq!(field(known, &["state"]).as_str(), Some("known"));
    assert_eq!(
        field(known, &["value", "can_promote"]).as_bool(),
        Some(can_promote)
    );
    assert_eq!(field(known, &["value", "block_reason"]).as_str(), reason);
    known
}

fn expected(
    config: &Config,
    entry: &Entry,
    accounting: &andon_settings::Settings,
) -> FileAndonRecoveryRequirements {
    let requirements = FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(entry.inputs.stream),
                decoder: Some(entry.inputs.decoder.clone()),
                decoder_stop: None,
                source: Some(config.source_policy),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: config.profile.delivery.policy.clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: 0,
                control_sequence: 0,
                authority_epoch: 0,
            },
        },
        accounting.config.clone(),
        accounting.windows.clone(),
        accounting.minimum_evaluation_revision,
    )
    .unwrap();
    requirements
        .require_joint(accounting.joint.unwrap())
        .unwrap()
}

#[test]
fn live_native_promotion_requires_joint_selection_and_paid_numerical_replay_before_startup() {
    for missing_joint in [true, false] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let mut accounting = joint_settings();
        let mut observer = promotion_operator(&root, &entries[0].actor);
        if missing_joint {
            accounting.joint = None;
        } else {
            observer
                .settings
                .credibility_promotion
                .as_mut()
                .unwrap()
                .numerical_replay_events = None;
        }
        let evaluator = evaluator::Settings::decode(
            peer_profile(
                &root,
                &entries[0].actor,
                std::process::id().checked_add(100_000).unwrap(),
                8,
            )
            .as_bytes(),
        )
        .unwrap();
        let (history, anchor) = history(&root, &config, &entries, &reviewer, Some(&observer));
        let source = fs::read(config.source.path()).unwrap();
        let source_path = config.source.path().to_path_buf();
        let store = config.store.clone();
        let profiles = entries
            .iter()
            .map(|entry| entry.actor.clone())
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        let result = run_with_andon(
            config,
            entries,
            &reviewer,
            None,
            Some(&observer),
            &history,
            &accounting,
            &evaluator,
            || panic!("invalid joint promotion selection observed time"),
            &mut output,
        );
        assert!(result.is_err());
        assert!(output.is_empty());
        assert!(!store.exists() && !anchor.exists());
        assert_eq!(fs::read(source_path).unwrap(), source);
        assert!(!evaluator.socket_path().exists());
        assert!(!observer.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

fn joint_sequence(approve_finish: bool) {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, true);
    let accounting = joint_settings();
    let reader = Reader::new(&config, &entries[0]);
    let expected = expected(&config, &entries[0], &accounting);
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 6);
    let evaluator = peer.selected.clone();
    let observer = promotion_operator(&root, &entries[0].actor);
    let (history, anchor) = history(&root, &config, &entries, &reviewer, Some(&observer));
    let observer_settings = observer.settings.clone();
    let actor = entries[0].actor.clone();
    let store = reader.store.clone();
    let original_profile = reader.profile.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
    let independent = thread::spawn(move || {
        let mut socket = connect(&observer_settings);
        let mut evaluator_inode = None;
        let operator_inode = fs::metadata(&observer_settings.socket).unwrap().ino();
        for (index, request_id) in REQUESTS.into_iter().enumerate() {
            let (offered, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(offered, request_id);
            assert_eq!(packet.action().spec().policy_epoch, 0);
            if index == 0 {
                evaluator_inode = Some(fs::metadata(peer.selected.socket_path()).unwrap().ino());
            }
            assert_eq!(
                fs::metadata(peer.selected.socket_path()).unwrap().ino(),
                evaluator_inode.unwrap(),
                "a handoff replaced the original independently authenticated evaluator"
            );
            assert_eq!(
                fs::metadata(&observer_settings.socket).unwrap().ino(),
                operator_inode
            );
            let before = FileOversight::read_publication(&store, &original_profile).unwrap();
            assert_eq!(before.executions, index as u64);
            let reply = exchange(
                &mut socket,
                &next(&actor, &format!("joint-next-{index}"), PROMOTION),
                observer_settings.response_bytes,
            );
            plan(&reply, false, Some("marginal_unqualified"));
            // Only the independent child's real peer transport can label the
            // original ticket. Benign-only evidence must stay unqualified.
            let (accepted, receipt) = peer.operation("label", request_id);
            assert!(accepted, "{}", String::from_utf8_lossy(&receipt));
            assert_eq!(
                field(&parse(&receipt), &["status"]).as_str(),
                Some("applied")
            );
            let after = FileOversight::read_publication(&store, &original_profile).unwrap();
            assert_eq!(after.executions, before.executions);
            assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
            assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
            if index == 2 {
                let exhausted = exchange(
                    &mut socket,
                    &next(&actor, "joint-query-exhausted", PROMOTION),
                    observer_settings.response_bytes,
                );
                assert_eq!(
                    field(&exhausted, &["result", "value", "error", "code"]).as_str(),
                    Some("capacity"),
                    "neither Message nor Finish may replenish the paid grant"
                );
            }
            release
                .send(Some(if index == 2 && !approve_finish {
                    ReviewDecision::Reject
                } else {
                    ReviewDecision::Approve
                }))
                .unwrap();
        }
        peer.finish();
    });
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let result = run_with_andon(
        config,
        entries,
        &reviewer,
        None,
        Some(&observer),
        &history,
        &accounting,
        &evaluator,
        crate::workflow::clock,
        &mut output,
    );
    assert_eq!(result.is_ok(), approve_finish, "{result:?}");
    independent.join().unwrap();
    let packets = human.join().unwrap();
    let replies = output.join();
    assert_eq!(packets.len(), 3);
    assert_eq!(replies.len(), 3);
    let mut sessions = BTreeSet::new();
    for (index, (packet, reply)) in packets.iter().zip(&replies).enumerate() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.views().len(), 2);
        assert_eq!(packet.action().spec().policy_epoch, 0);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
        assert_eq!(frame.is_finish(), index == 2);
        assert_eq!(executed(reply), index < 2 || approve_finish);
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 2 + u64::from(approve_finish));
    assert_eq!(stream.publication.payload, b"AA");
    assert_eq!(stream.publication.control.ledger.epoch, 0);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.charged,
        packets
            .iter()
            .take(if approve_finish { 3 } else { 2 })
            .map(|packet| packet.action().spec().units)
            .sum::<u64>(),
    );
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.confirmed.finished(), approve_finish);
    assert!(stream.pending.is_none());
    let progress = reader.progress(8);
    assert_eq!(progress.numerical.numerical.position, 8);
    assert_eq!(progress.numerical.numerical.sampled_draws, 4);
    reader.no_generation(9);
    retained(&anchor, &reader);
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(anchor).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    let saved = FileOversight::read_andon_guarded(
        &reader.store,
        &reader.profile,
        &expected,
        &accounting.protocol,
        &anchor,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.retained_cases, 3);
    assert_eq!(saved.credibility.report.pending_cases, 0);
    assert_eq!(saved.credibility.report.benign_origins, 3);
    assert_eq!(saved.credibility.report.violation_origins, 0);
    assert!(!saved.credibility.report.qualified());
    assert!(saved.credibility.promotions.is_empty());
    assert!(saved.accounting.windows().values().all(|window| {
        window.false_stops == 0 && window.total_cost == 0 && window.incident.is_none()
    }));
    assert!(!evaluator.socket_path().exists());
    assert!(!observer.settings.socket.exists());
    closed(&profiles, &reviewer);
}

#[test]
fn resident_joint_native_generation_keeps_original_labels_human_keys_and_paid_operator() {
    joint_sequence(true);
}

#[test]
fn resident_joint_native_finish_still_requires_its_fresh_human_key() {
    joint_sequence(false);
}

fn recovered_finish(root: &Root) -> (Config, Vec<Entry>, PeerProfile) {
    let mut config = fixture::configured(root);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    let (mut actor, reviewer) = profiles(root, &config);
    actor.request = 93;
    actor.socket = root.0.join("actor-93.sock");
    let mut inputs = recipe::load(
        &root.0.join("recipe.json"),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
    )
    .unwrap();
    inputs.generation = 8;
    (
        config,
        vec![Entry {
            actor,
            inputs,
            release: Release::Finish,
            precheck: None,
        }],
        reviewer,
    )
}

fn recovered_history(
    root: &Root,
    config: &Config,
    entries: &[Entry],
    reviewer: &PeerProfile,
    input: &Path,
) -> anchors::Selection {
    let arguments = [
        "serve-native-generated".to_owned(),
        "config".to_owned(),
        "sequence".to_owned(),
        "reviewer".to_owned(),
        "--after-message".to_owned(),
        "92".to_owned(),
        "--history-anchor".to_owned(),
        input.display().to_string(),
        "--retain-history-anchor".to_owned(),
        root.0.join("resumed.anchor").display().to_string(),
    ];
    anchors::take_options(&arguments)
        .unwrap()
        .1
        .admit(
            config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            reviewer,
            None,
            Some(92),
            None,
        )
        .unwrap()
}

fn changed_joint(original: JointPromotionPolicy, fault: usize) -> Option<JointPromotionPolicy> {
    if fault == 0 {
        return None;
    }
    let mut id = original.id();
    let mut generation = original.generation();
    let mut escape = original.maximum_escape_rate();
    let mut benign = original.maximum_benign_stop_rate();
    let mut budget = original.budget();
    match fault {
        1 => id += 1,
        2 => generation += 1,
        3 => escape.numerator = 1,
        4 => escape.denominator += 1,
        5 => benign.numerator = 0,
        6 => benign.denominator += 1,
        7 => budget.cases -= 1,
        8 => budget.member_outcomes -= 1,
        _ => unreachable!(),
    }
    Some(JointPromotionPolicy::new(id, generation, escape, benign, budget).unwrap())
}

#[test]
fn joint_native_recovery_requires_every_original_policy_field_before_fence_and_fresh_finish() {
    use super::super::super::evaluation::Recovery;

    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, false);
    entries.truncate(2);
    let accounting = joint_settings();
    let reader = Reader::new(&config, &entries[0]);
    // Separate subprocess root avoids reusing completion markers on recovery.
    let peer_root = Root::new();
    let mut peer = Some(EvaluatorProcess::start(&peer_root, &entries[0].actor, 1));
    let evaluator = peer.as_ref().unwrap().selected.clone();
    let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let mut output = Output::new(&entries);
    let original_profiles = output.profiles.clone();
    let mut discovery = None;
    let result = run_with_andon(
        config,
        entries,
        &reviewer,
        None,
        None,
        &history,
        &accounting,
        &evaluator,
        || {
            humans.poll();
            if reviewer.socket(92).exists() && discovery.is_none() {
                let mut peer = peer.take().unwrap();
                discovery = Some(thread::spawn(move || {
                    let (accepted, response) = peer.operation("read", 91);
                    assert!(accepted, "{}", String::from_utf8_lossy(&response));
                    assert_eq!(
                        field(&parse(&response), &["knowledge", "state"]).as_str(),
                        Some("known")
                    );
                    peer.finish();
                }));
            }
            crate::workflow::clock()
        },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    discovery
        .expect("original second human boundary")
        .join()
        .unwrap();
    assert!(output.join().iter().all(executed));
    assert_eq!(humans.join().into_iter().flatten().count(), 2);
    closed(&original_profiles, &reviewer);
    let before = reader.publication();
    assert_eq!(before.executions, 2);
    assert_eq!(before.payload, b"AA");
    assert_eq!(before.control.ledger.epoch, 0);
    let numerical = reader.progress(8).numerical.numerical;
    assert_eq!(numerical.position, 8);
    assert_eq!(numerical.sampled_draws, 4);
    retained(&anchor, &reader);
    let original_anchor = fs::read(&anchor).unwrap();
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    let path = root.0.join("evaluation-recovery.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        before.revision, before.control.sequence, before.control.ledger.epoch,
    )).unwrap();

    // None means the original marginal-only contract, not permission to infer
    // a joint policy from disk. Even equal-valued but differently represented
    // fractions and independently selected replay budgets must match exactly.
    for fault in 0..9 {
        let (config, entries, reviewer) = recovered_finish(&root);
        let history = recovered_history(&root, &config, &entries, &reviewer, &anchor);
        let requirements = Recovery::read(&path, &config).unwrap();
        let mut changed = joint_settings();
        changed.minimum_evaluation_revision = 2;
        changed.joint = changed_joint(accounting.joint.unwrap(), fault);
        let evaluator = evaluator::Settings::decode(
            peer_profile(
                &root,
                &entries[0].actor,
                std::process::id().checked_add(100_000).unwrap(),
                2,
            )
            .as_bytes(),
        )
        .unwrap();
        let profiles = entries
            .iter()
            .map(|entry| entry.actor.clone())
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        let result = run_with_andon_recovery(
            config,
            entries,
            &reviewer,
            None,
            (92, None),
            &history,
            &changed,
            &evaluator,
            &requirements,
            crate::workflow::clock,
            &mut output,
        );
        assert!(
            result.is_err(),
            "accepted changed original joint field {fault}"
        );
        assert!(output.is_empty());
        assert_eq!(
            fs::read(reader.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(fs::read(&anchor).unwrap(), original_anchor);
        assert_eq!(reader.progress(8).numerical.numerical, numerical);
        assert!(!history.output_path().unwrap().exists());
        assert!(!evaluator.socket_path().exists());
        closed(&profiles, &reviewer);
    }

    // The exact original joint contract takes the normal native recovery path,
    // fences historical keys once and retains the evaluator for old and new cases.
    let (config, entries, reviewer) = recovered_finish(&root);
    let history = recovered_history(&root, &config, &entries, &reviewer, &anchor);
    let requirements = Recovery::read(&path, &config).unwrap();
    let mut accounting = joint_settings();
    accounting.minimum_evaluation_revision = 2;
    let expected = expected(&config, &entries[0], &accounting);
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 4);
    let evaluator = peer.selected.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), vec![93], offered, decisions);
    let store = reader.store.clone();
    let original_profile = reader.profile.clone();
    let epoch = before.control.ledger.epoch + 1;
    let independent = thread::spawn(move || {
        let (offered, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
        assert_eq!(offered, 93);
        assert_eq!(packet.action().spec().policy_epoch, epoch);
        let inode = fs::metadata(peer.selected.socket_path()).unwrap().ino();
        for round in [91, 93] {
            assert_eq!(
                fs::metadata(peer.selected.socket_path()).unwrap().ino(),
                inode
            );
            let (accepted, receipt) = peer.operation("label", round);
            assert!(accepted, "{}", String::from_utf8_lossy(&receipt));
            assert_eq!(
                field(&parse(&receipt), &["status"]).as_str(),
                Some("applied")
            );
        }
        let publication = FileOversight::read_publication(&store, &original_profile).unwrap();
        assert_eq!(
            publication.executions, 2,
            "neither label replaces the fresh human key"
        );
        assert_eq!(publication.control.ledger.epoch, epoch);
        release.send(Some(ReviewDecision::Approve)).unwrap();
        peer.finish();
    });
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let result = run_with_andon_recovery(
        config,
        entries,
        &reviewer,
        None,
        (92, None),
        &history,
        &accounting,
        &evaluator,
        &requirements,
        crate::workflow::clock,
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    independent.join().unwrap();
    let packets = human.join().unwrap();
    assert_eq!(packets.len(), 1);
    assert!(output.join().iter().all(executed));
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 3);
    assert_eq!(stream.publication.payload, b"AA");
    assert_eq!(stream.publication.control.ledger.epoch, epoch);
    assert_eq!(
        stream.publication.control.ledger.charged,
        before.control.ledger.charged + packets[0].action().spec().units,
    );
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    assert_eq!(reader.progress(8).numerical.numerical, numerical);
    reader.no_generation(9);
    assert_eq!(fs::read(&anchor).unwrap(), original_anchor);
    let final_anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(history.output_path().unwrap()).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(final_anchor.revision(), stream.publication.revision);
    let saved = FileOversight::read_andon_guarded(
        &reader.store,
        &reader.profile,
        &expected,
        &accounting.protocol,
        &final_anchor,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.retained_cases, 3);
    assert_eq!(saved.credibility.report.pending_cases, 1);
    assert_eq!(saved.credibility.report.benign_origins, 2);
    assert_eq!(saved.credibility.report.revision, 5);
    assert!(!evaluator.socket_path().exists());
    closed(&profiles, &reviewer);
}
