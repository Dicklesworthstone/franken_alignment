//! Continuation controls use real nonzero attention, original fitted compression
//! and original monitors. Small synthetic weights are not deployment evidence.
use fa_reference::Error;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, DecoderRestoreBudget, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedDecoderStatus, LearnedStreamRetention,
            restart::KvRestartBudget}},
    experiment::KvSide,
    image::IMAGE_HEADER_BYTES,
    model::{ModelKvImage, MAX_MODEL_KV_VALUES,
        learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}},
};
use std::collections::{BTreeMap, BTreeSet};

fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2],
        queries: vec![1.0, 0.0, 0.0, 1.0], keys: vec![1.0, 0.0, 0.0, 1.0],
        values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.25, 0.0, 0.0, 0.25],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn policy(model: &DecoderModel, alarm: bool, retention: usize, rows: usize) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let retention = match retention {
        0 => LearnedStreamRetention::None,
        1 => LearnedStreamRetention::All,
        _ => LearnedStreamRetention::Heads(BTreeSet::from([*codec.groups().keys().next().unwrap()])),
    };
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 1 && side == KvSide::Value {
                weights[1] = 1.0; 0.5
            } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps,
        LearnedAuditBudget { rows, ..LearnedAuditBudget::default() }).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, retention,
        LearnedAuditPreparationBudget::default(), inference).unwrap()
}
fn capture_limit() -> DecoderRestoreBudget { DecoderRestoreBudget { cache_values: MAX_MODEL_KV_VALUES } }
fn restart_budget(policy: &LearnedDecoderPolicy) -> KvRestartBudget {
    KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: policy.allowance() }
}
fn logits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn same_cache(left: &ModelKvImage, right: &ModelKvImage) {
    assert_eq!(left.profile(), right.profile());
    assert_eq!(left.len(), right.len());
    for layer in left.profile().layers().keys() {
        let a = left.layer(*layer).unwrap().encode().unwrap();
        let b = right.layer(*layer).unwrap().encode().unwrap();
        // Headers intentionally differ: derived stream and bulk-restore revision.
        assert_eq!(&a[IMAGE_HEADER_BYTES..], &b[IMAGE_HEADER_BYTES..]);
    }
}

