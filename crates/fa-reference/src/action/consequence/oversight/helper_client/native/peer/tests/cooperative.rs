//! Actual token steps and original transport, without an inference callback.
use super::*;
use super::super::super::tests::decoder;
use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;

#[test]
fn long_prefill_yields_one_token_per_drive_and_can_be_cancelled_between_tokens() {
    let (decoder, policy) = decoder(b"allow", 0);
    let observer = decoder.observation();
    let evaluator = NativeEvaluator::new(decoder, policy).unwrap();
    let (mut worker, transport) = make_with(frame(&[b'?'; 512], &expected()), evaluator);
    until(&mut worker, ClientPhase::NeedsInference);
    let io_before = counts(&transport);
    for position in 0..6 {
        let drive = worker.drive(MAX_CLIENT_DRIVE_STEPS).unwrap();
        assert_eq!(drive.steps, 1);
        assert_eq!(drive.evaluations, usize::from(position == 0));
        let Ok(NativeClientProgress::Inference(progress)) = drive.progress else { panic!("prefill must yield"); };
        assert_eq!(progress.work.position, position + 1);
        assert_eq!(progress.work.decoder.tokens, position + 1);
        assert_eq!(progress.reviewed_prompt_tokens as u64, position + 1);
        assert_eq!(progress.requested_prompt_tokens, 512);
        assert_eq!(progress.work.sampled_draws, 0);
        assert_eq!(worker.interest(), ClientInterest::Inference);
        assert!(worker.report().is_none());
        assert_eq!(counts(&transport), io_before);
    }
    let before = worker.evaluation_progress();
    assert!(worker.cancel());
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    assert_eq!(worker.evaluation_progress().work, before.work);
    assert_eq!(worker.evaluation_progress().reviewed_prompt_tokens, 6);
    assert_eq!(worker.evaluation_progress().finish, None);
    assert_eq!(worker.drive(256).unwrap().steps, 0);
    assert_eq!(worker.failure(), Some(NativeClientError::Cancelled));
    assert_eq!(counts(&transport), io_before);
}

#[test]
fn short_worker_can_finish_while_a_long_worker_is_still_in_prefill() {
    let (mut long, a) = make_with(frame(&[b'?'; 64], &expected()), evaluator(b"allow", 0));
    let (mut short, b) = make_with(frame(b"!", &expected()), evaluator(b"allow", 0));
    until(&mut long, ClientPhase::NeedsInference); until(&mut short, ClientPhase::NeedsInference);
    for position in 0..3 {
        let short_before = short.evaluation_progress();
        assert!(matches!(long.step(), Ok(NativeClientProgress::Inference(_))));
        assert_eq!(short.evaluation_progress(), short_before);
        let long_before = long.evaluation_progress();
        let result = short.step().unwrap();
        if position == 2 { assert_eq!(result, NativeClientProgress::Judged(Verdict::Deny)); }
        else { assert!(matches!(result, NativeClientProgress::Inference(_))); }
        assert_eq!(long.evaluation_progress(), long_before);
    }
    assert_eq!(short.evaluations(), 1); assert_eq!(long.evaluations(), 1);
    assert_eq!(short.sampled_draws(), 2); assert_eq!(long.sampled_draws(), 0);
    assert_eq!(long.evaluation_progress().reviewed_prompt_tokens, 3);
    assert!(a.borrow().output.is_empty()); assert!(b.borrow().output.is_empty());
    until(&mut short, ClientPhase::AwaitingReveal);
    assert_eq!(b.borrow().output.len(), 9);
    assert!(long.cancel());
    b.borrow_mut().input.push_back(b'R');
    until(&mut short, ClientPhase::ReplySent);
    assert_eq!(short.sampled_draws(), 2);
    assert_eq!(long.evaluation_progress().reviewed_prompt_tokens, 3);
}

#[test]
fn quiet_answer_cannot_emit_any_commitment_until_its_terminal_is_reviewed() {
    let (mut worker, transport) = make();
    until(&mut worker, ClientPhase::NeedsInference);
    let before = counts(&transport);
    for _ in 0..2 {
        assert!(matches!(worker.step(), Ok(NativeClientProgress::Inference(_))));
        assert_eq!(worker.phase(), ClientPhase::NeedsInference);
        assert!(worker.report().is_none()); assert_eq!(counts(&transport), before);
    }
    assert_eq!(worker.evaluation_progress().released_answer_tokens, 1);
    assert_eq!(worker.sampled_draws(), 1);
    assert_eq!(worker.step(), Ok(NativeClientProgress::Judged(Verdict::Allow)));
    assert_eq!(counts(&transport), before); // Freezing a frame is not a write.
    assert_eq!(worker.phase(), ClientPhase::SendingCommitment);
    let completed_work = worker.evaluation_progress().work;
    until(&mut worker, ClientPhase::AwaitingReveal);
    for _ in 0..3 {
        assert_eq!(worker.step(), Ok(NativeClientProgress::Protocol(ClientProgress::Blocked)));
        assert_eq!(worker.evaluation_progress().work, completed_work);
    }
    assert_eq!(worker.evaluations(), 1);
}

