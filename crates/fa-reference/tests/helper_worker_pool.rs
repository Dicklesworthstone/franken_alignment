//! Real Unix I/O and separate helper processes; no trained model or sandbox claim.
#![cfg(unix)]
#[path = "support/helper_workers.rs"]
mod support;

use support::{Fixture, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{EndpointStatus, PublicationEndpoint};
use fa_reference::action::consequence::delivery::filesystem::FilePublicationLimits;
use fa_reference::action::consequence::oversight::ObservedSession;
use fa_reference::action::consequence::oversight::helper_workers::{HelperFailure, HelperLimits, HelperPhase};
use fa_reference::action::consequence::oversight::helper_workers::io::{HelperPool, WorkerIoError};
use fa_reference::action::consequence::oversight::helper_workers::wire::{REQUEST_HEADER_BYTES, decode_request, request_frame_len};
use fa_reference::action::{ActionState, ElapsedTick, ResolvedTarget};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-hw-{}-{stamp:x}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("helper test directory cleanup failed: {error}"); }
    }
}
struct WorkerChild(Child);
impl WorkerChild {
    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() { assert!(status.success()); return; }
            assert!(Instant::now() < deadline, "helper child failed to terminate");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for WorkerChild {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            if let Err(error) = self.0.kill() { eprintln!("helper test child kill failed: {error}"); return; }
            if let Err(error) = self.0.wait() { eprintln!("helper test child wait failed: {error}"); }
        }
    }
}

