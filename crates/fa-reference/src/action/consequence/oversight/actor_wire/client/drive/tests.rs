//! Differential tests execute the unchanged one-step client as the oracle.
use super::*;
use super::super::{ActorClientState, ClientSessionLimits, WireResponse};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::oversight::actor::{ActorProposal, Knowledge};
use std::{cell::RefCell, io::{self, Cursor}, rc::Rc};

#[derive(Default, Debug, PartialEq, Eq)]
struct Trace { calls: usize, writes: Vec<u8>, reads: usize, flushes: usize }
struct Peer {
    trace: Rc<RefCell<Trace>>, response: Cursor<Vec<u8>>, chunk: usize,
    transient: Option<(usize, io::ErrorKind)>, panic_at: Option<usize>,
}
impl Peer {
    fn new(chunk: usize) -> Self {
        let mut response = response().encode(); response.push(b'\n');
        Self { trace: Rc::new(RefCell::new(Trace::default())), response: Cursor::new(response),
            chunk, transient: None, panic_at: None }
    }
    fn enter(&mut self) -> io::Result<()> {
        let call = { let mut t = self.trace.borrow_mut(); t.calls += 1; t.calls };
        assert_ne!(self.panic_at, Some(call), "deliberate bounded peer interruption");
        if let Some((at, kind)) = self.transient {
            if at == call { self.transient = None; return Err(kind.into()); }
        }
        Ok(())
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.enter()?; let n = bytes.len().min(self.chunk);
        self.trace.borrow_mut().writes.extend_from_slice(&bytes[..n]); Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.trace.borrow_mut().flushes += 1; self.enter()
    }
}
impl Read for Peer {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.enter()?; let n = bytes.len().min(self.chunk);
        let n = self.response.read(&mut bytes[..n])?; self.trace.borrow_mut().reads += n; Ok(n)
    }
}
fn response() -> WireResponse { WireResponse { request: Some(7), result: Ok(Knowledge::Pending { request: 7 }) } }
fn proposal(size: usize) -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 1,
        expected_version: 3, generation: 4 }, payload: vec![b'x'; size],
        expected_policy_epoch: 0, deadline: ElapsedTick(50), units: size as u64 }
}
fn client(peer: Peer, size: usize, limits: ClientSessionLimits) -> ActorClient<Peer> {
    let mut client = ActorClientState::new(limits).unwrap().connect(peer);
    client.submit(7, &proposal(size)).unwrap(); client
}
fn within(actual: ClientIoWork, ceiling: ClientIoWork) {
    assert_eq!(actual.exchanges, ceiling.exchanges);
    assert!(actual.calls <= ceiling.calls);
    assert!(actual.written_bytes <= ceiling.written_bytes);
    assert!(actual.read_bytes <= ceiling.read_bytes);
}
fn oracle(client: &mut ActorClient<Peer>, steps: u64) -> Result<ClientProgress, ClientError> {
    for n in 1..=steps {
        let progress = client.step()?;
        if progress != ClientProgress::Progress || n == steps { return Ok(progress); }
    }
    unreachable!()
}

#[test]
fn quanta_match_exact_original_bytes_responses_and_lifetime_costs() {
    for size in [1, 9000] {
        for chunk in [17, CLIENT_IO_CHUNK] {
            for steps in [1, 3, 16, MAX_CLIENT_DRIVE_STEPS] {
                let a = Peer::new(chunk); let trace_a = Rc::clone(&a.trace);
                let b = Peer::new(chunk); let trace_b = Rc::clone(&b.trace);
                let mut batched = client(a, size, ClientSessionLimits::default());
                let mut original = client(b, size, ClientSessionLimits::default());
                let budget = ClientDriveBudget::new(steps).unwrap(); let mut done = false;
                for _ in 0..4096 {
                    let before = batched.state().work();
                    let ceiling = batched.drive_ceiling(budget).unwrap().unwrap();
                    let report = batched.drive(budget).unwrap();
                    assert_eq!(report.progress, oracle(&mut original, steps).unwrap());
                    assert!(report.steps <= steps); assert_eq!(report.work.exchanges, 0);
                    assert_eq!(report.work.calls, batched.state().work().calls - before.calls);
                    within(batched.state().work(), ceiling);
                    assert_eq!(batched.state().work(), original.state().work());
                    assert_eq!(*trace_a.borrow(), *trace_b.borrow());
                    if report.progress == ClientProgress::Response(response()) { done = true; break; }
                }
                assert!(done);
                let before = batched.state().work(); let calls = trace_a.borrow().calls;
                let idle = batched.drive(budget).unwrap();
                assert_eq!(idle.steps, 0); assert_eq!(idle.progress, ClientProgress::Complete);
                assert_eq!(batched.state().work(), before); assert_eq!(trace_a.borrow().calls, calls);
                assert_eq!(batched.into_state().encode_recovery(b"same-server").unwrap(),
                    original.into_state().encode_recovery(b"same-server").unwrap());
            }
        }
    }
}

