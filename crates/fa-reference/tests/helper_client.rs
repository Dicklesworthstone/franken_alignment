//! Scripted I/O tests inject specific errors; real socket composition is separate.
#[path = "support/helper_workers.rs"]
mod support;

use support::Fixture;
use fa_reference::action::consequence::oversight::helper_workers::{HelperLimits, HelperRound};
use fa_reference::action::consequence::oversight::helper_workers::io::WorkerIoError;
use fa_reference::action::consequence::oversight::helper_workers::wire::{WorkerInput, MAX_HELPER_FRAME_BYTES, decode_request, encode_request};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, ClientProgress, HelperClient};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::rc::Rc;

struct State {
    incoming: VecDeque<u8>, outgoing: Vec<u8>, chunk: usize, eof: bool,
    read_error: Option<io::ErrorKind>, write_error: Option<io::ErrorKind>, flush_error: Option<io::ErrorKind>,
    reads: usize, writes: usize, flushes: usize,
}
struct Scripted(Rc<RefCell<State>>);
impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = self.0.borrow_mut(); s.reads += 1;
        if let Some(error) = s.read_error.take() { return Err(error.into()); }
        if s.incoming.is_empty() { return if s.eof { Ok(0) } else { Err(io::ErrorKind::WouldBlock.into()) }; }
        let n = buf.len().min(s.chunk).min(s.incoming.len());
        for byte in &mut buf[..n] { *byte = s.incoming.pop_front().unwrap(); }
        Ok(n)
    }
}
impl Write for Scripted {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut s = self.0.borrow_mut(); s.writes += 1;
        if let Some(error) = s.write_error.take() { return Err(error.into()); }
        let n = buf.len().min(s.chunk); s.outgoing.extend_from_slice(&buf[..n]); Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut s = self.0.borrow_mut(); s.flushes += 1;
        if let Some(error) = s.flush_error.take() { return Err(error.into()); } Ok(())
    }
}
fn request() -> (Vec<u8>, WorkerInput) {
    let mut f = Fixture::new();
    let (_round, ports) = HelperRound::new(f.start(7), HelperLimits::default()).unwrap();
    let bytes = encode_request(&ports["alpha"]).unwrap();
    let input = decode_request(&bytes).unwrap();
    (bytes, input)
}
fn client(bytes: Vec<u8>, profile: InputProfileBinding, chunk: usize) -> (HelperClient<Scripted>, Rc<RefCell<State>>) {
    let s = Rc::new(RefCell::new(State { incoming: bytes.into(), outgoing: vec![], chunk, eof: false,
        read_error: None, write_error: None, flush_error: None, reads: 0, writes: 0, flushes: 0 }));
    (HelperClient::new(Scripted(Rc::clone(&s)), profile).unwrap(), s)
}
fn until(client: &mut HelperClient<Scripted>, phase: ClientPhase) {
    for _ in 0..100_000 {
        if client.phase() == phase { return; }
        client.step().unwrap();
    }
    panic!("bounded scripted client failed to reach {phase:?}");
}
fn failure(client: &mut HelperClient<Scripted>) -> WorkerIoError {
    for _ in 0..100_000 { if let Err(error) = client.step() { return error; } }
    panic!("scripted failure did not occur within its step budget");
}

#[test]
fn fragmented_request_requires_inference_and_separate_reveal_release() {
    let (mut bytes, input) = request(); bytes.push(b'R');
    let (mut c, state) = client(bytes, input.actual_input().input_profile().clone(), 1);
    until(&mut c, ClientPhase::NeedsInference);
    assert_eq!(c.input().unwrap(), &input);
    assert_eq!(state.borrow().incoming.len(), 1);
    assert_eq!(c.step(), Ok(ClientProgress::NeedsInference));
    assert!(state.borrow().outgoing.is_empty());
    c.respond(Verdict::Allow, b"private-salt").unwrap();
    assert!(state.borrow().outgoing.is_empty());
    until(&mut c, ClientPhase::AwaitingReveal);
    let expected_commit = input.commitment_frame(Verdict::Allow, b"private-salt").unwrap();
    assert_eq!(state.borrow().outgoing, expected_commit);
    assert_eq!(state.borrow().incoming.len(), 1);
    c.step().unwrap();
    until(&mut c, ClientPhase::ReplySent);
    let mut expected = expected_commit.to_vec();
    expected.extend(input.reveal_frame(Verdict::Allow, b"private-salt").unwrap());
    assert_eq!(state.borrow().outgoing, expected);
    let calls = (state.borrow().reads, state.borrow().writes, state.borrow().flushes);
    assert_eq!(c.step(), Ok(ClientProgress::ReplySent));
    assert_eq!((state.borrow().reads, state.borrow().writes, state.borrow().flushes), calls);
    assert!(!format!("{c:?}").contains("private-salt"));
}

