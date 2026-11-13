//! Serial publication on one locked authority and one authenticated actor session.
//! Scheduling is operator data; every effect still uses the original full-input
//! review, independent human key, checked publication and receipt settlement.
use super::super::continuation::creation::topology::Selection as TopologySelection;
use super::*;

#[path = "series/topology.rs"]
mod topology;

const FLAG: &str = "--requests";

pub(super) struct Options<'a> {
    pub requests: &'a [u64],
    pub open: bool,
    pub credibility: Option<&'a Path>,
}

/// An additive service-only option. The original positional/profile parsers still
/// validate everything else, and no actor wire schema or authority field changes.
pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<Vec<u64>>), String> {
    let mut positions = args
        .iter()
        .enumerate()
        .filter_map(|(i, arg)| (arg == FLAG).then_some(i));
    let Some(index) = positions.next() else {
        return Ok((args, None));
    };
    if positions.next().is_some() {
        return Err("duplicate request schedule".into());
    }
    let positional = match args.first().map(String::as_str) {
        Some("serve-create" | "serve-open") => 4,
        Some("serve-create-checked" | "serve-open-checked") => 5,
        _ => {
            return Err(
                "request schedules are supervisor service options, not actor commands".into(),
            );
        }
    };
    if index != positional || args.len() != index + 2 {
        return Err(
            "use a final --requests comma-separated-keys option after all positionals".into(),
        );
    }
    Ok((&args[..index], Some(parse_requests(&args[index + 1])?)))
}

/// One closed schedule grammar shared by native and Tower service selection.
/// This parses bounded operator data only and performs no file or owner work.
pub(super) fn parse_requests(text: &str) -> Result<Vec<u64>, String> {
    if text.len() > FileSupervisedDriver::MAX_REQUEST_SEQUENCE * 21 {
        return Err("request schedule is too large".into());
    }
    let mut requests = Vec::new();
    for field in text.split(',') {
        if requests.len() == FileSupervisedDriver::MAX_REQUEST_SEQUENCE
            || field.is_empty()
            || !field.bytes().all(|b| b.is_ascii_digit())
            || field.starts_with('0')
        {
            return Err(
                "request schedule requires at most 64 distinct nonzero canonical decimal keys"
                    .into(),
            );
        }
        let key = field.parse::<u64>().map_err(debug)?;
        if requests.contains(&key) {
            return Err("duplicate request key in schedule".into());
        }
        requests.push(key);
    }
    Ok(requests)
}

pub(super) fn validate(
    requests: &[u64],
    actor: &Profile,
    reviewer: &PeerProfile,
) -> Result<(), String> {
    if requests.is_empty()
        || requests.len() > FileSupervisedDriver::MAX_REQUEST_SEQUENCE
        || requests[0] != actor.request
    {
        return Err(
            "schedule must begin with the actor profile request and contain 1..=64 keys".into(),
        );
    }
    for (index, &key) in requests.iter().enumerate() {
        if key == 0 || requests[..index].contains(&key) {
            return Err("invalid request schedule".into());
        }
        if actor.socket == reviewer.socket(key)
            || actor.socket == super::super::control::socket_path(reviewer, key)
        {
            return Err(
                "actor, human-review and stop sockets must be distinct for every scheduled key"
                    .into(),
            );
        }
    }
    Ok(())
}

pub(super) fn serve<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_ingress(
        config,
        actor,
        reviewer_profile,
        publication,
        options,
        None,
        None,
        None,
        time,
    )
}

/// The common Tower transport changes only the original schedule's ingress.
/// Every selected request still passes the same source, review, human approval,
/// publication, settlement and confirmed-child-reaping boundaries below.
pub(super) fn serve_tower<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    generation: u64,
    operator_settings: Option<&operator::Settings>,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    if generation == 0 {
        return Err("Tower registration generation must be nonzero".into());
    }
    serve_ingress(
        config,
        actor,
        reviewer_profile,
        publication,
        options,
        Some(generation),
        operator_settings,
        None,
        time,
    )
}

/// A separately authenticated governor exists only on a newly guarded owner.
/// Every guard and the terminal reserve are in the first canonical image.
pub(super) fn serve_tower_governed<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    requests: &[u64],
    generation: u64,
    operator_settings: &operator::Settings,
    governor_settings: &governor::Settings,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_tower_governed_checked(
        config,
        actor,
        reviewer_profile,
        None,
        requests,
        generation,
        operator_settings,
        governor_settings,
        time,
    )
}

