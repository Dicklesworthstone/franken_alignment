//! Real Linux peer credentials over the original durable two-key protocol.
#![cfg(target_os = "linux")]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    ReviewerConnection, ReviewerError, ReviewerPhase, ReviewerProgress, ReviewApplication,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewerClient, ReviewerExpectation, ReviewClientPhase,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerCredentials, PeerPolicy, ReviewerPeerAdmission, ReviewerPeerError, VerifiedReviewerSocket, MAX_REVIEWER_CANDIDATES,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::Error;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixStream, UnixListener};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn expected() -> ReviewerExpectation {
    ReviewerExpectation { reviewer: profile().human.reviewer_id, scope: profile().delivery.scope,
        clock_domain: profile().delivery.clock_domain }
}
fn exact(credentials: PeerCredentials) -> PeerPolicy {
    PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap()
}
fn verified_pair() -> (VerifiedReviewerSocket, VerifiedReviewerSocket) {
    let (server, client) = UnixStream::pair().unwrap();
    let policy = exact(PeerCredentials::observe(&server).unwrap());
    (VerifiedReviewerSocket::verify(server, policy).unwrap(), VerifiedReviewerSocket::verify(client, policy).unwrap())
}
fn offer(server: &mut ReviewerConnection<UnixStream>, host: &mut FileOversight,
    reviewer: &FileHumanReviewer, client: &mut ReviewerClient<UnixStream>)
{
    for _ in 0..128 {
        assert!(!matches!(server.step(host, reviewer, || ElapsedTick(1)).unwrap(), ReviewerProgress::Applied(_)));
        client.step().unwrap();
        if client.phase() == ReviewClientPhase::NeedsDecision { return; }
    }
    panic!("original offer did not arrive within the test pump bound");
}
fn apply(server: &mut ReviewerConnection<UnixStream>, host: &mut FileOversight,
    reviewer: &FileHumanReviewer, client: &mut ReviewerClient<UnixStream>) -> ReviewApplication
{
    for _ in 0..128 {
        client.step().unwrap();
        if let ReviewerProgress::Applied(application) = server.step(host, reviewer, || ElapsedTick(1)).unwrap() {
            return application;
        }
    }
    panic!("original decision did not commit within the test pump bound");
}
fn no_received_bytes(stream: &mut UnixStream) {
    stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    match stream.read(&mut [0; 1]) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        other => panic!("rejected socket received bytes or remained open: {other:?}"),
    }
}

#[test]
fn bidirectionally_verified_peers_still_need_explicit_decision_and_both_original_keys() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let (action, inputs) = reviewed(&mut host, 1, b"published");
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let before = host.inspect(); let (server, client) = verified_pair();
    assert_eq!(server.peer().pid(), std::process::id());
    let mut server = server.into_connection(&host, &reviewer, request, [17; 32]).unwrap();
    let mut client = client.into_client(expected()).unwrap();
    offer(&mut server, &mut host, &reviewer, &mut client);
    assert_eq!(client.packet().unwrap().action(), &action);
    for _ in 0..4 { client.step().unwrap(); server.step(&mut host, &reviewer, || panic!("no decision yet")).unwrap(); }
    assert_eq!(host.inspect(), before);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    assert!(host.publish(host.revision(), 1).is_err());
    client.respond(ReviewDecision::Approve).unwrap();
    let application = apply(&mut server, &mut host, &reviewer, &mut client);
    let key = application.approval.unwrap(); let revision = host.revision();
    for _ in 0..128 {
        assert!(!matches!(server.step(&mut host, &reviewer, || panic!("receipt delivery is not approval")).unwrap(), ReviewerProgress::Applied(_)));
        client.step().unwrap();
        if client.phase() == ReviewClientPhase::Complete { break; }
    }
    assert_eq!(client.receipt(), Some(application.receipt));
    assert_eq!(host.revision(), revision);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    host.dispatch(host.revision(), &automatic, &key, &action, &inputs, snapshot()).unwrap();
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn uid_gid_and_pid_mismatch_reject_before_even_a_valid_prequeued_decision_is_read() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (_, inputs) = reviewed(&mut host, 1, b"unchanged");
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let frame = ReviewPacket::capture(&request, expected().clock_domain, host.revision(), HumanDisposition::Pending, [7; 32])
        .unwrap().decision_frame(ReviewDecision::Approve);
    let before = host.inspect();
    for field in 0..3 {
        let (server, mut client) = UnixStream::pair().unwrap();
        let peer = PeerCredentials::observe(&server).unwrap();
        let different_pid = if peer.pid() == 1 { 2 } else { 1 };
        let policy = match field {
            0 => PeerPolicy::new(peer.uid() ^ 1, peer.gid(), Some(peer.pid())).unwrap(),
            1 => PeerPolicy::new(peer.uid(), peer.gid() ^ 1, Some(peer.pid())).unwrap(),
            _ => PeerPolicy::new(peer.uid(), peer.gid(), Some(different_pid)).unwrap(),
        };
        client.write_all(&frame).unwrap();
        assert_eq!(VerifiedReviewerSocket::verify(server, policy).unwrap_err(),
            ReviewerPeerError::Credentials(io::ErrorKind::PermissionDenied));
        no_received_bytes(&mut client);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    }
}

