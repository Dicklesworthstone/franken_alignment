//! Original GQA K/V capture, complete training, and real sampled release/hold.
//! Synthetic labels establish reachability and causal dependence, not alignment.
#[path = "support/decoder_kv_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;

use fixture::*;
use fa_reference::action::consequence::activation::{ProgressiveFrame, HEADER_BYTES};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome,
    learned::model::KvTap};
use fa_reference::action::consequence::activation::probe::{ProbeOutcome,
    training::{ClassCounts, DataSplit, FitPolicy, decoder::kv::*}};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{monitoring::LearnedStreamRetention, sampling::{SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationPhase, GenerationStatus, GenerationStop}}},
    experiment::KvSide,
};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Error;
use std::collections::BTreeSet;

#[test]
fn complete_gqa_campaign_retains_original_frames_and_every_reserved_population() {
    let model = model();
    let cases = cases();
    let work = KvDecoderCorpus::estimate(&model, &cases).unwrap();
    assert_eq!(work.cases, 6);
    assert_eq!(work.original_tokens, 12);
    assert_eq!(work.kv_coordinates, 48);
    assert_eq!(work.scalar_products, 6 * model.estimate(0, 2).unwrap().scalar_products().unwrap());
    let corpus = capture(&model, &cases);
    assert_eq!(corpus.work(), work);
    assert_eq!(corpus.profile(), model.profile());
    assert_eq!(corpus.cases().values().cloned().collect::<Vec<_>>(), cases);
    assert_eq!(corpus.taps().len(), 4);
    let report = campaign(&corpus);
    assert!(report.accepted());
    assert_eq!(report.admitted_work(), report.completed_work());
    assert_eq!(report.completed_work().training_visits, 4 * 2 * 2 * (1 + 2 * 64));
    assert_eq!(report.completed_work().scoring_bytes, 16 * (HEADER_BYTES + 8));
    assert_eq!(report.completed_work().scoring_coordinates, 32);
    assert_eq!(report.completed_work().threshold_comparisons, 32);
    for (tap, result) in report.taps() {
        let corpus = &corpus.taps()[tap];
        assert_eq!(corpus.dimensions(), 2); // GQA cache width, not hidden/query width 4.
        for split in [DataSplit::Training, DataSplit::Calibration, DataSplit::Evaluation] {
            assert_eq!(corpus.counts(split), ClassCounts { benign: 1, violation: 1 });
        }
        let calibration = result.calibration();
        assert_eq!(calibration.selected_threshold(), Some(0.0));
        assert_eq!(calibration.trials().len(), 3);
        assert!(calibration.fitted().weights().iter().any(|weight| *weight != 0.0));
        let evaluation = result.evaluation().unwrap();
        assert_eq!(evaluation.counts().classes(), ClassCounts { benign: 1, violation: 1 });
        assert_eq!(evaluation.counts().benign_quiet, 1);
        assert_eq!(evaluation.counts().violation_alarm, 1);
        let contract = &model.cache_profile().layers()[&tap.layer];
        let tensor = match tap.side { KvSide::Key => contract.keys(), KvSide::Value => contract.values() };
        assert_eq!(report.probes().unwrap()[tap].identity().profile, tensor.profile());
    }
}

#[test]
fn scores_match_independent_original_decoder_execution_for_both_kv_sides() {
    let model = model();
    let cases = cases();
    let report = campaign(&capture(&model, &cases));
    for case in cases.iter().filter(|case| case.split == DataSplit::Calibration) {
        let mut direct = model.session(case.origin.task).unwrap();
        for (position, token) in case.tokens.iter().copied().enumerate() {
            direct.advance(position as u64, token, inference()).unwrap();
        }
        let cache = direct.cache_image().unwrap();
        for (tap, result) in report.taps() {
            let token = cache.layer(tap.layer).unwrap().token(1).unwrap();
            let source = match tap.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
            let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
            let frame = ProgressiveFrame::from_initial(&block).unwrap();
            let score = result.calibration().fitted().probe(0.0).unwrap().evaluate(&frame).unwrap();
            let stored = result.calibration().scores().iter().find(|score| score.origin() == case.origin).unwrap();
            assert_eq!(stored.frame(), source.identity());
            assert_eq!(stored.frame().sequence, 2);
            assert_eq!(stored.frame().position, 1);
            assert_eq!(stored.score(), &score.interval().lower);
            assert_eq!(score.interval().lower, score.interval().upper);
        }
    }
}