#[test]
fn transient_write_flush_and_read_stop_immediately_instead_of_spinning() {
    for call in [1, 2, 3] {
        for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted] {
            let mut peer = Peer::new(CLIENT_IO_CHUNK); peer.transient = Some((call, kind));
            let trace = Rc::clone(&peer.trace);
            let mut c = client(peer, 1, ClientSessionLimits::default());
            let report = c.drive(ClientDriveBudget::default()).unwrap();
            assert_eq!(report.progress, ClientProgress::Blocked);
            assert_eq!(trace.borrow().calls, call); assert_eq!(c.state().work().calls, call as u64);
            assert!(c.state().last_response(7).is_none());
            if call <= 2 { assert_eq!(trace.borrow().reads, 0); }
            assert_eq!(c.drive(ClientDriveBudget::default()).unwrap().progress, ClientProgress::Response(response()));
            assert_eq!(c.state().request_count(), 1);
        }
    }
}

#[test]
fn exhausted_lifetime_budget_remains_terminal_after_a_partial_quantum() {
    for calls in [1, 2, 4] {
        let mut limits = ClientSessionLimits::default(); limits.io.calls = calls;
        let peer = Peer::new(1); let trace = Rc::clone(&peer.trace);
        let mut c = client(peer, 1, limits);
        let ceiling = c.drive_ceiling(ClientDriveBudget::default()).unwrap().unwrap();
        assert_eq!(c.drive(ClientDriveBudget::default()), Err(ClientError::Limit));
        within(c.state().work(), ceiling);
        assert_eq!(trace.borrow().calls, calls as usize);
        let failure = c.state().last_failure().unwrap(); assert!(failure.request_may_have_reached_peer);
        assert_eq!(failure.error, ClientError::Limit);
        assert_eq!(c.drive(ClientDriveBudget::default()), Err(ClientError::Disconnected));
        assert_eq!(trace.borrow().calls, calls as usize);
        let state = c.into_state(); let work = state.work();
        let mut retry = state.connect(Peer::new(1024));
        assert_eq!(retry.retry_submission(7), Err(ClientError::Limit));
        assert_eq!(retry.state().work(), work);
    }
}

#[test]
fn quantum_validation_and_malformed_response_do_not_create_retries() {
    assert_eq!(ClientDriveBudget::new(0), Err(ClientError::Limit));
    assert_eq!(ClientDriveBudget::new(MAX_CLIENT_DRIVE_STEPS + 1), Err(ClientError::Limit));
    let mut peer = Peer::new(CLIENT_IO_CHUNK);
    peer.response = Cursor::new(b"not a response\n".to_vec()); let trace = Rc::clone(&peer.trace);
    let mut c = client(peer, 9000, ClientSessionLimits::default());
    let ceiling = c.drive_ceiling(ClientDriveBudget::default()).unwrap().unwrap();
    assert!(matches!(c.drive(ClientDriveBudget::default()), Err(ClientError::Response(_))));
    within(c.state().work(), ceiling);
    let calls = trace.borrow().calls;
    assert!(c.state().last_response(7).is_none()); assert_eq!(c.state().request_count(), 1);
    assert_eq!(c.drive(ClientDriveBudget::default()), Err(ClientError::Disconnected));
    assert_eq!(trace.borrow().calls, calls);
}

#[test]
fn caught_io_unwind_cannot_reenter_any_write_flush_or_read() {
    for call in [1, 2, 3] {
        let mut peer = Peer::new(CLIENT_IO_CHUNK); peer.panic_at = Some(call);
        let trace = Rc::clone(&peer.trace); let mut c = client(peer, 1, ClientSessionLimits::default());
        let ceiling = c.drive_ceiling(ClientDriveBudget::default()).unwrap().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.drive(ClientDriveBudget::default())));
        assert!(result.is_err()); within(c.state().work(), ceiling);
        assert_eq!(c.drive_ceiling(ClientDriveBudget::default()), Err(ClientError::InterruptedOperation));
        assert_eq!(c.drive(ClientDriveBudget::default()), Err(ClientError::InterruptedOperation));
        assert_eq!(trace.borrow().calls, call);
        assert!(c.state().interrupted_command().is_some());
    }
}

#[test]
fn reservations_cover_partially_sent_and_partially_read_exchanges() {
    for skip in [0, 1, 2, 10] {
        for steps in [1, 2, 7, 64] {
            let mut c = client(Peer::new(7), 100, ClientSessionLimits::default());
            for _ in 0..skip { assert_eq!(c.step().unwrap(), ClientProgress::Progress); }
            let budget = ClientDriveBudget::new(steps).unwrap();
            let ceiling = c.drive_ceiling(budget).unwrap().unwrap();
            let report = c.drive(budget).unwrap(); within(c.state().work(), ceiling);
            assert!(report.steps <= steps);
        }
    }
    let mut c = client(Peer::new(31), 1, ClientSessionLimits::default());
    while c.phase() == Some(ClientPhase::Sending) { c.step().unwrap(); }
    assert_eq!(c.step().unwrap(), ClientProgress::Progress);
    for steps in [1, 2] {
        let budget = ClientDriveBudget::new(steps).unwrap();
        let before = c.state().work(); let ceiling = c.drive_ceiling(budget).unwrap().unwrap();
        let _ = c.drive(budget).unwrap(); within(c.state().work(), ceiling);
        assert_eq!(ceiling.written_bytes, before.written_bytes);
        if c.phase().is_none() { break; }
    }
}
