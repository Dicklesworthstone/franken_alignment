//! A confirmed learned prefix survives an explicitly governed restart. These
//! use the original numerical owner, campaign replay, authenticated governor,
//! actor and human peers; fixture outputs do not establish policy quality.
use super::*;
use crate::workflow::actor_service::learned::{
    anchors as retained_history, campaigns as campaign_options,
    continuation as continue_message,
};
use fa_reference::action::consequence::{
    delivery::persistent::{
        FileDeliverySnapshot, governance::PolicyUpdate,
        observed::guarded::FileCampaignRequirement,
    },
    policy_campaign::ReplayLimits,
};

const RECOVERY_CHILD_ROOT: &str = "FA_RECOVERED_LEARNED_GOVERNOR_ROOT";
const RECOVERY_CHILD_TEST: &str = "workflow::actor_service::learned::tests::resident_sequence::governor::governed_recovery::independent_recovered_learned_governor_process";

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 4,
    }
}

fn policy_json(generation: u64, maximum: usize) -> String {
    format!(r#"{{"generation":{generation},"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":{maximum}}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}"#)
}

fn wrapped(inner: &str, generation: u64, maximum: usize) -> String {
    format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"recipe":{inner}}}"#,
        policy_json(generation, maximum))
}

fn recovery_paths(root: &Root, paths: &[PathBuf], before: Option<&FileDeliverySnapshot>)
    -> Vec<PathBuf>
{
    paths.iter().enumerate().map(|(index, original)| {
        let mut inner = fs::read_to_string(original).unwrap();
        if index == 0 {
            if let Some(before) = before {
                let empty = r#""recovery_floor":{"journal_revision":0,"control_sequence":0,"authority_epoch":0}"#;
                assert!(inner.contains(empty));
                inner = inner.replacen(empty, &format!(r#""recovery_floor":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}"#,
                    before.revision, before.control.sequence, before.control.ledger.epoch), 1);
            }
        }
        let path = root.0.join(format!("governed-recovery-{index}.json"));
        fs::write(&path, wrapped(&inner, 2, 256)).unwrap();
        path
    }).collect()
}

fn recovery_args(root: &Root, config: &Config, paths: &[PathBuf], entries: &[resident::Entry],
    continue_draft: bool) -> Vec<String>
{
    let mut args = inputs(root, config, paths, entries, None);
    let creation = args.iter().position(|arg| arg == "--campaign-profile").unwrap();
    drop(args.drain(creation..creation + 2));
    if continue_draft {
        args.push("--continue-generation".into());
    } else {
        args.extend(["--after-message".into(), "1".into()]);
    }
    args.extend(["--history-anchor".into(), root.0.join("prior.anchor").display().to_string()]);
    args
}

/// The original selected finite stream has exactly two Messages and a Finish.
/// Publish its first Message with real numerical/native/human work and promote
/// policy two while the first original governor still owns its custody. Optionally
/// complete the next unsubmitted draft before retaining the recovery boundary.
fn prepare(root: &Root, continue_draft: bool)
    -> (Config, Vec<PathBuf>, Vec<resident::Entry>, FileDeliverySnapshot)
{
    let (mut config, paths, mut entries) = selected(root, None, None);
    campaign_options::admit_selected(&mut entries[0].loaded, requirement()).unwrap();
    let ((host, reviewer, observer, identity), governor) =
        recovery::create_governed(&config, &entries[0].loaded).unwrap();
    assert!(observer.is_none() && identity.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let limit = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control, &limit,
        &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &entries[0].loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut config, &entries[0].loaded,
        &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    drop(history);
    let retiring = Instant::now();
    while !driver.retire_completed_request(1).unwrap() {
        assert!(retiring.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(1));
    }
    if continue_draft {
        let next = driver.supervisor().host().unwrap()
            .prepare_learned_text_stream_continuation(1, 2).unwrap();
        let mut control = Control::new(&config, 2, None).unwrap();
        assert!(continue_message::prepare(&mut driver, &reviewer, &mut config, next,
            &mut control, &limit, &mut || ElapsedTick(1000)).unwrap());
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control, &limit,
            &mut || ElapsedTick(1000)).unwrap());
    }
    let before = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let before = host.inspect();
        let update = PolicyUpdate::new(901, before.control.sequence,
            before.control.ledger.epoch, policy()).unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        assert!(!review.report().requires_shadow());
        assert!(review.report().newly_reviewable().is_empty());
        let revision = host.revision();
        let approval = governor.approve(&mut host, revision, &review, false).unwrap();
        let revision = host.revision();
        host.promote_policy_campaign(revision, &approval).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws),
            if continue_draft { (6, 4) } else { (4, 2) });
        assert_eq!(host.current_policy().unwrap(), &policy());
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.epoch, 1);
        retained_history::export_acknowledged(&host.history_anchor().unwrap(),
            &root.0.join("prior.anchor")).unwrap();
        host.inspect()
    };
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    (config, paths, entries, before)
}

