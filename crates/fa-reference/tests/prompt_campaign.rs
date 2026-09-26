//! Fixed-schedule tests use the original decoder, fitted codec and monitor.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::Error;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel,
    experiment::prompt::{PromptEdit, PromptExperimentConfig, PromptIntervention,
        campaign::{OutcomeClass, PromptCampaignBudget, PromptCampaignStatus, TokenObservation,
            TrialDisposition, MAX_PROMPT_TRIALS}},
    sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec,
        GenerationStatus, GenerationTelemetryBudget}},
};
use std::collections::BTreeSet;

fn plan(model: &DecoderModel, mode: u8, top_k: usize, horizon: usize) -> PromptIntervention {
    let config = PromptExperimentConfig { stream: 21, evaluation_origin: 201,
        original: GenerationSpec::new(vec![0], horizon, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 999 }).unwrap(),
        policy: fixture::policy(model, mode, 1), generation: GenerationBudget::default(),
        telemetry: GenerationTelemetryBudget::default() };
    model.prompt_intervention(9, config, PromptEdit { start: 0, expected: vec![0], replacement: vec![1] }).unwrap()
}
fn independent_outcome(tokens: &[u32], needle: &[u32], status: GenerationStatus) -> TokenObservation {
    match tokens.windows(needle.len()).position(|window| window == needle) {
        Some(offset) => TokenObservation::Present { first_generated_offset: offset },
        None if matches!(status, GenerationStatus::Finished(_)) => TokenObservation::AbsentWithinCompletedHorizon,
        None => TokenObservation::Censored { stopped: status },
    }
}

#[test]
fn every_seed_matches_two_separate_original_generators_and_the_frozen_question() {
    let model = fixture::model();
    let plan = plan(&model, 0, 3, 4);
    let seeds = vec![3, 17, 0, 81, 999];
    let needle = vec![2, 0];
    let mut campaign = plan.begin_campaign(44, seeds.clone(), needle.clone(), PromptCampaignBudget::default()).unwrap();
    let initial = campaign.progress();
    assert_eq!(initial.started_trials, 0);
    assert_eq!(initial.unstarted_trials, seeds.len());
    let progress = campaign.run_to_stop().unwrap();
    assert_eq!(progress.status, PromptCampaignStatus::Finished);
    assert_eq!(progress.declared_trials, seeds.len());
    assert_eq!(progress.recorded_trials, seeds.len());
    assert_eq!(progress.outcomes.observed_pairs(), seeds.len());
    assert_eq!(progress.unstarted_trials + progress.unrecorded_started_trials, 0);
    assert_eq!(campaign.seeds(), seeds);
    assert_eq!(campaign.question(), needle);
    assert!(progress.outcome_comparisons <= campaign.reservation().outcome_comparisons);
    for (trial, seed) in campaign.trials().iter().zip(&seeds) {
        assert_eq!(trial.seed(), *seed);
        assert_eq!(trial.disposition(), TrialDisposition::Observed);
        let report = trial.comparison().unwrap();
        let outcomes = trial.outcomes().unwrap();
        for (spec, observed, row) in [(&plan.config().original, outcomes.0, report.baseline),
            (plan.treated_spec(), outcomes.1, report.treated)] {
            let mut sampling = spec.sampling().clone();
            sampling.seed = *seed;
            let spec = GenerationSpec::new(spec.prompt().to_vec(), spec.max_new_tokens(),
                spec.stop_tokens().clone(), sampling).unwrap();
            let config = plan.config();
            let mut oracle = model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
                spec, config.policy.clone(), config.generation, config.telemetry).unwrap();
            oracle.run_to_stop().unwrap();
            assert_eq!(observed, independent_outcome(oracle.generated_tokens(), &needle, oracle.status()));
            assert_eq!(row.numerical, oracle.work());
            assert_eq!(row.telemetry, oracle.telemetry_work());
            assert_eq!(row.status, oracle.status());
        }
    }
    assert_eq!(plan.config().original.sampling().seed, 999);
    assert_eq!(campaign.run_to_stop().unwrap(), progress);
}

