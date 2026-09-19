use super::*;
use super::super::tests::fixture;

fn request(prompt: &[u32], max_new_tokens: usize) -> GenerationRequest {
    GenerationRequest {
        prompt: prompt.to_vec(), max_new_tokens, stop_tokens: Vec::new(),
        budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS,
            sampling_entries: MAX_SAMPLING_ENTRIES },
    }
}

#[test]
fn completion_matches_original_stepwise_sampler_and_charges_the_whole_request() {
    let (mut run, budget) = fixture();
    let (mut original, _) = fixture();
    let report = run.generate(0, request(&[0], 2)).unwrap();
    original.advance_forced(0, 0, budget.decoder).unwrap();
    let mut expected = Vec::new();
    for position in 1..3 {
        let step = original.advance_sampled(position, budget).unwrap().into_monitored();
        let MonitoredStep::Released(step) = step else { panic!("quiet control"); };
        expected.push(step.step().token);
    }
    assert_eq!(report.tokens(), expected.as_slice());
    assert_eq!(report.finish(), GenerationFinish::TokenLimit);
    assert_eq!(report.reviewed_prompt_tokens(), 1);
    assert_eq!(report.start_position(), 0);
    assert_eq!(report.end_position(), 3);
    assert_eq!(run.sampled_draws(), 2);
    assert_eq!(report.work().attempted_samples, 2);
    assert_eq!(report.work().admitted_sampling_entries, 4);
    assert_eq!(report.work().admitted_scalar_products, run.decoder_work().scalar_products().unwrap());
    assert_eq!(run.decoder_work(), original.decoder_work());
    assert_eq!(run.monitoring_work(), original.monitoring_work());
    assert_eq!(report.last_review().unwrap().position(), 2);
}

#[test]
fn stop_token_is_reviewed_and_charged_but_never_returned_as_output() {
    let (mut run, _) = fixture();
    let mut input = request(&[0], 3);
    input.stop_tokens = vec![0, 1];
    let report = run.generate(0, input).unwrap();
    assert_eq!(report.finish(), GenerationFinish::StopToken);
    assert!(report.tokens().is_empty());
    assert_eq!(report.end_position(), 2);
    assert_eq!(report.reviewed_prompt_tokens(), 1);
    assert_eq!(report.work().attempted_samples, 1);
    assert_eq!(run.sampled_draws(), 1);
    assert_eq!(run.monitoring_work().frame_reviews, 2);
    assert_eq!(run.status(), MonitoringStatus::Ready);
}

#[test]
fn invalid_prompt_tail_stop_set_context_and_predecessor_are_atomic_refusals() {
    let (mut run, _) = fixture();
    let mut invalid_stop = request(&[0], 1);
    invalid_stop.stop_tokens = vec![2];
    let mut duplicate_stop = request(&[0], 1);
    duplicate_stop.stop_tokens = vec![0, 0];
    for (position, input, error) in [
        (0, request(&[0, 2], 1), Error::InvalidInput),
        (0, invalid_stop, Error::InvalidInput),
        (0, duplicate_stop, Error::Duplicate),
        (0, request(&[0], 4), Error::Limit),
        (1, request(&[0], 1), Error::Stale),
        (0, request(&[], 1), Error::InvalidInput),
    ] {
        assert_eq!(run.generate(position, input).unwrap_err(), error);
        assert_eq!(run.position(), 0);
        assert_eq!(run.sampled_draws(), 0);
        assert_eq!(run.decoder_work(), DecoderWork::default());
        assert_eq!(run.status(), MonitoringStatus::Ready);
    }
    assert_eq!(run.generate(0, request(&[0], 1)).unwrap().tokens().len(), 1);
}

#[test]
fn product_budget_is_shared_by_prefill_and_sampling_not_reset_per_step() {
    let (mut run, _) = fixture();
    let prefix_products = run.estimate(1).unwrap().scalar_products().unwrap();
    let mut input = request(&[0], 2);
    input.budget.scalar_products = prefix_products;
    let report = run.generate(0, input).unwrap();
    assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
    assert_eq!(report.reviewed_prompt_tokens(), 1);
    assert!(report.tokens().is_empty());
    assert_eq!(report.work().admitted_scalar_products, prefix_products);
    assert_eq!(report.work().admitted_sampling_entries, 0);
    assert_eq!(run.position(), 1);
    assert_eq!(run.sampled_draws(), 0);
    // A new explicitly budgeted request may continue the quiet prefix.
    assert_eq!(run.generate(1, request(&[], 1)).unwrap().tokens().len(), 1);
}

