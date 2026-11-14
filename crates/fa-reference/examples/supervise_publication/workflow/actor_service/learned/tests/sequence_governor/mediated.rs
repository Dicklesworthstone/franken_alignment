//! Live policy governance and mediated publication share the original owner.
//! Real numerical generation, a separate governor, and independent human peers
//! exercise custody and accounting; these synthetic fixtures qualify no model.
use super::*;
use super::super::super::mediated_topology as mediated_fixture;
use crate::workflow::actor_service::learned::{anchors, campaigns, topology};
use fa_reference::action::consequence::{
    delivery::persistent::observed::{
        driver::topology::checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES},
        guarded::FileCampaignRequirement,
        mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
    },
    policy_campaign::ReplayLimits,
};
use std::{io::Cursor, sync::atomic::AtomicU64};

#[path = "mediated/preflight.rs"]
mod preflight;

const MEDIATED_CHILD_ROOT: &str = "FA_LEARNED_MEDIATED_GOVERNOR_ROOT";
const MEDIATED_CHILD_EPOCH: &str = "FA_LEARNED_MEDIATED_GOVERNOR_EPOCH";
const MEDIATED_CHILD_TEST: &str = "workflow::actor_service::learned::tests::resident_sequence::governor::mediated::independent_mediated_governor_process";

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 4,
    }
}

#[test]
fn independent_mediated_governor_process() {
    let Some(root) = std::env::var_os(MEDIATED_CHILD_ROOT) else { return; };
    let root = PathBuf::from(root);
    let expected_epoch = std::env::var(MEDIATED_CHILD_EPOCH).unwrap();
    assert!(matches!(expected_epoch.as_str(), "0" | "3"));
    wait(&root.join("ready"));
    let selected = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(selected.scope);
    let inspected = exchange(&root, "get.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"get","query":"1"}}"#));
    let cut = known(&inspected, "inspect");
    assert_eq!(field(cut, "authority_epoch").as_str(), Some(expected_epoch.as_str()));
    assert!(!root.join("evidence.json").exists());
    let inventory_generation = if expected_epoch == "3" { 2 } else { 1 };
    assert!(!root.join(format!("inventory-{inventory_generation}.json")).exists(),
        "campaign replay must use retained evidence, not fresh topology input");
    let reviewed = exchange(&root, "review.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{CAMPAIGN}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{},"policy_generation":"1"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{{"generation":2,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":256}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}}}}}"#,
        live_governor::protocol::json(field(cut, "journal_revision")),
        live_governor::protocol::json(field(cut, "control_sequence")),
        live_governor::protocol::json(field(cut, "authority_epoch"))));
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(1));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert!(field(replay, "newly_reviewable").as_array().unwrap().is_empty());
    let approved = exchange(&root, "approve.json", format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
        live_governor::protocol::json(field(value, "journal_revision")),
        live_governor::protocol::quote(report)));
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "approval").as_str(), Some("delegated"));
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    write_complete(&root.join("approved"), live_governor::protocol::json(&approved));
}

fn selected_mediated(root: &Root) -> (Config, Vec<PathBuf>, Vec<resident::Entry>) {
    let (config, first, initial) = mediated_fixture::selected(root, None, false);
    let (second, message) = native::next(root, &config, 2);
    let (last, finish) = native::next(root, &config, 3);
    let entries = [initial, message, finish].into_iter().enumerate().map(|(index, loaded)| {
        let mut actor = peers::actor_profile(root, &config, loaded.request);
        actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
        resident::Entry { loaded, actor, precheck: None, release: if index == 2 {
            LearnedTextRelease::Finish
        } else { LearnedTextRelease::Message } }
    }).collect();
    (config, vec![first, second, last], entries)
}

