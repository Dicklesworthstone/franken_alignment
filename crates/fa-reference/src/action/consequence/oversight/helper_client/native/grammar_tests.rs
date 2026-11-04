//! Actual native numerical owners; these are plumbing tests, not calibration.
use super::*;
use super::tests::{decoder, evaluator, input};

#[test]
fn fixed_grammar_produces_all_four_original_monitored_verdicts() {
    for (word, verdict) in [(b"allow".as_slice(), Verdict::Allow), (b"hold", Verdict::Hold),
        (b"deny", Verdict::Deny), (b"abstain", Verdict::Abstain)] {
        let mut worker = evaluator(word, 0).with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
        let request = input(b"\x00exact\xff?");
        assert_eq!(worker.evaluate(&request), Ok(verdict));
        assert_eq!(worker.verdict_grammar(), NativeVerdictGrammar::FiniteWordsV1);
        assert_eq!(worker.input(), Some(&request));
        assert_eq!(worker.report().unwrap().prompt().source(), request.actual_input().submitted_bytes());
        assert!(worker.report().unwrap().prefix_controls().is_empty());
        assert_eq!(worker.report().unwrap().bytes().unwrap(), word);
        assert_eq!(worker.sampled_draws(), 2);
        assert_eq!(worker.report().unwrap().generation().finish(), GenerationFinish::StopToken);
    }
}

#[test]
fn grammar_keeps_input_dependence_and_original_complete_prompt_review() {
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let mut worker = evaluator(b"allow", 0).with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
        worker.begin_cooperative(&input(prompt)).unwrap();
        while !matches!(worker.status(), NativeEvaluationStatus::Judged(_)) {
            worker.advance(worker.position()).unwrap();
        }
        assert_eq!(worker.status(), NativeEvaluationStatus::Judged(verdict));
        assert!(worker.progress().admission_steps > 0);
        let report = worker.report().unwrap().generation();
        assert_eq!(report.requested_prompt_tokens(), report.reviewed_prompt_tokens());
        assert_eq!(report.work().admitted_sampling_entries, 2 * worker.policy().decoder_profile.shape().vocabulary as u64);
    }
}

#[test]
fn invalid_highest_logit_is_excluded_while_multi_token_verdict_is_really_computed() {
    let (decoder, mut policy) = decoder(b"allow\n", 0); policy.max_new_tokens = 8;
    let mut constrained = NativeEvaluator::new(decoder, policy).unwrap()
        .with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
    // The model's preferred token contains a newline. Among permitted zero-score
    // ties, the ORIGINAL token-ID tie rule spells abstain from byte singletons.
    assert_eq!(constrained.evaluate(&input(b"?")), Ok(Verdict::Abstain));
    let report = constrained.report().unwrap();
    assert_eq!(report.bytes().unwrap(), b"abstain");
    let expected: Vec<_> = b"abstain".iter().map(|b| u32::from(*b)).collect();
    assert_eq!(report.generation().tokens(), expected.as_slice());
    assert_eq!(constrained.sampled_draws(), 8); assert_eq!(constrained.position(), 9);
    assert_eq!(report.generation().work().admitted_sampling_entries,
        8 * constrained.policy().decoder_profile.shape().vocabulary as u64);
    let mut legacy = evaluator(b"allow\n", 0);
    assert_eq!(legacy.evaluate(&input(b"?")), Err(NativeEvaluationError::InvalidVerdict));
    assert_eq!(legacy.report().unwrap().bytes().unwrap(), b"allow\n");
}

#[test]
fn constrained_answer_or_terminal_monitor_hold_never_yields_a_vote_or_reroll() {
    for alarm in [1, 2] {
        let mut worker = evaluator(b"allow", alarm).with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(GenerationFinish::Held)));
        assert_eq!(worker.sampled_draws(), u64::from(alarm));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), if alarm == 1 { b"".as_slice() } else { b"allow" });
        let position = worker.position();
        assert!(worker.advance(position).is_err()); assert!(worker.evaluate(&input(b"!")).is_err());
        assert_eq!(worker.position(), position); assert_eq!(worker.sampled_draws(), u64::from(alarm));
    }
}

#[test]
fn constrained_complete_word_without_a_reviewed_terminal_remains_incomplete() {
    for limited in [false, true] {
        let (decoder, mut policy) = decoder(b"allow", 0);
        let finish = if limited {
            policy.generation.sampling_entries = decoder.profile().shape().vocabulary as u64;
            GenerationFinish::BudgetExhausted
        } else { policy.max_new_tokens = 1; GenerationFinish::TokenLimit };
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap()
            .with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
        assert_eq!(worker.evaluate(&input(b"?")), Err(NativeEvaluationError::Incomplete(finish)));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), b"allow");
        assert_eq!(worker.sampled_draws(), 1);
    }
}

#[test]
fn grammar_selection_cannot_change_after_input_admission_or_installation() {
    let mut worker = evaluator(b"allow", 0); worker.begin(&input(b"?")).unwrap();
    assert_eq!(worker.with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap_err(), Error::WrongState);
    let worker = evaluator(b"allow", 0).with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1).unwrap();
    assert_eq!(worker.with_verdict_grammar(NativeVerdictGrammar::Unrestricted).unwrap_err(), Error::WrongState);
    let mut legacy = evaluator(b"allow\n", 0).with_verdict_grammar(NativeVerdictGrammar::Unrestricted).unwrap();
    assert_eq!(legacy.evaluate(&input(b"?")), Err(NativeEvaluationError::InvalidVerdict));
    assert_eq!(legacy.report().unwrap().bytes().unwrap(), b"allow\n");
}
