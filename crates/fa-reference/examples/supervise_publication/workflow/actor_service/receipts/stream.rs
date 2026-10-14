//! Anchored stream receipt recovery with no admitting or numerical fallback.
mod settings;
#[cfg(test)]
mod tests;
use settings::Settings;
use super::{ActorWire, BoundSocket, Config, Duration, FileOversight, Path, PeerSession,
    Profile, ServiceReport, debug, serve_session};
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements, recovery::FileGuardedRecoveryStatus,
};

const USAGE: &str = "serve-open CONFIG ACTOR_PROFILE --stream-recovery RECOVERY_PROFILE --receipts-only";
const EXPIRED: &str = "stream receipt recovery deadline reached; no actor request was admitted";

pub(super) fn command_with(args: &[String], credibility: Option<&Path>,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    if credibility.is_some() || args.len() != 6 || args[0] != "serve-open"
        || args[3] != "--stream-recovery" || args[5] != "--receipts-only"
        || [1, 2, 4].into_iter().any(|i| args[i].is_empty() || args[i].starts_with("--"))
    { return Err(USAGE.into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let settings = Settings::read(Path::new(&args[4]))?;
    let report = serve(&config, &actor, &settings, elapsed, wait)?;
    eprintln!("Stream receipt service window ended: frames={}, candidates={}, revision={}; no message, finish or reconciliation was admitted",
        report.frames, report.candidates, report.revision);
    Ok(report)
}

fn serve(config: &Config, actor: &Profile, settings: &Settings,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
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
    // Bind only after verified replay, but before the recovery fence. A socket
    // collision drops the original cursor without cleanup or a canonical write.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    checkpoint(wall, elapsed)?;
    let (host, roles) = recovery.finish().map_err(debug)?;
    drop(roles); // No separately held approval role enters actor receipt service.
    checkpoint(wall, elapsed)?; // A late failure cannot undo the acknowledged fence.
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
