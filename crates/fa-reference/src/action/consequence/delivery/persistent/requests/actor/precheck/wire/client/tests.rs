//! Deterministic I/O faults plus real authenticated source-backed prechecks.
//! Scripted response bytes test the client grammar, not provider authenticity.
use super::*;
use super::super::{FilePrecheckPeerSession, tests::{Fixture, request}};
use super::super::super::{FilePrecheckBasis, FilePrecheckOutcome};
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::oversight::actor::Knowledge;
use crate::action::consequence::oversight::actor_transport::{
    MAX_DRIVE_BYTES, MAX_DRIVE_FRAMES, MAX_DRIVE_IO_CALLS,
};
use std::collections::VecDeque;

enum ReadStep { Bytes(Vec<u8>), Error(io::ErrorKind), Eof }
enum WriteStep { Bytes(usize), Error(io::ErrorKind) }

#[derive(Default)]
struct Script {
    reads: VecDeque<ReadStep>,
    writes: VecDeque<WriteStep>,
    flushes: VecDeque<Result<(), io::ErrorKind>>,
    accepted: Vec<u8>,
    operations: Vec<IoDirection>,
    flushed: bool,
}

impl Script {
    fn reply(bytes: Vec<u8>) -> Self {
        Self { reads: VecDeque::from([ReadStep::Bytes(bytes)]), ..Self::default() }
    }
}

impl Read for Script {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        assert!(self.flushed, "a response read preceded full request flush");
        self.operations.push(IoDirection::Read);
        match self.reads.pop_front().unwrap_or(ReadStep::Error(io::ErrorKind::WouldBlock)) {
            ReadStep::Bytes(bytes) => {
                let count = bytes.len().min(buffer.len());
                buffer[..count].copy_from_slice(&bytes[..count]);
                if count < bytes.len() {
                    self.reads.push_front(ReadStep::Bytes(bytes[count..].to_vec()));
                }
                Ok(count)
            }
            ReadStep::Error(kind) => Err(io::Error::from(kind)),
            ReadStep::Eof => Ok(0),
        }
    }
}