#[test]
fn later_transport_bytes_do_not_change_the_frozen_prompt_or_retokenize_it() {
    let (mut worker, transport) = make();
    until(&mut worker, ClientPhase::NeedsInference);
    let original = worker.input().unwrap().clone();
    worker.step().unwrap();
    transport.borrow_mut().input.extend(frame(b"!", &expected()));
    let reads = transport.borrow().reads;
    assert_eq!(judge(&mut worker), Ok(Verdict::Allow));
    assert_eq!(worker.input(), Some(&original));
    assert_eq!(worker.report().unwrap().prompt().source(), b"?");
    assert_eq!(transport.borrow().reads, reads);
    assert_eq!(worker.evaluation_progress().work.decoder.tokens, 3);
    assert_eq!(worker.evaluations(), 1);
}

#[test]
fn cancel_after_quiet_answer_prevents_late_reveal_and_retains_consumed_draw() {
    let (decoder, policy) = decoder(b"allow", 0);
    let observer = decoder.observation();
    let (mut worker, transport) = make_with(frame(b"?", &expected()), NativeEvaluator::new(decoder, policy).unwrap());
    until(&mut worker, ClientPhase::NeedsInference);
    worker.step().unwrap(); worker.step().unwrap();
    assert_eq!(worker.sampled_draws(), 1);
    let before = worker.evaluation_progress().work;
    let io_before = counts(&transport);
    assert!(worker.cancel()); assert!(!worker.cancel());
    transport.borrow_mut().input.push_back(b'R');
    transport.borrow_mut().input.extend(frame(b"!", &expected()));
    for _ in 0..4 {
        assert_eq!(worker.step(), Err(NativeClientError::Cancelled));
        assert_eq!(worker.drive(256).unwrap().steps, 0);
        assert_eq!(worker.evaluation_progress().work, before);
        assert_eq!(counts(&transport), io_before);
    }
    assert!(worker.report().is_none());
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    assert_eq!(worker.evaluation_progress().finish, None);
}

#[test]
fn caught_post_token_pump_unwind_closes_ownership_without_cleaning_the_failure() {
    let (decoder, policy) = decoder(b"allow", 0);
    let observer = decoder.observation();
    let (mut worker, transport) = make_with(frame(b"?", &expected()), NativeEvaluator::new(decoder, policy).unwrap());
    until(&mut worker, ClientPhase::NeedsInference);
    worker.step().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        worker.step_with(|| panic!("after original sampled token, before pump acknowledgment"))
    })).is_err());
    assert_eq!(worker.failure(), Some(NativeClientError::Interrupted));
    let work = worker.evaluation_progress().work;
    assert_eq!(work.decoder.tokens, 2); assert_eq!(work.sampled_draws, 1);
    assert!(worker.report().is_none());
    let before = counts(&transport);
    // Even zero-step inspection of the failed pump disposes the numerical owner.
    let drive = worker.drive(256).unwrap();
    assert_eq!(drive.steps, 0); assert_eq!(drive.progress, Err(NativeClientError::Interrupted));
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    assert!(!worker.cancel());
    assert_eq!(worker.failure(), Some(NativeClientError::Interrupted));
    assert_eq!(worker.evaluation_progress().work, work);
    assert_eq!(worker.step(), Err(NativeClientError::Interrupted));
    assert_eq!(counts(&transport), before);
}

#[test]
fn invalid_drive_limits_during_inference_do_not_spend_a_token_or_reopen_admission() {
    let (mut worker, transport) = make();
    until(&mut worker, ClientPhase::NeedsInference);
    worker.step().unwrap();
    let before = worker.evaluation_progress(); let io_before = counts(&transport);
    assert_eq!(worker.drive(0), Err(Error::InvalidInput));
    assert_eq!(worker.drive(MAX_CLIENT_DRIVE_STEPS + 1), Err(Error::Limit));
    assert_eq!(worker.evaluation_progress(), before); assert_eq!(counts(&transport), io_before);
    assert_eq!(judge(&mut worker), Ok(Verdict::Allow));
    assert_eq!(worker.evaluations(), 1);
}

#[cfg(unix)]
#[test]
fn real_unix_worker_can_be_cancelled_mid_prefill_without_any_committed_vote() {
    use std::os::unix::net::UnixStream;
    let (mut supervisor, socket) = UnixStream::pair().unwrap();
    supervisor.set_nonblocking(true).unwrap();
    let (decoder, policy) = decoder(b"allow", 0); let observer = decoder.observation();
    let mut worker = NativeHelperClient::from_unix(socket, NativeEvaluator::new(decoder, policy).unwrap(), salt()).unwrap();
    supervisor.write_all(&frame(&[b'?'; 64], &expected())).unwrap();
    for _ in 0..16 {
        if worker.phase() == ClientPhase::NeedsInference { break; }
        worker.step().unwrap();
    }
    assert_eq!(worker.phase(), ClientPhase::NeedsInference);
    for _ in 0..4 { assert!(matches!(worker.step(), Ok(NativeClientProgress::Inference(_)))); }
    assert_eq!(worker.evaluation_progress().reviewed_prompt_tokens, 4);
    let work = worker.evaluation_progress().work;
    let mut received = [0_u8; 9];
    assert_eq!(supervisor.read(&mut received).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert!(worker.cancel());
    assert_eq!(worker.step(), Err(NativeClientError::Cancelled));
    assert_eq!(worker.evaluation_progress().work, work);
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    drop(worker);
    assert_eq!(supervisor.read(&mut received).unwrap(), 0);
}
