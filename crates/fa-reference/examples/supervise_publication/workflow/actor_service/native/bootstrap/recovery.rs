//! Bound native text recovery to one historical event per operator checkpoint.
//! The cursor keeps the original store lock and all selected recipe contracts;
//! this driver cannot inspect the candidate machine or issue live authority.

use super::{Config, FileHumanReviewer, FileOversight, GeneratedSourceProfile, Inputs, debug};
use super::super::{Control, Deadline, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    stream::generated::checked::source::FileGeneratedSourceRecoveryStatus,
};

pub(in super::super) fn open_with_checkpoints<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    control: Option<&mut Control>,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(FileOversight, FileHumanReviewer), String>
where
    F: FnMut() -> ElapsedTick,
{
    open_with_history(config, inputs, selected, None, control, deadline, time)
}

/// Preserve the original single-release service's selected archive custody.
pub(in super::super) fn open_with_history<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    history: Option<&super::super::anchors::Selection>,
    control: Option<&mut Control>,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(FileOversight, FileHumanReviewer), String>
where
    F: FnMut() -> ElapsedTick,
{
    open_selected(
        config, inputs, selected, control, deadline, time,
        history.and_then(|selection| selection.input()),
        history.map_or(TerminalRetention::Original, TerminalRetention::Service),
    )
}

/// Resident callers retain their acknowledged terminal result after the same
/// original replay loop. A failed replay supplies no replacement archive.
pub(in super::super) fn open_with_retained_history<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    control: Option<&mut Control>,
    deadline: &Deadline,
    time: &mut F,
    anchor: Option<&FileHistoryAnchor>,
    stopped: Option<&mut Option<FileHistoryAnchor>>,
) -> Result<(FileOversight, FileHumanReviewer), String>
where
    F: FnMut() -> ElapsedTick,
{
    open_selected(
        config, inputs, selected, control, deadline, time, anchor,
        stopped.map_or(TerminalRetention::Original, TerminalRetention::Resident),
    )
}

enum TerminalRetention<'a> {
    Original,
    Service(&'a super::super::anchors::Selection),
    Resident(&'a mut Option<FileHistoryAnchor>),
}

fn open_selected<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    mut control: Option<&mut Control>,
    deadline: &Deadline,
    time: &mut F,
    anchor: Option<&FileHistoryAnchor>,
    terminal: TerminalRetention<'_>,
) -> Result<(FileOversight, FileHumanReviewer), String>
where
    F: FnMut() -> ElapsedTick,
{
    deadline.check(time())?;
    let mut recovery = match anchor {
        Some(anchor) => FileOversight::begin_open_generated_text_stream_from_source_anchored(
            &config.store, config.profile.clone(), &inputs.decoder,
            &inputs.tokenizer, selected, anchor,
        ),
        None => FileOversight::begin_open_generated_text_stream_from_source(
            &config.store, config.profile.clone(), &inputs.decoder,
            &inputs.tokenizer, selected,
        ),
    }.map_err(debug)?;
    let total = recovery.progress().total_events;
    let mut pending = false;
    loop {
        let progress = recovery.progress();
        if progress.total_events != total
            || progress.replayed_events > total
            || (progress.status == FileGeneratedSourceRecoveryStatus::Ready
                && progress.replayed_events != total)
        {
            return Err("invalid original native recovery progress".into());
        }
        match progress.status {
            FileGeneratedSourceRecoveryStatus::Failed(error) => return Err(debug(error)),
            FileGeneratedSourceRecoveryStatus::Interrupted
            | FileGeneratedSourceRecoveryStatus::Cancelled => {
                return Err("native recovery interrupted before live custody".into());
            }
            FileGeneratedSourceRecoveryStatus::Replaying
            | FileGeneratedSourceRecoveryStatus::Ready => {}
        }
        // Ready has its own checkpoint: a Stop arriving after the last replay
        // step still wins before cleanup, Fence or reviewer-role construction.
        deadline.check(time())?;
        if let Some(control) = control.as_deref_mut() {
            pending |= control.recovery_pending_peer()?;
        }
        if progress.status == FileGeneratedSourceRecoveryStatus::Ready {
            if pending {
                let control = control.ok_or("native recovery Stop custody is unavailable")?;
                let target = recovery
                    .into_recovery_stop(control.recovery_operation()?)
                    .map_err(debug)?;
                return Err(match terminal {
                    TerminalRetention::Service(selected) => control.stop_recovery_with_history(
                        target, Some(selected), deadline, time,
                    )?,
                    TerminalRetention::Resident(retained) => control.stop_recovery_with_anchor(
                        target, deadline, time, retained,
                    )?,
                    TerminalRetention::Original => control.stop_recovery(target, deadline, time)?,
                });
            }
            return recovery.finish().map_err(debug);
        }
        let next = recovery
            .advance(progress.replayed_events, 1)
            .map_err(debug)?;
        if next != recovery.progress()
            || next.total_events != total
            || next.replayed_events < progress.replayed_events
            || next.replayed_events > progress.replayed_events.saturating_add(1)
            || (next.replayed_events == progress.replayed_events
                && next.status == FileGeneratedSourceRecoveryStatus::Replaying)
        {
            return Err("native recovery exceeded its single-event quantum".into());
        }
    }
}
