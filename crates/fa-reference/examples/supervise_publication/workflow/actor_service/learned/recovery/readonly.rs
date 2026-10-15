//! Explicit read-only receipt retrieval; never fall back to owner recovery.
use super::{receipt_replay, selected::Cursor};
use super::super::{Config, Loaded, Duration, Instant, RunResult, anchors, emit, finish, recipe};
use fa_reference::action::consequence::oversight::actor_wire::WireResponse;
use std::{io::Write, path::Path};

const FLAG: &str = "--receipts-only";
const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --receipts-only [--finish-after-message PREVIOUS_REQUEST] [--history-anchor INPUT]";
const EXPIRED: &str = "learned receipt recovery replay deadline reached";

pub(in super::super) fn command(args: &[String], credibility: Option<&Path>,
    output: &mut impl Write) -> Result<(), String>
{
    let started = Instant::now();
    command_with_elapsed(args, credibility, output, &mut || started.elapsed())
}

// Production owns one Instant, including intake time. The deterministic supplier
// lets tests cancel every replay boundary without changing a numerical clock.
pub(in super::super) fn command_with_elapsed(args: &[String], credibility: Option<&Path>,
    output: &mut impl Write, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
{
    if credibility.is_some() || args.iter().filter(|arg| arg.as_str() == FLAG).count() != 1
        || args.iter().any(|arg| matches!(arg.as_str(), "--retain-history-anchor"
            | "--recover-stop" | "--audit-review" | "--continue-generation"
            | "--after-message" | "--credibility-activation")) {
        return Err(USAGE.into());
    }
    // Consume path-valued options FIRST so a missing path cannot be repaired by
    // stripping our boolean flag out of another option's value position.
    let (mut args, anchors) = anchors::take_options(args)?;
    let at = args.iter().position(|arg| arg == FLAG).ok_or(USAGE)?;
    if at < 4 { return Err(USAGE.into()); }
    args.remove(at);
    let (args, after) = finish::take_option(&args)?;
    if args.len() != 4 || args[0] != "create-learned-generated" || args[3] != "--resume"
        || args[1..3].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    check_elapsed(&config, elapsed)?;
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    anchors.admit(&config, &mut loaded, true)?;
    let response = read(&config, &loaded, after, elapsed)?;
    // Same response encoding and exit semantics: denial/unknown is not successful
    // publication. There is no live owner or child process to drain or retry.
    emit(RunResult { response, failure: None, cleanup_pending: 0 }, output)
}

pub(in super::super) fn read(config: &Config, loaded: &Loaded, after: Option<u64>,
    elapsed: &mut impl FnMut() -> Duration) -> Result<WireResponse, String>
{
    if loaded.anchor_output.is_some() {
        return Err("read-only receipts do not acknowledge or export a successor anchor".into());
    }
    if loaded.request == 0 { return Err("receipt request must be nonzero".into()); }
    if let Some(after) = after { finish::check_ids(loaded.request, after)?; }
    let outcome = receipt_replay(config, loaded, elapsed, |run| match run {
        Cursor::Plain(run) => run.finish_receipt(loaded.request, after),
        Cursor::Raw(run) => run.finish_receipt(loaded.request, after),
        Cursor::Owned(run) => run.finish_receipt(loaded.request, after),
    })?;
    // The final canonical re-read can take time. Late expiry withholds output;
    // it cannot leave a fence, role or staged-file cleanup behind this path.
    check_elapsed(config, elapsed)?;
    Ok(WireResponse { request: Some(loaded.request), result: Ok(outcome) })
}

fn check_elapsed(config: &Config, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= Duration::from_millis(config.timing.runtime_ms) { Err(EXPIRED.into()) }
    else { Ok(()) }
}
