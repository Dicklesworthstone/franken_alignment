//! Real decoder/compression/probe execution on a small deterministic model.
//! These fixtures are numerical controls, not deployment or model-safety proof.
use fa_reference::Error;
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{
        DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{
            SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop, GenerationTelemetryBudget},
            replay::{CheckpointLimits, ReplayableGeneration, MAX_REPLAY_STATE_BYTES,
                comparison::{ComparisonLimits, ComparisonStatus}},
        },
    },
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use std::collections::{BTreeMap, BTreeSet};

fn model(generation: u64) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: generation, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn policy(model: &DecoderModel, alarm: bool, probe_count: u64) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut probes = Vec::new();
            for id in 1..=probe_count {
                let mut weights = vec![0.0; tensor.dimensions()];
                let threshold = if alarm && *layer == 2 && side == KvSide::Value {
                    weights[1] = 1.0; 0.5
                } else { 1.0 };
                probes.push(LinearProbe::new(id, 1, tensor.profile(), &weights, 0.0, threshold).unwrap());
            }
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(probes, LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap()
}
fn source(model: &DecoderModel, policy: LearnedDecoderPolicy, top_k: usize, stop: bool) -> ReplayableGeneration {
    let spec = GenerationSpec::new(vec![0], 3, if stop { BTreeSet::from([2]) } else { BTreeSet::new() },
        SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap();
    model.replayable_monitored_generation(21, 201, spec, policy, GenerationBudget::default(),
        GenerationTelemetryBudget::default()).unwrap()
}

#[test]
fn unchanged_policy_matches_original_stochastic_generation_without_touching_source() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone(), 3, false);
    original.advance(0).unwrap();
    let old_work = original.generation().work();
    let old_sampler = original.generation().sampler_state();
    let mut comparison = original.compare_policy_from_start(quiet, ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.run_to_stop(), Ok(ComparisonStatus::MatchedStop(GenerationStop::TokenLimit)));
    assert_eq!(original.generation().position(), 1);
    assert_eq!(original.generation().work(), old_work);
    assert_eq!(original.generation().sampler_state(), old_sampler);
    let report = comparison.report();
    assert_eq!(report.lineage.stream, 21);
    assert_eq!(report.lineage.evaluation_origin, 201);
    assert_eq!(report.lineage.source_position, 1);
    assert_eq!(report.attempted_positions, 4);
    assert_eq!(report.matched_positions, 4);
    assert!(report.state_bytes_compared > 0);
    assert!(report.work.all_attempted_telemetry_reported);
    original.run_to_stop().unwrap();
    assert_eq!(comparison.matched_tokens(), original.generation().accepted_tokens());
    assert_eq!(report.work.baseline, original.generation().work());
    assert_eq!(report.work.candidate, original.generation().work());
    assert_eq!(report.work.baseline_telemetry, original.generation().telemetry_work());
    assert_eq!(report.work.baseline_telemetry, report.work.candidate_telemetry);
}

#[test]
fn additional_quiet_probes_may_change_cost_without_changing_numerical_state() {
    let model = model(3);
    let original = source(&model, policy(&model, false, 1), 3, false);
    let mut comparison = original.compare_policy_from_start(policy(&model, false, 2), ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.run_to_stop(), Ok(ComparisonStatus::MatchedStop(GenerationStop::TokenLimit)));
    let report = comparison.report();
    assert_eq!(report.work.baseline, report.work.candidate);
    assert!(report.work.candidate_telemetry.monitor_probe_coordinates
        > report.work.baseline_telemetry.monitor_probe_coordinates);
    assert!(report.work.all_attempted_telemetry_reported);
}

#[test]
fn candidate_alarm_stops_both_arms_and_hides_the_unpaired_continuation() {
    let model = model(3);
    let original = source(&model, policy(&model, false, 1), 1, false);
    let mut comparison = original.compare_policy_from_start(policy(&model, true, 1), ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.advance(0).unwrap().accepted().unwrap().token, 0);
    let step = comparison.advance(1).unwrap();
    assert_eq!(comparison.status(), ComparisonStatus::DecisionDifference { position: 1 });
    assert!(step.accepted().is_none());
    assert_eq!(step.baseline_status(), GenerationStatus::Generating);
    assert!(matches!(step.candidate_status(), GenerationStatus::Held(_)));
    assert!(step.baseline_audit().unwrap().complete_quiet());
    assert!(!step.candidate_audit().unwrap().complete_quiet());
    assert_eq!(comparison.matched_tokens(), &[0]);
    let before = comparison.report();
    assert_eq!(before.attempted_positions, 2);
    assert_eq!(before.matched_positions, 1);
    assert_eq!(before.work.baseline.sampling_attempts, 1);
    assert_eq!(before.work.candidate.sampling_attempts, 1);
    assert_eq!(comparison.advance(2).err(), Some(Error::WrongState));
    assert_eq!(comparison.report(), before);
    assert_eq!(original.generation().position(), 0);
}

