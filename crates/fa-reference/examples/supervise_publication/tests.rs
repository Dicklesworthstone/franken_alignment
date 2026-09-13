//! Explicitly synthetic helpers and reviewer choices exercise real process/socket
//! plumbing. They are not trained-model evaluations or human-authentication tests.
use super::{config::{self, Config, CLOCK_DOMAIN}, console, workflow};
use fa_reference::action::{ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command, WireError};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_client::{HelperClient, ClientPhase};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Cursor, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TEMPLATE: &[u8] = include_bytes!("../../fixtures/supervised_publication.json");
const CHILD: &str = "tests::synthetic_helper_process";
const MEMBER_ENV: &str = "FA_EXAMPLE_HELPER_MEMBER";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-run-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = std::fs::remove_dir_all(&self.0) { eprintln!("example cleanup: {e:?}"); } }
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(TEMPLATE).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(MEMBER_ENV), OsString::from(name))])).unwrap();
        (name.to_owned(), program)
    }).collect();
    c
}
fn evidence(root: &Directory, scope: fa_reference::action::Scope) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    std::fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
}
fn proposal(config: &Config, id: u64, payload: &[u8]) -> Vec<u8> {
    encode_command(&Command::Submit { request: id, proposal: ActorProposal {
        target: config.profile.delivery.target, payload: payload.to_vec(), units: payload.len() as u64,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap()
}

#[test]
fn synthetic_helper_process() {
    let Ok(member) = std::env::var(MEMBER_ENV) else { return; };
    let profile = Config::decode(TEMPLATE).unwrap().profile;
    let expected = profile.committee.members()[&member].profile_at(0);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let own = format!("review this fixture: {member}");
            assert!(input.actual_input().submitted_bytes().windows(own.len()).any(|p| p == own.as_bytes()));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn reviewer(path: PathBuf, expected: ReviewerExpectation, decision: ReviewDecision) -> std::thread::JoinHandle<ReviewPacket> {
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10), "no reviewer offer: {e:?}");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("reviewer connection: {e:?}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        let mut seen = None;
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap().clone();
                    let verb = match decision { ReviewDecision::Approve => "APPROVE", ReviewDecision::Reject => "REJECT", ReviewDecision::Revoke => "REVOKE" };
                    let nonce: String = packet.binding().session.iter().map(|b| format!("{b:02x}")).collect();
                    let choice = format!("{verb} {} {nonce}\n", packet.binding().request);
                    let mut display = Vec::new();
                    let chosen = console::decide(&packet, &mut Cursor::new(choice), &mut display).unwrap();
                    assert!(display.iter().all(|b| b.is_ascii()));
                    assert!(!display.contains(&0x1b));
                    client.respond(chosen).unwrap(); seen = Some(packet);
                }
                ReviewClientProgress::Complete => return seen.unwrap(),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn expected(c: &Config) -> ReviewerExpectation {
    ReviewerExpectation { reviewer: c.profile.human.reviewer_id, scope: c.profile.delivery.scope, clock_domain: CLOCK_DOMAIN }
}

#[test]
fn original_request_runs_process_helpers_independent_review_and_one_publication() {
    let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
    let payload = b"publication\n\x1b[31muntrusted text\xff";
    let document = proposal(&config, 1, payload);
    let human = reviewer(config.socket(1), expected(&config), ReviewDecision::Approve);
    let result = workflow::run(config, &document, false, || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(packet.action().spec().payload, payload);
    assert_eq!(packet.views().len(), 2);
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, payload);
    assert_eq!(disk.control.ledger.charged, payload.len() as u64); assert_eq!(disk.control.ledger.reserved, 0);
}

#[test]
fn explicit_rejection_preserves_original_nonexecution_instead_of_default_consent() {
    let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
    let document = proposal(&config, 1, b"declined");
    let human = reviewer(config.socket(1), expected(&config), ReviewDecision::Reject);
    let result = workflow::run(config, &document, false, || ElapsedTick(1000)).unwrap();
    human.join().unwrap();
    assert!(result.failure.is_none()); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
    assert_eq!(disk.control.ledger.available, config.profile.delivery.total);
}

#[test]
fn exact_resume_needs_no_source_or_helpers_and_changed_request_cannot_republish() {
    let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
    let document = proposal(&config, 1, b"once");
    let human = reviewer(config.socket(1), expected(&config), ReviewDecision::Approve);
    let result = workflow::run(config, &document, false, || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(result.failure.is_none());
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root); config.programs.clear();
    let resumed = workflow::run(config, &document, true, || ElapsedTick(1001)).unwrap();
    assert!(resumed.failure.is_none()); assert_eq!(resumed.cleanup_pending, 0);
    assert!(matches!(resumed.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let config = configured(&root); let changed = proposal(&config, 1, b"different");
    let refused = workflow::run(config, &changed, true, || ElapsedTick(1002)).unwrap();
    assert_eq!(refused.response.result, Err(WireError::IdempotencyConflict));
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, b"once");
}

#[test]
fn malformed_configuration_and_request_are_rejected_before_store_creation() {
    let template = std::str::from_utf8(TEMPLATE).unwrap();
    for invalid in [template.replacen("\"version\": 1", "\"version\": 2", 1),
        template.replacen("\"clock\": \"unix_milliseconds\"", "\"clock\": \"process_instant\"", 1),
        template.replacen("\"op\": \"all\"", "\"op\": \"allow_anything\"", 1),
        template.replacen("\"grade\": \"audit_only\"", "\"grade\": \"exact_restart\"", 1),
        template.replacen("\"version\": 1", "\"version\": 1, \"surprise\": 7", 1)] {
        assert!(Config::decode(invalid.as_bytes()).is_err());
    }
    let root = Directory::new(); let config = configured(&root); let store = config.store.clone();
    assert!(workflow::run(config, b"{}", false, || ElapsedTick(1000)).is_err());
    assert!(!store.exists());
    assert!(config::hex("ff00").is_ok()); assert!(config::hex("FF00").is_err()); assert!(config::hex("f").is_err());
}

#[test]
fn display_failure_and_blank_or_wrong_offer_decision_never_default_to_approve() {
    let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
    let document = proposal(&config, 1, b"terminal safety");
    let human = reviewer(config.socket(1), expected(&config), ReviewDecision::Reject);
    workflow::run(config, &document, false, || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap();
    for input in [b"\n".as_slice(), b"yes\n", b"APPROVE 999\n", b""] {
        assert!(console::decide(&packet, &mut Cursor::new(input), &mut Vec::new()).is_err());
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let mut input = Cursor::new(b"APPROVE should not be read\n");
    assert!(console::decide(&packet, &mut input, &mut Broken).is_err()); assert_eq!(input.position(), 0);
    let mut out = Vec::new(); console::escaped(&mut out, &[0, 0x1b, b'\n', 0xff, b'\\']).unwrap();
    assert_eq!(out, b"\\x00\\x1b\\n\\xff\\\\");
}