#[test]
fn a_real_source_effect_is_counted_without_conflating_paired_seeds_with_independent_tasks() {
    let model = fixture::model();
    let plan = plan(&model, 0, 1, 1);
    let seeds = vec![0, 1, 7, u64::MAX];
    let exact = plan.estimate_campaign(seeds.len(), 1).unwrap();
    let mut campaign = plan.begin_campaign(1, seeds, vec![2], exact).unwrap();
    let report = campaign.run_to_stop().unwrap();
    assert_eq!(report.outcomes.count(OutcomeClass::Present, OutcomeClass::Absent), 4);
    assert_eq!(report.outcomes.observed_pairs(), 4);
    assert_eq!(report.admission_failures + report.interrupted_trials, 0);
    assert_eq!(report.outcome_comparisons, exact.outcome_comparisons);
    for trial in campaign.trials() {
        assert_eq!(trial.outcomes(), Some((TokenObservation::Present { first_generated_offset: 0 },
            TokenObservation::AbsentWithinCompletedHorizon)));
        assert_eq!(trial.comparison().unwrap().baseline.accepted_continuation_tokens, 1);
        assert_eq!(trial.comparison().unwrap().treated.accepted_continuation_tokens, 1);
    }
    assert_eq!(campaign.plan().config().evaluation_origin, 201);
}

#[test]
fn held_and_failed_runs_are_censored_not_absent_and_are_never_omitted() {
    let model = fixture::model();
    for missing in [false, true] {
        let initial = plan(&model, 2, 1, 3);
        let mut config = initial.config().clone();
        if missing { config.telemetry.source_check_values = 0; }
        let plan = model.prompt_intervention(9, config, initial.edit().clone()).unwrap();
        let mut campaign = plan.begin_campaign(1, vec![1, 2, 3], vec![0], PromptCampaignBudget::default()).unwrap();
        let progress = campaign.run_to_stop().unwrap();
        assert_eq!(progress.recorded_trials, 3);
        assert_eq!(progress.outcomes.observed_pairs(), 3);
        assert_eq!(progress.admission_failures, 0);
        for trial in campaign.trials() {
            let (a, b) = trial.outcomes().unwrap();
            let report = trial.comparison().unwrap();
            if missing {
                assert_eq!(a, TokenObservation::Censored { stopped: GenerationStatus::Failed(Error::Limit) });
                assert_eq!(b, a);
                assert!(!report.baseline.all_attempted_telemetry_reported);
                assert!(!report.treated.all_attempted_telemetry_reported);
            } else {
                assert!(matches!(a, TokenObservation::Censored { stopped: GenerationStatus::Held(_) }));
                // The treated arm really released token 0 before any later hold.
                assert_eq!(b, TokenObservation::Present { first_generated_offset: 0 });
            }
        }
        assert_eq!(progress.outcomes.count(OutcomeClass::Absent, OutcomeClass::Absent), 0);
    }
}

#[test]
fn trial_admission_failures_keep_their_seed_slots_and_cannot_be_rerolled() {
    let model = fixture::model();
    let initial = plan(&model, 0, 3, 3);
    let mut config = initial.config().clone();
    config.generation.decoder_products = 0;
    let invalid = model.prompt_intervention(9, config, initial.edit().clone()).unwrap();
    let mut campaign = invalid.begin_campaign(1, vec![7, 8], vec![2], PromptCampaignBudget::default()).unwrap();
    let first = campaign.advance(0).unwrap();
    assert_eq!(first.started_trials, 1);
    assert_eq!(first.admission_failures, 1);
    assert_eq!(campaign.advance(0), Err(Error::Stale));
    assert_eq!(campaign.progress(), first);
    let end = campaign.run_to_stop().unwrap();
    assert_eq!(end.recorded_trials, 2);
    assert_eq!(end.admission_failures, 2);
    assert_eq!(end.outcomes.observed_pairs(), 0);
    assert_eq!(campaign.trials().iter().map(|trial| trial.seed()).collect::<Vec<_>>(), vec![7, 8]);
    for trial in campaign.trials() {
        assert_eq!(trial.disposition(), TrialDisposition::AdmissionFailed(Error::Limit));
        assert!(trial.outcomes().is_none() && trial.comparison().is_none());
    }
    assert_eq!(campaign.advance(end.revision), Err(Error::WrongState));
}