#[test]
fn independent_recovered_learned_governor_process() {
    let Some(root) = std::env::var_os(RECOVERY_CHILD_ROOT) else { return; };
    let root = PathBuf::from(root);
    wait(&root.join("ready"));
    let selected = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(selected.scope);
    let inspected = exchange(&root, "recovered-get.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"get","query":"1"}}"#));
    let cut = known(&inspected, "inspect");
    assert_eq!(field(cut, "authority_epoch").as_str(), Some("2"));
    assert_eq!(field(cut, "policy_generation").as_str(), Some("2"));
    assert!(!root.join("evidence.json").exists());
    let reviewed = exchange(&root, "recovered-review.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{CAMPAIGN}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":"2","policy_generation":"2"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":2,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{}}}}}"#,
        live_governor::protocol::json(field(cut, "journal_revision")),
        live_governor::protocol::json(field(cut, "control_sequence")), policy_json(3, 255)));
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(1));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert!(field(replay, "newly_reviewable").as_array().unwrap().is_empty());
    let approved = exchange(&root, "recovered-approve.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
        live_governor::protocol::json(field(value, "journal_revision")),
        live_governor::protocol::quote(report)));
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "approval").as_str(), Some("delegated"));
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    write_complete(&root.join("approved"), live_governor::protocol::json(&approved));
}

#[test]
fn a_governed_recovery_finishes_the_original_message_suffix_with_fresh_human_authority() {
    recovered_sequence(false);
}

#[test]
fn a_governed_recovery_resumes_the_original_unsubmitted_draft_without_sampling_again() {
    recovered_sequence(true);
}

