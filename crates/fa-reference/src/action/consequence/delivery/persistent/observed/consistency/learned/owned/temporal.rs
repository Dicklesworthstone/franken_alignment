//! Original learned text, checked K/V prediction and canonical numerical replay.
//! Synthetic probabilities exercise mechanics; they do not qualify a detector.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::{requests::{FileRequestDisposition,
    actor::{FileActorSupervisor, LearnedTextProposal}}};
use crate::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyParameters};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;

#[allow(dead_code)]
#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn prediction_config(required: bool, stream: Option<StreamProfile>, bytes: usize,
    probability: BinaryForecast) -> FileLearnedConsistencyConfig
{
    let (model, _, _) = recipe();
    let tensor = model.cache_profile().layers()[&1].keys();
    let mut base = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: tensor.profile(), weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: probability, at_threshold: probability, positive: probability },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap();
    if let Some(stream) = stream { base = base.with_stream_messages(stream).unwrap(); }
    let predictor = FileLearnedConsistencyConfig::new(base, 1, KvSide::Key,
        LearnedMonitorBudget { encoded_bytes: bytes, ..LearnedMonitorBudget::default() },
        LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap();
    if required { predictor.with_pre_output_forecast().unwrap() } else { predictor }
}
fn predictor(required: bool) -> FileLearnedConsistencyConfig {
    prediction_config(required, None, MAX_CHECKED_KV_BYTES, BinaryForecast::new(32768, 32768).unwrap())
}
fn setup(root: &Directory, predictor: &FileLearnedConsistencyConfig)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig)
{
    let generation = config();
    let (mut host, human) = FileOversight::create_with_learned_text(root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), predictor.clone()).unwrap();
    (host, human, observer, generation)
}
fn forecast(host: &mut FileOversight, role: &FileConsistencyObserver, request: u64)
    -> Result<Result<LearnedForecastReport, Error>, JournalError>
{
    let actor = host.learned_generation_inspection()?.numerical.actor_revision;
    let revision = host.revision();
    role.forecast_owned_learned_request(host, revision, request, actor)
}
fn canonical(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
fn replay(host: &FileOversight, generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig) -> Machine {
    let mut events = journal::decode(&host.profile, host.store.identity(), &canonical(host)).unwrap();
    bind_owned_history(&mut events, generation, predictor).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}

mod integration;
