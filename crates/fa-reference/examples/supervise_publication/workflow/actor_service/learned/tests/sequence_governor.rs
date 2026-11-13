//! Actual learned Messages, a separate authenticated governor process, paid
//! numerical promotion and fresh native/human review in one resident owner.
//! Tiny imported fixtures exercise custody and accounting, not model quality.
use super::super::super::operator as learned_operator;
use super::*;
use crate::workflow::actor_service::governor as live_governor;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::address_resolver::Address;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::{
    io::Read,
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const GENERATION: u64 = 61;
const CAMPAIGN: u64 = 701;
const OPERATION: u64 = 702;
const CHILD_ROOT: &str = "FA_LEARNED_LIVE_GOVERNOR_ROOT";
const CHILD_TEST: &str = "workflow::actor_service::learned::tests::resident_sequence::governor::independent_learned_governor_process";

struct GovernorChild(Child);
impl GovernorChild {
    fn start(root: &Root) -> Self {
        Self(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD_TEST, "--nocapture"])
                .env(CHILD_ROOT, &root.0)
                .spawn()
                .unwrap(),
        )
    }
    fn finish(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(25),
                "governor process did not exit"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for GovernorChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn wait(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "missing {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(1));
    }
}
fn write_complete(path: &Path, bytes: impl AsRef<[u8]>) {
    let staging = path.with_extension("pending");
    fs::write(&staging, bytes).unwrap();
    fs::rename(staging, path).unwrap();
}
fn json(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: 1_048_576,
            max_depth: 32,
            max_items: 32_768,
            max_string_bytes: 1_048_576,
        },
    )
    .unwrap()
}
fn field<'a>(value: &'a Json, key: &str) -> &'a Json {
    value.get(key).unwrap()
}
fn known<'a>(response: &'a Json, phase: &str) -> &'a Json {
    assert_eq!(field(response, "phase").as_str(), Some(phase));
    let knowledge = field(response, "knowledge");
    assert_eq!(
        field(knowledge, "state").as_str(),
        Some("known"),
        "{response:?}"
    );
    field(knowledge, "value")
}
fn exchange(root: &Path, name: &str, request: String) -> Json {
    let path = root.join(name);
    write_complete(&path, request);
    let mut output = Vec::new();
    crate::command_with_output(
        vec![
            "policy-governor-exchange".into(),
            root.join("governor.json").display().to_string(),
            path.display().to_string(),
        ],
        &mut output,
    )
    .unwrap();
    json(&output)
}

fn policy() -> Policy {
    Policy::new(
        2,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(256),
            Predicate::UnitsAtMost(512),
            Predicate::All(vec![0, 1, 2]),
        ],
    )
    .unwrap()
}

#[test]
fn independent_learned_governor_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
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
    assert_eq!(field(cut, "authority_epoch").as_str(), Some("0"));
    assert!(
        !root.join("evidence.json").exists(),
        "campaigns must use retained closures"
    );
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
    assert!(
        field(replay, "newly_reviewable")
            .as_array()
            .unwrap()
            .is_empty()
    );
    // This explicit custodian fixture approves only the complete checked report.
    // The product exchange command itself never makes this decision.
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

fn scope_json(actor: &Profile) -> String {
    let scope = actor.scope;
    format!(
        r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority
    )
}
fn selections(root: &Root, actor: &Profile, governor_pid: u32) -> (PathBuf, PathBuf, PathBuf) {
    let identity = actor.supervisor;
    let scope = scope_json(actor);
    let principal = format!(
        r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        identity.uid(),
        identity.gid(),
        identity.pid().unwrap()
    );
    let governor = root.0.join("governor.json");
    fs::write(&governor, format!(r#"{{"schema":"fa.policy-governor-service/1","scope":{scope},"socket":{},"supervisor":{principal},"governor":{{"uid":{},"gid":{},"pid":{governor_pid}}},"candidate_limit":8,"connection_limit":8,"exchange_limit":8,"runtime_ms":60000,"poll_ms":1,"campaign_limit":1,"report_bytes":65536,"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"numerical_replay":{{"max_replay_events":4096,"events":8192}}}}"#,
        quote(root.0.join("peers/governor.sock").to_str().unwrap()), identity.uid(), identity.gid())).unwrap();
    let operator = root.0.join("operator.json");
    fs::write(&operator, format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{scope},"socket":{},"supervisor":{principal},"operator":{principal},"candidate_limit":1,"connection_limit":1,"exchange_limit":8,"runtime_ms":60000,"poll_ms":1,"inspection_queries":0,"explanation_queries":0,"explanation_replay_events":4096,"explanation_disclosure":"policy_and_selected_witnesses","policy_promotion":{{"queries":2,"max_replay_events":4096,"numerical_replay_events":4096,"authority":"promote_approved_policy"}}}}"#,
        quote(root.0.join("peers/operator.sock").to_str().unwrap()))).unwrap();
    let campaign = root.0.join("campaigns.json");
    fs::write(&campaign, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":4}"#).unwrap();
    (governor, operator, campaign)
}

