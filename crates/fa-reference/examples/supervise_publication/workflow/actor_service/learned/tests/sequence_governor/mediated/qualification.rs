//! Governed recovery retains the original joint policy, numerical history and
//! source custody. Synthetic labels exercise enforcement; they do not establish
//! evaluator independence or deployment calibration.
use super::*;
use crate::workflow::actor_service::learned::qualification as qualify;
use crate::workflow::actor_service::learned::tests::qualification::activation;
use fa_reference::action::consequence::delivery::persistent::{
    credibility::CredibilityActivation, governance::PolicyUpdate,
};

const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":2,"max_member_outcomes":8}"#;
const CHILD_ROOT: &str = "FA_LEARNED_JOINT_GOVERNOR_ROOT";
const CHILD_EPOCH: &str = "FA_LEARNED_JOINT_GOVERNOR_EPOCH";
const CHILD_TEST: &str = "workflow::actor_service::learned::tests::resident_sequence::governor::mediated::qualification::independent_joint_governor_process";

fn capsule(root: &Root, request: &CredibilityActivation) -> PathBuf {
    let path = root.0.join(format!("joint-{}.bin", request.operation));
    fs::write(&path, request.encode_reference().unwrap()).unwrap();
    path
}

struct Seed {
    config: Config,
    paths: Vec<PathBuf>,
    entries: Vec<resident::Entry>,
    before: FileDeliverySnapshot,
    old_capsule: PathBuf,
}

fn prepared(root: &Root, mediated: bool) -> Seed {
    let (mut config, paths, mut entries) = if mediated {
        selected_mediated(root)
    } else {
        selected(root, None, None)
    };
    for (path, entry) in paths.iter().zip(&mut entries) {
        let original = fs::read_to_string(path).unwrap();
        fs::write(path, format!(
            r#"{{"schema":"fa.learned-joint-publication/1","joint":{JOINT},"recipe":{original}}}"#
        )).unwrap();
        entry.loaded = recipe::load(path, &config, true).unwrap();
    }
    let first = &mut entries[0].loaded;
    campaigns::admit_selected(first, requirement()).unwrap();
    first.anchor_output = Some(root.0.join("history-1.bin"));
    if mediated {
        topology::admit(
            &config,
            first,
            Some(&root.0.join("create-topology.json")),
            topology::Mode::Create,
        )
        .unwrap();
    }
    let limit = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    let topology::Started {
        custody: (host, reviewer, observer, _identity),
        source,
        output,
    } = topology::start(&config, first, false, &mut control, &limit, &mut || {
        ElapsedTick(1000)
    })
    .unwrap();
    assert!(observer.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    topology::attach(&mut driver, source).unwrap();
    topology::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
    assert!(
        generate(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &limit,
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        first,
        &limit,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let (mut driver, history) =
        native::review_current(&mut config, first, &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    let (old_capsule, before) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let state = host.inspect();
        let request = activation(
            &config,
            state.control.sequence,
            state.control.ledger.epoch,
            20,
            2,
        );
        let revision = host.revision();
        host.activate_credibility(revision, request.clone(), &config.profile.committee)
            .unwrap();
        host.check_credibility().unwrap();
        assert!(host.held_out_joint_report(20).unwrap().unwrap().qualified());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (4, 2));
        (capsule(root, &request), host.inspect())
    };
    assert!(topology::retain(output.as_ref(), &driver, None).is_none());
    assert!(anchors::retain(first, &driver, None).is_none());
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(history);
    drop(wire);
    assert_eq!(before.executions, 1);
    assert_eq!(before.payload, b"A");
    if mediated {
        let saved = checkpoint(&root.0.join("source-1.bin"));
        assert_eq!(saved.work().checks, 2);
        assert_eq!(
            saved.history_anchor(),
            &anchors::load(&root.0.join("history-1.bin")).unwrap()
        );
    }
    recovery_recipes(&paths[1..]);
    if mediated {
        next_source(root);
    }
    quiescent(root);
    Seed {
        config,
        paths,
        entries,
        before,
        old_capsule,
    }
}

fn selected_recovery(root: &Root, seed: &Seed, mediated: bool) -> Vec<String> {
    let mut args = recovered_args(
        inputs(
            root,
            &seed.config,
            &seed.paths[1..],
            &seed.entries[1..],
            None,
        ),
        root,
    );
    if mediated {
        let path = archived(
            root,
            &root.0.join("source-1.bin"),
            2,
            Some(&root.0.join("source-2.bin")),
        );
        args.extend(["--topology-profile".into(), path.display().to_string()]);
    }
    args
}

#[test]
fn independent_joint_governor_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let epoch = std::env::var(CHILD_EPOCH).unwrap();
    wait(&root.join("ready"));
    let selected = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(selected.scope);
    let inspected = exchange(
        &root,
        "get.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"get","query":"1"}}"#
        ),
    );
    let cut = known(&inspected, "inspect");
    assert_eq!(field(cut, "authority_epoch").as_str(), Some(epoch.as_str()));
    assert!(!root.join("evidence.json").exists());
    if root.join("initial.json").exists() {
        assert!(!root.join("inventory-2.json").exists());
    }
    let reviewed = exchange(
        &root,
        "review.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{CAMPAIGN}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{},"policy_generation":"1"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{{"generation":2,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":256}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}}}}}"#,
            live_governor::protocol::json(field(cut, "journal_revision")),
            live_governor::protocol::json(field(cut, "control_sequence")),
            live_governor::protocol::json(field(cut, "authority_epoch"))
        ),
    );
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(1));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    let approved = exchange(
        &root,
        "approve.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
            live_governor::protocol::json(field(value, "journal_revision")),
            live_governor::protocol::quote(report)
        ),
    );
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "approval").as_str(), Some("delegated"));
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    write_complete(
        &root.join("approved"),
        live_governor::protocol::json(&approved),
    );
}

