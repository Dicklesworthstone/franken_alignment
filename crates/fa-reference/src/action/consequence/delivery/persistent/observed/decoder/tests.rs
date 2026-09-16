//! Canonical inputs and exact numerical replay; no imported cache or verdict.
use super::*;
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SamplingBudget;
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
mod data { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/decoder_inputs.rs")); }

fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap()
}
fn config(threshold: f32) -> FileDecoderConfig {
    FileDecoderConfig::new(profile(), data::weights(), data::monitor(threshold), data::sampling(),
        5, DecoderBindingLimits::default()).unwrap()
}
fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }

#[test]
fn numerical_family_roundtrips_exact_inputs_and_rejects_truncation_and_empty_witnesses() {
    let events = [DecoderEvent::Enable(Rc::new(config(3.0))),
        DecoderEvent::Step(StepRequest::Forced { revision: 1, position: 0, token: 0, products: 200 }, Rc::from(&b"comparison-only"[..])),
        DecoderEvent::Step(StepRequest::Sampled { revision: 2, position: 1, products: 200, vocabulary: 2 }, Rc::from(&b"comparison-only"[..])),
        DecoderEvent::Resume { revision: 3, position: 2 }];
    for event in &events {
        let mut w = Writer::new(100_000); write(&mut w, event).unwrap(); let bytes = w.finish();
        let mut r = Reader::new(&bytes); let decoded = read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(100_000); write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
        for end in 0..bytes.len() { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
    let empty = DecoderEvent::Step(StepRequest::Forced { revision: 1, position: 0, token: 0, products: 1 }, Rc::from(&b""[..]));
    assert_eq!(write(&mut Writer::new(1000), &empty), Err(Error::Incomplete));
    assert!(read(&mut Reader::new(&[255])).is_err());
}

#[test]
fn original_recomputation_matches_cache_sampler_logits_scores_and_counts_exactly() {
    let c = config(3.0); let mut a = c.build().unwrap(); let mut b = c.build().unwrap();
    assert_eq!(a.replay_bytes().unwrap(), b.replay_bytes().unwrap());
    for run in [&mut a, &mut b] {
        assert!(matches!(run.advance_forced(0, 0, budget()).unwrap(), MonitoredStep::Released(_)));
        assert!(matches!(run.advance_sampled(1, SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 2 } }).unwrap(), MonitoredSampledStep::Released(_)));
    }
    assert_eq!(a.replay_bytes().unwrap(), b.replay_bytes().unwrap());
    let mut held = config(1.5).build().unwrap();
    held.advance_forced(0, 0, budget()).unwrap();
    assert!(matches!(held.advance_sampled(1, SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 2 } }).unwrap(), MonitoredSampledStep::Held(_)));
    assert_eq!(held.position(), a.position()); assert_eq!(held.sampled_draws(), a.sampled_draws());
    assert_ne!(held.replay_bytes().unwrap(), a.replay_bytes().unwrap());
}

#[test]
fn bad_weights_missing_monitor_layers_and_sampling_mismatch_never_build_an_owner() {
    assert!(config(3.0).build().is_ok());
    let mut weights = data::weights();
    let name = b"lm_head.weight";
    let offset = weights.windows(name.len()).position(|w| w == name).unwrap();
    weights[offset] = b'x';
    assert!(FileDecoderConfig::new(profile(), weights, data::monitor(3.0), data::sampling(), 5, DecoderBindingLimits::default()).is_err());
    assert!(FileDecoderConfig::new(profile(), data::weights(), b"{}".to_vec(), data::sampling(), 5, DecoderBindingLimits::default()).is_err());
    let sampling = String::from_utf8(data::sampling()).unwrap().replace("\"vocabulary\":2", "\"vocabulary\":3").into_bytes();
    assert!(FileDecoderConfig::new(profile(), data::weights(), data::monitor(3.0), sampling, 5, DecoderBindingLimits::default()).is_err());
    assert!(FileDecoderConfig::new(profile(), data::weights(), data::monitor(3.0), data::sampling(), 0, DecoderBindingLimits::default()).is_err());
}

#[path = "checkpoint_inspection/storage_tests.rs"]
mod checkpoint_storage_tests;