#[test]
fn a_permissive_experiment_cannot_clear_or_disclose_a_held_sources_continuation() {
    let model = model(3);
    let mut original = source(&model, policy(&model, true, 1), 1, false);
    let held = original.run_to_stop().unwrap();
    assert!(matches!(held, GenerationStatus::Held(_)));
    let spent = original.generation().work();
    let mut comparison = original.compare_policy_from_start(policy(&model, false, 1), ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.run_to_stop(), Ok(ComparisonStatus::DecisionDifference { position: 1 }));
    let step = comparison.last_step().unwrap();
    assert!(matches!(step.baseline_status(), GenerationStatus::Held(_)));
    assert_eq!(step.candidate_status(), GenerationStatus::Generating);
    assert!(step.accepted().is_none());
    assert_eq!(comparison.matched_tokens(), &[0]);
    assert_eq!(original.generation().status(), held);
    assert_eq!(original.generation().work(), spent);
    assert_eq!(original.generation().accepted_tokens(), &[0]);
    assert_eq!(original.advance(1).err(), Some(Error::WrongState));
}

#[test]
fn two_held_policies_are_not_a_successfully_completed_continuation() {
    let model = model(3);
    let alarm = policy(&model, true, 1);
    let original = source(&model, alarm.clone(), 1, false);
    let mut comparison = original.compare_policy_from_start(alarm, ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.run_to_stop(), Ok(ComparisonStatus::BothHeld { position: 1 }));
    let step = comparison.last_step().unwrap();
    assert!(matches!(step.baseline_status(), GenerationStatus::Held(_)));
    assert!(matches!(step.candidate_status(), GenerationStatus::Held(_)));
    assert!(step.accepted().is_none());
    assert_eq!(comparison.matched_tokens(), &[0]);
}

#[test]
fn zero_partial_and_exact_position_caps_are_distinct_and_do_not_refill() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone(), 1, false);
    for cap in 0..=4 {
        let mut comparison = original.compare_policy_from_start(quiet.clone(),
            ComparisonLimits { positions: cap, ..ComparisonLimits::default() }).unwrap();
        let result = comparison.run_to_stop();
        if cap == 4 {
            assert_eq!(result, Ok(ComparisonStatus::MatchedStop(GenerationStop::TokenLimit)));
        } else {
            assert_eq!(result, Err(Error::Limit));
            assert_eq!(comparison.status(), ComparisonStatus::Exhausted);
        }
        let report = comparison.report();
        assert_eq!(report.attempted_positions, cap);
        assert_eq!(report.matched_positions, cap);
        assert_eq!(report.work.baseline.admitted_tokens, cap as u64);
        assert_eq!(report.work.candidate.admitted_tokens, cap as u64);
        assert_eq!(comparison.advance(cap as u64).err(), Some(Error::WrongState));
        assert_eq!(comparison.report(), report);
    }
    assert!(matches!(original.compare_policy_from_start(quiet,
        ComparisonLimits { state_bytes: MAX_REPLAY_STATE_BYTES + 1, ..ComparisonLimits::default() }), Err(Error::Limit)));
}

#[test]
fn exact_state_cap_accepts_while_one_less_retains_work_but_no_joint_output() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone(), 1, false);
    original.advance(0).unwrap();
    let bytes = original.checkpoint(CheckpointLimits::default()).unwrap().state_bytes();
    let limits = ComparisonLimits { positions: 1, state_bytes: bytes, ..ComparisonLimits::default() };
    let mut valid = original.compare_policy_from_start(quiet.clone(), limits).unwrap();
    assert!(valid.advance(0).unwrap().accepted().is_some());
    let mut short = original.compare_policy_from_start(quiet,
        ComparisonLimits { state_bytes: bytes - 1, ..limits }).unwrap();
    assert_eq!(short.advance(0).err(), Some(Error::Limit));
    assert_eq!(short.status(), ComparisonStatus::Failed(Error::Limit));
    assert!(short.last_step().unwrap().accepted().is_none());
    assert!(short.matched_tokens().is_empty());
    let report = short.report();
    assert_eq!(report.work.baseline.admitted_tokens, 1);
    assert_eq!(report.work.candidate.admitted_tokens, 1);
    assert!(report.work.all_attempted_telemetry_reported);
    assert_eq!(short.advance(0).err(), Some(Error::WrongState));
    assert_eq!(short.report(), report);
}

