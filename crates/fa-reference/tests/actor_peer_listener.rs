#![cfg(target_os = "linux")]

#[path = "support/actor_gateway.rs"]
#[allow(dead_code)]
mod fixture;

use fa_reference::action::consequence::oversight::actor::{ActorPort, IntakeLimits};
use fa_reference::action::consequence::oversight::actor_peer::{
    AcceptEvent, PeerCredentials, PeerPolicy, PeerRefusal, PeerSession, UnixPeerListener,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, encode_command};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, FilePublicationLimits, PublicationEndpoint};
use fa_reference::action::ElapsedTick;
use fa_reference::Error;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-peer-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf { self.0.join("actor.sock") }
}
impl Drop for Directory { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

struct ChildOwner(Child);
impl ChildOwner {
    fn finish(&mut self) { assert!(self.0.wait().unwrap().success()); }
}
impl Drop for ChildOwner {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}
fn own_policy() -> PeerPolicy {
    let c = credentials(); PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap()
}
fn listener(port: ActorPort, socket: UnixListener, policy: PeerPolicy, connections: u64) -> UnixPeerListener {
    UnixPeerListener::new(socket, PeerSession::new(policy, ActorWire::new(port), ChannelLimits::default(), connections).unwrap()).unwrap()
}
fn accept(listener: &mut UnixPeerListener) -> AcceptEvent {
    for _ in 0..5_000 {
        match listener.accept_once().unwrap() {
            AcceptEvent::Idle => std::thread::sleep(Duration::from_millis(1)),
            event => return event,
        }
    }
    panic!("no connection before the bounded fixture deadline");
}
fn connect(listener: &mut UnixPeerListener, path: &Path) -> UnixStream {
    let stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    own_policy().verify(&stream).unwrap();
    assert!(matches!(accept(listener), AcceptEvent::Admitted(_)));
    stream
}
fn submit(request: u64) -> Command { Command::Submit { request, proposal: fixture::proposal() } }
fn exchange(listener: &mut UnixPeerListener, stream: &mut UnixStream, command: &Command) -> String {
    let mut bytes = encode_command(command).unwrap(); bytes.push(b'\n');
    stream.write_all(&bytes).unwrap();
    assert_eq!(listener.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    let mut reply = String::new(); BufReader::new(stream).read_line(&mut reply).unwrap(); reply
}
fn child(path: &Path, rejected: bool) -> ChildOwner {
    let c = credentials();
    ChildOwner(ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", "peer_process_entry", "--nocapture"])
        .env("FA_PEER_CHILD_PATH", path)
        .env("FA_PEER_CHILD_REJECTED", if rejected { "yes" } else { "no" })
        .env("FA_PEER_PARENT_UID", c.uid().to_string())
        .env("FA_PEER_PARENT_GID", c.gid().to_string())
        .env("FA_PEER_PARENT_PID", c.pid().to_string())
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap())
}

/// Subprocess plumbing, not an additional independent scenario or model.
#[test]
fn peer_process_entry() {
    let Some(path) = std::env::var_os("FA_PEER_CHILD_PATH") else { return; };
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let number = |name| std::env::var(name).unwrap().parse::<u32>().unwrap();
    PeerPolicy::new(number("FA_PEER_PARENT_UID"), number("FA_PEER_PARENT_GID"), Some(number("FA_PEER_PARENT_PID")))
        .unwrap().verify(&stream).unwrap();
    let rejected = std::env::var("FA_PEER_CHILD_REJECTED").unwrap() == "yes";
    let mut bytes = encode_command(&submit(77)).unwrap(); bytes.push(b'\n');
    if let Err(error) = stream.write_all(&bytes) {
        assert!(rejected && matches!(error.kind(), io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset));
        return;
    }
    if rejected {
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => {}, Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {},
            other => panic!("refused peer received data or remained open: {other:?}"),
        }
    } else {
        let mut reply = String::new(); BufReader::new(stream).read_line(&mut reply).unwrap();
        assert!(reply.contains("\"state\":\"pending\""));
    }
}

#[test]
fn accepted_child_identity_comes_from_the_kernel_not_the_json_request() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let mut child = child(&directory.socket(), false);
    let c = credentials();
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port, bound, PeerPolicy::new(c.uid(), c.gid(), Some(child.0.id())).unwrap(), 2);
    let AcceptEvent::Admitted(admitted) = accept(&mut listener) else { panic!("matching child refused"); };
    assert_eq!(admitted.credentials.pid(), child.0.id());
    assert_ne!(admitted.credentials.pid(), std::process::id());
    assert_eq!(admitted.credentials.uid(), c.uid());
    let mut received = false;
    for _ in 0..5_000 {
        if listener.drive(DriveBudget::default()).unwrap().progress.frames == 1 { received = true; break; }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(received); child.finish();
    let intake = supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    assert_eq!(intake.request, 77); assert!(intake.result.unwrap().is_some());
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 0);
}

