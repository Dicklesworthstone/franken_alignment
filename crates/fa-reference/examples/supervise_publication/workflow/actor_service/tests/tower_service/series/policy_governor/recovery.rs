//! Original published history, retained external custody, and a fresh separate
//! governor process. Helper votes and explicit human decisions are fixtures.
use super::*;
use crate::publication::PublicationProfile;
use crate::recover_stop::guarded::SubmitProfile;
use crate::workflow::actor_service::policy_recovery;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FilePublicationCapture, PublicationInputFile,
};
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, RecoveryReserve};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::ReviewWindow;
use fa_reference::action::{ActionSpec, VERSION};
use fa_reference::round::{Verdict, commitment};
use fa_reference::witness::{QueryRole, WitnessRequest};
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;

const RECOVERY_ROOT: &str = "FA_RECOVERED_GOVERNOR_E2E_ROOT";
const RECOVERY_CHILD: &str = "workflow::actor_service::tests::tower_service::series::policy_governor::recovery::independent_recovered_governor_process";
const RECOVERY_CAMPAIGN: u64 = 901;
const RECOVERY_OPERATION: u64 = 902;

fn start_governor(root: &Directory) -> GovernorChild {
    GovernorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", RECOVERY_CHILD, "--nocapture"])
            .env(RECOVERY_ROOT, &root.0)
            .spawn()
            .unwrap(),
    )
}

fn policy() -> Policy {
    Policy::new(
        2,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(6),
            Predicate::UnitsAtMost(512),
            Predicate::All(vec![0, 1, 2]),
        ],
    )
    .unwrap()
}

fn seed(
    root: &Directory,
    config: &mut Config,
    governor: &governor::Settings,
    publication: Option<&PublicationProfile>,
) -> (SubmitProfile, Vec<u8>) {
    let guards = FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: Some(config.source_policy),
        identity: None,
        campaigns: Some(governor.campaign_requirement()),
        credential: None,
    };
    let (mut host, roles) = match publication {
        Some(profile) => FileOversight::create_guarded_publication_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            RecoveryReserve::terminal(),
            profile.limits,
        ),
        None => FileOversight::create_guarded_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            RecoveryReserve::terminal(),
        ),
    }
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
        .unwrap();
    let observation = config.source.read().unwrap();
    let action = host
        .propose(
            host.revision(),
            1,
            ActionSpec {
                version: VERSION,
                scope: config.profile.delivery.scope,
                target: Some(config.profile.delivery.target),
                payload: b"seed".to_vec(),
                required_witnesses: Vec::new(),
                policy_epoch: 0,
                deadline: ElapsedTick(100000),
                units: 7,
            },
            observation.snapshot().clone(),
        )
        .unwrap();
    let inputs = observation
        .inputs_for(&action, &config.profile.committee)
        .unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone())
        .unwrap();
    let reference = observation.reference_root();
    host.begin_review(
        host.revision(),
        1,
        101,
        reference,
        ReviewWindow {
            commit_by: ElapsedTick(1100),
            reveal_by: ElapsedTick(1200),
        },
        observation.snapshot().clone(),
    )
    .unwrap();
    for member in config.profile.committee.members().keys() {
        let salt = format!("original-seed-{member}").into_bytes();
        let digest = commitment(101, member, &reference, Verdict::Allow, &salt).unwrap();
        host.commit_review(host.revision(), 101, member, digest)
            .unwrap();
    }
    host.open_reveals(host.revision(), 101).unwrap();
    for member in config.profile.committee.members().keys() {
        host.reveal_review(
            host.revision(),
            101,
            member,
            Verdict::Allow,
            format!("original-seed-{member}").into_bytes(),
        )
        .unwrap();
    }
    host.finish_review(
        host.revision(),
        101,
        Some(&inputs),
        observation.snapshot().clone(),
    )
    .unwrap()
    .unwrap();
    if publication.is_some() {
        let bytes = checked::capture(config, 1, 0, b"seed");
        write_complete(&root.0.join("original.bin"), &bytes);
        write_complete(&root.0.join("current.bin"), &bytes);
        host.bind_publication_file_source(
            host.revision(),
            1,
            FilePublicationCapture::from_bytes(&bytes).unwrap(),
            vec![
                WitnessRequest::ExactValue {
                    key: 0,
                    role: QueryRole::Subject,
                },
                WitnessRequest::AbsentKey { key: 1 },
                WitnessRequest::RangeMembers { start: 2, end: 6 },
            ],
        )
        .unwrap();
        let reader = PublicationInputFile::new(root.0.join("current.bin"), 91).unwrap();
        host.refresh_publication_from_file(host.revision(), 1, &reader)
            .unwrap()
            .unwrap();
    }
    let automatic = host
        .authorize(host.revision(), 1, &inputs, observation.snapshot().clone())
        .unwrap();
    let request = host
        .request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(2000))
        .unwrap();
    let revision = host.revision();
    let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        observation.snapshot().clone(),
    )
    .unwrap();
    let published = if publication.is_some() {
        host.publish_checked(
            host.revision(),
            1,
            Some(&inputs),
            observation.snapshot().clone(),
            ElapsedTick(1001),
        )
        .unwrap()
        .outcome
    } else {
        host.publish(host.revision(), 1).unwrap()
    };
    assert_eq!(
        published,
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(published)
    );
    let cut = host.inspect();
    let update = PolicyUpdate::new(
        501,
        cut.control.sequence,
        cut.control.ledger.epoch,
        policy(),
    )
    .unwrap();
    let report = host
        .request_policy_campaign(host.revision(), &update)
        .unwrap();
    assert_eq!(report.report().cases().len(), 2);
    let revision = host.revision();
    let key = roles
        .policy_governor
        .unwrap()
        .approve(&mut host, revision, &report, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    let before = host.inspect();
    assert_eq!(
        (
            before.executions,
            before.control.ledger.charged,
            before.control.ledger.epoch
        ),
        (1, 7, 1)
    );
    assert_eq!(host.current_policy().unwrap(), &policy());
    let mut anchor = Vec::new();
    host.history_anchor()
        .unwrap()
        .write_to(&mut anchor, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    write_complete(&root.0.join("before.anchor"), &anchor);
    // The independent expectations name the actual acknowledged effective
    // policy and original campaign gate; they never instruct a promotion.
    let document = format!(
        concat!(
            "{{\"schema_version\":\"fa.guarded-submit/2\",\"minimum\":{{",
            "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
            "\"history_anchor\":{:?},\"retain_history_anchor\":{:?},",
            "\"effective_policy\":{{\"generation\":2,\"nodes\":[",
            "{{\"op\":\"exact_value\",\"key\":7,\"value_hex\":\"6f6b\"}},",
            "{{\"op\":\"payload_at_most\",\"maximum\":6}},",
            "{{\"op\":\"units_at_most\",\"maximum\":512}},",
            "{{\"op\":\"all\",\"children\":[0,1,2]}}]}},",
            "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":4}}}}"
        ),
        before.revision,
        before.control.sequence,
        before.control.ledger.epoch,
        root.0.join("before.anchor").to_str().unwrap(),
        root.0.join("after.anchor").to_str().unwrap()
    );
    let profile = SubmitProfile::decode(document.as_bytes()).unwrap();
    drop(host);
    (profile, anchor)
}

