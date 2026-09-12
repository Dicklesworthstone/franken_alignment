//! Operator configuration feeds the same monitored stochastic owner.
#[path = "support/decoder_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder, MonitoredSampledStep};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::config::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use fa_reference::Error;

const CONFIG: &[u8] = include_bytes!("fixtures/decoder_sampling.json");
const MONITOR: &[u8] = include_bytes!("fixtures/decoder_monitor_quiet.json");
fn source() -> &'static str { std::str::from_utf8(CONFIG).unwrap().trim() }
fn changed(from: &str, to: &str) -> String {
    assert!(source().contains(from)); let changed = source().replace(from, to); assert_ne!(changed, source()); changed
}
fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }

#[test]
fn explicit_policy_and_zero_or_maximum_seed_are_preserved_without_defaults() {
    let config = SamplingConfig::decode(CONFIG, 6).unwrap(); let start = config.start();
    assert_eq!(start.policy.id(), 13); assert_eq!(start.policy.generation(), 2);
    assert_eq!(start.policy.vocabulary(), 6); assert_eq!(start.policy.temperature(), 0.7);
    assert_eq!(start.policy.top_k(), 4); assert_eq!(start.policy.top_p(), 1.0);
    assert_eq!(start.stream, 99); assert_eq!(start.seed, 0);
    let config = SamplingConfig::decode(changed("\"seed\":0", "\"seed\":18446744073709551615").as_bytes(), 6).unwrap();
    assert_eq!(config.start().seed, u64::MAX);
    let text = format!("{config:?}"); assert!(!text.contains("seed")); assert!(!text.contains("18446744073709551615"));
}

#[test]
fn every_field_is_required_and_extra_execution_or_authority_options_refuse() {
    SamplingConfig::decode(CONFIG, 6).unwrap();
    let entries: Vec<_> = source().strip_prefix('{').unwrap().strip_suffix('}').unwrap().split(',').collect();
    assert_eq!(entries.len(), 9);
    for omitted in 0..entries.len() {
        let remaining = entries.iter().enumerate().filter_map(|(index, value)| (index != omitted).then_some(*value)).collect::<Vec<_>>().join(",");
        let bytes = format!("{{{remaining}}}");
        assert!(matches!(SamplingConfig::decode(bytes.as_bytes(), 6), Err(SamplingConfigError::Field("$"))));
    }
    for field in ["permit", "rng_algorithm", "checkpoint", "executable", "default_seed"] {
        let bytes = format!("{},\"{field}\":true}}", source().strip_suffix('}').unwrap());
        assert!(matches!(SamplingConfig::decode(bytes.as_bytes(), 6), Err(SamplingConfigError::Field("$"))));
    }
}

#[test]
fn incompatible_vocabulary_invalid_policy_or_ambiguous_scalar_types_refuse() {
    assert!(matches!(SamplingConfig::decode(CONFIG, 5), Err(SamplingConfigError::Field("vocabulary"))));
    for (from, to) in [
        ("\"id\":13", "\"id\":0"), ("\"generation\":2", "\"generation\":2.0"),
        ("\"stream\":99", "\"stream\":0"), ("\"seed\":0", "\"seed\":-1"),
        ("\"seed\":0", "\"seed\":0.0"), ("\"seed\":0", "\"seed\":\"0\""),
        ("\"seed\":0", "\"seed\":null"), ("\"seed\":0", "\"seed\":18446744073709551616"),
        ("\"temperature\":0.7", "\"temperature\":0"), ("\"temperature\":0.7", "\"temperature\":1e999"),
        ("\"temperature\":0.7", "\"temperature\":\"0.7\""), ("\"top_k\":4", "\"top_k\":7"),
        ("\"top_p\":1.0", "\"top_p\":0"), ("\"top_p\":1.0", "\"top_p\":1.01"),
        ("fa.decoder-sampling/1", "fa.decoder-sampling/2"),
    ] { assert!(SamplingConfig::decode(changed(from, to).as_bytes(), 6).is_err(), "{to}"); }
    let full = SamplingConfig::decode(changed("\"top_k\":4", "\"top_k\":0").as_bytes(), 6).unwrap();
    assert_eq!(full.start().policy.top_k(), 0);
}

