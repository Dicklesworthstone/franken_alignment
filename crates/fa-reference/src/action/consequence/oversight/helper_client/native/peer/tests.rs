use super::*;
use super::super::tests::{evaluator, expected, frame};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

type Shared = Rc<RefCell<Transport>>;
#[derive(Default)]
struct Transport {
    input: VecDeque<u8>, output: Vec<u8>, chunk: usize,
    reads: usize, writes: usize, flushes: usize,
    read_error: Option<io::ErrorKind>, write_error: Option<io::ErrorKind>, flush_error: Option<io::ErrorKind>,
    eof: bool, panic_after_write: bool,
}
struct Stream(Shared);
impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut(); state.reads += 1;
        if let Some(error) = state.read_error.take() { return Err(io::Error::from(error)); }
        if state.input.is_empty() {
            return if state.eof { Ok(0) } else { Err(io::Error::from(io::ErrorKind::WouldBlock)) };
        }
        let count = bytes.len().min(state.chunk).min(state.input.len());
        for byte in &mut bytes[..count] { *byte = state.input.pop_front().unwrap(); }
        Ok(count)
    }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut(); state.writes += 1;
        if let Some(error) = state.write_error.take() { return Err(io::Error::from(error)); }
        let count = bytes.len().min(state.chunk);
        state.output.extend_from_slice(&bytes[..count]);
        assert!(!state.panic_after_write, "simulated unwind AFTER bytes reached transport");
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.0.borrow_mut(); state.flushes += 1;
        match state.flush_error.take() { Some(error) => Err(io::Error::from(error)), None => Ok(()) }
    }
}
fn salt() -> Vec<u8> { b"independent-host-test-salt-123456".to_vec() }
fn make_with(input: Vec<u8>, evaluator: NativeEvaluator) -> (NativeHelperClient<Stream>, Shared) {
    let shared = Rc::new(RefCell::new(Transport { input: input.into(), chunk: 2, ..Transport::default() }));
    let worker = NativeHelperClient::new(Stream(Rc::clone(&shared)), evaluator, salt()).unwrap();
    (worker, shared)
}
fn make() -> (NativeHelperClient<Stream>, Shared) { make_with(frame(b"?", &expected()), evaluator(b"allow", 0)) }
fn until(worker: &mut NativeHelperClient<Stream>, phase: ClientPhase) {
    for _ in 0..2048 {
        if worker.phase() == phase { return; }
        worker.step().unwrap();
    }
    panic!("bounded script did not reach {phase:?}");
}
fn counts(shared: &Shared) -> (usize, usize, usize, Vec<u8>) {
    let s = shared.borrow(); (s.reads, s.writes, s.flushes, s.output.clone())
}

// Drive the now-cooperative evaluator without concealing token-sized work.
fn judge(worker: &mut NativeHelperClient<Stream>) -> Result<Verdict, NativeClientError> {
    for _ in 0..2048 {
        let before = worker.evaluation_progress().work.decoder.tokens;
        match worker.step()? {
            NativeClientProgress::Inference(progress) => {
                assert_eq!(progress.status, NativeEvaluationStatus::Running);
                assert_eq!(progress.work.decoder.tokens, before + 1);
                assert!(worker.report().is_none());
            }
            NativeClientProgress::Judged(verdict) => {
                assert_eq!(worker.evaluation_progress().work.decoder.tokens, before + 1);
                return Ok(verdict);
            }
            other => panic!("unexpected protocol progress during native evaluation: {other:?}"),
        }
    }
    panic!("bounded native evaluation did not terminate");
}

