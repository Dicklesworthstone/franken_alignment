//! Serial publication on one locked authority and one authenticated actor session.
//! Scheduling is operator data; every effect still uses the original full-input
//! review, independent human key, checked publication and receipt settlement.
use super::*;

const FLAG: &str = "--requests";

pub(super) struct Options<'a> {
    pub requests: &'a [u64],
    pub open: bool,
    pub credibility: Option<&'a Path>,
}

/// An additive service-only option. The original positional/profile parsers still
/// validate everything else, and no actor wire schema or authority field changes.
pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<Vec<u64>>), String> {
    let mut positions = args.iter().enumerate().filter_map(|(i, arg)| (arg == FLAG).then_some(i));
    let Some(index) = positions.next() else { return Ok((args, None)); };
    if positions.next().is_some() { return Err("duplicate request schedule".into()); }
    let positional = match args.first().map(String::as_str) {
        Some("serve-create" | "serve-open") => 4,
        Some("serve-create-checked" | "serve-open-checked") => 5,
        _ => return Err("request schedules are supervisor service options, not actor commands".into()),
    };
    if index != positional || args.len() != index + 2 {
        return Err("use a final --requests comma-separated-keys option after all positionals".into());
    }
    let text = &args[index + 1];
    if text.len() > FileSupervisedDriver::MAX_REQUEST_SEQUENCE * 21 {
        return Err("request schedule is too large".into());
    }
    let mut requests = Vec::new();
    for field in text.split(',') {
        if requests.len() == FileSupervisedDriver::MAX_REQUEST_SEQUENCE || field.is_empty()
            || !field.bytes().all(|b| b.is_ascii_digit()) || field.starts_with('0') {
            return Err("request schedule requires at most 64 distinct nonzero canonical decimal keys".into());
        }
        let key = field.parse::<u64>().map_err(debug)?;
        if requests.contains(&key) { return Err("duplicate request key in schedule".into()); }
        requests.push(key);
    }
    Ok((&args[..index], Some(requests)))
}

fn validate(requests: &[u64], actor: &Profile, reviewer: &PeerProfile) -> Result<(), String> {
    if requests.is_empty() || requests.len() > FileSupervisedDriver::MAX_REQUEST_SEQUENCE
        || requests[0] != actor.request {
        return Err("schedule must begin with the actor profile request and contain 1..=64 keys".into());
    }
    for (index, &key) in requests.iter().enumerate() {
        if key == 0 || requests[..index].contains(&key) { return Err("invalid request schedule".into()); }
        if actor.socket == reviewer.socket(key)
            || actor.socket == super::super::control::socket_path(reviewer, key) {
            return Err("actor, human-review and stop sockets must be distinct for every scheduled key".into());
        }
    }
    Ok(())
}

