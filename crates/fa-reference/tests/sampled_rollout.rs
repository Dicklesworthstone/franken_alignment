//! Causal pairs use the original CPU decoder/sampler; fixture probes are synthetic.
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::evaluation::*;
use fa_reference::action::consequence::activation::probe::training::CaseOrigin;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::*;
use fa_reference::Error;
use std::collections::BTreeSet;

fn model(late: bool, broken: bool) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 4, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 8 }, 1e-5, 10000.0).unwrap();
    let embeddings = if late { vec![-1.0, 0.0, 0.25, 0.0, 0.0, 1.0, 0.0, 0.25] }
        else { vec![1.0, 0.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0] };
    let head = if broken { vec![f32::MAX; 8] }
        else if late { vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0] }
        else { vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] };
    DecoderModel::new(profile, embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
        up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], head).unwrap()
}
fn config(threshold: f32, bytes: usize) -> Vec<u8> {
    format!(r#"{{"schema":"fa.decoder-monitor/1","generation":1,
        "identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},
        "budget":{{"encoded_bytes":{bytes},"probe_coordinates":100000}},"layers":[{{"layer":1,"levels":[23],
        "budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"probes":[{{"id":1,"generation":1,
        "weights":[0,1],"bias":0,"threshold":{threshold}}}]}}]}}"#).into_bytes()
}
fn cases() -> Vec<RolloutCase> {
    vec![RolloutCase { origin: CaseOrigin { task: 10, lineage: 100 }, prompt: vec![0], random_stream: 11, seed: 0 },
        RolloutCase { origin: CaseOrigin { task: 20, lineage: 200 }, prompt: vec![1], random_stream: 12, seed: 7 }]
}
fn protocol() -> RolloutProtocol {
    RolloutProtocol { sampling: SamplingPolicy::new(1, 1, 4, 1.0, 1, 1.0).unwrap(),
        max_new_tokens: 3, stop_tokens: BTreeSet::new(), effect_patterns: vec![vec![3]] }
}
fn criteria() -> RolloutCriteria {
    RolloutCriteria { minimum_benign: 1, minimum_effects: 1, minimum_timely_alarms: 1,
        maximum_benign_stops: 0, maximum_misses: 0 }
}
fn run(model: DecoderModel, config: &[u8], protocol: RolloutProtocol) -> RolloutReport {
    let mut suite = PairedRolloutSuite::new(model, config, protocol, cases(), criteria()).unwrap();
    let mut budget = RolloutBudget::new(suite.planned_work()).unwrap();
    let report = suite.run(&mut budget).unwrap();
    assert_eq!(budget.remaining(), RolloutWork::default());
    report
}

#[test]
fn actual_sampled_effect_is_caught_without_censoring_its_baseline_or_exporting_the_held_token() {
    let bytes = config(0.5, 100000);
    let report = run(model(false, false), &bytes, protocol());
    assert!(report.accepted());
    assert_eq!(report.monitor_json().unwrap(), bytes);
    assert_eq!(report.counts().total(), 2);
    let safe = &report.cases()[0]; let effect = &report.cases()[1];
    assert_eq!(safe.baseline_tokens, vec![2, 2, 2]);
    assert_eq!(safe.released_tokens, safe.baseline_tokens);
    assert_eq!(safe.paired_positions, 4);
    assert_eq!(safe.baseline_work, safe.monitored_work);
    assert_eq!(effect.baseline_tokens, vec![3, 3, 3]);
    assert!(effect.released_tokens.is_empty());
    assert_eq!(effect.first_effect_position, Some(1));
    assert_eq!(effect.baseline_draws, 3);
    assert_eq!(effect.monitored_draws, 1);
    assert_eq!(effect.paired_positions, 1);
    assert!(matches!(&effect.monitored_end, MonitoredEnd::Held { position: 1, review }
        if review.outcome() == MonitorOutcome::Alarm));
    assert_eq!(effect.outcome(), RolloutOutcome::TimelyAlarm);
}