fn recovered_sequence(continue_draft: bool) {
    let root = Root::new();
    let (config, paths, entries, before) = prepare(&root, continue_draft);
    let prior = fs::read(root.0.join("prior.anchor")).unwrap();
    let selected = recovery_paths(&root, &paths[1..], Some(&before));
    let mut peers = Peers::new(&root, &config, &entries);
    let mut child = GovernorChild(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", RECOVERY_CHILD_TEST, "--nocapture"])
        .env(RECOVERY_CHILD_ROOT, &root.0).spawn().unwrap());
    let (governor_path, operator_path, _) = selections(&root, &entries[1].actor, child.0.id());
    assert_ne!(child.0.id(), std::process::id());
    let governor = live_governor::Settings::read(&governor_path).unwrap();
    let selected_operator = learned_operator::Selection::read_governed(&operator_path, GENERATION).unwrap();
    let args = recovery_args(&root, &config, &selected, &entries[1..], continue_draft);
    // A selected first-entry precheck is created only after the original second
    // Message is ready. Hold its actor until fresh separate governance finishes.
    let barrier = root.0.join("peers/recovered-draft.sock");
    let precheck = root.0.join("recovered-precheck.json");
    fs::write(&precheck, format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":2,"disclosure_id":91,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":60000}}"#,
        quote(barrier.to_str().unwrap()))).unwrap();
    let sequence = fs::read_to_string(&args[2]).unwrap();
    fs::write(&args[2], sequence.replacen(r#""precheck_profile":null"#,
        &format!(r#""precheck_profile":{}"#, quote(precheck.to_str().unwrap())), 1)).unwrap();
    let promoted = Arc::new(AtomicBool::new(false));
    let human_ready = Arc::new(AtomicBool::new(false));
    let actor_release = promoted.clone();
    let human_release = human_ready.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let next_human = peers.reviewer.socket(2);
    let source_recipe = selected[0].clone();
    let scope = entries[1].actor.scope;
    let before_charged = before.control.ledger.charged;
    let operator_peer = thread::spawn(move || {
        let source_config = Config::read_learned_recovery(&directory.join("command-host.json")).unwrap();
        let source_loaded = recipe::load(&source_recipe, &source_config, false).unwrap();
        let reader = Reader { config: source_config, loaded: source_loaded };
        wait(&barrier);
        let retained = reader.snapshot();
        assert_eq!(retained.executions, 1);
        assert_eq!(retained.control.ledger.epoch, 2);
        assert_eq!(retained.control.ledger.charged, before_charged);
        assert_eq!(retained.payload, b"A");
        let bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        let mut socket = UnixStream::connect(&selected_operator.settings.socket).unwrap();
        selected_operator.settings.supervisor.verify(&socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
        let mut request = TowerRequest::new("recovered-learned-promotion",
            Address::parse(&format!("fa://{}/run/{}@{GENERATION}", scope.tenant, scope.run)).unwrap(), Verb::Next);
        request.arguments = ReadArguments::PolicyPromotionNext { campaign: CAMPAIGN, operation: OPERATION };
        let next = encode_request(&request).unwrap();
        let unavailable = operator_exchange(&mut socket, &next);
        assert_eq!(field(field(&unavailable, "envelope"), "reason_code").as_str(), Some("CapabilityUnsupported"));
        write_complete(&directory.join("ready"), b"original recovered Message is complete");
        wait(&directory.join("approved"));
        let approved = reader.snapshot();
        assert_eq!(approved.revision, retained.revision + 2);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let promoted = operator_exchange(&mut socket, &command);
        operator_known(&promoted, "policy-promotion");
        let current = reader.snapshot();
        assert_eq!(current.revision, approved.revision + 1);
        assert_eq!(current.control.ledger.epoch, 3);
        assert_eq!((current.executions, current.control.ledger.charged),
            (retained.executions, retained.control.ledger.charged));
        assert!(!source.exists(), "governance cannot fabricate current source evidence");
        write_complete(&source, bytes);
        actor_release.store(true, Ordering::Release);
        wait(&next_human);
        let awaiting = reader.snapshot();
        assert_eq!(awaiting.executions, 1, "a fresh human decision remains necessary");
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), promoted);
        assert_eq!(fs::read(directory.join("store/delivery.bin")).unwrap(), canonical);
        human_release.store(true, Ordering::Release);
    });
    let result = resident::command_with_clock(&args, None, || {
        let admitted = promoted.load(Ordering::Acquire);
        let reviewed = human_ready.load(Ordering::Acquire);
        peers.poll([None, admitted.then_some(Actor::Observe), admitted.then_some(Actor::Observe)],
            [None, reviewed.then_some(ReviewDecision::Approve), Some(ReviewDecision::Approve)]);
        ElapsedTick(1000)
    });
    operator_peer.join().unwrap();
    child.finish();
    result.unwrap();
    assert_eq!(peers.seen_actor, [false, true, true]);
    assert_eq!(peers.seen_human, [false, true, true]);
    let (replies, packets) = peers.join();
    assert!(replies[0].is_none() && packets[0].is_none());
    assert!(confirmed(replies[1].as_ref().unwrap()));
    assert!(confirmed(replies[2].as_ref().unwrap()));
    let second = packets[1].as_ref().unwrap();
    let last = packets[2].as_ref().unwrap();
    assert_ne!(second.binding().session, last.binding().session);
    assert_eq!(second.action().spec().policy_epoch, 3);
    assert_eq!(second.policy_generation(), 3);
    assert_eq!(last.policy_generation(), 3);
    let frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(frame.message(), Some("A"));
    let mut reader = Reader::new(&root, &selected[0]);
    let state = reader.snapshot();
    assert_eq!((state.executions, state.payload.as_slice()), (3, b"AA".as_slice()));
    assert_eq!(state.control.ledger.charged, before.control.ledger.charged
        + second.action().spec().units + last.action().spec().units);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.available + state.control.ledger.charged,
        config.profile.delivery.total);
    assert_eq!(fs::read(root.0.join("prior.anchor")).unwrap(), prior);
    reader.loaded.expected_policy = Policy::new(3, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(255), Predicate::UnitsAtMost(512), Predicate::All(vec![0, 1, 2]),
    ]).unwrap();
    reader.loaded.expected_campaigns = Some(governor.campaign_requirement());
    reader.loaded.recovery_anchor = Some(retained_history::load(&root.0.join("final.anchor")).unwrap());
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(host.current_policy().unwrap(), &reader.loaded.expected_policy);
    drop(host);
    quiescent(&root);
}


