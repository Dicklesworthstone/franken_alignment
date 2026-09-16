//! Differential stochastic rollouts against independently driven original APIs.
#[path = "support/investigation_decoder.rs"] mod fixture;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderLayerWeights, DecoderModel, DecoderWork};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::{DecoderExperimentArm, DecoderIntervention};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::{
    DecoderComparisonBudget, MAX_COMPARISON_LOGIT_VALUES, cursor::DecoderComparisonStatus,
    sampled::{DecoderSampledComparisonBudget, DecoderSampledComparisonCursor},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{Sampler, SamplingBudget, SamplingPolicy, SamplingStart};
use fa_reference::Error;
use std::collections::BTreeMap;

fn sampling(seed: u64, temperature: f64, top_k: usize, top_p: f64) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(1, 1, 2, temperature, top_k, top_p).unwrap(), stream: 99, seed }
}
fn budget() -> DecoderSampledComparisonBudget {
    DecoderSampledComparisonBudget { comparison: fixture::comparison_budget(), sampling_logits: 64 }
}
fn run(plan: &DecoderIntervention, count: usize, start: SamplingStart) -> DecoderSampledComparisonCursor {
    plan.begin_sampled_comparison(0, count, start, budget()).unwrap()
}
fn counts(w: DecoderWork) -> [u64; 8] {
    [w.tokens, w.matrix_products, w.attention_products, w.attention_exponentials,
        w.normalization_coordinates, w.rotary_pairs, w.gate_coordinates, w.cache_values_appended]
}
fn words(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }

#[test]
fn every_pair_matches_independent_original_sampler_and_decoder_execution() {
    for value in [0.0, 10.0] {
        let plan = fixture::plan(value);
        for seed in 0..8 {
            for (temperature, top_k, top_p) in [(0.25, 0, 1.0), (2.0, 0, 1.0), (1.0, 1, 1.0), (2.0, 0, 0.6)] {
                for count in 1..=3 {
                    let start = sampling(seed, temperature, top_k, top_p);
                    let mut cursor = run(&plan, count, start.clone());
                    // The oracle has its own loop and original sessions, not a
                    // second call to begin_sampled_comparison or its internals.
                    let mut left = plan.session(DecoderExperimentArm::Control);
                    let mut right = plan.session(DecoderExperimentArm::Intervention);
                    let mut a_rng = Sampler::seeded(start.policy.clone(), start.stream, start.seed).unwrap();
                    let mut b_rng = Sampler::seeded(start.policy.clone(), start.stream, start.seed).unwrap();
                    for offset in 0..count {
                        let (a_choice, b_choice) = if offset == 0 { (None, None) } else {
                            (Some(a_rng.sample(left.logits().unwrap(), SamplingBudget { vocabulary: 2 }).unwrap()),
                             Some(b_rng.sample(right.logits().unwrap(), SamplingBudget { vocabulary: 2 }).unwrap()))
                        };
                        let position = 1 + offset as u64;
                        let a = left.advance(position, a_choice.as_ref().map_or(0, |s| s.token), fixture::budget()).unwrap();
                        let b = right.advance(position, b_choice.as_ref().map_or(0, |s| s.token), fixture::budget()).unwrap();
                        assert_eq!(cursor.advance().unwrap(), DecoderComparisonStatus::Running);
                        assert_eq!(cursor.completed_pairs().len(), offset);
                        assert!(cursor.finish().is_err()); // No completed-control-only report.
                        cursor.advance().unwrap();
                        let pair = &cursor.completed_pairs()[offset];
                        assert_eq!(pair.control_choice, a_choice); assert_eq!(pair.intervention_choice, b_choice);
                        assert_eq!(pair.control.token, a.token); assert_eq!(pair.intervention.token, b.token);
                        assert_eq!(words(&pair.control.logits), words(&a.logits));
                        assert_eq!(words(&pair.intervention.logits), words(&b.logits));
                        let changed = a.logits.iter().zip(b.logits.iter()).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
                        let squared: f64 = a.logits.iter().zip(b.logits.iter()).map(|(a, b)| (f64::from(*b) - f64::from(*a)).powi(2)).sum();
                        assert_eq!(pair.changed_logit_words, changed); assert_eq!(pair.l2_logit_delta, squared.sqrt());
                        if let (Some(a), Some(b)) = (&pair.control_choice, &pair.intervention_choice) {
                            assert_eq!((a.draw, a.random_word), (b.draw, b.random_word));
                        }
                    }
                    let report = cursor.finish().unwrap(); let work = report.work();
                    for (actual, (a, b)) in counts(work.numerical.completed).into_iter()
                        .zip(counts(left.work()).into_iter().zip(counts(right.work()))) { assert_eq!(actual, a + b); }
                    assert_eq!(work.control_draws, (count - 1) as u64);
                    assert_eq!(work.intervention_draws, work.control_draws);
                    assert_eq!(work.sampling.logits_scanned, 4 * (count - 1));
                    assert_eq!(report.sampling_start().seed, seed);
                    assert_eq!(report.sampling_start().policy, start.policy);
                    assert_eq!(cursor.advance().unwrap(), DecoderComparisonStatus::Complete);
                    assert_eq!(cursor.finish().unwrap().work(), work);
                    if value == 0.0 {
                        assert_eq!(report.first_different_logits(), None);
                        assert_eq!(report.first_different_consumed_token(), None);
                    }
                }
            }
        }
    }
}