#[test]
fn same_account_wrong_process_is_rejected_and_does_not_consume_the_connection_allowance() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let mut child = child(&directory.socket(), true);
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port, bound, own_policy(), 1);
    assert_eq!(accept(&mut listener), AcceptEvent::Rejected(PeerRefusal::CredentialsRejected));
    child.finish();
    assert_eq!(listener.status().session.connections_admitted, 0);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1));
    assert_eq!(supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap().request, 1);
}

#[test]
fn authenticated_request_still_requires_congress_and_original_one_use_authorization() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let (port, mut supervisor, mut endpoint) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port, bound, own_policy(), 2);
    assert_eq!(listener.accept_once().unwrap(), AcceptEvent::Idle);
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    assert!(supervisor.authorize_request(1, None, &fixture::snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    let receipt = supervisor.deliver_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    assert!(matches!(receipt.outcome(), EndpointOutcome::Executed { .. }));
    assert_eq!(endpoint.payload(), fixture::proposal().payload);
    let reply = exchange(&mut listener, &mut stream, &Command::Poll { request: 1 });
    assert!(reply.contains("\"value\":\"executed\""));
    for private in ["secret-helper", "secret-detector-question", "secret-cohort", "secret-model-profile"] { assert!(!reply.contains(private)); }
    assert!(exchange(&mut listener, &mut stream, &submit(1)).contains("\"value\":\"executed\""));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    assert!(supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn authenticated_peer_does_not_supply_the_independent_human_key() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let (port, mut supervisor, mut endpoint) = fixture::fixture(IntakeLimits::default());
    let reviewer = supervisor.broker_mut().enable_human_review(HumanReviewPolicy {
        reviewer_id: 8, max_validity_ticks: 20, max_requests: 4,
    }).unwrap();
    let mut listener = listener(port, bound, own_policy(), 1);
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1)); supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    assert!(matches!(supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()), Err(Error::Incomplete)));
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 16); assert_eq!(endpoint.execution_count(), 0);
    let attempt = supervisor.attempt(1).unwrap();
    let request = supervisor.broker_mut().request_human_approval(1, attempt, Some(&inputs), ElapsedTick(10)).unwrap();
    let key = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    supervisor.deliver_request(1, DispatchKeys::two(&permit, &key), Some(&inputs), &fixture::snapshot(), &mut endpoint).unwrap();
    assert!(exchange(&mut listener, &mut stream, &Command::Poll { request: 1 }).contains("\"value\":\"executed\""));
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn revoking_authenticated_ingress_preserves_an_already_disclosed_unknown_effect() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let (port, mut supervisor, mut endpoint) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port.clone(), bound, own_policy(), 2);
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1)); supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let ticket = port.submit(1, &fixture::proposal()).unwrap();
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    let message = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap(); supervisor.acknowledgment_lost(1).unwrap();
    assert!(listener.revoke()); assert!(!listener.revoke());
    assert!(listener.listener_fd().is_none() && listener.socket_fd().is_none());
    assert_eq!(listener.accept_once().unwrap(), AcceptEvent::Stopped);
    port.cancel(&ticket).unwrap(); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    supervisor.accept_receipt(receipt).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.available, 84);
    assert_eq!(endpoint.execution_count(), 1);
    let session = listener.into_session(); assert!(session.status().revoked);
}