fn inputs(
    root: &Root,
    config: &Config,
    paths: &[PathBuf],
    entries: &[resident::Entry],
    precheck: Option<&Path>,
) -> Vec<String> {
    let mut selected = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let input = super::super::mediated_topology::command_inputs(
            root,
            config,
            &paths[index],
            &entry.actor,
        );
        let actor = root.0.join(format!("actor-{}.json", entry.loaded.request));
        fs::rename(&input[5], &actor).unwrap();
        selected.push(format!(
            r#"{{"recipe":{},"actor_profile":{},"release":"{}","precheck_profile":{}}}"#,
            quote(paths[index].to_str().unwrap()),
            quote(actor.to_str().unwrap()),
            if entry.release == LearnedTextRelease::Message {
                "message"
            } else {
                "finish"
            },
            if index == 1 {
                precheck
                    .map(|path| quote(path.to_str().unwrap()))
                    .unwrap_or("null".into())
            } else {
                "null".into()
            }
        ));
    }
    let plan = root.0.join("sequence.json");
    fs::write(
        &plan,
        format!(
            r#"{{"schema":"fa.learned-publication-sequence/2","entries":[{}]}}"#,
            selected.join(",")
        ),
    )
    .unwrap();
    vec![
        "serve-learned-generated".into(),
        root.0.join("command-host.json").display().to_string(),
        plan.display().to_string(),
        root.0.join("command-reviewer.json").display().to_string(),
        "--governor-profile".into(),
        root.0.join("governor.json").display().to_string(),
        "--campaign-profile".into(),
        root.0.join("campaigns.json").display().to_string(),
        "--operator-profile".into(),
        root.0.join("operator.json").display().to_string(),
        "--tower-generation".into(),
        GENERATION.to_string(),
        "--retain-history-anchor".into(),
        root.0.join("final.anchor").display().to_string(),
    ]
}

fn operator_exchange(socket: &mut UnixStream, request: &str) -> Json {
    socket.write_all(request.as_bytes()).unwrap();
    if !request.ends_with('\n') {
        socket.write_all(b"\n").unwrap();
    }
    socket.flush().unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 1_048_576);
        if byte[0] == b'\n' {
            return json(&bytes);
        }
    }
}
fn operator_known<'a>(value: &'a Json, kind: &str) -> &'a Json {
    let result = field(value, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let knowledge = field(field(result, "value"), "knowledge");
    assert_eq!(
        field(knowledge, "state").as_str(),
        Some("known"),
        "{value:?}"
    );
    field(knowledge, "value")
}

#[test]
fn a_separate_governor_promotes_after_second_learned_generation_and_requires_fresh_human_review() {
    governed_sequence(ReviewDecision::Approve);
}

#[test]
fn learned_promotion_never_substitutes_for_the_second_human_decision() {
    governed_sequence(ReviewDecision::Reject);
}

