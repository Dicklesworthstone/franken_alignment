//! Explicit clock-free stop and later drain; never a fallback from guarded recovery.
mod custody;
use custody::AnchorOutput;
use super::{Config, FileOversight, debug, ids, report};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::{
    FileAdmissionStoppedHistory, FileStopRecoveryStatus};
use fa_reference::Error;
use std::{io::Write, path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

const USAGE: &str = "usage: supervise_publication stop-admission|drain-stopped CONFIG OPERATION [--retain-history-anchor NEW_FILE]";
const DEADLINE: &str = "terminal replay/acquisition deadline reached; no additional native completion was acknowledged";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase { Admission, Drain }

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(args, || {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Incomplete)?;
        Ok(ElapsedTick(u64::try_from(elapsed.as_millis()).map_err(|_| Error::Overflow)?))
    }, &mut || started.elapsed(), out)
}

fn command_with<F>(args: &[String], clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    // Validate the whole option set before CONFIG. No ignored credibility,
    // guarded, witness or role flag can silently select this ordinary profile.
    if args.len() != 3 && args.len() != 5 { return Err(USAGE.into()); }
    if args.len() == 5 && (args[3] != "--retain-history-anchor"
        || args[4].is_empty() || args[4].starts_with("--")) { return Err(USAGE.into()); }
    let phase = match args[0].as_str() {
        "stop-admission" => Phase::Admission, "drain-stopped" => Phase::Drain,
        _ => return Err(USAGE.into()),
    };
    let operation = args[2].parse::<u64>().map_err(debug)?;
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let retention = if args.len() == 5 {
        Some(AnchorOutput::admit(&config, Path::new(&args[4]))?)
    } else { None };
    run_with_retention(&config, phase, operation, clock, elapsed, out, retention.as_ref())
}

fn run_with_elapsed<F>(config: &Config, phase: Phase, operation: u64, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    run_with_retention(config, phase, operation, clock, elapsed, out, None)
}

fn run_with_retention<F>(config: &Config, phase: Phase, operation: u64, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write,
    retention: Option<&AnchorOutput>) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    if elapsed() >= wall { return Err(DEADLINE.into()); }
    let mut recovery = FileOversight::begin_stopped_recovery(&config.store, config.profile.clone())
        .map_err(unconfirmed)?;
    loop {
        let progress = recovery.progress();
        if elapsed() >= wall {
            recovery.cancel();
            return Err(DEADLINE.into());
        }
        match progress.status {
            FileStopRecoveryStatus::Replaying => {
                recovery.advance(progress.replayed_events, 1).map_err(unconfirmed)?;
            }
            FileStopRecoveryStatus::Ready => break,
            _ => return Err("native terminal replay is unavailable".into()),
        }
    }
    let request = recovery.stop_request(operation).map_err(unconfirmed)?;
    match phase {
        Phase::Admission => {
            let stopped = recovery.finish_admission_stop(request).map_err(unconfirmed)?;
            let retained = retention.map(|output| output.retain(stopped.history_anchor()))
                .transpose().map(|_| ());
            let reported = admission_report(&stopped, out);
            combine_report(reported, retained)
        }
        Phase::Drain => {
            let mut timed_out = false;
            let result = recovery.finish_stopped_drain(request, || {
                if elapsed() >= wall { timed_out = true; return Err(JournalError::Unavailable); }
                let tick = clock()?;
                if elapsed() >= wall { timed_out = true; return Err(JournalError::Unavailable); }
                Ok(tick)
            });
            if timed_out { return Err(DEADLINE.into()); }
            let recovered = result.map_err(unconfirmed)?;
            let retained = retention.map(|output| output.retain(recovered.history_anchor()))
                .transpose().map(|_| ());
            let reported = report(recovered.stopped(), out);
            combine_report(reported, retained)
        }
    }
}

fn combine_report(reported: Result<(), String>, retained: Result<(), String>) -> Result<(), String> {
    match (reported, retained) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(report), Err(retention)) => Err(format!("{report}; {retention}")),
    }
}

fn unconfirmed(error: JournalError) -> String {
    format!("native terminal completion is unconfirmed: {}; no effect was resent", debug(error))
}

fn admission_report(stopped: &FileAdmissionStoppedHistory, out: &mut impl Write) -> Result<(), String> {
    let state = stopped.snapshot(); let progress = stopped.progress();
    let document = format!(concat!(
        "{{\"schema_version\":\"fa.admission-stop/1\",\"status\":\"admission_stopped\",",
        "\"stop_acknowledged\":true,\"drain_attempted\":false,\"drained\":{},",
        "\"operation\":\"{}\",\"revision\":\"{}\",\"appended_events\":{},",
        "\"endpoint_fenced\":{},\"dispatcher_epoch\":\"{}\",",
        "\"executions\":\"{}\",\"reserved\":\"{}\",\"charged\":\"{}\",",
        "\"unresolved\":[{}],\"irrecoverable\":[{}]}}\n"),
        progress.drained(), progress.receipt.request().operation, state.revision, stopped.appended_events(),
        progress.endpoint_fenced, progress.dispatcher_epoch, state.executions,
        progress.reserved_units, progress.charged_units, ids(&progress.unresolved), ids(&progress.irrecoverable));
    out.write_all(document.as_bytes()).and_then(|_| out.flush()).map_err(|error| format!(
        "native admission stop acknowledged at revision {}, but output failed: {error:?}; do not resend an effect", state.revision))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod custody_tests;
