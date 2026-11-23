//! Live ordinary publication with native evaluation and Andon in the first image.
//! One original owner spans the finite request schedule and independent evaluator.
use super::{
    BoundSocket, Config, Deadline, Duration, ElapsedTick, FileOversight, Instant, OwnerStart,
    PeerProfile, Profile, PublicationProfile, RecoveryReserve, andon_settings::Settings,
    check_deployment, debug, evaluator, operator, plus, precheck, series,
};
use crate::recover_stop::guarded::{CreationOutput, Custody, SubmitProfile};
use crate::workflow::{continuation::guarded, control::Control};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    evaluation::andon::FileAndonRecovery, recovery::FileGuardedRecoveryStatus,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) enum Mode<'a> {
    Create(&'a Path),
    Existing(&'a SubmitProfile),
}

pub(super) struct Selection<'a> {
    pub settings: &'a Settings,
    pub mode: Mode<'a>,
    pub requests: &'a [u64],
    pub generation: u64,
    pub operator: Option<&'a operator::Settings>,
    pub evaluator: Option<&'a evaluator::Settings>,
    pub prechecks: Option<&'a precheck::schedule::Schedule>,
    pub inputs: &'a [&'a Path],
}

enum Retention {
    Creation(CreationOutput),
    Recovery(Custody),
}
impl Retention {
    fn retain(&self, host: &FileOversight) -> Result<(), String> {
        match self {
            Self::Creation(output) => output.retain(host),
            Self::Recovery(custody) => guarded::retain(custody, host),
        }
    }
    fn failure(&self, host: &FileOversight, error: String) -> String {
        let revision = host.revision();
        let error = match self.retain(host) {
            Ok(()) => error,
            Err(retention) => format!("{error}; {retention}"),
        };
        guarded::acknowledged(revision, &error)
    }
}

pub(super) fn serve<F>(
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
        evaluator,
        prechecks,
        inputs,
    } = selected;
    // All specialised guards must be selected explicitly. A richer witness,
    // campaign profile or source-generated owner has no ordinary fallback.
    let validation = publication
        .map(PublicationProfile::simple_limits)
        .transpose()?;
    if generation == 0 || operator.is_some_and(|settings| settings.policy_promotion.is_some()) {
        return Err(
            "live Andon requires an ordinary Tower generation without policy promotion".into(),
        );
    }
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err("live Andon requires the configured Unix-millisecond clock domain".into());
    }
    series::validate(requests, actor, reviewer)?;
    actor.check_host(&config)?;
    reviewer.check_host(&config)?;
    let recovery = match &mode {
        Mode::Create(_) => {
            settings.creation()?;
            None
        }
        Mode::Existing(profile) => {
            profile.require_legacy()?;
            let custody = profile.admit(&config)?;
            if !custody.retention_requested() {
                return Err(
                    "live Andon recovery requires a new independent successor history anchor"
                        .into(),
                );
            }
            Some(custody)
        }
    };
    let creation = match &mode {
        Mode::Create(path) => Some(CreationOutput::admit(&config, path)?),
        Mode::Existing(_) => None,
    };
    let mut custody_paths = inputs.to_vec();
    custody_paths.push(config.source.path());
    if let Some(publication) = publication {
        custody_paths.extend(publication.input_paths().iter().map(PathBuf::as_path));
    }
    let history_output = match &mode {
        Mode::Create(path) => *path,
        Mode::Existing(profile) => {
            custody_paths.push(profile.history_anchor_path());
            recovery
                .as_ref()
                .and_then(Custody::output_path)
                .expect("successor retention required above")
        }
    };
    let output_identity = destination(history_output)?;
    if custody_paths
        .iter()
        .map(|path| destination(path))
        .collect::<Result<Vec<_>, _>>()?
        .contains(&output_identity)
    {
        return Err(
            "live Andon history output must be distinct from every original input and source"
                .into(),
        );
    }
    custody_paths.push(history_output);
    let endpoints = check_paths(&config, actor, reviewer, operator, requests, &custody_paths)?;
    let endpoint_paths: Vec<_> = endpoints.iter().map(PathBuf::as_path).collect();
    if let Some(operator) = operator {
        operator.check_host(&config, actor, reviewer, &endpoint_paths)?;
    }
    if let Some(evaluator) = evaluator {
        evaluator.check_host(
            &config,
            actor,
            reviewer,
            operator,
            requests,
            settings.config.identity(),
        )?;
        evaluator.check_custody_paths(&custody_paths)?;
        custody_paths.push(evaluator.socket_path());
    }
    if let Some(schedule) = prechecks {
        // The same reservation applies to creation: no later selected disclosure
        // can overwrite a retained input, new archive, or evaluator listener.
        schedule.check_recovery_host(
            &config,
            actor,
            reviewer,
            requests,
            operator,
            None,
            &custody_paths,
        )?;
    }

    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(time(), runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let mut control = Control::new(&config, actor.request, Some(reviewer))?;
    let (host, roles, retention, owner_start) = match mode {
        Mode::Create(_) => {
            deadline.check(time())?;
            let (host, roles) = match validation {
                Some(limits) => {
                    settings.create_checked(&config, limits, RecoveryReserve::terminal())?
                }
                None => settings.create(&config, Some(RecoveryReserve::terminal()))?,
            };
            (
                host,
                roles,
                Retention::Creation(creation.expect("creation custody admitted")),
                OwnerStart::Ordinary,
            )
        }
        Mode::Existing(profile) => {
            let custody = recovery.expect("recovery custody admitted");
            let expected = settings.requirements(&config, profile.minimum())?;
            let expected = match validation {
                Some(limits) => expected.require_publication(limits).map_err(debug)?,
                None => expected,
            };
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
            let now = time();
            deadline.check(now)?;
            let (host, roles, outcomes) = recovery
                .finish_reconciled(|| now)
                .map_err(guarded::unconfirmed)?;
            let retention = Retention::Recovery(custody);
            if let Err(error) = guarded::mediated::require_settled(&host, &outcomes) {
                return Err(retention.failure(&host, error));
            }
            (host, roles, retention, OwnerStart::Reconciled)
        }
    };
    let acknowledged = host.revision();
    if let Err(error) = check_deployment(&config, &host, publication)
        .and_then(|_| prechecks.map_or(Ok(()), |schedule| schedule.check_reconciled(&host)))
        .and_then(|_| deadline.check(time()))
    {
        return Err(retention.failure(&host, error));
    }
    let (port, mut driver) = host.into_supervised_driver();
    let human = roles.oversight.human;
    let prepared = control.checkpoint(&mut driver, &human, &deadline, &mut time);
    if !matches!(prepared, Ok(false)) {
        let error = prepared.err().unwrap_or_else(||
            "original native Stop acknowledged before live Andon actor intake; no new work admitted".into());
        return Err(match driver.supervisor().host() {
            Ok(host) => retention.failure(&host, error),
            Err(failure) => guarded::acknowledged(
                acknowledged,
                &format!("{error}; history retention unavailable: {}", debug(failure)),
            ),
        });
    }
    let live_evaluator = evaluator.map(|settings| (settings, roles.evaluator));
    let result = series::serve_ready_driver_with_evaluator(
        config,
        actor,
        reviewer,
        publication,
        series::Options {
            requests,
            open: owner_start == OwnerStart::Reconciled,
            credibility: None,
        },
        generation,
        operator,
        socket,
        port,
        driver,
        human,
        owner_start,
        Some(control),
        live_evaluator,
        prechecks,
        deadline,
        time,
        |host, _, _| retention.retain(host),
    );
    result.map_err(|error| guarded::acknowledged(acknowledged, &error))
}