fn governed_sequence(decision: ReviewDecision) {
    let root = Root::new();
    let (config, paths, entries) = selected(&root, None, None);
    let mut reader = Reader::new(&root, &paths[0]);
    let mut peers = Peers::new(&root, &config, &entries);
    let mut child = GovernorChild::start(&root);
    let (governor_path, operator_path, _) = selections(&root, &entries[0].actor, child.0.id());
    assert_ne!(child.0.id(), std::process::id());
    let governor = live_governor::Settings::read(&governor_path).unwrap();
    let selected_operator =
        learned_operator::Selection::read_governed(&operator_path, GENERATION).unwrap();
    let barrier = root.0.join("peers/draft.sock");
    let precheck_path = root.0.join("draft-precheck.json");
    fs::write(&precheck_path, format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":0,"disclosure_id":91,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":60000}}"#,
        quote(barrier.to_str().unwrap()))).unwrap();
    let args = inputs(&root, &config, &paths, &entries, Some(&precheck_path));
    let promoted = Arc::new(AtomicBool::new(false));
    let human_ready = Arc::new(AtomicBool::new(false));
    let release_actor = promoted.clone();
    let release_human = human_ready.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let next_human = peers.reviewer.socket(entries[1].loaded.request);
    let source_recipe = paths[0].clone();
    let scope = entries[0].actor.scope;
    let operator_peer = thread::spawn(move || {
        let source_config =
            Config::read_learned_generation(&directory.join("command-host.json")).unwrap();
        let source_loaded = recipe::load(&source_recipe, &source_config, false).unwrap();
        let source_reader = Reader {
            config: source_config,
            loaded: source_loaded,
        };
        wait(&barrier); // This original disclosure can start only after generation.
        let before = source_reader.snapshot();
        assert_eq!(before.executions, 1);
        assert_eq!(before.control.ledger.epoch, 0);
        assert_eq!(before.payload, b"A");
        let retained = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        let mut socket = UnixStream::connect(&selected_operator.settings.socket).unwrap();
        selected_operator
            .settings
            .supervisor
            .verify(&socket)
            .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(25)))
            .unwrap();
        let mut request = TowerRequest::new(
            "learned-campaign-promotion",
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
            b"original second Message is complete",
        );
        wait(&directory.join("approved"));
        let approved = source_reader.snapshot();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(approved.control.ledger.epoch, 0);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let result = operator_exchange(&mut socket, &command);
        operator_known(&result, "policy-promotion");
        let after = source_reader.snapshot();
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
        write_complete(&source, retained);
        release_actor.store(true, Ordering::Release);
        wait(&next_human);
        let awaiting = source_reader.snapshot();
        assert_eq!(
            awaiting.executions, 1,
            "governor cannot substitute for human approval"
        );
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), result);
        assert_eq!(
            fs::read(directory.join("store/delivery.bin")).unwrap(),
            canonical,
            "promotion retry disturbed the fresh second review"
        );
        release_human.store(true, Ordering::Release);
    });
    let result = resident::command_with_clock(&args, None, || {
        let admitted = promoted.load(Ordering::Acquire);
        let second_human = human_ready.load(Ordering::Acquire).then_some(decision);
        peers.poll(
            [
                Some(Actor::Observe),
                admitted.then_some(Actor::Observe),
                admitted.then_some(Actor::Observe),
            ],
            [
                Some(ReviewDecision::Approve),
                second_human,
                Some(ReviewDecision::Approve),
            ],
        );
        ElapsedTick(1000)
    });
    operator_peer.join().unwrap();
    child.finish();
    let (replies, packets) = peers.join();
    assert!(confirmed(replies[0].as_ref().unwrap()));
    let first = packets[0].as_ref().unwrap();
    let second = packets[1].as_ref().unwrap();
    assert_ne!(first.binding().session, second.binding().session);
    assert_eq!(first.action().spec().policy_epoch, 0);
    assert_eq!(second.action().spec().policy_epoch, 1);
    assert_eq!(second.policy_generation(), 2);
    assert_eq!(second.views().len(), 2);
    let second_frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
    assert_eq!(second_frame.prior_messages(), &["A"]);
    assert_eq!(second_frame.message(), Some("A"));
    let state = reader.snapshot();
    if decision == ReviewDecision::Approve {
        assert!(result.is_ok(), "{result:?}");
        assert!(
            replies
                .iter()
                .all(|reply| confirmed(reply.as_ref().unwrap()))
        );
        assert_eq!(
            (state.executions, state.payload.as_slice()),
            (3, b"AA".as_slice())
        );
        assert_eq!(
            state.control.ledger.charged,
            packets
                .iter()
                .map(|packet| packet.as_ref().unwrap().action().spec().units)
                .sum::<u64>()
        );
    } else {
        assert!(result.is_err());
        assert!(!confirmed(replies[1].as_ref().unwrap()));
        assert!(replies[2].is_none() && packets[2].is_none());
        assert_eq!(
            (state.executions, state.payload.as_slice()),
            (1, b"A".as_slice())
        );
        assert_eq!(state.control.ledger.charged, first.action().spec().units);
    }
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(
        state.control.ledger.available + state.control.ledger.charged,
        reader.config.profile.delivery.total
    );
    // Verify the retained original numerical owner, after the live operation.
    reader.loaded.expected_policy = policy();
    reader.loaded.expected_campaigns = Some(governor.campaign_requirement());
    reader.loaded.recovery_anchor =
        Some(super::super::super::anchors::load(&root.0.join("final.anchor")).unwrap());
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(
        (numerical.position, numerical.sampled_draws),
        (6, 4),
        "promotion must not regenerate a ready Message or replenish generation work"
    );
    assert_eq!(host.current_policy().unwrap(), &policy());
    quiescent(&root);
}