#[test]
fn fragmented_io_and_transient_flush_preserve_one_native_result_and_reveal_gate() {
    let (mut worker, shared) = make();
    shared.borrow_mut().read_error = Some(io::ErrorKind::Interrupted);
    assert_eq!(worker.drive(256).unwrap().progress, Ok(NativeClientProgress::Protocol(ClientProgress::Blocked)));
    until(&mut worker, ClientPhase::NeedsInference);
    assert_eq!(worker.evaluations(), 0); assert_eq!(worker.sampled_draws(), 0);
    assert_eq!(judge(&mut worker), Ok(Verdict::Allow));
    assert_eq!(worker.evaluations(), 1); assert_eq!(worker.sampled_draws(), 2);
    assert!(shared.borrow().output.is_empty());
    shared.borrow_mut().write_error = Some(io::ErrorKind::WouldBlock);
    assert_eq!(worker.step(), Ok(NativeClientProgress::Protocol(ClientProgress::Blocked)));
    shared.borrow_mut().flush_error = Some(io::ErrorKind::WouldBlock);
    until(&mut worker, ClientPhase::AwaitingReveal);
    let original = worker.input().unwrap();
    let commit = original.commitment_frame(Verdict::Allow, &salt()).unwrap();
    let reveal = original.reveal_frame(Verdict::Allow, &salt()).unwrap();
    assert_eq!(shared.borrow().output.as_slice(), commit);
    assert_eq!(worker.step(), Ok(NativeClientProgress::Protocol(ClientProgress::Blocked)));
    assert_eq!(shared.borrow().output.as_slice(), commit);
    shared.borrow_mut().input.push_back(b'R');
    until(&mut worker, ClientPhase::ReplySent);
    assert_eq!(shared.borrow().output, [commit.as_slice(), reveal.as_slice()].concat());
    assert_eq!(worker.evaluations(), 1); assert_eq!(worker.sampled_draws(), 2);
    assert_eq!(worker.report().unwrap().prompt().source(), b"?");
    let before = counts(&shared);
    assert_eq!(worker.drive(256).unwrap().steps, 0);
    assert!(!worker.cancel()); assert_eq!(counts(&shared), before);
    assert!(!format!("{worker:?}").contains("independent-host"));
}

#[test]
fn mismatched_or_broken_wire_inputs_never_enter_inference_or_emit_a_default_vote() {
    for mode in 0..3 {
        let mut p = expected(); if mode == 0 { p.tokenizer_epoch += 1; }
        let mut bytes = frame(b"?", &p);
        if mode == 1 { bytes.pop(); }
        if mode == 2 { bytes[0] = b'X'; }
        let (mut worker, shared) = make_with(bytes, evaluator(b"allow", 0));
        shared.borrow_mut().eof = true;
        for _ in 0..2048 { if worker.step().is_err() { break; } }
        assert_eq!(worker.phase(), ClientPhase::Failed);
        assert_eq!(worker.evaluations(), 0); assert!(shared.borrow().output.is_empty());
        let before = counts(&shared); assert!(worker.step().is_err()); assert_eq!(counts(&shared), before);
    }
}

#[test]
fn held_and_invalid_native_output_never_write_even_a_commitment() {
    for (word, alarm) in [(b"allow".as_slice(), 1), (b"allow", 2), (b"allow\n", 0)] {
        let (mut worker, shared) = make_with(frame(b"?", &expected()), evaluator(word, alarm));
        until(&mut worker, ClientPhase::NeedsInference);
        assert!(matches!(judge(&mut worker), Err(NativeClientError::Inference(_))));
        assert!(worker.report().is_some()); assert_eq!(worker.evaluations(), 1);
        assert!(shared.borrow().output.is_empty());
        let draws = worker.sampled_draws(); let before = counts(&shared);
        assert!(worker.step().is_err()); assert_eq!(counts(&shared), before);
        assert_eq!(worker.sampled_draws(), draws);
    }
}

