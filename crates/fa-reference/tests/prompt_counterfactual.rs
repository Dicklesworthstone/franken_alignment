//! Real original decoder/codec controls, not deployment or causal-safety proof.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::Error;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel,
    experiment::prompt::{PromptArm, PromptComparisonBudget, PromptComparisonStatus,
        PromptEdit, PromptExperimentConfig, PromptIntervention},
    sampling::{SamplingPolicy, SamplingStart, monitored::{
        GenerationBudget, GenerationPhase, GenerationSpec, GenerationStatus, GenerationStop,
        GenerationTelemetryBudget,
    }},
};
use std::collections::BTreeSet;

fn config(model: &DecoderModel, prompt: Vec<u32>, mode: u8, top_k: usize, stop: bool) -> PromptExperimentConfig {
    PromptExperimentConfig { stream: 21, evaluation_origin: 201,
        original: GenerationSpec::new(prompt, 3, if stop { BTreeSet::from([2]) } else { BTreeSet::new() },
            SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy: fixture::policy(model, mode, 1), generation: GenerationBudget::default(),
        telemetry: GenerationTelemetryBudget::default() }
}
fn plan(model: &DecoderModel, config: PromptExperimentConfig, replacement: Vec<u32>) -> PromptIntervention {
    let expected = config.original.prompt().to_vec();
    model.prompt_intervention(1, config, PromptEdit { start: 0, expected, replacement }).unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|x| x.to_bits()).collect() }

#[test]
fn sham_matches_both_original_stochastic_engines_including_logits_and_random_words() {
    let model = fixture::model();
    let config = config(&model, vec![0, 1], 0, 3, false);
    let intervention = plan(&model, config.clone(), vec![0, 1]);
    let mut pair = intervention.begin(intervention.reservation()).unwrap();
    let mut oracle = model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
        config.original, config.policy, config.generation, config.telemetry).unwrap();
    while pair.status() == PromptComparisonStatus::Active {
        let step = pair.advance(pair.revision()).unwrap();
        let left = step.baseline.as_ref().unwrap();
        let right = step.treated.as_ref().unwrap();
        let original = oracle.advance(oracle.position()).unwrap();
        assert_eq!(left.token, right.token);
        assert_eq!(left.token, Some(original.accepted().unwrap().token));
        assert_eq!(bits(left.logits.as_ref().unwrap()), bits(right.logits.as_ref().unwrap()));
        assert_eq!(bits(left.logits.as_ref().unwrap()), bits(&original.accepted().unwrap().logits));
        match (&left.sample, &right.sample, original.sample()) {
            (Some(a), Some(b), Some(c)) => {
                assert_eq!(a, b); assert_eq!(a, c);
                assert_eq!(a.probability.to_bits(), b.probability.to_bits());
                assert_eq!(a.probability.to_bits(), c.probability.to_bits());
            }
            (None, None, None) => {}
            _ => panic!("inconsistent sampling phase"),
        }
    }
    let report = pair.report();
    assert_eq!(report.baseline, report.treated);
    assert_eq!(report.baseline.numerical, oracle.work());
    assert_eq!(report.baseline.telemetry, oracle.telemetry_work());
    assert_eq!(pair.generated_tokens(PromptArm::Baseline), oracle.generated_tokens());
    assert!(report.baseline.all_attempted_telemetry_reported);
    assert_eq!(report.baseline.status, GenerationStatus::Finished(GenerationStop::TokenLimit));
}

#[test]
fn a_single_source_token_changes_the_actual_sampled_continuation() {
    let model = fixture::model();
    let config = config(&model, vec![0], 0, 1, false);
    let intervention = plan(&model, config.clone(), vec![1]);
    let mut pair = intervention.begin(PromptComparisonBudget::default()).unwrap();
    pair.advance(0).unwrap();
    let step = pair.advance(1).unwrap();
    let a = step.baseline.as_ref().unwrap().sample.as_ref().unwrap();
    let b = step.treated.as_ref().unwrap().sample.as_ref().unwrap();
    assert_eq!((a.token, b.token), (2, 0));
    assert_eq!(a.random_word, b.random_word);
    assert_eq!(a.draw, b.draw);
    pair.run_to_stop().unwrap();
    for (arm, spec) in [(PromptArm::Baseline, &config.original), (PromptArm::Treated, intervention.treated_spec())] {
        let mut original = model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
            spec.clone(), config.policy.clone(), config.generation, config.telemetry).unwrap();
        original.run_to_stop().unwrap();
        assert_eq!(pair.generated_tokens(arm), original.generated_tokens());
    }
    assert_eq!(intervention.config().original.prompt(), &[0]);
    assert_eq!(intervention.treated_spec().prompt(), &[1]);
}

