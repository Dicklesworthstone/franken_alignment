//! End-to-end sampled continuation through the ordered original audit verifier.
#[path = "support/restart_model.rs"]
mod support;
use support::*;
use fa_reference::Error;
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel, monitoring::{LearnedDecoderPolicy, restart::{KvRestartBudget,
        incremental::IncrementalRestartStatus}},
    sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop,
            GenerationTelemetryBudget, LearnedGeneration}},
};
use std::collections::BTreeSet;

fn generation(model: &DecoderModel, policy: LearnedDecoderPolicy, prompt: Vec<u32>, top_k: usize,
    stop: bool, telemetry: GenerationTelemetryBudget) -> LearnedGeneration
{
    let spec = GenerationSpec::new(prompt, 4, if stop { BTreeSet::from([2]) } else { BTreeSet::new() },
        SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(),
            stream: 71, seed: 173 }).unwrap();
    let estimate = model.estimate_monitored_generation(&spec).unwrap();
    let numerical = GenerationBudget { decoder_products: estimate.decoder.scalar_products().unwrap(),
        vocabulary_scores: estimate.vocabulary_scores };
    model.monitored_generation_with_telemetry(21, 201, spec, policy, numerical, telemetry).unwrap()
}
fn same_generation(a: &LearnedGeneration, b: &LearnedGeneration) {
    assert_eq!(a.status(), b.status());
    assert_eq!(a.position(), b.position());
    assert_eq!(a.accepted_tokens(), b.accepted_tokens());
    assert_eq!(a.generated_tokens(), b.generated_tokens());
    assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.work(), b.work());
    assert_eq!(a.telemetry_work(), b.telemetry_work());
    assert_eq!(a.budget(), b.budget());
    assert_eq!(a.telemetry_budget(), b.telemetry_budget());
    assert_eq!(a.estimate(), b.estimate());
    assert_eq!(a.samples().len(), b.samples().len());
    for (a, b) in a.samples().iter().zip(b.samples()) {
        assert_eq!(a.token, b.token);
        assert_eq!(a.stream, b.stream);
        assert_eq!(a.draw, b.draw);
        assert_eq!(a.random_word, b.random_word);
        assert_eq!(a.probability.to_bits(), b.probability.to_bits());
        assert_eq!(a.work, b.work);
    }
    match (a.accepted_logits(), b.accepted_logits()) {
        (Ok(a), Ok(b)) => assert_eq!(logits(a), logits(b)),
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("only one arm has accepted logits"),
    }
    same_cache(&a.accepted_cache_image().unwrap(), &b.accepted_cache_image().unwrap());
}

#[test]
fn every_prompt_sample_and_terminal_cut_survives_one_position_audit_caps() {
    let model = model();
    let policy = policy(&model, 0, 1);
    for cut in 0..=6 {
        let mut original = generation(&model, policy.clone(), vec![0, 1], 3, false,
            GenerationTelemetryBudget::default());
        for position in 0..cut { original.advance(position).unwrap(); }
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let legacy = saved.begin_restart(90, KvRestartBudget {
            cache_values: saved.cache_values(), audit: policy.allowance(),
        });
        if cut > 1 { assert!(matches!(legacy, Err(Error::Limit))); }
        else { same_generation(&original, &legacy.unwrap().finish().unwrap().0); }
        let mut restart = saved.begin_incremental_restart(22, budget(&policy, cut as usize)).unwrap();
        assert_eq!(restart.position_count(), cut as usize);
        for position in 0..cut {
            assert_eq!(restart.advance(position + 1).err(), Some(Error::Stale));
            assert_eq!(restart.next_position(), position);
            assert!(restart.advance(position).unwrap().monitoring().complete_quiet());
        }
        assert!(restart.is_ready());
        let (mut resumed, receipt) = restart.finish().unwrap();
        assert_eq!(receipt.historical_work(), original.work());
        assert_eq!(receipt.historical_telemetry(), original.telemetry_work());
        assert_eq!(receipt.sampler_draws(), original.sampler_state().draws());
        assert_eq!(receipt.status(), original.status());
        assert_eq!(receipt.kv().work().quiet_positions, cut as usize);
        assert_eq!(receipt.kv().evaluation_origin(), 201);
        assert_eq!(receipt.kv().restoration().position, cut);
        assert!(resumed.last_event().is_none());
        same_generation(&original, &resumed);
        while original.status().is_active() {
            let position = original.position();
            assert!(original.advance(position).unwrap().accepted().is_some());
            assert!(resumed.advance(position).unwrap().accepted().is_some());
            same_generation(&original, &resumed);
        }
        assert_eq!(resumed.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
        assert_eq!(resumed.work().reserved_decoder_products, resumed.budget().decoder_products);
        assert_eq!(resumed.work().reserved_vocabulary_scores, resumed.budget().vocabulary_scores);
        assert_eq!(resumed.advance(resumed.position()).err(), Some(Error::WrongState));
    }
}

#[test]
fn repeated_incremental_restarts_do_not_refill_numerical_or_telemetry_budgets() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget::default());
    let mut current = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget::default());
    for position in 0..6 {
        original.advance(position).unwrap();
        current.advance(position).unwrap();
        let saved = current.checkpoint_kv(capture_limit()).unwrap();
        let mut restart = saved.begin_incremental_restart(22 + position, budget(&policy, position as usize + 1)).unwrap();
        for audit_position in 0..=position { restart.advance(audit_position).unwrap(); }
        let (next, receipt) = restart.finish().unwrap();
        assert_eq!(receipt.historical_work(), current.work());
        assert_eq!(receipt.historical_telemetry(), current.telemetry_work());
        assert_eq!(receipt.kv().work().attempted_positions, position as usize + 1);
        same_generation(&original, &next);
        current = next;
    }
    assert_eq!(current.sampler_state().draws(), 4);
    assert_eq!(current.work().admitted_tokens, 6);
    assert_eq!(current.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
}

