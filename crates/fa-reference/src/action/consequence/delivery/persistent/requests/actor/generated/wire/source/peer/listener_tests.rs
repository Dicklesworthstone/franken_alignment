//! Named Unix sockets enter the original source, numerical and two-key paths.
use super::super::*;
use crate::Error;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_peer::{
    AcceptEvent, ListenerPollBudget, PeerCredentials, PeerPolicy, PeerRefusal,
    PeerSession, UnixPeerListener,
};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, MAX_DRIVE_FRAMES};
use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, Command, WireError, WireResponse, decode_response, encode_command,
};
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::FileActorPeerDriveError;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

// Compile the original fixture unchanged under this test module. It still runs
// the real native generator from synthetic weights and uses actual source files.
#[path = "../tests/fixture.rs"]
mod fixture;
use fixture::*;

type Listener = UnixPeerListener<FileGeneratedTextActorPort>;

fn no_clock() -> ElapsedTick { panic!("this path must not acquire evidence") }

fn policy() -> PeerPolicy {
    let (server, _client) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&server).unwrap();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
}

fn bound(port: FileGeneratedTextActorPort, path: &Path, policy: PeerPolicy, limit: u64) -> Listener {
    let session = PeerSession::new(policy, ActorWire::new(port), ChannelLimits::default(), limit).unwrap();
    UnixPeerListener::new(UnixListener::bind(path).unwrap(), session).unwrap()
}

fn connect(path: &Path) -> UnixStream {
    let client = UnixStream::connect(path).unwrap();
    client.set_nonblocking(true).unwrap();
    client
}

fn send(client: &mut UnixStream, document: &[u8]) {
    let mut line = document.to_vec();
    line.push(b'\n');
    client.write_all(&line).unwrap();
}

fn input(accept: bool) -> ListenerPollBudget {
    ListenerPollBudget { accept, drive: DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
}

fn drain(listener: &mut Listener, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    for _ in 0..64 {
        // Only write/flush is permitted here. Even buffered input cannot bypass
        // source-aware admission because the complete-frame allowance is zero.
        let poll = listener.poll(ListenerPollBudget { accept: false,
            drive: DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() } }).unwrap();
        assert!(poll.accept.is_none());
        assert_eq!(poll.drive.unwrap().unwrap().progress.frames, 0);
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("EOF before response"),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("response read: {error}"),
        }
        if bytes.last() == Some(&b'\n') {
            return decode_response(&bytes[..bytes.len() - 1]).unwrap();
        }
    }
    panic!("response did not drain in bounded turns");
}

#[test]
fn named_generated_listener_reaches_publication_only_after_both_original_keys() {
    use crate::action::consequence::oversight::ReviewWindow;
    use crate::action::consequence::delivery::EndpointOutcome;
    use crate::action::consequence::delivery::stream::ReleaseFrame;
    use crate::round::{Verdict, commitment};
    let mut s = setup();
    let path = s.root.source().with_file_name("actor.sock");
    let mut listener = bound(s.port.clone(), &path, policy(), 4);
    let before = disk(&s);
    let idle = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(true)).unwrap();
    assert_eq!(idle.accept, Some(Ok(AcceptEvent::Idle)));
    assert!(idle.drive.is_none());
    assert_eq!(disk(&s), before);
    let mut client = connect(&path);
    send(&mut client, &document(&s.input));
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(true)).unwrap();
    assert!(matches!(report.accept, Some(Ok(AcceptEvent::Admitted(admitted))) if admitted.connection == 1));
    let drive = report.drive.unwrap().unwrap();
    assert_eq!(drive.drive.progress.frames, 1);
    assert_eq!(drive.intakes.len(), 1);
    assert_eq!(drive.intakes[0].result, Ok(capture(1, true, b"allow").identity()));
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));

    let mut borrowed = s.supervisor.host_mut().unwrap();
    let host = &mut *borrowed;
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(91).unwrap().disposition
        else { panic!("expected admission"); };
    let action = host.request_action(91).unwrap().clone();
    assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
    assert_eq!(action.spec().units, action.spec().payload.len() as u64);
    assert_eq!(host.inspect().executions, 0);
    let observed = capture(1, true, b"allow");
    let snapshot = observed.snapshot().clone();
    let inputs = observed.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), attempt, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot.clone()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer", commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot.clone()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot.clone()).unwrap();
    assert!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot.clone(), ElapsedTick(2)).is_err());
    assert_eq!(host.inspect().executions, 0);
    let request = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = s.reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), attempt).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"A");
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    drop(borrowed);
    let reads = s.source.status().read_attempts;
    send(&mut client, &encode_command(&Command::Poll { request: 91 }).unwrap());
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(false)).unwrap();
    assert!(report.drive.unwrap().unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut listener, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(s.source.status().read_attempts, reads);
}

