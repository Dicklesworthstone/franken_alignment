use super::*;
use fa_reference::action::consequence::oversight::helper_client::{ClientInterest, ClientPhase, ClientProgress};
use fa_reference::action::consequence::oversight::helper_client_drive::MAX_CLIENT_DRIVE_STEPS;
use fa_reference::action::consequence::oversight::helper_workers::io::WorkerIoError;
use fa_reference::action::consequence::oversight::sidecar::probe_helper::peer::{
    ProbeClientError, ProbeClientProgress, ProbeHelperClient,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::rc::Rc;

#[derive(Default)]
struct State {
    input: VecDeque<u8>, output: Vec<u8>, reads: usize, writes: usize,
    drops: usize, chunk: usize, interrupt_read: bool, panic_after_write: bool,
}
struct Script(Rc<RefCell<State>>);
impl Read for Script {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut(); state.reads += 1;
        if state.interrupt_read { state.interrupt_read = false; return Err(io::ErrorKind::Interrupted.into()); }
        if state.input.is_empty() { return Err(io::ErrorKind::WouldBlock.into()); }
        let count = state.input.len().min(out.len()).min(state.chunk);
        for byte in out.iter_mut().take(count) { *byte = state.input.pop_front().unwrap(); }
        Ok(count)
    }
}
impl Write for Script {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut(); state.writes += 1;
        let count = bytes.len().min(state.chunk); state.output.extend_from_slice(&bytes[..count]);
        if state.panic_after_write { state.panic_after_write = false; panic!("injected interruption after actual write"); }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
impl Drop for Script { fn drop(&mut self) { self.0.borrow_mut().drops += 1; } }
fn script(bytes: Vec<u8>, chunk: usize) -> (Script, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State { input: bytes.into(), chunk, ..State::default() }));
    (Script(Rc::clone(&state)), state)
}
fn to_phase<S: Read + Write>(client: &mut ProbeHelperClient<S>, phase: ClientPhase) {
    for _ in 0..4096 {
        if client.phase() == phase { return; }
        let report = client.drive(16).unwrap();
        assert!(report.steps <= 16); assert!(report.progress.is_ok(), "{:?}", report.progress);
    }
    panic!("did not reach {phase:?}");
}

#[test]
fn fragmented_protocol_computes_once_and_never_reveals_without_original_signal() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let salt = vec![9; 32];
    let (stream, state) = script(wire::encode_request(&port).unwrap(), 3);
    state.borrow_mut().interrupt_read = true;
    let mut client = ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), salt.clone()).unwrap();
    assert_eq!(client.drive(1).unwrap().progress, Ok(ProbeClientProgress::Protocol(ClientProgress::Blocked)));
    to_phase(&mut client, ClientPhase::NeedsInference);
    assert_eq!(client.evaluations(), 0); assert!(client.report().is_none());
    assert!(state.borrow().output.is_empty());
    let judged = client.drive(16).unwrap();
    assert_eq!(judged.steps, 1); assert_eq!(judged.evaluations, 1);
    assert_eq!(judged.progress, Ok(ProbeClientProgress::Judged(Verdict::Allow)));
    assert!(state.borrow().output.is_empty());
    to_phase(&mut client, ClientPhase::AwaitingReveal);
    let commit = input.commitment_frame(Verdict::Allow, &salt).unwrap();
    assert_eq!(state.borrow().output, commit);
    let work = client.work();
    for _ in 0..4 {
        assert_eq!(client.drive(16).unwrap().progress, Ok(ProbeClientProgress::Protocol(ClientProgress::Blocked)));
        assert_eq!(state.borrow().output, commit);
    }
    state.borrow_mut().input.push_back(wire::REVEAL_REQUEST);
    to_phase(&mut client, ClientPhase::ReplySent);
    let mut exact = commit.to_vec(); exact.extend_from_slice(&input.reveal_frame(Verdict::Allow, &salt).unwrap());
    assert_eq!(state.borrow().output, exact); assert_eq!(client.work(), work);
    assert_eq!(client.evaluations(), 1); assert_eq!(client.drive(16).unwrap().steps, 0);
    assert_eq!(client.interest(), ClientInterest::Finished);
    assert!(!client.cancel()); assert_eq!(client.phase(), ClientPhase::ReplySent);
}

#[test]
fn cancellation_closes_transport_before_inference_without_a_synthetic_abstention() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (mut round, port, _) = channel(&mut owner, &sidecar, 101);
    let (stream, state) = script(wire::encode_request(&port).unwrap(), 1024);
    let mut client = ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), vec![9; 32]).unwrap();
    to_phase(&mut client, ClientPhase::NeedsInference);
    assert!(client.cancel()); assert_eq!(state.borrow().drops, 1);
    let counts = (state.borrow().reads, state.borrow().writes);
    assert_eq!(client.step(), Err(ProbeClientError::Cancelled));
    assert_eq!(client.drive(16).unwrap().steps, 0); assert!(!client.cancel());
    assert_eq!((state.borrow().reads, state.borrow().writes), counts);
    assert_eq!(client.work(), ProbeHelperWork::default()); assert_eq!(client.evaluations(), 0);
    assert!(state.borrow().output.is_empty()); assert!(client.report().is_none());
    let result = round.finish(ElapsedTick(30)).unwrap();
    assert_eq!(result.missing(), &["reviewer".to_owned()]); assert!(result.abstained().is_empty());
}