#[test]
fn reconnect_and_actor_cancel_do_not_refund_until_the_endpoint_seals_nonexecution() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let (port, mut supervisor, mut endpoint) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port, bound, own_policy(), 2);
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1)); supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    let message = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    supervisor.acknowledgment_lost(1).unwrap();
    listener.disconnect(); drop(stream);
    let mut stream = connect(&mut listener, &directory.socket());
    assert!(exchange(&mut listener, &mut stream, &Command::Poll { request: 1 }).contains("outcome_unknown"));
    exchange(&mut listener, &mut stream, &Command::Cancel { request: 1 }); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    let query = supervisor.broker().status_query(supervisor.attempt(1).unwrap()).unwrap();
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    let seal = endpoint.seal_unexecuted(&query).unwrap(); supervisor.accept_receipt(seal.clone()).unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap(), seal);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    assert!(exchange(&mut listener, &mut stream, &submit(1)).contains("confirmed_not_executed"));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
}

#[test]
fn busy_listener_does_not_evict_a_peer_and_moving_it_retains_connection_quotas() {
    let directory = Directory::new(); let bound = UnixListener::bind(directory.socket()).unwrap();
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut listener = listener(port, bound, own_policy(), 1);
    let mut original = connect(&mut listener, &directory.socket());
    let before = listener.status();
    let _contender = UnixStream::connect(directory.socket()).unwrap();
    assert_eq!(accept(&mut listener), AcceptEvent::Rejected(PeerRefusal::Busy));
    assert_eq!(listener.status(), before);
    exchange(&mut listener, &mut original, &submit(1));
    listener.disconnect(); drop(original);
    let session = listener.into_session();
    let path = directory.0.join("replacement.sock");
    let mut listener = UnixPeerListener::new(UnixListener::bind(&path).unwrap(), session).unwrap();
    let _new_peer = UnixStream::connect(path).unwrap();
    assert_eq!(accept(&mut listener), AcceptEvent::Rejected(PeerRefusal::Capacity));
    assert_eq!(supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap().request, 1);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
}

#[test]
fn authenticated_reconnect_composes_with_real_file_publication_recovery() {
    let directory = Directory::new(); let path = directory.0.join("publication");
    let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(
        &path, fixture::proposal().target, b"old".to_vec(), 200, 128,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (port, mut supervisor) = fixture::attach(&mut endpoint, IntakeLimits::default());
    let mut listener = listener(port, UnixListener::bind(directory.socket()).unwrap(), own_policy(), 2);
    let mut stream = connect(&mut listener, &directory.socket());
    exchange(&mut listener, &mut stream, &submit(1)); supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    let message = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &fixture::snapshot()).unwrap();
    endpoint.deliver(&message).unwrap(); supervisor.acknowledgment_lost(1).unwrap();
    assert_eq!(PublicationEndpoint::read_file_publication(&path).unwrap().payload, fixture::proposal().payload);
    listener.disconnect(); drop(stream); drop(endpoint); drop(inputs);
    let mut endpoint = recovery.reopen().unwrap(); endpoint.observe_time(ElapsedTick(2)).unwrap();
    supervisor.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    let fence = supervisor.broker_mut().restart_dispatcher().unwrap();
    let ack = endpoint.install_fence(fence).unwrap(); supervisor.broker_mut().confirm_fence(ack).unwrap();
    let mut stream = connect(&mut listener, &directory.socket());
    assert!(exchange(&mut listener, &mut stream, &Command::Poll { request: 1 }).contains("outcome_unknown"));
    let results = supervisor.reconcile_pending(&mut endpoint).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results.values().all(|result| matches!(result, Ok(EndpointStatus::Resolved(_)))));
    assert!(exchange(&mut listener, &mut stream, &submit(1)).contains("\"value\":\"executed\""));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(supervisor.broker().inspect().ledger.available, 84);
}
