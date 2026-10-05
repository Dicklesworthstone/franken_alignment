//! Actual named Unix sockets and original kernel credentials before policy I/O.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::FileActorPeerDriveError;
use crate::action::consequence::oversight::actor_peer::{AcceptEvent, ListenerPollBudget,
    PeerCredentials, PeerPolicy, PeerRefusal, PeerSession, UnixPeerListener};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, MAX_DRIVE_FRAMES};
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, decode_response};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

pub(super) fn policy() -> PeerPolicy {
    let (server, _client) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&server).unwrap();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
}
pub(super) fn bound<P: ActorRequestPort>(port: P, path: &Path, policy: PeerPolicy) -> UnixPeerListener<P> {
    let session = PeerSession::new(policy, ActorWire::new(port), ChannelLimits::default(), 4).unwrap();
    UnixPeerListener::new(UnixListener::bind(path).unwrap(), session).unwrap()
}
pub(super) fn connect(path: &Path) -> UnixStream {
    let client = UnixStream::connect(path).unwrap(); client.set_nonblocking(true).unwrap(); client
}
pub(super) fn send(client: &mut UnixStream, document: &[u8]) {
    let mut line = document.to_vec(); line.push(b'\n'); client.write_all(&line).unwrap();
}
pub(super) fn input(accept: bool) -> ListenerPollBudget {
    ListenerPollBudget { accept, drive: DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
}
pub(super) fn drain<P: ActorRequestPort>(listener: &mut UnixPeerListener<P>, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    for _ in 0..64 {
        // The ordinary transport may WRITE only. Zero frame budget prevents
        // unobserved requests in its buffered input bypassing coupled intake.
        let report = listener.poll(ListenerPollBudget { accept: false,
            drive: DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() } }).unwrap();
        assert_eq!(report.drive.unwrap().unwrap().progress.frames, 0);
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("EOF before original wire response"),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("wire response read: {error}"),
        }
        if bytes.last() == Some(&b'\n') { return decode_response(&bytes[..bytes.len() - 1]).unwrap(); }
    }
    panic!("original wire response did not drain in bounded turns");
}

#[test]
fn named_listener_credentials_and_idle_are_read_free_before_original_positive_intake() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let matching = policy(); let wrong_uid = if matching.uid() == 0 { 1 } else { 0 };
    let wrong = PeerPolicy::new(wrong_uid, matching.gid(), matching.pid()).unwrap();
    let rejected_path = path(&root).with_extension("rejected.sock");
    let mut rejected = bound(port.clone(), &rejected_path, wrong);
    let mut rejected_client = connect(&rejected_path); send(&mut rejected_client, &raw_document(71));
    let revision = supervisor.host().unwrap().revision();
    let report = supervisor.poll_learned_text_listener_from_policy_file(&mut rejected,
        &mut source, no_clock, input(true)).unwrap();
    assert_eq!(report.accept, Some(Ok(AcceptEvent::Rejected(PeerRefusal::CredentialsRejected))));
    assert!(report.drive.is_none()); assert_eq!(source.status().read_attempts, 0);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    let socket = path(&root).with_extension("actor.sock"); let mut listener = bound(port, &socket, matching);
    let idle = supervisor.poll_learned_text_listener_from_policy_file(&mut listener, &mut source, no_clock, input(true)).unwrap();
    assert_eq!(idle.accept, Some(Ok(AcceptEvent::Idle))); assert!(idle.drive.is_none());
    let mut client = connect(&socket); send(&mut client, &raw_document(71));
    let report = supervisor.poll_learned_text_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(2), input(true)).unwrap();
    assert!(matches!(report.accept, Some(Ok(AcceptEvent::Admitted(peer))) if peer.connection == 1));
    let drive = report.drive.unwrap().unwrap(); assert_eq!(drive.drive.progress.frames, 1);
    assert_eq!(drive.intakes.len(), 1); assert_eq!(drive.intakes[0].result, Ok(capture(1).identity()));
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(supervisor.host().unwrap().request_action(71).unwrap().spec().payload, b"aa");
    send(&mut client, &raw_document(71));
    let report = supervisor.poll_learned_text_listener_from_policy_file(&mut listener,
        &mut source, no_clock, input(false)).unwrap();
    assert!(report.drive.unwrap().unwrap().intakes.is_empty());
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(source.status().read_attempts, 1); assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn foreign_owner_and_invalid_budget_do_not_accept_queued_peers_or_read_sources() {
    let a = Directory::new(); let b = Directory::new();
    let (host_a, _) = owner(&a, &config()); let (host_b, _) = owner(&b, &config());
    let (_port_a, mut supervisor_a) = host_a.into_learned_text_actor_gateway().unwrap();
    let (port_b, mut supervisor_b) = host_b.into_learned_text_actor_gateway().unwrap();
    let socket = path(&b).with_extension("actor.sock"); let mut listener = bound(port_b, &socket, policy());
    let mut client = connect(&socket); send(&mut client, &raw_document(71));
    write(&path(&b), &capture(1)); let mut source = source(&b);
    let status = listener.status();
    assert!(matches!(supervisor_a.poll_learned_text_listener_from_policy_file(&mut listener, &mut source, no_clock, input(true)),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(listener.status(), status);
    let invalid = ListenerPollBudget { accept: true,
        drive: DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..DriveBudget::default() } };
    assert!(matches!(supervisor_b.poll_learned_text_listener_from_policy_file(&mut listener, &mut source, no_clock, invalid),
        Err(FileActorPeerDriveError::Wire(WireError::Capacity))));
    assert_eq!(listener.status(), status); assert_eq!(source.status().read_attempts, 0);
    let report = supervisor_b.poll_learned_text_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(2), input(true)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
}

#[test]
fn authenticated_stream_fragments_and_pending_output_preserve_policy_read_boundaries() {
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let socket = path(&root).with_extension("actor.sock"); let mut listener = bound(port, &socket, policy());
    let mut client = connect(&socket);
    let doc = stream_document(71, LearnedTextRelease::Message); client.write_all(&doc).unwrap();
    let report = supervisor.poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, no_clock, input(true)).unwrap();
    let drive = report.drive.unwrap().unwrap();
    assert_eq!(drive.drive.progress.frames, 0); assert!(drive.intakes.is_empty());
    client.write_all(b"\n").unwrap();
    let report = supervisor.poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(2), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    send(&mut client, &stream_document(72, LearnedTextRelease::Message));
    let report = supervisor.poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, no_clock, input(false)).unwrap();
    let drive = report.drive.unwrap().unwrap();
    assert_eq!(drive.drive.progress.frames, 0); assert!(drive.intakes.is_empty());
    assert_eq!(source.status().read_attempts, 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
    let report = supervisor.poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(3), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 72 }));
    assert_eq!(source.status().read_attempts, 2);
    assert!(supervisor.host().unwrap().stream_snapshot().unwrap().published.visible().is_empty());
}