#[test]
fn governed_startup_refusal_retains_the_fence_and_preserves_raced_anchor_custody() {
    let mut original_failure = None;
    for collision in [false, true] {
        let root = Root::new();
        let (config, paths, entries, before) = prepare(&root, false);
        let old_path = root.0.join("prior.anchor");
        let old_bytes = fs::read(&old_path).unwrap();
        let old_anchor = retained_history::load(&old_path).unwrap();
        assert_eq!(old_anchor.revision(), before.revision);
        let selected = recovery_paths(&root, &paths[1..], Some(&before));
        selections(&root, &entries[1].actor,
            std::process::id().checked_add(10_000).unwrap());
        let mut args = recovery_args(&root, &config, &selected, &entries[1..], false);
        let predecessor = args.iter().position(|arg| arg == "--after-message").unwrap() + 1;
        // This distinct nonzero ID is syntactically valid, but has no original
        // receipt. Native recovery therefore reaches its Fence before the
        // read-only request check refuses it; no actor decision is admitted.
        args[predecessor] = "99".into();
        let output = root.0.join("final.anchor");
        let mut clock_seen = false;
        let error = resident::command_with_clock(&args, None, || {
            if !clock_seen {
                clock_seen = true;
                // Both profiles and all native assets were already admitted.
                // Replay and the startup refusal use retained closures only.
                fs::remove_file(root.0.join("evidence.json")).unwrap();
                if collision {
                    fs::OpenOptions::new().write(true).create_new(true).open(&output)
                        .unwrap().write_all(b"independent custodian won the output path").unwrap();
                }
            }
            ElapsedTick(1000)
        }).unwrap_err();
        assert!(clock_seen, "the independently anchored replay must actually run");
        let reader = Reader::new(&root, &selected[0]);
        let after = reader.snapshot();
        assert_eq!(after.revision, before.revision + 1,
            "startup refusal may retain only the original acknowledged recovery Fence");
        assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
        assert_eq!(after.control.ledger.available, before.control.ledger.available);
        assert_eq!(after.stop, before.stop);
        assert!(!root.0.join("evidence.json").exists());
        assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
        if collision {
            let original = original_failure.as_ref().unwrap();
            assert!(error.starts_with(&format!(
                "{original}; acknowledged recovery history retention failed:")), "{error}");
            assert!(error.contains("preserve the previous anchor and original store"));
            assert_eq!(fs::read(&output).unwrap(),
                b"independent custodian won the output path");
        } else {
            assert!(!error.contains("history retention failed"), "{error}");
            let retained = retained_history::load(&output).unwrap();
            assert_eq!(retained.revision(), after.revision);
            assert!(retained.revision() > old_anchor.revision());
            original_failure = Some(error);
        }
        // Inspection and the error path never reopen or advance the owner.
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        assert_eq!(reader.snapshot(), after);
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
        quiescent(&root);
    }
}
