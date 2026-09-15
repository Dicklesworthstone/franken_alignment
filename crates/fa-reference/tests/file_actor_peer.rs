//! Kernel-authenticated ingress for the durable actor request backend.
#![cfg(target_os = "linux")]
#[path = "support/file_delivery.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerRefusal, PeerSession};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, WireResponse, encode_command};
use fa_reference::round::Verdict;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

fn proposal() -> ActorProposal {
    ActorProposal { target: profile().target, payload: b"payload".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
fn submit() -> Command { Command::Submit { request: 70, proposal: proposal() } }
fn encoded(response: WireResponse) -> Vec<u8> { let mut bytes = response.encode(); bytes.push(b'\n'); bytes }
fn policy(socket: &UnixStream) -> PeerPolicy {
    let observed = PeerCredentials::observe(socket).unwrap();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}
fn exchange(session: &mut PeerSession<FileActorPort>, peer: &mut UnixStream, command: &Command) -> Vec<u8> {
    peer.set_nonblocking(true).unwrap();
    let mut frame = encode_command(command).unwrap(); frame.push(b'\n'); peer.write_all(&frame).unwrap();
    let budget = DriveBudget { read_bytes: 7, write_bytes: 3, frames: 1, io_calls: 4 };
    let mut output = Vec::new();
    for _ in 0..4096 {
        let report = session.drive(budget).unwrap();
        assert!(report.status.failure.is_none());
        assert!(report.progress.read_bytes <= budget.read_bytes);
        assert!(report.progress.written_bytes <= budget.write_bytes);
        assert!(report.progress.frames <= budget.frames);
        assert!(report.progress.io_calls <= budget.io_calls);
        let mut bytes = [0; 1024];
        match peer.read(&mut bytes) {
            Ok(0) => panic!("unexpected peer closure"),
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("peer read: {error}"),
        }
        if output.ends_with(b"\n") { return output; }
        std::thread::yield_now();
    }
    panic!("authenticated durable exchange did not finish within fixed bound");
}

#[test]
fn authenticated_peer_drives_the_original_durable_request_and_publication() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (server, mut peer) = UnixStream::pair().unwrap();
    let admission_policy = policy(&server);
    let mut session: PeerSession<FileActorPort> = PeerSession::new(admission_policy,
        ActorWire::new(port.clone()), ChannelLimits::default(), 4).unwrap();
    let admission = session.attach(server).unwrap();
    assert_eq!(admission.credentials.uid(), admission_policy.uid());
    assert_eq!(admission.credentials.gid(), admission_policy.gid());
    assert_eq!(admission.credentials.pid(), admission_policy.pid().unwrap());
    assert_eq!(exchange(&mut session, &mut peer, &submit()), encoded(WireResponse {
        request: Some(70), result: Ok(Knowledge::Pending { request: 70 }) }));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);

    let ticket = port.submit(70, &proposal()).unwrap();
    {
        let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(70).unwrap().disposition
            else { panic!("authenticated request not admitted"); };
        host.review(host.revision(), review(attempt, 100, Verdict::Allow)).unwrap();
        let permit = host.authorize(host.revision(), attempt, snapshot()).unwrap();
        let action = host.request_action(70).unwrap().clone();
        host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
        host.publish(host.revision(), attempt).unwrap();
        host.reconcile(host.revision(), attempt).unwrap();
        assert_eq!(host.inspect().executions, 1);
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}

#[test]
fn rejected_kernel_peer_cannot_consume_the_durable_request_or_one_use_snapshot() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (server, _peer) = UnixStream::pair().unwrap();
    let observed = PeerCredentials::observe(&server).unwrap();
    let wrong_pid = if observed.pid() < i32::MAX as u32 { observed.pid() + 1 } else { observed.pid() - 1 };
    let wrong = PeerPolicy::new(observed.uid(), observed.gid(), Some(wrong_pid)).unwrap();
    let mut refused: PeerSession<FileActorPort> = PeerSession::new(wrong,
        ActorWire::new(port.clone()), ChannelLimits::default(), 1).unwrap();
    assert_eq!(refused.attach(server), Err(PeerRefusal::CredentialsRejected));
    assert_eq!(refused.status().connections_admitted, 0);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);

    // Positive control: the refused socket did not consume the supervisor's
    // one-use admission snapshot. A separately authenticated session can use it.
    let (server, mut peer) = UnixStream::pair().unwrap(); let good = policy(&server);
    let mut accepted: PeerSession<FileActorPort> = PeerSession::new(good,
        ActorWire::new(port), ChannelLimits::default(), 1).unwrap();
    accepted.attach(server).unwrap();
    assert_eq!(exchange(&mut accepted, &mut peer, &submit()), encoded(WireResponse {
        request: Some(70), result: Ok(Knowledge::Pending { request: 70 }) }));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
}

#[test]
fn reconnect_reauthenticates_and_exact_retry_does_not_require_a_new_snapshot() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    let revision = supervisor.host().unwrap().revision(); supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (server, mut peer) = UnixStream::pair().unwrap(); let admission_policy = policy(&server);
    let mut session: PeerSession<FileActorPort> = PeerSession::new(admission_policy,
        ActorWire::new(port), ChannelLimits::default(), 2).unwrap();
    session.attach(server).unwrap(); peer.set_nonblocking(true).unwrap();
    let mut frame = encode_command(&submit()).unwrap(); frame.push(b'\n'); peer.write_all(&frame).unwrap();
    for _ in 0..256 {
        let report = session.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
        if report.status.pending_output_bytes > 0 { break; }
        std::thread::yield_now();
    }
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let committed_revision = supervisor.host().unwrap().revision();
    assert!(session.disconnect()); drop(peer);

    let (server, mut peer) = UnixStream::pair().unwrap();
    session.attach(server).unwrap();
    assert_eq!(session.status().connections_admitted, 2);
    assert_eq!(exchange(&mut session, &mut peer, &submit()), encoded(WireResponse {
        request: Some(70), result: Ok(Knowledge::Pending { request: 70 }) }));
    assert_eq!(supervisor.host().unwrap().revision(), committed_revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}