#[test]
fn full_schedule_costs_and_query_costs_are_admitted_before_the_first_trial() {
    let model = fixture::model();
    let plan = plan(&model, 0, 3, 4);
    let exact = plan.estimate_campaign(3, 2).unwrap();
    plan.begin_campaign(1, vec![1, 2, 3], vec![2, 0], exact).unwrap().run_to_stop().unwrap();
    for short in [PromptCampaignBudget { trials: exact.trials - 1, ..exact },
        PromptCampaignBudget { positions: exact.positions - 1, ..exact },
        PromptCampaignBudget { decoder_products: exact.decoder_products - 1, ..exact },
        PromptCampaignBudget { vocabulary_scores: exact.vocabulary_scores - 1, ..exact },
        PromptCampaignBudget { outcome_comparisons: exact.outcome_comparisons - 1, ..exact }] {
        assert!(matches!(plan.begin_campaign(1, vec![1, 2, 3], vec![2, 0], short), Err(Error::Limit)));
    }
    let default = PromptCampaignBudget::default();
    assert!(matches!(plan.begin_campaign(1, vec![7, 7], vec![2], default), Err(Error::Duplicate)));
    assert!(matches!(plan.begin_campaign(1, vec![], vec![2], default), Err(Error::InvalidInput)));
    assert!(matches!(plan.begin_campaign(1, vec![7], vec![], default), Err(Error::InvalidInput)));
    assert!(matches!(plan.begin_campaign(1, vec![7], vec![99], default), Err(Error::InvalidInput)));
    assert!(matches!(plan.begin_campaign(1, (0..=MAX_PROMPT_TRIALS as u64).collect(), vec![2], default), Err(Error::Limit)));
    assert!(matches!(plan.begin_campaign(1, vec![7], vec![2; 65], default), Err(Error::Limit)));
    assert!(matches!(plan.begin_campaign(0, vec![7], vec![2], default), Err(Error::InvalidInput)));
}

#[test]
fn cancellation_records_partial_work_and_leaves_future_seeds_unobserved() {
    let model = fixture::model();
    let plan = plan(&model, 0, 3, 4);
    let mut campaign = plan.begin_campaign(1, vec![7, 8, 9], vec![2], PromptCampaignBudget::default()).unwrap();
    let after = campaign.advance(0).unwrap();
    assert_eq!(after.started_trials, 1);
    assert_eq!(after.recorded_trials, 0);
    assert_eq!(after.unrecorded_started_trials, 1);
    assert_eq!(campaign.active_report().unwrap().baseline.numerical.admitted_tokens, 1);
    assert_eq!(campaign.cancel(0), Err(Error::Stale));
    assert_eq!(campaign.progress(), after);
    let cancelled = campaign.cancel(after.revision).unwrap();
    assert_eq!(cancelled.status, PromptCampaignStatus::Cancelled);
    assert_eq!(cancelled.recorded_trials, 1);
    assert_eq!(cancelled.interrupted_trials, 1);
    assert_eq!(cancelled.unstarted_trials, 2);
    assert_eq!(cancelled.outcomes.observed_pairs(), 0);
    let trial = &campaign.trials()[0];
    assert_eq!(trial.seed(), 7);
    assert_eq!(trial.disposition(), TrialDisposition::Interrupted(Error::Incomplete));
    assert_eq!(trial.comparison().unwrap().baseline.numerical.admitted_tokens, 1);
    assert!(trial.outcomes().is_none());
    assert!(campaign.active_report().is_none());
    assert_eq!(campaign.advance(cancelled.revision), Err(Error::WrongState));
    assert_eq!(campaign.run_to_stop().unwrap(), cancelled);
}
