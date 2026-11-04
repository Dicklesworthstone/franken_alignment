//! Original mediated recovery sealed to one reset and its retained source cut.
//! Checked Stop wins at Ready before any numerical completion, Fence or role.
use super::cooperative::{Progress, finish_replay};
use super::{Config, Control, Deadline, ElapsedTick, FileOversight, Loaded,
    RecoveryReserve, debug, requirements};
use super::super::{stream_reset::{Request, SourceRetention}, topology};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRequirements;

#[inline(never)]
pub(in super::super) fn complete<F>(config: &Config, loaded: &Loaded,
    request: &Request, source: &SourceRetention<'_>, control: &mut Control,
    deadline: &Deadline, time: &mut F,
) -> Result<FileOversight, String>
where F: FnMut() -> ElapsedTick {
    let history = topology::history(loaded)?
        .ok_or("stream-reset source custody requires an explicit topology profile")?;
    let anchor = loaded.recovery_anchor.as_ref()
        .ok_or("stream-reset source custody requires an independent input history anchor")?;
    let expected = FileMediatedRequirements {
        oversight: requirements(config, loaded), topology: history.requirement.clone(),
        prediction: None, evaluation: None,
    };
    let mut pending = false;
    let run = {
        let mut checkpoint = |_: Option<Progress>| {
            deadline.check(time())?;
            pending |= control.recovery_pending_peer()?;
            Ok(())
        };
        checkpoint(None)?;
        let run = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
            .and_then(|run| run.require_stream_reset_checkpoint(source.saved,
                &request.intent, request.first_incident_sequence)).map_err(debug)?;
        finish_replay(run, &mut checkpoint, |run| run.progress(),
            |run, done| run.advance(done, 1), Ok)?
    };
    // The final Ready checkpoint is part of the same original bounded replay.
    // It cannot fall through to a live finish while a checked peer is retained.
    if pending {
        let target = run.into_recovery_stop(control.recovery_operation()?).map_err(debug)?;
        let outcome = control.stop_recovery(target, deadline, time)?;
        let mut terminal = outcome.terminal_message();
        if let Ok(history) = &outcome.application.result {
            // Both acknowledged archives are attempted despite a late deadline
            // or lost transport reply; neither error causes another Stop/reset.
            if let Err(error) = source.retain_stop(config, loaded, history, request) {
                terminal.push_str("; ");
                terminal.push_str(&error);
            }
        }
        return Err(terminal);
    }
    let (host, _roles) = run.finish_pending_stream_reset().map_err(debug)?;
    Ok(host)
}
