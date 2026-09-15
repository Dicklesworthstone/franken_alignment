//! Numerical fixtures, not trained-detector or serving-host qualification.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget,
    LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity,
    DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy,
    LearnedStreamRetention}, sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart,
    monitored::{GenerationBudget, GenerationPhase, GenerationSpec, GenerationStatus, GenerationStop}}};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{LearnedKvCodec, LearnedKvPolicy, FitBudget};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn alarm_model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 32 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    // Teacher-forced 0 produces a unique highest logit for 2. Token 2 then
    // introduces a Y-axis value omitted by an X-axis rank-one training set.
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn policy(model: &DecoderModel, selected: Option<(Vec<f32>, f32)>, retention: LearnedStreamRetention,
    budget: LearnedAuditBudget) -> LearnedDecoderPolicy
{
    let source = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, source)]), FitBudget::default()).unwrap();
    let profile = model.cache_profile();
    let mut taps = BTreeMap::new();
    for (layer, contract) in profile.layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let (weights, threshold) = match &selected {
                Some(value) if *layer == 2 && side == KvSide::Value => value.clone(),
                _ => (vec![0.0; tensor.dimensions()], 1.0),
            };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(profile.clone(), taps, budget).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, retention, LearnedAuditPreparationBudget::default(), inference()).unwrap()
}
fn quiet(model: &DecoderModel) -> LearnedDecoderPolicy {
    policy(model, None, LearnedStreamRetention::None, LearnedAuditBudget::default())
}
fn start(vocabulary: usize, seed: u64, top_k: usize, top_p: f64) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(1, 1, vocabulary, 0.8, top_k, top_p).unwrap(), stream: 71, seed }
}
fn spec(model: &DecoderModel, prompt: Vec<u32>, count: usize, stops: BTreeSet<u32>) -> GenerationSpec {
    GenerationSpec::new(prompt, count, stops, start(model.profile().shape().vocabulary, 19, 1, 1.0)).unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }

