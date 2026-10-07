//! Independent wire shapes, bounded wrapper composition and actual runtime use.
use super::*;
use super::super::{FileConsistencyParameters, BinaryForecast, ErrorBudget, ForecastRegistration, CaptureProfile};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::consistency::ConsistencyStopPolicy;

fn parameters() -> FileConsistencyParameters {
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 },
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10 }
}
fn precision() -> ProgressiveForecastPolicy {
    ProgressiveForecastPolicy::new(0, 4, 23, MAX_PROGRESSIVE_FORECAST_BYTES).unwrap()
}
fn wrap(inner: &[u8], p: ProgressiveForecastPolicy) -> Vec<u8> {
    let mut bytes = b"FACPRED\x06".to_vec();
    bytes.extend_from_slice(&(inner.len() as u64).to_be_bytes()); bytes.extend_from_slice(inner);
    bytes.extend_from_slice(&[p.initial_bits(), p.refinement_bits(), p.maximum_bits()]);
    bytes.extend_from_slice(&(p.max_encoded_bytes() as u64).to_be_bytes()); bytes
}

#[test]
fn progressive_configuration_has_independent_golden_bytes_and_no_default_change() {
    let base = FileConsistencyConfig::new(parameters()).unwrap(); let before = base.encoded().to_vec();
    assert_eq!(base.progressive_forecast_policy(), None); assert_eq!(base.build().unwrap().model.progressive_policy(), None);
    let selected = base.clone().with_progressive_forecast(precision()).unwrap();
    assert_eq!(selected.encoded(), wrap(&before, precision()));
    assert_eq!(selected.progressive_forecast_policy(), Some(precision()));
    assert_eq!(selected.build().unwrap().model.progressive_policy(), Some(precision()));
    assert_eq!(FileConsistencyConfig::from_bytes(selected.encoded()), Ok(selected.clone()));
    assert_eq!(base.encoded(), before);
    assert_eq!(selected.with_progressive_forecast(precision()), Err(Error::Duplicate));
}

#[test]
fn all_supported_builder_orders_keep_stop_outermost_and_timing_source_and_stream_exact() {
    let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let stop = ConsistencyStopPolicy::new(9, 1, 900).unwrap();
    let base = FileConsistencyConfig::new(parameters()).unwrap().with_hosted_residual(1).unwrap();
    let expected = base.clone().with_stream_messages(stream).unwrap().with_pre_output_forecast().unwrap()
        .with_progressive_forecast(precision()).unwrap().with_terminal_stop(stop).unwrap();
    let mut orders = 0;
    for a in 0..4 { for b in 0..4 { for c in 0..4 { for d in 0..4 {
        let order = [a, b, c, d];
        if order.iter().copied().collect::<std::collections::BTreeSet<_>>().len() != 4 { continue; }
        let mut config = base.clone();
        for op in order {
            config = match op {
                0 => config.with_progressive_forecast(precision()).unwrap(),
                1 => config.with_terminal_stop(stop).unwrap(),
                2 => config.with_stream_messages(stream).unwrap(),
                _ => config.with_pre_output_forecast().unwrap(),
            };
        }
        assert_eq!(config, expected); assert!(config.requires_pre_output_forecast());
        assert_eq!(config.hosted_residual_layer(), Some(1)); assert_eq!(config.stream_message_profile(), Some(stream));
        assert_eq!(config.terminal_stop_policy(), Some(stop));
        assert_eq!(config.build().unwrap().model.progressive_policy(), Some(precision())); orders += 1;
    }}}}
    assert_eq!(orders, 24);
    let early = FileConsistencyConfig::new(parameters()).unwrap().with_progressive_forecast(precision()).unwrap()
        .with_stream_messages(stream).unwrap().with_terminal_stop(stop).unwrap().with_hosted_residual(1).unwrap()
        .with_pre_output_forecast().unwrap();
    assert_eq!(early, expected);
    for original in [base.clone(), base.clone().with_pre_output_forecast().unwrap(),
        base.with_stream_messages(stream).unwrap().with_terminal_stop(stop).unwrap()] {
        assert_eq!(original.progressive_forecast_policy(), None);
        assert_eq!(FileConsistencyConfig::from_bytes(original.encoded()), Ok(original));
    }
}

#[test]
fn truncation_suffixes_invalid_caps_and_recursive_or_reordered_wrappers_never_downgrade() {
    let base = FileConsistencyConfig::new(parameters()).unwrap().with_hosted_residual(1).unwrap();
    let good = base.clone().with_progressive_forecast(precision()).unwrap();
    for length in 0..good.encoded().len() { assert!(FileConsistencyConfig::from_bytes(&good.encoded()[..length]).is_err()); }
    let mut suffix = good.encoded().to_vec(); suffix.push(0);
    assert!(FileConsistencyConfig::from_bytes(&suffix).is_err());
    assert!(FileConsistencyConfig::from_bytes(&wrap(good.encoded(), precision())).is_err());
    let stopped = base.clone().with_terminal_stop(ConsistencyStopPolicy::new(9, 1, 900).unwrap()).unwrap();
    assert!(FileConsistencyConfig::from_bytes(&wrap(stopped.encoded(), precision())).is_err());
    for (offset, value) in [(0, 24), (1, 0), (1, 24), (2, 24)] {
        let mut bad = good.encoded().to_vec(); let start = bad.len() - 11; bad[start + offset] = value;
        assert!(FileConsistencyConfig::from_bytes(&bad).is_err());
    }
    for bytes in [0_u64, (MAX_PROGRESSIVE_FORECAST_BYTES as u64) + 1, u64::MAX] {
        let mut bad = good.encoded().to_vec(); let start = bad.len() - 8;
        bad[start..].copy_from_slice(&bytes.to_be_bytes());
        assert!(FileConsistencyConfig::from_bytes(&bad).is_err());
    }
    for domain in [b"FACPRED\x03", b"FACPRED\x05"] {
        let mut wrong = domain.to_vec(); wrong.extend_from_slice(&(good.encoded().len() as u64).to_be_bytes());
        wrong.extend_from_slice(good.encoded());
        assert!(FileConsistencyConfig::from_bytes(&wrong).is_err());
    }
    assert!(FileConsistencyConfig::from_bytes(&vec![0; MAX_CONFIG_BYTES + 1]).is_err());
    assert_eq!(FileConsistencyConfig::from_bytes(good.encoded()), Ok(good));
}

mod runtime;