#[test]
fn governed_joint_recovery_requalifies_once_and_retains_fresh_message_and_finish_keys() {
    for mediated in [false, true] {
        for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
            live_requalification(mediated, decision);
        }
    }
}

fn live_requalification(mediated: bool, decision: ReviewDecision) {
    let root = Root::new();
    let seed = prepared(&root, mediated);
    let input_history = fs::read(root.0.join("history-1.bin")).unwrap();
    let input_source = mediated.then(|| fs::read(root.0.join("source-1.bin")).unwrap());
    // Original Fence, optional topology withdrawal/rebind, then one activation.
    let epoch = seed.before.control.ledger.epoch + if mediated { 4 } else { 2 };
    let mut child = GovernorChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env(CHILD_ROOT, &root.0)
            .env(CHILD_EPOCH, epoch.to_string())
            .spawn()
            .unwrap(),
    );
    assert_ne!(child.0.id(), std::process::id());
    selections(&root, &seed.entries[1].actor, child.0.id());
    let args = selected_recovery(&root, &seed, mediated);
    let barrier = root.0.join("peers/joint-draft.sock");
    let precheck = root.0.join("joint-precheck.json");
    fs::write(&precheck, format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":{epoch},"disclosure_id":91,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":60000}}"#,
        quote(barrier.to_str().unwrap()))).unwrap();
    let manifest = root.0.join("sequence.json");
    let raw = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        raw.replacen(
            "\"precheck_profile\":null",
            &format!("\"precheck_profile\":{}", quote(precheck.to_str().unwrap())),
            1,
        ),
    )
    .unwrap();
    let mut peers = Peers::new(&root, &seed.config, &seed.entries);
    let reader = Reader::new(&root, &seed.paths[1]);
    let capsule_path = root.0.join("joint-21.bin");
    let ready = Arc::new(AtomicBool::new(false));
    let human_ready = Arc::new(AtomicBool::new(false));
    let release_actor = ready.clone();
    let release_human = human_ready.clone();
    let directory = root.0.clone();
    let source = seed.config.source.path().to_owned();
    let next_human = peers.reviewer.socket(2);
    let source_recipe = seed.paths[1].clone();
    let governor_peer = thread::spawn(move || {
        let config = Config::read_learned_recovery(&directory.join("command-host.json")).unwrap();
        let loaded = recipe::load(&source_recipe, &config, false).unwrap();
        let reader = Reader { config, loaded };
        wait(&barrier);
        let before = reader.snapshot();
        assert_eq!(before.executions, 1);
        assert_eq!(before.control.ledger.epoch, epoch);
        let evidence = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        let inventory = mediated.then(|| {
            let path = directory.join("inventory-2.json");
            let bytes = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            (path, bytes)
        });
        write_complete(
            &directory.join("ready"),
            b"original second generation is complete",
        );
        wait(&directory.join("approved"));
        let after = reader.snapshot();
        assert_eq!(after.revision, before.revision + 2);
        assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
        assert_eq!(
            (after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged)
        );
        write_complete(&source, evidence);
        if let Some((path, bytes)) = inventory {
            write_complete(&path, bytes);
        }
        release_actor.store(true, Ordering::Release);
        wait(&next_human);
        assert_eq!(
            reader.snapshot().executions,
            1,
            "qualification and governor approval do not replace human approval"
        );
        release_human.store(true, Ordering::Release);
    });
    let mut provisioned = false;
    let mut consumed = false;
    let result = resident::command_with_clock(&args, Some(&capsule_path), || {
        if !provisioned && root.0.join("peers/governor.sock").exists() {
            let cut = reader.snapshot();
            assert_eq!(cut.control.ledger.epoch + 1, epoch);
            let request = activation(
                &seed.config,
                cut.control.sequence,
                cut.control.ledger.epoch,
                21,
                3,
            );
            assert_eq!(capsule(&root, &request), capsule_path);
            provisioned = true;
        }
        if provisioned && !consumed && root.0.join("peers/joint-draft.sock").exists() {
            fs::remove_file(&capsule_path).unwrap();
            consumed = true;
        }
        let admitted = ready.load(Ordering::Acquire);
        let human = human_ready.load(Ordering::Acquire);
        peers.poll(
            [
                None,
                admitted.then_some(Actor::Observe),
                admitted.then_some(Actor::Observe),
            ],
            [
                None,
                human.then_some(decision),
                Some(ReviewDecision::Approve),
            ],
        );
        ElapsedTick(1000)
    });
    governor_peer.join().unwrap();
    child.finish();
    assert!(provisioned && consumed);
    assert!(
        !capsule_path.exists(),
        "Finish must not reload the first request's capsule"
    );
    assert_eq!(
        result.is_ok(),
        decision == ReviewDecision::Approve,
        "{result:?}"
    );
    let (replies, packets) = peers.join();
    assert!(replies[0].is_none() && packets[0].is_none());
    let second = packets[1]
        .as_ref()
        .expect("fresh authenticated second human");
    assert_eq!(second.policy_generation(), 1);
    assert_eq!(second.action().spec().policy_epoch, epoch);
    assert_eq!(second.views().len(), 2);
    let allowed = decision == ReviewDecision::Approve;
    assert_eq!(confirmed(replies[1].as_ref().unwrap()), allowed);
    let state = reader.snapshot();
    assert_eq!(state.executions, if allowed { 3 } else { 1 });
    assert_eq!(state.control.ledger.epoch, epoch);
    let mut charged = seed.before.control.ledger.charged;
    if allowed {
        let finish = packets[2].as_ref().expect("separate Finish human");
        assert!(confirmed(replies[2].as_ref().unwrap()));
        assert_ne!(second.binding().session, finish.binding().session);
        assert_eq!(finish.policy_generation(), 1);
        assert_eq!(finish.action().spec().policy_epoch, epoch);
        charged += second.action().spec().units + finish.action().spec().units;
    } else {
        assert!(replies[2].is_none() && packets[2].is_none());
    }
    assert_eq!(state.control.ledger.charged, charged);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(
        fs::read(root.0.join("history-1.bin")).unwrap(),
        input_history
    );
    let mut loaded = recipe::load(&seed.paths[1], &seed.config, false).unwrap();
    loaded.recovery_anchor = Some(anchors::load(&root.0.join("final.anchor")).unwrap());
    if mediated {
        assert_eq!(
            fs::read(root.0.join("source-1.bin")).unwrap(),
            input_source.unwrap()
        );
        let saved = checkpoint(&root.0.join("source-2.bin"));
        assert_eq!(
            saved.history_anchor(),
            loaded.recovery_anchor.as_ref().unwrap()
        );
        assert_eq!(saved.work().checks, if allowed { 6 } else { 3 });
        assert_eq!(
            saved.work().reserved_read_bytes,
            saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        let historical = archived(&root, &root.0.join("source-2.bin"), 99, None);
        topology::admit(
            &seed.config,
            &mut loaded,
            Some(&historical),
            topology::Mode::Historical,
        )
        .unwrap();
    }
    let (host, _, _, _) = recovery::open_selected(&seed.config, &loaded).unwrap();
    assert!(host.held_out_joint_report(20).unwrap().unwrap().qualified());
    assert!(host.held_out_joint_report(21).unwrap().unwrap().qualified());
    assert!(
        host.check_credibility().is_err(),
        "another Fence withdraws live qualification"
    );
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(
        host.stream_snapshot().unwrap().confirmed.finished(),
        allowed
    );
    drop(host);
    quiescent(&root);
}