fn known<'a>(response: &'a Json, phase: &str) -> &'a Json {
    assert_eq!(
        field(response, "schema").as_str(),
        Some("fa.policy-governor-response/1")
    );
    assert_eq!(field(response, "phase").as_str(), Some(phase));
    let knowledge = field(response, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_eq!(
        field(field(knowledge, "basis"), "origin").as_str(),
        Some("acknowledged_owner")
    );
    field(knowledge, "value")
}

#[test]
fn independent_recovered_governor_process() {
    let Some(root) = std::env::var_os(RECOVERY_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("governor-ready.json"));
    let scope = governor_json(&fs::read(root.join("governor-ready.json")).unwrap());
    assert!(!root.join("evidence.json").exists());
    let request = root.join("governor-inspect.json");
    write_complete(
        &request,
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{},"verb":"get","query":"31"}}"#,
            governor::protocol::json(&scope)
        ),
    );
    let inspected = custodian_exchange(&root, &request);
    assert_eq!(field(&inspected, "query").as_str(), Some("31"));
    let cut = known(&inspected, "inspect");
    assert_eq!(field(cut, "policy_generation").as_str(), Some("2"));
    assert_eq!(field(cut, "authority_epoch").as_str(), Some("2"));
    assert_eq!(field(cut, "stopped").as_bool(), Some(false));
    assert_eq!(field(cut, "source_interrupted").as_bool(), Some(false));
    let request = root.join("governor-review.json");
    write_complete(
        &request,
        format!(
            concat!(
                "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},",
                "\"verb\":\"policy-replay\",\"operation\":\"901\",\"expected\":{{",
                "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{},\"policy_generation\":\"2\"}},",
                "\"candidate\":{{\"schema_version\":\"fa.policy-replay-request/1\",\"expected_policy_generation\":2,",
                "\"limits\":{{\"cases\":16,\"input_bytes\":1048576,\"output_bytes\":65536}},",
                "\"candidate\":{{\"generation\":3,\"nodes\":[{{\"op\":\"exact_value\",\"key\":7,\"value_hex\":\"6f6b\"}},",
                "{{\"op\":\"payload_at_most\",\"maximum\":6}},{{\"op\":\"units_at_most\",\"maximum\":512}},",
                "{{\"op\":\"all\",\"children\":[0,1,2]}}]}}}}}}"
            ),
            governor::protocol::json(&scope),
            governor::protocol::json(field(cut, "journal_revision")),
            governor::protocol::json(field(cut, "control_sequence")),
            governor::protocol::json(field(cut, "authority_epoch"))
        ),
    );
    let reviewed = custodian_exchange(&root, &request);
    assert_eq!(field(&reviewed, "campaign").as_str(), Some("901"));
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = governor_json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(4));
    assert_eq!(field(replay, "cases").as_array().unwrap().len(), 4);
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "charged").as_u64(), Some(14));
    assert_eq!(
        field(field(replay, "previous_policy"), "generation").as_u64(),
        Some(2)
    );
    assert_eq!(
        field(field(replay, "candidate_policy"), "generation").as_u64(),
        Some(3)
    );
    assert!(
        field(replay, "newly_reviewable")
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert!(!root.join("evidence.json").exists());
    // This fresh fixture custodian approves this exact reviewed report only.
    let request = root.join("governor-approve.json");
    write_complete(
        &request,
        format!(
            concat!(
                "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},",
                "\"verb\":\"promote\",\"phase\":\"approve\",\"campaign\":\"901\",",
                "\"expected_journal_revision\":{},\"reviewed_report\":{},\"accept_newly_reviewable\":false}}"
            ),
            governor::protocol::json(&scope),
            governor::protocol::json(field(value, "journal_revision")),
            governor::protocol::quote(report)
        ),
    );
    let approved = custodian_exchange(&root, &request);
    let value = known(&approved, "approve");
    assert_eq!(field(value, "approval").as_str(), Some("delegated"));
    assert_eq!(field(value, "policy_changed").as_bool(), Some(false));
    assert_eq!(field(value, "effect_authority").as_bool(), Some(false));
    write_complete(
        &root.join("governor-approved.json"),
        governor::protocol::json(&approved),
    );
}

