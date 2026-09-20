//! Real sockets and executable children; synthetic verdicts test orchestration,
//! not a human identity, detector quality, or bounded kernel/filesystem latency.
use super::*;
use super::super::{clock, RunResult};
use crate::config::{Config, CLOCK_DOMAIN};
use crate::peers::PeerProfile;
use crate::workflow;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClient, StopClientPhase, StopClientProgress, StopControlReceipt,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-stop-live-{}-{}-{}", std::process::id(), clock().0,
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new().mode(0o750).create(path.join("peers")).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = fs::remove_dir_all(&self.0) { eprintln!("stop test cleanup: {e}"); } }
}
fn credentials() -> PeerCredentials {
    let (socket, _other) = UnixStream::pair().unwrap(); PeerCredentials::observe(&socket).unwrap()
}
fn configured(root: &Directory, stalled: bool) -> Config {
    let mut c = Config::decode(include_bytes!("../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let child = if stalled { "workflow::control::tests::silent_helper_process" }
            else { "tests::synthetic_helper_process" };
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), child.into(), "--nocapture".into()], BTreeMap::from([
                (OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name)),
                (OsString::from("FA_STOP_HELPER_MARKER"), root.0.join("helper-started").into_os_string()),
            ])).unwrap();
        (name.to_owned(), program)
    }).collect();
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
    c
}
fn profile(root: &Directory, c: &Config, wrong_pid: bool) -> PeerProfile {
    let id = credentials(); let s = c.profile.delivery.scope;
    let json = format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority, c.profile.human.reviewer_id, root.0.display(),
        id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid() + u32::from(wrong_pid));
    PeerProfile::decode(json.as_bytes()).unwrap()
}
fn document(c: &Config) -> Vec<u8> {
    encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: c.profile.delivery.target, payload: b"controlled".to_vec(), units: 10,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap()
}
fn wait(path: &Path) {
    let start = Instant::now();
    while !path.exists() { assert!(start.elapsed() < Duration::from_secs(10), "missing {path:?}"); pause(1); }
}
fn client(profile: &PeerProfile) -> StopClient<UnixStream> {
    let path = socket_path(profile, 1); wait(&path);
    let id = credentials();
    let socket = VerifiedReviewerSocket::verify(UnixStream::connect(path).unwrap(),
        PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()).unwrap();
    socket.into_stop_client(profile.expected, 1).unwrap()
}
fn stopping(profile: PeerProfile, ready: PathBuf, lose_reply: bool) -> std::thread::JoinHandle<Option<StopControlReceipt>> {
    std::thread::spawn(move || {
        wait(&ready); let mut client = client(&profile); let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt(),
                _ => {}
            }
            if lose_reply && client.phase() == StopClientPhase::AwaitingReceipt { return None; }
            pause(1);
        }
    })
}
fn approving(profile: PeerProfile) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        wait(&profile.socket(1)); let mut client = profile.connect_client(1).unwrap(); let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}
fn executed(result: &RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn deadline() -> Deadline {
    Deadline { logical: ElapsedTick(100000), started: Instant::now(), wall: Duration::from_secs(15) }
}

#[test]
fn silent_helper_process() {
    let Some(marker) = std::env::var_os("FA_STOP_HELPER_MARKER") else { return; };
    fs::write(marker, b"running").unwrap();
    // The real native driver must kill/reap this child; it will not produce a verdict.
    std::thread::sleep(Duration::from_secs(15));
}

#[test]
fn live_operator_stop_interrupts_helpers_and_the_wait_for_any_human_reviewer() {
    for stalled in [true, false] {
        let root = Directory::new(); let c = configured(&root, stalled); let profile = profile(&root, &c, false);
        let doc = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let ready = if stalled { root.0.join("helper-started") } else { profile.socket(1) };
        let controller = stopping(profile.clone(), ready, false);
        let result = workflow::run_with_peers(c, &doc, false, Some(&profile), || ElapsedTick(1000)).unwrap();
        let receipt = controller.join().unwrap().unwrap();
        assert!(receipt.acknowledged()); assert!(receipt.drained());
        assert_eq!(receipt.binding.expected.clock_domain, CLOCK_DOMAIN);
        assert!(result.failure.is_none(), "{:?}", result.failure); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert!(disk.stop.is_some()); assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
        assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
        assert!(!socket_path(&profile, 1).exists()); assert!(!profile.socket(1).exists());
    }
}

#[test]
fn idle_stop_listener_preserves_successful_review_and_source_free_exact_retries() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
    let doc = document(&c); let human = approving(profile.clone());
    let result = workflow::run_with_peers(c, &doc, false, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    assert_eq!(result.cleanup_pending, 0); assert!(!socket_path(&profile, 1).exists());
    // A foreign pre-existing socket makes unintended binding observable. An exact
    // retained request must not even try to own it or read missing evidence.
    let occupied = socket_path(&profile, 1); let _listener = UnixListener::bind(&occupied).unwrap();
    let mut c = configured(&root, false); c.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let retry = workflow::continuation::submit_existing(c, &doc, Some(&profile), None, || ElapsedTick(200000)).unwrap();
    assert!(executed(&retry)); assert!(retry.failure.is_none()); assert!(occupied.exists());
}

#[test]
fn wrong_process_identity_gets_no_protocol_bytes_and_no_native_transition() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, true);
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let (_port, mut driver) = host.into_supervised_driver(); let mut service = Control::new(&c, 1, Some(&profile)).unwrap();
    let before = driver.supervisor().host().unwrap().revision();
    let mut stream = UnixStream::connect(socket_path(&profile, 1)).unwrap();
    assert!(!service.checkpoint(&mut driver, &reviewer, &deadline(), &mut || panic!("wrong peer observed time")).unwrap());
    stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(stream.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(driver.supervisor().host().unwrap().revision(), before);
    assert!(driver.supervisor().host().unwrap().inspect().stop.is_none());
}

