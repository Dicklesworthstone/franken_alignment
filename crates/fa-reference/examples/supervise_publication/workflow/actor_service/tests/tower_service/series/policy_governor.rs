//! A real separately authenticated governor approves the retained first effect,
//! and the same live operator installs its policy before a newly reviewed second
//! effect. Helper verdicts and explicit custodian choices are test fixtures.
use super::super::operator::{settings_document, store_files};
use super::*;
use crate::workflow::actor_service::{governor, operator};
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_PROMOTION_REPLAY_EVENTS;
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::process::{Child, Command as ProcessCommand, ExitStatus};
use std::sync::mpsc;

#[path = "policy_governor/checked.rs"]
mod checked;
#[path = "policy_governor/recovery.rs"]
mod recovery;

const CHILD_ROOT: &str = "FA_POLICY_GOVERNOR_E2E_ROOT";
const CHILD_TEST: &str = "workflow::actor_service::tests::tower_service::series::policy_governor::independent_governor_process";
const CAMPAIGN: u64 = 801;
const OPERATION: u64 = 802;

struct GovernorChild(Child);
impl GovernorChild {
    fn start(root: &Directory) -> Self {
        Self(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD_TEST, "--nocapture"])
                .env(CHILD_ROOT, &root.0)
                .spawn()
                .unwrap(),
        )
    }
    fn finish(&mut self) -> ExitStatus {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "governor child did not exit"
            );
            pause(1);
        }
    }
}
impl Drop for GovernorChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            if let Err(error) = self.0.kill() {
                eprintln!("governor child termination: {error}");
            }
            if let Err(error) = self.0.wait() {
                eprintln!("governor child reap: {error}");
            }
        }
    }
}

fn governor_document(root: &Directory, config: &Config, pid: u32) -> String {
    let identity = credentials();
    format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-service/1\",\"scope\":{},",
            "\"socket\":\"{}/actors/governor.sock\",",
            "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
            "\"governor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
            "\"candidate_limit\":4,\"connection_limit\":4,\"exchange_limit\":4,",
            "\"runtime_ms\":15000,\"poll_ms\":1,\"campaign_limit\":1,\"report_bytes\":65536,",
            "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":4}}}}"
        ),
        scope(config),
        root.0.display(),
        identity.uid(),
        identity.gid(),
        identity.pid(),
        identity.uid(),
        identity.gid(),
        pid
    )
}

fn operator_settings(root: &Directory, config: &Config) -> operator::Settings {
    let mut settings = operator::Settings::decode(
        settings_document(root, config, 0, 0)
            .replacen(
                '{',
                &format!(
                    concat!(
                        "{{\"policy_promotion\":{{\"queries\":2,",
                        "\"max_replay_events\":{},\"authority\":\"promote_approved_policy\"}},"
                    ),
                    MAX_POLICY_PROMOTION_REPLAY_EVENTS
                ),
                1,
            )
            .as_bytes(),
    )
    .unwrap();
    settings.candidate_limit = 1;
    settings.connection_limit = 1;
    settings
}

fn write_complete(path: &Path, bytes: impl AsRef<[u8]>) {
    let pending = path.with_extension("pending");
    fs::write(&pending, bytes).unwrap();
    fs::rename(pending, path).unwrap();
}

fn governor_json(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: governor::protocol::MAX_RESPONSE_BYTES,
            max_depth: 32,
            max_items: 16_512,
            max_string_bytes: governor::protocol::MAX_REPORT_BYTES,
        },
    )
    .unwrap()
}

fn custodian_exchange(root: &Path, request: &Path) -> Json {
    let mut output = Vec::new();
    crate::command_with_output(
        vec![
            "policy-governor-exchange".into(),
            root.join("governor.json").to_str().unwrap().into(),
            request.to_str().unwrap().into(),
        ],
        &mut output,
    )
    .unwrap();
    assert_eq!(output.last(), Some(&b'\n'));
    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    governor_json(&output)
}