#[test]
fn quiet_prompt_and_sampling_match_the_original_engine_bit_for_bit() {
    let model = fixture::model(fixture::profile(16));
    let prompt = vec![4, 0, 3];
    let sampling = start(6, 173, 4, 0.75);
    let spec = GenerationSpec::new(prompt.clone(), 5, BTreeSet::new(), sampling.clone()).unwrap();
    let estimate = model.estimate_monitored_generation(&spec).unwrap();
    let exact_budget = GenerationBudget { decoder_products: estimate.decoder.scalar_products().unwrap(),
        vocabulary_scores: estimate.vocabulary_scores };
    let mut guarded = model.monitored_generation(21, 201, spec, quiet(&model), exact_budget).unwrap();
    let mut ordinary = model.sampled_session(21, sampling).unwrap();
    let mut source_values = 0;
    for position in 0..8_u64 {
        let event = guarded.advance(position).unwrap();
        let computation = if let Some(token) = prompt.get(position as usize) {
            assert_eq!(event.phase(), GenerationPhase::Prompt); assert!(event.sample().is_none());
            assert_eq!(guarded.sampler_state().draws(), 0);
            ordinary.advance_forced(position, *token, inference()).unwrap()
        } else {
            let step = ordinary.advance_sampled(position, SampleBudget { decoder: inference(),
                sampling: SamplingBudget { vocabulary: 6 } }).unwrap();
            assert_eq!(event.phase(), GenerationPhase::Continuation);
            assert_eq!(event.sample(), Some(&step.choice));
            step.computation
        };
        assert_eq!(bits(&event.accepted().unwrap().logits), bits(&computation.logits));
        assert_eq!(event.accepted().unwrap().work, computation.work);
        assert!(event.audit().complete_quiet()); assert_eq!(event.audit().planned_rows(), 4);
        assert_eq!(event.audit().first_position(), position); assert_eq!(event.audit().end_position(), position + 1);
        source_values += event.audit().source().report().source_values;
        assert_eq!(guarded.accepted_tokens(), ordinary.tokens());
        assert_eq!(guarded.sampler_state().encode(), ordinary.sampler_state().encode());
        assert_eq!(guarded.accepted_cache_image().unwrap().encode().unwrap(), ordinary.cache_image().unwrap().encode().unwrap());
    }
    assert_eq!(guarded.generated_tokens(), &ordinary.tokens()[prompt.len()..]);
    assert_eq!(source_values, 8 * model.cache_profile().values_per_token());
    assert_eq!(guarded.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
    assert_eq!(guarded.samples().len(), 5); assert_eq!(guarded.work().sampling_attempts, 5);
    assert_eq!(guarded.work().accepted_decoder, ordinary.work());
    assert_eq!(guarded.work().reserved_decoder_products, exact_budget.decoder_products);
    assert_eq!(guarded.work().reserved_vocabulary_scores, exact_budget.vocabulary_scores);
    assert_eq!(guarded.work().admitted_tokens, 8);
}

#[test]
fn segmentation_and_terminal_polling_do_not_change_draws_or_repeat_inference() {
    let model = fixture::model(fixture::profile(16));
    let spec = GenerationSpec::new(vec![1, 3], 6, BTreeSet::new(), start(6, 88, 0, 1.0)).unwrap();
    let frozen = quiet(&model);
    let mut segmented = model.monitored_generation(21, 201, spec.clone(), frozen.clone(), GenerationBudget::default()).unwrap();
    let mut continuous = model.monitored_generation(21, 201, spec, frozen, GenerationBudget::default()).unwrap();
    segmented.advance(0).unwrap();
    let old = segmented.advance(1).unwrap();
    segmented.advance(2).unwrap();
    let stale_work = segmented.work();
    let stale_rng = segmented.sampler_state();
    assert_eq!(segmented.advance(2).unwrap_err(), Error::Stale);
    assert_eq!(segmented.work(), stale_work); assert_eq!(segmented.sampler_state(), stale_rng);
    assert_eq!(segmented.run_to_stop().unwrap(), continuous.run_to_stop().unwrap());
    assert_eq!(segmented.samples(), continuous.samples());
    assert_eq!(segmented.accepted_tokens(), continuous.accepted_tokens());
    assert_eq!(segmented.sampler_state(), continuous.sampler_state());
    assert_eq!(segmented.work(), continuous.work());
    assert_eq!(segmented.accepted_cache_image().unwrap().encode().unwrap(), continuous.accepted_cache_image().unwrap().encode().unwrap());
    let work = segmented.work();
    let rng = segmented.sampler_state();
    assert_eq!(segmented.advance(segmented.position()).unwrap_err(), Error::WrongState);
    assert_eq!(segmented.run_to_stop().unwrap(), GenerationStatus::Finished(GenerationStop::TokenLimit));
    assert_eq!(segmented.work(), work); assert_eq!(segmented.sampler_state(), rng);
    assert_eq!(old.position(), 1); assert_eq!(old.status(), GenerationStatus::Generating);
    assert!(old.audit().complete_quiet()); assert!(old.sample().is_none());
}

#[test]
fn a_prompt_alarm_prevents_sampling_and_cannot_be_skipped() {
    let model = alarm_model();
    let policy = policy(&model, Some((vec![0.0, 1.0], 0.5)), LearnedStreamRetention::All, LearnedAuditBudget::default());
    let spec = spec(&model, vec![2, 0], 3, BTreeSet::new());
    let mut run = model.monitored_generation(21, 201, spec, policy, GenerationBudget::default()).unwrap();
    let before = run.accepted_cache_image().unwrap().encode().unwrap();
    let rng = run.sampler_state();
    let event = run.advance(0).unwrap();
    assert_eq!(event.phase(), GenerationPhase::Prompt);
    assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(event.accepted().is_none()); assert!(event.sample().is_none());
    assert_eq!(event.audit().quiet_rows(), 3); assert_eq!(event.audit().planned_rows(), 4);
    assert!(run.accepted_tokens().is_empty()); assert!(run.generated_tokens().is_empty());
    assert_eq!(run.accepted_logits(), Err(Error::Incomplete));
    assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(run.sampler_state(), rng); assert_eq!(run.work().sampling_attempts, 0);
    assert_eq!(run.work().reserved_vocabulary_scores, 0); assert_eq!(run.work().accepted_decoder.tokens, 0);
    let work = run.work();
    assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
    assert_eq!(run.run_to_stop().unwrap(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(run.work(), work);
}

#[test]
fn rejected_sample_keeps_rng_and_accepted_cache_unchanged_but_spends_reservations() {
    let model = alarm_model();
    let policy = policy(&model, Some((vec![0.0, 1.0], 0.5)), LearnedStreamRetention::All, LearnedAuditBudget::default());
    let spec = spec(&model, vec![0], 4, BTreeSet::from([2]));
    let mut oracle = model.recompute_sampled(21, spec.prompt(), inference(), spec.sampling().clone()).unwrap();
    assert_eq!(oracle.advance_sampled(1, SampleBudget { decoder: inference(), sampling: SamplingBudget { vocabulary: 3 } })
        .unwrap().choice.token, 2);
    let mut run = model.monitored_generation(21, 201, spec, policy, GenerationBudget::default()).unwrap();
    let accepted = run.advance(0).unwrap();
    let before = run.accepted_cache_image().unwrap().encode().unwrap();
    let logits = bits(run.accepted_logits().unwrap());
    let rng = run.sampler_state();
    let event = run.advance(1).unwrap();
    assert_eq!(event.phase(), GenerationPhase::Continuation);
    assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(event.accepted().is_none()); assert!(event.sample().is_none());
    assert_eq!(event.audit().work().refinements, 1);
    assert_eq!(run.accepted_tokens(), &[0]); assert!(run.generated_tokens().is_empty()); assert!(run.samples().is_empty());
    assert_eq!(run.sampler_state(), rng); assert_eq!(run.sampler_state().draws(), 0);
    assert_eq!(bits(run.accepted_logits().unwrap()), logits);
    assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(run.work().admitted_tokens, 2); assert_eq!(run.work().sampling_attempts, 1);
    assert_eq!(run.work().reserved_vocabulary_scores, 3); assert_eq!(run.work().accepted_decoder.tokens, 1);
    assert_eq!(run.work().reserved_decoder_products, model.estimate(0, 2).unwrap().scalar_products().unwrap());
    let work = run.work();
    assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
    assert_eq!(run.advance(2).unwrap_err(), Error::WrongState);
    assert_eq!(run.run_to_stop().unwrap(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(run.work(), work);
    assert!(accepted.accepted().is_some()); assert_eq!(accepted.position(), 0);
    assert!(!format!("{event:?}").contains("random_word"));
    assert!(!format!("{run:?}").contains("LearnedDecoderEvent"));
}

#[test]
fn stop_tokens_are_honored_only_after_auditing_the_accepted_continuation() {
    let model = alarm_model();
    for limit in [1, 5] {
        let spec = spec(&model, vec![0], limit, BTreeSet::from([0, 2]));
        let mut run = model.monitored_generation(21, 201, spec, quiet(&model), GenerationBudget::default()).unwrap();
        let prompt = run.advance(0).unwrap();
        assert_eq!(prompt.status(), GenerationStatus::Generating);
        assert_eq!(run.sampler_state().draws(), 0);
        let stop = run.advance(1).unwrap();
        assert_eq!(stop.status(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
        assert_eq!(stop.sample().unwrap().token, 2); assert_eq!(stop.accepted().unwrap().token, 2);
        assert!(stop.audit().complete_quiet()); assert_eq!(stop.audit().planned_rows(), 4);
        assert_eq!(run.generated_tokens(), &[2]); assert_eq!(run.samples().len(), 1);
        assert_eq!(run.sampler_state().draws(), 1); assert_eq!(run.work().accepted_decoder.tokens, 2);
        assert_eq!(run.work().reserved_vocabulary_scores, 3);
        let work = run.work();
        let state = run.sampler_state();
        assert_eq!(run.run_to_stop().unwrap(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
        assert_eq!(run.advance(2).unwrap_err(), Error::WrongState);
        assert_eq!(run.work(), work); assert_eq!(run.sampler_state(), state);
    }
}

#[test]
fn unresolved_threshold_and_exhausted_audits_cannot_become_an_eos_or_length_stop() {
    let model = alarm_model();
    for (retention, refinements, weights, threshold, expected) in [
        (LearnedStreamRetention::None, 128, vec![0.0, 1.0], 0.5, MonitorOutcome::Unresolved),
        (LearnedStreamRetention::All, 0, vec![0.0, 1.0], 0.5, MonitorOutcome::BudgetExhausted),
        (LearnedStreamRetention::All, 128, vec![-1.0, 0.0], 0.0, MonitorOutcome::AtThreshold),
    ] {
        let budget = LearnedAuditBudget { monitoring: LearnedMonitorBudget { refinements, ..LearnedMonitorBudget::default() },
            ..LearnedAuditBudget::default() };
        let frozen = policy(&model, Some((weights, threshold)), retention, budget);
        let spec = spec(&model, vec![0], 1, BTreeSet::from([2]));
        let mut run = model.monitored_generation(21, 201, spec, frozen, GenerationBudget::default()).unwrap();
        assert!(run.advance(0).unwrap().accepted().is_some());
        let before = run.accepted_cache_image().unwrap().encode().unwrap();
        let rng = run.sampler_state();
        let event = run.advance(1).unwrap();
        assert_eq!(event.status(), GenerationStatus::Held(expected));
        assert_eq!(event.audit().outcome(), expected);
        assert!(event.sample().is_none()); assert!(event.accepted().is_none());
        assert_eq!(run.generated_tokens(), &[]); assert_eq!(run.accepted_tokens(), &[0]);
        assert_eq!(run.sampler_state(), rng); assert_eq!(run.work().reserved_vocabulary_scores, 3);
        assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
        let work = run.work();
        assert_eq!(run.run_to_stop().unwrap(), GenerationStatus::Held(expected));
        assert_eq!(run.advance(1).unwrap_err(), Error::WrongState);
        assert_eq!(run.work(), work);
    }
}

#[test]
fn whole_run_and_largest_single_token_costs_are_admitted_before_prefill() {
    let model = alarm_model();
    let spec = spec(&model, vec![0], 5, BTreeSet::from([2]));
    let frozen = quiet(&model);
    let estimate = model.estimate_monitored_generation(&spec).unwrap();
    let exact = GenerationBudget { decoder_products: estimate.decoder.scalar_products().unwrap(),
        vocabulary_scores: estimate.vocabulary_scores };
    for budget in [
        GenerationBudget { decoder_products: exact.decoder_products - 1, ..exact },
        GenerationBudget { vocabulary_scores: exact.vocabulary_scores - 1, ..exact },
        GenerationBudget { decoder_products: MAX_DECODER_PRODUCTS + 1, ..exact },
        GenerationBudget { vocabulary_scores: 1_073_741_825, ..exact },
    ] {
        assert_eq!(model.monitored_generation(21, 201, spec.clone(), frozen.clone(), budget).unwrap_err(), Error::Limit);
    }
    let largest = model.estimate(estimate.audited_positions - 1, 1).unwrap().scalar_products().unwrap();
    let limited = LearnedDecoderPolicy::new(frozen.codec().clone(), frozen.monitor().clone(),
        LearnedStreamRetention::None, frozen.preparation(), DecoderBudget { scalar_products: largest - 1 }).unwrap();
    assert_eq!(model.monitored_generation(21, 201, spec.clone(), limited, exact).unwrap_err(), Error::Limit);
    let mut run = model.monitored_generation(21, 201, spec, frozen, exact).unwrap();
    assert_eq!(run.work().admitted_tokens, 0); assert_eq!(run.position(), 0);
    assert_eq!(run.estimate(), estimate); assert_eq!(run.budget(), exact);
    // The declared longest run was admitted, but actual early-stop charges are
    // only the two attempts that really entered the controlled execution path.
    assert_eq!(run.run_to_stop().unwrap(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
    assert_eq!(run.work().admitted_tokens, 2);
    assert!(run.work().reserved_decoder_products < exact.decoder_products);
    assert!(run.work().reserved_vocabulary_scores < exact.vocabulary_scores);
}

#[test]
fn invalid_original_ids_context_and_sampler_bindings_refuse_before_any_session_is_returned() {
    let model = alarm_model();
    let frozen = quiet(&model);
    for (request, expected) in [
        (spec(&model, vec![3], 2, BTreeSet::new()), Error::InvalidInput),
        (spec(&model, vec![0], 2, BTreeSet::from([3])), Error::InvalidInput),
        (spec(&model, vec![0], 32, BTreeSet::from([2])), Error::Limit),
        (GenerationSpec::new(vec![0], 2, BTreeSet::new(), start(4, 0, 0, 1.0)).unwrap(), Error::Binding),
    ] {
        assert_eq!(model.monitored_generation(21, 201, request, frozen.clone(), GenerationBudget::default()).unwrap_err(), expected);
    }
    let mut sampling = start(3, 0, 0, 1.0); sampling.stream = 0;
    let invalid = GenerationSpec::new(vec![0], 2, BTreeSet::new(), sampling).unwrap();
    assert_eq!(model.monitored_generation(21, 201, invalid, frozen.clone(), GenerationBudget::default()).unwrap_err(), Error::InvalidInput);
    let request = spec(&model, vec![0], 2, BTreeSet::new());
    for (stream, origin, expected) in [(11, 201, Error::Duplicate), (21, 101, Error::Duplicate),
        (0, 201, Error::InvalidInput), (21, 0, Error::InvalidInput)]
    {
        assert_eq!(model.monitored_generation(stream, origin, request.clone(), frozen.clone(), GenerationBudget::default())
            .unwrap_err(), expected);
    }
    assert_eq!(GenerationSpec::new(vec![], 1, BTreeSet::new(), start(3, 0, 0, 1.0)).unwrap_err(), Error::InvalidInput);
    assert_eq!(GenerationSpec::new(vec![0], 0, BTreeSet::new(), start(3, 0, 0, 1.0)).unwrap_err(), Error::InvalidInput);
    assert_eq!(GenerationSpec::new(vec![0], 4096, BTreeSet::new(), start(3, 0, 0, 1.0)).unwrap_err(), Error::Limit);
}

#[test]
fn preparation_failures_latch_and_cannot_be_reinterpreted_as_a_successful_stop() {
    let model = alarm_model();
    let frozen = quiet(&model);
    for source_check_failure in [false, true] {
        let mut preparation = frozen.preparation();
        if source_check_failure { preparation.source_check.encoded_bytes = 0; }
        else { preparation.compression.work_units = 0; }
        let limited = LearnedDecoderPolicy::new(frozen.codec().clone(), frozen.monitor().clone(),
            LearnedStreamRetention::None, preparation, inference()).unwrap();
        let spec = spec(&model, vec![0], 2, BTreeSet::new());
        let mut run = model.monitored_generation(21, 201, spec, limited, GenerationBudget::default()).unwrap();
        let before = run.accepted_cache_image().unwrap().encode().unwrap();
        let rng = run.sampler_state();
        assert_eq!(run.advance(0).unwrap_err(), Error::Limit);
        assert_eq!(run.status(), GenerationStatus::Failed(Error::Limit));
        assert_eq!(run.position(), 0); assert!(run.last_event().is_none());
        assert!(run.accepted_tokens().is_empty()); assert!(run.generated_tokens().is_empty());
        assert_eq!(run.work().admitted_tokens, 1); assert_eq!(run.work().accepted_decoder.tokens, 0);
        assert_eq!(run.work().reserved_decoder_products, model.estimate(0, 1).unwrap().scalar_products().unwrap());
        assert_eq!(run.work().sampling_attempts, 0);
        assert_eq!(run.sampler_state(), rng);
        assert_eq!(run.accepted_cache_image().unwrap().encode().unwrap(), before);
        let work = run.work();
        assert_eq!(run.run_to_stop().unwrap_err(), Error::Limit);
        assert_eq!(run.advance(0).unwrap_err(), Error::WrongState);
        assert_eq!(run.work(), work);
    }
}
