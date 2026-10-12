//! Existing prompt-only forecasts must not strand an admitted second message.
use super::*;
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use fa_reference::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyParameters, learned::FileLearnedConsistencyConfig,
};

#[path = "forecast_messages.rs"]
mod messages;
#[path = "forecast/owned_native.rs"]
mod owned_native;
#[path = "forecast_reserve.rs"]
mod reserve;

enum Forecast { Residual(FileConsistencyConfig), Learned(FileLearnedConsistencyConfig) }
impl Forecast {
    fn new(owned: bool) -> Self {
        let model = finite_model(); let neutral = BinaryForecast::new(32768, 32768).unwrap();
        let capture = if owned { model.cache_profile().layers()[&1].values().profile() }
            else { model.residual_contract(1).unwrap().profile() };
        let parameters = FileConsistencyParameters { probe_id: 91, probe_generation: 1,
            profile: capture, weights: vec![0.0; 6], bias: 0.0, threshold: 0.0,
            forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
                event_prefix: b"A".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
            alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
            max_predictions: 8, max_prediction_age_ticks: 10 };
        let config = FileConsistencyConfig::new(parameters).unwrap().with_stream_messages(stream()).unwrap();
        if owned {
            Self::Learned(FileLearnedConsistencyConfig::new(config, 1, KvSide::Value,
                LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
                .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap())
        } else {
            Self::Residual(config.with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap())
        }
    }
    fn pin(&self, config: FileLearnedConfig) -> Result<FileLearnedConfig, Error> {
        match self {
            Self::Residual(forecast) => config.with_required_pre_output_forecast(forecast.clone()),
            Self::Learned(forecast) => config.with_required_owned_pre_output_forecast(forecast.clone()),
        }
    }
    fn enable(&self, host: &mut FileOversight) -> Result<(), JournalError> {
        let revision = host.revision();
        match self {
            Self::Residual(forecast) => host.enable_action_consistency(revision, forecast.clone()).map(|_| ()),
            Self::Learned(forecast) => host.enable_learned_action_consistency(revision, forecast.clone()).map(|_| ()),
        }
    }
}

#[test]
fn prompt_only_forecasts_refuse_finite_message_composition_in_both_orders_and_original_installation() {
    for owned in [false, true] {
        let forecast = Forecast::new(owned);
        let legacy = finite_recipe(4, false, false).with_required_sidecar().unwrap();
        let pinned = forecast.pin(legacy.clone()).unwrap();
        let control = Directory::new();
        let (host, _, _) = FileOversight::create_with_pre_output_forecast(
            control.store(), profile(), pinned.clone()).unwrap();
        assert!(host.action_consistency_required());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
        assert_eq!(pinned.with_text_stream_continuation().err(), Some(Error::Binding));
        let continued = legacy.with_text_stream_continuation().unwrap();
        assert_eq!(forecast.pin(continued.clone()).err(), Some(Error::Binding));
        let root = Directory::new();
        let (mut host, _) = FileOversight::create_with_learned_text_stream(
            root.store(), profile(), continued).unwrap();
        let files = root.files(); let revision = host.revision();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(forecast.enable(&mut host).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(root.files(), files); assert_eq!(host.revision(), revision);
        assert!(!host.action_consistency_required());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
}