#[test]
fn real_value_interventions_change_sampled_paths_without_changing_the_random_words() {
    let mut different_paths = 0; let mut non_greedy_control = 0;
    for seed in 0..32 {
        let mut cursor = run(&fixture::plan(10.0), 3, sampling(seed, 2.0, 0, 1.0));
        while cursor.status() == DecoderComparisonStatus::Running { cursor.advance().unwrap(); }
        let report = cursor.finish().unwrap();
        assert_eq!(report.first_different_logits(), Some(1));
        different_paths += usize::from(report.first_different_consumed_token().is_some());
        for pair in &report.steps()[1..] {
            let a = pair.control_choice.as_ref().unwrap(); let b = pair.intervention_choice.as_ref().unwrap();
            assert_eq!(a.random_word, b.random_word); assert_eq!(a.draw, b.draw);
            non_greedy_control += usize::from(a.token == 1); // Baseline logits prefer token zero.
        }
    }
    assert!(different_paths > 0); assert!(non_greedy_control > 0);
    // Counts describe this fixed deterministic seed set, not detection rates.
}

#[test]
fn cancellation_at_every_boundary_preserves_unpaired_numerical_and_sampling_work() {
    for completed in 0_usize..6 {
        let mut cursor = run(&fixture::plan(10.0), 3, sampling(9, 2.0, 0, 1.0));
        for _ in 0..completed { cursor.advance().unwrap(); }
        let before = cursor.work().unwrap();
        assert_eq!(before.numerical.completed.tokens, completed as u64);
        assert_eq!(before.numerical.completed_pairs, completed / 2);
        assert_eq!(before.control_draws, ((completed + 1) / 2).saturating_sub(1) as u64);
        assert_eq!(before.intervention_draws, (completed / 2).saturating_sub(1) as u64);
        cursor.cancel().unwrap(); cursor.cancel().unwrap();
        assert_eq!(cursor.status(), DecoderComparisonStatus::Cancelled);
        assert_eq!(cursor.advance(), Err(Error::WrongState)); assert!(cursor.finish().is_err());
        assert_eq!(cursor.work().unwrap(), before);
    }
}

#[test]
fn arithmetic_failure_retains_entered_work_and_never_returns_a_partial_success() {
    let mut cursor = run(&fixture::plan(f32::MAX), 3, sampling(9, 2.0, 0, 1.0));
    cursor.advance().unwrap(); assert_eq!(cursor.advance(), Err(Error::Overflow));
    let work = cursor.work().unwrap();
    assert_eq!(work.numerical.entered_tokens, 2); assert_eq!(work.numerical.completed.tokens, 1);
    assert_eq!(work.numerical.completed_pairs, 0); assert_eq!(work.control_draws, 0);
    assert_eq!(cursor.status(), DecoderComparisonStatus::Failed(Error::Overflow));
    assert!(cursor.finish().is_err()); assert_eq!(cursor.advance(), Err(Error::WrongState));
    assert_eq!(cursor.work().unwrap(), work);
}

