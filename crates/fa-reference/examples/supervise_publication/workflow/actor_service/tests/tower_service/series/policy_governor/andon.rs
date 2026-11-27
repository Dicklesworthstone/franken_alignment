//! The original independently evaluated owner survives a governed policy change.
//! Real helper, human, governor and evaluator transports are exercised; their
//! fixture judgments are not an empirical model-quality claim.
use super::*;
use crate::recover_stop::guarded::SubmitProfile;
use crate::workflow::actor_service::{andon as evaluated, andon_settings::Settings, evaluator};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    evaluation::andon::{FileAndonRecoveryRequirements, FileNativeAndonSnapshot},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    credibility::Fraction,
    joint_credibility::{JointPromotionPolicy, JointReplayBudget},
};
use std::io::Cursor;

const EVALUATOR_ROOT: &str = "FA_GOVERNED_ANDON_EVALUATOR_ROOT";
const EVALUATOR_TEST: &str = "workflow::actor_service::tests::tower_service::series::policy_governor::andon::independent_evaluator_process";

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| field(value, key))
}

fn number(value: &Json, path: &[&str]) -> u64 {
    at(value, path).as_str().unwrap().parse().unwrap()
}

fn settings(joint: bool) -> Settings {
    let mut selected = Settings::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/fixtures/native_andon_operator.json"
    ))).unwrap();
    if joint {
        selected.joint = Some(JointPromotionPolicy::new(
            41, 1,
            Fraction { numerator: 1, denominator: 1 },
            Fraction { numerator: 1, denominator: 1 },
            JointReplayBudget { cases: 16, member_outcomes: 64 },
        ).unwrap());
    }
    selected
}

fn evaluator_settings(root: &Directory, actor: &Profile, pid: u32) -> evaluator::Settings {
    let scope = actor.scope;
    let server = actor.supervisor;
    let document = format!(
        r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}/evaluator.sock",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "deployment":101,"evaluator":202,
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "peer":{{"uid":{},"gid":{},"pid":{pid}}},
        "candidate_limit":4,"runtime_ms":15000,"poll_ms":1}}"#,
        root.0.display(), scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        server.uid(), server.gid(), server.pid().unwrap(), server.uid(), server.gid(),
    );
    write_complete(&root.0.join("evaluator.json"), &document);
    evaluator::Settings::decode(document.as_bytes()).unwrap()
}

/// One separately authenticated process labels the old-policy case AFTER the
/// promotion and the new-policy case AFTER its independent human decision.
#[test]
fn independent_evaluator_process() {
    let Some(root) = std::env::var_os(EVALUATOR_ROOT).map(PathBuf::from) else { return; };
    let profile = root.join("evaluator.json");
    let epoch: u64 = std::env::var("FA_GOVERNED_ANDON_EVALUATOR_EPOCH").unwrap().parse().unwrap();
    for (round, generation) in [(7, 1), (8, 2)] {
        wait(&root.join(format!("evaluate-{round}")));
        assert!(!root.join("evidence.json").exists());
        let mut bytes = Vec::new();
        evaluator::client::command(
            &["evaluator-case".into(), profile.display().to_string(), round.to_string()],
            &mut bytes,
        ).unwrap();
        let case = parse(&bytes);
        assert_eq!(at(&case, &["knowledge", "state"]).as_str(), Some("known"));
        assert_eq!(at(&case, &["effect_authority"]).as_bool(), Some(false));
        assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
        assert_eq!(number(&case, &["knowledge", "value", "policy_generation"]), generation);
        assert_eq!(number(&case, &["knowledge", "basis", "authority_epoch"]), epoch);
        let value = at(&case, &["knowledge", "value"]);
        let assessment = format!(
            r#"{{"schema_version":"fa.native-assessment/1","round":{round},"attempt":{},
            "policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{}",
            "expected_evaluation_revision":{},"origin":{round},"evidence_id_hex":"{}","truth":"benign"}}"#,
            number(value, &["attempt"]), number(value, &["policy_generation"]),
            number(value, &["congress_generation"]),
            at(value, &["case_evidence_root_hex"]).as_str().unwrap(),
            number(&case, &["knowledge", "basis", "evaluation_revision"]),
            format!("{round:02x}").repeat(32),
        );
        let path = root.join(format!("assessment-{round}.json"));
        write_complete(&path, assessment);
        let mut receipt = Vec::new();
        evaluator::client::command(
            &["evaluator-assess".into(), profile.display().to_string(), path.display().to_string()],
            &mut receipt,
        ).unwrap();
        let response = parse(&receipt);
        assert_eq!(field(&response, "status").as_str(), Some("applied"));
        for coordinate in ["control_sequence", "authority_epoch"] {
            assert_eq!(at(&response, &["before", coordinate]), at(&response, &["after", coordinate]));
        }
        assert_eq!(number(&response, &["after", "evaluation_revision"]),
            number(&response, &["before", "evaluation_revision"]) + 1);
        write_complete(&root.join(format!("evaluated-{round}")), receipt);
    }
}

