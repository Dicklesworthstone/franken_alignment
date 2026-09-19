use super::*;
use super::super::tests::{evaluator, expected, frame, input};
use super::super::{NativeEvaluationStatus, NativeEvaluationError};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use crate::round::Verdict;
use std::io::{Read, Write};

fn salt() -> Vec<u8> { b"independently-provisioned-test-salt".to_vec() }
fn queued(prompt: &[u8], reveal: bool) -> (UnixStream, UnixStream) {
    let (mut parent, child) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    parent.write_all(&frame(prompt, &expected())).unwrap();
    if reveal { parent.write_all(b"R").unwrap(); }
    (parent, child)
}
fn budget(steps: usize) -> NativeProcessBudget { NativeProcessBudget::new(2000, steps).unwrap() }

#[test]
fn native_process_sends_original_frames_for_input_dependent_verdicts_then_closes() {
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let (mut parent, child) = queued(prompt, true);
        let report = run_native_worker(child, evaluator(b"allow", 0), salt(), budget(4096)).unwrap();
        assert_eq!(report.stop, NativeProcessStop::ReplySent);
        assert_eq!(report.phase, ClientPhase::ReplySent);
        assert_eq!(report.evaluations, 1);
        assert_eq!(report.evaluation.work.sampled_draws, 2);
        assert_eq!(report.evaluation.status, NativeEvaluationStatus::Judged(verdict));
        let request = input(prompt);
        let expected = [request.commitment_frame(verdict, &salt()).unwrap().as_slice(),
            request.reveal_frame(verdict, &salt()).unwrap().as_slice()].concat();
        let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
    }
}

#[test]
fn expired_startup_budget_reads_no_request_and_computes_no_token() {
    let (mut parent, child) = queued(b"?", false);
    let b = budget(4096); let deadline = b.deadline;
    let report = run_with_clock(child, evaluator(b"allow", 0), salt(), b, |_, _| deadline).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Deadline); assert_eq!(report.steps, 0);
    assert_eq!(report.evaluation.work.decoder.tokens, 0); assert_eq!(report.evaluations, 0);
    let mut bytes = Vec::new(); let _ = parent.read_to_end(&mut bytes);
    assert!(bytes.is_empty());
}

#[test]
fn expired_after_quiet_terminal_cannot_write_the_frozen_commitment() {
    let (mut parent, child) = queued(b"?", false);
    let b = budget(4096); let start = b.started; let deadline = b.deadline;
    let report = run_with_clock(child, evaluator(b"allow", 0), salt(), b, |_, worker| {
        if worker.phase() == ClientPhase::SendingCommitment { deadline } else { start }
    }).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Deadline);
    assert_eq!(report.phase, ClientPhase::SendingCommitment);
    assert_eq!(report.evaluation.work.sampled_draws, 2);
    assert_eq!(report.evaluation.status, NativeEvaluationStatus::Judged(Verdict::Allow));
    let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
}

#[test]
fn deadline_after_a_write_retains_phase_and_does_not_claim_nothing_was_sent() {
    let (mut parent, child) = queued(b"?", false);
    let b = budget(4096); let start = b.started; let deadline = b.deadline;
    let report = run_with_clock(child, evaluator(b"allow", 0), salt(), b, |_, worker| {
        if worker.phase() == ClientPhase::AwaitingReveal { deadline } else { start }
    }).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Deadline);
    assert_eq!(report.phase, ClientPhase::AwaitingReveal);
    let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, input(b"?").commitment_frame(Verdict::Allow, &salt()).unwrap());
}

#[test]
fn interrupted_prefill_keeps_work_but_neither_answer_nor_replacement_vote() {
    let (mut parent, child) = queued(b"long prompt?", false);
    let b = budget(4096); let start = b.started; let deadline = b.deadline;
    let report = run_with_clock(child, evaluator(b"allow", 0), salt(), b, |_, worker| {
        if worker.evaluation_progress().work.decoder.tokens == 2 { deadline } else { start }
    }).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Deadline);
    assert_eq!(report.evaluation.work.decoder.tokens, 2);
    assert_eq!(report.evaluation.work.sampled_draws, 0);
    assert_eq!(report.evaluation.status, NativeEvaluationStatus::Cancelled);
    let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
}

#[test]
fn complete_request_step_cap_stops_before_inference_and_is_not_a_default_vote() {
    let (mut parent, child) = queued(b"?", false);
    let report = run_native_worker(child, evaluator(b"allow", 0), salt(), budget(2)).unwrap();
    assert_eq!(report.stop, NativeProcessStop::StepLimit); assert_eq!(report.steps, 2);
    assert_eq!(report.evaluation.work.decoder.tokens, 0);
    let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
}

#[test]
fn native_hold_closes_without_commitment_and_preserves_original_diagnostics() {
    let (mut parent, child) = queued(b"?", false);
    let report = run_native_worker(child, evaluator(b"allow", 1), salt(), budget(4096)).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Client(NativeClientError::Inference(
        NativeEvaluationError::Incomplete(GenerationFinish::Held))));
    assert_eq!(report.evaluation.work.sampled_draws, 1);
    let mut bytes = Vec::new(); parent.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
}

#[test]
fn real_socket_wait_uses_absolute_timeout_and_does_not_spin_without_a_request() {
    let (_parent, child) = UnixStream::pair().unwrap();
    let report = run_native_worker(child, evaluator(b"allow", 0), salt(),
        NativeProcessBudget::new(20, 4096).unwrap()).unwrap();
    assert!(matches!(report.stop, NativeProcessStop::Deadline | NativeProcessStop::Client(_)));
    assert!(report.steps < 4096); assert_eq!(report.evaluations, 0);
}

#[test]
fn original_reveal_gate_still_waits_for_the_supervisor() {
    let (mut parent, child) = queued(b"?", false);
    let worker = std::thread::spawn(move || {
        run_native_worker(child, evaluator(b"allow", 0), salt(), budget(4096)).unwrap()
    });
    let mut commitment = [0; 9]; parent.read_exact(&mut commitment).unwrap();
    assert_eq!(commitment, input(b"?").commitment_frame(Verdict::Allow, &salt()).unwrap());
    parent.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let mut byte = [0];
    assert!(matches!(parent.read(&mut byte).unwrap_err().kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut));
    parent.write_all(b"R").unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut reveal = Vec::new(); parent.read_to_end(&mut reveal).unwrap();
    assert_eq!(reveal, input(b"?").reveal_frame(Verdict::Allow, &salt()).unwrap());
    assert_eq!(worker.join().unwrap().stop, NativeProcessStop::ReplySent);
}

#[test]
fn invalid_lifetime_and_step_limits_have_no_permissive_defaults() {
    for (time, steps, error) in [(0, 1, Error::InvalidInput), (1, 0, Error::InvalidInput),
        (MAX_PROCESS_MILLIS + 1, 1, Error::Limit), (1, MAX_PROCESS_STEPS + 1, Error::Limit)] {
        assert_eq!(NativeProcessBudget::new(time, steps).unwrap_err(), error);
    }
    assert!(NativeProcessBudget::new(MAX_PROCESS_MILLIS, MAX_PROCESS_STEPS).is_ok());
}