#[test]
fn a_draw_followed_by_numerical_failure_is_not_erased_or_called_a_consumed_token() {
    let q = 0.6 * f32::MAX;
    let model = DecoderModel::new(fixture::profile(), vec![1.0, 0.0, 1.0, 1.0], vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![q, q, 0.0, 0.0], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], vec![0.0, 0.0, 1.0, 0.0]).unwrap();
    let mut source = model.session(5).unwrap(); source.advance(0, 0, fixture::budget()).unwrap();
    let plan = source.checkpoint().unwrap().intervene(71, BTreeMap::new(), 0).unwrap();
    let mut cursor = run(&plan, 3, sampling(9, 1.0, 1, 1.0));
    cursor.advance().unwrap(); cursor.advance().unwrap(); // Shared forced zero.
    assert_eq!(cursor.advance(), Err(Error::Overflow)); // Sample selects token one; Q projection overflows.
    let work = cursor.work().unwrap();
    assert_eq!(work.numerical.completed_pairs, 1); assert_eq!(work.numerical.completed.tokens, 2);
    assert_eq!(work.numerical.entered_tokens, 3); assert_eq!(work.control_draws, 1);
    assert_eq!(work.intervention_draws, 0); assert_eq!(work.entered_sampling_calls, 1);
    assert_eq!(work.sampling.logits_scanned, 2); assert_eq!(work.sampling.exponentials, 1);
    assert!(cursor.finish().is_err()); assert_eq!(cursor.advance(), Err(Error::WrongState));
    assert_eq!(cursor.work().unwrap(), work); assert_eq!(source.tokens(), &[0]);
}

#[test]
fn entire_sampling_and_numerical_budgets_are_admitted_before_the_common_token() {
    let plan = fixture::plan(10.0);
    let terms = plan.source().model().estimate(1, 3).unwrap().scalar_products().unwrap() * 2;
    let exact = DecoderSampledComparisonBudget {
        comparison: DecoderComparisonBudget { scalar_products: terms, retained_logit_values: 12 }, sampling_logits: 8,
    };
    for field in 0..4 {
        let mut small = exact;
        match field {
            0 => small.comparison.scalar_products -= 1,
            1 => small.comparison.retained_logit_values -= 1,
            2 => small.sampling_logits -= 1,
            _ => small.sampling_logits = MAX_COMPARISON_LOGIT_VALUES + 1,
        }
        assert!(matches!(plan.begin_sampled_comparison(0, 3, sampling(0, 1.0, 0, 1.0), small), Err(Error::Limit)));
    }
    let mut valid = plan.begin_sampled_comparison(0, 3, sampling(0, 1.0, 0, 1.0), exact).unwrap();
    for _ in 0..6 { valid.advance().unwrap(); } assert!(valid.finish().is_ok());
    let mut first_only = budget(); first_only.sampling_logits = 0;
    let mut cursor = plan.begin_sampled_comparison(0, 1, sampling(0, 1.0, 0, 1.0), first_only).unwrap();
    cursor.advance().unwrap(); cursor.advance().unwrap();
    assert_eq!(cursor.finish().unwrap().work().entered_sampling_calls, 0);
    assert!(plan.begin_sampled_comparison(0, 0, sampling(0, 1.0, 0, 1.0), budget()).is_err());
    assert!(plan.begin_sampled_comparison(2, 1, sampling(0, 1.0, 0, 1.0), budget()).is_err());
    let mut wrong = sampling(0, 1.0, 0, 1.0); wrong.stream = 0;
    assert!(plan.begin_sampled_comparison(0, 1, wrong, budget()).is_err());
    let mut wrong = sampling(0, 1.0, 0, 1.0); wrong.policy = SamplingPolicy::new(1, 1, 3, 1.0, 0, 1.0).unwrap();
    assert!(matches!(plan.begin_sampled_comparison(0, 1, wrong, budget()), Err(Error::Binding)));
}