fn policy() -> Policy {
    Policy::new(2, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(6),
        Predicate::UnitsAtMost(512),
        Predicate::All(vec![0, 1, 2]),
    ]).unwrap()
}

fn expected(
    config: &Config,
    governor: &governor::Settings,
    settings: &Settings,
    floor: FileRecoveryFloor,
    publication: Option<&crate::publication::PublicationProfile>,
) -> FileAndonRecoveryRequirements {
    let mut required = FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: None, decoder: None, decoder_stop: None,
                source: Some(config.source_policy), identity: None,
                campaigns: Some(governor.campaign_requirement()), credential: None,
            },
            effective_policy: policy(), credential_epoch: None, minimum: floor,
        },
        settings.config.clone(), settings.windows.clone(), settings.minimum_evaluation_revision,
    ).unwrap();
    if let Some(policy) = settings.joint { required = required.require_joint(policy).unwrap(); }
    if let Some(publication) = publication {
        required = required.require_publication(publication.simple_limits().unwrap()).unwrap();
    }
    required
}

fn inspect(
    config: &Config,
    governor: &governor::Settings,
    settings: &Settings,
    publication: Option<&crate::publication::PublicationProfile>,
    path: &Path,
) -> FileNativeAndonSnapshot {
    let cut = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    let floor = FileRecoveryFloor {
        journal_revision: cut.revision, control_sequence: cut.control.sequence,
        authority_epoch: cut.control.ledger.epoch,
    };
    FileOversight::read_andon_guarded(
        &config.store, &config.profile, &expected(config, governor, settings, floor, publication),
        &settings.protocol, &anchor,
    ).unwrap()
}

#[test]
fn governed_andon_retains_independent_labels_across_plain_checked_and_joint_publication() {
    for checked in [false, true] {
        for joint in [false, true] {
            run_pair(checked, joint, false, false);
        }
    }
}

#[test]
fn approved_policy_cannot_replace_the_next_independent_human_publication_key() {
    run_pair(false, false, true, false);
}

#[test]
fn delayed_labels_span_both_policy_generations_after_the_governor_retires() {
    run_pair(true, true, false, true);
}

fn start_evaluator(root: &Directory, actor: &Profile, epoch: u64) -> (GovernorChild, evaluator::Settings) {
    let child = GovernorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", EVALUATOR_TEST, "--nocapture"])
            .env(EVALUATOR_ROOT, &root.0)
            .env("FA_GOVERNED_ANDON_EVALUATOR_EPOCH", epoch.to_string())
            .spawn().unwrap(),
    );
    let evaluator = evaluator_settings(root, actor, child.0.id());
    (child, evaluator)
}