#[test]
fn duplicate_fields_truncations_and_nonobjects_never_produce_a_configuration() {
    SamplingConfig::decode(source().as_bytes(), 6).unwrap();
    for end in 0..source().len() { assert!(SamplingConfig::decode(&source().as_bytes()[..end], 6).is_err()); }
    assert!(matches!(SamplingConfig::decode(changed("\"seed\":0", "\"seed\":0,\"seed\":1").as_bytes(), 6), Err(SamplingConfigError::Syntax)));
    for bytes in [b"[]".as_slice(), b"null", b"false", b"1", b"{}{}"] {
        assert!(SamplingConfig::decode(bytes, 6).is_err());
    }
}

#[test]
fn exact_byte_limit_succeeds_and_excessive_bytes_or_nested_input_refuse() {
    let mut bytes = source().as_bytes().to_vec(); bytes.resize(MAX_SAMPLING_CONFIG_BYTES, b' ');
    SamplingConfig::decode(&bytes, 6).unwrap(); bytes.push(b' ');
    assert!(matches!(SamplingConfig::decode(&bytes, 6), Err(SamplingConfigError::Limit)));
    let deeply_nested = changed("\"seed\":0", "\"seed\":[[[0]]]");
    assert!(matches!(SamplingConfig::decode(deeply_nested.as_bytes(), 6), Err(SamplingConfigError::Limit)));
}

#[test]
fn both_configurations_initialize_only_an_empty_owner_then_use_the_original_sampler() {
    let model = fixture::model(fixture::profile(16));
    let mut run = MonitoredSampledDecoder::from_json(model.clone(), 7, MONITOR, CONFIG).unwrap();
    let mut raw = model.sampled_session(7, SamplingConfig::decode(CONFIG, 6).unwrap().start()).unwrap();
    assert_eq!(run.position(), 0); assert_eq!(run.sampled_draws(), 0);
    assert_eq!(run.observation().availability(), DecoderAvailability::Empty);
    for token in [0, 2, 1] {
        let position = run.position();
        assert!(matches!(run.advance_forced(position, token, compute()).unwrap(), MonitoredStep::Released(_)));
        raw.advance_forced(position, token, compute()).unwrap();
    }
    for _ in 0..8 {
        let budget = SampleBudget { decoder: compute(), sampling: SamplingBudget { vocabulary: 6 } };
        let position = run.position(); let expected = raw.advance_sampled(position, budget).unwrap();
        let MonitoredSampledStep::Released(actual) = run.advance_sampled(position, budget).unwrap() else { panic!("quiet fixture held"); };
        assert_eq!(actual.choice(), &expected.choice);
        assert_eq!(actual.reviewed().step().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.computation.logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    }
    assert_eq!(run.sampled_draws(), 8); assert_eq!(run.status(), MonitoringStatus::Ready);
    assert_eq!(run.observation().capture().unwrap().tokens(), raw.tokens());
}

#[test]
fn invalid_monitor_or_sampling_config_has_no_partial_owner_or_fallback() {
    let model = fixture::model(fixture::profile(8));
    assert!(matches!(MonitoredSampledDecoder::from_json(model.clone(), 7, b"", CONFIG), Err(SamplingConfigError::Monitor(_))));
    assert!(matches!(MonitoredSampledDecoder::from_json(model.clone(), 7, MONITOR, b""), Err(SamplingConfigError::Syntax)));
    assert!(matches!(MonitoredSampledDecoder::from_json(model.clone(), 0, MONITOR, CONFIG), Err(SamplingConfigError::Monitor(_))));
    let config = changed("\"vocabulary\":6", "\"vocabulary\":5");
    assert!(matches!(MonitoredSampledDecoder::from_json(model.clone(), 7, MONITOR, config.as_bytes()), Err(SamplingConfigError::Field("vocabulary"))));
    let run = MonitoredSampledDecoder::from_json(model, 7, MONITOR, CONFIG).unwrap();
    assert_eq!(run.position(), 0); assert_eq!(run.sampled_draws(), 0);
    assert_eq!(run.observation().capture().unwrap_err(), Error::Incomplete);
}
