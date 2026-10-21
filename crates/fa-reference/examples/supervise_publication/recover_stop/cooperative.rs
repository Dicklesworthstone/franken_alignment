//! The ordinary offline command consumes one stop-only native recovery cursor.
//! Guarded recovery remains separately selected, with its independent custody.
use super::{Config, FileOversight, FileStoppedRecovery, debug, report};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecoveryStatus;
use std::{io::Write, time::Duration};

// Deterministic elapsed seam for boundary tests. The actual command uses one
// monotonic Instant; it never samples the logical clock to budget historical work.
pub(super) fn run_with_elapsed<F>(config: &Config, operation: u64, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    if elapsed() >= wall {
        return Err("stop recovery replay deadline reached before open; no recovery completion was acknowledged".into());
    }
    // Unlike read_publication followed by recover_stopped, no unlocked first
    // replay or independently reread predecessor exists. No live role is opened.
    let mut recovery = FileOversight::begin_stopped_recovery(&config.store, config.profile.clone())
        .map_err(unconfirmed)?;
    let recovered: FileStoppedRecovery = loop {
        let progress = recovery.progress();
        // Ready is a checkpoint too: expiry after the final original event
        // cannot trigger a stop/fence, staging cleanup or terminal clock call.
        if elapsed() >= wall {
            recovery.cancel();
            return Err(format!("stop recovery replay deadline reached after {} of {} original events; no recovery completion was acknowledged; no effect was resent",
                progress.replayed_events, progress.total_events));
        }
        match progress.status {
            FileStopRecoveryStatus::Ready => {
                let request = recovery.stop_request(operation).map_err(unconfirmed)?;
                let mut timed_out = false;
                let result = recovery.try_finish_stopped_with_anchor(request, || {
                    if elapsed() >= wall { timed_out = true; return Err(JournalError::Unavailable); }
                    let tick = clock();
                    if elapsed() >= wall { timed_out = true; return Err(JournalError::Unavailable); }
                    Ok(tick)
                });
                if timed_out {
                    return Err("stop recovery acquisition deadline reached; no native completion was acknowledged; no effect was resent".into());
                }
                break result.map_err(unconfirmed)?.into_parts().0;
            }
            FileStopRecoveryStatus::Replaying => {
                recovery.advance(progress.replayed_events, 1).map_err(unconfirmed)?;
            }
            _ => return Err("native stop replay is unavailable; no recovery completion was acknowledged".into()),
        }
    };
    // Keep the original receipt/report law, including output-loss uncertainty,
    // pending liabilities, and executed charges. No source acquisition is needed.
    report(&recovered, out)
}

fn unconfirmed(error: JournalError) -> String {
    format!("native recovery completion is unconfirmed: {}; no effect was resent", debug(error))
}
