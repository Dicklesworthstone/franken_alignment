//! Original numerical execution and complete learned audits, not model safety.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor, model::{KvTap, LearnedAuditBudget,
    LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
    sampling::{SamplingPolicy, SamplingStart, SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop, GenerationTelemetryBudget},
        replay::{CheckpointLimits, ReplayBudget, ReplayableGeneration, ReplayStatus}},
};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{LearnedKvCodec, LearnedKvPolicy, FitBudget};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn policy(model: &DecoderModel, alarm: bool) -> LearnedDecoderPolicy {
    let source = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, source)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 2 && side == KvSide::Value {
                weights[1] = 1.0; 0.5
            } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference()).unwrap()
}
fn spec(model: &DecoderModel, prompt: Vec<u32>, count: usize, stops: BTreeSet<u32>, top_k: usize) -> GenerationSpec {
    GenerationSpec::new(prompt, count, stops, SamplingStart {
        policy: SamplingPolicy::new(7, 2, model.profile().shape().vocabulary, 0.8, top_k, 1.0).unwrap(),
        stream: 71, seed: 173,
    }).unwrap()
}
fn run(model: &DecoderModel, spec: GenerationSpec, alarm: bool, telemetry: GenerationTelemetryBudget) -> ReplayableGeneration {
    model.replayable_monitored_generation(21, 201, spec, policy(model, alarm),
        GenerationBudget::default(), telemetry).unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn equivalent(left: &ReplayableGeneration, right: &ReplayableGeneration) {
    let a = left.generation(); let b = right.generation();
    assert_eq!(a.accepted_tokens(), b.accepted_tokens());
    assert_eq!(a.samples(), b.samples());
    assert_eq!(a.sampler_state().encode(), b.sampler_state().encode());
    assert_eq!(a.status(), b.status());
    assert_eq!(a.work(), b.work());
    assert_eq!(a.telemetry_work(), b.telemetry_work());
    assert_eq!(a.budget(), b.budget());
    assert_eq!(a.telemetry_budget(), b.telemetry_budget());
    assert_eq!(a.accepted_logits().map(bits), b.accepted_logits().map(bits));
    assert_eq!(a.accepted_cache_image().unwrap().encode().unwrap(), b.accepted_cache_image().unwrap().encode().unwrap());
}

#[test]
fn every_prompt_sampling_and_terminal_cut_reconstructs_the_same_continuation() {
    let model = fixture::model(fixture::profile(16));
    let spec = spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4);
    let mut continuous = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
    continuous.run_to_stop().unwrap();
    for cut in 0..=8 {
        let mut original = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
        for position in 0..cut { original.advance(position).unwrap(); }
        let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
        assert_eq!(saved.positions(), cut as usize);
        let (mut restored, receipt) = saved.replay(ReplayBudget::default()).unwrap();
        assert_eq!(receipt.positions, cut as usize);
        assert_eq!(receipt.sampled_positions, (cut as usize).saturating_sub(3));
        assert_eq!(receipt.recomputation, original.generation().work());
        assert_eq!(receipt.telemetry_recomputation, original.generation().telemetry_work());
        assert_eq!(receipt.stream, 21); assert_eq!(receipt.evaluation_origin, 201);
        equivalent(&original, &restored);
        restored.run_to_stop().unwrap();
        equivalent(&continuous, &restored);
        // Reconstruction never mutates the checkpoint's original owner.
        assert_eq!(original.generation().position(), cut);
        original.run_to_stop().unwrap();
        equivalent(&original, &restored);
    }
}

#[test]
fn reconstructed_samples_match_the_original_unmonitored_numerical_oracle() {
    let model = fixture::model(fixture::profile(16));
    let spec = spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4);
    let mut original = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
    for position in 0..5 { original.advance(position).unwrap(); }
    let (mut restored, _) = original.checkpoint(CheckpointLimits::default()).unwrap()
        .replay(ReplayBudget::default()).unwrap();
    restored.run_to_stop().unwrap();
    let mut oracle = model.sampled_session(21, spec.sampling().clone()).unwrap();
    for (position, token) in spec.prompt().iter().enumerate() {
        oracle.advance_forced(position as u64, *token, inference()).unwrap();
    }
    let mut choices = Vec::new();
    for position in 3..8 {
        choices.push(oracle.advance_sampled(position, SampleBudget {
            decoder: inference(), sampling: SamplingBudget { vocabulary: 6 },
        }).unwrap().choice);
    }
    assert_eq!(restored.generation().accepted_tokens(), oracle.tokens());
    assert_eq!(restored.generation().samples(), choices);
    assert_eq!(restored.generation().sampler_state(), oracle.sampler_state());
    assert_eq!(bits(restored.generation().accepted_logits().unwrap()), bits(oracle.logits().unwrap()));
    assert_eq!(restored.generation().accepted_cache_image().unwrap().encode().unwrap(), oracle.cache_image().unwrap().encode().unwrap());
}