#[test]
fn salt_limits_are_checked_before_inference_and_secrets_are_not_model_input() {
    let mut bytes = frame(b"?", &expected());
    let salt_offset = 9 + 8 + 32 + 2 + 6;
    bytes[salt_offset..salt_offset + 2].copy_from_slice(&8_u16.to_be_bytes());
    let (mut worker, shared) = make_with(bytes, evaluator(b"allow", 0));
    until(&mut worker, ClientPhase::NeedsInference);
    assert_eq!(worker.step(), Err(NativeClientError::Protocol(WorkerIoError::Protocol(Error::Limit))));
    assert_eq!(worker.evaluations(), 0); assert!(shared.borrow().output.is_empty());
    for length in [MIN_NATIVE_SALT_BYTES - 1, MAX_WORKER_SALT_BYTES + 1] {
        assert_eq!(NativeHelperClient::new(io::Cursor::new(Vec::<u8>::new()), evaluator(b"allow", 0),
            vec![1; length]).unwrap_err(), Error::Limit);
    }
}

#[test]
fn cancellation_before_inference_or_after_commitment_cannot_replace_the_missing_reveal() {
    for after_commit in [false, true] {
        let (mut worker, shared) = make();
        until(&mut worker, if after_commit { ClientPhase::AwaitingReveal } else { ClientPhase::NeedsInference });
        let before = counts(&shared); let draws = worker.sampled_draws();
        assert!(worker.cancel()); assert!(!worker.cancel());
        shared.borrow_mut().input.push_back(b'R');
        assert_eq!(worker.step(), Err(NativeClientError::Cancelled));
        assert_eq!(worker.interest(), ClientInterest::Finished);
        assert_eq!(counts(&shared), before); assert_eq!(worker.sampled_draws(), draws);
        assert_eq!(shared.borrow().output.len(), if after_commit { 9 } else { 0 });
    }
}

#[test]
fn fatal_writes_or_flushes_latch_without_recomputing_or_resending_the_result() {
    for flush in [false, true] {
        let (mut worker, shared) = make();
        until(&mut worker, ClientPhase::SendingCommitment);
        if flush { shared.borrow_mut().flush_error = Some(io::ErrorKind::BrokenPipe); }
        else { shared.borrow_mut().write_error = Some(io::ErrorKind::BrokenPipe); }
        for _ in 0..16 { if worker.step().is_err() { break; } }
        let error = NativeClientError::Protocol(WorkerIoError::Io(io::ErrorKind::BrokenPipe));
        assert_eq!(worker.failure(), Some(error));
        let before = counts(&shared);
        for _ in 0..3 { assert_eq!(worker.step(), Err(error)); }
        assert_eq!(counts(&shared), before); assert_eq!(worker.evaluations(), 1);
        assert_eq!(worker.sampled_draws(), 2);
    }
}

#[test]
fn caught_unwind_after_transport_acceptance_does_not_resend_ambiguous_bytes() {
    let (mut worker, shared) = make();
    until(&mut worker, ClientPhase::SendingCommitment);
    shared.borrow_mut().panic_after_write = true;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.step())).is_err());
    assert_eq!(worker.failure(), Some(NativeClientError::Interrupted));
    assert!(!shared.borrow().output.is_empty());
    let before = counts(&shared);
    shared.borrow_mut().panic_after_write = false;
    assert_eq!(worker.step(), Err(NativeClientError::Interrupted));
    assert_eq!(counts(&shared), before); assert_eq!(worker.evaluations(), 1);
}