impl Write for Script {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert!(!self.flushed, "request was transmitted after its flush");
        self.operations.push(IoDirection::Write);
        match self.writes.pop_front().unwrap_or(WriteStep::Bytes(bytes.len())) {
            WriteStep::Bytes(count) => {
                let count = count.min(bytes.len());
                self.accepted.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            WriteStep::Error(kind) => Err(io::Error::from(kind)),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.operations.push(IoDirection::Flush);
        match self.flushes.pop_front().unwrap_or(Ok(())) {
            Ok(()) => { self.flushed = true; Ok(()) }
            Err(kind) => Err(io::Error::from(kind)),
        }
    }
}

fn observation(query: u64, epoch: u64) -> PrecheckWireResponse {
    PrecheckWireResponse { query: Some(query), result: Ok(Knowledge::Known {
        value: FilePrecheckOutcome::PrecheckPassed,
        basis: FilePrecheckBasis { disclosure: 81, query, policy_epoch: epoch, observed_at: ElapsedTick(1) },
    }) }
}
fn frame(response: PrecheckWireResponse) -> Vec<u8> {
    let mut bytes = response.encode().unwrap(); bytes.push(b'\n'); bytes
}
fn exchange() -> Exchange { Exchange::new(request(7, b"ok"), 0).unwrap() }
fn one_call() -> DriveBudget { DriveBudget { io_calls: 1, ..DriveBudget::default() } }
fn peer_policy(socket: &UnixStream) -> PeerPolicy {
    let credentials = PeerCredentials::observe(socket).unwrap();
    PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap()
}
fn assert_latched(exchange: &mut Exchange, script: &mut Script, expected: PrecheckClientError) {
    assert_eq!(exchange.status().phase, PrecheckClientPhase::Failed);
    assert_eq!(exchange.status().failure, Some(expected));
    assert!(exchange.response.is_none());
    let calls = script.operations.len();
    let again = exchange.drive_io(script, DriveBudget::default()).unwrap();
    assert_eq!(again.progress, DriveProgress::default());
    assert_eq!(again.status.failure, Some(expected));
    assert_eq!(script.operations.len(), calls);
}

#[test]
fn partial_request_and_flush_resume_before_the_first_response_read() {
    let mut exchange = exchange();
    let exact_request = exchange.request.clone();
    let mut script = Script::reply(frame(observation(7, 0)));
    script.writes = VecDeque::from([WriteStep::Bytes(2),
        WriteStep::Error(io::ErrorKind::Interrupted), WriteStep::Error(io::ErrorKind::WouldBlock)]);
    script.flushes = VecDeque::from([Err(io::ErrorKind::Interrupted), Err(io::ErrorKind::WouldBlock)]);
    let budget = DriveBudget { io_calls: 3, ..DriveBudget::default() };
    let first = exchange.drive_io(&mut script, budget).unwrap();
    assert_eq!((first.progress.io_calls, first.progress.written_bytes), (3, 2));
    assert_eq!(first.progress.would_block, Some(IoDirection::Write));
    assert_eq!(first.status.phase, PrecheckClientPhase::Writing);
    let second = exchange.drive_io(&mut script, budget).unwrap();
    assert_eq!(second.progress.io_calls, 3);
    assert_eq!(second.progress.would_block, Some(IoDirection::Flush));
    assert_eq!(second.status.phase, PrecheckClientPhase::Flushing);
    assert_eq!(second.status.interest(), DriveInterest::LocalWork);
    assert_eq!(script.accepted, exact_request);
    let flushed = exchange.drive_io(&mut script, DriveBudget { write_bytes: 0, ..one_call() }).unwrap();
    assert_eq!(flushed.status.phase, PrecheckClientPhase::Reading);
    assert_eq!(flushed.progress.read_bytes, 0);
    let line = exchange.drive_io(&mut script, one_call()).unwrap();
    assert_eq!((line.progress.frames, line.status.phase), (1, PrecheckClientPhase::Confirming));
    assert_eq!(line.status.interest(), DriveInterest::LocalWork);
    assert!(exchange.response.is_none());
    let confirmed = exchange.drive_io(&mut script, one_call()).unwrap();
    assert_eq!(confirmed.status.phase, PrecheckClientPhase::Complete);
    assert_eq!(confirmed.progress.would_block, Some(IoDirection::Read));
    assert_eq!(exchange.response.take(), Some(observation(7, 0)));
    assert_eq!(script.accepted, exact_request);
    assert_eq!(script.operations, vec![IoDirection::Write, IoDirection::Write, IoDirection::Write,
        IoDirection::Write, IoDirection::Flush, IoDirection::Flush, IoDirection::Flush,
        IoDirection::Read, IoDirection::Read]);
}

#[test]
fn one_byte_drives_preserve_the_exact_request_and_response_boundary() {
    let mut exchange = exchange();
    let expected_request = exchange.request.clone();
    let response = frame(observation(7, 0));
    let response_bytes = response.len();
    let mut script = Script::reply(response);
    let budget = DriveBudget { read_bytes: 1, write_bytes: 1, frames: 1, io_calls: 1 };
    let mut read = 0; let mut written = 0; let mut frames = 0;
    for _ in 0..4096 {
        let report = exchange.drive_io(&mut script, budget).unwrap();
        assert!(report.progress.read_bytes <= 1 && report.progress.written_bytes <= 1);
        assert!(report.progress.frames <= 1 && report.progress.io_calls <= 1);
        read += report.progress.read_bytes; written += report.progress.written_bytes;
        frames += report.progress.frames;
        if report.status.closed() { break; }
    }
    assert_eq!(exchange.status().phase, PrecheckClientPhase::Complete);
    assert_eq!((read, written, frames), (response_bytes, expected_request.len(), 1));
    assert_eq!(script.accepted, expected_request);
    assert_eq!(exchange.response.take(), Some(observation(7, 0)));
}

#[test]
fn zero_and_invalid_budgets_do_no_work_and_do_not_poison_the_exchange() {
    let mut exchange = exchange(); let mut script = Script::default();
    let before = exchange.status();
    for budget in [DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 },
        DriveBudget { frames: 0, ..DriveBudget::default() },
        DriveBudget { io_calls: 0, ..DriveBudget::default() }] {
        let report = exchange.drive_io(&mut script, budget).unwrap();
        assert_eq!(report.progress, DriveProgress::default()); assert_eq!(report.status, before);
    }
    for budget in [DriveBudget { read_bytes: MAX_DRIVE_BYTES + 1, ..DriveBudget::default() },
        DriveBudget { write_bytes: MAX_DRIVE_BYTES + 1, ..DriveBudget::default() },
        DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..DriveBudget::default() },
        DriveBudget { io_calls: MAX_DRIVE_IO_CALLS + 1, ..DriveBudget::default() }] {
        assert_eq!(exchange.drive_io(&mut script, budget), Err(PrecheckClientError::InvalidBudget));
        assert_eq!(exchange.status(), before);
    }
    assert!(script.operations.is_empty());
    script.reads.push_back(ReadStep::Bytes(frame(observation(7, 0))));
    assert_eq!(exchange.drive_io(&mut script, DriveBudget::default()).unwrap().status.phase,
        PrecheckClientPhase::Complete);
}

