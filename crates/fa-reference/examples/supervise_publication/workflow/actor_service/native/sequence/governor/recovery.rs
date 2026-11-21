//! Drive exact guarded native reconstruction under the resident invocation's
//! original Stop and deadline. Only the final durable Fence returns fresh roles.
use super::super::super::{Config, Control, Deadline, ElapsedTick, FileOversight, Inputs, debug};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{FileOversightRoles, FileRecoveryRequirements, anchored::FileHistoryAnchor},
    stream::generated::checked::source::{
        FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile,
    },
};

pub(in super::super) fn open<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
    stopped: Option<&mut Option<FileHistoryAnchor>>,
) -> Result<(FileOversight, FileOversightRoles), String>
where
    F: FnMut() -> ElapsedTick,
{
    deadline.check(time())?;
    let mut recovery =
        FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
            &config.store,
            config.profile.clone(),
            expected,
            &inputs.decoder,
            &inputs.tokenizer,
            selected,
            anchor,
        )
        .map_err(debug)?;
    let total = recovery.progress().total_events;
    let mut pending = false;
    loop {
        let progress = recovery.progress();
        if progress.total_events != total
            || progress.replayed_events > total
            || (progress.status == FileGeneratedSourceRecoveryStatus::Ready
                && progress.replayed_events != total)
        {
            return Err("invalid guarded native recovery progress".into());
        }
        match progress.status {
            FileGeneratedSourceRecoveryStatus::Failed(error) => return Err(debug(error)),
            FileGeneratedSourceRecoveryStatus::Interrupted
            | FileGeneratedSourceRecoveryStatus::Cancelled => {
                return Err("guarded native recovery interrupted before live custody".into());
            }
            FileGeneratedSourceRecoveryStatus::Replaying
            | FileGeneratedSourceRecoveryStatus::Ready => {}
        }
        // Ready has its own checkpoint. A Stop accepted after the final event
        // still wins before staging cleanup, the sole Fence and live role return.
        deadline.check(time())?;
        pending |= control.recovery_pending_peer()?;
        if progress.status == FileGeneratedSourceRecoveryStatus::Ready {
            if pending {
                let target = recovery
                    .into_recovery_stop(control.recovery_operation()?)
                    .map_err(debug)?;
                let reason = match stopped {
                    Some(retained) => {
                        control.stop_recovery_with_anchor(target, deadline, time, retained)?
                    }
                    None => control.stop_recovery(target, deadline, time)?,
                };
                return Err(reason);
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
            return Err("guarded native recovery exceeded its single-event quantum".into());
        }
    }
}