fn governor_known<'a>(response: &'a Json, phase: &str) -> &'a Json {
    assert_eq!(
        field(response, "schema").as_str(),
        Some("fa.policy-governor-response/1")
    );
    assert_eq!(field(response, "phase").as_str(), Some(phase));
    assert_eq!(field(response, "campaign").as_str(), Some("801"));
    let knowledge = field(response, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    field(knowledge, "value")
}

#[test]
fn independent_governor_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let request = root.join("governor-review.json");
    wait(&request);
    let reviewed = custodian_exchange(&root, &request);
    let value = governor_known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = governor_json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    let cases = field(replay, "cases").as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(
        field(field(&cases[0], "id"), "kind").as_str(),
        Some("proposal")
    );
    assert_eq!(
        field(field(&cases[1], "id"), "kind").as_str(),
        Some("review")
    );
    assert_eq!(
        field(field(&cases[0], "id"), "attempt"),
        field(field(&cases[1], "id"), "attempt")
    );
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(1));
    assert_eq!(field(field(replay, "cut"), "charged").as_u64(), Some(7));
    assert_eq!(
        field(field(replay, "previous_policy"), "generation").as_u64(),
        Some(1)
    );
    assert_eq!(
        field(field(replay, "candidate_policy"), "generation").as_u64(),
        Some(2)
    );
    assert!(
        field(replay, "newly_reviewable")
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert_eq!(field(replay, "policy_changed").as_bool(), Some(false));
    assert!(!root.join("evidence.json").exists());
    // This explicit fixture decision approves exactly the report just checked.
    // The product client performs one requested exchange and never auto-approves.
    let approval = format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-request/1\",",
            "\"scope\":{},\"verb\":\"promote\",\"phase\":\"approve\",\"campaign\":\"801\",",
            "\"expected_journal_revision\":{},\"reviewed_report\":{},\"accept_newly_reviewable\":false}}"
        ),
        governor::protocol::json(field(&reviewed, "scope")),
        governor::protocol::json(field(value, "journal_revision")),
        governor::protocol::quote(report)
    );
    let request = root.join("governor-approve.json");
    write_complete(&request, approval);
    let approved = custodian_exchange(&root, &request);
    let value = governor_known(&approved, "approve");
    assert_eq!(field(value, "approval").as_str(), Some("delegated"));
    assert_eq!(field(value, "policy_changed").as_bool(), Some(false));
    assert_eq!(field(value, "effect_authority").as_bool(), Some(false));
    write_complete(
        &root.join("governor-approved.json"),
        governor::protocol::json(&approved),
    );
}

fn review_document(cut: &FileDeliverySnapshot, scope: Scope) -> String {
    format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},",
            "\"verb\":\"policy-replay\",\"operation\":\"801\",\"expected\":{{",
            "\"journal_revision\":\"{}\",\"control_sequence\":\"{}\",",
            "\"authority_epoch\":\"{}\",\"policy_generation\":\"1\"}},",
            "\"candidate\":{{\"schema_version\":\"fa.policy-replay-request/1\",",
            "\"expected_policy_generation\":1,\"limits\":{{\"cases\":16,",
            "\"input_bytes\":1048576,\"output_bytes\":65536}},\"candidate\":{{",
            "\"generation\":2,\"nodes\":[{{\"op\":\"exact_value\",\"key\":7,\"value_hex\":\"6f6b\"}},",
            "{{\"op\":\"payload_at_most\",\"maximum\":6}},{{\"op\":\"units_at_most\",\"maximum\":512}},",
            "{{\"op\":\"all\",\"children\":[0,1,2]}}]}}}}}}"
        ),
        governor::protocol::scope_json(scope),
        cut.revision,
        cut.control.sequence,
        cut.control.ledger.epoch
    )
}

