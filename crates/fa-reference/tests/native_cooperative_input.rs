//! Public original-owner composition: complete binary input, causal tokenizer
//! budgets, real native judgments and the actual worker process consumer.
#![forbid(unsafe_code)]
#![cfg(unix)]

#[path = "support/native_worker_assets.rs"]
mod assets;
#[path = "native_worker_rotary/process.rs"]
mod process_fixture;
mod native_cooperative_input {
    pub mod fixture;
    mod peer;
    mod process;
    use super::assets;
}
use assets::Fixture;
use native_cooperative_input::fixture::{admit, finish, input, load, prompt, tokenizer};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationBudget;
use fa_reference::action::consequence::oversight::helper_client::native::{
    NativeEvaluationError, NativeEvaluationStatus, NativeEvaluationWork,
};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn cooperative_native_evaluator_preserves_complete_binary_input_and_original_inference() {
    let fixture = Fixture::new(false);
    for (last, verdict) in [(b'?', Verdict::Allow), (b'!', Verdict::Deny)] {
        let bytes = prompt(last);
        let packet = input(&bytes);
        assert_eq!(packet.actual_input().ordered_parts().len(), 3);
        assert_eq!(packet.actual_input().omissions().len(), 1);
        let encoded = tokenizer(&fixture).encode(&bytes, fixture.policy.tokenization).unwrap();
        assert_eq!(encoded.tokens().len(), 83);
        assert_eq!(encoded.work().merges, 320);
        let mut original = load(&fixture, &fixture.policy);
        let mut cooperative = load(&fixture, &fixture.policy);
        let admitted = original.begin(&packet).unwrap();
        assert_eq!(admitted.status, NativeEvaluationStatus::Running);
        assert_eq!(admitted.admission_steps, 0);
        assert_eq!(admitted.tokenization, encoded.work());
        assert_eq!(admitted.work, NativeEvaluationWork::default());

        let pending = cooperative.begin_cooperative(&packet).unwrap();
        assert_eq!(pending.status, NativeEvaluationStatus::Admitting);
        assert_eq!(pending.admission_steps, 0);
        assert_eq!(pending.tokenization.input_bytes, bytes.len());
        assert_eq!(pending.tokenization.pair_lookups, 0);
        assert_eq!(cooperative.input(), Some(&packet));
        assert_eq!(cooperative.advance(1), Err(NativeEvaluationError::Contract(Error::Stale)));
        assert_eq!(cooperative.progress(), pending);
        assert!(cooperative.begin_cooperative(&input(&prompt(b'!'))).is_err());
        assert_eq!(cooperative.progress(), pending);

        let complete = admit(&mut cooperative).unwrap();
        assert!(complete.admission_steps > 256);
        assert_eq!(complete.tokenization, encoded.work());
        assert_eq!(complete.requested_prompt_tokens, encoded.tokens().len());
        assert_eq!(complete.work, NativeEvaluationWork::default());
        let expected = finish(&mut original).unwrap();
        let result = finish(&mut cooperative).unwrap();
        assert_eq!(result.status, NativeEvaluationStatus::Judged(verdict));
        assert_eq!(result.work, expected.work);
        assert_eq!(result.tokenization, expected.tokenization);
        assert_eq!(result.reviewed_prompt_tokens, encoded.tokens().len());
        assert_eq!(result.work.decoder.tokens, encoded.tokens().len() as u64 + 2);
        assert_eq!(result.work.sampled_draws, 2);
        assert_eq!(result.finish, Some(GenerationFinish::StopToken));
        let report = cooperative.report().unwrap();
        let baseline = original.report().unwrap();
        assert_eq!(report.prompt().source(), bytes);
        assert_eq!(report.prompt().tokens(), encoded.tokens());
        assert_eq!(report.prompt().spans(), encoded.spans());
        assert_eq!(report.prompt().source(), baseline.prompt().source());
        assert_eq!(report.generation().work(), baseline.generation().work());
        assert_eq!(report.generation().last_review(), baseline.generation().last_review());
        assert_eq!(report.bytes(), baseline.bytes());
        assert!(report.prefix_controls().is_empty());
        assert_eq!(cooperative.input(), Some(&packet));
        assert_eq!(cooperative.advance(cooperative.position()), Ok(result));
    }
}

