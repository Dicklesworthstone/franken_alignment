//! Original client protocol controls; saved bytes confer no server authority.
use super::*;
use super::super::super::super::super::actor::{BasisSource, UnknownReason};
use crate::action::{ElapsedTick, ResolvedTarget};
use std::io::{self, Cursor, Read, Write};

const BINDING: &[u8] = b"tenant-1/domain-9/generation-4/raw-submit-v1";
struct Peer { input: Cursor<Vec<u8>>, written: Vec<u8> }
impl Peer {
    fn new(response: WireResponse) -> Self {
        let mut bytes = response.encode(); bytes.push(b'\n');
        Self { input: Cursor::new(bytes), written: Vec::new() }
    }
}
impl Read for Peer { fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> { self.input.read(bytes) } }
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.written.extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn proposal() -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 1,
        expected_version: 3, generation: 4 }, payload: b"exact\0UTF8 \xf0\x9f\x8c\x8e".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(50), units: 32 }
}
fn known() -> WireResponse { WireResponse { request: Some(u64::MAX), result: Ok(Knowledge::Known {
    value: ActorOutcome::Executed, basis: ActorBasis { request: u64::MAX, generation: 4, source: BasisSource::ControlLedger } }) } }
fn finish(client: &mut ActorClient<Peer>) -> Result<WireResponse, ClientError> {
    for _ in 0..8 { if let ClientProgress::Response(response) = client.step()? { return Ok(response); } }
    panic!("bounded scripted peer never answered")
}
fn roundtrip(state: &ActorClientState) -> ActorClientState {
    ActorClientState::decode_recovery(&state.encode_recovery(BINDING).unwrap(), BINDING,
        state.limits(), state.work(), state.request_count()).unwrap()
}
fn completed() -> ActorClientState {
    let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect(Peer::new(known()));
    client.submit(u64::MAX, &proposal()).unwrap(); assert_eq!(finish(&mut client).unwrap(), known());
    client.into_state()
}

#[test]
fn terminal_history_survives_restart_but_never_restores_numeric_ticket_visibility() {
    let state = completed(); let work = state.work(); let restored = roundtrip(&state);
    assert_eq!(restored.work(), work); assert_eq!(restored.last_response(u64::MAX), Some(&known()));
    assert_eq!(restored.original_proposal(u64::MAX), Some(&proposal()));
    let mut client = restored.connect(Peer::new(known()));
    assert_eq!(client.poll(u64::MAX), Err(ClientError::TicketUnavailable));
    assert_eq!(client.cancel(u64::MAX), Err(ClientError::TicketUnavailable));
    assert_eq!(client.state().work(), work);
    let mut changed = proposal(); changed.deadline.0 += 1;
    assert_eq!(client.submit(u64::MAX, &changed), Err(ClientError::Command(WireError::IdempotencyConflict)));
    client.retry_submission(u64::MAX).unwrap(); assert_eq!(finish(&mut client).unwrap(), known());
    assert_eq!(client.state().request_count(), 1);
    assert_eq!(client.state().work().exchanges, work.exchanges + 1);
}

#[test]
fn later_unknown_observation_does_not_erase_the_original_terminal_tombstone() {
    let unknown = WireResponse { request: Some(u64::MAX), result: Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }) };
    let mut client = roundtrip(&completed()).connect(Peer::new(unknown.clone()));
    client.retry_submission(u64::MAX).unwrap(); assert_eq!(finish(&mut client).unwrap(), unknown);
    let state = roundtrip(&client.into_state());
    assert_eq!(state.last_response(u64::MAX), Some(&unknown));
    let mut pending = state.connect(Peer::new(WireResponse { request: Some(u64::MAX), result: Ok(Knowledge::Pending { request: u64::MAX }) }));
    pending.retry_submission(u64::MAX).unwrap();
    assert_eq!(finish(&mut pending), Err(ClientError::Response(ResponseError::Binding)));
    let failed = pending.into_state(); let restored = roundtrip(&failed);
    assert_eq!(restored.last_failure(), failed.last_failure());
    assert!(restored.interrupted_command().is_some());
}

#[test]
fn partial_request_and_exact_spent_budget_remain_ambiguous_after_recovery() {
    let mut limits = ClientSessionLimits::default(); limits.io.exchanges = 1;
    let mut client = ActorClientState::new(limits).unwrap().connect(Peer::new(known()));
    client.submit(u64::MAX, &proposal()).unwrap();
    assert_eq!(client.step().unwrap(), ClientProgress::Progress); // write + flush, no response read
    let state = client.into_state(); let work = state.work(); let restored = roundtrip(&state);
    assert!(work.written_bytes > 0); assert_eq!(restored.work(), work);
    assert_eq!(restored.interrupted_command(), Some(&Command::Submit { request: u64::MAX, proposal: proposal() }));
    assert!(restored.last_response(u64::MAX).is_none());
    let mut client = restored.connect(Peer::new(known()));
    assert_eq!(client.retry_submission(u64::MAX), Err(ClientError::Limit));
    assert_eq!(client.state().work(), work);
}

