//! Scripted I/O faults, not host isolation or trained-model tests.
#[path = "support/helper_workers.rs"]
mod support;

use support::Fixture;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_workers::{HelperFailure, HelperLimits, HelperPhase, HelperRound};
use fa_reference::action::consequence::oversight::helper_workers::io::{HelperConnection, IoProgress, WorkerIoError};
use fa_reference::action::consequence::oversight::helper_workers::wire::{decode_request, encode_request};
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::rc::Rc;

#[derive(Default)]
struct State {
    incoming: VecDeque<u8>, outgoing: Vec<u8>, read_calls: usize, write_calls: usize,
    flush_blocks: usize, read_limit: usize, write_limit: usize, eof: bool,
    write_failure: Option<io::ErrorKind>,
}
#[derive(Clone)]
struct Duplex(Rc<RefCell<State>>);
impl Duplex {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(State { read_limit: 1, write_limit: 1, ..State::default() })))
    }
    fn feed(&self, bytes: &[u8]) { self.0.borrow_mut().incoming.extend(bytes); }
}
impl Read for Duplex {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let mut s = self.0.borrow_mut(); s.read_calls += 1;
        if s.incoming.is_empty() {
            return if s.eof { Ok(0) } else { Err(io::ErrorKind::WouldBlock.into()) };
        }
        let count = output.len().min(s.read_limit).min(s.incoming.len());
        for byte in &mut output[..count] { *byte = s.incoming.pop_front().unwrap(); }
        Ok(count)
    }
}
impl Write for Duplex {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let mut s = self.0.borrow_mut(); s.write_calls += 1;
        if let Some(kind) = s.write_failure.take() { return Err(kind.into()); }
        let count = input.len().min(s.write_limit);
        s.outgoing.extend_from_slice(&input[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut s = self.0.borrow_mut();
        if s.flush_blocks > 0 { s.flush_blocks -= 1; return Err(io::ErrorKind::WouldBlock.into()); }
        Ok(())
    }
}

#[test]
fn bytewise_io_preserves_exact_request_and_waits_for_the_complete_commit_barrier() {
    let mut f = Fixture::new();
    let (mut round, mut ports) = HelperRound::new(f.start(11), HelperLimits::default()).unwrap();
    let alpha = ports.remove("alpha").unwrap();
    let expected = encode_request(&alpha).unwrap();
    let packet = decode_request(&expected).unwrap();
    let peer = Duplex::new();
    let mut connection = HelperConnection::new(alpha, peer.clone()).unwrap();
    for _ in 0..expected.len() { connection.step().unwrap(); }
    assert_eq!(peer.0.borrow().outgoing, expected);
    assert_eq!(connection.step().unwrap(), IoProgress::Blocked);
    peer.feed(&packet.commitment_frame(Verdict::Allow, b"salt").unwrap());
    for _ in 0..9 { connection.step().unwrap(); }
    round.advance(ElapsedTick(1)).unwrap();
    assert_eq!(connection.step().unwrap(), IoProgress::AwaitCoordinator);
    assert_eq!(peer.0.borrow().outgoing.len(), expected.len());
    let beta = &ports["beta"];
    beta.submit_commitment(beta.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    connection.step().unwrap();
    assert_eq!(&peer.0.borrow().outgoing[expected.len()..], b"R");
    let reveal = packet.reveal_frame(Verdict::Allow, b"salt").unwrap();
    peer.feed(&reveal);
    for _ in 0..reveal.len() { connection.step().unwrap(); }
    beta.reveal(Verdict::Allow, b"salt").unwrap();
    let review = round.finish(ElapsedTick(1)).unwrap();
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert_eq!(connection.step().unwrap(), IoProgress::Complete);
    assert!(connection.failure().is_none());
    assert!(peer.0.borrow().incoming.is_empty());
}

#[test]
fn flush_backpressure_cannot_resend_request_bytes_or_accept_a_commit_early() {
    let mut f = Fixture::new();
    let (mut round, mut ports) = HelperRound::new(f.start(11), HelperLimits::default()).unwrap();
    let alpha = ports.remove("alpha").unwrap();
    let expected = encode_request(&alpha).unwrap();
    let packet = decode_request(&expected).unwrap();
    let peer = Duplex::new();
    peer.0.borrow_mut().flush_blocks = 2;
    peer.feed(&packet.commitment_frame(Verdict::Allow, b"salt").unwrap());
    let mut connection = HelperConnection::new(alpha, peer.clone()).unwrap();
    for _ in 0..expected.len() { connection.step().unwrap(); }
    assert_eq!(peer.0.borrow().read_calls, 0);
    assert_eq!(connection.step().unwrap(), IoProgress::Blocked);
    assert_eq!(peer.0.borrow().read_calls, 0);
    assert_eq!(peer.0.borrow().write_calls, expected.len());
    round.advance(ElapsedTick(1)).unwrap();
    assert!(!round.statuses()["alpha"].committed);
    connection.step().unwrap();
    assert_eq!(peer.0.borrow().write_calls, expected.len());
    for _ in 0..9 { connection.step().unwrap(); }
    round.advance(ElapsedTick(1)).unwrap();
    assert!(round.statuses()["alpha"].committed);
    assert_eq!(peer.0.borrow().outgoing, expected);
}

#[test]
fn malformed_commitment_is_terminal_for_that_worker_without_poisoning_another() {
    let mut f = Fixture::new();
    let (mut round, mut ports) = HelperRound::new(f.start(11), HelperLimits::default()).unwrap();
    let alpha = ports.remove("alpha").unwrap();
    let length = encode_request(&alpha).unwrap().len();
    let peer = Duplex::new(); peer.feed(b"P12345678");
    let mut connection = HelperConnection::new(alpha, peer).unwrap();
    for _ in 0..length { connection.step().unwrap(); }
    for _ in 0..8 { connection.step().unwrap(); }
    assert_eq!(connection.step(), Err(WorkerIoError::Protocol(Error::InvalidInput)));
    assert_eq!(connection.step().unwrap(), IoProgress::Closed);
    assert_eq!(round.statuses()["alpha"].failure, Some(HelperFailure::Rejected(Error::InvalidInput)));
    let beta = &ports["beta"];
    beta.submit_commitment(beta.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    round.advance(ElapsedTick(5)).unwrap();
    beta.reveal(Verdict::Allow, b"salt").unwrap();
    round.advance(ElapsedTick(6)).unwrap();
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned()]);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
}

#[test]
fn truncated_reveal_and_oversized_salt_do_not_emit_partial_verdicts() {
    for oversized in [false, true] {
        let mut f = Fixture::new();
        let (mut round, mut ports) = HelperRound::new(f.start(11), HelperLimits::default()).unwrap();
        let alpha = ports.remove("alpha").unwrap();
        let encoded = encode_request(&alpha).unwrap();
        let packet = decode_request(&encoded).unwrap();
        let peer = Duplex::new();
        let mut connection = HelperConnection::new(alpha, peer.clone()).unwrap();
        for _ in 0..encoded.len() { connection.step().unwrap(); }
        peer.feed(&packet.commitment_frame(Verdict::Allow, b"salt").unwrap());
        for _ in 0..9 { connection.step().unwrap(); }
        let beta = &ports["beta"];
        beta.submit_commitment(beta.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
        round.advance(ElapsedTick(1)).unwrap();
        connection.step().unwrap();
        if oversized { peer.feed(b"R\x01\xff\xffignored"); }
        else { peer.feed(b"R\x01\x00\x04sa"); peer.0.borrow_mut().eof = true; }
        let mut failed = false;
        for _ in 0..8 {
            if connection.step().is_err() { failed = true; break; }
        }
        assert!(failed);
        assert!(!round.statuses()["alpha"].revealed);
        if oversized { assert_eq!(peer.0.borrow().incoming.len(), 7); }
        beta.reveal(Verdict::Allow, b"salt").unwrap();
        round.advance(ElapsedTick(2)).unwrap();
        let review = round.finish(ElapsedTick(10)).unwrap();
        assert_eq!(review.missing(), &["alpha".to_owned()]);
        assert_ne!(review.decision().consequence, Consequence::Continue);
    }
}

#[test]
fn interrupted_write_resumes_but_broken_pipe_closes_without_a_retry() {
    let mut f = Fixture::new();
    let (round, mut ports) = HelperRound::new(f.start(11), HelperLimits::default()).unwrap();
    let peer = Duplex::new();
    peer.0.borrow_mut().write_failure = Some(io::ErrorKind::Interrupted);
    let mut connection = HelperConnection::new(ports.remove("alpha").unwrap(), peer.clone()).unwrap();
    assert_eq!(connection.step().unwrap(), IoProgress::Blocked);
    assert!(peer.0.borrow().outgoing.is_empty());
    connection.step().unwrap();
    assert_eq!(peer.0.borrow().outgoing, b"F");
    peer.0.borrow_mut().write_failure = Some(io::ErrorKind::BrokenPipe);
    assert_eq!(connection.step(), Err(WorkerIoError::Io(io::ErrorKind::BrokenPipe)));
    let calls = peer.0.borrow().write_calls;
    assert_eq!(connection.step().unwrap(), IoProgress::Closed);
    assert_eq!(peer.0.borrow().write_calls, calls);
    assert_eq!(round.statuses()["alpha"].phase, HelperPhase::Failed);
    assert_eq!(f.broker.inspect().ledger.available, 100);
}