#[test]
fn valid_but_unregistered_action_input_never_emits_a_commitment() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, first) = propose(&mut owner, 1); let (_, second) = propose(&mut owner, 2);
    let (_first_round, first_port, _) = channel(&mut owner, &first, 101);
    let (_round, port, _) = channel(&mut owner, &second, 102);
    let bytes = wire::encode_request(&port).unwrap();
    let (stream, state) = script(bytes.clone(), 1024);
    let mut client = ProbeHelperClient::new(stream, evaluator(&first, 0, &first_port), vec![9; 32]).unwrap();
    to_phase(&mut client, ClientPhase::NeedsInference);
    assert_eq!(client.step(), Err(ProbeClientError::Evaluation(Error::Binding)));
    assert_eq!(state.borrow().drops, 1); assert!(state.borrow().output.is_empty());
    assert_eq!(client.work(), ProbeHelperWork::default()); assert!(client.report().is_none());
    assert_eq!(client.drive(1).unwrap().steps, 0);
    let (stream, _) = script(bytes, 1024);
    let mut control = ProbeHelperClient::new(stream, evaluator(&second, 0, &port), vec![9; 32]).unwrap();
    to_phase(&mut control, ClientPhase::NeedsInference);
    assert_eq!(control.step(), Ok(ProbeClientProgress::Judged(Verdict::Allow)));
}

#[test]
fn invalid_reveal_signal_cannot_rewrite_or_resend_the_computed_commitment() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, _) = channel(&mut owner, &sidecar, 101);
    let (stream, state) = script(wire::encode_request(&port).unwrap(), 1024);
    let mut client = ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), vec![9; 32]).unwrap();
    to_phase(&mut client, ClientPhase::AwaitingReveal);
    let commit = state.borrow().output.clone(); assert_eq!(commit.len(), 9);
    state.borrow_mut().input.push_back(b'X');
    let expected = ProbeClientError::Protocol(WorkerIoError::Protocol(Error::InvalidInput));
    assert_eq!(client.step(), Err(expected)); assert_eq!(client.failure(), Some(expected));
    assert_eq!(state.borrow().drops, 1);
    let calls = (state.borrow().reads, state.borrow().writes);
    assert_eq!(client.step(), Err(expected));
    assert_eq!((state.borrow().reads, state.borrow().writes), calls);
    assert_eq!(state.borrow().output, commit); assert_eq!(client.evaluations(), 1);
    assert_eq!(client.report().unwrap().verdict(), Verdict::Allow);
}

#[test]
fn caught_interruption_after_partial_write_closes_connection_and_never_retries() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let salt = vec![9; 32];
    let (stream, state) = script(wire::encode_request(&port).unwrap(), 3);
    let mut client = ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), salt.clone()).unwrap();
    to_phase(&mut client, ClientPhase::SendingCommitment);
    state.borrow_mut().panic_after_write = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| client.step()));
    assert!(result.is_err()); assert_eq!(client.failure(), Some(ProbeClientError::Interrupted));
    assert_eq!(state.borrow().drops, 1);
    assert_eq!(state.borrow().output, input.commitment_frame(Verdict::Allow, &salt).unwrap()[..3]);
    let calls = (state.borrow().reads, state.borrow().writes);
    assert_eq!(client.step(), Err(ProbeClientError::Interrupted));
    assert_eq!(client.drive(16).unwrap().steps, 0); assert!(!client.cancel());
    assert_eq!((state.borrow().reads, state.borrow().writes), calls);
    assert_eq!(client.evaluations(), 1); assert_eq!(client.work().evaluated_probes, 4);
}

#[test]
fn salt_adoption_and_drive_limits_refuse_before_io_with_funded_controls() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let bytes = wire::encode_request(&port).unwrap();
    for length in [15, 257] {
        let (stream, state) = script(bytes.clone(), 1024);
        assert_eq!(ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), vec![9; length]).err(), Some(Error::Limit));
        assert_eq!(state.borrow().reads, 0); assert_eq!(state.borrow().writes, 0);
    }
    let mut used = evaluator(&sidecar, 0, &port); assert_eq!(used.evaluate(&input), Ok(Verdict::Allow));
    let (stream, state) = script(bytes.clone(), 1024);
    assert_eq!(ProbeHelperClient::new(stream, used, vec![9; 32]).err(), Some(Error::WrongState));
    assert_eq!(state.borrow().reads, 0);
    let (stream, state) = script(bytes, 1024);
    let mut client = ProbeHelperClient::new(stream, evaluator(&sidecar, 0, &port), vec![9; 32]).unwrap();
    assert_eq!(client.drive(0), Err(Error::InvalidInput));
    assert_eq!(client.drive(MAX_CLIENT_DRIVE_STEPS + 1), Err(Error::Limit));
    assert_eq!(state.borrow().reads, 0); assert_eq!(state.borrow().writes, 0);
    to_phase(&mut client, ClientPhase::SendingCommitment);
    assert_eq!(client.evaluations(), 1); assert_eq!(client.report().unwrap().verdict(), Verdict::Allow);
}