fn stop_recovery<F>(
    recovery: FileAndonRecovery,
    custody: &Custody,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let target = recovery
        .into_recovery_stop(control.recovery_operation()?)
        .map_err(debug)?;
    let outcome = control.stop_recovery(target, deadline, time)?;
    let mut message = outcome.terminal_message();
    if let Ok(stopped) = &outcome.application.result {
        if let Err(error) = custody.retain(stopped.history_anchor()) {
            message.push_str(&format!("; history anchor retention failed: {error}; preserve the previous archive; no Stop retry was sent"));
        }
    }
    Err(message)
}

fn check_paths(
    config: &Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    operator: Option<&operator::Settings>,
    requests: &[u64],
    custody: &[&Path],
) -> Result<Vec<PathBuf>, String> {
    let store = destination(&config.store)?;
    let custody = custody
        .iter()
        .map(|path| destination(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut endpoints = vec![actor.socket.clone()];
    endpoints.extend(operator.map(|settings| settings.socket.clone()));
    for &request in requests {
        endpoints.push(reviewer.socket(request));
        endpoints.push(crate::workflow::control::socket_path(reviewer, request));
    }
    let mut resolved = Vec::new();
    for endpoint in &endpoints {
        let path = destination(endpoint)?;
        if path.starts_with(&store) || resolved.contains(&path) || custody.contains(&path) {
            return Err("live Andon service endpoints must be distinct from every scheduled endpoint, source, profile, history archive and authority store".into());
        }
        resolved.push(path);
    }
    // The operator performs its own self-comparison; only other endpoints are
    // additional reservations for that profile.
    Ok(endpoints
        .into_iter()
        .filter(|path| operator.is_none_or(|settings| *path != settings.socket))
        .collect())
}

fn destination(path: &Path) -> Result<PathBuf, String> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = fs::canonicalize(path.parent().ok_or("service path needs a parent")?)
                .map_err(debug)?;
            Ok(parent.join(path.file_name().ok_or("service path needs a filename")?))
        }
        Err(error) => Err(debug(error)),
    }
}
