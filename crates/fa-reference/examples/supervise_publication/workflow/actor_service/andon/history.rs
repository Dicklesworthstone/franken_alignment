//! Delayed independent assessment of recorded ordinary publication requests.
//! One original guarded replay and Fence retain accounting and liabilities;
//! no actor intake, source acquisition, review or effect reconciliation occurs.
use super::{
    Config, Control, Deadline, Duration, ElapsedTick, FileGuardedRecoveryStatus, FileOversight,
    Instant, Mode, PeerProfile, Profile, PublicationProfile, RecoveryReserve, Retention, Selection,
    check_deployment, check_paths, debug, destination, evaluator, guarded, plus, series,
    stop_recovery,
};
use super::super::pause;
use std::path::PathBuf;

pub(in super::super) fn serve<F>(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selected: Selection<'_>,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let Selection {
        settings,
        mode,
        requests,
        generation,
        operator,
        governor,
        evaluator,
        prechecks,
        inputs,
    } = selected;
    let Mode::Existing(profile) = mode else {
        return Err("historical ordinary evaluation requires guarded recovery".into());
    };
    let evaluator = evaluator.ok_or("historical ordinary evaluation requires an evaluator")?;
    if generation == 0 || operator.is_some() || governor.is_some() || prechecks.is_some() {
        return Err(
            "historical ordinary evaluation requires a Tower generation without operator, governor or prechecks"
                .into(),
        );
    }
    let validation = publication
        .map(PublicationProfile::simple_limits)
        .transpose()?;
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err("historical ordinary evaluation requires the configured clock domain".into());
    }
    series::validate(requests, actor, reviewer)?;
    actor.check_host(&config)?;
    reviewer.check_host(&config)?;
    profile.require_legacy()?;
    let custody = profile.admit(&config)?;
    let history_output = custody.output_path().ok_or(
        "historical ordinary evaluation requires a new independent successor history anchor",
    )?;
    let mut custody_paths = inputs.to_vec();
    custody_paths.push(config.source.path());
    custody_paths.push(profile.history_anchor_path());
    if let Some(publication) = publication {
        custody_paths.extend(publication.input_paths().iter().map(PathBuf::as_path));
    }
    let output_identity = destination(history_output)?;
    if custody_paths
        .iter()
        .map(|path| destination(path))
        .collect::<Result<Vec<_>, _>>()?
        .contains(&output_identity)
    {
        return Err(
            "historical evaluation history output must be distinct from every original input and source"
                .into(),
        );
    }
    custody_paths.push(history_output);
    // Reserve every selected endpoint even though actor and human-review
    // listeners are never bound. Future pathname aliases cannot acquire custody.
    check_paths(&config, actor, reviewer, None, requests, &custody_paths)?;
    evaluator.check_host(
        &config,
        actor,
        reviewer,
        None,
        requests,
        settings.config.identity(),
    )?;
    evaluator.check_custody_paths(&custody_paths)?;
    let expected = settings.requirements(&config, profile.minimum())?;
    let expected = match validation {
        Some(limits) => expected.require_publication(limits).map_err(debug)?,
        None => expected,
    };

    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(time(), runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    deadline.check(time())?;
    let mut control = Control::new(&config, actor.request, Some(reviewer))?;
    let mut recovery = FileOversight::begin_open_andon_guarded(
        &config.store,
        config.profile.clone(),
        &expected,
        &settings.protocol,
        custody.anchor(),
    )
    .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
    .map_err(guarded::unconfirmed)?;
    let mut pending = false;
    loop {
        deadline.check(time())?;
        pending |= control.recovery_pending_peer()?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready {
            break;
        }
        recovery
            .advance(progress.replayed_events, 1)
            .map_err(guarded::unconfirmed)?;
    }
    if pending {
        return stop_recovery(recovery, &custody, &mut control, &deadline, &mut time);
    }
    // Select only the exact original request book. Denied, held and uncertain
    // requests remain historical cases; none is turned into a fresh proposal.
    for &request in requests {
        recovery.recorded_request(request).map_err(debug)?;
    }
    deadline.check(time())?;
    if control.recovery_pending_peer()? {
        return stop_recovery(recovery, &custody, &mut control, &deadline, &mut time);
    }
    // No Time/Sweep reconciliation: unknown delivery liabilities stay charged.
    let (host, roles) = recovery.finish().map_err(guarded::unconfirmed)?;
    let retention = Retention::Recovery(custody);
    if let Err(error) = check_deployment(&config, &host, publication) {
        return Err(retention.failure(&host, error));
    }
    if host.inspect().stop.is_some() {
        // An existing terminal Stop grants no new evaluation opportunity. Do
        // not enter Control::native_stop and thereby resume its old sweep.
        return Err(retention.failure(
            &host,
            "a stopped ordinary owner cannot start historical evaluation".into(),
        ));
    }
    let acknowledged = host.revision();
    let (unused_actor, mut driver) = host.into_supervised_driver();
    drop(unused_actor);
    let human = roles.oversight.human;
    let mut service = None;
    let outcome: Result<(), String> = (|| {
        deadline.check(time())?;
        if control.checkpoint(&mut driver, &human, &deadline, &mut time)? {
            return Ok(());
        }
        service = Some(evaluator::Service::start(
            evaluator,
            actor,
            roles.evaluator,
            &driver,
            &deadline,
        )?);
        let service = service.as_mut().expect("original evaluator started");
        while !service.is_retired() && deadline.check(time()).is_ok() {
            if control.checkpoint(&mut driver, &human, &deadline, &mut time)? {
                break;
            }
            // Every bounded evaluator turn follows the same original Stop
            // checkpoint. A reconnect never replaces this service or its role.
            service.checkpoint_driver(&mut driver);
            if !service.is_retired() {
                pause(actor.poll_ms.clamp(1, 50));
            }
        }
        Ok(())
    })();
    if let Some(service) = service.as_mut() {
        service.finish();
        service.retire();
    }
    drop(control);
    // Retire every mutation-capable endpoint before capturing the final cut,
    // including late labels, an acknowledged Stop, or a startup refusal.
    match driver.supervisor().host() {
        Ok(host) => match outcome {
            Ok(()) => retention
                .retain(&host)
                .map_err(|error| guarded::acknowledged(host.revision(), &error)),
            Err(error) => Err(retention.failure(&host, error)),
        },
        Err(error) => {
            let failure = format!("history retention unavailable: {}", debug(error));
            let failure = match outcome {
                Ok(()) => failure,
                Err(error) => format!("{error}; {failure}"),
            };
            Err(guarded::acknowledged(acknowledged, &failure))
        }
    }
}