#[test]
fn governed_learned_command_rejects_unsupported_or_unfunded_modes_before_assets_and_time() {
    for case in 0..7 {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, None, None);
        let governor_pid = std::process::id().checked_add(10_000).unwrap();
        let (governor, operator, campaign) = selections(&root, &entries[0].actor, governor_pid);
        let mut args = inputs(&root, &config, &paths, &entries, None);
        let expected = match case {
            0 => {
                args.push("--continue-generation".into());
                "new nonpredictive"
            }
            1 => {
                args.extend([
                    "--topology-profile".into(),
                    root.0.join("absent-topology.json").display().to_string(),
                ]);
                "nonmediated"
            }
            2 => {
                // The later recipe must refuse before the FIRST recipe's assets.
                super::super::forecast::install(&root, &paths[1], false, 60_000);
                "nonpredictive"
            }
            3 => {
                let original = fs::read_to_string(&paths[1]).unwrap();
                fs::write(&paths[1], format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{{"id":1,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":1000000,"maximum_false_stop_ppm":1000000,"max_cases":4,"max_member_outcomes":16}},"recipe":{original}}}"#)).unwrap();
                "joint credibility"
            }
            4 => {
                let original = fs::read_to_string(&governor).unwrap();
                fs::write(
                    &governor,
                    original.replace(
                        ",\"numerical_replay\":{\"max_replay_events\":4096,\"events\":8192}",
                        "",
                    ),
                )
                .unwrap();
                "numerical"
            }
            5 => {
                let original = fs::read_to_string(&operator).unwrap();
                fs::write(
                    &operator,
                    original.replace(",\"numerical_replay_events\":4096", ""),
                )
                .unwrap();
                "numerical"
            }
            6 => {
                let original = fs::read_to_string(&campaign).unwrap();
                fs::write(&campaign, original.replace("\"cases\":16", "\"cases\":15")).unwrap();
                "limits must match"
            }
            _ => unreachable!(),
        };
        fs::remove_file(root.0.join("weights.safetensors")).unwrap();
        fs::remove_file(root.0.join("native-roster.json")).unwrap();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let error = resident::command_with_clock(&args, None, || {
            panic!("unsupported governed mode reached the clock")
        })
        .unwrap_err();
        assert!(error.contains(expected), "case {case}: {error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("final.anchor").exists());
        quiescent(&root);
    }
}

#[test]
fn governed_learned_preflight_checks_later_roles_and_retained_output_before_startup() {
    for collision in 0..5 {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, None, None);
        let (governor, _, _) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let mut args = inputs(&root, &config, &paths, &entries, None);
        let original = fs::read_to_string(&governor).unwrap();
        let socket = root.0.join("peers/governor.sock");
        let changed = match collision {
            0 => entries[1].actor.socket.clone(),
            1 => fixture::peers(&root, &config).socket(entries[2].loaded.request),
            2 => socket_path(&fixture::peers(&root, &config), entries[2].loaded.request),
            3 => {
                *args.last_mut().unwrap() = socket.display().to_string();
                socket.clone()
            }
            4 => {
                let old = format!("\"pid\":{}", std::process::id() + 10_000);
                fs::write(
                    &governor,
                    original.replace(&old, &format!("\"pid\":{}", std::process::id())),
                )
                .unwrap();
                socket.clone()
            }
            _ => unreachable!(),
        };
        if collision < 3 {
            fs::write(
                &governor,
                original.replace(socket.to_str().unwrap(), changed.to_str().unwrap()),
            )
            .unwrap();
        }
        let result = resident::command_with_clock(&args, None, || {
            panic!("colliding governor custody reached time")
        });
        assert!(result.is_err(), "collision {collision} was admitted");
        assert!(!config.store.exists());
        quiescent(&root);
    }
}