pub(super) fn serve<F>(mut config: Config, actor: &Profile, reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>, options: Options<'_>, mut time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let Options { requests, open, credibility } = options;
    validate(requests, actor, reviewer_profile)?;
    if !open && credibility.is_some() { return Err("live credibility activation requires an existing journal".into()); }
    actor.check_host(&config)?;
    reviewer_profile.check_host(&config)?;
    let start = time();
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline { logical: plus(start, runtime)?, started: Instant::now(), wall: Duration::from_millis(runtime) };
    // Preflight all schedule/socket bindings before either namespace is changed.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let (host, reviewer) = prepare_host(&config, publication, open)?;
    let (port, mut driver) = host.into_supervised_driver();
    let session = PeerSession::new(actor.actor, ActorWire::new(port), actor.channels(), actor.connections).map_err(debug)?;
    let mut ingress = Intake { socket, session, selected: requests[0], attempted: 0, maximum: actor.candidates };
    // Copy only immutable, bounded launch recipes per NEW review. Never duplicate
    // a child, socket, captured input, vote, approval or effect capability.
    let programs = std::mem::take(&mut config.programs);
    let mut completed = Vec::new();
    completed.try_reserve_exact(requests.len()).map_err(debug)?;
    let mut activated = false;
    let mut frames_seen = 0_usize;
    eprintln!("Sequential actor endpoint ready: {:?}; requests={requests:?}", actor.socket);
    let work: Result<(), String> = (|| {
        for (index, &request) in requests.iter().enumerate() {
            ingress.selected = request;
            deadline.check(time())?;
            let recorded = {
                let host = driver.supervisor().host().map_err(debug)?;
                match host.request_status(request) {
                    Ok(_) => true,
                    Err(JournalError::Contract(Error::Missing)) => false,
                    Err(error) => return Err(debug(error)),
                }
            };
            // Native stop survives the entire schedule, not just a local job.
            if driver.supervisor().host().map_err(debug)?.inspect().stop.is_some() { break; }
            if recorded {
                resume_existing(&mut driver, request, &mut time)?;
            } else {
                // Keep independent operator stop available while waiting for any
                // new actor request, and transfer this SAME socket into execution.
                let mut early_control = super::super::control::Control::new(&config, request, Some(reviewer_profile))?;
                if early_control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { break; }
                {
                    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                    if !activated && let Some(path) = credibility {
                        qualification::activate(&mut host, &config, path)?;
                        activated = true;
                        deadline.check(time())?;
                    }
                    // Later expiry cannot trigger an implicit reactivation or a
                    // fallback to unqualified weights. Historical retries skip it.
                    host.check_credibility().map_err(debug)?;
                }
                loop {
                    deadline.check(time())?;
                    if early_control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { return Ok(()); }
                    ingress.accept()?;
                    if ingress.session.status().active.is_some() {
                        let report = driver.drive_peer_sequence_from_file(&mut ingress.session, request,
                            &completed, &mut config.source, &mut time, drive_budget()).map_err(debug)?;
                        frames_seen = frames_seen.saturating_add(report.drive.progress.frames);
                        if report.drive.status.closed() { ingress.session.disconnect(); }
                    }
                    match driver.supervisor().host().map_err(debug)?.request_status(request) {
                        Ok(_) => break,
                        Err(JournalError::Contract(Error::Missing)) => {}
                        Err(error) => return Err(debug(error)),
                    }
                    pause(actor.poll_ms);
                }
                let status = driver.supervisor().host().map_err(debug)?.request_status(request).map_err(debug)?;
                if matches!(status.disposition, FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }) {
                    let expires = driver.supervisor().host().map_err(debug)?.request_action(request).map_err(debug)?.spec().deadline;
                    let active = Deadline { logical: expires.min(deadline.logical), started: deadline.started, wall: deadline.wall };
                    config.programs = programs.clone();
                    let mut pump = |driver: &mut FileSupervisedDriver| {
                        frames_seen = frames_seen.saturating_add(ingress.observe_sequence(driver, &completed)?);
                        let status = driver.supervisor().host().map_err(debug)?.request_status(request).map_err(debug)?;
                        Ok(matches!(status.disposition, FileRequestDisposition::Admitted {
                            stage: ActionState::Cancelled | ActionState::Denied
                            | ActionState::Confirmed | ActionState::ConfirmedNotExecuted, .. }))
                    };
                    execute_serviced(&mut driver, &reviewer, &mut config, request, &active,
                        ExecuteServices { peers: Some(reviewer_profile), publication, ingress: &mut pump,
                            stop: Some(early_control) }, &mut time)?;
                }
            }
            if driver.supervisor().host().map_err(debug)?.inspect().stop.is_some() { break; }
            // Do not read more actor frames in the handoff: queued next requests
            // wait for the previous direct children to be confirmed reaped. No
            // replacement session, authority, permit or deadline is constructed.
            let retiring = Instant::now();
            while !driver.retire_completed_request(request).map_err(debug)? {
                deadline.check(time())?;
                if retiring.elapsed() >= Duration::from_millis(config.timing.cleanup_ms) {
                    return Err("previous helper children are not confirmed reaped; next request remains unadmitted".into());
                }
                pause(config.timing.poll_ms);
            }
            if index + 1 != requests.len() { completed.push(request); }
        }
        // A completely historical schedule still waits for an actual actor
        // command before starting reply grace; it reads no source or capsule.
        while frames_seen == 0 && driver.supervisor().host().map_err(debug)?.inspect().stop.is_none() {
            deadline.check(time())?;
            frames_seen = ingress.observe_sequence(&mut driver, &completed)?;
            if frames_seen == 0 { pause(actor.poll_ms); }
        }
        Ok(())
    })();
    let mut failure = work.err().map(|error| match stop(&mut driver, ingress.selected, &mut time) {
        Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
    });
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(actor.reply_ms) {
        if let Err(error) = ingress.observe_sequence(&mut driver, &completed) {
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

impl Intake {
    fn observe_sequence(&mut self, driver: &mut FileSupervisedDriver, completed: &[u64]) -> Result<usize, String> {
        self.accept()?;
        if self.session.status().active.is_some() {
            let report = driver.drive_peer_sequence_observe(&mut self.session, self.selected, completed, drive_budget()).map_err(debug)?;
            if report.status.closed() { self.session.disconnect(); }
            Ok(report.progress.frames)
        } else { Ok(0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(mode: &str, keys: &str) -> Vec<String> {
        [mode, "config", "actor", "reviewer", FLAG, keys].into_iter().map(str::to_owned).collect()
    }
    #[test]
    fn explicit_service_option_accepts_exact_limit_without_changing_positionals() {
        for keys in ["7,8".to_owned(), "18446744073709551615".to_owned(),
            (1..=64).map(|n| n.to_string()).collect::<Vec<_>>().join(",")] {
            let input = args("serve-open", &keys);
            let (rest, keys) = take_option(&input).unwrap();
            assert_eq!(rest, &input[..4]); assert!(!keys.unwrap().is_empty());
        }
        let mut checked = args("serve-create-checked", "7,8");
        checked.insert(4, "witness".into());
        assert_eq!(take_option(&checked).unwrap().0, &checked[..5]);
    }
    #[test]
    fn malformed_unbounded_and_actor_supplied_schedules_refuse_without_io() {
        for keys in ["", "0", "7,0", "07,8", "7,7", "7,", "7,,8", "+7", "7, 8", "18446744073709551616"] {
            assert!(take_option(&args("serve-open", keys)).is_err(), "{keys}");
        }
        let excessive = (1..=65).map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        assert!(take_option(&args("serve-open", &excessive)).is_err());
        for mode in ["actor-submit", "submit", "review-peer", "recover-stop"] {
            assert!(take_option(&args(mode, "7,8")).is_err());
        }
        let mut repeated = args("serve-open", "7,8"); repeated.extend([FLAG.into(), "9".into()]);
        assert!(take_option(&repeated).is_err());
    }
    #[test]
    fn absent_option_preserves_the_entire_legacy_command() {
        let args = vec!["actor-submit".to_owned(), "actor".to_owned(), "document".to_owned()];
        let (rest, selected) = take_option(&args).unwrap();
        assert_eq!(rest, args.as_slice()); assert!(selected.is_none());
    }
}
