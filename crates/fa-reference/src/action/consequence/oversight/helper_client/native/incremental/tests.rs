//! Synthetic models exercise real tokenization, numerical work and monitor gates.
use super::*;
use super::super::Verdict;
use super::super::tests::{decoder, evaluator, expected, frame, input};
use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use crate::action::consequence::oversight::helper_workers::wire::decode_request;

fn finish(worker: &mut NativeEvaluator) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
    for _ in 0..2048 {
        let progress = worker.advance(worker.position())?;
        if matches!(progress.status, NativeEvaluationStatus::Judged(_)) { return Ok(progress); }
    }
    panic!("bounded fixture never reached terminal judgment");
}

#[test]
fn admission_is_complete_and_each_advance_runs_at_most_one_original_token() {
    let mut worker = evaluator(b"allow", 0);
    let original = input(b"\x00\xff???");
    let admitted = worker.begin(&original).unwrap();
    assert_eq!(admitted.status, NativeEvaluationStatus::Running);
    assert_eq!(admitted.work, NativeEvaluationWork::default());
    assert_eq!(admitted.requested_prompt_tokens, 5);
    assert_eq!(admitted.reviewed_prompt_tokens, 0);
    assert_eq!(worker.input(), Some(&original));
    for position in 0..5 {
        let progress = worker.advance(position).unwrap();
        assert_eq!(progress.work.position, position + 1);
        assert_eq!(progress.work.decoder.tokens, position + 1);
        assert_eq!(progress.work.sampled_draws, 0);
        assert_eq!(progress.reviewed_prompt_tokens as u64, position + 1);
        assert_eq!(progress.released_answer_tokens, 0);
        assert_eq!(progress.status, NativeEvaluationStatus::Running);
        assert!(worker.report().is_none());
    }
    let partial = worker.advance(5).unwrap();
    assert_eq!(partial.released_answer_tokens, 1);
    assert_eq!(partial.work.sampled_draws, 1);
    assert_eq!(partial.status, NativeEvaluationStatus::Running);
    assert_eq!(partial.finish, None);
    assert!(worker.report().is_none()); // Even the complete spelling "allow" is not a vote.
    let terminal = worker.advance(6).unwrap();
    assert_eq!(terminal.status, NativeEvaluationStatus::Judged(Verdict::Allow));
    assert_eq!(terminal.work.decoder.tokens, 7);
    assert_eq!(terminal.work.sampled_draws, 2);
    assert_eq!(terminal.finish, Some(GenerationFinish::StopToken));
    assert_eq!(worker.report().unwrap().prompt().source(), original.actual_input().submitted_bytes());
}

#[test]
fn segmentation_and_one_shot_have_identical_native_results_and_work() {
    for (word, verdict) in [(b"allow".as_slice(), Verdict::Allow), (b"hold", Verdict::Hold),
        (b"deny", Verdict::Deny), (b"abstain", Verdict::Abstain)] {
        for prompt in [b"raw\xff?".as_slice(), b"!"] {
            let mut whole = evaluator(word, 0);
            let mut segmented = evaluator(word, 0);
            let packet = input(prompt);
            let expected = if prompt == b"!" { Verdict::Deny } else { verdict };
            assert_eq!(whole.evaluate(&packet), Ok(expected));
            segmented.begin(&packet).unwrap();
            let result = finish(&mut segmented).unwrap();
            assert_eq!(result.status, NativeEvaluationStatus::Judged(expected));
            assert_eq!(whole.progress(), result);
            let a = whole.report().unwrap(); let b = segmented.report().unwrap();
            assert_eq!(a.bytes(), b.bytes());
            assert_eq!(a.prompt().tokens(), b.prompt().tokens());
            assert_eq!(a.generation().work(), b.generation().work());
            assert_eq!(a.generation().last_review(), b.generation().last_review());
        }
    }
}

