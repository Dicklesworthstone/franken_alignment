use super::*;
use super::super::{FileConsistencyParameters, CaptureProfile, BinaryForecast, ErrorBudget,
    ForecastRegistration, StreamProfile, MAX_VALUES, MAX_EVENT_PREFIX_BYTES};

fn parameters() -> FileConsistencyParameters {
    FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(), positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10 }
}
fn profile() -> StreamProfile { StreamProfile::new(7, 1, 4, 1024, 4096).unwrap() }
fn policy() -> ConsistencyStopPolicy { ConsistencyStopPolicy::new(11, 12, 13).unwrap() }
// Independent wire construction: no production Writer or wrapper builder.
fn wire(inner: &[u8]) -> Vec<u8> {
    let mut bytes = b"FACPRED\x04".to_vec();
    bytes.extend_from_slice(&(inner.len() as u32).to_be_bytes()); bytes.extend_from_slice(inner);
    for value in [11_u64, 12, 13] { bytes.extend_from_slice(&value.to_be_bytes()); }
    bytes
}

#[test]
fn all_builder_orders_retain_the_same_exact_legacy_configuration_and_stop_policy() {
    let base = FileConsistencyConfig::new(parameters()).unwrap();
    let h = |c: FileConsistencyConfig| c.with_hosted_residual(4).unwrap();
    let m = |c: FileConsistencyConfig| c.with_stream_messages(profile()).unwrap();
    let s = |c: FileConsistencyConfig| c.with_terminal_stop(policy()).unwrap();
    let variants = [s(m(h(base.clone()))), s(h(m(base.clone()))), h(s(m(base.clone()))),
        h(m(s(base.clone()))), m(s(h(base.clone()))), m(h(s(base.clone())))];
    for actual in &variants {
        assert_eq!(actual, &variants[0]); assert_eq!(actual.hosted_residual_layer(), Some(4));
        assert_eq!(actual.stream_message_profile(), Some(profile()));
        assert_eq!(actual.terminal_stop_policy(), Some(policy()));
        assert_eq!(actual.build().unwrap().model.dimensions(), 1);
        assert_eq!(actual.clone().with_terminal_stop(policy()), Err(Error::Duplicate));
        assert_eq!(actual.clone().with_hosted_residual(4), Err(Error::Duplicate));
        assert_eq!(actual.clone().with_stream_messages(profile()), Err(Error::Duplicate));
    }
    for legacy in [base.clone(), h(base.clone()), m(base.clone()), m(h(base))] {
        let original = legacy.encoded().to_vec();
        assert_eq!(FileConsistencyConfig::from_bytes(&original).unwrap(), legacy);
        assert!(legacy.terminal_stop_policy().is_none());
        let configured = legacy.with_terminal_stop(policy()).unwrap();
        assert_eq!(configured.encoded(), wire(&original));
        assert_eq!(configured.without_stop(), original);
    }
}

#[test]
fn every_truncation_invalid_stop_identity_nested_wrapper_and_extra_suffix_refuses() {
    let original = FileConsistencyConfig::new(parameters()).unwrap().with_hosted_residual(4).unwrap()
        .with_stream_messages(profile()).unwrap();
    let bytes = wire(original.encoded());
    assert!(FileConsistencyConfig::from_bytes(&bytes).is_ok());
    for end in 0..bytes.len() { assert!(FileConsistencyConfig::from_bytes(&bytes[..end]).is_err(), "cut {end}"); }
    for field in 0..3 {
        let mut bad = bytes.clone(); let offset = bad.len() - 24 + field * 8;
        bad[offset..offset + 8].fill(0);
        assert_eq!(FileConsistencyConfig::from_bytes(&bad), Err(Error::InvalidInput));
    }
    let mut extra = bytes.clone(); extra.push(0); assert!(FileConsistencyConfig::from_bytes(&extra).is_err());
    assert_eq!(FileConsistencyConfig::from_bytes(&wire(&bytes)), Err(Error::Binding));
    let mut bad = bytes; bad[7] = 5; assert_eq!(FileConsistencyConfig::from_bytes(&bad), Err(Error::Binding));
    assert_eq!(original.with_terminal_stop(ConsistencyStopPolicy::new(11, 12, 14).unwrap()).unwrap()
        .terminal_stop_policy().unwrap().operation(), 14);
}

#[test]
fn maximal_original_coefficients_and_category_still_fit_all_optional_contracts() {
    let mut p = parameters(); p.weights = vec![1.0; MAX_VALUES];
    p.forecast.event_prefix = vec![b'r'; MAX_EVENT_PREFIX_BYTES];
    let configured = FileConsistencyConfig::new(p).unwrap().with_hosted_residual(4).unwrap()
        .with_stream_messages(profile()).unwrap().with_terminal_stop(policy()).unwrap();
    assert!(configured.encoded().len() <= MAX_CONFIG_BYTES);
    assert_eq!(FileConsistencyConfig::from_bytes(configured.encoded()).unwrap(), configured);
    let oversized = vec![0; MAX_CONFIG_BYTES + 1];
    assert_eq!(FileConsistencyConfig::from_bytes(&oversized), Err(Error::Limit));
}
