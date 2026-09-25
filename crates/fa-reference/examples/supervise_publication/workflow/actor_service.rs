//! Bounded live actor intake driving the SAME helper/human/publication workflow.
//! Single or explicitly scheduled requests; no concurrent congress or new authority.
mod profile;
mod client;
mod qualification;
mod series;
mod multi;
mod generated;
use profile::Profile;
use super::{ActionState, ActorWire, BoundSocket, Config, Deadline, Duration, ElapsedTick,
    ExecuteServices, FileOversight, FileRequestDisposition, FileSupervisedDriver, Instant,
    PeerProfile, PublicationProfile, RecoveryReserve, CLOCK_DOMAIN, cleanup, debug,
    execute_serviced, io, pause, plus, resume_existing, stop};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::oversight::actor_peer::{PeerRefusal, PeerSession};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use std::path::Path;

type Port = FileActorPort<FileOversight>;
const USAGE: &str = "serve-create|serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE; serve-create-checked|serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE; actor-submit ACTOR_PROFILE SUBMIT_JSON [SUBMIT_JSON ...]; serve-open and serve-open-checked also accept --credibility-activation EVIDENCE_FILE; all serve modes accept a final --requests 7,8,9 option OR --peers ACTOR_PROFILE [ACTOR_PROFILE ...]";

pub(crate) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    if args.first().is_some_and(|mode| mode == "create-generated") {
        return generated::command(args, credibility);
    }
    if args.iter().any(|arg| arg == "--peers") { return multi::command(args, credibility); }
    let (args, requests) = series::take_option(args)?;
    let mode = args.first().map(String::as_str).ok_or(USAGE)?;
    if mode == "actor-submit" {
        if args.len() < 3 || args.len() > client::MAX_SEQUENCE_DOCUMENTS + 2
            || credibility.is_some() { return Err(USAGE.into()); }
        let profile = Profile::read(Path::new(&args[1]))?;
        if args.len() == 3 {
            return client::submit(&profile, Path::new(&args[2]), &mut std::io::stdout().lock());
        }
        let documents: Vec<_> = args[2..].iter().map(|path| Path::new(path.as_str())).collect();
        return client::submit_sequence(&profile, &documents, &mut std::io::stdout().lock());
    }
    let (existing, checked) = match mode {
        "serve-create" => (false, false), "serve-open" => (true, false),
        "serve-create-checked" => (false, true), "serve-open-checked" => (true, true),
        _ => return Err(USAGE.into()),
    };
    if args.len() != if checked { 5 } else { 4 } { return Err(USAGE.into()); }
    if !existing && credibility.is_some() {
        return Err("live credibility activation requires an existing journal".into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    let publication = if checked { Some(PublicationProfile::read(Path::new(&args[4]))?) } else { None };
    if let Some(requests) = requests {
        return series::serve(config, &actor, &reviewer, publication.as_ref(),
            series::Options { requests: &requests, open: existing, credibility }, super::clock);
    }
    match credibility {
        Some(path) => serve_with_credibility(config, &actor, &reviewer, publication.as_ref(),
            existing, Some(path), super::clock),
        None => serve(config, &actor, &reviewer, publication.as_ref(), existing, super::clock),
    }
}

fn serve<F>(config: Config, actor: &Profile, reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>, open: bool, time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    serve_with_credibility(config, actor, reviewer_profile, publication, open, None, time)
}

/// Qualification belongs to this operator-owned lifetime, not an actor frame.
/// Recorded requests remain observation/reconciliation-only, even with a path.
fn serve_with_credibility<F>(mut config: Config, actor: &Profile, reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>, open: bool, credibility: Option<&Path>, mut time: F)
    -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    if !open && credibility.is_some() {
        return Err("live credibility activation requires an existing journal".into());
    }
    actor.check_host(&config)?; reviewer_profile.check_host(&config)?;
    if actor.socket == reviewer_profile.socket(actor.request)
        || actor.socket == super::control::socket_path(reviewer_profile, actor.request) {
        return Err("actor, human-review and stop sockets must be distinct".into());
    }
    let start = time();
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline { logical: plus(start, runtime)?, started: Instant::now(), wall: Duration::from_millis(runtime) };
    // Claim only this new socket. Existing/stale paths are never unlinked. No
    // untrusted frame can run before the native owner and all guards are ready.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let (mut host, reviewer) = prepare_host(&config, publication, open)?;
    let recorded = match host.request_status(actor.request) {
        Ok(_) => true,
        Err(JournalError::Contract(Error::Missing)) => false,
        Err(error) => return Err(debug(error)),
    };
    if !recorded {
        if let Some(path) = credibility {
            deadline.check(time())?;
            qualification::activate(&mut host, &config, path)?;
            deadline.check(time())?;
        }
        // Never fall back to unqualified weights after recovery. This check is
        // read-only, and a never-qualified legacy journal retains its semantics.
        host.check_credibility().map_err(debug)?;
    }
    let (port, mut driver) = host.into_supervised_driver();
    // The operator can stop while the actor is absent or holds an incomplete
    // frame. Recorded keys stay receipt-only; they open no new control listener.
    let mut early_control = if recorded { None } else {
        Some(super::control::Control::new(&config, actor.request, Some(reviewer_profile))?)
    };
    let session = PeerSession::new(actor.actor, ActorWire::new(port), actor.channels(), actor.connections).map_err(debug)?;
    let mut ingress = Intake { socket, session, selected: actor.request, attempted: 0, maximum: actor.candidates };
    eprintln!("Live actor endpoint ready: {:?}; request={}", actor.socket, actor.request);
    let work = (|| {
        if recorded {
            // Opening fenced any old keys. Receipt reconciliation never launches
            // helpers or treats the old request as a new admission.
            resume_existing(&mut driver, actor.request, &mut time)?;
        }
        loop {
            deadline.check(time())?;
            if let Some(control) = &mut early_control {
                // Independent stop wins before source acquisition or actor I/O.
                if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                    return Ok(());
                }
            }
            if recorded {
                // Do not spend the reply grace before an actor has connected.
                // Even malformed first frames remain subject to the original
                // codec/ticket checks; no canonical data is pushed unsolicited.
                if ingress.observe(&mut driver)? != 0 { return Ok(()); }
                pause(actor.poll_ms);
                continue;
            }
            ingress.accept()?;
            if ingress.session.status().active.is_some() {
                let report = driver.drive_peer_request_from_file(&mut ingress.session, actor.request,
                    &mut config.source, &mut time, drive_budget()).map_err(debug)?;
                if report.drive.status.closed() { ingress.session.disconnect(); }
            }
            match driver.supervisor().host().map_err(debug)?.request_status(actor.request) {
                Ok(_) => break,
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(error) => return Err(debug(error)),
            }
            pause(actor.poll_ms);
        }
        let status = driver.supervisor().host().map_err(debug)?.request_status(actor.request).map_err(debug)?;
        if !matches!(status.disposition, FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }) {
            return Ok(());
        }
        let expires = driver.supervisor().host().map_err(debug)?.request_action(actor.request).map_err(debug)?.spec().deadline;
        let active = Deadline { logical: expires.min(deadline.logical), started: deadline.started, wall: deadline.wall };
        let mut pump = |driver: &mut FileSupervisedDriver| {
            ingress.observe(driver)?;
            let status = driver.supervisor().host().map_err(debug)?.request_status(actor.request).map_err(debug)?;
            Ok(matches!(status.disposition, FileRequestDisposition::Admitted {
                stage: ActionState::Cancelled | ActionState::Denied | ActionState::Confirmed | ActionState::ConfirmedNotExecuted, .. }))
        };
        execute_serviced(&mut driver, &reviewer, &mut config, actor.request, &active,
            ExecuteServices { peers: Some(reviewer_profile), publication, ingress: &mut pump,
                stop: early_control.take() }, &mut time)
    })();
    let mut failure = match work {
        Ok(()) => None,
        Err(error) => Some(match stop(&mut driver, actor.request, &mut time) {
            Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
        }),
    };
    // Read-free grace for terminal polls and same-session reconnects. The only
    // replies are original ActorWire responses to actual actor commands. Neither
    // disconnection nor a failed output proves nonexecution or requests a resend.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(actor.reply_ms) {
        if let Err(error) = ingress.observe(&mut driver) {
            if failure.is_none() { failure = Some(error); }
            break;
        }
        pause(actor.poll_ms);
    }
    ingress.session.revoke();
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure { return Err(format!("{error}; helper cleanup pending={pending}")); }
    if pending != 0 { return Err(format!("{pending} direct helper children lack reaping confirmation")); }
    Ok(())
}