/// The independently selected plain witness profile remains mandatory through
/// policy promotion and every later effect on this SAME original owner.
pub(super) fn serve_tower_governed_checked<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    requests: &[u64],
    generation: u64,
    operator_settings: &operator::Settings,
    governor_settings: &governor::Settings,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileGuardSet;
    if generation == 0 || requests.len() < 2 {
        return Err("live governor creation requires a nonzero Tower generation and at least two scheduled requests".into());
    }
    // Richer producer/feed/wait/snapshot/joint selections cannot silently lose
    // their extra native gates at guarded bootstrap. Refuse before any clock,
    // source read, listener or canonical store is entered.
    let publication_limits = publication
        .map(PublicationProfile::simple_limits)
        .transpose()?;
    validate(requests, actor, reviewer_profile)?;
    let mut additional = requests
        .iter()
        .flat_map(|&request| {
            [
                reviewer_profile.socket(request),
                super::super::control::socket_path(reviewer_profile, request),
            ]
        })
        .collect::<Vec<_>>();
    additional.push(governor_settings.socket.clone());
    let additional = additional
        .iter()
        .map(|path| path.as_path())
        .collect::<Vec<_>>();
    operator_settings.check_host(&config, actor, reviewer_profile, &additional)?;
    governor_settings.check_host(
        &config,
        actor,
        reviewer_profile,
        operator_settings,
        requests,
    )?;
    let start = time();
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(start, runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let guards = FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: Some(config.source_policy),
        identity: None,
        campaigns: Some(governor_settings.campaign_requirement()),
        credential: None,
    };
    let (host, roles) = match publication_limits {
        Some(limits) => FileOversight::create_guarded_publication_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            RecoveryReserve::terminal(),
            limits,
        ),
        None => FileOversight::create_guarded_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            RecoveryReserve::terminal(),
        ),
    }
    .map_err(debug)?;
    check_deployment(&config, &host, publication)?;
    let governor = roles
        .policy_governor
        .ok_or("guarded bootstrap did not provision the selected governor")?;
    let (port, driver) = host.into_supervised_driver();
    serve_ready_driver_with_governor(
        config,
        actor,
        reviewer_profile,
        publication,
        Options {
            requests,
            open: false,
            credibility: None,
        },
        Some(generation),
        Some(operator_settings),
        socket,
        port,
        driver,
        roles.human,
        None,
        OwnerStart::Ordinary,
        None,
        Some((governor_settings, governor)),
        deadline,
        time,
        |_, _, _| Ok(()),
    )
}

/// An explicit schedule through the original atomically mediated bootstrap.
/// One selected source and its cumulative work follow every final publication.
pub(super) fn serve_tower_with_topology<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    generation: u64,
    operator_settings: Option<&operator::Settings>,
    topology: TopologySelection,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    if generation == 0 || options.open {
        return Err(
            "scheduled topology creation requires a nonzero Tower generation and a new owner"
                .into(),
        );
    }
    serve_ingress(
        config,
        actor,
        reviewer_profile,
        publication,
        options,
        Some(generation),
        operator_settings,
        Some(topology),
        time,
    )
}

