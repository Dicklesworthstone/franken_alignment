//! Multi-listener consumer of the ORIGINAL pooled ingress and publication workflow.
mod ingress;
use ingress::{Intake, Listeners};
use super::*;
use super::super::control::Control;
use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::MAX_POOL_PEERS;
use std::collections::BTreeSet;

const FLAG: &str = "--peers";

// Use existing strict actor profiles, including EACH profile's request binding.
// Do not reinterpret a single-request profile as an unrestricted multi-user port.
fn arguments(args: &[String]) -> Result<(usize, bool, bool), String> {
    let (open, checked) = match args.first().map(String::as_str) {
        Some("serve-create") => (false, false), Some("serve-open") => (true, false),
        Some("serve-create-checked") => (false, true), Some("serve-open-checked") => (true, true),
        _ => return Err("--peers is a supervisor service option".into()),
    };
    let index = if checked { 5 } else { 4 };
    if args.get(index).map(String::as_str) != Some(FLAG)
        || args.len() <= index + 1 || args.len() > index + MAX_POOL_PEERS
        || args.iter().filter(|a| a.as_str() == FLAG).count() != 1
        || args.iter().any(|a| a == "--requests")
        || args[index + 1..].iter().any(|a| a.is_empty() || a.starts_with("--")) {
        return Err("use final --peers ACTOR_PROFILE [ACTOR_PROFILE ...], with 2..=16 total peers and no --requests".into());
    }
    Ok((index, open, checked))
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    let (index, open, checked) = arguments(args)?;
    if !open && credibility.is_some() { return Err("live credibility activation requires an existing journal".into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let mut actors = Vec::new();
    actors.try_reserve_exact(args.len() - index).map_err(debug)?;
    actors.push(Profile::read(Path::new(&args[2]))?);
    for path in &args[index + 1..] { actors.push(Profile::read(Path::new(path))?); }
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    let publication = if checked { Some(PublicationProfile::read(Path::new(&args[4]))?) } else { None };
    serve(config, &actors, &reviewer, publication.as_ref(), open, credibility, super::super::clock)
}

fn validate(config: &Config, actors: &[Profile], reviewer: &PeerProfile) -> Result<(), String> {
    if actors.is_empty() || actors.len() > MAX_POOL_PEERS { return Err("peer count out of range".into()); }
    reviewer.check_host(config)?;
    for (i, actor) in actors.iter().enumerate() {
        actor.check_host(config)?;
        if actors[..i].iter().any(|other| other.request == actor.request || other.socket == actor.socket) {
            return Err("peer requests and actor socket paths must be unique".into());
        }
        for other in actors {
            if actor.socket == reviewer.socket(other.request)
                || actor.socket == super::super::control::socket_path(reviewer, other.request) {
                return Err("actor, review and stop socket namespaces overlap".into());
            }
        }
    }
    Ok(())
}

// Return only journal facts. Missing means still waiting, never a negative verdict.
fn terminal_requests(driver: &FileSupervisedDriver, actors: &[Profile]) -> Result<BTreeSet<u64>, String> {
    let host = driver.supervisor().host().map_err(debug)?;
    let mut done = BTreeSet::new();
    for actor in actors {
        match host.request_status(actor.request) {
            Ok(status) if matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)
                | FileRequestDisposition::Admitted { stage: ActionState::Confirmed | ActionState::Denied
                    | ActionState::Cancelled | ActionState::ConfirmedNotExecuted, .. }) => {
                done.insert(actor.request);
            }
            Ok(_) | Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(debug(error)),
        }
    }
    Ok(done)
}

fn retire<F>(driver: &mut FileSupervisedDriver, intake: &mut Intake, request: u64,
    config: &Config, deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    while !driver.retire_completed_request(request).map_err(debug)? {
        deadline.check(time())?;
        if started.elapsed() >= Duration::from_millis(config.timing.cleanup_ms) {
            return Err("previous helper children are not confirmed reaped; no new request admitted".into());
        }
        intake.observe(driver)?;
        pause(config.timing.poll_ms);
    }
    Ok(())
}