#[test]
fn stale_pulls_and_terminal_polls_do_not_compute_or_accept_another_input() {
    let mut worker = evaluator(b"allow", 0);
    worker.begin(&input(b"??")).unwrap();
    let initial = worker.progress();
    assert_eq!(worker.advance(1), Err(Error::Stale.into()));
    assert_eq!(worker.progress(), initial);
    assert!(worker.begin(&input(b"!")).is_err());
    assert!(worker.evaluate(&input(b"!")).is_err());
    assert_eq!(worker.progress(), initial);
    worker.advance(0).unwrap();
    let before = worker.progress();
    assert_eq!(worker.advance(0), Err(Error::Stale.into()));
    assert_eq!(worker.progress(), before);
    let done = finish(&mut worker).unwrap();
    for _ in 0..4 { assert_eq!(worker.advance(worker.position()), Ok(done)); }
    assert!(worker.begin(&input(b"!")).is_err());
    assert_eq!(worker.progress(), done);
}

#[test]
fn held_answer_or_held_terminal_never_produces_a_verdict() {
    for alarm in [1, 2] {
        let mut worker = evaluator(b"allow", alarm);
        worker.begin(&input(b"?")).unwrap();
        worker.advance(0).unwrap();
        if alarm == 2 {
            let partial = worker.advance(1).unwrap();
            assert_eq!(partial.status, NativeEvaluationStatus::Running);
            assert!(worker.report().is_none());
        }
        let error = NativeEvaluationError::Incomplete(GenerationFinish::Held);
        assert_eq!(worker.advance(worker.position()), Err(error));
        let after = worker.progress();
        assert_eq!(after.status, NativeEvaluationStatus::Failed(error));
        assert_eq!(after.work.sampled_draws, u64::from(alarm));
        assert_eq!(after.finish, Some(GenerationFinish::Held));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), if alarm == 1 { b"".as_slice() } else { b"allow" });
        assert_eq!(worker.advance(worker.position()), Err(error));
        assert_eq!(worker.progress(), after);
    }
}

#[test]
fn truncation_and_exhaustion_preserve_the_quiet_prefix_without_a_vote() {
    for limited in [false, true] {
        let (decoder, mut policy) = decoder(b"allow", 0);
        let end = if limited {
            policy.generation.sampling_entries = decoder.profile().shape().vocabulary as u64;
            GenerationFinish::BudgetExhausted
        } else { policy.max_new_tokens = 1; GenerationFinish::TokenLimit };
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        worker.begin(&input(b"?")).unwrap();
        assert_eq!(finish(&mut worker), Err(NativeEvaluationError::Incomplete(end)));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow");
        assert_eq!(worker.sampled_draws(), 1);
        let after = worker.progress();
        assert!(worker.evaluate(&input(b"?")).is_err());
        assert_eq!(worker.progress(), after);
    }
}

#[test]
fn strict_complete_output_reducer_is_not_replaced_by_prefix_matching() {
    let mut worker = evaluator(b"allow\n", 0);
    worker.begin(&input(b"?")).unwrap();
    assert_eq!(finish(&mut worker), Err(NativeEvaluationError::InvalidVerdict));
    assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow\n");
    assert_eq!(worker.progress().finish, Some(GenerationFinish::StopToken));
    assert_eq!(worker.sampled_draws(), 2);
}

#[test]
fn refused_full_input_or_profile_has_no_partial_numerical_session() {
    for mode in 0..3 {
        let (decoder, mut policy) = decoder(b"allow", 0);
        if mode == 1 { policy.tokenization.input_bytes = 1; }
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        let mut profile = expected(); if mode == 0 { profile.policy_epoch += 1; }
        let prompt = if mode == 2 { vec![b'?'; 1024] } else { b"??".to_vec() };
        let packet = decode_request(&frame(&prompt, &profile)).unwrap();
        let error = worker.begin(&packet).unwrap_err();
        if mode == 0 { assert_eq!(error, NativeEvaluationError::Contract(Error::Binding)); }
        else { assert!(matches!(error, NativeEvaluationError::Admission(_))); }
        assert_eq!(worker.work(), NativeEvaluationWork::default());
        assert!(worker.report().is_none());
        assert!(worker.begin(&input(b"?")).is_err());
        assert_eq!(worker.work(), NativeEvaluationWork::default());
    }
}