fn run_pair(checked: bool, joint: bool, reject: bool, historical: bool) {
    let root = Directory::new();
    let config = mediated::governed_config(&root);
    evidence(&root, &config);
    let publication = checked.then(|| checked::prepare(&root, &config));
    let next_capture = publication.as_ref().map(|(_, bytes)| bytes.clone());
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    let operator = operator_settings(&root, &config);
    let mut governor_child = GovernorChild::start(&root);
    let document = governor_document(&root, &config, governor_child.0.id());
    let governor = governor::Settings::decode(document.as_bytes()).unwrap();
    write_complete(&root.0.join("governor.json"), document);
    let mut live_evaluator = (!historical).then(|| start_evaluator(&root, &actor, 1));
    let settings = settings(joint);
    let output = root.0.join("created.anchor");
    let first = super::super::document(&config, 7, 1, "6669727374");
    let second = super::super::document(&config, 8, 2, "7365636f6e64")
        .replace("\"expected_policy_epoch\":\"0\"", "\"expected_policy_epoch\":\"1\"");
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
    let next_reviewer = reviewer.clone();
    let human = std::thread::spawn(move || {
        let mut client = next_reviewer.connect_client_waiting(8, Duration::from_secs(10)).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap();
                    assert_eq!(packet.policy_generation(), 2);
                    assert_eq!(packet.action().spec().policy_epoch, 1);
                    assert_eq!(packet.action().spec().payload, b"second");
                    offered_send.send(()).unwrap();
                    approve_receive.recv_timeout(Duration::from_secs(10)).unwrap();
                    client.respond(if reject { ReviewDecision::Reject } else { ReviewDecision::Approve }).unwrap();
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
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&next_control);
        let before = FileOversight::read_publication(&store, &profile).unwrap();
        let source = directory.join("evidence.json");
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        write_complete(&directory.join("governor-review.json"), review_document(&before, selected_scope));
        wait(&directory.join("governor-approved.json"));
        let planned = raw_exchange(&mut operator, &next(selected_scope));
        let plan = field(known_operator(&planned, "policy-promotion-next"), "value");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = governor::protocol::json(field(plan, "action"));
        let saved = store_files(&store);
        let denied = raw_exchange(&mut actor, &command);
        assert_eq!(field(field(&denied, "envelope"), "reason_code").as_str(), Some("DeniedByPolicy"));
        assert_eq!(store_files(&store), saved);
        let promoted = raw_exchange(&mut operator, &command);
        let basis = field(known_operator(&promoted, "policy-promotion"), "basis");
        assert_eq!(field(basis, "policy_generation").as_str(), Some("2"));
        assert_eq!(field(basis, "authority_epoch").as_str(), Some("1"));
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!((after.executions, after.control.ledger.charged), (1, 7));
        if !historical {
            write_complete(&directory.join("evaluate-7"), b"label the original old-policy case");
            wait(&directory.join("evaluated-7"));
        }
        assert!(!source.exists(), "labeling cannot reacquire current source");
        write_complete(&source, source_bytes);
        write_complete(&directory.join("helper-epoch"), b"1");
        if let Some(bytes) = next_capture {
            write_complete(&directory.join("original.bin"), &bytes);
            write_complete(&directory.join("current.bin"), &bytes);
        }
        let pending = raw_exchange(&mut actor, &second);
        assert_eq!(field(knowledge(&pending), "state").as_str(), Some("pending"));
        offered_receive.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(FileOversight::read_publication(&store, &profile).unwrap().executions, 1);
        // An exact old promotion retry cannot supply the fresh human key.
        let saved = store_files(&store);
        assert_eq!(raw_exchange(&mut operator, &command), promoted);
        assert_eq!(store_files(&store), saved);
        approve_send.send(()).unwrap();
        let result = finish(&mut actor, &second_poll, &second_poll);
        assert_eq!(executed(&result), !reject);
        fs::remove_file(&source).unwrap();
        if !historical {
            write_complete(&directory.join("evaluate-8"), b"label the original new-policy case");
            wait(&directory.join("evaluated-8"));
        }
    });
    let first_human = human_for(reviewer.clone(), 7);
    let result = evaluated::serve(
        config, &actor, &reviewer, publication.as_ref().map(|(profile, _)| profile),
        evaluated::Selection {
            settings: &settings, mode: evaluated::Mode::Create(&output),
            requests: &[7, 8], generation: GENERATION, operator: Some(&operator),
            governor: Some(&governor), evaluator: live_evaluator.as_ref().map(|(_, peer)| peer),
            prechecks: None, inputs: &[],
        },
        || ElapsedTick(1000),
    );
    peer.join().unwrap();
    first_human.join().unwrap();
    human.join().unwrap();
    assert!(governor_child.finish().success());
    if let Some((child, peer)) = &mut live_evaluator {
        assert!(child.finish().success());
        assert!(!peer.socket_path().exists());
    }
    result.unwrap();
    let config = configured(&root);
    let saved = inspect(&config, &governor, &settings,
        publication.as_ref().map(|(profile, _)| profile), &output);
    assert_eq!(saved.credibility.report.retained_cases, 2);
    assert_eq!(saved.credibility.report.pending_cases, if historical { 1 } else { 0 });
    assert_eq!(saved.credibility.journal.executions, if reject { 1 } else { 2 });
    assert_eq!(saved.credibility.journal.control.ledger.charged, if reject { 7 } else { 14 });
    assert_eq!(saved.credibility.journal.control.ledger.epoch, 1);
    assert_eq!(saved.accounting.windows()[&1].false_stops, 0);
    assert!(saved.receipts.is_empty());
    assert!(!config.source.path().exists());
    assert!(!governor.socket.exists());
    assert!(!operator.socket.exists());
    if historical {
        historical_labels(&root, &settings, &governor, &operator,
            publication.as_ref().map(|(profile, _)| profile), &output, &saved);
    } else if !checked && !joint && !reject {
        recover(&root, &settings, &governor, &operator, &output, &saved);
    }
}