fn serve<F>(mut config: Config, actors: &[Profile], reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>, open: bool, credibility: Option<&Path>, mut time: F)
    -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    validate(&config, actors, reviewer_profile)?;
    if !open && credibility.is_some() { return Err("live credibility activation requires an existing journal".into()); }
    let runtime = actors.iter().map(|a| a.runtime_ms).min().unwrap().min(config.timing.runtime_ms);
    let poll_ms = actors.iter().map(|a| a.poll_ms).min().unwrap();
    let reply_ms = actors.iter().map(|a| a.reply_ms).min().unwrap();
    let deadline = Deadline { logical: plus(time(), runtime)?, started: Instant::now(), wall: Duration::from_millis(runtime) };
    // Bind ALL independent endpoints before creating/opening a durable owner.
    // The original BoundSocket removes only its own identity on setup failure.
    let listeners = Listeners::bind(actors)?;
    let (host, reviewer) = prepare_host(&config, publication, open)?;
    let mut recorded = BTreeSet::new();
    for actor in actors {
        match host.request_status(actor.request) {
            Ok(_) => { recorded.insert(actor.request); }
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(debug(error)),
        }
    }
    let (port, mut driver) = host.into_supervised_driver();
    let mut intake = listeners.connect(port)?;
    // One stop transport for the WHOLE service, named by the first profile.
    // Shared handles passed to execute retain the SAME socket and admission quota.
    // Pure historical recovery creates no stop/review endpoint or helper process.
    let operation = actors[0].request;
    let mut control = if recorded.len() == actors.len() { None } else {
        Some(Control::new(&config, operation, Some(reviewer_profile))?)
    };
    let programs = std::mem::take(&mut config.programs);
    let mut activated = false;
    eprintln!("Multi-peer actor service ready: {} endpoints; stop operation={operation}", actors.len());
    let work: Result<(), String> = (|| {
        for &request in &recorded {
            deadline.check(time())?;
            resume_existing(&mut driver, request, &mut time)?;
            retire(&mut driver, &mut intake, request, &config, &deadline, &mut time)?;
        }
        loop {
            deadline.check(time())?;
            if let Some(control) = &mut control {
                if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { break; }
            }
            if driver.supervisor().host().map_err(debug)?.inspect().stop.is_some() { break; }
            let done = terminal_requests(&driver, actors)?;
            if done.len() == actors.len() && intake.all_seen() { break; }
            if done.len() == actors.len() {
                intake.observe(&mut driver)?;
            } else {
                // At most ONE complete frame across all peers per intake turn.
                // Process its journal outcome before admitting another request.
                // Never turn a queued old target/version into a new proposal.
                if recorded.len() != actors.len() {
                    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                    if !activated && let Some(path) = credibility {
                        qualification::activate(&mut host, &config, path)?;
                        activated = true;
                        deadline.check(time())?;
                    }
                    host.check_credibility().map_err(debug)?;
                }
                intake.drive(&mut driver, &mut config.source, &mut time)?;
            }
            if let Some(ready) = intake.pool.next_request(&driver).map_err(debug)? {
                let request = ready.status.request;
                if ready.peer != request { return Err("request-bound pool returned a foreign key".into()); }
                match ready.status.disposition {
                    FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. } => {
                        if recorded.contains(&request) { return Err("recovered request cannot start a new review".into()); }
                        let expires = driver.supervisor().host().map_err(debug)?.request_action(request).map_err(debug)?.spec().deadline;
                        let active = Deadline { logical: expires.min(deadline.logical), started: deadline.started, wall: deadline.wall };
                        config.programs = programs.clone();
                        let mut pump = |driver: &mut FileSupervisedDriver| {
                            intake.observe(driver)?;
                            let status = driver.supervisor().host().map_err(debug)?.request_status(request).map_err(debug)?;
                            Ok(matches!(status.disposition, FileRequestDisposition::Admitted {
                                stage: ActionState::Cancelled | ActionState::Denied | ActionState::Confirmed
                                    | ActionState::ConfirmedNotExecuted, .. }))
                        };
                        let retained_stop = control.as_ref().ok_or("missing service stop owner")?.share();
                        execute_serviced(&mut driver, &reviewer, &mut config, request, &active,
                            ExecuteServices { peers: Some(reviewer_profile), publication, ingress: &mut pump,
                                stop: Some(retained_stop) }, &mut time)?;
                    }
                    FileRequestDisposition::Admitted { stage: ActionState::Dispatching | ActionState::Unknown, .. } => {
                        resume_existing(&mut driver, request, &mut time)?;
                    }
                    _ => return Err("unexpected original ready-queue state".into()),
                }
                if driver.supervisor().host().map_err(debug)?.inspect().stop.is_some() { break; }
                retire(&mut driver, &mut intake, request, &config, &deadline, &mut time)?;
            }
            pause(poll_ms);
        }
        Ok(())
    })();
    let mut failure = work.err().map(|error| match stop(&mut driver, operation, &mut time) {
        Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
    });
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(reply_ms) {
        if let Err(error) = intake.observe(&mut driver) {
            if failure.is_none() { failure = Some(error); }
            break;
        }
        pause(poll_ms);
    }
    intake.pool.revoke_all();
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure { return Err(format!("{error}; helper cleanup pending={pending}")); }
    if pending != 0 { return Err(format!("{pending} direct helper children lack reaping confirmation")); }
    Ok(())
}

#[cfg(test)]
mod tests;