#[test]
fn cancellation_during_prefill_destroys_the_owner_and_keeps_actual_work() {
    let (decoder, policy) = decoder(b"allow", 0);
    let observer = decoder.observation();
    let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
    worker.begin(&input(b"????????")).unwrap();
    worker.advance(0).unwrap();
    let evidence = observer.capture().unwrap();
    let before = worker.progress();
    assert_eq!(before.reviewed_prompt_tokens, 1);
    assert_eq!(before.work.sampled_draws, 0);
    assert!(worker.cancel()); assert!(!worker.cancel());
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    assert_eq!(observer.validate(&evidence), Err(Error::Incomplete));
    assert_eq!(worker.work(), before.work);
    assert_eq!(worker.progress().reviewed_prompt_tokens, 1);
    assert_eq!(worker.progress().finish, None);
    assert_eq!(worker.status(), NativeEvaluationStatus::Cancelled);
    assert!(worker.report().is_none());
    assert!(worker.advance(1).is_err()); assert!(worker.evaluate(&input(b"?")).is_err());
    assert_eq!(worker.work(), before.work);
}

#[test]
fn cancellation_after_a_quiet_answer_does_not_complete_or_refund_it() {
    let mut worker = evaluator(b"allow", 0);
    worker.begin(&input(b"?")).unwrap();
    worker.advance(0).unwrap(); worker.advance(1).unwrap();
    assert_eq!(worker.progress().released_answer_tokens, 1);
    assert!(worker.report().is_none());
    let before = worker.work();
    assert!(worker.cancel());
    assert_eq!(worker.work(), before);
    assert_eq!(worker.sampled_draws(), 1);
    assert_eq!(worker.progress().finish, None);
    assert!(worker.report().is_none());
    assert!(worker.advance(2).is_err());
}

#[test]
fn caught_post_token_unwind_cannot_resume_or_erase_already_consumed_draws() {
    let (decoder, policy) = decoder(b"allow", 0);
    let observer = decoder.observation();
    let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
    worker.begin(&input(b"?")).unwrap(); worker.advance(0).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        worker.advance_with(1, || panic!("after actual native token, before verdict handling"))
    })).is_err());
    assert_eq!(worker.status(), NativeEvaluationStatus::Evaluating);
    assert_eq!(worker.sampled_draws(), 1);
    assert_eq!(worker.work().decoder.tokens, 2);
    assert!(worker.report().is_none());
    let before = worker.progress();
    assert!(worker.advance(2).is_err()); assert!(worker.begin(&input(b"!")).is_err());
    assert_eq!(worker.progress(), before);
    assert!(worker.cancel());
    assert_eq!(worker.progress(), before); // Preserve the interrupted diagnostic, not a clean cancellation.
    assert_eq!(observer.availability(), DecoderAvailability::Closed);
    assert!(!worker.cancel());
}

#[test]
fn cancelling_terminal_ownership_preserves_the_original_judgment_or_alarm() {
    for alarm in [0, 1] {
        let mut worker = evaluator(b"allow", alarm);
        let result = worker.evaluate(&input(b"?"));
        assert_eq!(result.is_ok(), alarm == 0);
        let before = worker.progress();
        let bytes = worker.report().unwrap().bytes().unwrap().to_vec();
        assert!(worker.cancel());
        assert_eq!(worker.progress(), before);
        assert_eq!(worker.report().unwrap().bytes().unwrap(), bytes);
        assert!(worker.evaluate(&input(b"!")).is_err());
    }
}

#[test]
fn independent_workers_can_interleave_without_sharing_prefixes_or_draws() {
    let mut a = evaluator(b"allow", 0); let mut b = evaluator(b"allow", 0);
    a.begin(&input(b"????????")).unwrap(); b.begin(&input(b"!")).unwrap();
    for position in 0..3 {
        let before = b.work();
        a.advance(position).unwrap();
        assert_eq!(b.work(), before);
        b.advance(position).unwrap();
    }
    assert_eq!(b.status(), NativeEvaluationStatus::Judged(Verdict::Deny));
    assert_eq!(a.status(), NativeEvaluationStatus::Running);
    assert_eq!(a.sampled_draws(), 0); assert_eq!(b.sampled_draws(), 2);
    let b_done = b.progress();
    assert_eq!(finish(&mut a).unwrap().status, NativeEvaluationStatus::Judged(Verdict::Allow));
    assert_eq!(b.progress(), b_done);
}
