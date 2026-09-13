//! Drive durable requests through the original bounded nonblocking Unix adapter.
#![cfg(unix)]
#[path = "support/file_delivery.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget, MAX_DRIVE_BYTES};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ActorChannel, ChannelLimits, ChannelState, Command, WireError, WireResponse, encode_command};
use fa_reference::round::Verdict;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

fn command() -> Command {
    Command::Submit { request: 70, proposal: ActorProposal {
        target: profile().target, payload: b"payload".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0,
    } }
}
fn connection(wire: ActorWire<FileActorPort>) -> (UnixActorConnection<FileActorPort>, UnixStream) {
    let (socket, peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let channel = ActorChannel::new(wire, ChannelLimits::default()).unwrap();
    (UnixActorConnection::new(socket, channel).unwrap(), peer)
}
fn exchange(connection: &mut UnixActorConnection<FileActorPort>, peer: &mut UnixStream,
    command: &Command) -> Vec<u8>
{
    let mut frame = encode_command(command).unwrap(); frame.push(b'\n');
    peer.write_all(&frame).unwrap();
    let budget = DriveBudget { read_bytes: 7, write_bytes: 3, frames: 1, io_calls: 4 };
    let mut output = Vec::new(); let mut frames = 0;
    for _ in 0..4096 {
        let report = connection.drive(budget).unwrap();
        assert!(report.status.failure.is_none());
        assert!(report.progress.read_bytes <= budget.read_bytes);
        assert!(report.progress.written_bytes <= budget.write_bytes);
        assert!(report.progress.io_calls <= budget.io_calls);
        assert!(report.progress.frames <= budget.frames);
        frames += report.progress.frames;
        let mut bytes = [0; 1024];
        match peer.read(&mut bytes) {
            Ok(0) => panic!("unexpected peer closure"),
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {},
            Err(error) => panic!("peer read: {error}"),
        }
        if output.ends_with(b"\n") { assert_eq!(frames, 1); return output; }
        std::thread::yield_now();
    }
    panic!("bounded socket fixture did not finish");
}
fn encoded(response: WireResponse) -> Vec<u8> {
    let mut bytes = response.encode(); bytes.push(b'\n'); bytes
}

#[test]
fn durable_backend_keeps_original_socket_budgets_and_full_publication_path() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (mut channel, mut peer) = connection(ActorWire::new(port.clone()));
    assert_eq!(channel.drive(DriveBudget { read_bytes: MAX_DRIVE_BYTES + 1, ..DriveBudget::default() }), Err(WireError::Capacity));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    let output = exchange(&mut channel, &mut peer, &command());
    assert_eq!(output, encoded(WireResponse { request: Some(70), result: Ok(Knowledge::Pending { request: 70 }) }));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let ticket = match command() {
        Command::Submit { proposal, .. } => port.submit(70, &proposal).unwrap(),
        _ => unreachable!(),
    };
    {
        let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(70).unwrap().disposition
            else { panic!("request not admitted"); };
        host.review(host.revision(), review(attempt, 100, Verdict::Allow)).unwrap();
        let permit = host.authorize(host.revision(), attempt, snapshot()).unwrap();
        let action = host.request_action(70).unwrap().clone();
        host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
        host.publish(host.revision(), attempt).unwrap();
        host.reconcile(host.revision(), attempt).unwrap();
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    }
    let result = port.poll(&ticket);
    assert!(matches!(result, Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let revision = supervisor.host().unwrap().revision();
    let bytes = exchange(&mut channel, &mut peer, &Command::Poll { request: 70 });
    assert_eq!(bytes, encoded(WireResponse { request: Some(70), result: Ok(result) }));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
}

#[test]
fn losing_a_ready_socket_response_cannot_repeat_the_durable_submission() {
    let root = Directory::new(); let (port, mut supervisor) = create(&root).into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (mut channel, mut peer) = connection(ActorWire::new(port));
    let mut frame = encode_command(&command()).unwrap(); frame.push(b'\n'); peer.write_all(&frame).unwrap();
    for _ in 0..128 {
        channel.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
        if channel.status().channel == ChannelState::ReplyReady { break; }
        std::thread::yield_now();
    }
    assert_eq!(channel.status().channel, ChannelState::ReplyReady);
    assert!(channel.status().pending_output_bytes > 0);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let revision = supervisor.host().unwrap().revision();
    let wire = channel.into_session(); drop(peer);
    let (mut channel, mut peer) = connection(wire);
    let output = exchange(&mut channel, &mut peer, &command());
    assert_eq!(output, encoded(WireResponse { request: Some(70), result: Ok(Knowledge::Pending { request: 70 }) }));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}
