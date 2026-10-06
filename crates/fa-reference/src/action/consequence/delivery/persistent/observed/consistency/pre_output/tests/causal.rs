//! A later residual selects a different probability band: no post-output repair.
use super::*;

#[test]
fn later_sample_residual_cannot_replace_the_prompt_forecast_that_scores_the_event() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let registered = FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model().residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"visible".to_vec(), negative: neutral, at_threshold: neutral,
            positive: BinaryForecast::new(16_384, 49_152).unwrap() },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap();
    let role = host.enable_action_consistency(host.revision(), registered.clone()).unwrap();
    step(&mut host).unwrap(); let mut run = begin(&role, &mut host); let revision = host.revision();
    let sampled = run.advance(&mut host, revision).unwrap().unwrap();
    let late = registered.build().unwrap().model.predict(sampled.accepted().unwrap().layers[0].residual.source()).unwrap();
    assert_ne!(run.prediction().forecast(), late.forecast());
    assert_eq!(late.forecast(), neutral);
    let prior = run.prediction().clone(); let spec = action_spec(&host);
    host.submit_request(host.revision(), 71, spec, snapshot()).unwrap();
    let observed = host.action_consistency_observation(1).unwrap();
    assert_eq!(observed.prediction(), &prior);
    assert_eq!(observed.factor(), prior.forecast().factor(true));
    assert!(observed.crossed(), "a later neutral forecast must not erase the original crossing");
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); same(&host, &config);
}