#[test]
fn segmented_reconstruction_cannot_release_a_partial_candidate_or_repeat_work() {
    let model = fixture::model(fixture::profile(16));
    let mut original = run(&model, spec(&model, vec![1, 2], 5, BTreeSet::new(), 0), false,
        GenerationTelemetryBudget::default());
    for position in 0..5 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let incomplete = saved.begin_replay(ReplayBudget::default()).unwrap();
    assert!(matches!(incomplete.finish(), Err(Error::Incomplete)));
    for quantum in [1, 2, usize::MAX] {
        let mut replay = saved.begin_replay(ReplayBudget::default()).unwrap();
        assert_eq!(replay.advance(0).unwrap(), ReplayStatus::Pending { compared: 0, remaining: 5 });
        assert!(replay.receipt().is_none());
        while replay.status() != ReplayStatus::Verified {
            replay.advance(quantum).unwrap();
        }
        let receipt = *replay.receipt().unwrap();
        assert_eq!(replay.advance(0).unwrap(), ReplayStatus::Verified);
        assert_eq!(replay.advance(usize::MAX).unwrap(), ReplayStatus::Verified);
        assert_eq!(replay.receipt(), Some(&receipt));
        let (restored, final_receipt) = replay.finish().unwrap();
        assert_eq!(final_receipt, receipt);
        equivalent(&original, &restored);
    }
}

#[test]
fn exact_retention_and_replay_caps_work_while_each_one_less_refuses() {
    let model = fixture::model(fixture::profile(16));
    let mut original = run(&model, spec(&model, vec![1, 2], 5, BTreeSet::new(), 0), false,
        GenerationTelemetryBudget::default());
    for position in 0..5 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let limits = CheckpointLimits { positions: 5, state_bytes: saved.state_bytes() };
    original.checkpoint(limits).unwrap();
    assert!(matches!(original.checkpoint(CheckpointLimits { positions: 4, ..limits }), Err(Error::Limit)));
    assert!(matches!(original.checkpoint(CheckpointLimits { state_bytes: limits.state_bytes - 1, ..limits }), Err(Error::Limit)));
    let budget = ReplayBudget { positions: 5, state_bytes: saved.state_bytes(),
        decoder_products: saved.work().reserved_decoder_products,
        vocabulary_scores: saved.work().reserved_vocabulary_scores };
    let (restored, _) = saved.replay(budget).unwrap(); equivalent(&original, &restored);
    for smaller in [
        ReplayBudget { positions: 4, ..budget },
        ReplayBudget { state_bytes: budget.state_bytes - 1, ..budget },
        ReplayBudget { decoder_products: budget.decoder_products - 1, ..budget },
        ReplayBudget { vocabulary_scores: budget.vocabulary_scores - 1, ..budget },
    ] { assert!(matches!(saved.begin_replay(smaller), Err(Error::Limit))); }
    equivalent(&original, &restored);
}

#[test]
fn replay_does_not_refill_consumed_source_check_bytes() {
    let model = fixture::model(fixture::profile(16));
    let spec = spec(&model, vec![1, 2], 5, BTreeSet::new(), 0);
    let mut measure = run(&model, spec.clone(), false, GenerationTelemetryBudget::default());
    measure.advance(0).unwrap();
    let spent = measure.generation().telemetry_work().source_check_encoded_bytes;
    assert!(spent > 0);
    let mut original = run(&model, spec, false, GenerationTelemetryBudget {
        source_check_encoded_bytes: spent, ..GenerationTelemetryBudget::default()
    });
    original.advance(0).unwrap();
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let (mut restored, _) = saved.replay(ReplayBudget::default()).unwrap();
    equivalent(&original, &restored);
    assert_eq!(restored.generation().telemetry_work().source_check_encoded_bytes, spent);
    assert_eq!(original.advance(1).unwrap_err(), Error::Limit);
    assert_eq!(restored.advance(1).unwrap_err(), Error::Limit);
    equivalent(&original, &restored);
    assert_eq!(restored.generation().status(), GenerationStatus::Failed(Error::Limit));
    assert!(matches!(restored.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
}

fn alarm_model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 32 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}

#[test]
fn old_checkpoint_does_not_clear_a_later_hold_or_skip_the_same_alarm_on_replay() {
    let model = alarm_model();
    let mut original = run(&model, spec(&model, vec![0], 4, BTreeSet::from([2]), 1), true,
        GenerationTelemetryBudget::default());
    original.advance(0).unwrap();
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let held = original.advance(1).unwrap();
    assert_eq!(held.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(held.accepted().is_none()); assert!(held.sample().is_none());
    assert!(matches!(original.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
    let work = original.generation().work();
    let (mut restored, _) = saved.replay(ReplayBudget::default()).unwrap();
    assert_eq!(original.generation().status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(original.generation().work(), work);
    let held_again = restored.advance(1).unwrap();
    assert_eq!(held_again.status(), held.status());
    assert!(held_again.accepted().is_none()); assert!(held_again.sample().is_none());
    equivalent(&original, &restored);
}

#[test]
fn a_completed_stop_is_terminal_after_reconstruction() {
    let model = alarm_model();
    let mut original = run(&model, spec(&model, vec![0], 4, BTreeSet::from([2]), 1), false,
        GenerationTelemetryBudget::default());
    assert_eq!(original.run_to_stop().unwrap(), GenerationStatus::Finished(GenerationStop::StopToken(2)));
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let (mut restored, receipt) = saved.replay(ReplayBudget::default()).unwrap();
    assert_eq!(receipt.restored_status, GenerationStatus::Finished(GenerationStop::StopToken(2)));
    equivalent(&original, &restored);
    assert_eq!(restored.advance(2).unwrap_err(), Error::WrongState);
    assert_eq!(restored.run_to_stop().unwrap(), receipt.restored_status);
    equivalent(&original, &restored);
}

#[path = "learned_generation_replay/archive.rs"]
mod archive;