#[test]
fn trained_roster_releases_real_benign_samples_and_holds_real_violation_samples() {
    let report = campaign(&capture(&model(), &cases()));
    // Keep the campaign's actual immutable model; no reconstruction from IDs.
    let model = report.model();
    let codec = codec(model);
    let settings = settings(model);
    let policy = report.policy(codec, &settings).unwrap();
    for (seed, expected) in [(4, 0), (5, 1)] {
        let spec = spec(seed);
        let mut oracle = model.recompute_sampled(2000, spec.prompt(), inference(), spec.sampling().clone()).unwrap();
        let oracle_next = oracle.advance_sampled(1, SampleBudget { decoder: inference(),
            sampling: SamplingBudget { vocabulary: 10 } }).unwrap();
        assert_eq!(oracle_next.choice.token, expected);
        let mut run = model.monitored_generation(2000, 2001, spec, policy.clone(), GenerationBudget::default()).unwrap();
        let prompt = run.advance(0).unwrap();
        assert!(prompt.audit().complete_quiet());
        assert_eq!(prompt.audit().planned_rows(), 4);
        assert_eq!(prompt.audit().source().report().source_values, 8);
        let accepted_cache = run.accepted_cache_image().unwrap().encode().unwrap();
        let rng = run.sampler_state();
        let next = run.advance(1).unwrap();
        assert_eq!(next.phase(), GenerationPhase::Continuation);
        assert_eq!(next.audit().planned_rows(), 4);
        assert_eq!(next.audit().source().report().source_values, 8);
        assert_eq!(next.audit().source().descriptor().profile(), model.cache_profile());
        if expected == 0 {
            assert!(next.audit().complete_quiet());
            assert_eq!(next.sample(), Some(&oracle_next.choice));
            assert_eq!(next.accepted().unwrap().logits, oracle_next.computation.logits);
            assert_eq!(run.accepted_tokens(), &[4, 0]);
            let stop = run.advance(2).unwrap();
            assert!(stop.audit().complete_quiet());
            assert_eq!(stop.sample().unwrap().token, 2);
            assert_eq!(run.status(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
            assert_eq!(run.accepted_tokens(), &[4, 0, 2]);
            assert_eq!(run.generated_tokens(), &[0, 2]);
        } else {
            assert_eq!(next.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(next.accepted().is_none());
            assert!(next.sample().is_none());
            assert_eq!(run.accepted_tokens(), &[5]);
            assert!(run.generated_tokens().is_empty());
            assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), accepted_cache);
            assert_eq!(run.sampler_state(), rng);
            assert_eq!(run.work().sampling_attempts, 1);
            assert_eq!(run.work().admitted_tokens, 2);
            assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
        }
    }
}

#[test]
fn removing_one_actual_value_projection_blocks_the_entire_export_not_other_results() {
    let good = campaign(&capture(&model(), &cases()));
    assert!(good.accepted());
    let model = model_without_value_signal();
    let bad = campaign(&capture(&model, &cases()));
    let removed = KvTap { layer: 2, side: KvSide::Value };
    assert!(!bad.accepted());
    assert_eq!(bad.taps().len(), 4);
    for (tap, result) in bad.taps() {
        assert_eq!(result.accepted(), *tap != removed);
        assert_eq!(result.calibration().scores().len(), 2);
        for trial in result.calibration().trials() {
            assert_eq!(trial.counts().classes(), ClassCounts { benign: 1, violation: 1 });
        }
        assert_eq!(result.calibration().fitted().corpus().counts(DataSplit::Evaluation),
            ClassCounts { benign: 1, violation: 1 });
        if *tap == removed {
            assert_eq!(result.calibration().fitted().weights(), &[0.0, 0.0]);
            assert!(result.evaluation().is_none());
        } else {
            assert_eq!(result.calibration().trials(), good.taps()[tap].calibration().trials());
            assert_eq!(result.evaluation().unwrap().scores(), good.taps()[tap].evaluation().unwrap().scores());
        }
    }
    assert!(bad.completed_work().scoring_coordinates < bad.admitted_work().scoring_coordinates);
    assert_eq!(bad.probes().unwrap_err(), Error::WrongState);
    assert_eq!(bad.monitor(&settings(&model)).unwrap_err(), Error::WrongState);
    assert_eq!(bad.monitor_json(&codec(&model), &settings(&model), MAX_KV_MONITOR_JSON_BYTES).unwrap_err(), Error::WrongState);
}

#[test]
fn untouched_final_cases_cannot_retune_coefficients_or_selected_thresholds() {
    let model = model();
    let baseline = cases();
    let good = campaign(&capture(&model, &baseline));
    let mut changed = baseline;
    for case in &mut changed {
        if case.split == DataSplit::Evaluation { case.tokens[1] ^= 1; }
    }
    let bad = campaign(&capture(&model, &changed));
    assert!(!bad.accepted());
    assert_eq!(bad.probes().unwrap_err(), Error::WrongState);
    for (tap, result) in bad.taps() {
        let a = good.taps()[tap].calibration();
        let b = result.calibration();
        assert_eq!(a.fitted().weights(), b.fitted().weights());
        assert_eq!(a.fitted().bias(), b.fitted().bias());
        assert_eq!(a.trials(), b.trials());
        let evaluation = result.evaluation().unwrap();
        assert_eq!(evaluation.counts().violation_quiet, 1);
        assert_eq!(evaluation.counts().benign_alarm, 1);
        assert_eq!(evaluation.scores().len(), 2);
    }
}

#[test]
fn capture_admission_is_atomic_for_every_field_and_persistent_across_calls() {
    let model = model();
    let cases = cases();
    let work = KvDecoderCorpus::estimate(&model, &cases).unwrap();
    for field in 0..4 {
        let mut short = work;
        match field { 0 => short.cases -= 1, 1 => short.original_tokens -= 1,
            2 => short.kv_coordinates -= 1, _ => short.scalar_products -= 1 }
        let mut budget = KvCaptureBudget::new(short).unwrap();
        assert_eq!(KvDecoderCorpus::capture(&model, 1, 1, &cases, &mut budget).unwrap_err(), Error::Limit);
        assert_eq!(budget.remaining(), short);
    }
    let mut budget = KvCaptureBudget::new(work).unwrap();
    KvDecoderCorpus::capture(&model, 1, 1, &cases, &mut budget).unwrap();
    assert_eq!(budget.remaining(), KvCaptureWork::default());
    assert_eq!(KvDecoderCorpus::capture(&model, 2, 1, &cases, &mut budget).unwrap_err(), Error::Limit);
}

#[test]
fn late_bad_tokens_duplicate_histories_or_reused_split_origins_are_free_refusals() {
    let model = model();
    let baseline = cases();
    let work = KvDecoderCorpus::estimate(&model, &baseline).unwrap();
    let errors = [Error::InvalidInput, Error::InvalidInput, Error::Duplicate, Error::Duplicate,
        Error::Duplicate, Error::Limit, Error::Incomplete, Error::InvalidInput];
    for (failure, expected) in errors.into_iter().enumerate() {
        let mut malformed = baseline.clone();
        match failure {
            0 => malformed[5].tokens[1] = 10,
            1 => malformed[5].tokens.clear(),
            2 => malformed[5].origin.lineage = malformed[0].origin.lineage,
            3 => malformed[5].origin.task = malformed[0].origin.task,
            4 => malformed[5].tokens = malformed[0].tokens.clone(),
            5 => malformed[5].tokens = vec![0; 9],
            6 => malformed[5].label = malformed[4].label,
            _ => malformed[5].origin.lineage = 0,
        }
        let mut budget = KvCaptureBudget::new(work).unwrap();
        assert_eq!(KvDecoderCorpus::capture(&model, 1, 1, &malformed, &mut budget).unwrap_err(), expected);
        assert_eq!(budget.remaining(), work);
        assert!(KvDecoderCorpus::capture(&model, 1, 1, &baseline, &mut budget).is_ok());
    }
}

#[test]
fn entire_fit_calibration_and_final_roster_is_admitted_before_any_training() {
    let corpus = capture(&model(), &cases());
    let plans = policies();
    let work = corpus.estimate_campaign(&plans).unwrap();
    for field in 0..4 {
        let mut short = work;
        match field { 0 => short.training_visits -= 1, 1 => short.scoring_bytes -= 1,
            2 => short.scoring_coordinates -= 1, _ => short.threshold_comparisons -= 1 }
        let mut budget = KvCampaignBudget::new(short).unwrap();
        assert_eq!(corpus.run(plans.clone(), &mut budget).unwrap_err(), Error::Limit);
        assert_eq!(budget.remaining(), short);
    }
    let mut incomplete = plans.clone(); incomplete.remove(&KvTap { layer: 2, side: KvSide::Value });
    let mut budget = KvCampaignBudget::new(work).unwrap();
    assert_eq!(corpus.run(incomplete, &mut budget).unwrap_err(), Error::Binding);
    assert_eq!(budget.remaining(), work);
    assert!(corpus.run(plans.clone(), &mut budget).unwrap().accepted());
    assert_eq!(budget.remaining(), KvCampaignWork::default());
    assert_eq!(corpus.run(plans, &mut budget).unwrap_err(), Error::Limit);
}

#[test]
fn hard_capture_or_late_fit_failure_cannot_return_a_partial_roster_or_refund_work() {
    let broken = model_with_output_overflow();
    let cases = cases();
    let mut budget = KvCaptureBudget::new(KvDecoderCorpus::estimate(&broken, &cases).unwrap()).unwrap();
    assert_eq!(KvDecoderCorpus::capture(&broken, 1, 1, &cases, &mut budget).unwrap_err(), Error::Overflow);
    assert_eq!(budget.remaining(), KvCaptureWork::default());
    let corpus = capture(&model(), &cases);
    let mut plans = policies();
    plans.get_mut(&KvTap { layer: 2, side: KvSide::Value }).unwrap().fit =
        FitPolicy::new(5, 1, 512, 1.0, 1000.0, 0.001).unwrap();
    let mut budget = KvCampaignBudget::new(corpus.estimate_campaign(&plans).unwrap()).unwrap();
    assert_eq!(corpus.run(plans, &mut budget).unwrap_err(), Error::Overflow);
    assert_eq!(budget.remaining(), KvCampaignWork::default());
    assert!(campaign(&corpus).accepted());
}

#[test]
fn policy_refuses_foreign_cache_profiles_and_declared_codec_probe_overlap() {
    let model = model();
    let report = campaign(&capture(&model, &cases()));
    let settings = settings(&model);
    assert_eq!(report.profile().identity(), identity());
    for field in 0..2 {
        let mut changed = identity();
        if field == 0 { changed.model_generation += 1; } else { changed.profile_generation += 1; }
        let foreign = model_with_identity(changed);
        assert_eq!(report.policy(codec(&foreign), &settings).unwrap_err(), Error::Binding);
        let policy = report.policy(codec(&model), &settings).unwrap();
        assert_eq!(foreign.monitored_generation(2000, 2001, spec(4), policy, GenerationBudget::default())
            .unwrap_err(), Error::Binding);
    }
    for (stream, lineage) in [(1000, 101), (1, 1010)] {
        let overlapping = codec_from(&model, stream, lineage);
        assert_eq!(report.policy(overlapping, &settings).unwrap_err(), Error::Duplicate);
    }
    assert!(report.policy(codec(&model), &settings).is_ok());
}

#[test]
fn export_is_complete_bounded_and_preserves_original_finite_coefficient_bits() {
    let model = model();
    let report = campaign(&capture(&model, &cases()));
    let codec = codec(&model);
    let mut settings = settings(&model);
    let bytes = report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap();
    assert_eq!(report.monitor_json(&codec, &settings, bytes.len()).unwrap(), bytes);
    assert_eq!(report.monitor_json(&codec, &settings, bytes.len() - 1).unwrap_err(), Error::Limit);
    assert_eq!(report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES + 1).unwrap_err(), Error::Limit);
    let json = strict_json::parse(&bytes, Limits::default()).unwrap();
    assert_eq!(json.get("schema").unwrap().as_str(), Some("fa.learned-kv-monitor/1"));
    assert!(json.get("identity").is_none()); // Existing provisioning schema has no authentication claim.
    let taps = json.get("taps").unwrap().as_array().unwrap();
    assert_eq!(taps.len(), 4);
    for (emitted, (tap, result)) in taps.iter().zip(report.taps()) {
        assert_eq!(emitted.get("layer").unwrap().as_u64(), Some(tap.layer));
        assert_eq!(emitted.get("side").unwrap().as_str(), Some(match tap.side { KvSide::Key => "key", KvSide::Value => "value" }));
        let probe = &emitted.get("probes").unwrap().as_array().unwrap()[0];
        let fitted = result.calibration().fitted();
        assert_eq!(probe.get("id").unwrap().as_u64(), Some(fitted.policy().id()));
        let weights = probe.get("weights").unwrap().as_array().unwrap().iter().map(bits).collect::<Vec<_>>();
        assert_eq!(weights, fitted.weights().iter().map(|value| value.to_bits()).collect::<Vec<_>>());
        assert_eq!(bits(probe.get("bias").unwrap()), fitted.bias().to_bits());
        assert_eq!(bits(probe.get("threshold").unwrap()), result.calibration().selected_threshold().unwrap().to_bits());
    }
    settings.retention = LearnedStreamRetention::Heads(BTreeSet::new());
    assert!(report.policy(codec.clone(), &settings).is_ok());
    assert_eq!(report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap_err(), Error::InvalidInput);
    settings.retention = LearnedStreamRetention::None;
    assert!(report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).is_ok());
    settings.taps.remove(&KvTap { layer: 2, side: KvSide::Value });
    assert_eq!(report.monitor_json(&codec, &settings, MAX_KV_MONITOR_JSON_BYTES).unwrap_err(), Error::Binding);
}