#[test]
fn bad_salt_is_atomic_but_a_frozen_answer_cannot_be_changed() {
    let (bytes, input) = request();
    let (mut c, state) = client(bytes, input.actual_input().input_profile().clone(), usize::MAX);
    until(&mut c, ClientPhase::NeedsInference);
    assert_eq!(c.respond(Verdict::Allow, &vec![0; input.salt_limit() + 1]), Err(Error::Limit));
    assert_eq!(c.phase(), ClientPhase::NeedsInference);
    assert!(state.borrow().outgoing.is_empty());
    c.respond(Verdict::Hold, b"fixed").unwrap();
    assert_eq!(c.respond(Verdict::Allow, b"replacement"), Err(Error::WrongState));
    until(&mut c, ClientPhase::AwaitingReveal);
    assert_eq!(state.borrow().outgoing, input.commitment_frame(Verdict::Hold, b"fixed").unwrap());
}

#[test]
fn blocked_flush_and_interruption_never_resend_commitment_bytes() {
    let (bytes, input) = request();
    let (mut c, state) = client(bytes, input.actual_input().input_profile().clone(), usize::MAX);
    state.borrow_mut().read_error = Some(io::ErrorKind::Interrupted);
    assert_eq!(c.step(), Ok(ClientProgress::Blocked));
    until(&mut c, ClientPhase::NeedsInference);
    c.respond(Verdict::Allow, b"x").unwrap();
    state.borrow_mut().flush_error = Some(io::ErrorKind::WouldBlock);
    assert_eq!(c.step(), Ok(ClientProgress::Blocked));
    assert_eq!(state.borrow().outgoing.len(), 9);
    assert_eq!(state.borrow().writes, 1);
    c.step().unwrap();
    assert_eq!(state.borrow().outgoing.len(), 9);
    assert_eq!(state.borrow().writes, 1);
    assert_eq!(state.borrow().flushes, 2);
    assert_eq!(c.phase(), ClientPhase::AwaitingReveal);
}

#[test]
fn every_profile_field_is_required_before_exposing_input() {
    let (bytes, input) = request();
    for changed in 0..5 {
        let mut profile = input.actual_input().input_profile().clone();
        match changed { 0 => profile.profile_id += 1, 1 => profile.model_epoch += 1,
            2 => profile.tokenizer_epoch += 1, 3 => profile.policy_epoch += 1,
            _ => profile.profile_bytes.push(0) }
        let (mut c, state) = client(bytes.clone(), profile, usize::MAX);
        assert_eq!(failure(&mut c), WorkerIoError::Protocol(Error::Binding));
        assert!(c.input().is_none());
        assert!(state.borrow().outgoing.is_empty());
        assert_eq!(c.respond(Verdict::Allow, b"x"), Err(Error::WrongState));
        let reads = state.borrow().reads;
        assert_eq!(c.step(), Err(WorkerIoError::Protocol(Error::Binding)));
        assert_eq!(state.borrow().reads, reads);
    }
}

#[test]
fn malformed_size_and_truncated_job_never_expose_partial_input() {
    let (bytes, input) = request();
    let profile = input.actual_input().input_profile().clone();
    let mut oversized = b"FAHW1".to_vec();
    oversized.extend_from_slice(&(MAX_HELPER_FRAME_BYTES as u32).to_be_bytes());
    for (data, expected) in [(oversized, WorkerIoError::Protocol(Error::Limit)),
        (bytes[..bytes.len() - 1].to_vec(), WorkerIoError::Io(io::ErrorKind::UnexpectedEof))] {
        let (mut c, state) = client(data, profile.clone(), usize::MAX);
        state.borrow_mut().eof = true;
        assert_eq!(failure(&mut c), expected);
        assert!(c.input().is_none()); assert!(state.borrow().outgoing.is_empty());
        let reads = state.borrow().reads;
        assert_eq!(c.step(), Err(expected)); assert_eq!(state.borrow().reads, reads);
    }
}

#[test]
fn invalid_release_and_write_failure_do_not_reroll_or_disclose_a_reveal() {
    let (bytes, input) = request();
    let (mut c, state) = client(bytes.clone(), input.actual_input().input_profile().clone(), usize::MAX);
    until(&mut c, ClientPhase::NeedsInference); c.respond(Verdict::Allow, b"secret").unwrap();
    until(&mut c, ClientPhase::AwaitingReveal);
    state.borrow_mut().incoming.push_back(b'X');
    assert_eq!(c.step(), Err(WorkerIoError::Protocol(Error::InvalidInput)));
    assert_eq!(state.borrow().outgoing.len(), 9);
    assert_eq!(c.respond(Verdict::Deny, b"new"), Err(Error::WrongState));
    let (mut c, state) = client(bytes, input.actual_input().input_profile().clone(), 1);
    until(&mut c, ClientPhase::NeedsInference); c.respond(Verdict::Allow, b"secret").unwrap();
    c.step().unwrap(); state.borrow_mut().write_error = Some(io::ErrorKind::BrokenPipe);
    assert_eq!(c.step(), Err(WorkerIoError::Io(io::ErrorKind::BrokenPipe)));
    assert_eq!(state.borrow().outgoing.len(), 1);
    let calls = state.borrow().writes;
    assert!(c.step().is_err()); assert_eq!(state.borrow().writes, calls);
}