#[test]
fn invalid_server_credentials_do_not_turn_a_framed_offer_into_a_decision() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let (_, inputs) = reviewed(&mut host, 1, b"not approved");
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&client).unwrap();
    let mut server = VerifiedReviewerSocket::verify(server, exact(credentials)).unwrap()
        .into_connection(&host, &reviewer, request, [5; 32]).unwrap();
    for _ in 0..128 {
        server.step(&mut host, &reviewer, || panic!("no incoming decision")).unwrap();
        if server.phase() == ReviewerPhase::AwaitingDecision { break; }
    }
    let wrong = PeerPolicy::new(credentials.uid() ^ 1, credentials.gid(), None).unwrap();
    assert_eq!(VerifiedReviewerSocket::verify(client, wrong).unwrap_err(),
        ReviewerPeerError::Credentials(io::ErrorKind::PermissionDenied));
    assert!(server.step(&mut host, &reviewer, || panic!("rejected server must receive no decision")).is_err());
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn credential_success_does_not_bypass_logical_reviewer_binding_or_changed_evidence() {
    for stale_input in [false, true] {
        let root = Directory::new(); let (mut host, reviewer) = create(&root);
        let (action, original) = reviewed(&mut host, 1, b"held");
        let request = host.request_human_approval(host.revision(), 1001, 1, &original, ElapsedTick(31)).unwrap();
        let (server, client) = verified_pair();
        let mut server = server.into_connection(&host, &reviewer, request, [6; 32]).unwrap();
        let mut audience = expected(); if !stale_input { audience.reviewer += 1; }
        let mut client = client.into_client(audience).unwrap();
        if stale_input {
            offer(&mut server, &mut host, &reviewer, &mut client);
            let changed = inputs(&action, b"changed source");
            host.record_inputs(host.revision(), 1, host.input_revision(1).unwrap(), changed).unwrap();
            client.respond(ReviewDecision::Approve).unwrap(); client.step().unwrap();
            assert!(server.step(&mut host, &reviewer, || ElapsedTick(1)).is_err());
        } else {
            let mut failed = false;
            for _ in 0..128 {
                server.step(&mut host, &reviewer, || panic!("no valid logical audience")).unwrap();
                match client.step() {
                    Err(ReviewerError::Protocol(Error::Binding)) => { failed = true; break; }
                    Err(error) => panic!("unexpected refusal: {error:?}"),
                    Ok(_) => {}
                }
            }
            assert!(failed); assert!(client.packet().is_none());
        }
        assert_ne!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn one_success_and_finite_candidate_budget_never_widen_the_peer_rule() {
    let (server, client) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&server).unwrap();
    assert!(matches!(ReviewerPeerAdmission::new(exact(peer), 0), Err(Error::InvalidInput)));
    assert!(matches!(ReviewerPeerAdmission::new(exact(peer), MAX_REVIEWER_CANDIDATES + 1), Err(Error::Limit)));
    let mut gate = ReviewerPeerAdmission::new(exact(peer), 2).unwrap();
    let accepted = gate.admit(server).unwrap();
    let (other, mut rejected) = UnixStream::pair().unwrap();
    assert_eq!(gate.admit(other).unwrap_err(), ReviewerPeerError::AlreadyAdmitted);
    assert_eq!(gate.status().attempted, 1); assert_eq!(gate.status().rejected, 0);
    no_received_bytes(&mut rejected);
    drop(accepted); drop(client);
    let wrong = PeerPolicy::new(peer.uid() ^ 1, peer.gid(), None).unwrap();
    let mut gate = ReviewerPeerAdmission::new(wrong, 2).unwrap();
    for _ in 0..2 {
        let (server, mut client) = UnixStream::pair().unwrap();
        assert!(matches!(gate.admit(server), Err(ReviewerPeerError::Credentials(_))));
        no_received_bytes(&mut client);
    }
    assert!(gate.exhausted());
    let (server, mut client) = UnixStream::pair().unwrap();
    assert_eq!(gate.admit(server).unwrap_err(), ReviewerPeerError::Exhausted);
    no_received_bytes(&mut client);
    assert_eq!(gate.status().attempted, 2); assert_eq!(gate.status().rejected, 2);
    assert_eq!(gate.policy(), wrong);
}

const CHILD: &str = "reviewer_peer_child";
const CHILD_SOCKET: &str = "FA_REVIEWER_PEER_SOCKET";
const CHILD_PARENT: &str = "FA_REVIEWER_PEER_PARENT";
const CHILD_UID: &str = "FA_REVIEWER_PEER_UID";
const CHILD_GID: &str = "FA_REVIEWER_PEER_GID";
#[test]
fn reviewer_peer_child() {
    let Some(path) = std::env::var_os(CHILD_SOCKET) else { return; };
    let number = |key| std::env::var(key).unwrap().parse::<u32>().unwrap();
    let policy = PeerPolicy::new(number(CHILD_UID), number(CHILD_GID), Some(number(CHILD_PARENT))).unwrap();
    let stream = UnixStream::connect(path).unwrap();
    let mut client = VerifiedReviewerSocket::verify(stream, policy).unwrap().into_client(expected()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        client.step().unwrap();
        if client.phase() == ReviewClientPhase::NeedsDecision {
            assert_eq!(client.packet().unwrap().binding().request, 1001);
            client.respond(ReviewDecision::Approve).unwrap();
        }
        if client.phase() == ReviewClientPhase::Complete { return; }
        assert!(Instant::now() < deadline, "child reviewer timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
struct ChildOwner(Child);
impl Drop for ChildOwner {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            if let Err(error) = self.0.kill() { eprintln!("reviewer child termination: {error}"); }
            if let Err(error) = self.0.wait() { eprintln!("reviewer child reaping: {error}"); }
        }
    }
}

#[test]
fn same_account_wrong_pid_is_rejected_then_the_pinned_child_can_review() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let (action, inputs) = reviewed(&mut host, 1, b"child reviewed");
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let path = root.0.join("review.sock"); let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    // Queue the same-account parent first; only the independently pinned child
    // may receive evidence. This uses connect, not an inherited socketpair PID.
    let mut intruder = UnixStream::connect(&path).unwrap();
    let (probe, _) = UnixStream::pair().unwrap(); let self_id = PeerCredentials::observe(&probe).unwrap();
    let mut child = ChildOwner(Command::new(std::env::current_exe().unwrap()).arg("--exact").arg(CHILD)
        .env(CHILD_SOCKET, &path).env(CHILD_PARENT, std::process::id().to_string())
        .env(CHILD_UID, self_id.uid().to_string()).env(CHILD_GID, self_id.gid().to_string())
        .spawn().unwrap());
    let policy = PeerPolicy::new(self_id.uid(), self_id.gid(), Some(child.0.id())).unwrap();
    let mut gate = ReviewerPeerAdmission::new(policy, 2).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let socket = loop {
        match listener.accept() {
            Ok((stream, _)) => match gate.admit(stream) {
                Ok(socket) => break socket,
                Err(ReviewerPeerError::Credentials(io::ErrorKind::PermissionDenied)) => {
                    no_received_bytes(&mut intruder);
                    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
                }
                Err(error) => panic!("unexpected peer failure: {error:?}"),
            },
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("accept: {error}"),
        }
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(gate.status().rejected, 1); assert_eq!(socket.peer().pid(), child.0.id());
    let mut server = socket.into_connection(&host, &reviewer, request, [42; 32]).unwrap();
    let mut approval = None;
    while server.phase() != ReviewerPhase::Complete {
        if let ReviewerProgress::Applied(application) = server.step(&mut host, &reviewer, || ElapsedTick(1)).unwrap() {
            assert!(approval.is_none()); approval = application.approval;
        }
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1));
    }
    loop {
        if let Some(status) = child.0.try_wait().unwrap() { assert!(status.success()); break; }
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(1));
    }
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    host.dispatch(host.revision(), &automatic, &approval.unwrap(), &action, &inputs, snapshot()).unwrap();
    host.publish(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1);
}
