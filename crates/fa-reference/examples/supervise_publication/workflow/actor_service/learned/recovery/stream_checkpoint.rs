//! One original anchored guarded replay, consumed into observations only.
use super::cooperative::{Progress, finish_replay};
use super::{Config, FileOversight, Loaded, RecoveryReserve, debug, requirements};
use super::super::{stream_checkpoint::{self, Request}, topology};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedStreamControlHistory;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::owned::FileOwnedPredictiveRequirements;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::TopologyCheckpoint,
    guarded::mediated::predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
        FileMediatedStreamControlRequest},
};

#[cfg(test)]
pub(in super::super) fn execute(config: &Config, loaded: &Loaded, request: &Request,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>)
    -> Result<FileLearnedStreamControlHistory, String>
{
    if loaded.topology.is_some() {
        return Err("mediated stream control requires paired result custody".into());
    }
    execute_paired(config, loaded, request, checkpoint).map(|(history, _)| history)
}

pub(in super::super) fn execute_paired(config: &Config, loaded: &Loaded, request: &Request,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>)
    -> Result<(FileLearnedStreamControlHistory, Option<TopologyCheckpoint>), String>
{
    stream_checkpoint::check_request_selection(loaded, request)?;
    checkpoint(None)?;
    let anchor = loaded.recovery_anchor.as_ref().ok_or("missing independent stream-control history anchor")?;
    if let Some(history) = topology::history(loaded)? {
        let saved = history.checkpoint().ok_or("mediated stream control requires an exact v2 topology checkpoint")?;
        if saved.history_anchor() != anchor {
            return Err("stream-control topology checkpoint and history anchor describe different cuts".into());
        }
        let prediction = loaded.generation.required_owned_pre_output_forecast()
            .ok_or("mediated stream control requires the original owned predictor")?;
        let expected = FilePredictiveMediatedRequirements {
            oversight: requirements(config, loaded), topology: history.requirement.clone(),
            prediction: FileMediatedPredictor::Owned(prediction.clone()), evaluation: None,
        };
        let request = match request {
            Request::Capture(expected) => FileMediatedStreamControlRequest::Capture(*expected),
            Request::Narrow(expected) => FileMediatedStreamControlRequest::Narrow(expected.clone()),
            Request::Begin(expected) => FileMediatedStreamControlRequest::BeginReset {
                intent: expected.intent.clone(), first_incident_sequence: expected.first_incident_sequence },
        };
        let run = FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
            .and_then(|run| run.require_stream_control_checkpoint(saved, request)).map_err(debug)?;
        return finish_replay(run, checkpoint, |run| run.progress(), |run, done| run.advance(done, 1),
            |run| run.finish_stream_control().map(|result| {
                let (control, saved) = result.into_parts(); (control, Some(saved))
            }));
    }
    if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        let expected = FileOwnedPredictiveRequirements {
            oversight: requirements(config, loaded), prediction: prediction.clone(), evaluation: None,
        };
        let run = FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(debug)?;
        return finish_replay(run, checkpoint, |run| run.progress(), |run, done| run.advance(done, 1),
            |run| (match request {
                Request::Capture(expected) => run.finish_captured_stream_checkpoint(*expected),
                Request::Narrow(expected) => run.finish_narrowed_stream_checkpoint(expected),
                Request::Begin(expected) => run.finish_begun_stream_reset(
                    &expected.intent, expected.first_incident_sequence),
            }).map(|history| (history, None)));
    }
    let run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        &config.store, config.profile.clone(), &requirements(config, loaded),
        &loaded.generation, anchor)
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(debug)?;
    finish_replay(run, checkpoint, |run| run.progress(), |run, done| run.advance(done, 1),
        |run| (match request {
            Request::Capture(expected) => run.finish_captured_stream_checkpoint(*expected),
            Request::Narrow(_) => Err(fa_reference::action::consequence::delivery::persistent::JournalError::Contract(
                fa_reference::Error::Binding)),
            Request::Begin(expected) => run.finish_begun_stream_reset(
                &expected.intent, expected.first_incident_sequence),
        }).map(|history| (history, None)))
}