#[test]
fn named_listener_credentials_are_checked_before_source_intake() {
    let mut s = setup();
    let matching = policy();
    let wrong_uid = if matching.uid() == 0 { 1 } else { 0 };
    let wrong = PeerPolicy::new(wrong_uid, matching.gid(), matching.pid()).unwrap();
    let refused_path = s.root.source().with_file_name("refused.sock");
    let mut refused = bound(s.port.clone(), &refused_path, wrong, 4);
    let mut client = connect(&refused_path);
    send(&mut client, &document(&s.input));
    let before = disk(&s);
    let reads = s.source.status().read_attempts;
    let report = s.supervisor.poll_generated_listener_from_file(&mut refused, &mut s.source, no_clock, input(true)).unwrap();
    assert_eq!(report.accept, Some(Ok(AcceptEvent::Rejected(PeerRefusal::CredentialsRejected))));
    assert!(report.drive.is_none());
    assert_eq!(refused.status().session.connections_admitted, 0);
    assert_eq!(disk(&s), before);
    assert_eq!(s.source.status().read_attempts, reads);

    let path = s.root.source().with_file_name("matching.sock");
    let mut listener = bound(s.port.clone(), &path, matching, 4);
    let mut client = connect(&path);
    send(&mut client, &document(&s.input));
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    let report = driver.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(true)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn named_listener_foreign_owner_invalid_budget_and_unscheduled_accept_leave_backlog_untouched() {
    let mut a = setup();
    let mut b = setup();
    let path = b.root.source().with_file_name("actor.sock");
    let mut listener = bound(b.port.clone(), &path, policy(), 4);
    let mut client = connect(&path);
    send(&mut client, &document(&b.input));
    let status = listener.status();
    let before_a = disk(&a);
    let before_b = disk(&b);
    assert!(matches!(a.supervisor.poll_generated_listener_from_file(&mut listener, &mut a.source, no_clock, input(true)),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(listener.status(), status);
    assert_eq!(disk(&a), before_a);
    assert_eq!(disk(&b), before_b);
    let invalid = ListenerPollBudget { accept: true,
        drive: DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..DriveBudget::default() } };
    assert!(matches!(b.supervisor.poll_generated_listener_from_file(&mut listener, &mut b.source, no_clock, invalid),
        Err(FileActorPeerDriveError::Wire(WireError::Capacity))));
    assert_eq!(listener.status(), status);
    let zero = ListenerPollBudget { accept: false,
        drive: DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 } };
    let report = b.supervisor.poll_generated_listener_from_file(&mut listener, &mut b.source, no_clock, zero).unwrap();
    assert!(report.accept.is_none());
    assert!(report.drive.is_none());
    assert_eq!(listener.status(), status);
    let report = b.supervisor.poll_generated_listener_from_file(&mut listener, &mut b.source, || ElapsedTick(2), input(true)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
}

#[test]
fn named_listener_fragments_and_reply_backpressure_preserve_source_read_boundaries() {
    let mut s = setup();
    let path = s.root.source().with_file_name("actor.sock");
    let mut listener = bound(s.port.clone(), &path, policy(), 4);
    let mut client = connect(&path);
    let doc = document(&s.input);
    let reads = s.source.status().read_attempts;
    client.write_all(&doc).unwrap();
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(true)).unwrap();
    let drive = report.drive.unwrap().unwrap();
    assert_eq!(drive.drive.progress.frames, 0);
    assert!(drive.intakes.is_empty());
    client.write_all(b"\n").unwrap();
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    send(&mut client, &encode_command(&Command::Cancel { request: 91 }).unwrap());
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(false)).unwrap();
    let drive = report.drive.unwrap().unwrap();
    assert_eq!(drive.drive.progress.frames, 0);
    assert!(drive.intakes.is_empty());
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(false)).unwrap();
    assert!(report.drive.unwrap().unwrap().intakes.is_empty());
    assert!(matches!(drain(&mut listener, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(s.source.status().read_attempts, reads + 1);

    s.root.replace(&capture(2, true, b"deny"));
    let mut next = s.input.clone();
    next.request = 92;
    send(&mut client, &document(&next));
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(3), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes[0].result, Ok(capture(2, true, b"deny").identity()));
    assert!(matches!(drain(&mut listener, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    assert_eq!(s.supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn named_listener_busy_rejection_does_not_starve_the_existing_peer_or_reset_reconnect_limits() {
    let mut s = setup();
    let path = s.root.source().with_file_name("actor.sock");
    let mut listener = bound(s.port.clone(), &path, policy(), 2);
    let mut client = connect(&path);
    send(&mut client, &document(&s.input));
    s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(true)).unwrap();
    let reads = s.source.status().read_attempts;
    let before = disk(&s);
    let _candidate = connect(&path);
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock,
        ListenerPollBudget { accept: true, drive: DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() } }).unwrap();
    assert_eq!(report.accept, Some(Ok(AcceptEvent::Rejected(PeerRefusal::Busy))));
    let drive = report.drive.unwrap().unwrap();
    assert!(drive.drive.progress.written_bytes > 0);
    assert!(drive.intakes.is_empty());
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(listener.status().session.connections_admitted, 1);
    assert!(listener.disconnect());
    drop(client);
    let mut client = connect(&path);
    send(&mut client, &encode_command(&Command::Poll { request: 91 }).unwrap());
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(true)).unwrap();
    assert!(matches!(report.accept, Some(Ok(AcceptEvent::Admitted(admitted))) if admitted.connection == 2));
    assert!(report.drive.unwrap().unwrap().intakes.is_empty());
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
    assert!(listener.disconnect());
    let _over_limit = connect(&path);
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(true)).unwrap();
    assert_eq!(report.accept, Some(Ok(AcceptEvent::Rejected(PeerRefusal::Capacity))));
    assert!(report.drive.is_none());
    assert_eq!(disk(&s), before);
    assert_eq!(s.source.status().read_attempts, reads);
    assert!(listener.revoke());
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, no_clock, input(true)).unwrap();
    assert_eq!(report.accept, Some(Ok(AcceptEvent::Stopped)));
    assert!(report.drive.is_none());
    assert!(matches!(s.supervisor.host().unwrap().request_status(91).unwrap().disposition,
        FileRequestDisposition::Admitted { .. }));
}

