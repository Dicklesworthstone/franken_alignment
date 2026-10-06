use super::*;
use crate::action::consequence::delivery::persistent::observed::consistency::FileConsistencyParameters;
use crate::action::consequence::activation::{CaptureProfile,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::consistency::ConsistencyStopPolicy;

fn base() -> FileConsistencyConfig {
    let pair = BinaryForecast::new(1, 2).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 1, generation: 1, policy_generation: 1,
            event_prefix: vec![1], negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap()
}
fn wrap(inner: &[u8]) -> Vec<u8> {
    let mut w = Writer::new(MAX_CONFIG_BYTES); w.raw(b"FACPRED\x05").unwrap();
    w.blob(inner).unwrap(); w.finish()
}

#[test]
fn temporal_mode_preserves_exact_original_predictor_and_is_not_a_legacy_fallback() {
    let plain = base(); let original = plain.clone().with_hosted_residual(2).unwrap();
    let temporal = original.clone().with_pre_output_forecast().unwrap();
    assert_eq!(temporal.encoded(), wrap(original.encoded()));
    assert_eq!(parts(temporal.encoded()).unwrap(), original.encoded());
    assert_eq!(temporal.hosted_residual_layer(), Some(2));
    assert!(temporal.requires_pre_output_forecast()); assert!(!original.requires_pre_output_forecast());
    assert_eq!(FileConsistencyConfig::from_bytes(temporal.encoded()).unwrap(), temporal);
    assert_eq!(FileConsistencyConfig::from_bytes(original.encoded()).unwrap(), original);
    assert_eq!(FileConsistencyConfig::from_bytes(plain.encoded()).unwrap(), plain);
    assert_eq!(plain.with_pre_output_forecast(), Err(Error::Binding));
    assert_eq!(temporal.clone().with_pre_output_forecast(), Err(Error::Duplicate));
    assert_eq!(temporal.with_hosted_residual(1), Err(Error::Duplicate));
}

#[test]
fn stream_and_terminal_stop_composition_has_one_canonical_order() {
    let original = base().with_hosted_residual(2).unwrap();
    let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let stop = ConsistencyStopPolicy::new(1, 1, 900).unwrap();
    let a = original.clone().with_pre_output_forecast().unwrap().with_stream_messages(stream).unwrap()
        .with_terminal_stop(stop).unwrap();
    let b = original.clone().with_terminal_stop(stop).unwrap().with_stream_messages(stream).unwrap()
        .with_pre_output_forecast().unwrap();
    let c = original.with_stream_messages(stream).unwrap().with_pre_output_forecast().unwrap()
        .with_terminal_stop(stop).unwrap();
    assert_eq!(a, b); assert_eq!(b, c);
    assert!(a.requires_pre_output_forecast()); assert_eq!(a.hosted_residual_layer(), Some(2));
    assert_eq!(a.stream_message_profile(), Some(stream)); assert_eq!(a.terminal_stop_policy(), Some(stop));
    assert_eq!(FileConsistencyConfig::from_bytes(a.encoded()).unwrap(), a);
    assert_eq!(a.clone().with_pre_output_forecast(), Err(Error::Duplicate));
    assert_eq!(a.clone().with_stream_messages(stream), Err(Error::Duplicate));
    assert_eq!(a.with_terminal_stop(stop), Err(Error::Duplicate));
}

#[test]
fn malformed_truncated_recursive_or_supplied_frame_timing_profiles_are_rejected() {
    let hosted = base().with_hosted_residual(1).unwrap();
    let temporal = hosted.clone().with_pre_output_forecast().unwrap();
    for end in 0..temporal.encoded().len() {
        assert!(FileConsistencyConfig::from_bytes(&temporal.encoded()[..end]).is_err());
    }
    let mut suffix = temporal.encoded().to_vec(); suffix.push(0);
    assert!(FileConsistencyConfig::from_bytes(&suffix).is_err());
    for invalid in [wrap(base().encoded()), wrap(temporal.encoded()), wrap(b""),
        wrap(hosted.with_terminal_stop(ConsistencyStopPolicy::new(1, 1, 900).unwrap()).unwrap().encoded())] {
        assert!(FileConsistencyConfig::from_bytes(&invalid).is_err());
    }
}