fn next(scope: Scope) -> String {
    let mut request = TowerRequest::new(
        "original-policy-promotion",
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
    encode_request(&request).unwrap()
}

fn known_operator<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let knowledge = field(field(result, "value"), "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    knowledge
}

#[test]
fn independent_live_governor_promotes_between_two_publications_and_requires_fresh_human_review() {
    governed_pair(false);
}

#[test]
fn independent_live_governor_preserves_checked_publication_across_policy_promotion() {
    governed_pair(true);
}

fn governed_pair(checked: bool) {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let publication = checked.then(|| checked::prepare(&root, &config));
    let second_capture = publication.as_ref().map(|(_, bytes)| bytes.clone());
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let operator = operator_settings(&root, &config);
    let mut governor_child = GovernorChild::start(&root);
    let document = governor_document(&root, &config, governor_child.0.id());
    let governor = governor::Settings::decode(document.as_bytes()).unwrap();
    write_complete(&root.0.join("governor.json"), document);
    assert_ne!(governor_child.0.id(), std::process::id());
    let first = super::document(&config, 7, 1, "6669727374");
    let second = super::document(&config, 8, 2, "7365636f6e64").replace(
        "\"expected_policy_epoch\":\"0\"",
        "\"expected_policy_epoch\":\"1\"",
    );
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let selected_scope = config.profile.delivery.scope;
    let profile = config.profile.clone();
    let store = config.store.clone();
    let directory = root.0.clone();
    let actor_profile = actor.clone();
    let operator_profile = operator.clone();
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let (offered_send, offered_receive) = mpsc::channel();
    let (approve_send, approve_receive) = mpsc::channel();
    let fresh_reviewer = reviewer.clone();
    let second_human = std::thread::spawn(move || {
        let mut client = fresh_reviewer
            .connect_client_waiting(8, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap();
                    assert_eq!(packet.policy_generation(), 2);
                    assert_eq!(packet.action().spec().policy_epoch, 1);
                    assert_eq!(packet.action().spec().payload, b"second");
                    assert_eq!(packet.views().len(), 2);
                    offered_send.send(()).unwrap();
                    approve_receive
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap();
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    });
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&actor_profile);
        wait(&operator_profile.socket);
        let mut operator = UnixStream::connect(&operator_profile.socket).unwrap();
        operator_profile.supervisor.verify(&operator).unwrap();
        let unavailable = raw_exchange(&mut operator, &next(selected_scope));
        assert_eq!(
            field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported")
        );
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&next_control);
        let before = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(
            (
                before.executions,
                before.control.ledger.charged,
                before.control.ledger.epoch
            ),
            (1, 7, 0)
        );
        let source = directory.join("evidence.json");
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        write_complete(
            &directory.join("governor-review.json"),
            review_document(&before, selected_scope),
        );
        wait(&directory.join("governor-approved.json"));
        let approval = governor_json(&fs::read(directory.join("governor-approved.json")).unwrap());
        governor_known(&approval, "approve");
        let approved = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(
            (
                approved.executions,
                approved.control.ledger.charged,
                approved.control.ledger.epoch
            ),
            (1, 7, 0)
        );
        let saved = store_files(&store);
        let planned = raw_exchange(&mut operator, &next(selected_scope));
        let plan = field(known_operator(&planned, "policy-promotion-next"), "value");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        assert_eq!(
            field(plan, "candidate_policy_generation").as_str(),
            Some("2")
        );
        let command = governor::protocol::json(field(plan, "action"));
        assert_eq!(store_files(&store), saved);
        let denied = raw_exchange(&mut actor, &command);
        assert_eq!(
            field(field(&denied, "envelope"), "reason_code").as_str(),
            Some("DeniedByPolicy")
        );
        assert_eq!(store_files(&store), saved);
        let promoted = raw_exchange(&mut operator, &command);
        let promotion = known_operator(&promoted, "policy-promotion");
        assert_eq!(
            field(field(promotion, "basis"), "policy_generation").as_str(),
            Some("2")
        );
        assert_eq!(
            field(field(promotion, "basis"), "authority_epoch").as_str(),
            Some("1")
        );
        assert!(
            field(field(promotion, "value"), "cancelled")
                .as_array()
                .unwrap()
                .is_empty()
        );
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.sequence, approved.control.sequence + 1);
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.epoch
            ),
            (1, 7, 1)
        );
        assert!(!source.exists());
        assert!(executed(&raw_exchange(&mut actor, &first)));
        write_complete(&source, source_bytes);
        if let Some(capture) = second_capture {
            write_complete(&directory.join("original.bin"), &capture);
            write_complete(&directory.join("current.bin"), &capture);
        }
        let pending = raw_exchange(&mut actor, &second);
        assert_eq!(
            field(knowledge(&pending), "state").as_str(),
            Some("pending")
        );
        offered_receive
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        let awaiting = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(awaiting.executions, 1);
        assert_eq!(awaiting.payload, b"first");
        let saved = store_files(&store);
        assert_eq!(raw_exchange(&mut operator, &command), promoted);
        assert_eq!(
            store_files(&store),
            saved,
            "old promotion retry disturbed the new original human review"
        );
        approve_send.send(()).unwrap();
        assert!(executed(&finish(&mut actor, &second_poll, &second_poll)));
    });
    let first_human = human_for(reviewer.clone(), 7);
    let served = service::serve_tower_governed_checked(
        config,
        &actor,
        &reviewer,
        publication.as_ref().map(|(profile, _)| profile),
        &[7, 8],
        GENERATION,
        &operator,
        &governor,
        || ElapsedTick(1000),
    );
    let peer_result = peer.join();
    let first_human_result = first_human.join();
    let second_human_result = second_human.join();
    let governor_status = governor_child.finish();
    served.unwrap();
    peer_result.unwrap();
    first_human_result.unwrap();
    second_human_result.unwrap();
    assert!(governor_status.success());
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            disk.executions,
            disk.control.ledger.charged,
            disk.control.ledger.reserved
        ),
        (2, 14, 0)
    );
    assert_eq!(
        disk.control.ledger.available,
        config.profile.delivery.total - 14
    );
    assert_eq!(disk.control.ledger.epoch, 1);
    assert_eq!(disk.control.ledger.stages.len(), 2);
    assert_eq!(disk.target.expected_version, 3);
    assert_eq!(disk.payload, b"second");
    assert!(disk.stop.is_none());
    for socket in [
        &actor.socket,
        &operator.socket,
        &governor.socket,
        &reviewer.socket(7),
        &reviewer.socket(8),
    ] {
        assert!(!socket.exists());
    }
}

#[test]
fn overlapping_governor_identity_refuses_before_clock_store_or_any_socket() {
    for unpinned in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let mut document = governor_document(&root, &config, std::process::id());
        if unpinned {
            let identity = credentials();
            document = document.replace(
                &format!(
                    "\"governor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
                    identity.uid(),
                    identity.gid(),
                    identity.pid()
                ),
                &format!(
                    "\"governor\":{{\"uid\":{},\"gid\":{},\"pid\":null}}",
                    identity.uid(),
                    identity.gid()
                ),
            );
        }
        let governor = governor::Settings::decode(document.as_bytes()).unwrap();
        let error = service::serve_tower_governed(
            config,
            &actor,
            &reviewer,
            &[7, 8],
            GENERATION,
            &operator,
            &governor,
            || panic!("overlapping governor observed time"),
        )
        .unwrap_err();
        assert!(error.contains("overlapping actor credentials"), "{error}");
        assert!(!root.0.join("store").exists());
        assert!(!root.0.join("evidence.json").exists());
        for socket in [
            &actor.socket,
            &operator.socket,
            &governor.socket,
            &reviewer.socket(7),
            &reviewer.socket(8),
        ] {
            assert!(!socket.exists());
        }
    }
}