fn seed(root: &Root, path: &Path) -> ReviewPacket {
    let config = configured(root);
    let mut loaded = recipe::load(path, &config, true).unwrap();
    campaigns::admit_selected(&mut loaded, requirement()).unwrap();
    loaded.anchor_output = Some(root.0.join("history-1.bin"));
    topology::admit(&config, &mut loaded, Some(&root.0.join("create-topology.json")),
        topology::Mode::Create).unwrap();
    let reviewer = fixture::peers(root, &config);
    let actor = peers::actor_profile(root, &config, loaded.request);
    let mut actor_peer = None;
    let mut human_peer = None;
    let result = run_with_actor(config, loaded, &reviewer, false, None, None, None,
        Some(&actor), || {
            if actor_peer.is_none() && actor.socket.exists() {
                actor_peer = Some(peers::actor(actor.clone(), LearnedTextRelease::Message,
                    DUE[0], peers::Behavior::Observe));
            }
            if human_peer.is_none() && reviewer.socket(1).exists() {
                human_peer = Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        }).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    let reply = actor_peer.expect("original first actor").join().unwrap();
    assert!(confirmed(&reply));
    assert_eq!(reply.result, result.response.result);
    let packet = human_peer.expect("original first human").join().unwrap();
    assert_eq!(packet.policy_generation(), 1);
    assert_eq!(packet.action().spec().policy_epoch, 0);
    let saved = mediated_fixture::checkpoint(root, 1);
    assert_eq!(saved.work().checks, 2);
    assert_eq!(saved.history_anchor(), &anchors::load(&root.0.join("history-1.bin")).unwrap());
    quiescent(root);
    packet
}

fn recovery_recipes(paths: &[PathBuf]) {
    for path in paths {
        let original = fs::read_to_string(path).unwrap();
        fs::write(path, format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{{"generation":1,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":512}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"recipe":{original}}}"#)).unwrap();
    }
}

fn next_source(root: &Root) {
    let graph = fs::read_to_string(root.0.join("initial.json")).unwrap()
        .replacen("\"generation\":1", "\"generation\":2", 1)
        .replacen("\"inventory_generation\":1", "\"inventory_generation\":2", 1)
        .replacen("\"complete_inventory_generation\":1", "\"complete_inventory_generation\":2", 1);
    fs::write(root.0.join("live-2.json"), graph).unwrap();
    fs::copy(root.0.join("initial-inventory.json"), root.0.join("inventory-2.json")).unwrap();
}

fn checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}

fn archived(root: &Root, input: &Path, next: u64, output: Option<&Path>) -> PathBuf {
    let saved = checkpoint(input);
    let retained = output.map(|path| format!(r#", "retain_checkpoint":{}"#,
        quote(path.to_str().unwrap()))).unwrap_or_default();
    let profile = root.0.join(format!("governed-topology-{next}-{}.json", output.is_some()));
    fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":{},"inventory_path":{}}},"next":{{"topology_path":{},"inventory_path":{}}},"operation":{},"source_checkpoint":{}{retained}}}"#,
        quote(root.0.join("initial.json").to_str().unwrap()),
        quote(root.0.join("initial-inventory.json").to_str().unwrap()),
        quote(root.0.join(format!("live-{next}.json")).to_str().unwrap()),
        quote(root.0.join(format!("inventory-{next}.json")).to_str().unwrap()),
        saved.last_operation() + 1, quote(input.to_str().unwrap()))).unwrap();
    profile
}

fn recovered_args(mut args: Vec<String>, root: &Root) -> Vec<String> {
    let at = args.iter().position(|arg| arg == "--campaign-profile").unwrap();
    args.drain(at..at + 2);
    args.extend(["--after-message".into(), "1".into(), "--history-anchor".into(),
        root.0.join("history-1.bin").display().to_string()]);
    args
}

#[test]
fn live_governor_promotes_inside_created_mediated_learned_resident() {
    governed_mediated(false, false);
}

#[test]
fn anchored_mediated_resident_retains_source_work_and_recovers_its_original_governor() {
    governed_mediated(true, false);
}

#[test]
fn promoted_mediated_resident_still_requires_the_final_original_source_check() {
    for recovering in [false, true] { governed_mediated(recovering, true); }
}

fn governed_mediated(recovering: bool, lose_source: bool) {
    let root = Root::new();
    let (config, paths, entries) = selected_mediated(&root);
    let seed_packet = recovering.then(|| seed(&root, &paths[0]));
    let input_source = recovering.then(|| fs::read(root.0.join("source-1.bin")).unwrap());
    let input_history = recovering.then(|| fs::read(root.0.join("history-1.bin")).unwrap());
    let epoch = if recovering { 3 } else { 0 };
    let generation = if recovering { 2 } else { 1 };
    let output = root.0.join(format!("source-{generation}.bin"));
    let topology_profile = if recovering {
        recovery_recipes(&paths[1..]);
        next_source(&root);
        archived(&root, &root.0.join("source-1.bin"), 2, Some(&output))
    } else { root.0.join("create-topology.json") };
    let mut reader = Reader::new(&root, &paths[0]);
    let mut peers = Peers::new(&root, &config, &entries);
    let mut child = GovernorChild(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", MEDIATED_CHILD_TEST, "--nocapture"])
        .env(MEDIATED_CHILD_ROOT, &root.0)
        .env(MEDIATED_CHILD_EPOCH, epoch.to_string()).spawn().unwrap());
    assert_ne!(child.0.id(), std::process::id());
    let (_, operator_path, _) = selections(&root, &entries[0].actor, child.0.id());
    let operator = learned_operator::Selection::read_governed(&operator_path, GENERATION).unwrap();
    let barrier = root.0.join("peers/mediated-draft.sock");
    let precheck_path = root.0.join("mediated-precheck.json");
    fs::write(&precheck_path, format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":{epoch},"disclosure_id":91,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":60000}}"#,
        quote(barrier.to_str().unwrap()))).unwrap();
    let start = usize::from(recovering);
    let mut args = inputs(&root, &config, &paths[start..], &entries[start..],
        (!recovering).then_some(precheck_path.as_path()));
    if recovering {
        args = recovered_args(args, &root);
        let manifest = root.0.join("sequence.json");
        let original = fs::read_to_string(&manifest).unwrap();
        fs::write(manifest, original.replacen("\"precheck_profile\":null",
            &format!("\"precheck_profile\":{}", quote(precheck_path.to_str().unwrap())), 1)).unwrap();
    }
    args.extend(["--topology-profile".into(), topology_profile.display().to_string()]);
    let promoted = Arc::new(AtomicBool::new(false));
    let human_ready = Arc::new(AtomicBool::new(false));
    let first_charge = Arc::new(AtomicU64::new(0));
    let release_actor = promoted.clone();
    let release_human = human_ready.clone();
    let initial_charge = first_charge.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let inventory = root.0.join(format!("inventory-{generation}.json"));
    let selected_inventory = inventory.clone();
    let next_human = peers.reviewer.socket(2);
    let source_recipe = paths[0].clone();
    let scope = entries[0].actor.scope;
    let operator_peer = thread::spawn(move || {
        let source_config = Config::read_learned_generation(&directory.join("command-host.json")).unwrap();
        let source_loaded = recipe::load(&source_recipe, &source_config, false).unwrap();
        let source_reader = Reader { config: source_config, loaded: source_loaded };
        wait(&barrier);
        let before = source_reader.snapshot();
        assert_eq!(before.executions, 1);
        assert_eq!(before.payload, b"A");
        assert_eq!(before.control.ledger.epoch, epoch);
        initial_charge.store(before.control.ledger.charged, Ordering::Release);
        let evidence = fs::read(&source).unwrap();
        let topology_inventory = fs::read(&selected_inventory).unwrap();
        fs::remove_file(&source).unwrap();
        fs::remove_file(&selected_inventory).unwrap();
        let mut socket = UnixStream::connect(&operator.settings.socket).unwrap();
        operator.settings.supervisor.verify(&socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
        let mut request = TowerRequest::new("mediated-campaign-promotion",
            Address::parse(&format!("fa://{}/run/{}@{GENERATION}", scope.tenant, scope.run)).unwrap(), Verb::Next);
        request.arguments = ReadArguments::PolicyPromotionNext { campaign: CAMPAIGN, operation: OPERATION };
        let next = encode_request(&request).unwrap();
        let unavailable = operator_exchange(&mut socket, &next);
        assert_eq!(field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported"));
        write_complete(&directory.join("ready"), b"original second Message generation completed");
        wait(&directory.join("approved"));
        let approved = source_reader.snapshot();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(approved.control.ledger.epoch, epoch);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let result = operator_exchange(&mut socket, &command);
        operator_known(&result, "policy-promotion");
        let after = source_reader.snapshot();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.ledger.epoch, epoch + 1);
        assert_eq!((after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged));
        assert!(!source.exists() && !selected_inventory.exists(),
            "policy promotion cannot obtain a current evidence or topology certificate");
        write_complete(&source, evidence);
        write_complete(&selected_inventory, topology_inventory);
        release_actor.store(true, Ordering::Release);
        wait(&next_human);
        assert_eq!(source_reader.snapshot().executions, 1,
            "the governor does not replace the fresh human decision");
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), result);
        assert_eq!(fs::read(directory.join("store/delivery.bin")).unwrap(), canonical,
            "exact promotion retry disturbed the new policy's human review");
        release_human.store(true, Ordering::Release);
    });
    let mut removed = false;
    let mut charged_at_loss = 0;
    let result = resident::command_with_clock(&args, None, || {
        let admitted = promoted.load(Ordering::Acquire);
        let approved = human_ready.load(Ordering::Acquire);
        if lose_source && approved && !removed {
            let current = reader.snapshot();
            if current.control.ledger.charged > first_charge.load(Ordering::Acquire) {
                assert_eq!(current.executions, 1, "loss must precede the second effect");
                charged_at_loss = current.control.ledger.charged;
                fs::remove_file(&inventory).unwrap();
                removed = true;
            }
        }
        peers.poll([(!recovering).then_some(Actor::Observe), admitted.then_some(Actor::Observe),
            admitted.then_some(Actor::Observe)], [(!recovering).then_some(ReviewDecision::Approve),
            approved.then_some(ReviewDecision::Approve), Some(ReviewDecision::Approve)]);
        ElapsedTick(1000)
    });
    operator_peer.join().unwrap();
    child.finish();
    let (replies, packets) = peers.join();
    let first = seed_packet.as_ref().or(packets[0].as_ref()).unwrap();
    let second = packets[1].as_ref().expect("second release used a fresh authenticated human");
    if recovering { assert!(replies[0].is_none()); }
    else { assert!(confirmed(replies[0].as_ref().unwrap())); }
    assert_eq!(first.policy_generation(), 1);
    assert_eq!(first.action().spec().policy_epoch, 0);
    assert_eq!(first.action().spec().deadline, DUE[0]);
    assert_ne!(first.binding().session, second.binding().session);
    assert_eq!(second.policy_generation(), 2);
    assert_eq!(second.action().spec().policy_epoch, epoch + 1);
    assert_eq!(second.action().spec().deadline, DUE[1]);
    assert_eq!(second.views().len(), 2);
    let frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(frame.message(), Some("A"));
    let state = reader.snapshot();
    let saved = checkpoint(&output);
    assert_eq!(saved.history_anchor(), &anchors::load(&root.0.join("final.anchor")).unwrap());
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
    assert_eq!(saved.work().reserved_read_bytes, saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(saved.expected().graph().spec().generation, generation);
    if lose_source {
        assert!(removed, "source loss was injected after the original dispatch");
        assert!(result.is_err());
        assert!(!confirmed(replies[1].as_ref().unwrap()));
        assert!(replies[2].is_none() && packets[2].is_none());
        assert_eq!(charged_at_loss, first.action().spec().units + second.action().spec().units);
        assert_eq!((state.executions, state.payload.as_slice()), (1, b"A".as_slice()));
        assert_eq!(saved.work().checks, 4);
        assert!(!saved.available());
    } else {
        assert!(result.is_ok(), "{result:?}");
        assert!(replies[start..].iter().all(|reply| confirmed(reply.as_ref().unwrap())));
        let finish = packets[2].as_ref().unwrap();
        assert_ne!(second.binding().session, finish.binding().session);
        assert_eq!(finish.policy_generation(), 2);
        assert_eq!(finish.action().spec().policy_epoch, epoch + 1);
        assert_eq!(finish.action().spec().deadline, DUE[2]);
        assert_eq!(ReleaseFrame::decode(&finish.action().spec().payload).unwrap().prior_messages(), &["A", "A"]);
        assert_eq!(state.control.ledger.epoch, epoch + 1,
            "resident Message/Finish handoff must not recover or rebind another owner");
        assert_eq!((state.executions, state.payload.as_slice()), (3, b"AA".as_slice()));
        assert_eq!(state.control.ledger.charged,
            first.action().spec().units + second.action().spec().units + finish.action().spec().units);
        assert_eq!(saved.work().checks, 6);
        assert!(saved.available());
    }
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.available + state.control.ledger.charged,
        reader.config.profile.delivery.total);
    if recovering {
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), input_source.unwrap());
        assert_eq!(fs::read(root.0.join("history-1.bin")).unwrap(), input_history.unwrap());
        let original = mediated_fixture::checkpoint(&root, 1);
        assert!(saved.last_operation() > original.last_operation());
        assert_eq!(saved.expected().source_limits(), original.expected().source_limits());
    }
    // Independent fully guarded replay checks that promotion and recovery kept
    // the original numerical budget, stream and complete policy. No live source
    // is needed for this historical check, even after the causal source loss.
    reader.loaded.expected_policy = policy();
    reader.loaded.expected_campaigns = Some(requirement());
    reader.loaded.recovery_anchor = Some(anchors::load(&root.0.join("final.anchor")).unwrap());
    let historical = archived(&root, &output, 99, None);
    topology::admit(&reader.config, &mut reader.loaded, Some(&historical),
        topology::Mode::Historical).unwrap();
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(host.current_policy().unwrap(), &policy());
    if !lose_source {
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
        assert!(stream.confirmed.finished());
        assert_eq!(stream.confirmed, stream.published);
    }
    quiescent(&root);
}