#[cfg(unix)]
fn socket_review(owner: &mut OversightBroker, sidecar: &LearnedSidecar, id: u64, mode: u8)
    -> (ObservedReview, ProbeHelperClient<std::os::unix::net::UnixStream>)
{
    use fa_reference::action::consequence::oversight::helper_workers::io::HelperConnection;
    let (mut round, port, _) = channel(owner, sidecar, id);
    let (server, client) = std::os::unix::net::UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    let evaluator = evaluator(sidecar, mode, &port);
    let mut server = HelperConnection::new(port, server).unwrap();
    let mut client = ProbeHelperClient::from_unix(client, evaluator, vec![9; 32]).unwrap();
    for _ in 0..1024 {
        server.step().unwrap(); client.drive(16).unwrap().progress.unwrap();
        match round.finish(ElapsedTick(1)) {
            Ok(review) => {
                assert_eq!(client.phase(), ClientPhase::ReplySent); assert_eq!(client.evaluations(), 1);
                assert!(review.missing().is_empty()); return (review, client);
            }
            Err(Error::Incomplete) => {}
            Err(error) => panic!("original coordinator refused: {error:?}"),
        }
    }
    panic!("bounded Unix conversation did not complete");
}

#[cfg(unix)]
#[test]
fn real_sockets_carry_computed_abstention_refinement_and_two_key_publication() {
    let (mut owner, mut endpoint, human) = owner(); step(&mut owner); step(&mut owner);
    let (action, mut sidecar) = propose(&mut owner, 1);
    let (review, first) = socket_review(&mut owner, &sidecar, 101, 1);
    assert_eq!(first.report().unwrap().verdict(), Verdict::Abstain);
    assert_eq!(first.work().refined_groups, 0);
    assert!(matches!(owner.refine_learned_sidecar(&mut sidecar, &review).unwrap(), SidecarRefinementOutcome::Refined { .. }));
    let mut allowed = None;
    for id in 102..=110 {
        let (review, client) = socket_review(&mut owner, &sidecar, id, 1);
        assert_eq!(client.report().unwrap().selected_groups(), sidecar.round().selected_groups());
        match client.report().unwrap().verdict() {
            Verdict::Allow => { allowed = Some(review); break; }
            Verdict::Abstain => assert!(matches!(owner.refine_learned_sidecar(&mut sidecar, &review).unwrap(), SidecarRefinementOutcome::Refined { .. })),
            other => panic!("unexpected computed result: {other:?}"),
        }
    }
    let input = sidecar.round().input().clone();
    owner.apply_review(allowed.expect("refined exact quiet source"), Some(&input), &snapshot()).unwrap();
    let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&key, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human = human.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&key, &human, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(endpoint.payload(), b"visible");
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatch_with_human(&key, &human, &action, Some(&input), &snapshot()).is_err());
}

#[cfg(unix)]
#[test]
fn disconnect_after_real_commit_remains_missing_at_reveal_deadline() {
    use fa_reference::action::consequence::oversight::helper_workers::io::HelperConnection;
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (mut round, port, _) = channel(&mut owner, &sidecar, 101);
    let (server, client) = std::os::unix::net::UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    let evaluator = evaluator(&sidecar, 0, &port);
    let mut server = HelperConnection::new(port, server).unwrap();
    let mut client = ProbeHelperClient::from_unix(client, evaluator, vec![9; 32]).unwrap();
    for _ in 0..1024 {
        server.step().unwrap(); client.drive(16).unwrap().progress.unwrap();
        if client.phase() == ClientPhase::AwaitingReveal { break; }
    }
    assert_eq!(client.phase(), ClientPhase::AwaitingReveal);
    // Receipt of the actual commitment uses the original server and coordinator.
    server.step().unwrap(); round.advance(ElapsedTick(1)).unwrap();
    assert!(round.statuses()["reviewer"].committed);
    assert!(!round.statuses()["reviewer"].revealed);
    assert!(client.cancel()); assert_eq!(client.evaluations(), 1);
    let review = round.finish(ElapsedTick(30)).unwrap();
    assert_eq!(review.missing(), &["reviewer".to_owned()]); assert!(review.abstained().is_empty());
    owner.observe_time(ElapsedTick(30)).unwrap();
    owner.apply_review(review, Some(sidecar.round().input()), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(sidecar.round().input()), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(client.step(), Err(ProbeClientError::Cancelled));
}
