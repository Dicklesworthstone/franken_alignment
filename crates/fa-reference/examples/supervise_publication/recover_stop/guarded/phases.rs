//! Explicit guarded Stop/Fence and later stopped drain through the original cursors.
use super::{Config, Profile, JournalError, FileGuardedRecoveryStatus,
    debug, prepare_recovery, report, unconfirmed};
use super::super::admission::{Phase, admission_report, combine_report};
use fa_reference::action::ElapsedTick;
use std::{io::Write, path::Path, time::Duration};

const USAGE: &str = "usage: supervise_publication stop-admission|drain-stopped CONFIG OPERATION --guarded RECOVERY_PROFILE (retention is selected in that profile)";
const DEADLINE: &str = "guarded terminal replay/acquisition deadline reached; no additional native completion was acknowledged";

// Called before shared or ordinary options can be removed. Only these five
// arguments select this route; an output-only flag cannot replace input custody.
pub(crate) fn command_with<F>(args: &[String], clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    if args.len() != 5 || args[3] != "--guarded"
        || args[4].is_empty() || args[4].starts_with("--") {
        return Err(USAGE.into());
    }
    let phase = match args[0].as_str() {
        "stop-admission" => Phase::Admission,
        "drain-stopped" => Phase::Drain,
        _ => return Err(USAGE.into()),
    };
    let operation = args[2].parse::<u64>().map_err(debug)?;
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let profile = Profile::read(Path::new(&args[4]))?;
    run_with_elapsed(&config, &profile, phase, operation, clock, elapsed, out)
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall { Err(DEADLINE.into()) } else { Ok(()) }
}

fn run_with_elapsed<F>(config: &Config, profile: &Profile, phase: Phase, operation: u64,
    clock: F, elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    // SAME original custody, complete guard inventory, topology, floor and
    // terminal reserve as combined guarded recovery; no inferred requirements.
    let (mut recovery, custody) = prepare_recovery(config, profile,
        &mut || checkpoint(wall, elapsed))?;
    loop {
        checkpoint(wall, elapsed)?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(progress.replayed_events, 1).map_err(unconfirmed)?;
    }
    let request = recovery.stop_request(operation).map_err(unconfirmed)?;
    match phase {
        Phase::Admission => {
            let history = recovery.finish_admission_stop(request).map_err(unconfirmed)?;
            let retained = retain(&custody, history.history_anchor(), history.snapshot().revision);
            let reported = admission_report(&history, out);
            combine_report(reported, retained)
        }
        Phase::Drain => {
            let mut expired = false;
            let result = recovery.finish_stopped_drain(request, || {
                if checkpoint(wall, elapsed).is_err() {
                    expired = true;
                    return Err(JournalError::Unavailable);
                }
                let tick = clock()?;
                if checkpoint(wall, elapsed).is_err() {
                    expired = true;
                    return Err(JournalError::Unavailable);
                }
                Ok(tick)
            });
            if expired { return Err(DEADLINE.into()); }
            let history = result.map_err(unconfirmed)?;
            let retained = retain(&custody, history.history_anchor(), history.stopped().snapshot().revision);
            let reported = report(history.stopped(), out);
            combine_report(reported, retained)
        }
    }
}

fn retain(custody: &super::Custody,
    anchor: &fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor,
    revision: u64) -> Result<(), String>
{
    custody.retain(anchor).map_err(|error| format!(
        "native terminal operation acknowledged at revision {revision}, but history anchor retention failed: {error}; keep the previous archive and select a new output path; no effect was resent"))
}

#[cfg(test)]
#[path = "phases/tests.rs"] mod tests;
