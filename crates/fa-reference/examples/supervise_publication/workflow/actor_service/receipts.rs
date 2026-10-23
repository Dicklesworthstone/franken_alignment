//! Explicit non-admitting recovery endpoint on the ORIGINAL live-service profile.
//! No fresh logical clock, source acquisition, helper, reviewer socket or drain.
use super::{ActorWire, BoundSocket, Config, Duration, FileOversight, Instant,
    PeerRefusal, PeerSession, Profile, PublicationProfile, debug, drive_budget,
    io, pause, prepare_host};
use std::path::Path;
use fa_reference::action::consequence::oversight::actor_wire::ActorRequestPort;
#[path = "receipts/stream.rs"] mod stream;

const USAGE: &str = "serve-open CONFIG ACTOR_PROFILE --receipts-only | serve-open-checked CONFIG ACTOR_PROFILE WITNESS_PROFILE --receipts-only";
const EXPIRED: &str = "receipt-only startup deadline reached; no actor request was admitted";

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    // The outer router selects this flag before the learned command. Delegate
    // that exact verb to its independently pinned numerical receipt parser;
    // never interpret a learned recipe as a baseline service configuration.
    if args.first().is_some_and(|arg| arg == "create-learned-generated") {
        return super::learned::command(args, credibility);
    }
    let start = Instant::now();
    command_with(args, credibility, &mut || start.elapsed(), &mut pause).map(|_| ())
}
fn command_with(args: &[String], credibility: Option<&Path>,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    if args.iter().any(|arg| arg == "--stream-recovery") {
        return stream::command_with(args, credibility, elapsed, wait);
    }
    // Own the complete flag grammar BEFORE config reads or shared flag removal.
    // Creation, live qualification, recipes, reviewer, sequence and fleet flags
    // must not silently select this less capable endpoint.
    let checked = match args.first().map(String::as_str) {
        Some("serve-open") if args.len() == 4 => false,
        Some("serve-open-checked") if args.len() == 5 => true,
        _ => return Err(USAGE.into()),
    };
    if credibility.is_some() || args.last().map(String::as_str) != Some("--receipts-only")
        || args[1..args.len() - 1].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let publication = if checked { Some(PublicationProfile::read(Path::new(&args[3]))?) } else { None };
    let report = serve(&config, &actor, publication.as_ref(), elapsed, wait)?;
    eprintln!("Receipt-only service window ended: frames={}, candidates={}, revision={}; no new admission or reconciliation was attempted",
        report.frames, report.candidates, report.revision);
    Ok(report)
}

/// Transport work only. Completion says that the bounded service window ended,
/// not that a request executed, was absent, or had its liability reconciled.
#[derive(Debug)]
struct ServiceReport { frames: usize, candidates: u64, revision: u64 }
fn serve(config: &Config, actor: &Profile, publication: Option<&PublicationProfile>,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    let wall = Duration::from_millis(actor.runtime_ms.min(config.timing.runtime_ms));
    if elapsed() >= wall { return Err(EXPIRED.into()); }
    actor.check_host(config)?;
    if elapsed() >= wall { return Err(EXPIRED.into()); }
    // Existing socket ownership/cleanup rules apply. No conflicting path is
    // removed, and no bytes are processed before original recovery finishes.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    if elapsed() >= wall { return Err(EXPIRED.into()); }
    let (host, reviewer) = prepare_host(config, publication, true)?;
    drop(reviewer); // no approval role is passed to the recovery endpoint
    if elapsed() >= wall { return Err(EXPIRED.into()); }
    serve_owner(host, actor, socket, wall, elapsed, wait)
}
fn serve_owner(host: FileOversight, actor: &Profile, socket: BoundSocket, wall: Duration,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<ServiceReport, String>
{
    let (port, supervisor) = host.into_actor_gateway();
    let port = port.recovery_only().for_request(actor.request).map_err(debug)?;
    let mut session = PeerSession::new(actor.actor, ActorWire::new(port), actor.channels(), actor.connections).map_err(debug)?;
    let (frames, candidates) = serve_session(&mut session, actor, socket, wall, elapsed, wait)?;
    // The supervisor stays alive throughout service; dropping it now releases
    // original exclusive custody. A weak actor port cannot retain that lock.
    let revision = supervisor.host().map_err(debug)?.revision();
    Ok(ServiceReport { frames, candidates, revision })
}

// One original bounded peer loop for raw, stream and sealed learned receipts.
// Only restricted receipt-service constructors call this private family helper.
pub(super) fn serve_session<P: ActorRequestPort>(session: &mut PeerSession<P>, actor: &Profile,
    socket: BoundSocket, wall: Duration, elapsed: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(u64)) -> Result<(usize, u64), String>
{
    let mut candidates = 0_u64;
    let mut frames = 0_usize;
    loop {
        if elapsed() >= wall { break; }
        if session.status().active.is_none() {
            match socket.listener.accept() {
                Ok((stream, _)) => {
                    if candidates == actor.candidates { return Err("receipt-only candidate quota exhausted".into()); }
                    candidates += 1;
                    match session.attach(stream) {
                        Ok(_) => {},
                        Err(PeerRefusal::CredentialsRejected) if candidates < actor.candidates => {
                            eprintln!("Rejected receipt-only actor credentials; candidate={candidates}");
                        }
                        Err(error) => return Err(debug(error)),
                    }
                }
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {},
                Err(error) => return Err(debug(error)),
            }
        }
        // Both successful and rejected admission consume this same invocation
        // window. Never create a new deadline or reset the original peer session.
        if elapsed() >= wall { break; }
        if session.status().active.is_some() {
            let report = session.drive(drive_budget()).map_err(debug)?;
            frames = frames.checked_add(report.progress.frames).ok_or("receipt frame counter exhausted")?;
            if report.status.closed() { session.disconnect(); }
        }
        if elapsed() >= wall { break; }
        wait(actor.poll_ms);
    }
    session.revoke();
    Ok((frames, candidates))
}

#[cfg(test)]
#[path = "receipts/tests.rs"] mod tests;
