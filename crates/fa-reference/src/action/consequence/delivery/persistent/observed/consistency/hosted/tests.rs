use super::super::{codec, FileConsistencyConfig, FileConsistencyParameters, ConsistencyEvent};
use crate::action::consequence::activation::{CaptureProfile, consistency::{BinaryForecast, ErrorBudget, ForecastRegistration}};
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::Error;

fn config() -> FileConsistencyConfig {
    let pair = BinaryForecast::new(1, 2).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 1, generation: 1, policy_generation: 1, event_prefix: vec![1],
            negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 7, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap()
}
#[test]
fn mode_is_exact_configuration_data_with_no_truncation_or_legacy_fallback() {
    let old = config(); let hosted = old.clone().with_hosted_residual(2).unwrap();
    assert_eq!(old.hosted_residual_layer(), None); assert_eq!(hosted.hosted_residual_layer(), Some(2));
    let mut independent = old.encoded().to_vec(); independent[7] = 2;
    independent.extend_from_slice(&2_u64.to_be_bytes());
    assert_eq!(hosted.encoded(), independent);
    assert_eq!(FileConsistencyConfig::from_bytes(&independent).unwrap(), hosted);
    for end in 0..independent.len() { assert!(FileConsistencyConfig::from_bytes(&independent[..end]).is_err()); }
    let last = independent.len() - 8; independent[last..].fill(0);
    assert_eq!(FileConsistencyConfig::from_bytes(&independent), Err(Error::InvalidInput));
    assert_eq!(old.clone().with_hosted_residual(0), Err(Error::InvalidInput));
    assert_eq!(hosted.with_hosted_residual(1), Err(Error::Duplicate));
    assert_eq!(FileConsistencyConfig::from_bytes(old.encoded()).unwrap(), old);
}
#[test]
fn owned_forecast_commands_contain_no_supplied_frame_and_match_independent_vectors() {
    for (tag, event) in [(4, ConsistencyEvent::ForecastHosted(9000, 2)),
        (5, ConsistencyEvent::ForecastHostedRequest(9000, 2))] {
        let mut expected = vec![tag]; expected.extend_from_slice(&9000_u64.to_be_bytes()); expected.extend_from_slice(&2_u64.to_be_bytes());
        let mut w = Writer::new(17); codec::write(&mut w, &event).unwrap(); assert_eq!(w.finish(), expected);
        let mut r = Reader::new(&expected); let decoded = codec::read(&mut r).unwrap(); r.end().unwrap();
        assert!(matches!((tag, decoded), (4, ConsistencyEvent::ForecastHosted(9000, 2))
            | (5, ConsistencyEvent::ForecastHostedRequest(9000, 2))));
        for end in 0..expected.len() { assert!(codec::read(&mut Reader::new(&expected[..end])).is_err()); }
        expected.push(0); let mut r = Reader::new(&expected); codec::read(&mut r).unwrap(); assert!(r.end().is_err());
    }
}
