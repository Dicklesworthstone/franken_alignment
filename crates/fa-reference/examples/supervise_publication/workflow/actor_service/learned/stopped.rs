//! Offline terminal recovery through the original independently guarded cursor.
//! This command returns a native stop report, never live actor/reviewer custody.
use super::{Config, Loaded, ElapsedTick, Instant, USAGE, anchors, recipe, recovery, topology,
    Duration, JournalError, Error};
use recovery::{StopPhase, TerminalHistory};
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use std::{fs, io::Write, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const CHECKPOINT_OUTPUT: &str = "--retain-topology-checkpoint";
const CHECKPOINT_RECOVERY: &str = "--recover-terminal-checkpoint";

struct TerminalOutput { archive: AcknowledgedOutput, identity: PathBuf }

fn take_options(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>, bool), String> {
    let mut remaining = Vec::new();
    let mut output = None;
    let mut recover_checkpoint = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == CHECKPOINT_OUTPUT {
            if index < 5 || output.is_some() { return Err(USAGE.into()); }
            output = Some(super::fields::path(args.get(index + 1).ok_or(USAGE)?)?);
            index += 2;
        } else if args[index] == CHECKPOINT_RECOVERY {
            if index < 5 || std::mem::replace(&mut recover_checkpoint, true) {
                return Err(USAGE.into());
            }
            index += 1;
        } else {
            remaining.push(args[index].clone());
            index += 1;
        }
    }
    Ok((remaining, output, recover_checkpoint))
}

fn admit_output(config: &Config, loaded: &Loaded, path: Option<&Path>)
    -> Result<Option<TerminalOutput>, String>
{
    let Some(path) = path else { return Ok(None); };
    let history = topology::history(loaded)?
        .ok_or("terminal topology checkpoint retention requires an explicit topology profile")?;
    if history.checkpoint().is_none() {
        return Err("terminal topology checkpoint retention requires an exact v2 source checkpoint; manual source-work minima cannot establish complete custody".into());
    }
    let anchor = loaded.anchor_output.as_deref()
        .ok_or("terminal topology checkpoint retention requires --retain-history-anchor for the same acknowledged cut")?;
    let archive = AcknowledgedOutput::admit(config, path)?;
    if archive.path() == anchor {
        return Err("history anchor and topology checkpoint outputs must be distinct new files".into());
    }
    // Canonical identity is admitted before native completion. Export never
    // reopens the mutable authority directory or substitutes a newer journal.
    let identity = fs::canonicalize(&config.store).map_err(super::debug)?;
    Ok(Some(TerminalOutput { archive, identity }))
}

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
    let (args, output_path, recover_checkpoint) = take_options(args)?;
    let (args, topology_path) = topology::take_option(&args)?;
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
    let output = admit_output(&config, &loaded, output_path.as_deref())?;
    checkpoint()?;
    run_phase_retaining_with_elapsed(&config, &loaded, phase, operation, recover_checkpoint,
        output.as_ref(), clock, elapsed, out)
}

#[cfg(test)]
pub(super) fn run<F>(config: &Config, loaded: &Loaded, operation: u64,
    clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let started = Instant::now();
    run_phase_with_elapsed(config, loaded, StopPhase::Combined, operation, || Ok(clock()),
        &mut || started.elapsed(), out)
}

#[cfg(test)]
pub(super) fn run_phase_with_elapsed<F>(config: &Config, loaded: &Loaded,
    phase: StopPhase, operation: u64, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    run_phase_retaining_with_elapsed(config, loaded, phase, operation, false, None, clock, elapsed, out)
}

fn run_phase_retaining_with_elapsed<F>(config: &Config, loaded: &Loaded,
    phase: StopPhase, operation: u64, recover_checkpoint: bool,
    output: Option<&TerminalOutput>, clock: F,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> Result<ElapsedTick, JournalError> {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let history = recovery::terminal_selected_with_elapsed(config, loaded, phase, operation,
        recover_checkpoint, clock, elapsed)
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
    // Source work comes only from the independently selected exact v2 cut.
    // Native terminal validation proves the suffix contains no further source
    // work and pairs the unchanged source custody with this SAME stopped anchor.
    // Attempt this output even when independent history retention failed.
    let topology_retained = output.map(|output| {
        let saved = topology::history(loaded)?.and_then(|history| history.checkpoint())
            .ok_or("terminal topology checkpoint custody is missing")?;
        let checkpoint = match &history {
            TerminalHistory::Admission(history) => saved.after_admission_stop(
                &config.profile, &output.identity, history),
            TerminalHistory::Completed(history) => saved.after_stopped(
                &config.profile, &output.identity, history),
        }.map_err(super::debug)?;
        output.archive.retain(&checkpoint)
    }).transpose().map(|_| ()).map_err(|error: String| format!(
        "native stop acknowledged at revision {}, but terminal topology checkpoint retention failed: {error}; no effect retry was sent; keep previous paired custody and retry with --recover-terminal-checkpoint and new output paths",
        revision));
    // A failed export must not hide the native stop or its remaining liabilities.
    // Conversely a failed/pending report must not hide failed archive retention.
    let reported = match &history {
        TerminalHistory::Admission(history) => crate::recover_stop::report_admission(history, out),
        TerminalHistory::Completed(history) => crate::recover_stop::report(history.stopped(), out),
    };
    let failures: Vec<_> = [reported, retained, topology_retained].into_iter().filter_map(Result::err).collect();
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
}

fn unconfirmed(error: String) -> String {
    format!("learned native stop completion is unconfirmed: {error}; no effect was resent")
}
