//! One original anchored guarded replay, consumed into observations only.
use super::cooperative::{Progress, finish_replay};
use super::{Config, FileOversight, Loaded, RecoveryReserve, debug, requirements};
use super::super::stream_checkpoint::{self, Request};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedStreamControlHistory;

pub(in super::super) fn execute(config: &Config, loaded: &Loaded, request: &Request,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>)
    -> Result<FileLearnedStreamControlHistory, String>
{
    stream_checkpoint::check_selection(loaded)?;
    checkpoint(None)?;
    let anchor = loaded.recovery_anchor.as_ref().ok_or("missing independent stream-control history anchor")?;
    let run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        &config.store, config.profile.clone(), &requirements(config, loaded),
        &loaded.generation, anchor)
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(debug)?;
    finish_replay(run, checkpoint, |run| run.progress(), |run, done| run.advance(done, 1),
        |run| match request {
            Request::Capture(expected) => run.finish_captured_stream_checkpoint(*expected),
            Request::Begin(expected) => run.finish_begun_stream_reset(
                &expected.intent, expected.first_incident_sequence),
        })
}
