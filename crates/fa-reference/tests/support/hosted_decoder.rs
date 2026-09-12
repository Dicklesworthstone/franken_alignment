//! Real reference arithmetic with synthetic parameters, probes and helper verdicts.
#[path = "decoder_control.rs"]
#[allow(dead_code)]
pub mod control;
pub use control::{Fixture, snapshot, spec, approved};
use control::numerical::{self, fixture};
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart};
use std::collections::BTreeMap;

pub fn model() -> DecoderModel {
    let p = fixture::profile(32);
    let p = DecoderProfile::new(DecoderIdentity { model_generation: 1, tokenizer_generation: 1,
        ..p.identity() }, p.shape(), p.epsilon(), p.theta()).unwrap();
    fixture::model(p)
}
pub fn sampling(vocabulary: usize) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(1, 1, vocabulary, 1.0, 0, 1.0).unwrap(), stream: 99, seed: 0 }
}
pub fn compute() -> DecoderBudget { numerical::compute() }
pub fn budget(vocabulary: usize) -> SampleBudget {
    SampleBudget { decoder: compute(), sampling: SamplingBudget { vocabulary } }
}
pub fn monitored(model: DecoderModel, threshold: f32, budget: RefinementBudget) -> MonitoredSampledDecoder {
    let monitors = (1..=model.profile().shape().layers as u64).map(|layer| {
        let contract = model.residual_contract(layer).unwrap();
        let mut weights = vec![0.0; contract.dimensions()]; weights[0] = 1.0;
        (layer, RefinementMonitor::new(vec![LinearProbe::new(1, 1, contract.profile(), &weights,
            0.0, threshold).unwrap()], vec![23], numerical::allowance()).unwrap())
    }).collect::<BTreeMap<_, _>>();
    let start = sampling(model.profile().shape().vocabulary);
    MonitoredSampledDecoder::new(model, 7, 11, monitors, budget, start).unwrap()
}
pub fn quiet() -> MonitoredSampledDecoder { monitored(model(), 1_000_000.0, numerical::allowance()) }
pub fn alarm() -> MonitoredSampledDecoder {
    let p = DecoderProfile::new(model().profile().identity(), DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap();
    let layers = fixture::zero_layers(&p);
    monitored(DecoderModel::new(p, vec![0.0, 1.0, 1.0, 0.0], layers, vec![1.0; 2], vec![0.0; 4]).unwrap(),
        0.5, numerical::allowance())
}