fn recovery_document(input: &Path, output: &Path, cut: &FileDeliverySnapshot) -> String {
    format!(
        r#"{{"schema_version":"fa.guarded-submit/2","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},
        "history_anchor":{:?},"retain_history_anchor":{:?},
        "effective_policy":{{"generation":2,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":6}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}},
        "campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}}}}"#,
        cut.revision, cut.control.sequence, cut.control.ledger.epoch, input, output,
    )
}

fn historical_labels(
    root: &Directory,
    settings: &Settings,
    governor: &governor::Settings,
    operator: &operator::Settings,
    publication: Option<&crate::publication::PublicationProfile>,
    input: &Path,
    before: &FileNativeAndonSnapshot,
) {
    let mut config = configured(root);
    config.programs.clear();
    let (actor, reviewer) = profiles(root, &config);
    let output = root.0.join("history-evaluated.anchor");
    let cut = &before.credibility.journal;
    let document = recovery_document(input, &output, cut);
    let original_anchor = fs::read(input).unwrap();
    let files = store_files(&config.store);
    let (mut child, peer) = start_evaluator(root, &actor, cut.control.ledger.epoch + 1);
    let stale_inputs = [
        document.replace("\"generation\":2", "\"generation\":1"),
        document.replace("\"max_campaigns\":4", "\"max_campaigns\":3"),
    ];
    let stale: Vec<_> = stale_inputs.iter()
        .map(|bytes| SubmitProfile::decode(bytes.as_bytes()).unwrap()).collect();
    for profile in &stale {
        assert!(evaluated::serve_history(configured(root), &actor, &reviewer, publication,
            evaluated::Selection {
                settings, mode: evaluated::Mode::Existing(profile), requests: &[7, 8],
                generation: GENERATION, operator: None, governor: None, evaluator: Some(&peer),
                prechecks: None, inputs: &[],
            }, || ElapsedTick(1001)).is_err());
        assert_eq!(store_files(&config.store), files);
        assert!(!output.exists());
    }

    let selected = SubmitProfile::decode(document.as_bytes()).unwrap();
    evaluated::serve_history(config, &actor, &reviewer, publication, evaluated::Selection {
        settings, mode: evaluated::Mode::Existing(&selected), requests: &[7, 8],
        generation: GENERATION, operator: None, governor: None, evaluator: Some(&peer),
        prechecks: None, inputs: &[],
    }, || {
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(7).exists() && !reviewer.socket(8).exists());
        assert!(!operator.socket.exists() && !governor.socket.exists());
        if peer.socket_path().exists() && !root.0.join("evaluate-7").exists() {
            write_complete(&root.0.join("evaluate-7"), b"historical old-policy case");
            write_complete(&root.0.join("evaluate-8"), b"historical new-policy case");
        }
        ElapsedTick(1001)
    }).unwrap();
    assert!(child.finish().success());
    let config = configured(root);
    let after = inspect(&config, governor, settings, publication, &output);
    assert_eq!(after.credibility.report.pending_cases, 0);
    assert_eq!(after.credibility.report.benign_origins, 1);
    assert_eq!(after.credibility.report.retained_cases, 2);
    // The aggregate follows the current policy. Each receipt belongs to its
    // original case, including the independently assessed prior-policy round.
    for round in [7, 8] {
        let receipt = parse(&fs::read(root.0.join(format!("evaluated-{round}"))).unwrap());
        assert_eq!(field(&receipt, "status").as_str(), Some("applied"));
    }
    assert_eq!(after.credibility.journal.executions, cut.executions);
    assert_eq!(after.credibility.journal.control.ledger.charged, cut.control.ledger.charged);
    assert_eq!(after.accounting, before.accounting);
    assert_eq!(fs::read(input).unwrap(), original_anchor);
    assert!(!config.source.path().exists());
}

fn recover(
    root: &Directory,
    settings: &Settings,
    governor: &governor::Settings,
    operator: &operator::Settings,
    input: &Path,
    before: &FileNativeAndonSnapshot,
) {
    let mut config = configured(root);
    config.programs.clear();
    let (actor, reviewer) = profiles(root, &config);
    let output = root.0.join("recovered.anchor");
    let cut = &before.credibility.journal;
    let document = recovery_document(input, &output, cut);
    let selected = SubmitProfile::decode(document.as_bytes()).unwrap();
    let stale = SubmitProfile::decode(document.replace("\"generation\":2", "\"generation\":1").as_bytes()).unwrap();
    let files = store_files(&config.store);
    assert!(evaluated::serve(
        configured(root), &actor, &reviewer, None,
        evaluated::Selection {
            settings, mode: evaluated::Mode::Existing(&stale), requests: &[7, 8],
            generation: GENERATION, operator: Some(operator), governor: Some(governor),
            evaluator: None, prechecks: None, inputs: &[],
        }, || ElapsedTick(1001),
    ).is_err());
    assert_eq!(store_files(&config.store), files, "mismatched recovery must not Fence");
    assert!(!output.exists());
    let first = super::super::document(&config, 7, 1, "6669727374");
    let second = super::super::document(&config, 8, 2, "7365636f6e64")
        .replace("\"expected_policy_epoch\":\"0\"", "\"expected_policy_epoch\":\"1\"");
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor);
        assert!(executed(&raw_exchange(&mut socket, &first)));
        assert!(executed(&raw_exchange(&mut socket, &second)));
    });
    evaluated::serve(
        config, &actor, &reviewer, None,
        evaluated::Selection {
            settings, mode: evaluated::Mode::Existing(&selected), requests: &[7, 8],
            generation: GENERATION, operator: Some(operator), governor: Some(governor),
            evaluator: None, prechecks: None, inputs: &[],
        }, || ElapsedTick(1001),
    ).unwrap();
    peer.join().unwrap();
    let config = configured(root);
    let after = inspect(&config, governor, settings, None, &output);
    assert_eq!(after.credibility.report, before.credibility.report);
    assert_eq!(after.accounting, before.accounting);
    assert_eq!(after.receipts, before.receipts);
    assert_eq!(after.credibility.journal.executions, 2);
    assert_eq!(after.credibility.journal.control.ledger.charged, 14);
    assert!(after.credibility.journal.control.ledger.epoch > cut.control.ledger.epoch);
    assert!(!config.source.path().exists());
}

