//! Small ORIGINAL numerical model and fitted monitor recipe for transport tests.
#[path = "decoder_fixture.rs"]
mod decoder;
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{
        DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{
            SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget},
            replay::ReplayableGeneration,
        },
    },
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn generation() -> ReplayableGeneration {
    generation_with(173, GenerationTelemetryBudget::default())
}

pub fn generation_with(seed: u64, telemetry: GenerationTelemetryBudget) -> ReplayableGeneration {
    let model = decoder::model(decoder::profile(16));
    let spec = GenerationSpec::new(vec![4, 0, 3], 5, BTreeSet::new(), SamplingStart {
        policy: SamplingPolicy::new(7, 2, model.profile().shape().vocabulary, 0.8, 4, 1.0).unwrap(),
        stream: 71, seed,
    }).unwrap();
    construct(model, spec, false, telemetry)
}

pub fn controlled_generation(alarm: bool) -> ReplayableGeneration {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 32 }, 0.00001, 10000.0).unwrap();
    let mut layers = decoder::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    let model = DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
    let spec = GenerationSpec::new(vec![0], 4, BTreeSet::from([2]), SamplingStart {
        policy: SamplingPolicy::new(7, 2, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
    }).unwrap();
    construct(model, spec, alarm, GenerationTelemetryBudget::default())
}

fn construct(model: DecoderModel, spec: GenerationSpec, alarm: bool,
    telemetry: GenerationTelemetryBudget) -> ReplayableGeneration
{
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let source = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
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
    let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap();
    model.replayable_monitored_generation(21, 201, spec, policy, GenerationBudget::default(), telemetry).unwrap()
}

pub fn equivalent(left: &ReplayableGeneration, right: &ReplayableGeneration) {
    let a = left.generation();
    let b = right.generation();
    assert_eq!(a.accepted_tokens(), b.accepted_tokens());
    assert_eq!(a.samples(), b.samples());
    for (a, b) in a.samples().iter().zip(b.samples()) {
        assert_eq!(a.probability.to_bits(), b.probability.to_bits());
    }
    assert_eq!(a.sampler_state().encode(), b.sampler_state().encode());
    assert_eq!(a.status(), b.status());
    assert_eq!(a.work(), b.work());
    assert_eq!(a.telemetry_work(), b.telemetry_work());
    assert_eq!(a.budget(), b.budget());
    assert_eq!(a.telemetry_budget(), b.telemetry_budget());
    let bits = |values: &[f32]| values.iter().map(|value| value.to_bits()).collect::<Vec<_>>();
    assert_eq!(a.accepted_logits().map(bits), b.accepted_logits().map(bits));
    assert_eq!(a.accepted_cache_image().unwrap().encode().unwrap(), b.accepted_cache_image().unwrap().encode().unwrap());
}