#[test]
fn sampling_budget_checks_the_entire_admission_before_charging_decoder_work() {
    for allowance in [1, 2] {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        let before = run.decoder_work();
        let mut input = request(&[], 1);
        input.budget.sampling_entries = allowance;
        let report = run.generate(1, input).unwrap();
        if allowance == 1 {
            assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
            assert_eq!(report.work(), GenerationWork::default());
            assert_eq!(run.decoder_work(), before);
            assert_eq!(run.sampled_draws(), 0);
        } else {
            assert_eq!(report.finish(), GenerationFinish::TokenLimit);
            assert_eq!(report.tokens().len(), 1);
            assert_eq!(report.work().admitted_sampling_entries, 2);
            assert_eq!(run.sampled_draws(), 1);
        }
    }
}

#[test]
fn a_held_sample_retains_its_draw_and_cannot_be_rerolled_or_exposed() {
    let (mut run, budget) = fixture();
    run.advance_forced(0, 0, budget.decoder).unwrap();
    let used = run.monitoring_work();
    run.monitored.budget = super::super::RefinementBudget {
        encoded_bytes: used.encoded_bytes, probe_coordinates: used.probe_coordinates,
    };
    let report = run.generate(1, request(&[], 2)).unwrap();
    assert_eq!(report.finish(), GenerationFinish::Held);
    assert!(report.tokens().is_empty());
    assert_eq!(report.end_position(), 2);
    assert_eq!(report.work().attempted_samples, 1);
    assert!(report.work().admitted_scalar_products > 0);
    assert_eq!(run.sampled_draws(), 1);
    assert_eq!(run.status(), MonitoringStatus::Held);
    assert_eq!(run.generate(2, request(&[], 1)).unwrap_err(), Error::WrongState);
    assert_eq!(run.sampled_draws(), 1);
}

#[test]
fn post_compute_error_returns_partial_accounting_without_a_token_or_old_quiet_review() {
    let (mut run, budget) = fixture();
    run.advance_forced(0, 0, budget.decoder).unwrap();
    run.monitored.work.frame_reviews = u64::MAX;
    let report = run.generate(1, request(&[], 2)).unwrap();
    assert_eq!(report.finish(), GenerationFinish::Failed(Error::Overflow));
    assert!(report.tokens().is_empty());
    assert!(report.last_review().is_none());
    assert_eq!(report.end_position(), 2);
    assert!(report.work().admitted_scalar_products > 0);
    assert_eq!(report.work().attempted_samples, 1);
    assert_eq!(run.sampled_draws(), 1);
    assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
}

#[test]
fn prefill_only_reviews_stop_ids_as_prompt_and_allows_later_continuation() {
    let (mut run, _) = fixture();
    let mut input = request(&[0, 1], 0);
    input.stop_tokens = vec![0, 1];
    let report = run.generate(0, input).unwrap();
    assert_eq!(report.finish(), GenerationFinish::TokenLimit);
    assert_eq!(report.reviewed_prompt_tokens(), 2);
    assert_eq!(report.work().attempted_samples, 0);
    assert!(report.tokens().is_empty());
    assert_eq!(run.position(), 2);
    assert_eq!(run.sampled_draws(), 0);
    assert_eq!(run.generate(2, request(&[], 2)).unwrap().tokens().len(), 2);
}

#[test]
fn full_prompt_product_boundary_refuses_before_any_quiet_partial_prefix() {
    // Two fixture tokens need 32 matrix products each, then 4 and 8
    // attention products: 76 total, independently of the admission helper.
    for allowance in [75, 76] {
        let (mut run, _) = fixture();
        assert_eq!(run.estimate(2).unwrap().scalar_products().unwrap(), 76);
        let mut input = request(&[0, 1], 1);
        input.budget.scalar_products = allowance;
        let result = run.generate(0, input);
        if allowance == 75 {
            assert_eq!(result.unwrap_err(), Error::Limit);
            assert_eq!(run.position(), 0);
            assert_eq!(run.decoder_work(), DecoderWork::default());
            assert_eq!(run.monitoring_work().frame_reviews, 0);
        } else {
            let report = result.unwrap();
            assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
            assert_eq!(report.requested_prompt_tokens(), 2);
            assert_eq!(report.reviewed_prompt_tokens(), 2);
            assert_eq!(report.work().admitted_scalar_products, 76);
            assert_eq!(run.position(), 2);
        }
        assert_eq!(run.sampled_draws(), 0);
    }
}