#[test]
fn source_can_advance_while_verifier_keeps_its_sealed_prefix_and_original_rng() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget::default());
    original.advance(0).unwrap();
    original.advance(1).unwrap();
    original.advance(2).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let snapshot = original.sampler_state();
    let mut restart = saved.begin_incremental_restart(22, budget(&policy, 3)).unwrap();
    restart.advance(0).unwrap();
    let first = restart.last_audit().unwrap().monitoring().source().descriptor().clone();
    original.run_to_stop().unwrap();
    assert_eq!(restart.next_position(), 1);
    assert_eq!(restart.position_count(), 3);
    restart.advance(1).unwrap();
    restart.advance(2).unwrap();
    let (mut resumed, _) = restart.finish().unwrap();
    assert_eq!(resumed.position(), 3);
    assert_eq!(resumed.sampler_state(), snapshot);
    assert!(first.layers().values().all(|layer| layer.stream == 21 && layer.first_position == 0));
    resumed.run_to_stop().unwrap();
    same_generation(&original, &resumed);
}

#[test]
fn original_exhausted_telemetry_stays_exhausted_after_separately_paid_restart_audits() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut control = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget::default());
    control.advance(0).unwrap(); control.advance(1).unwrap();
    let used = control.telemetry_work().source_check_values;
    assert!(used > 0);
    assert!(control.advance(2).unwrap().accepted().is_some());
    let mut original = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget { source_check_values: used, ..GenerationTelemetryBudget::default() });
    original.advance(0).unwrap(); original.advance(1).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let rng = original.sampler_state();
    let mut restart = saved.begin_incremental_restart(22, budget(&policy, 2)).unwrap();
    restart.advance(0).unwrap(); restart.advance(1).unwrap();
    let (mut resumed, receipt) = restart.finish().unwrap();
    assert_eq!(receipt.historical_telemetry().source_check_values, used);
    assert_eq!(receipt.kv().work().reported.source_check_values, used);
    assert_eq!(original.advance(2).err(), Some(Error::Limit));
    assert_eq!(resumed.advance(2).err(), Some(Error::Limit));
    same_generation(&original, &resumed);
    assert_eq!(resumed.sampler_state(), rng);
    assert_eq!(resumed.work().admitted_tokens, 3);
    assert_eq!(resumed.work().accepted_decoder.tokens, 2);
    assert!(matches!(resumed.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
}

#[test]
fn eos_is_terminal_and_later_held_source_cannot_be_reset_by_a_checkpoint() {
    let model = model();
    let quiet = policy(&model, 0, 1);
    let mut original = generation(&model, quiet.clone(), vec![0], 1, true,
        GenerationTelemetryBudget::default());
    assert_eq!(original.run_to_stop(), Ok(GenerationStatus::Finished(GenerationStop::StopToken(2))));
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let mut restart = saved.begin_incremental_restart(22, budget(&quiet, 2)).unwrap();
    restart.advance(0).unwrap(); restart.advance(1).unwrap();
    let (mut resumed, receipt) = restart.finish().unwrap();
    same_generation(&original, &resumed);
    assert_eq!(receipt.sampler_draws(), 1);
    assert_eq!(resumed.advance(2).err(), Some(Error::WrongState));

    let alarm = policy(&model, 2, 1);
    let mut original = generation(&model, alarm.clone(), vec![0], 1, false,
        GenerationTelemetryBudget::default());
    original.advance(0).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let mut restart = saved.begin_incremental_restart(23, budget(&alarm, 1)).unwrap();
    let held = original.advance(1).unwrap();
    assert!(held.accepted().is_none() && held.sample().is_none());
    assert_eq!(original.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    restart.advance(0).unwrap();
    let (mut resumed, _) = restart.finish().unwrap();
    let held = resumed.advance(1).unwrap();
    assert!(held.accepted().is_none() && held.sample().is_none());
    same_generation(&original, &resumed);
    assert_eq!(original.advance(1).err(), Some(Error::WrongState));
}

#[test]
fn partial_and_failed_audits_never_release_a_sampled_generation() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = generation(&model, policy.clone(), vec![0, 1], 3, false,
        GenerationTelemetryBudget::default());
    original.advance(0).unwrap(); original.advance(1).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    for cut in 0..2 {
        let mut partial = saved.begin_incremental_restart(22, budget(&policy, 2)).unwrap();
        for position in 0..cut { partial.advance(position).unwrap(); }
        assert!(!partial.is_ready());
        assert!(matches!(partial.finish(), Err(Error::Incomplete)));
    }
    let mut insufficient = budget(&policy, 2);
    insufficient.per_position.monitoring.probe_coordinates = 0;
    let mut held = saved.begin_incremental_restart(23, insufficient).unwrap();
    assert_eq!(held.advance(0).unwrap().monitoring().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(held.status(), IncrementalRestartStatus::Held(MonitorOutcome::BudgetExhausted));
    assert_eq!(held.advance(0).err(), Some(Error::WrongState));
    assert!(matches!(held.finish(), Err(Error::Incomplete)));
    let mut valid = saved.begin_incremental_restart(24, budget(&policy, 2)).unwrap();
    valid.advance(0).unwrap(); valid.advance(1).unwrap();
    same_generation(&original, &valid.finish().unwrap().0);
}