#[test]
fn named_listener_reports_admission_even_when_the_trusted_drive_callback_fails() {
    let mut s = setup();
    let path = s.root.source().with_file_name("actor.sock");
    let mut listener = bound(s.port.clone(), &path, policy(), 4);
    let mut client = connect(&path);
    send(&mut client, &document(&s.input));
    let before = disk(&s);
    // Inject only a trusted scheduler callback failure; this is a report-law
    // unit check, not evidence of an actual disk or allocator failure.
    let report = listener.poll_with(input(true), |_, _| Err::<(), _>(WireError::Unavailable)).unwrap();
    assert!(matches!(report.accept, Some(Ok(AcceptEvent::Admitted(_)))));
    assert_eq!(report.drive, Some(Err(WireError::Unavailable)));
    assert_eq!(disk(&s), before);
    let report = s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(false)).unwrap();
    assert!(report.accept.is_none());
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
}

#[test]
fn named_listener_recovery_retains_the_durable_source_floor_and_paused_decoder() {
    use crate::action::consequence::delivery::persistent::observed::guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    };
    use crate::action::consequence::oversight::evidence_source::FileEvidenceSource;
    let mut s = setup();
    s.root.replace(&capture(2, true, b"allow"));
    let path = s.root.source().with_file_name("actor.sock");
    let mut listener = bound(s.port.clone(), &path, policy(), 4);
    let mut client = connect(&path);
    let doc = document(&s.input);
    send(&mut client, &doc);
    s.supervisor.poll_generated_listener_from_file(&mut listener, &mut s.source, || ElapsedTick(2), input(true)).unwrap();
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 91 }));
    let host = s.supervisor.host().unwrap();
    let control = host.inspect().control;
    let requirements = FileRecoveryRequirements { guards: FileGuardSet {
        stream: Some(s.port.profile()), decoder: Some(s.configuration.clone()), decoder_stop: None,
        source: Some(source_policy()), identity: None, campaigns: None, credential: None,
    }, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } };
    let anchor = host.history_anchor().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    drop(host);
    drop(s.supervisor);
    drop(listener);
    drop(client);
    let (host, _roles) = FileOversight::open_generated_text_stream_anchored(s.root.store(), profile(),
        &requirements, &s.tokenizer, &anchor).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    s.root.replace(&capture(1, true, b"allow"));
    let mut source = FileEvidenceSource::new(s.root.source(), 7, profile().delivery.scope, 4096).unwrap();
    let path = s.root.source().with_file_name("recovered.sock");
    let mut listener = bound(port, &path, policy(), 4);
    let mut client = connect(&path);
    send(&mut client, &doc);
    let report = supervisor.poll_generated_listener_from_file(&mut listener, &mut source, no_clock, input(true)).unwrap();
    assert!(report.drive.unwrap().unwrap().intakes.is_empty());
    assert_eq!(source.status().read_attempts, 0);
    assert!(matches!(drain(&mut listener, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let mut next = s.input.clone();
    next.request = 92;
    next.policy_epoch = supervisor.host().unwrap().inspect().control.ledger.epoch;
    send(&mut client, &document(&next));
    let report = supervisor.poll_generated_listener_from_file(&mut listener, &mut source, || ElapsedTick(3), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes[0].result, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(drain(&mut listener, &mut client).result, Err(WireError::Unavailable));
    assert_eq!(supervisor.host().unwrap().file_source_status().unwrap().producer.unwrap().generation, 2);
    assert!(supervisor.host().unwrap().decoder_inspection().unwrap().paused);
}