#[test]
fn query_epoch_and_closed_response_schema_failures_latch() {
    let good = String::from_utf8(frame(observation(7, 0))).unwrap();
    for (bytes, error) in [
        (frame(observation(8, 0)), PrecheckResponseError::Binding),
        (frame(observation(7, 1)), PrecheckResponseError::Binding),
        (good.replace("\"review_required\":true,", "").into_bytes(), PrecheckResponseError::Malformed),
        (good.replace("\"precheck\":true", "\"precheck\":false").into_bytes(), PrecheckResponseError::Malformed),
        (good.replace("fa.actor-precheck-response/1", "fa.actor-precheck-response/2").into_bytes(),
            PrecheckResponseError::UnsupportedVersion),
        (good.replacen("{", "{\"permit\":true,", 1).into_bytes(), PrecheckResponseError::Malformed),
    ] {
        let mut exchange = exchange(); let mut script = Script::reply(bytes);
        exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
        assert_latched(&mut exchange, &mut script, PrecheckClientError::Response(error));
    }
    // Stale is useful correlated Knowledge, not a falsely passed check.
    let stale = PrecheckWireResponse { query: Some(7),
        result: Ok(Knowledge::Stale { generation: 0, current: 1 }) };
    let mut exchange = exchange(); let mut script = Script::reply(frame(stale.clone()));
    exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
    assert_eq!(exchange.response, Some(stale));
}

#[test]
fn newline_and_exact_response_byte_limit_are_required() {
    let body = observation(7, 0).encode().unwrap();
    for prefix in [Vec::new(), body.clone()] {
        let mut exchange = exchange(); let mut script = Script::default();
        if !prefix.is_empty() { script.reads.push_back(ReadStep::Bytes(prefix)); }
        script.reads.push_back(ReadStep::Eof);
        exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
        assert_latched(&mut exchange, &mut script, PrecheckClientError::TruncatedResponse);
    }
    for extra in 0..=1 {
        let mut bytes = vec![b' '; MAX_PRECHECK_RESPONSE_BYTES - body.len() + extra];
        bytes.extend_from_slice(&body); bytes.push(b'\n');
        let mut exchange = exchange(); let mut script = Script::reply(bytes);
        script.reads.push_back(ReadStep::Eof);
        exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
        if extra == 0 {
            assert_eq!(exchange.response, Some(observation(7, 0)));
            assert_eq!(exchange.status().phase, PrecheckClientPhase::Complete);
        } else {
            assert_latched(&mut exchange, &mut script, PrecheckClientError::ResponseTooLarge);
        }
    }
}

#[test]
fn coalesced_and_fragmented_extra_bytes_cannot_hide_behind_a_drive_boundary() {
    let line = frame(observation(7, 0));
    let mut doubled = line.clone(); doubled.extend_from_slice(&line);
    for (reads, read_bytes) in [
        (VecDeque::from([ReadStep::Bytes(doubled)]), MAX_DRIVE_BYTES),
        (VecDeque::from([ReadStep::Bytes(line.clone()), ReadStep::Bytes(b"x".to_vec())]), 1),
        (VecDeque::from([ReadStep::Bytes(line), ReadStep::Error(io::ErrorKind::Interrupted),
            ReadStep::Bytes(b"\n".to_vec())]), MAX_DRIVE_BYTES),
    ] {
        let mut exchange = exchange(); let mut script = Script { reads, ..Script::default() };
        for _ in 0..2048 {
            exchange.drive_io(&mut script, DriveBudget { read_bytes, ..one_call() }).unwrap();
            assert!(exchange.response.is_none());
            if exchange.status().closed() { break; }
        }
        assert_latched(&mut exchange, &mut script, PrecheckClientError::TrailingResponseData);
    }
}