#[test]
fn unequal_prompt_lengths_pair_continuation_draws_not_absolute_positions() {
    let model = fixture::model();
    let intervention = plan(&model, config(&model, vec![0], 0, 3, false), vec![0, 1, 2]);
    let mut pair = intervention.begin(PromptComparisonBudget::default()).unwrap();
    let first = pair.advance(0).unwrap();
    assert!(first.baseline.as_ref().unwrap().sample.is_none());
    for revision in 1..3 {
        let step = pair.advance(revision).unwrap();
        assert!(step.baseline.is_none());
        assert_eq!(step.treated.as_ref().unwrap().phase, GenerationPhase::Prompt);
        assert!(pair.generated_tokens(PromptArm::Baseline).is_empty());
    }
    for draw in 1..=3 {
        let step = pair.advance(pair.revision()).unwrap();
        let a = step.baseline.as_ref().unwrap(); let b = step.treated.as_ref().unwrap();
        assert_eq!(a.position + 2, b.position);
        assert_eq!(a.sample.as_ref().unwrap().draw, draw);
        assert_eq!(a.sample.as_ref().unwrap().random_word, b.sample.as_ref().unwrap().random_word);
    }
    let report = pair.report();
    assert_eq!(report.baseline.attempted_calls, 4);
    assert_eq!(report.treated.attempted_calls, 6);
    assert_eq!(report.reservation.positions, 10);
    assert_eq!(report.status, PromptComparisonStatus::Stopped);
}

#[test]
fn edit_preconditions_are_exact_and_do_not_mutate_the_original_recipe() {
    let model = fixture::model();
    let original = config(&model, vec![0, 1, 2], 0, 1, false);
    let inserted = model.prompt_intervention(1, original.clone(),
        PromptEdit { start: 1, expected: vec![], replacement: vec![2] }).unwrap();
    assert_eq!(inserted.treated_spec().prompt(), &[0, 2, 1, 2]);
    let deleted = model.prompt_intervention(2, original.clone(),
        PromptEdit { start: 1, expected: vec![1], replacement: vec![] }).unwrap();
    assert_eq!(deleted.treated_spec().prompt(), &[0, 2]);
    for edit in [PromptEdit { start: 1, expected: vec![0], replacement: vec![2] },
        PromptEdit { start: 4, expected: vec![], replacement: vec![0] }] {
        assert!(matches!(model.prompt_intervention(3, original.clone(), edit), Err(Error::Binding)));
    }
    assert!(matches!(model.prompt_intervention(3, original.clone(),
        PromptEdit { start: usize::MAX, expected: vec![0], replacement: vec![] }), Err(Error::Overflow)));
    assert!(matches!(model.prompt_intervention(3, original.clone(),
        PromptEdit { start: 0, expected: vec![0, 1, 2], replacement: vec![] }), Err(Error::InvalidInput)));
    assert!(matches!(model.prompt_intervention(3, original.clone(),
        PromptEdit { start: 0, expected: vec![0], replacement: vec![99] }), Err(Error::InvalidInput)));
    assert_eq!(original.original.prompt(), &[0, 1, 2]);
}

#[test]
fn complete_paired_and_per_arm_budgets_are_required_before_inference() {
    let model = fixture::model();
    let original = config(&model, vec![0], 0, 3, false);
    let intervention = plan(&model, original.clone(), vec![0, 1]);
    let exact = intervention.reservation();
    intervention.begin(exact).unwrap().run_to_stop().unwrap();
    for short in [PromptComparisonBudget { positions: exact.positions - 1, ..exact },
        PromptComparisonBudget { decoder_products: exact.decoder_products - 1, ..exact },
        PromptComparisonBudget { vocabulary_scores: exact.vocabulary_scores - 1, ..exact }] {
        assert!(matches!(intervention.begin(short), Err(Error::Limit)));
    }
    let mut limited = original;
    limited.generation.decoder_products = model.estimate_monitored_generation(&limited.original).unwrap()
        .decoder.scalar_products().unwrap();
    // The treated prompt costs more; a generous paired cap cannot bypass its
    // ORIGINAL per-arm numerical ceiling.
    let longer = plan(&model, limited, vec![0, 1]);
    assert!(matches!(longer.begin(PromptComparisonBudget::default()), Err(Error::Limit)));
}