fn next_recovered(scope: Scope) -> String {
    let mut request = TowerRequest::new(
        "recovered-original-promotion",
        Address::parse(&format!(
            "fa://{}/run/{}@{GENERATION}",
            scope.tenant, scope.run
        ))
        .unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::PolicyPromotionNext {
        campaign: RECOVERY_CAMPAIGN,
        operation: RECOVERY_OPERATION,
    };
    encode_request(&request).unwrap()
}

fn human_at(
    profile: PeerProfile,
    request: u64,
    generation: u64,
    epoch: u64,
    payload: &'static [u8],
    hold: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap();
                    assert_eq!(packet.policy_generation(), generation);
                    assert_eq!(packet.action().spec().policy_epoch, epoch);
                    assert_eq!(packet.action().spec().payload, payload);
                    assert_eq!(packet.views().len(), 2);
                    if let Some((offered, approve)) = &hold {
                        offered.send(()).unwrap();
                        approve.recv_timeout(Duration::from_secs(10)).unwrap();
                    }
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

#[test]
fn independently_anchored_promoted_history_recovers_a_fresh_governor_and_new_reviews() {
    recovered_pair(false);
}

#[test]
fn independently_anchored_checked_history_retains_publication_guard_through_fresh_promotion() {
    recovered_pair(true);
}

fn recovered_pair(checked_mode: bool) {
    let root = Directory::new();
    let mut config = configured(&root);
    evidence(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let operator = operator_settings(&root, &config);
    let mut child = start_governor(&root);
    assert_ne!(child.0.id(), std::process::id());
    let document = governor_document(&root, &config, child.0.id());
    let governor = governor::Settings::decode(document.as_bytes()).unwrap();
    write_complete(&root.0.join("governor.json"), document);
    let publication = checked_mode
        .then(|| PublicationProfile::decode(checked::profile_document(&root).as_bytes()).unwrap());
    let (recovery, original_anchor) = seed(&root, &mut config, &governor, publication.as_ref());
    let source = root.0.join("evidence.json");
    let source_bytes = fs::read(&source).unwrap();
    fs::remove_file(&source).unwrap();
    let first_capture = checked_mode.then(|| checked::capture(&config, 2, 2, b"first"));
    let second_capture = checked_mode.then(|| checked::capture(&config, 3, 3, b"second"));
    let first = super::super::document(&config, 7, 2, "6669727374").replace(
        "\"expected_policy_epoch\":\"0\"",
        "\"expected_policy_epoch\":\"2\"",
    );
    let second = super::super::document(&config, 8, 3, "7365636f6e64").replace(
        "\"expected_policy_epoch\":\"0\"",
        "\"expected_policy_epoch\":\"3\"",
    );
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let selected_scope = config.profile.delivery.scope;
    let bootstrap = config.profile.clone();
    let store = config.store.clone();
    let directory = root.0.clone();
    let actor_profile = actor.clone();
    let operator_profile = operator.clone();
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let (offered_send, offered_receive) = mpsc::channel();
    let (approve_send, approve_receive) = mpsc::channel();
    let first_human = human_at(reviewer.clone(), 7, 2, 2, b"first", None);
    let second_human = human_at(
        reviewer.clone(),
        8,
        3,
        3,
        b"second",
        Some((offered_send, approve_receive)),
    );
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&actor_profile);
        wait(&operator_profile.socket);
        let mut operator = UnixStream::connect(&operator_profile.socket).unwrap();
        operator_profile.supervisor.verify(&operator).unwrap();
        let unavailable = raw_exchange(&mut operator, &next_recovered(selected_scope));
        assert_eq!(
            field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported")
        );
        let recovered = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(
            (
                recovered.executions,
                recovered.control.ledger.charged,
                recovered.control.ledger.epoch
            ),
            (1, 7, 2)
        );
        assert_eq!(recovered.payload, b"seed");
        assert!(!source.exists());
        write_complete(&source, &source_bytes);
        if let Some(capture) = first_capture {
            write_complete(&directory.join("original.bin"), &capture);
            write_complete(&directory.join("current.bin"), &capture);
        }
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&next_control);
        let before = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(
            (
                before.executions,
                before.control.ledger.charged,
                before.control.ledger.epoch
            ),
            (2, 14, 2)
        );
        fs::remove_file(&source).unwrap();
        // Only the scope and readiness travel out of band. The custodian gets
        // the actual current predecessor from its authenticated Get exchange.
        write_complete(
            &directory.join("governor-ready.json"),
            governor::protocol::scope_json(selected_scope),
        );
        wait(&directory.join("governor-approved.json"));
        let approved = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(
            (
                approved.executions,
                approved.control.ledger.charged,
                approved.control.ledger.epoch
            ),
            (2, 14, 2)
        );
        let saved = store_files(&store);
        let planned = raw_exchange(&mut operator, &next_recovered(selected_scope));
        let plan = field(known_operator(&planned, "policy-promotion-next"), "value");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        assert_eq!(
            field(plan, "candidate_policy_generation").as_str(),
            Some("3")
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
            Some("3")
        );
        assert_eq!(
            field(field(promotion, "basis"), "authority_epoch").as_str(),
            Some("3")
        );
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.epoch
            ),
            (2, 14, 3)
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
        let awaiting = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(awaiting.executions, 2);
        assert_eq!(awaiting.payload, b"first");
        let saved = store_files(&store);
        assert_eq!(raw_exchange(&mut operator, &command), promoted);
        assert_eq!(
            store_files(&store),
            saved,
            "old promotion retry disturbed the fresh original review"
        );
        approve_send.send(()).unwrap();
        assert!(executed(&finish(&mut actor, &second_poll, &second_poll)));
    });
    let result = policy_recovery::serve(
        config,
        &actor,
        &reviewer,
        publication.as_ref(),
        None,
        &recovery,
        &[7, 8],
        GENERATION,
        &operator,
        &governor,
        || ElapsedTick(2000),
    );
    let peer_result = peer.join();
    let first_human_result = first_human.join();
    let second_human_result = second_human.join();
    let governor_status = child.finish();
    result.unwrap();
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
            disk.control.ledger.reserved,
            disk.control.ledger.epoch
        ),
        (3, 21, 0, 3)
    );
    assert_eq!(
        disk.control.ledger.available,
        config.profile.delivery.total - 21
    );
    assert_eq!(disk.control.ledger.stages.len(), 3);
    assert_eq!(disk.target.expected_version, 4);
    assert_eq!(disk.payload, b"second");
    assert!(disk.stop.is_none());
    let output = root.0.join("after.anchor");
    let retained = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(&output).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(retained.revision(), disk.revision);
    assert_eq!(
        retained.retained_bytes(),
        fs::read(config.store.join("delivery.bin")).unwrap().len()
    );
    assert_eq!(
        fs::metadata(output).unwrap().permissions().mode() & 0o077,
        0
    );
    assert_eq!(
        fs::read(root.0.join("before.anchor")).unwrap(),
        original_anchor
    );
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