fn spawn(root: &Path, member: &str, mode: &str) -> (UnixStream, WorkerChild) {
    let path = root.join(member);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.arg("--exact").arg("helper_process_entry").arg("--nocapture").arg("--test-threads=1");
    // Seed the builder explicitly, then clear it: the child tests actual removal,
    // not the vacuous absence of a credential that was never configured.
    command.env("FA_EFFECT_CREDENTIAL", "fixture-secret-must-not-reach-helper");
    command.env_clear();
    command.env("FA_HELPER_TEST_SOCKET", &path).env("FA_HELPER_TEST_MEMBER", member).env("FA_HELPER_TEST_MODE", mode);
    command.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let mut child = WorkerChild(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return (stream, child),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("helper accept failed: {error}"),
        }
        assert!(child.0.try_wait().unwrap().is_none(), "helper exited before connecting");
        assert!(Instant::now() < deadline, "helper did not connect");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Subprocess entry selected by the parent tests. Normal suite discovery does
/// not count this no-environment invocation as a worker execution experiment.
#[test]
fn helper_process_entry() {
    let Some(path) = std::env::var_os("FA_HELPER_TEST_SOCKET") else { return; };
    assert!(std::env::var_os("FA_EFFECT_CREDENTIAL").is_none());
    let member = std::env::var("FA_HELPER_TEST_MEMBER").unwrap();
    let mode = std::env::var("FA_HELPER_TEST_MODE").unwrap();
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut header = [0; REQUEST_HEADER_BYTES];
    stream.read_exact(&mut header).unwrap();
    let mut request = vec![0; request_frame_len(&header).unwrap()];
    request[..REQUEST_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut request[REQUEST_HEADER_BYTES..]).unwrap();
    let request = decode_request(&request).unwrap();
    assert_eq!(request.member(), member);
    let actual = request.actual_input();
    let peer = if member == "alpha" { "beta" } else { "alpha" };
    let private = format!("{peer}-private-question");
    assert!(!actual.submitted_bytes().windows(private.len()).any(|part| part == private.as_bytes()));
    // A declared deterministic fixture helper really examines the delivered
    // bytes. This is not a trained classifier or an empirical safety result.
    let verdict = if mode == "hold" || !actual.submitted_bytes().windows(7).any(|part| part == b"publish") {
        Verdict::Hold
    } else { Verdict::Allow };
    let salt = member.as_bytes();
    if mode == "malformed" { stream.write_all(b"X12345678").unwrap(); stream.flush().unwrap(); return; }
    stream.write_all(&request.commitment_frame(verdict, salt).unwrap()).unwrap();
    stream.flush().unwrap();
    let mut signal = [0]; stream.read_exact(&mut signal).unwrap();
    assert_eq!(&signal, b"R");
    if mode == "withhold" { return; }
    let revealed_salt = if mode == "bad-reveal" { &b"substituted"[..] } else { salt };
    stream.write_all(&request.reveal_frame(verdict, revealed_salt).unwrap()).unwrap();
    stream.flush().unwrap();
}

fn collect(root: &Path, session: ObservedSession, alpha_mode: &str) -> (HelperPool, u64) {
    let mut streams = BTreeMap::new();
    let mut children = Vec::new();
    for (member, mode) in [("alpha", alpha_mode), ("beta", "content")] {
        let (stream, child) = spawn(root, member, mode);
        streams.insert(member.to_owned(), stream); children.push(child);
    }
    let mut pool = HelperPool::new(session, streams, HelperLimits::default()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut tick = 1;
    loop {
        let report = pool.pump(ElapsedTick(tick)).unwrap();
        if report.workers.values().all(|s| matches!(s.phase, HelperPhase::Complete | HelperPhase::Failed)) { break; }
        // A missing commitment remains in the roster until the real logical
        // commit cutoff, even when its transport has already failed.
        if tick == 1 && report.workers.values().any(|s| s.phase == HelperPhase::Failed)
            && report.workers.values().all(|s| s.committed || s.phase == HelperPhase::Failed)
        { tick = 5; }
        assert!(Instant::now() < deadline, "helper pool did not make bounded progress: {:?}", report.workers);
        std::thread::sleep(Duration::from_millis(1));
    }
    for child in &mut children { child.finish(); }
    // No helper process remains alive when the caller applies the review.
    (pool, tick)
}

#[test]
fn separate_helpers_drive_file_publication_and_recovery_without_rerunning_workers() {
    let root = Temp::new();
    let target = ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 };
    let (endpoint, key) = PublicationEndpoint::create_file_publication(
        root.0.join("effect"), target, b"old".to_vec(), 200, 16,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let mut f = Fixture::with_endpoint(endpoint);
    let (mut pool, tick) = collect(&root.0, f.start(11), "content");
    assert!(pool.ready_to_finish());
    assert_eq!(pool.next_deadline(), None);
    let reviewed = pool.finish(ElapsedTick(tick)).unwrap();
    assert_eq!(reviewed.decision().consequence, Consequence::Continue);
    assert!(reviewed.missing().is_empty());
    assert_eq!(reviewed.inputs(), &f.inputs);
    drop(pool);
    f.broker.observe_time(ElapsedTick(tick)).unwrap();
    f.broker.apply_review(reviewed, Some(&f.inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(1, Some(&f.inputs), &snapshot()).unwrap();
    let message = f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).unwrap();
    f.endpoint.deliver(&message).unwrap(); // Deliberately lose the returned receipt.
    f.broker.acknowledgment_lost(1).unwrap();
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"publish");
    f.broker.inputs_unavailable(1, 1).unwrap();
    let fence = f.broker.restart_dispatcher().unwrap();
    drop(f.endpoint);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(tick)).unwrap();
    f.broker.confirm_fence(recovered.install_fence(fence).unwrap()).unwrap();
    let results = f.broker.reconcile_pending(&mut recovered).unwrap();
    assert!(matches!(results.get(&1), Some(Ok(EndpointStatus::Resolved(_)))));
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(f.broker.inspect().ledger.available, 84);
    assert_eq!(recovered.execution_count(), 1);
    assert!(recovered.deliver(&message).is_err());
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().execution_count, 1);
}