#[test]
fn cooperative_native_tokenizer_budgets_have_exact_success_and_one_short_refusals() {
    let fixture = Fixture::new(false);
    let bytes = prompt(b'?');
    let packet = input(&bytes);
    let tokenizer = tokenizer(&fixture);
    let work = tokenizer.encode(&bytes, fixture.policy.tokenization).unwrap().work();
    let exact = TokenizationBudget { input_bytes: bytes.len(),
        pair_lookups: work.pair_lookups, heap_pops: work.heap_pops };
    let mut policy = fixture.policy.clone();
    policy.tokenization = exact;
    let mut valid = load(&fixture, &policy);
    valid.begin_cooperative(&packet).unwrap();
    assert_eq!(admit(&mut valid).unwrap().tokenization, work);
    assert_eq!(finish(&mut valid).unwrap().status, NativeEvaluationStatus::Judged(Verdict::Allow));

    for budget in [
        TokenizationBudget { input_bytes: exact.input_bytes - 1, ..exact },
        TokenizationBudget { pair_lookups: exact.pair_lookups - 1, ..exact },
        TokenizationBudget { heap_pops: exact.heap_pops - 1, ..exact },
    ] {
        policy.tokenization = budget;
        let tokenizer_failure = tokenizer.encode(&bytes, budget).unwrap_err();
        let mut synchronous = load(&fixture, &policy);
        let expected = synchronous.begin(&packet).unwrap_err();
        let mut cooperative = load(&fixture, &policy);
        let error = match cooperative.begin_cooperative(&packet) {
            Err(error) => error,
            Ok(_) => admit(&mut cooperative).unwrap_err(),
        };
        assert_eq!(error, expected);
        assert_eq!(cooperative.progress().tokenization, tokenizer_failure.work);
        assert_eq!(cooperative.progress().tokenization, synchronous.progress().tokenization);
        assert_eq!(cooperative.work(), NativeEvaluationWork::default());
        assert_eq!(cooperative.status(), NativeEvaluationStatus::Failed(error));
        assert!(cooperative.report().is_none());
        let retained = cooperative.progress();
        assert_eq!(cooperative.advance(0), Err(error));
        assert!(cooperative.begin(&packet).is_err());
        assert!(cooperative.begin_cooperative(&packet).is_err());
        assert!(cooperative.evaluate(&packet).is_err());
        cooperative.cancel();
        assert_eq!(cooperative.progress(), retained);
    }
}

#[test]
fn cooperative_native_context_admission_uses_complete_tokenization_before_any_prefill() {
    let fixture = Fixture::new(false);
    for length in [1022, 1023] {
        let bytes = vec![b'?'; length];
        let packet = input(&bytes);
        let encoded = tokenizer(&fixture).encode(&bytes, fixture.policy.tokenization).unwrap();
        let mut synchronous = load(&fixture, &fixture.policy);
        let expected = synchronous.begin(&packet);
        let mut cooperative = load(&fixture, &fixture.policy);
        cooperative.begin_cooperative(&packet).unwrap();
        let observed = admit(&mut cooperative);
        if length == 1022 {
            assert_eq!(expected.unwrap().status, NativeEvaluationStatus::Running);
            assert_eq!(observed.unwrap().status, NativeEvaluationStatus::Running);
        } else {
            assert_eq!(observed.unwrap_err(), expected.unwrap_err());
            assert!(matches!(cooperative.status(), NativeEvaluationStatus::Failed(
                NativeEvaluationError::Admission(_))));
        }
        assert_eq!(cooperative.progress().tokenization, encoded.work());
        assert_eq!(cooperative.work(), NativeEvaluationWork::default());
        assert!(cooperative.report().is_none());
        let before = cooperative.progress();
        cooperative.cancel();
        assert_eq!(cooperative.progress().tokenization, before.tokenization);
        assert_eq!(cooperative.progress().admission_steps, before.admission_steps);
    }
}
