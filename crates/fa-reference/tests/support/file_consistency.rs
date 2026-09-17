#![allow(dead_code)]
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration, Prediction};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
};

pub fn parameters() -> FileConsistencyParameters {
    FileConsistencyParameters {
        probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
            positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16,
        max_prediction_age_ticks: 10,
    }
}
pub fn configuration() -> FileConsistencyConfig { FileConsistencyConfig::new(parameters()).unwrap() }
pub fn frame(host: &FileOversight, sequence: u64, value: f32) -> SourceFrame {
    let position = host.actor_snapshot().unwrap().state.next_position() - 1;
    SourceFrame::capture(FrameIdentity { profile: parameters().profile,
        stream: 17, sequence, position }, &[value]).unwrap()
}
pub fn forecast(host: &mut FileOversight, observer: &FileConsistencyObserver,
    attempt: u64, sequence: u64, value: f32) -> Prediction
{
    let source = frame(host, sequence, value);
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_action(host, revision, attempt, actor, &source).unwrap().unwrap()
}