#[test]
fn terminal_io_failures_never_retry_and_interrupted_reads_spend_the_bound() {
    for (direction, kind) in [(IoDirection::Write, io::ErrorKind::WriteZero),
        (IoDirection::Write, io::ErrorKind::BrokenPipe),
        (IoDirection::Flush, io::ErrorKind::BrokenPipe),
        (IoDirection::Read, io::ErrorKind::ConnectionReset)] {
        let mut exchange = exchange(); let mut script = Script::default();
        match direction {
            IoDirection::Write if kind == io::ErrorKind::WriteZero => script.writes.push_back(WriteStep::Bytes(0)),
            IoDirection::Write => script.writes.push_back(WriteStep::Error(kind)),
            IoDirection::Flush => script.flushes.push_back(Err(kind)),
            IoDirection::Read => script.reads.push_back(ReadStep::Error(kind)),
        }
        exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
        assert_latched(&mut exchange, &mut script, PrecheckClientError::Io { direction, kind });
    }
    let mut exchange = exchange(); let mut script = Script::default();
    exchange.drive_io(&mut script, DriveBudget { read_bytes: 0, ..DriveBudget::default() }).unwrap();
    for _ in 0..4 { script.reads.push_back(ReadStep::Error(io::ErrorKind::Interrupted)); }
    let first = exchange.drive_io(&mut script, DriveBudget { io_calls: 3, ..DriveBudget::default() }).unwrap();
    assert_eq!(first.progress.io_calls, 3); assert_eq!(first.progress.read_bytes, 0);
    assert_eq!(first.status.phase, PrecheckClientPhase::Reading);
    let next = exchange.drive_io(&mut script, DriveBudget { io_calls: 3, ..DriveBudget::default() }).unwrap();
    assert_eq!(next.progress.io_calls, 2);
    assert_eq!(next.progress.would_block, Some(IoDirection::Read));
    assert!(next.status.failure.is_none());
}

#[test]
fn matching_wire_refusals_remain_responses_not_transport_failures() {
    for response in [
        PrecheckWireResponse { query: Some(7), result: Err(PrecheckWireError::Capacity) },
        PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Capacity) },
        PrecheckWireResponse { query: Some(7), result: Err(PrecheckWireError::Unavailable) },
        PrecheckWireResponse { query: Some(7), result: Ok(Knowledge::Withheld {
            authority_required: "exact_policy_disclosure" }) },
    ] {
        let mut exchange = exchange(); let mut script = Script::reply(frame(response.clone()));
        let report = exchange.drive_io(&mut script, DriveBudget::default()).unwrap();
        assert_eq!(report.status.phase, PrecheckClientPhase::Complete);
        assert!(report.status.failure.is_none());
        assert_eq!(exchange.response.take(), Some(response));
    }
}

#[test]
fn wrong_server_identity_or_request_binding_sends_no_bytes() {
    for dimension in 0..5 {
        let (socket, mut server) = UnixStream::pair().unwrap();
        let exact = peer_policy(&socket);
        let mut policy = exact; let mut candidate = request(7, b"ok"); let mut epoch = 0;
        let expected = match dimension {
            0 => { policy = PeerPolicy::new(if exact.uid() == 0 { 1 } else { 0 }, exact.gid(), exact.pid()).unwrap();
                PrecheckClientError::Peer(PeerRefusal::CredentialsRejected) }
            1 => { policy = PeerPolicy::new(exact.uid(), if exact.gid() == 0 { 1 } else { 0 }, exact.pid()).unwrap();
                PrecheckClientError::Peer(PeerRefusal::CredentialsRejected) }
            2 => { policy = PeerPolicy::new(exact.uid(), exact.gid(), Some(if exact.pid() == Some(1) { 2 } else { 1 })).unwrap();
                PrecheckClientError::Peer(PeerRefusal::CredentialsRejected) }
            3 => { epoch = 1; PrecheckClientError::ExpectedPolicyEpoch }
            _ => { candidate.query = 0; PrecheckClientError::Request(PrecheckWireError::MalformedRequest) }
        };
        assert_eq!(PrecheckPeerClient::connect(socket, policy, candidate, epoch).err(), Some(expected));
        server.set_nonblocking(true).unwrap();
        assert_eq!(server.read(&mut [0_u8; 1]).unwrap(), 0);
    }
    let (socket, mut server) = UnixStream::pair().unwrap();
    let policy = peer_policy(&socket);
    let mut candidate = request(7, b"payload-secret"); candidate.proposal.units = 32;
    let client = PrecheckPeerClient::connect(socket, policy, candidate, 0).unwrap();
    let debug = format!("{client:?}");
    assert!(!debug.contains("payload-secret")); assert!(!debug.contains("7061796c6f61642d736563726574"));
    assert!(client.socket_fd().is_some());
    drop(client); server.set_nonblocking(true).unwrap();
    assert_eq!(server.read(&mut [0_u8; 1]).unwrap(), 0);
}