fn serve_ingress<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    tower_generation: Option<u64>,
    operator_settings: Option<&operator::Settings>,
    topology_selection: Option<TopologySelection>,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let Options {
        requests,
        open,
        credibility,
    } = options;
    validate(requests, actor, reviewer_profile)?;
    if operator_settings.is_some() && tower_generation.is_none() {
        return Err("operator request schedules require an explicit Tower generation".into());
    }
    if !open && credibility.is_some() {
        return Err("live credibility activation requires an existing journal".into());
    }
    actor.check_host(&config)?;
    reviewer_profile.check_host(&config)?;
    let topology_publication = if let Some(selection) = &topology_selection {
        if open || tower_generation.is_none() {
            return Err(
                "scheduled topology creation requires an explicit Tower generation and a new owner"
                    .into(),
            );
        }
        // Admission retains the original plain checked profile. Richer feed,
        // snapshot, wait and joint contracts cannot lose their selected guards.
        let limits = publication
            .map(PublicationProfile::simple_limits)
            .transpose()?;
        topology::check_destinations(
            &config,
            requests,
            actor,
            reviewer_profile,
            operator_settings,
            selection.acknowledged_output(),
        )?;
        limits
    } else {
        None
    };
    if let Some(settings) = operator_settings {
        // Check every future review/Stop path before the first socket or store
        // exists. A later handoff must never collide with this listener.
        let additional: Vec<_> = requests
            .iter()
            .flat_map(|&request| {
                [
                    reviewer_profile.socket(request),
                    super::super::control::socket_path(reviewer_profile, request),
                ]
            })
            .collect();
        let paths: Vec<_> = additional.iter().map(|path| path.as_path()).collect();
        settings.check_host(&config, actor, reviewer_profile, &paths)?;
    }
    let start = time();
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(start, runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    let prepared_topology = topology_selection
        .map(|selection| selection.prepare(&config, &mut || deadline.check(time())))
        .transpose()?;
    // Preflight all schedule/socket bindings before either namespace is changed.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let (host, reviewer, topology, output) = match prepared_topology {
        Some(mut prepared) => {
            let output = prepared.take_output();
            let (host, reviewer, source) = prepared.create(&config, topology_publication)?;
            check_deployment(&config, &host, publication)?;
            (host, reviewer, Some(source), output)
        }
        None => {
            let (host, reviewer) = prepare_host(&config, publication, open)?;
            (host, reviewer, None, None)
        }
    };
    serve_ready_ingress(
        config,
        actor,
        reviewer_profile,
        publication,
        Options {
            requests,
            open,
            credibility,
        },
        tower_generation,
        operator_settings,
        socket,
        host,
        reviewer,
        topology,
        OwnerStart::Ordinary,
        deadline,
        time,
        |host, source, _succeeded| {
            // Earlier effects and all source expenditure survive a later failed
            // request. Attempt their exact native checkpoint before releasing
            // this owner, using retained documents rather than changed files.
            // Retention supplies no new source check, authority or effect retry;
            // the common lifecycle preserves both work and export failures.
            output
                .as_ref()
                .map_or(Ok(()), |output| output.retain(host, source))
        },
    )
}

/// Drive one original ready owner through the finite schedule. Creation and
/// complete guarded recovery select the same intake, source, review, publication
/// and helper-retirement path; the caller retains its exact native custody before
/// cleanup releases the owner. A recovered historical schedule is observation-only.
pub(super) fn serve_ready_ingress<F, R>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    tower_generation: Option<u64>,
    operator_settings: Option<&operator::Settings>,
    socket: BoundSocket,
    host: FileOversight,
    reviewer: super::super::FileHumanReviewer,
    topology: Option<FileDriverTopology>,
    owner_start: OwnerStart,
    deadline: Deadline,
    time: F,
    retain: R,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
    R: FnOnce(&FileOversight, Option<&FileDriverTopology>, bool) -> Result<(), String>,
{
    let (port, driver) = host.into_supervised_driver();
    serve_ready_driver(
        config,
        actor,
        reviewer_profile,
        publication,
        options,
        tower_generation,
        operator_settings,
        socket,
        port,
        driver,
        reviewer,
        topology,
        owner_start,
        None,
        deadline,
        time,
        retain,
    )
}

/// Continue the SAME recovered gateway and first request's Stop transport.
/// Later schedule entries acquire only their originally selected endpoints.
pub(super) fn serve_ready_driver<F, R>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    tower_generation: Option<u64>,
    operator_settings: Option<&operator::Settings>,
    socket: BoundSocket,
    port: Port,
    driver: FileSupervisedDriver,
    reviewer: super::super::FileHumanReviewer,
    topology: Option<FileDriverTopology>,
    owner_start: OwnerStart,
    early_control: Option<super::super::control::Control>,
    deadline: Deadline,
    time: F,
    retain: R,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
    R: FnOnce(&FileOversight, Option<&FileDriverTopology>, bool) -> Result<(), String>,
{
    serve_ready_driver_with_governor(
        config,
        actor,
        reviewer_profile,
        publication,
        options,
        tower_generation,
        operator_settings,
        socket,
        port,
        driver,
        reviewer,
        topology,
        owner_start,
        early_control,
        None,
        deadline,
        time,
        retain,
    )
}