#[test]
fn actual_worker_hold_withheld_reveal_bad_reveal_and_bad_frame_cannot_publish() {
    for mode in ["hold", "withhold", "bad-reveal", "malformed"] {
        let root = Temp::new();
        let mut f = Fixture::new();
        let (mut pool, tick) = collect(&root.0, f.start(11), mode);
        assert!(pool.statuses()["beta"].revealed, "healthy peer starved in {mode}");
        let finish_at = if pool.ready_to_finish() { tick } else { 10 };
        let review = pool.finish(ElapsedTick(finish_at)).unwrap();
        assert_eq!(review.decision().consequence, Consequence::HoldEffect, "{mode}");
        if mode == "hold" { assert!(review.missing().is_empty()); }
        else { assert_eq!(review.missing(), &["alpha".to_owned()]); }
        if mode == "bad-reveal" {
            assert_eq!(pool.statuses()["alpha"].failure, Some(HelperFailure::Rejected(Error::Binding)));
        }
        f.broker.observe_time(ElapsedTick(finish_at)).unwrap();
        f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
        assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
        assert_eq!(f.endpoint.payload(), b"old");
        assert_eq!(f.endpoint.execution_count(), 0);
        assert_eq!(f.broker.inspect().ledger.available, 100);
    }
}

#[test]
fn a_silent_socket_does_not_block_another_worker_or_defeat_deadline_closure() {
    let mut f = Fixture::new();
    let (alpha, _silent_alpha) = UnixStream::pair().unwrap();
    let (beta, mut peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut pool = HelperPool::new(f.start(11), BTreeMap::from([
        ("alpha".to_owned(), alpha), ("beta".to_owned(), beta),
    ]), HelperLimits::default()).unwrap();
    pool.pump(ElapsedTick(1)).unwrap();
    let mut header = [0; REQUEST_HEADER_BYTES]; peer.read_exact(&mut header).unwrap();
    let mut bytes = vec![0; request_frame_len(&header).unwrap()];
    bytes[..REQUEST_HEADER_BYTES].copy_from_slice(&header);
    peer.read_exact(&mut bytes[REQUEST_HEADER_BYTES..]).unwrap();
    let packet = decode_request(&bytes).unwrap();
    peer.write_all(&packet.commitment_frame(Verdict::Allow, b"salt").unwrap()).unwrap();
    pool.pump(ElapsedTick(1)).unwrap();
    assert!(pool.statuses()["beta"].committed);
    assert_eq!(pool.next_deadline(), Some(ElapsedTick(5)));
    pool.pump(ElapsedTick(5)).unwrap();
    let mut signal = [0]; peer.read_exact(&mut signal).unwrap(); assert_eq!(&signal, b"R");
    peer.write_all(&packet.reveal_frame(Verdict::Allow, b"salt").unwrap()).unwrap();
    pool.pump(ElapsedTick(6)).unwrap(); // Read reveal header.
    pool.pump(ElapsedTick(6)).unwrap(); // Read its bounded salt.
    assert!(pool.statuses()["beta"].revealed);
    assert_eq!(pool.next_deadline(), Some(ElapsedTick(10)));
    assert_eq!(pool.pump(ElapsedTick(4)), Err(Error::Stale));
    let review = pool.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned()]);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
}

#[test]
fn omitted_or_extra_transport_never_changes_the_frozen_roster() {
    for extra in [false, true] {
        let mut f = Fixture::new();
        let (alpha, _a) = UnixStream::pair().unwrap();
        let (beta, _b) = UnixStream::pair().unwrap();
        let mut streams = BTreeMap::from([("alpha".to_owned(), alpha)]);
        if extra {
            let (other, _other) = UnixStream::pair().unwrap();
            streams.insert("beta".to_owned(), beta);
            streams.insert("unregistered".to_owned(), other);
        }
        assert!(matches!(HelperPool::new(f.start(11), streams, HelperLimits::default()),
            Err(WorkerIoError::Protocol(Error::Binding))));
        assert_eq!(f.broker.inspect().ledger.available, 100);
        assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
    }
}
