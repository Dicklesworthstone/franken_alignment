//! Synthetic numerical parameters/probes, not learned-detector qualification.
#[path = "decoder_fixture.rs"]
#[allow(dead_code)]
pub mod fixture;
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredDecoder;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS};
use std::collections::BTreeMap;

pub fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn allowance() -> RefinementBudget { RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 1_000_000 } }
pub fn monitored(model: DecoderModel, threshold: f32, budget: RefinementBudget) -> MonitoredDecoder {
    let monitors: BTreeMap<_, _> = (1..=model.profile().shape().layers as u64).map(|layer| {
        let contract = model.residual_contract(layer).unwrap();
        let mut weights = vec![0.0; contract.dimensions()]; weights[0] = 1.0;
        (layer, RefinementMonitor::new(vec![LinearProbe::new(1, 1, contract.profile(), &weights,
            0.0, threshold).unwrap()], vec![23], allowance()).unwrap())
    }).collect();
    MonitoredDecoder::new(model, 7, 11, monitors, budget).unwrap()
}
pub fn quiet() -> MonitoredDecoder {
    monitored(fixture::model(fixture::profile(16)), 1_000_000.0, allowance())
}