fn serve_ready_driver_with_governor<F, R>(
    mut config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    options: Options<'_>,
    tower_generation: Option<u64>,
    operator_settings: Option<&operator::Settings>,
    socket: BoundSocket,
    port: Port,
    mut driver: FileSupervisedDriver,
    reviewer: super::super::FileHumanReviewer,
    topology: Option<FileDriverTopology>,
    owner_start: OwnerStart,
    mut early_control: Option<super::super::control::Control>,
    governor: Option<(&governor::Settings, fa_reference::action::consequence::delivery::persistent::observed::governance::campaigns::FilePolicyGovernor)>,
    deadline: Deadline,
    mut time: F,
    retain: R,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
    R: FnOnce(&FileOversight, Option<&FileDriverTopology>, bool) -> Result<(), String>,
{
    let Options {
        requests,
        credibility,
        ..
    } = options;
    let result = (|| {
        if topology::install(
            &mut driver,
            topology,
            &deadline,
            &mut time,
            early_control.as_mut(),
            &reviewer,
        )? {
            return Ok(());
        }
        // One independent read session spans this original owner lifetime. A new
        // selected request never reissues custody, clears a pending native cursor
        // or resets transport, query, candidate or elapsed allowances.
        let await_abandonment =
            operator_settings.is_some_and(|settings| settings.abandonment.is_some());
        let mut operator = operator_settings
            .map(|settings| {
                operator::Service::start(
                    settings,
                    actor,
                    port.clone(),
                    &driver,
                    tower_generation.expect("operator generation checked above"),
                    &deadline,
                )
            })
            .transpose()?;
        if let Some((settings, role)) = governor {
            let service = governor::Service::start(settings, actor, role, &deadline)?;
            operator
                .as_mut()
                .ok_or("governor has no independently selected operator")?
                .attach_governor(service)?;
        }
        let mut ingress = tower::SelectedIngress::new(socket, port, actor, tower_generation)?;
        // Copy only immutable, bounded launch recipes per NEW review. Never duplicate
        // a child, socket, captured input, vote, approval or effect capability.
        let programs = std::mem::take(&mut config.programs);
        let mut completed = Vec::new();
        completed.try_reserve_exact(requests.len()).map_err(debug)?;
        let mut activated = false;
        let mut frames_seen = 0_usize;
        // A reconciled history has no live admission to revoke merely because an
        // observer disconnected or timed out. The first new scheduled request ends
        // that observational mode, retaining ordinary error containment thereafter.
        let mut observational = owner_start == OwnerStart::Reconciled;
        eprintln!(
            "Sequential actor endpoint ready: {:?}; requests={requests:?}",
            actor.socket
        );
        let work: Result<(), String> = (|| {
            for (index, &request) in requests.iter().enumerate() {
                // Drop only the prior idle Stop listener before selecting its
                // successor. Active execution owns and retires its original control.
                if index != 0 {
                    early_control = None;
                }
                ingress.select(request);
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
                if driver
                    .supervisor()
                    .host()
                    .map_err(debug)?
                    .inspect()
                    .stop
                    .is_some()
                {
                    break;
                }
                if !recorded || owner_start == OwnerStart::Reconciled || await_abandonment {
                    if early_control.is_none() {
                        early_control = Some(super::super::control::Control::new(
                            &config,
                            request,
                            Some(reviewer_profile),
                        )?);
                    }
                    if early_control
                        .as_mut()
                        .expect("selected Stop control")
                        .checkpoint(&mut driver, &reviewer, &deadline, &mut time)?
                    {
                        break;
                    }
                } else {
                    early_control =
                        Some(super::super::control::Control::new(&config, request, None)?);
                }
                if recorded {
                    if owner_start == OwnerStart::Ordinary {
                        resume_existing(&mut driver, request, &mut time)?;
                    }
                } else {
                    observational = false;
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
                        if early_control
                            .as_mut()
                            .expect("new request Stop control")
                            .checkpoint(&mut driver, &reviewer, &deadline, &mut time)?
                        {
                            return Ok(());
                        }
                        if let Some(service) = &mut operator {
                            service.checkpoint_driver(&mut driver);
                        }
                        if early_control
                            .as_mut()
                            .expect("original control")
                            .native_stop(&mut driver, &mut time)?
                        {
                            return Ok(());
                        }
                        frames_seen = frames_seen.saturating_add(ingress.admit_sequence(
                            &mut driver,
                            &completed,
                            &mut config.source,
                            &mut time,
                        )?);
                        match driver
                            .supervisor()
                            .host()
                            .map_err(debug)?
                            .request_status(request)
                        {
                            Ok(_) => break,
                            Err(JournalError::Contract(Error::Missing)) => {}
                            Err(error) => return Err(debug(error)),
                        }
                        pause(actor.poll_ms);
                    }
                    let status = driver
                        .supervisor()
                        .host()
                        .map_err(debug)?
                        .request_status(request)
                        .map_err(debug)?;
                    if matches!(
                        status.disposition,
                        FileRequestDisposition::Admitted {
                            stage: ActionState::Reviewing,
                            ..
                        }
                    ) {
                        let expires = driver
                            .supervisor()
                            .host()
                            .map_err(debug)?
                            .request_action(request)
                            .map_err(debug)?
                            .spec()
                            .deadline;
                        let active = Deadline {
                            logical: expires.min(deadline.logical),
                            started: deadline.started,
                            wall: deadline.wall,
                        };
                        config.programs = programs.clone();
                        let mut pump = |driver: &mut FileSupervisedDriver| {
                            // LiveControl services independent Stop before this pump.
                            if let Some(service) = &mut operator {
                                service.checkpoint_driver(driver);
                            }
                            frames_seen = frames_seen
                                .saturating_add(ingress.observe_sequence(driver, &completed)?);
                            let status = driver
                                .supervisor()
                                .host()
                                .map_err(debug)?
                                .request_status(request)
                                .map_err(debug)?;
                            Ok(matches!(
                                status.disposition,
                                FileRequestDisposition::Admitted {
                                    stage: ActionState::Cancelled
                                        | ActionState::Denied
                                        | ActionState::Confirmed
                                        | ActionState::ConfirmedNotExecuted
                                        | ActionState::IrrecoverablyUnknown,
                                    ..
                                }
                            ))
                        };
                        execute_serviced(
                            &mut driver,
                            &reviewer,
                            &mut config,
                            request,
                            &active,
                            ExecuteServices {
                                peers: Some(reviewer_profile),
                                publication,
                                ingress: &mut pump,
                                stop: early_control.as_ref().map(|control| control.share()),
                            },
                            &mut time,
                        )?;
                    }
                }
                if driver
                    .supervisor()
                    .host()
                    .map_err(debug)?
                    .inspect()
                    .stop
                    .is_some()
                {
                    break;
                }
                // Explicit abandonment may end a local Unknown request, but cannot
                // establish its external outcome or release its charged liability.
                // Keep this same operator grant and observation-only actor session
                // live within their original deadline before deciding handoff.
                // No provider/driver step, source read, reconciliation, resend or
                // journal clock observation occurs merely because the wait polls.
                while await_abandonment
                    && matches!(
                        driver
                            .supervisor()
                            .host()
                            .map_err(debug)?
                            .request_status(request)
                            .map_err(debug)?
                            .disposition,
                        FileRequestDisposition::Admitted {
                            stage: ActionState::Unknown,
                            ..
                        }
                    )
                {
                    deadline.check(time())?;
                    let control = early_control
                        .as_mut()
                        .expect("original abandonment Stop control");
                    if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                        return Ok(());
                    }
                    if let Some(service) = &mut operator {
                        service.checkpoint_driver(&mut driver);
                    }
                    if control.native_stop(&mut driver, &mut time)? {
                        return Ok(());
                    }
                    frames_seen = frames_seen
                        .saturating_add(ingress.observe_sequence(&mut driver, &completed)?);
                    pause(actor.poll_ms);
                }
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
                if index + 1 != requests.len() {
                    completed.push(request);
                }
            }
            // A completely historical schedule still waits for an actual actor
            // command before starting reply grace; it reads no source or capsule.
            while frames_seen == 0
                && driver
                    .supervisor()
                    .host()
                    .map_err(debug)?
                    .inspect()
                    .stop
                    .is_none()
            {
                deadline.check(time())?;
                if let Some(control) = &mut early_control {
                    if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                        return Ok(());
                    }
                }
                if let Some(service) = &mut operator {
                    service.checkpoint_driver(&mut driver);
                }
                if let Some(control) = &mut early_control {
                    if control.native_stop(&mut driver, &mut time)? {
                        return Ok(());
                    }
                }
                frames_seen = ingress.observe_sequence(&mut driver, &completed)?;
                if frames_seen == 0 {
                    pause(actor.poll_ms);
                }
            }
            Ok(())
        })();
        let mut automatic_cleanup_attempted = false;
        let mut failure = work.err().map(|error| {
            if observational
                || early_control
                    .as_ref()
                    .is_some_and(|control| control.native_cleanup_started())
            {
                error
            } else {
                automatic_cleanup_attempted = true;
                match stop(&mut driver, ingress.selected(), &mut time) {
                    Ok(()) => error,
                    Err(stopping) => format!("{error}; stop/drain: {stopping}"),
                }
            }
        });
        if automatic_cleanup_attempted {
            // Automatic cleanup has already spent its one attempt. Keep the
            // original operator reply channel, never a second drain entry.
            drop(early_control.take());
        }
        let started = Instant::now();
        let mut actor_transport_failed = false;
        while started.elapsed() < Duration::from_millis(actor.reply_ms) {
            if owner_start == OwnerStart::Reconciled {
                if let Some(control) = &mut early_control {
                    if let Err(error) =
                        control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)
                    {
                        failure.get_or_insert(error);
                    }
                }
            }
            if let Some(service) = &mut operator {
                service.checkpoint();
            }
            if let Some(control) = &mut early_control {
                if let Err(error) = control.native_stop(&mut driver, &mut time) {
                    failure.get_or_insert(error);
                }
            }
            if !actor_transport_failed {
                if let Err(error) = ingress.observe_sequence(&mut driver, &completed) {
                    failure.get_or_insert(error);
                    actor_transport_failed = true;
                    // The actor transport does not own the independent
                    // operator's remaining original terminal reply grace.
                    ingress.revoke();
                }
            }
            pause(actor.poll_ms);
        }
        if let Some(service) = &mut operator {
            service.retire();
        }
        ingress.revoke();
        failure.map_or(Ok(()), Err)
    })();
    let mut failure = result.err();
    let retained = match driver.supervisor().host() {
        Ok(host) => retain(&host, driver.topology(), failure.is_none()),
        Err(error) => Err(debug(error)),
    };
    if let Err(error) = retained {
        failure = Some(match failure {
            Some(failure) => format!("{failure}; {error}"),
            None => error,
        });
    }
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure {
        return Err(format!("{error}; helper cleanup pending={pending}"));
    }
    if pending != 0 {
        return Err(format!(
            "{pending} direct helper children lack reaping confirmation"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(mode: &str, keys: &str) -> Vec<String> {
        [mode, "config", "actor", "reviewer", FLAG, keys]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
    #[test]
    fn explicit_service_option_accepts_exact_limit_without_changing_positionals() {
        for keys in [
            "7,8".to_owned(),
            "18446744073709551615".to_owned(),
            (1..=64)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ] {
            let input = args("serve-open", &keys);
            let (rest, keys) = take_option(&input).unwrap();
            assert_eq!(rest, &input[..4]);
            assert!(!keys.unwrap().is_empty());
        }
        let mut checked = args("serve-create-checked", "7,8");
        checked.insert(4, "witness".into());
        assert_eq!(take_option(&checked).unwrap().0, &checked[..5]);
    }
    #[test]
    fn malformed_unbounded_and_actor_supplied_schedules_refuse_without_io() {
        for keys in [
            "",
            "0",
            "7,0",
            "07,8",
            "7,7",
            "7,",
            "7,,8",
            "+7",
            "7, 8",
            "18446744073709551616",
        ] {
            assert!(take_option(&args("serve-open", keys)).is_err(), "{keys}");
        }
        let excessive = (1..=65)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert!(take_option(&args("serve-open", &excessive)).is_err());
        for mode in ["actor-submit", "submit", "review-peer", "recover-stop"] {
            assert!(take_option(&args(mode, "7,8")).is_err());
        }
        let mut repeated = args("serve-open", "7,8");
        repeated.extend([FLAG.into(), "9".into()]);
        assert!(take_option(&repeated).is_err());
    }
    #[test]
    fn absent_option_preserves_the_entire_legacy_command() {
        let args = vec![
            "actor-submit".to_owned(),
            "actor".to_owned(),
            "document".to_owned(),
        ];
        let (rest, selected) = take_option(&args).unwrap();
        assert_eq!(rest, args.as_slice());
        assert!(selected.is_none());
    }
}
