//! Anchored stream receipt recovery with no admitting or numerical fallback.
mod settings;
#[cfg(test)]
mod tests;
use settings::Settings;
use super::{ActorWire, BoundSocket, Config, Duration, FileOversight, Path, PeerSession,
    Profile, ServiceReport, debug, serve_session};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements, recovery::{FileGuardedRecoveryStatus, ReconciledGuardedOwner},
};

const USAGE: &str = "serve-open CONFIG ACTOR_PROFILE --stream-recovery RECOVERY_PROFILE --receipts-only | --reconcile-receipts";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { ReceiptsOnly, ReconcileReceipts }
const EXPIRED: &str = "stream receipt recovery deadline reached; no actor request was admitted";

pub(super) fn command_with(args: &[String], credibility: Option<&Path>,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    let mode = match args.get(5).map(String::as_str) {
        Some("--receipts-only") => Mode::ReceiptsOnly,
        Some("--reconcile-receipts") => Mode::ReconcileReceipts,
        _ => return Err(USAGE.into()),
    };
    if credibility.is_some() || args.len() != 6 || args[0] != "serve-open"
        || args[3] != "--stream-recovery"
        || [1, 2, 4].into_iter().any(|i| args[i].is_empty() || args[i].starts_with("--"))
    { return Err(USAGE.into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let settings = Settings::read(Path::new(&args[4]))?;
    let report = match mode {
        Mode::ReceiptsOnly => serve(&config, &actor, &settings, elapsed, wait)?,
        Mode::ReconcileReceipts => serve_reconciled(&config, &actor, &settings,
            crate::workflow::clock, elapsed, wait)?,
    };
    eprintln!("Stream receipt service window ended: frames={}, candidates={}, revision={}, reconciled={}; no message or finish was admitted",
        report.frames, report.candidates, report.revision, mode == Mode::ReconcileReceipts);
    Ok(report)
}

fn serve(config: &Config, actor: &Profile, settings: &Settings,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    serve_mode(config, actor, settings, Mode::ReceiptsOnly, || ElapsedTick(0), elapsed, wait)
}

fn serve_reconciled<F>(config: &Config, actor: &Profile, settings: &Settings, clock: F,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
where F: FnOnce() -> ElapsedTick {
    serve_mode(config, actor, settings, Mode::ReconcileReceipts, clock, elapsed, wait)
}

fn serve_mode<F>(config: &Config, actor: &Profile, settings: &Settings, mode: Mode, clock: F,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
where F: FnOnce() -> ElapsedTick
{
    let wall = Duration::from_millis(actor.runtime_ms.min(config.timing.runtime_ms));
    checkpoint(wall, elapsed)?;
    actor.check_host(config)?;
    let anchor = settings.anchor(config)?;
    checkpoint(wall, elapsed)?;
    // Every None requires absence. This explicitly selected baseline is not a
    // route around numerical, predictor, identity, evaluator or mediation checks.
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(settings.stream), decoder: None, decoder_stop: None,
            source: settings.registered_source.then_some(config.source_policy),
            identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: settings.minimum,
    };
    let mut recovery = FileOversight::begin_open_guarded_anchored(&config.store,
        config.profile.clone(), &expected, &anchor)
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(debug)?;
    loop {
        checkpoint(wall, elapsed)?; // Includes Ready: expiry cannot skip to Fence.
        let progress = recovery.progress();
        match progress.status {
            FileGuardedRecoveryStatus::Replaying => {
                recovery.advance(progress.replayed_events, 1).map_err(debug)?;
            }
            FileGuardedRecoveryStatus::Ready => break,
            _ => return Err("original stream recovery is unavailable".into()),
        }
    }
    // Require the independently selected request before any Fence/Time/Sweep.
    // Missing keys cannot mutate recovery state merely because an actor asked
    // for a receipt. Exact proposal equality is still checked by the wire port.
    let recovery = recovery.require_existing_request(actor.request).map_err(debug)?;
    // Bind only after verified replay and request existence, but before recovery
    // mutation. A socket collision drops the cursor without cleanup or writes.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    checkpoint(wall, elapsed)?;
    let host = match mode {
        Mode::ReceiptsOnly => {
            let (host, roles) = recovery.finish().map_err(debug)?;
            drop(roles);
            host
        }
        Mode::ReconcileReceipts => {
            let ReconciledGuardedOwner { owner, roles, outcomes: _ } =
                recovery.finish_reconciled(clock).map_err(debug)?;
            drop(roles);
            owner
        }
    };
    checkpoint(wall, elapsed)?; // Late expiry cannot undo an acknowledged cut.
    let (port, supervisor) = host.into_stream_actor_gateway().map_err(debug)?;
    let port = port.recovery_only().for_request(actor.request).map_err(debug)?;
    let mut session = PeerSession::new(actor.actor, ActorWire::new(port), actor.channels(), actor.connections).map_err(debug)?;
    let (frames, candidates) = serve_session(&mut session, actor, socket, wall, elapsed, wait)?;
    let revision = supervisor.host().map_err(debug)?.revision();
    Ok(ServiceReport { frames, candidates, revision })
}
fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall { Err(EXPIRED.into()) } else { Ok(()) }
}