fn drive_budget() -> DriveBudget { DriveBudget { frames: 1, ..DriveBudget::default() } }
struct Intake { socket: BoundSocket, session: PeerSession<Port>, selected: u64, attempted: u64, maximum: u64 }
impl Intake {
    fn accept(&mut self) -> Result<(), String> {
        if self.session.status().active.is_some() { return Ok(()); }
        let stream = match self.socket.listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => return Ok(()),
            Err(error) => return Err(debug(error)),
        };
        if self.attempted == self.maximum { return Err("actor candidate quota exhausted".into()); }
        self.attempted += 1;
        match self.session.attach(stream) {
            Ok(_) => Ok(()),
            Err(PeerRefusal::CredentialsRejected) if self.attempted < self.maximum => {
                eprintln!("Rejected actor process credentials; candidate={}", self.attempted); Ok(())
            }
            Err(error) => Err(debug(error)),
        }
    }
    fn observe(&mut self, driver: &mut FileSupervisedDriver) -> Result<usize, String> {
        self.accept()?;
        if self.session.status().active.is_some() {
            let report = driver.drive_peer_request_observe(&mut self.session, self.selected, drive_budget()).map_err(debug)?;
            if report.status.closed() { self.session.disconnect(); }
            return Ok(report.progress.frames);
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod qualified_tests;

// One shared bootstrap and deployment check for single and sequential service.
fn prepare_host(config: &Config, publication: Option<&PublicationProfile>, open: bool)
    -> Result<(FileOversight, super::FileHumanReviewer), String>
{
    let (mut host, reviewer) = match (open, publication) {
        (false, None) => FileOversight::create(&config.store, config.profile.clone()),
        (false, Some(profile)) => profile.create(&config.store, config.profile.clone()),
        (true, None) => FileOversight::open(&config.store, config.profile.clone()),
        (true, Some(profile)) => profile.open(&config.store, config.profile.clone()),
    }.map_err(debug)?;
    if !open {
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).map_err(debug)?;
        host.enable_file_source(host.revision(), config.source_policy).map_err(debug)?;
    }
    if host.publication_validation_profile().map_err(debug)? != publication.map(|p| p.limits)
        || host.file_source_status().map(|s| s.policy) != Some(config.source_policy)
        || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
        || !host.publication_guard_required() || config.profile.delivery.clock_domain != CLOCK_DOMAIN {
        return Err("stored deployment does not match the explicit live service profile".into());
    }
    Ok((host, reviewer))
}