#[test]
fn native_restart_reaudits_original_source_and_continues_the_same_nonzero_attention() {
    let model = model();
    let quiet = policy(&model, false, 1, 4096);
    let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    for (position, token) in [0, 1, 2].into_iter().enumerate() {
        assert!(original.advance(position as u64, token).unwrap().step().is_some());
    }
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let before = original.accepted_cache_image().unwrap();
    let restart = saved.begin_restart(22, restart_budget(&quiet)).unwrap();
    assert!(restart.is_ready());
    let audit = restart.audit().unwrap().monitoring();
    assert!(audit.complete_quiet());
    assert_eq!(audit.planned_rows(), 12);
    assert_eq!(audit.first_position(), 0);
    assert_eq!(audit.end_position(), 3);
    assert!(audit.source().descriptor().layers().values().all(|d| d.stream == 21));
    let (mut resumed, receipt) = restart.finish().unwrap();
    assert_eq!(receipt.evaluation_origin(), 201);
    assert_eq!(receipt.restoration().resumed_stream, 22);
    assert_eq!(receipt.restoration().values_restored, saved.cache_values());
    assert_eq!(receipt.restoration().bytes_written, saved.cache_values() * 4);
    assert_eq!(receipt.restoration().bytes_recaptured, saved.cache_values() * 4);
    assert!(resumed.last_event().is_none());
    assert_eq!(resumed.position(), 3);
    assert_eq!(resumed.evaluation_origin(), 201);
    same_cache(&before, &resumed.accepted_cache_image().unwrap());
    assert_eq!(logits(original.accepted_logits().unwrap()), logits(resumed.accepted_logits().unwrap()));
    for (position, token) in [(3, 0), (4, 2), (5, 1)] {
        let left = original.advance(position, token).unwrap();
        let right = resumed.advance(position, token).unwrap();
        assert!(left.audit().complete_quiet() && right.audit().complete_quiet());
        assert_eq!(logits(&left.step().unwrap().logits), logits(&right.step().unwrap().logits));
        assert_eq!(left.step().unwrap().work, right.step().unwrap().work);
        same_cache(&original.accepted_cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
    }
    let independent = model.recompute(90, &[0, 1, 2, 0, 2, 1], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert_eq!(logits(independent.logits().unwrap()), logits(resumed.accepted_logits().unwrap()));
    same_cache(&independent.cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
    // A cache-insensitive toy would not detect a broken restore. This model's
    // logits actually depend on history, rather than only on the last token.
    let no_history = model.recompute(91, &[1], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    assert_ne!(logits(no_history.logits().unwrap()), logits(resumed.accepted_logits().unwrap()));
}

#[test]
fn fresh_incomplete_audit_cannot_release_a_previously_quiet_prefix() {
    let model = model();
    let quiet = policy(&model, false, 1, 4096);
    let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    original.advance(0, 0).unwrap();
    original.advance(1, 1).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let valid = saved.begin_restart(22, restart_budget(&quiet)).unwrap();
    assert!(valid.is_ready());
    valid.finish().unwrap();
    let mut budget = restart_budget(&quiet);
    budget.audit.monitoring.probe_coordinates = 0;
    let blocked = saved.begin_restart(23, budget).unwrap();
    assert!(!blocked.is_ready());
    assert_eq!(blocked.audit().unwrap().monitoring().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(blocked.audit().unwrap().monitoring().planned_rows(), 8);
    assert!(matches!(blocked.finish(), Err(Error::Incomplete)));
    assert_eq!(original.status(), LearnedDecoderStatus::Active);
    assert_eq!(original.position(), 2);
    // An independent fresh preparation can succeed; the blocked object itself
    // has neither a budget mutation nor an executable-session accessor.
    assert!(saved.begin_restart(24, restart_budget(&quiet)).unwrap().finish().is_ok());
}

#[test]
fn snapshot_and_restart_cache_limits_and_fresh_source_checks_are_not_optional() {
    let model = model();
    let quiet = policy(&model, false, 1, 4096);
    let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    original.advance(0, 0).unwrap();
    original.advance(1, 1).unwrap();
    let values = original.accepted_cache_image().unwrap().normalized_values();
    let saved = original.checkpoint_kv(DecoderRestoreBudget { cache_values: values }).unwrap();
    assert!(matches!(original.checkpoint_kv(DecoderRestoreBudget { cache_values: values - 1 }), Err(Error::Limit)));
    let mut exact = restart_budget(&quiet);
    exact.cache_values = values;
    exact.audit.preparation.source_check.source_values = values;
    assert!(saved.begin_restart(22, exact).unwrap().finish().is_ok());
    for budget in [KvRestartBudget { cache_values: values - 1, ..exact },
        KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES + 1, ..exact }] {
        assert!(matches!(saved.begin_restart(22, budget), Err(Error::Limit)));
    }
    let mut short = exact;
    short.audit.preparation.source_check.source_values -= 1;
    assert!(matches!(saved.begin_restart(22, short), Err(Error::Limit)));
    let mut compression = exact;
    compression.audit.preparation.compression.source_values = 0;
    assert!(matches!(saved.begin_restart(22, compression), Err(Error::Limit)));
    assert_eq!(original.position(), 2);
}

#[test]
fn destination_stream_preflight_preserves_split_and_derivation_lineage() {
    let model = model();
    let quiet = policy(&model, false, 1, 4096);
    let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    original.advance(0, 0).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    assert!(saved.begin_restart(22, restart_budget(&quiet)).unwrap().finish().is_ok());
    for stream in [0, 21] {
        assert!(matches!(saved.begin_restart(stream, restart_budget(&quiet)), Err(Error::InvalidInput)));
    }
    assert!(matches!(saved.begin_restart(11, restart_budget(&quiet)), Err(Error::Duplicate)));
    assert_eq!(saved.stream(), 21);
    assert_eq!(saved.evaluation_origin(), 201);
}

#[test]
fn full_prefix_row_capacity_cannot_be_replaced_with_only_a_quiet_last_token() {
    let model = model();
    let quiet = policy(&model, false, 1, 4);
    let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    original.advance(0, 0).unwrap();
    let one = original.checkpoint_kv(capture_limit()).unwrap();
    assert!(one.begin_restart(22, restart_budget(&quiet)).unwrap().finish().is_ok());
    original.advance(1, 1).unwrap();
    let two = original.checkpoint_kv(capture_limit()).unwrap();
    assert!(matches!(two.begin_restart(23, restart_budget(&quiet)), Err(Error::Limit)));
    assert_eq!(original.position(), 2);
    assert_eq!(original.status(), LearnedDecoderStatus::Active);
}

#[test]
fn an_old_checkpoint_cannot_clear_a_source_hold_or_bypass_a_future_alarm() {
    let model = model();
    let alarm = policy(&model, true, 1, 4096);
    let mut original = model.monitored_session(21, 201, alarm.clone()).unwrap();
    assert!(original.advance(0, 0).unwrap().step().is_some());
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let held = original.advance(1, 2).unwrap();
    assert!(held.step().is_none());
    assert_eq!(original.status(), LearnedDecoderStatus::Held(MonitorOutcome::Alarm));
    assert!(matches!(original.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
    let (mut resumed, _) = saved.begin_restart(22, restart_budget(&alarm)).unwrap().finish().unwrap();
    assert!(resumed.advance(1, 2).unwrap().step().is_none());
    assert_eq!(resumed.status(), original.status());
    assert_eq!(resumed.position(), 1);
    assert_eq!(original.advance(1, 0).err(), Some(Error::WrongState));
    assert!(matches!(resumed.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
}

#[test]
fn empty_prefix_is_only_admission_and_stale_calls_do_not_change_restored_state() {
    let model = model();
    let quiet = policy(&model, false, 1, 4096);
    let original = model.monitored_session(21, 201, quiet.clone()).unwrap();
    let saved = original.checkpoint_kv(DecoderRestoreBudget { cache_values: 0 }).unwrap();
    let empty = saved.begin_restart(22, KvRestartBudget { cache_values: 0, audit: quiet.allowance() }).unwrap();
    assert!(empty.is_ready());
    assert!(empty.audit().is_none());
    let (mut resumed, receipt) = empty.finish().unwrap();
    assert!(receipt.audit().is_none());
    assert_eq!(receipt.restoration().bytes_written, 0);
    assert_eq!(resumed.position(), 0);
    assert_eq!(resumed.accepted_logits(), Err(Error::Incomplete));
    assert_eq!(resumed.advance(1, 0).err(), Some(Error::Stale));
    assert_eq!(resumed.position(), 0);
    assert_eq!(resumed.status(), LearnedDecoderStatus::Active);
    assert!(resumed.advance(0, 0).unwrap().step().is_some());
    assert_eq!(original.position(), 0);
}

#[test]
fn none_all_and_structural_head_retention_keep_their_original_policy() {
    let model = model();
    for retention in 0..3 {
        let quiet = policy(&model, false, retention, 4096);
        let mut original = model.monitored_session(21, 201, quiet.clone()).unwrap();
        original.advance(0, 0).unwrap();
        original.advance(1, 1).unwrap();
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let (mut resumed, receipt) = saved.begin_restart(22, restart_budget(&quiet)).unwrap().finish().unwrap();
        assert!(receipt.audit().unwrap().monitoring().complete_quiet());
        let left = original.advance(2, 2).unwrap();
        let right = resumed.advance(2, 2).unwrap();
        assert_eq!(logits(&left.step().unwrap().logits), logits(&right.step().unwrap().logits));
        same_cache(&original.accepted_cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
    }
}

mod sampled {
    use super::*;
    use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
        SamplingPolicy, SamplingStart, SampledToken,
        monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop,
            GenerationTelemetryBudget, LearnedGeneration},
    };

    fn generation(model: &DecoderModel, policy: LearnedDecoderPolicy, prompt: Vec<u32>, top_k: usize,
        stop: bool, telemetry: GenerationTelemetryBudget) -> LearnedGeneration
    {
        let spec = GenerationSpec::new(prompt, 4, if stop { BTreeSet::from([2]) } else { BTreeSet::new() },
            SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(),
                stream: 71, seed: 173 }).unwrap();
        // Exact original whole-run numerical allowance, not a default slack cap.
        let estimate = model.estimate_monitored_generation(&spec).unwrap();
        let budget = GenerationBudget { decoder_products: estimate.decoder.scalar_products().unwrap(),
            vocabulary_scores: estimate.vocabulary_scores };
        model.monitored_generation_with_telemetry(21, 201, spec, policy, budget, telemetry).unwrap()
    }
    fn sample_eq(left: &SampledToken, right: &SampledToken) {
        assert_eq!(left.token, right.token);
        assert_eq!(left.stream, right.stream);
        assert_eq!(left.draw, right.draw);
        assert_eq!(left.random_word, right.random_word);
        assert_eq!(left.probability.to_bits(), right.probability.to_bits());
        assert_eq!(left.work, right.work);
    }
    fn same_generation(left: &LearnedGeneration, right: &LearnedGeneration) {
        assert_eq!(left.status(), right.status());
        assert_eq!(left.position(), right.position());
        assert_eq!(left.accepted_tokens(), right.accepted_tokens());
        assert_eq!(left.sampler_state(), right.sampler_state());
        assert_eq!(left.work(), right.work());
        assert_eq!(left.telemetry_work(), right.telemetry_work());
        assert_eq!(left.budget(), right.budget());
        assert_eq!(left.telemetry_budget(), right.telemetry_budget());
        assert_eq!(left.samples().len(), right.samples().len());
        for (a, b) in left.samples().iter().zip(right.samples()) { sample_eq(a, b); }
        match (left.accepted_logits(), right.accepted_logits()) {
            (Ok(a), Ok(b)) => assert_eq!(logits(a), logits(b)),
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("only one generation has accepted logits"),
        }
        same_cache(&left.accepted_cache_image().unwrap(), &right.accepted_cache_image().unwrap());
    }

    #[test]
    fn every_prompt_sample_and_terminal_cut_continues_with_exact_rng_and_lifetime_spend() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        for cut in 0..=6 {
            let mut original = generation(&model, quiet.clone(), vec![0, 1], 3, false,
                GenerationTelemetryBudget::default());
            for position in 0..cut { original.advance(position).unwrap(); }
            let saved = original.checkpoint_kv(capture_limit()).unwrap();
            assert_eq!(saved.position(), cut);
            assert_eq!(saved.status(), original.status());
            let before_work = original.work();
            let before_telemetry = original.telemetry_work();
            let preparation = saved.begin_restart(22, restart_budget(&quiet)).unwrap();
            assert!(preparation.is_ready());
            assert_eq!(preparation.audit().is_some(), cut > 0);
            let (mut resumed, receipt) = preparation.finish().unwrap();
            same_generation(&original, &resumed);
            assert!(resumed.last_event().is_none());
            assert_eq!(receipt.historical_work(), before_work);
            assert_eq!(receipt.historical_telemetry(), before_telemetry);
            assert_eq!(receipt.sampler_draws(), original.sampler_state().draws());
            assert_eq!(receipt.status(), original.status());
            assert_eq!(receipt.kv().restoration().position, cut);
            assert_eq!(receipt.kv().evaluation_origin(), 201);
            while original.status().is_active() {
                let position = original.position();
                let a = original.advance(position).unwrap();
                let b = resumed.advance(position).unwrap();
                assert!(a.audit().complete_quiet() && b.audit().complete_quiet());
                match (a.sample(), b.sample()) {
                    (Some(a), Some(b)) => sample_eq(a, b),
                    (None, None) => {}
                    _ => panic!("only one arm sampled"),
                }
                same_generation(&original, &resumed);
            }
            assert_eq!(resumed.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
            assert_eq!(resumed.work().reserved_decoder_products, resumed.budget().decoder_products);
            assert_eq!(resumed.work().reserved_vocabulary_scores, resumed.budget().vocabulary_scores);
            assert_eq!(resumed.advance(resumed.position()).err(), Some(Error::WrongState));
        }
    }

    #[test]
    fn repeated_native_restarts_cannot_reset_draws_or_original_numerical_allowances() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        let mut uninterrupted = generation(&model, quiet.clone(), vec![0, 1], 3, false,
            GenerationTelemetryBudget::default());
        let mut current = generation(&model, quiet.clone(), vec![0, 1], 3, false,
            GenerationTelemetryBudget::default());
        for position in 0..6 {
            uninterrupted.advance(position).unwrap();
            current.advance(position).unwrap();
            let saved = current.checkpoint_kv(capture_limit()).unwrap();
            let (next, receipt) = saved.begin_restart(22 + position, restart_budget(&quiet)).unwrap().finish().unwrap();
            assert_eq!(receipt.historical_work().admitted_tokens, position + 1);
            same_generation(&uninterrupted, &next);
            assert_eq!(current.work(), receipt.historical_work());
            current = next;
        }
        assert_eq!(current.sampler_state().draws(), 4);
        assert_eq!(current.work().admitted_tokens, 6);
        assert_eq!(current.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
    }

    #[test]
    fn restart_preserves_exhausted_aggregate_telemetry_and_does_not_commit_failed_draws() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        let mut control = generation(&model, quiet.clone(), vec![0], 3, false, GenerationTelemetryBudget::default());
        control.advance(0).unwrap();
        let one = control.telemetry_work().source_check_values;
        assert!(one > 0);
        assert!(control.advance(1).unwrap().accepted().is_some());
        let mut limited = generation(&model, quiet.clone(), vec![0], 3, false,
            GenerationTelemetryBudget { source_check_values: one, ..GenerationTelemetryBudget::default() });
        limited.advance(0).unwrap();
        let saved = limited.checkpoint_kv(capture_limit()).unwrap();
        let (mut resumed, receipt) = saved.begin_restart(22, restart_budget(&quiet)).unwrap().finish().unwrap();
        assert_eq!(receipt.historical_telemetry().source_check_values, one);
        assert!(receipt.kv().audit().unwrap().monitoring().complete_quiet());
        let sampler = limited.sampler_state();
        assert_eq!(limited.advance(1).err(), Some(Error::Limit));
        assert_eq!(resumed.advance(1).err(), Some(Error::Limit));
        same_generation(&limited, &resumed);
        assert_eq!(resumed.sampler_state(), sampler);
        assert_eq!(resumed.work().admitted_tokens, 2);
        assert_eq!(resumed.work().accepted_decoder.tokens, 1);
        assert_eq!(resumed.telemetry_work().source_check_values, one);
        assert!(matches!(resumed.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
        assert_eq!(resumed.advance(1).err(), Some(Error::WrongState));
    }

    #[test]
    fn eos_checkpoint_remains_terminal_instead_of_becoming_a_fresh_generation() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        let mut original = generation(&model, quiet.clone(), vec![0], 1, true, GenerationTelemetryBudget::default());
        assert_eq!(original.run_to_stop(), Ok(GenerationStatus::Finished(GenerationStop::StopToken(2))));
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let (mut resumed, receipt) = saved.begin_restart(22, restart_budget(&quiet)).unwrap().finish().unwrap();
        same_generation(&original, &resumed);
        assert_eq!(receipt.sampler_draws(), 1);
        assert_eq!(resumed.position(), 2);
        assert_eq!(resumed.run_to_stop(), Ok(GenerationStatus::Finished(GenerationStop::StopToken(2))));
        assert_eq!(resumed.advance(2).err(), Some(Error::WrongState));
        same_generation(&original, &resumed);
    }

    #[test]
    fn earlier_checkpoint_does_not_clear_a_later_sampled_hold_or_expose_held_token() {
        let model = model();
        let alarm = policy(&model, true, 1, 4096);
        let mut original = generation(&model, alarm.clone(), vec![0], 1, false, GenerationTelemetryBudget::default());
        original.advance(0).unwrap();
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let sampler = original.sampler_state();
        let event = original.advance(1).unwrap();
        assert!(matches!(original.status(), GenerationStatus::Held(MonitorOutcome::Alarm)));
        assert!(event.accepted().is_none() && event.sample().is_none());
        assert!(matches!(original.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
        let (mut resumed, _) = saved.begin_restart(22, restart_budget(&alarm)).unwrap().finish().unwrap();
        let event = resumed.advance(1).unwrap();
        assert!(event.accepted().is_none() && event.sample().is_none());
        assert_eq!(resumed.sampler_state(), sampler);
        same_generation(&original, &resumed);
        assert_eq!(original.advance(1).err(), Some(Error::WrongState));
        assert_eq!(resumed.advance(1).err(), Some(Error::WrongState));
    }

    #[test]
    fn generation_cannot_escape_a_blocked_full_prefix_preparation() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        let mut original = generation(&model, quiet.clone(), vec![0, 1], 3, false, GenerationTelemetryBudget::default());
        original.advance(0).unwrap();
        original.advance(1).unwrap();
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let work = original.work();
        let sampler = original.sampler_state();
        let mut budget = restart_budget(&quiet);
        budget.audit.monitoring.probe_coordinates = 0;
        let blocked = saved.begin_restart(22, budget).unwrap();
        assert!(!blocked.is_ready());
        assert_eq!(blocked.audit().unwrap().monitoring().planned_rows(), 8);
        assert!(matches!(blocked.finish(), Err(Error::Incomplete)));
        let (resumed, _) = saved.begin_restart(23, restart_budget(&quiet)).unwrap().finish().unwrap();
        same_generation(&original, &resumed);
        assert_eq!(original.work(), work);
        assert_eq!(original.sampler_state(), sampler);
    }

    #[test]
    fn stale_call_after_restart_cannot_spend_resources_or_skip_a_prompt_position() {
        let model = model();
        let quiet = policy(&model, false, 1, 4096);
        let mut original = generation(&model, quiet.clone(), vec![0, 1], 3, false, GenerationTelemetryBudget::default());
        original.advance(0).unwrap();
        assert_eq!(original.status(), GenerationStatus::Prefilling);
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        let (mut resumed, _) = saved.begin_restart(22, restart_budget(&quiet)).unwrap().finish().unwrap();
        for position in [0, 2] { assert_eq!(resumed.advance(position).err(), Some(Error::Stale)); }
        same_generation(&original, &resumed);
        original.run_to_stop().unwrap();
        resumed.run_to_stop().unwrap();
        same_generation(&original, &resumed);
    }
}