#[test]
fn bounded_drive_yields_after_input_and_inference_and_invalid_limits_do_no_io() {
    let (mut worker, shared) = make(); let before = counts(&shared);
    assert_eq!(worker.drive(0), Err(Error::InvalidInput));
    assert_eq!(worker.drive(MAX_CLIENT_DRIVE_STEPS + 1), Err(Error::Limit));
    assert_eq!(counts(&shared), before);
    let load = worker.drive(256).unwrap();
    assert_eq!(load.progress, Ok(NativeClientProgress::Protocol(ClientProgress::NeedsInference)));
    assert_eq!(load.evaluations, 0); assert_eq!(shared.borrow().writes, 0);
    let inferred = worker.drive(256).unwrap();
    assert_eq!(inferred.steps, 1); assert_eq!(inferred.evaluations, 1);
    assert!(matches!(inferred.progress, Ok(NativeClientProgress::Inference(_))));
    assert_eq!(worker.evaluation_progress().work.decoder.tokens, 1);
    assert_eq!(worker.sampled_draws(), 0);
    let partial = worker.drive(256).unwrap();
    assert_eq!(partial.steps, 1); assert_eq!(partial.evaluations, 0);
    assert!(matches!(partial.progress, Ok(NativeClientProgress::Inference(_))));
    assert_eq!(worker.sampled_draws(), 1); assert!(worker.report().is_none());
    let complete = worker.drive(256).unwrap();
    assert_eq!(complete.steps, 1); assert_eq!(complete.evaluations, 0);
    assert_eq!(complete.progress, Ok(NativeClientProgress::Judged(Verdict::Allow)));
    assert_eq!(worker.sampled_draws(), 2);
    assert_eq!(shared.borrow().writes, 0);
}

#[test]
fn precomputed_evaluator_cannot_be_imported_as_an_answer_to_another_connection() {
    let mut evaluated = evaluator(b"allow", 0);
    evaluated.evaluate(&super::super::tests::input(b"?")).unwrap();
    assert_eq!(NativeHelperClient::new(io::Cursor::new(Vec::<u8>::new()), evaluated, salt()).unwrap_err(), Error::WrongState);
    // An identical fresh model DOES remain a usable independently created worker.
    let (mut worker, _) = make(); until(&mut worker, ClientPhase::SendingCommitment);
    assert_eq!(worker.evaluations(), 1);
}