#[test]
fn case_order_cannot_change_training_and_exact_probes_are_not_constant_quiet() {
    let model = model();
    let cases = cases();
    let a = campaign(&capture(&model, &cases));
    let mut reversed = cases; reversed.reverse();
    let b = campaign(&capture(&model, &reversed));
    for (tap, result) in a.taps() {
        assert_eq!(result.calibration().trials(), b.taps()[tap].calibration().trials());
        assert_eq!(result.evaluation().unwrap().scores(), b.taps()[tap].evaluation().unwrap().scores());
        for (class, expected) in [(0, ProbeOutcome::CertifiedQuiet), (1, ProbeOutcome::CertifiedAlarm)] {
            let source = model.recompute(2000, &[4, class], inference()).unwrap().cache_image().unwrap();
            let token = source.layer(tap.layer).unwrap().token(1).unwrap();
            let frame = match tap.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
            let block = frame.verify_block(&frame.encode_initial(23).unwrap()).unwrap();
            assert_eq!(a.probes().unwrap()[tap].evaluate(&ProgressiveFrame::from_initial(&block).unwrap())
                .unwrap().outcome(), expected);
        }
    }
}

fn bits(value: &Json) -> u32 {
    let Json::Number(value) = value else { panic!("not a binary32 number") };
    value.lexeme().parse::<f32>().unwrap().to_bits()
}
