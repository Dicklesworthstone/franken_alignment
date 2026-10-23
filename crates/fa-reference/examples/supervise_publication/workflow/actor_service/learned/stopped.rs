//! Offline terminal recovery through the original independently guarded cursor.
//! This command returns a native stop report, never live actor/reviewer custody.
use super::{Config, Loaded, ElapsedTick, Instant, USAGE, anchors, recipe, recovery, topology,
    Duration, JournalError, Error};
use recovery::{StopPhase, TerminalHistory};
use std::{io::Write, path::Path, time::{SystemTime, UNIX_EPOCH}};

pub(super) fn is_option(value: &str) -> bool {
    matches!(value, "--recover-stop" | "--stop-admission" | "--drain-stopped")
}

pub(super) fn command(args: &[String], options: anchors::Options) -> Result<(), String> {
    let started = Instant::now();
    let clock = || {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Incomplete)?;
        Ok(ElapsedTick(u64::try_from(elapsed.as_millis()).map_err(|_| Error::Overflow)?))
    };
    command_with(args, options, clock, &mut || started.elapsed(), &mut std::io::stdout().lock())
}

// One real command parser and original file loaders, with deterministic clock,
// elapsed and output seams. Admission never invokes the clock it receives.
pub(super) fn command_with<F>(args: &[String], options: anchors::Options, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    let (args, topology_path) = topology::take_option(args)?;
    if args.len() != 5 || args[0] != "create-learned-generated" {
        return Err(USAGE.into());
    }
    let phase = match args[3].as_str() {
        "--recover-stop" => StopPhase::Combined,
        "--stop-admission" => StopPhase::Admission,
        "--drain-stopped" => StopPhase::Drain,
        _ => return Err(USAGE.into()),
    };
    let text = &args[4];
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(USAGE.into()); }
    let operation = text.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if operation == 0 { return Err(USAGE.into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall { Err("learned stop recovery replay deadline reached".to_owned()) }
        else { Ok(()) }
    };
    checkpoint()?;
    // Recovery binds primary model and predictor history without loading native
    // helper models, opening a reviewer/stop socket or reading current evidence.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checkpoint()?;
    options.admit(&config, &mut loaded, true)?;
    checkpoint()?;
    topology::admit(&config, &mut loaded, topology_path.as_deref(), topology::Mode::Historical)?;
    checkpoint()?;
    run_phase_with_elapsed(&config, &loaded, phase, operation, clock, elapsed, out)
}

#[cfg(test)]
pub(super) fn run<F>(config: &Config, loaded: &Loaded, operation: u64,
    clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let started = Instant::now();
    run_phase_with_elapsed(config, loaded, StopPhase::Combined, operation, || Ok(clock()),
        &mut || started.elapsed(), out)
}

pub(super) fn run_phase_with_elapsed<F>(config: &Config, loaded: &Loaded,
    phase: StopPhase, operation: u64, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let history = recovery::terminal_selected_with_elapsed(config, loaded, phase, operation, clock, elapsed)
        .map_err(unconfirmed)?;
    let (anchor, revision) = match &history {
        TerminalHistory::Admission(history) => (history.history_anchor(), history.snapshot().revision),
        TerminalHistory::Completed(history) => (history.history_anchor(), history.stopped().snapshot().revision),
    };
    // Export exactly the acknowledged cut before rendering, even for admission
    // with unresolved liabilities. No fresh owner or newer canonical read exists.
    let retained = loaded.anchor_output.as_deref().map(|path| anchors::export_acknowledged(anchor, path))
        .transpose().map(|_| ()).map_err(|error| format!(
        "native stop acknowledged at revision {}, but history anchor retention failed: {error}; no effect retry was sent; keep the previous archive and select a new output path",
        revision));
    // A failed export must not hide the native stop or its remaining liabilities.
    // Conversely a failed/pending report must not hide failed archive retention.
    let reported = match &history {
        TerminalHistory::Admission(history) => crate::recover_stop::report_admission(history, out),
        TerminalHistory::Completed(history) => crate::recover_stop::report(history.stopped(), out),
    };
    match (reported, retained) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(report), Err(retention)) => Err(format!("{report}; {retention}")),
    }
}

fn unconfirmed(error: String) -> String {
    format!("learned native stop completion is unconfirmed: {error}; no effect was resent")
}