#[cfg(unix)]
#[test]
fn unix_peer_answers_the_original_worker_frames_and_withholds_reveal_until_requested() {
    use std::os::unix::net::UnixStream;
    let (mut supervisor, worker_socket) = UnixStream::pair().unwrap();
    supervisor.set_nonblocking(true).unwrap();
    let mut worker = NativeHelperClient::from_unix(worker_socket, evaluator(b"allow", 0), salt()).unwrap();
    supervisor.write_all(&frame(b"?", &expected())).unwrap();
    for _ in 0..128 {
        worker.step().unwrap();
        if worker.phase() == ClientPhase::AwaitingReveal { break; }
    }
    assert_eq!(worker.phase(), ClientPhase::AwaitingReveal);
    let mut commit = [0; 9]; supervisor.read_exact(&mut commit).unwrap();
    assert_eq!(commit, worker.input().unwrap().commitment_frame(Verdict::Allow, &salt()).unwrap());
    let mut extra = [0];
    assert_eq!(supervisor.read(&mut extra).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    supervisor.write_all(b"R").unwrap();
    for _ in 0..128 {
        worker.step().unwrap(); if worker.phase() == ClientPhase::ReplySent { break; }
    }
    assert_eq!(worker.phase(), ClientPhase::ReplySent);
    let expected = worker.input().unwrap().reveal_frame(Verdict::Allow, &salt()).unwrap();
    let mut reveal = vec![0; expected.len()]; supervisor.read_exact(&mut reveal).unwrap();
    assert_eq!(reveal, expected); assert_eq!(worker.evaluations(), 1);
}

#[cfg(unix)]
#[test]
fn native_socket_judgment_reaches_original_congress_and_held_or_cancelled_workers_stay_missing() {
    use crate::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
    use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use crate::action::consequence::delivery::PublicationEndpoint;
    use crate::action::consequence::gate::TargetCeiling;
    use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
    use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
    use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
        OversightBroker, ReviewWindow, action_frame};
    use crate::action::consequence::oversight::helper_workers::{HelperLimits, HelperRound};
    use crate::action::consequence::oversight::helper_workers::io::HelperConnection;
    use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
    use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
    use crate::reducer::Caps;
    use crate::Snapshot;
    use std::collections::BTreeMap;
    use std::os::unix::net::UnixStream;

    // Near-identical native success, monitor hold, prefill cancellation and
    // cancellation after the quiet answer but BEFORE the terminal review.
    for mode in 0..4 {
        let alarm = u8::from(mode == 1);
        let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
        let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
        let contracts = CommitteeContract::new(BTreeMap::from([("native".to_owned(),
            HelperContract::new(expected(), 7, b"?".to_vec()).unwrap())])).unwrap();
        let config = ControllerConfig { scope, total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("native".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
        };
        let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        let fence = endpoint.install_fence(broker.fence_request()).unwrap(); broker.confirm_fence(fence).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
        let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) };
        let action = broker.propose(1, ActionSpec { version: VERSION, scope, target: Some(target),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(100), units: 16 }, &snapshot).unwrap().action;
        let mut bytes = action_frame(&action); let split = bytes.len(); bytes.push(b'?'); let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, expected(), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: split } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: split, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("native".to_owned(), view)])).unwrap();
        broker.record_inputs(1, 0, inputs.clone()).unwrap();
        let session = broker.begin_review(1, 9, [7; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, &snapshot).unwrap();
        let (mut round, mut ports) = HelperRound::new(session, HelperLimits::default()).unwrap();
        let (server, worker_socket) = UnixStream::pair().unwrap(); server.set_nonblocking(true).unwrap();
        let mut connection = HelperConnection::new(ports.remove("native").unwrap(), server).unwrap();
        let mut worker = NativeHelperClient::from_unix(worker_socket, evaluator(b"allow", alarm), salt()).unwrap();
        // At most one token per original byte, two answer/control steps and
        // bounded framing overhead. This is a step bound, not elapsed time.
        for _ in 0..end + 32 {
            connection.step().unwrap();
            let before = worker.evaluation_progress().work.decoder.tokens;
            let progress = worker.step();
            let native = worker.evaluation_progress();
            assert!(native.work.decoder.tokens <= before + 1);
            round.advance(ElapsedTick(1)).unwrap();
            if matches!(progress, Ok(NativeClientProgress::Inference(_))) {
                assert!(!round.statuses()["native"].committed);
                assert!(worker.report().is_none());
                if (mode == 2 && native.reviewed_prompt_tokens == 1)
                    || (mode == 3 && native.released_answer_tokens == 1) {
                    assert!(worker.cancel());
                    break;
                }
            }
            if progress.is_err() || round.statuses()["native"].revealed { break; }
        }
        assert_eq!(worker.evaluations(), 1);
        if mode == 0 {
            assert!(round.statuses()["native"].revealed);
            let review = round.finish(ElapsedTick(1)).unwrap();
            broker.apply_review(review, Some(&inputs), &snapshot).unwrap();
            assert!(broker.authorize(1, Some(&inputs), &snapshot).is_ok());
            assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Authorized);
        } else {
            if mode == 1 {
                assert!(matches!(worker.failure(), Some(NativeClientError::Inference(_))));
            } else {
                assert_eq!(worker.failure(), Some(NativeClientError::Cancelled));
                assert_eq!(worker.sampled_draws(), u64::from(mode == 3));
            }
            assert!(!round.statuses()["native"].committed);
            drop(worker);
            assert!(connection.step().is_err());
            let review = round.finish(ElapsedTick(8)).unwrap();
            broker.observe_time(ElapsedTick(8)).unwrap();
            broker.apply_review(review, Some(&inputs), &snapshot).unwrap();
            assert!(broker.authorize(1, Some(&inputs), &snapshot).is_err());
            assert_eq!(round.statuses().len(), 1);
            assert!(!round.statuses()["native"].revealed);
        }
        assert_eq!(endpoint.execution_count(), 0);
    }
}

mod cooperative;