#[test]
fn governed_joint_recovery_refuses_historical_capsule_before_more_source_or_numerical_work() {
    for mediated in [false, true] {
        let root = Root::new();
        let seed = prepared(&root, mediated);
        selections(
            &root,
            &seed.entries[1].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let args = selected_recovery(&root, &seed, mediated);
        let old = fs::read(&seed.old_capsule).unwrap();
        let original_anchor = fs::read(root.0.join("history-1.bin")).unwrap();
        let reader = Reader::new(&root, &seed.paths[1]);
        assert!(
            resident::command_with_clock(&args, Some(&seed.old_capsule), || ElapsedTick(1000))
                .is_err()
        );
        let state = reader.snapshot();
        assert_eq!(state.executions, 1);
        assert_eq!(
            state.control.ledger.charged,
            seed.before.control.ledger.charged
        );
        assert_eq!(fs::read(&seed.old_capsule).unwrap(), old);
        assert_eq!(
            fs::read(root.0.join("history-1.bin")).unwrap(),
            original_anchor
        );
        let mut loaded = recipe::load(&seed.paths[1], &seed.config, false).unwrap();
        loaded.recovery_anchor = Some(anchors::load(&root.0.join("final.anchor")).unwrap());
        if mediated {
            let saved = checkpoint(&root.0.join("source-2.bin"));
            assert_eq!(
                saved.history_anchor(),
                loaded.recovery_anchor.as_ref().unwrap()
            );
            assert_eq!(
                saved.work().checks,
                2,
                "failed activation precedes fresh certification"
            );
            let historical = archived(&root, &root.0.join("source-2.bin"), 99, None);
            topology::admit(
                &seed.config,
                &mut loaded,
                Some(&historical),
                topology::Mode::Historical,
            )
            .unwrap();
        }
        let (host, _, _, _) = recovery::open_selected(&seed.config, &loaded).unwrap();
        assert!(host.check_credibility().is_err());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (4, 2));
        assert!(host.request_status(2).is_err());
        assert!(host.credibility_change(21).is_err());
        drop(host);
        quiescent(&root);
    }
}