#[test]
fn a_held_arm_stays_censored_while_the_other_collects_its_own_outcome() {
    let model = fixture::model();
    let intervention = plan(&model, config(&model, vec![2], 2, 1, false), vec![1]);
    let mut pair = intervention.begin(PromptComparisonBudget::default()).unwrap();
    let first = pair.advance(0).unwrap();
    let held = first.baseline.as_ref().unwrap();
    assert!(matches!(held.status, GenerationStatus::Held(_)));
    assert!(held.token.is_none() && held.sample.is_none() && held.logits.is_none());
    assert!(first.treated.as_ref().unwrap().token.is_some());
    let next = pair.advance(1).unwrap();
    assert!(next.baseline.is_none());
    assert_eq!(next.treated.as_ref().unwrap().sample.as_ref().unwrap().token, 0);
    let report = pair.run_to_stop().unwrap();
    assert_eq!(report.baseline.attempted_calls, 1);
    assert_eq!(report.baseline.accepted_continuation_tokens, 0);
    assert!(matches!(report.baseline.status, GenerationStatus::Held(_)));
    assert!(report.treated.attempted_calls > 1);
    assert!(pair.generated_tokens(PromptArm::Baseline).is_empty());
    assert_eq!(pair.advance(pair.revision()).err(), Some(Error::WrongState));
}

#[test]
fn missing_audits_remain_failed_outcomes_instead_of_disappearing_from_the_pair() {
    let model = fixture::model();
    let mut original = config(&model, vec![0], 0, 1, false);
    original.telemetry.source_check_values = 0;
    let mut pair = plan(&model, original, vec![1]).begin(PromptComparisonBudget::default()).unwrap();
    let step = pair.advance(0).unwrap();
    for arm in [step.baseline.as_ref().unwrap(), step.treated.as_ref().unwrap()] {
        assert_eq!(arm.status, GenerationStatus::Failed(Error::Limit));
        assert!(arm.audit.is_none() && arm.token.is_none() && arm.sample.is_none());
    }
    let report = pair.report();
    assert_eq!(report.status, PromptComparisonStatus::Stopped);
    assert_eq!(report.baseline.attempted_calls, 1);
    assert_eq!(report.treated.attempted_calls, 1);
    assert!(!report.baseline.all_attempted_telemetry_reported);
    assert!(!report.treated.all_attempted_telemetry_reported);
    assert_eq!(pair.advance(1).err(), Some(Error::WrongState));
}

#[test]
fn independent_eos_and_stale_calls_cannot_become_fresh_rollouts() {
    let model = fixture::model();
    let mut pair = plan(&model, config(&model, vec![0], 0, 1, true), vec![1])
        .begin(PromptComparisonBudget::default()).unwrap();
    let initial = pair.report();
    assert_eq!(pair.advance(1).err(), Some(Error::Stale));
    assert_eq!(pair.report(), initial);
    pair.advance(0).unwrap();
    let after_prompt = pair.report();
    assert_eq!(pair.advance(0).err(), Some(Error::Stale));
    assert_eq!(pair.report(), after_prompt);
    pair.advance(1).unwrap();
    assert_eq!(pair.report().baseline.status, GenerationStatus::Finished(GenerationStop::StopToken(2)));
    let later = pair.advance(2).unwrap();
    assert!(later.baseline.is_none());
    let final_report = pair.run_to_stop().unwrap();
    assert_eq!(final_report.baseline.accepted_continuation_tokens, 1);
    assert!(final_report.treated.accepted_continuation_tokens > 1);
    assert_eq!(pair.run_to_stop().unwrap(), final_report);
    assert_eq!(pair.advance(pair.revision()).err(), Some(Error::WrongState));
}