#[test]
fn no_connection_never_reads_the_journal_or_clock_and_conflicting_path_is_not_removed() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
    let (host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    let (_port, mut driver) = host.into_supervised_driver();
    let before = driver.supervisor().host().unwrap().revision();
    let mut service = Control::new(&c, 1, Some(&profile)).unwrap();
    assert!(!service.checkpoint(&mut driver, &reviewer, &deadline(), &mut || panic!("idle clock read")).unwrap());
    assert_eq!(driver.supervisor().host().unwrap().revision(), before); drop(service);
    let path = socket_path(&profile, 1); fs::write(&path, b"retain unrelated file").unwrap();
    assert!(Control::new(&c, 1, Some(&profile)).is_err());
    assert_eq!(fs::read(path).unwrap(), b"retain unrelated file");
}

#[test]
fn lost_stop_reply_does_not_erase_or_reapply_the_native_stop() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let (_port, mut driver) = host.into_supervised_driver(); let mut service = Control::new(&c, 1, Some(&profile)).unwrap();
    let controller = stopping(profile.clone(), socket_path(&profile, 1), true);
    let limit = deadline();
    while !service.checkpoint(&mut driver, &reviewer, &limit, &mut || ElapsedTick(1000)).unwrap() {
        assert!(limit.started.elapsed() < Duration::from_secs(10)); pause(1);
    }
    assert!(controller.join().unwrap().is_none());
    let before = driver.supervisor().host().unwrap().inspect(); assert!(before.stop.is_some());
    assert!(service.checkpoint(&mut driver, &reviewer, &limit, &mut || panic!("stop reapplied")).unwrap());
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
}

#[test]
fn authenticated_wrong_session_is_not_an_operator_stop_request() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let (_port, mut driver) = host.into_supervised_driver(); let mut service = Control::new(&c, 1, Some(&profile)).unwrap();
    let path = socket_path(&profile, 1);
    let peer = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(path).unwrap(); stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut request = [0; 104]; stream.read_exact(&mut request).unwrap();
        assert_eq!(&request[..8], b"FASTOFF1"); request[..8].copy_from_slice(b"FASTREQ1"); request[103] ^= 1;
        stream.write_all(&request).unwrap();
    });
    let before = driver.supervisor().host().unwrap().revision(); let limit = deadline();
    loop {
        assert!(limit.started.elapsed() < Duration::from_secs(10));
        match service.checkpoint(&mut driver, &reviewer, &limit, &mut || ElapsedTick(1000)) {
            Ok(false) => pause(1),
            Err(error) => { assert!(error.contains("Binding"), "{error}"); break; }
            Ok(true) => panic!("wrong-session request stopped the native owner"),
        }
    }
    peer.join().unwrap(); assert_eq!(driver.supervisor().host().unwrap().revision(), before);
    assert!(driver.supervisor().host().unwrap().inspect().stop.is_none());
}

#[test]
fn silent_authenticated_controller_uses_the_original_failure_stop_not_a_weaker_listener() {
    let root = Directory::new(); let c = configured(&root, false); let mut profile = profile(&root, &c, false);
    profile.runtime_ms = 20;
    let doc = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
    let observer = profile.clone();
    let (release, held) = std::sync::mpsc::channel();
    let silent = std::thread::spawn(move || {
        wait(&observer.socket(1));
        let _stream = UnixStream::connect(socket_path(&observer, 1)).unwrap();
        held.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let result = workflow::run_with_peers(c, &doc, false, Some(&profile), || ElapsedTick(1000)).unwrap();
    release.send(()).unwrap(); silent.join().unwrap();
    assert!(result.failure.as_ref().unwrap().contains("stop session timed out"), "{:?}", result.failure);
    assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert!(disk.stop.is_some()); assert_eq!(disk.executions, 0);
    assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
}

#[path = "late_control_tests.rs"]
mod late_control_tests;