#[test]
fn governed_andon_rejects_shared_evaluator_credentials_and_listener_custody_before_bootstrap() {
    for failure in ["same_role", "same_socket", "history", "missing_operator", "missing_governor"] {
        let root = Directory::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let other_pid = std::process::id().checked_add(10_000).unwrap();
        let mut governor = governor::Settings::decode(
            governor_document(&root, &config, other_pid).as_bytes(),
        ).unwrap();
        let evaluator = evaluator_settings(&root, &actor,
            if failure == "same_role" { other_pid } else { other_pid + 1 });
        let output = root.0.join("new.anchor");
        match failure {
            "same_socket" => governor.socket = evaluator.socket_path().to_path_buf(),
            "history" => governor.socket = output.clone(),
            _ => {}
        }
        let result = evaluated::serve(
            config, &actor, &reviewer, None,
            evaluated::Selection {
                settings: &settings(false), mode: evaluated::Mode::Create(&output),
                requests: &[7, 8], generation: GENERATION,
                operator: (failure != "missing_operator").then_some(&operator),
                governor: (failure != "missing_governor").then_some(&governor),
                evaluator: Some(&evaluator), prechecks: None, inputs: &[],
            },
            || panic!("invalid independent custody must refuse before clock or storage"),
        );
        assert!(result.is_err(), "{failure}");
        assert!(!configured(&root).store.exists(), "{failure}");
        assert!(!actor.socket.exists(), "{failure}");
        assert!(!output.exists(), "{failure}");
    }
}