#[test]
fn original_governor_promotion_withdraws_recovered_joint_qualification_without_refunding_work() {
    for mediated in [false, true] {
        let root = Root::new();
        let seed = prepared(&root, mediated);
        let mut loaded = recipe::load(&seed.paths[1], &seed.config, true).unwrap();
        loaded.recovery_anchor = Some(anchors::load(&root.0.join("history-1.bin")).unwrap());
        if mediated {
            let profile = archived(&root, &root.0.join("source-1.bin"), 2, None);
            topology::admit(
                &seed.config,
                &mut loaded,
                Some(&profile),
                topology::Mode::Live,
            )
            .unwrap();
        }
        let limit = deadline();
        let mut control = Control::new(&seed.config, 2, None).unwrap();
        let (started, governor) = topology::start_governed(
            &seed.config,
            &loaded,
            true,
            &mut control,
            &limit,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let topology::Started {
            custody: (host, reviewer, _, _),
            source,
            ..
        } = started;
        assert!(host.check_credibility().is_err());
        let before = host.learned_generation_inspection().unwrap().numerical;
        let cut = host.inspect();
        let path = capsule(
            &root,
            &activation(
                &seed.config,
                cut.control.sequence,
                cut.control.ledger.epoch,
                21,
                3,
            ),
        );
        let (_, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        topology::attach(&mut driver, source).unwrap();
        assert!(
            qualify::prepare(
                &mut driver,
                &seed.config,
                Some(&path),
                (&mut control, &reviewer, &limit),
                &mut || ElapsedTick(1000)
            )
            .unwrap()
        );
        let source_work = driver.topology().map(|source| source.source().work());
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        if mediated {
            fs::remove_file(root.0.join("inventory-2.json")).unwrap();
        }
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            host.check_credibility().unwrap();
            let qualified = host.inspect();
            let update = PolicyUpdate::new(
                900,
                qualified.control.sequence,
                qualified.control.ledger.epoch,
                policy(),
            )
            .unwrap();
            let revision = host.revision();
            let review = host.request_policy_campaign(revision, &update).unwrap();
            let revision = host.revision();
            let approval = governor
                .approve(&mut host, revision, &review, false)
                .unwrap();
            let revision = host.revision();
            host.promote_policy_campaign(revision, &approval).unwrap();
            assert_eq!(host.current_policy().unwrap(), &policy());
            assert!(host.held_out_joint_report(21).unwrap().unwrap().qualified());
            assert!(host.check_credibility().is_err());
            assert!(request_preflight(&host, &loaded, true, None, Some(1), None).is_err());
            assert!(host.request_status(2).is_err());
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                before
            );
            let state = host.inspect();
            assert_eq!(state.executions, qualified.executions);
            assert_eq!(
                state.control.ledger.charged,
                qualified.control.ledger.charged
            );
            assert_eq!(
                state.control.ledger.epoch,
                qualified.control.ledger.epoch + 1
            );
        }
        assert_eq!(
            driver.topology().map(|source| source.source().work()),
            source_work
        );
        assert_eq!(cleanup(driver, 1000, 1), 0);
        quiescent(&root);
    }
}
