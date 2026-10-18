//! Explicit receipt inspection or settlement on the SAME guarded replay.
//! Neither mode returns a live owner; a failed mode never falls back to the other.
pub(in super::super) mod service;
use super::{receipt_replay, selected::Cursor};
use super::super::{Config, Loaded, Duration, Instant, RunResult, anchors, emit, finish, recipe, topology};
use fa_reference::action::consequence::oversight::actor_wire::WireResponse;
use fa_reference::action::ElapsedTick;
use std::{io::Write, path::Path};

const FLAG: &str = "--receipts-only";
const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --receipts-only [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT]";
const RECONCILE_FLAG: &str = "--reconcile-receipts";
const RECONCILE_USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --reconcile-receipts [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT]";
const ACKNOWLEDGED_EXPIRED: &str = "learned receipt reconciliation acknowledged; deadline expired before response; inspect with --receipts-only; no effect retry was sent";

#[derive(Clone, Copy)]
enum Mode { ReadOnly, Reconcile }
impl Mode {
    fn flag(self) -> &'static str {
        match self { Self::ReadOnly => FLAG, Self::Reconcile => RECONCILE_FLAG }
    }
    fn usage(self) -> &'static str {
        match self { Self::ReadOnly => USAGE, Self::Reconcile => RECONCILE_USAGE }
    }
}

const EXPIRED: &str = "learned receipt recovery replay deadline reached";

pub(in super::super) fn command(args: &[String], credibility: Option<&Path>,
    output: &mut impl Write) -> Result<(), String>
{
    if args.iter().any(|arg| arg == "--serve-receipts") {
        return service::command(args, credibility);
    }
    let started = Instant::now();
    if args.iter().any(|arg| arg == RECONCILE_FLAG) {
        command_reconciled_with_elapsed(args, credibility, output,
            crate::workflow::clock, &mut || started.elapsed())
    } else {
        command_with_elapsed(args, credibility, output, &mut || started.elapsed())
    }
}

// Production owns one Instant, including intake time. The deterministic supplier
// lets tests cancel every replay boundary without changing a numerical clock.
pub(in super::super) fn command_with_elapsed(args: &[String], credibility: Option<&Path>,
    output: &mut impl Write, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
{
    let (config, loaded, after) = load(args, credibility, Mode::ReadOnly, elapsed)?;
    let response = read(&config, &loaded, after, elapsed)?;
    emit(RunResult { response, failure: None, cleanup_pending: 0 }, output)
}

/// Explicit mutating mode. The supplied trusted clock is consumed only by the
/// final settlement after full replay; parsing and historical work cannot use it.
/// Kept separate from the read-only entry point so neither can silently upgrade.
pub(in super::super) fn command_reconciled_with_elapsed<F>(args: &[String],
    credibility: Option<&Path>, output: &mut impl Write, clock: F,
    elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let (config, loaded, after) = load(args, credibility, Mode::Reconcile, elapsed)?;
    let response = reconcile(&config, &loaded, after, clock, elapsed)?;
    // Same encoder/exit law. A failed stdout write cannot erase the acknowledged
    // transaction or trigger another recovery, helper session or effect retry.
    emit(RunResult { response, failure: None, cleanup_pending: 0 }, output)
}

fn load(args: &[String], credibility: Option<&Path>, mode: Mode,
    elapsed: &mut impl FnMut() -> Duration) -> Result<(Config, Loaded, Option<u64>), String>
{
    let (args, topology_path) = topology::take_option(args)?;
    let usage = mode.usage(); let flag = mode.flag();
    if credibility.is_some() || args.iter().filter(|arg|
        matches!(arg.as_str(), FLAG | RECONCILE_FLAG)).count() != 1
        || !args.iter().any(|arg| arg == flag)
        || args.iter().any(|arg| matches!(arg.as_str(), "--retain-history-anchor"
            | "--recover-stop" | "--audit-review" | "--continue-generation"
            | "--after-message" | "--credibility-activation")) {
        return Err(usage.into());
    }
    // Consume path-valued options FIRST so a missing path cannot be repaired by
    // stripping our boolean flag out of another option's value position.
    let (mut args, anchors) = anchors::take_options(&args)?;
    let at = args.iter().position(|arg| arg == flag).ok_or(usage)?;
    if at < 4 { return Err(usage.into()); }
    args.remove(at);
    let (args, after) = finish::take_option(&args)?;
    if args.len() != 4 || args[0] != "create-learned-generated" || args[3] != "--resume"
        || args[1..3].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(usage.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    check_elapsed(&config, elapsed)?;
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    anchors.admit(&config, &mut loaded, true)?;
    topology::admit(&config, &mut loaded, topology_path.as_deref(), topology::Mode::Historical)?;
    Ok((config, loaded, after))
}

pub(in super::super) fn read(config: &Config, loaded: &Loaded, after: Option<u64>,
    elapsed: &mut impl FnMut() -> Duration) -> Result<WireResponse, String>
{
    if loaded.anchor_output.is_some() {
        return Err("read-only receipts do not acknowledge or export a successor anchor".into());
    }
    if loaded.request == 0 { return Err("receipt request must be nonzero".into()); }
    if let Some(after) = after { finish::check_ids(loaded.request, after)?; }
    let outcome = receipt_replay(config, loaded, elapsed, |run|
        run.finish_receipt(loaded.request, after))?;
    // The final canonical re-read can take time. Late expiry withholds output;
    // it cannot leave a fence, role or staged-file cleanup behind this path.
    check_elapsed(config, elapsed)?;
    Ok(WireResponse { request: Some(loaded.request), result: Ok(outcome) })
}

/// Settlement consumes the same original guarded cursor and one-event timer.
/// The fixed transaction never runs a new generation or creates a replacement
/// actor request. A successful result says nothing about other unresolved keys.
pub(in super::super) fn reconcile<F>(config: &Config, loaded: &Loaded, after: Option<u64>,
    clock: F, elapsed: &mut impl FnMut() -> Duration) -> Result<WireResponse, String>
where F: FnOnce() -> ElapsedTick {
    if loaded.anchor_output.is_some() {
        return Err("receipt-only reconciliation does not export a successor anchor".into());
    }
    if loaded.request == 0 { return Err("receipt request must be nonzero".into()); }
    if let Some(after) = after { finish::check_ids(loaded.request, after)?; }
    let outcome = receipt_replay(config, loaded, elapsed, |run|
        run.finish_reconciled_receipt(loaded.request, after, clock))?;
    // The single replacement has ACKNOWLEDGED. Expiry now withholds stdout but
    // cannot truthfully claim no mutation; it never retries or attempts an undo.
    if elapsed() >= Duration::from_millis(config.timing.runtime_ms) {
        return Err(ACKNOWLEDGED_EXPIRED.into());
    }
    Ok(WireResponse { request: Some(loaded.request), result: Ok(outcome) })
}

fn check_elapsed(config: &Config, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= Duration::from_millis(config.timing.runtime_ms) { Err(EXPIRED.into()) }
    else { Ok(()) }
}