#[test]
fn missing_candidate_audit_is_a_latched_failure_not_an_empirical_decision() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone(), 1, false);
    let mut valid = original.compare_policy_from_start(quiet.clone(), ComparisonLimits::default()).unwrap();
    assert!(valid.advance(0).unwrap().accepted().is_some());
    let mut preparation = quiet.preparation();
    preparation.source_check.source_values = 0;
    let limited = LearnedDecoderPolicy::new(quiet.codec().clone(), quiet.monitor().clone(),
        LearnedStreamRetention::All, preparation, quiet.inference()).unwrap();
    let mut comparison = original.compare_policy_from_start(limited, ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.advance(0).err(), Some(Error::Limit));
    assert_eq!(comparison.status(), ComparisonStatus::Failed(Error::Limit));
    let step = comparison.last_step().unwrap();
    assert_eq!(step.baseline_status(), GenerationStatus::Generating);
    assert_eq!(step.candidate_status(), GenerationStatus::Failed(Error::Limit));
    assert!(step.baseline_audit().unwrap().complete_quiet());
    assert!(step.candidate_audit().is_none());
    assert!(step.accepted().is_none());
    assert!(!comparison.report().work.all_attempted_telemetry_reported);
    assert!(comparison.matched_tokens().is_empty());
    assert_eq!(comparison.advance(0).err(), Some(Error::WrongState));
}

#[test]
fn stale_calls_cannot_advance_only_one_arm() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone(), 1, false);
    let mut comparison = original.compare_policy_from_start(quiet, ComparisonLimits::default()).unwrap();
    let before = comparison.report();
    assert_eq!(comparison.advance(1).err(), Some(Error::Stale));
    assert_eq!(comparison.report(), before);
    comparison.advance(0).unwrap();
    let before = comparison.report();
    assert_eq!(comparison.advance(0).err(), Some(Error::Stale));
    assert_eq!(comparison.report(), before);
    comparison.advance(1).unwrap();
    assert_eq!(comparison.position(), 2);
}

#[test]
fn a_foreign_model_policy_cannot_be_mistaken_for_a_codec_intervention() {
    let first = model(3);
    let second = model(99);
    let original = source(&first, policy(&first, false, 1), 1, false);
    assert!(original.compare_policy_from_start(policy(&first, false, 1), ComparisonLimits::default()).is_ok());
    assert!(matches!(original.compare_policy_from_start(policy(&second, false, 1), ComparisonLimits::default()), Err(Error::Binding)));
    assert_eq!(original.generation().position(), 0);
    assert_eq!(original.generation().work().admitted_tokens, 0);
}

#[test]
fn stop_token_remains_a_real_stop_not_a_full_horizon_equivalence_claim() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone(), 1, true);
    let mut comparison = original.compare_policy_from_start(quiet, ComparisonLimits::default()).unwrap();
    assert_eq!(comparison.run_to_stop(), Ok(ComparisonStatus::MatchedStop(GenerationStop::StopToken(2))));
    assert_eq!(comparison.matched_tokens(), &[0, 2]);
    assert_eq!(comparison.report().attempted_positions, 2);
    assert_eq!(comparison.advance(2).err(), Some(Error::WrongState));
}

#[test]
fn fresh_numerical_allowance_must_cover_both_arms_before_either_starts() {
    let model = model(3);
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone(), 1, false);
    let estimate = original.generation().estimate();
    let products = estimate.decoder.scalar_products().unwrap() * 2;
    let scores = estimate.vocabulary_scores * 2;
    let exact = ComparisonLimits { positions: 4, decoder_products: products,
        vocabulary_scores: scores, ..ComparisonLimits::default() };
    let mut accepted = original.compare_policy_from_start(quiet.clone(), exact).unwrap();
    assert_eq!(accepted.run_to_stop(), Ok(ComparisonStatus::MatchedStop(GenerationStop::TokenLimit)));
    for limits in [ComparisonLimits { decoder_products: products - 1, ..exact },
        ComparisonLimits { vocabulary_scores: scores - 1, ..exact }] {
        assert!(matches!(original.compare_policy_from_start(quiet.clone(), limits), Err(Error::Limit)));
        assert_eq!(original.generation().position(), 0);
        assert_eq!(original.generation().work().admitted_tokens, 0);
    }
}
