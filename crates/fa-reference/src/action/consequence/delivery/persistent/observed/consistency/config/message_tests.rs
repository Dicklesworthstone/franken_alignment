use super::*;
fn stream() -> StreamProfile { StreamProfile::new(7, 1, 4, 1024, 4096).unwrap() }
fn base() -> FileConsistencyConfig {
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 11, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7, max_predictions: 16, max_prediction_age_ticks: 8 }).unwrap()
}
fn wrap(inner: &[u8]) -> Vec<u8> {
    let mut bytes = b"FACPRED\x03".to_vec();
    bytes.extend_from_slice(&(inner.len() as u32).to_be_bytes()); bytes.extend_from_slice(inner);
    for n in [7_u64, 1] { bytes.extend_from_slice(&n.to_be_bytes()); }
    for n in [4_u32, 1024, 4096] { bytes.extend_from_slice(&n.to_be_bytes()); }
    bytes
}

#[test]
fn independent_envelope_preserves_legacy_bytes_and_builder_order() {
    let raw = base(); let raw_bytes = raw.encoded().to_vec();
    let hosted = raw.clone().with_hosted_residual(1).unwrap();
    for inner in [raw.clone(), hosted.clone()] {
        let expected = wrap(inner.encoded());
        let config = inner.clone().with_stream_messages(stream()).unwrap();
        assert_eq!(config.encoded(), expected);
        assert_eq!(config.stream_message_profile(), Some(stream()));
        assert_eq!(config.hosted_residual_layer(), inner.hosted_residual_layer());
        assert_eq!(FileConsistencyConfig::from_bytes(&expected).unwrap(), config);
        assert_eq!(config.build().unwrap().model.profile(), inner.build().unwrap().model.profile());
        assert_eq!(config.clone().with_stream_messages(stream()), Err(Error::Duplicate));
    }
    assert_eq!(hosted.clone().with_stream_messages(stream()).unwrap(),
        raw.clone().with_stream_messages(stream()).unwrap().with_hosted_residual(1).unwrap());
    assert_eq!(raw.encoded(), raw_bytes); assert_eq!(&raw.encoded()[..8], b"FACPRED\x01");
    assert_eq!(&hosted.encoded()[..8], b"FACPRED\x02");
    assert_eq!(raw.stream_message_profile(), None); assert_eq!(hosted.stream_message_profile(), None);
}

#[test]
fn every_truncation_nested_wrapper_and_invalid_profile_refuse() {
    for inner in [base(), base().with_hosted_residual(1).unwrap()] {
        let bytes = wrap(inner.encoded());
        for end in 0..bytes.len() { assert!(FileConsistencyConfig::from_bytes(&bytes[..end]).is_err()); }
        let mut extra = bytes.clone(); extra.push(0); assert!(FileConsistencyConfig::from_bytes(&extra).is_err());
        assert!(FileConsistencyConfig::from_bytes(&wrap(&bytes)).is_err());
        let mut missing = bytes.clone(); let end = missing.len(); missing[end - 4..].fill(0);
        assert!(FileConsistencyConfig::from_bytes(&missing).is_err());
        let mut length = bytes.clone(); length[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(FileConsistencyConfig::from_bytes(&length).is_err());
        assert!(FileConsistencyConfig::from_bytes(&bytes).is_ok());
    }
}