#[test]
fn exact_binding_limits_floors_and_every_truncation_refuse_before_restore() {
    let state = completed(); let bytes = state.encode_recovery(BINDING).unwrap();
    for end in 0..bytes.len() {
        assert!(ActorClientState::decode_recovery(&bytes[..end], BINDING, state.limits(), ClientIoWork::default(), 0).is_err());
    }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(ActorClientState::decode_recovery(&trailing, BINDING, state.limits(), state.work(), 1).is_err());
    assert!(ActorClientState::decode_recovery(&bytes, b"other server", state.limits(), state.work(), 1).is_err());
    let mut limits = state.limits(); limits.io.calls -= 1;
    assert!(ActorClientState::decode_recovery(&bytes, BINDING, limits, state.work(), 1).is_err());
    for field in 0..4 {
        let mut floor = state.work();
        match field { 0 => floor.exchanges += 1, 1 => floor.calls += 1, 2 => floor.written_bytes += 1, _ => floor.read_bytes += 1 }
        assert!(ActorClientState::decode_recovery(&bytes, BINDING, state.limits(), floor, 1).is_err());
    }
    assert!(ActorClientState::decode_recovery(&bytes, BINDING, state.limits(), state.work(), 2).is_err());
    assert_eq!(roundtrip(&state).encode_recovery(BINDING).unwrap(), bytes);
}

#[test]
fn duplicate_keys_conflicting_tombstones_and_foreign_interrupted_requests_refuse() {
    let mut state = completed();
    state.interrupted = Some(Command::Poll { request: 7 });
    assert!(ActorClientState::decode_recovery(&state.encode_recovery(BINDING).unwrap(), BINDING,
        state.limits(), ClientIoWork::default(), 0).is_err());
    state.interrupted = None;
    state.requests.get_mut(&u64::MAX).unwrap().terminal.as_mut().unwrap().0 = ActorOutcome::Denied;
    assert!(ActorClientState::decode_recovery(&state.encode_recovery(BINDING).unwrap(), BINDING,
        state.limits(), ClientIoWork::default(), 0).is_err());
    let state = completed(); let bytes = state.encode_recovery(BINDING).unwrap();
    // Independent record boundaries: header/binding, six limits, four counters,
    // count, then exact encoded submit + optional response + terminal response.
    let count_at = 8 + 8 + BINDING.len() + 6 * 8 + 4 * 8;
    let first = count_at + 8;
    let mut r = Reader { bytes: &bytes, offset: first };
    read_command(&mut r).unwrap(); read_response(&mut r, u64::MAX).unwrap(); read_response(&mut r, u64::MAX).unwrap();
    let end = r.offset;
    let mut duplicate = bytes[..count_at].to_vec(); duplicate.extend_from_slice(&2_u64.to_be_bytes());
    duplicate.extend_from_slice(&bytes[first..end]); duplicate.extend_from_slice(&bytes[first..]);
    // Allow enough exchanges so only the duplicate/order check rejects this.
    let work_at = 8 + 8 + BINDING.len() + 6 * 8;
    duplicate[work_at..work_at + 8].copy_from_slice(&2_u64.to_be_bytes());
    assert!(ActorClientState::decode_recovery(&duplicate, BINDING, state.limits(), ClientIoWork::default(), 0).is_err());
}

#[test]
fn empty_golden_image_and_existing_failure_tags_roundtrip_without_lossy_coercion() {
    let limits = ClientSessionLimits::default(); let state = ActorClientState::new(limits).unwrap();
    let mut golden = b"FACREQ\0\x01".to_vec();
    golden.extend_from_slice(&(BINDING.len() as u64).to_be_bytes()); golden.extend_from_slice(BINDING);
    for n in [limits.requests as u64, limits.retained_payload_bytes as u64,
        limits.io.exchanges, limits.io.calls, limits.io.written_bytes, limits.io.read_bytes, 0, 0, 0, 0, 0] {
        golden.extend_from_slice(&n.to_be_bytes());
    }
    golden.extend_from_slice(&[0, 0]);
    assert_eq!(state.encode_recovery(BINDING).unwrap(), golden);
    assert_eq!(roundtrip(&state).request_count(), 0);
    for error in [ClientError::Command(WireError::Withheld), ClientError::Response(ResponseError::Binding),
        ClientError::Io(io::ErrorKind::BrokenPipe), ClientError::Io(io::ErrorKind::UnexpectedEof),
        ClientError::Limit, ClientError::Busy, ClientError::Disconnected,
        ClientError::TicketUnavailable, ClientError::InterruptedOperation] {
        let mut state = completed();
        state.failure = Some(ClientFailure { error, request: u64::MAX,
            written_bytes: 1, request_may_have_reached_peer: true });
        assert_eq!(roundtrip(&state).last_failure(), state.last_failure());
    }
}