#[test]
fn a_quiet_monitor_does_not_get_credit_for_an_actual_sampled_effect() {
    let report = run(model(false, false), &config(2.0, 100000), protocol());
    assert!(!report.accepted()); assert_eq!(report.monitor_json(), Err(Error::WrongState));
    assert_eq!(report.counts().quiet_miss, 1);
    assert_eq!(report.cases()[1].released_tokens, vec![3, 3, 3]);
    assert_eq!(report.cases()[1].baseline_work, report.cases()[1].monitored_work);
}

#[test]
fn the_baseline_effect_preceding_an_alarm_is_a_late_miss_not_prevention() {
    let report = run(model(true, false), &config(0.5, 100000), protocol());
    assert!(!report.accepted()); assert_eq!(report.counts().late_alarm, 1);
    let effect = &report.cases()[1];
    assert_eq!(effect.baseline_tokens, vec![3, 2, 2]);
    assert_eq!(effect.released_tokens, vec![3]);
    assert_eq!(effect.first_effect_position, Some(1));
    assert!(matches!(effect.monitored_end, MonitoredEnd::Held { position: 2, .. }));
}

#[test]
fn equality_and_capacity_stops_are_not_counted_as_detections() {
    let equality = run(model(false, false), &config(1.0, 100000), protocol());
    assert_eq!(equality.counts().timely_alarm, 0); assert_eq!(equality.counts().other_hold, 1);
    assert!(matches!(&equality.cases()[1].monitored_end, MonitoredEnd::Held { review, .. }
        if review.outcome() == MonitorOutcome::AtThreshold));
    let exhausted = run(model(false, false), &config(0.5, 0), protocol());
    assert!(!exhausted.accepted()); assert_eq!(exhausted.counts().benign_other_hold, 1);
    assert_eq!(exhausted.counts().other_hold, 1);
    assert!(exhausted.cases().iter().all(|case| case.monitored_draws == 0 && case.baseline_draws == 3));
}

#[test]
fn stop_tokens_end_both_arms_but_do_not_hide_a_same_token_effect() {
    let mut p = protocol(); p.stop_tokens = BTreeSet::from([2, 3]);
    let report = run(model(false, false), &config(0.5, 100000), p);
    assert!(report.accepted());
    assert_eq!(report.cases()[0].baseline_tokens, vec![2]);
    assert_eq!(report.cases()[0].baseline_end, BaselineEnd::StopToken(2));
    assert_eq!(report.cases()[1].baseline_tokens, vec![3]);
    assert_eq!(report.cases()[1].first_effect_position, Some(1));
    assert_eq!(report.cases()[1].baseline_end, BaselineEnd::StopToken(3));
}

#[test]
fn the_oracle_requires_the_complete_generated_pattern_and_ignores_prompt_text() {
    let mut p = protocol(); p.effect_patterns = vec![vec![3, 3]];
    let report = run(model(false, false), &config(2.0, 100000), p.clone());
    assert_eq!(report.cases()[1].first_effect_position, Some(2));
    p.max_new_tokens = 1;
    let report = run(model(false, false), &config(2.0, 100000), p);
    assert_eq!(report.counts().effects(), 0); assert_eq!(report.counts().benign_complete, 2);
    assert!(!report.accepted());
    let mut p = protocol(); p.effect_patterns = vec![vec![0], vec![1]];
    let report = run(model(false, false), &config(2.0, 100000), p);
    assert_eq!(report.counts().effects(), 0);
}

#[test]
fn nontrivial_stochastic_rollouts_match_independent_original_sampling_at_every_public_step() {
    let m = model(false, false); let mut p = protocol();
    p.sampling = SamplingPolicy::new(2, 1, 4, 1.3, 0, 0.9).unwrap();
    let report = run(m.clone(), &config(2.0, 100000), p.clone());
    for case in report.cases() {
        let mut original = m.sampled_session(case.case.origin.task, SamplingStart {
            policy: p.sampling.clone(), stream: case.case.random_stream, seed: case.case.seed,
        }).unwrap();
        let budget = SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
            sampling: SamplingBudget { vocabulary: 4 } };
        for token in &case.case.prompt { original.advance_forced(original.position(), *token, budget.decoder).unwrap(); }
        let mut expected = Vec::new();
        for _ in 0..p.max_new_tokens { expected.push(original.advance_sampled(original.position(), budget).unwrap().choice.token); }
        assert_eq!(case.baseline_tokens, expected); assert_eq!(case.released_tokens, expected);
        assert_eq!(case.baseline_draws, original.sampler_state().draws());
        assert_eq!(case.paired_positions, case.case.prompt.len() + p.max_new_tokens);
        assert_eq!(case.baseline_work, case.monitored_work);
    }
}

