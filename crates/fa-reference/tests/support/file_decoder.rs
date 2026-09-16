#![allow(dead_code)]
#[path = "file_oversight.rs"] mod oversight;
#[path = "decoder_inputs.rs"] pub mod data;
pub use oversight::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity,
    DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer, decoder::FileDecoderConfig};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;

pub fn numerical_profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap()
}
pub fn configuration(threshold: f32) -> FileDecoderConfig {
    FileDecoderConfig::new(numerical_profile(), data::weights(), data::monitor(threshold),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap()
}
pub fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn sample_budget() -> SampleBudget { SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 2 } } }
pub fn create_decoder(root: &Directory, threshold: f32) -> (FileOversight, FileHumanReviewer, FileDecoderConfig) {
    let config = configuration(threshold);
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer, config)
}
pub fn forced(host: &mut FileOversight, token: u32) -> MonitoredStep {
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, token, budget()).unwrap().unwrap()
}
pub fn sampled(host: &mut FileOversight) -> MonitoredSampledStep {
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_sampled(host.revision(), n.actor_revision, n.position, sample_budget()).unwrap().unwrap()
}
