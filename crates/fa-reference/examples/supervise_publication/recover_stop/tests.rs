//! Native journals plus real helper/reviewer plumbing. Helper verdicts are
//! synthetic enforcement fixtures, not calibrated models or human identity.
use super::*;
use crate::{config::CLOCK_DOMAIN, workflow};
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-recover-command-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("recovery command cleanup: {error}"); }
    }
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap();
        (name.to_owned(), program)
    }).collect();
    c
}
fn empty_owner(c: &Config) -> FileOversight {
    let (mut host, _) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), c.source_policy).unwrap();
    host
}
fn output(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn reviewer(c: &Config) -> std::thread::JoinHandle<()> {
    let path = c.socket(1);
    let expected = ReviewerExpectation { reviewer: c.profile.human.reviewer_id, scope: c.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reviewer socket: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => break,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn publish(root: &Directory, expire_after_dispatch: bool) -> (Config, u64) {
    let c = configured(root);
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    std::fs::write(root.0.join("evidence.json"), evidence.encode()).unwrap();
    let document = encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: c.profile.delivery.target, payload: b"once".to_vec(), units: 4,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap();
    let late = 100001 + c.profile.delivery.retention_ticks;
    let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut expired = false;
    let human = reviewer(&c);
    let result = workflow::run(c, &document, false, || {
        if expire_after_dispatch && !expired && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
            && disk.control.ledger.charged != 0 && disk.executions == 0 { expired = true; }
        ElapsedTick(if expired { late } else { 1000 })
    }).unwrap();
    human.join().unwrap();
    assert_eq!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), !expire_after_dispatch);
    assert_eq!(expired, expire_after_dispatch);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut c = configured(root); c.programs.clear();
    (c, if expired { late + 1 } else { 1001 })
}

#[test]
fn offline_recovery_needs_no_sources_helpers_or_original_submit() {
    let root = Directory::new(); let mut c = configured(&root); drop(empty_owner(&c)); c.programs.clear();
    assert!(!root.0.join("evidence.json").exists());
    let mut bytes = Vec::new(); run(&c, 900, || ElapsedTick(1000), &mut bytes).unwrap();
    let json = output(&bytes);
    assert_eq!(json.get("status").unwrap().as_str(), Some("stopped_drained"));
    assert_eq!(json.get("recovery").unwrap().as_str(), Some("advanced"));
    assert_eq!(json.get("stop_acknowledged").unwrap().as_bool(), Some(true));
    let before = std::fs::read(c.store.join("delivery.bin")).unwrap();
    bytes.clear(); run(&c, 900, || panic!("completed retry must not acquire time"), &mut bytes).unwrap();
    assert_eq!(output(&bytes).get("recovery").unwrap().as_str(), Some("already_drained"));
    assert_eq!(std::fs::read(c.store.join("delivery.bin")).unwrap(), before);
}

#[test]
fn lost_output_is_recovered_without_another_native_stop() {
    struct Broken { flush: bool }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.flush { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) }
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for flush in [false, true] {
        let root = Directory::new(); let c = configured(&root); drop(empty_owner(&c));
        let error = run(&c, 900, || ElapsedTick(1000), &mut Broken { flush }).unwrap_err();
        assert!(error.contains("native stop acknowledged")); assert!(error.contains("drained=true"));
        let before = std::fs::read(c.store.join("delivery.bin")).unwrap();
        let mut bytes = Vec::new(); run(&c, 900, || panic!("no renewed stop"), &mut bytes).unwrap();
        assert_eq!(output(&bytes).get("recovery").unwrap().as_str(), Some("already_drained"));
        assert_eq!(std::fs::read(c.store.join("delivery.bin")).unwrap(), before);
    }
}

#[test]
fn existing_unfenced_stop_is_settled_instead_of_being_reported_complete() {
    let root = Directory::new(); let c = configured(&root); let mut host = empty_owner(&c);
    let before = host.inspect();
    let stop = StopRequest { operation: 900, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch };
    host.request_stop(host.revision(), stop).unwrap(); assert!(!host.stop_progress().unwrap().drained()); drop(host);
    let mut bytes = Vec::new(); run(&c, 900, || ElapsedTick(1000), &mut bytes).unwrap();
    let json = output(&bytes); assert_eq!(json.get("recovery").unwrap().as_str(), Some("advanced"));
    assert_eq!(json.get("endpoint_fenced").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(true));
}

#[test]
fn completed_publication_remains_charged_after_offline_stop_and_retry() {
    let root = Directory::new(); let (c, tick) = publish(&root, false);
    let mut bytes = Vec::new(); run(&c, 900, || ElapsedTick(tick), &mut bytes).unwrap();
    let json = output(&bytes); assert_eq!(json.get("executions").unwrap().as_str(), Some("1"));
    assert_eq!(json.get("charged").unwrap().as_str(), Some("4"));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(true));
    let before = FileOversight::read_publication(&c.store, &c.profile).unwrap(); assert_eq!(before.payload, b"once");
    run(&c, 900, || panic!("no repeat execution"), &mut Vec::new()).unwrap();
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
}

#[test]
fn expired_dispatched_work_returns_pending_json_and_retains_its_charge() {
    let root = Directory::new(); let (c, tick) = publish(&root, true);
    let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(before.executions, 0); assert_eq!(before.control.ledger.charged, 4);
    let operation = before.stop.unwrap().request().operation;
    let mut bytes = Vec::new(); let error = run(&c, operation, || ElapsedTick(tick), &mut bytes).unwrap_err();
    assert!(error.contains("unresolved obligations"));
    let json = output(&bytes); assert_eq!(json.get("status").unwrap().as_str(), Some("stopped_pending"));
    assert_eq!(json.get("stop_acknowledged").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(false));
    assert_eq!(json.get("charged").unwrap().as_str(), Some("4"));
    assert_eq!(json.get("unresolved").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(json.get("irrecoverable").unwrap().as_array().unwrap().len(), 1);
    let after = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(after.executions, 0); assert_eq!(after.control.ledger.charged, 4);
}

#[test]
fn invalid_missing_busy_and_foreign_recovery_never_report_success() {
    let root = Directory::new(); let c = configured(&root);
    let mut out = Vec::new();
    assert!(run(&c, 0, || panic!("invalid"), &mut out).is_err());
    assert!(run(&c, 900, || panic!("missing"), &mut out).is_err()); assert!(!c.store.exists());
    let host = empty_owner(&c); let original = std::fs::read(c.store.join("delivery.bin")).unwrap();
    assert!(run(&c, 900, || panic!("busy"), &mut out).is_err());
    assert_eq!(std::fs::read(c.store.join("delivery.bin")).unwrap(), original); drop(host);
    let mut wrong = configured(&root); wrong.profile.delivery.total += 1;
    assert!(run(&wrong, 900, || panic!("profile"), &mut out).is_err()); assert!(out.is_empty());
    run(&c, 900, || ElapsedTick(1000), &mut Vec::new()).unwrap();
    let stopped = std::fs::read(c.store.join("delivery.bin")).unwrap();
    assert!(run(&c, 901, || panic!("different operation"), &mut out).is_err()); assert!(out.is_empty());
    assert_eq!(std::fs::read(c.store.join("delivery.bin")).unwrap(), stopped);
    assert!(crate::command(vec!["recover-stop".into(), "missing-config".into()]).unwrap_err().contains("usage:"));
    assert!(crate::command(vec!["recover-stop".into(), "missing-config".into(), "900".into(), "--reviewer-profile".into(), "anything".into()]).unwrap_err().contains("usage:"));
}