#[test]
fn authenticated_native_client_preserves_state_and_still_requires_real_review() {
    let (fixture, wire) = Fixture::new(2, 2);
    let files = fixture.root.files(); let before = fixture.supervisor.host().unwrap().inspect();
    let reads = fixture.source.status().read_attempts;
    let (socket, accepted) = UnixStream::pair().unwrap();
    let server_policy = peer_policy(&socket); let client_policy = peer_policy(&accepted);
    let mut server = FilePrecheckPeerSession::new(client_policy, wire, MAX_PRECHECK_FRAME_BYTES, 1).unwrap();
    server.attach(accepted).unwrap();
    let candidate = request(7, b"ok");
    let mut client = PrecheckPeerClient::connect(socket, server_policy, candidate.clone(), 0).unwrap();
    let budget = DriveBudget { read_bytes: 1, write_bytes: 1, frames: 1, io_calls: 1 };
    for _ in 0..4096 {
        let report = client.drive(budget).unwrap();
        assert!(report.status.failure.is_none());
        if report.status.closed() { break; }
        server.drive(budget).unwrap();
    }
    assert_eq!(client.status().phase, PrecheckClientPhase::Complete);
    assert_eq!(client.take_response(), Some(observation(7, 0)));
    assert!(client.take_response().is_none()); assert!(client.socket_fd().is_none());
    assert_eq!(client.drive(DriveBudget::default()).unwrap().progress, DriveProgress::default());
    assert_eq!(fixture.remaining_queries(), 1);
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
    assert_eq!(fixture.source.status().read_attempts, reads);
    assert_eq!(fixture.supervisor.host().unwrap().retained_requests(), 0);
    // The precheck label is still a new ordinary request, not a reserved ticket.
    let ticket = fixture.actor.submit(7, &candidate.proposal).unwrap();
    assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 7 });
    let host = fixture.supervisor.host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!((host.inspect().executions, host.inspect().control.ledger.reserved), (0, 0));
}

#[test]
fn losing_the_reply_never_refunds_server_work_or_automatically_retries() {
    let (fixture, wire) = Fixture::new(2, 1);
    let (socket, accepted) = UnixStream::pair().unwrap();
    let policy = peer_policy(&socket);
    let mut server = FilePrecheckPeerSession::new(peer_policy(&accepted), wire,
        MAX_PRECHECK_FRAME_BYTES, 2).unwrap();
    server.attach(accepted).unwrap();
    let mut client = PrecheckPeerClient::connect(socket, policy, request(7, b"ok"), 0).unwrap();
    client.drive(DriveBudget::default()).unwrap();
    assert_eq!(client.status().phase, PrecheckClientPhase::Reading);
    // One read attempt may consume a short socket prefix; its local feed still
    // runs, but the exhausted I/O allowance cannot write the resulting reply.
    for _ in 0..1024 {
        let report = server.drive(one_call()).unwrap();
        assert_eq!(report.progress.written_bytes, 0);
        if fixture.remaining_queries() == 0 {
            assert_eq!(report.progress.frames, 1);
            assert!(report.status.pending_output_bytes > 0);
            break;
        }
    }
    assert_eq!(fixture.remaining_queries(), 0);
    assert!(client.take_response().is_none());
    drop(client);
    assert!(server.disconnect());
    // This explicit new query is not a retry hidden inside the client. The
    // original native allowance remains exhausted after the lost response.
    let (socket, accepted) = UnixStream::pair().unwrap();
    server.attach(accepted).unwrap();
    let mut second = PrecheckPeerClient::connect(socket, policy, request(7, b"ok"), 0).unwrap();
    for _ in 0..16 {
        second.drive(DriveBudget::default()).unwrap();
        if second.status().closed() { break; }
        server.drive(DriveBudget::default()).unwrap();
    }
    assert_eq!(second.status().phase, PrecheckClientPhase::Complete);
    assert_eq!(second.take_response(), Some(PrecheckWireResponse {
        query: Some(7), result: Err(PrecheckWireError::Capacity),
    }));
    assert_eq!(fixture.remaining_queries(), 0);
    assert_eq!(fixture.supervisor.host().unwrap().retained_requests(), 0);
}