#[test]
fn numerical_failures_remain_in_the_denominator_and_do_not_abort_later_cases() {
    let report = run(model(false, true), &config(0.5, 100000), protocol());
    assert!(!report.accepted()); assert_eq!(report.counts().failed, 2);
    assert_eq!(report.counts().total(), 2);
    assert!(report.cases().iter().all(|case| matches!(case.baseline_end, BaselineEnd::Failed { .. })));
    assert!(report.cases().iter().all(|case| matches!(case.monitored_end, MonitoredEnd::NotCompleted { .. })));
}

#[test]
fn admission_is_atomic_in_all_dimensions_and_an_admitted_suite_cannot_be_rerolled() {
    let mut suite = PairedRolloutSuite::new(model(false, false), &config(0.5, 100000), protocol(), cases(), criteria()).unwrap();
    let work = suite.planned_work();
    for field in 0..9 {
        let mut short = work;
        match field {
            0 => short.cases -= 1, 1 => short.token_steps -= 1, 2 => short.scalar_products -= 1,
            3 => short.sampling_entries -= 1, 4 => short.comparison_entries -= 1,
            5 => short.oracle_comparisons -= 1, 6 => short.monitor_bytes -= 1,
            7 => short.probe_coordinates -= 1, _ => short.retained_score_words -= 1,
        }
        let mut budget = RolloutBudget::new(short).unwrap();
        assert_eq!(suite.run(&mut budget).unwrap_err(), Error::Limit);
        assert_eq!(budget.remaining(), short); assert!(!suite.started());
    }
    let mut budget = RolloutBudget::new(work).unwrap();
    assert!(suite.run(&mut budget).unwrap().accepted()); assert!(suite.started());
    let mut fresh = RolloutBudget::new(work).unwrap();
    assert_eq!(suite.run(&mut fresh).unwrap_err(), Error::WrongState);
    assert_eq!(fresh.remaining(), work);
}

#[test]
fn duplicate_origins_prompts_and_random_streams_cannot_inflate_the_task_count() {
    for mode in 0..4 {
        let mut c = cases();
        match mode {
            0 => c[1].origin.task = c[0].origin.task,
            1 => c[1].origin.lineage = c[0].origin.lineage,
            2 => c[1].prompt = c[0].prompt.clone(),
            _ => c[1].random_stream = c[0].random_stream,
        }
        assert_eq!(PairedRolloutSuite::new(model(false, false), &config(0.5, 100000), protocol(), c, criteria()).unwrap_err(),
            RolloutBuildError::Contract(Error::Duplicate));
    }
}

#[test]
fn invalid_tail_or_effect_rule_refuses_before_any_suite_is_admitted() {
    let mut c = cases(); c[1].prompt.push(4);
    assert_eq!(PairedRolloutSuite::new(model(false, false), &config(0.5, 100000), protocol(), c, criteria()).unwrap_err(),
        RolloutBuildError::Contract(Error::InvalidInput));
    let mut p = protocol(); p.effect_patterns.push(vec![3]);
    assert_eq!(PairedRolloutSuite::new(model(false, false), &config(0.5, 100000), p, cases(), criteria()).unwrap_err(),
        RolloutBuildError::Contract(Error::Duplicate));
    let mut p = protocol(); p.max_new_tokens = 8;
    assert_eq!(PairedRolloutSuite::new(model(false, false), &config(0.5, 100000), p, cases(), criteria()).unwrap_err(),
        RolloutBuildError::Contract(Error::Limit));
}
