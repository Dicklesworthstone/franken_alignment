//! Independently anchored ordinary recovery with fresh governor custody.
//! The original cursor owns replay, fencing, reconciliation and Stop. No old
//! approval, service cache, source observation or actor proposal is recreated.
use super::{
    BoundSocket, Config, Deadline, Duration, ElapsedTick, FileOversight, Instant, OwnerStart,
    PeerProfile, Profile, PublicationProfile, RecoveryReserve, check_deployment, debug, governor,
    operator, plus, series,
};
use crate::recover_stop::guarded::{Custody, SubmitProfile};
use crate::workflow::{
    continuation::guarded::{self, mediated::require_settled},
    control::Control,
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::{
    FileGuardedRecovery, FileGuardedRecoveryStatus, FilePublicationRecoveryRequirements,
    ReconciledGuardedOwner,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Verify the exact independent ordinary requirements on the original locked
/// cursor, yielding at each event and once at Ready. This performs no Fence,
/// clock observation, source read, role issuance or staging-file cleanup.
fn ready(
    config: &Config,
    profile: &SubmitProfile,
    custody: &Custody,
    publication: FilePublicationRecoveryRequirements,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
) -> Result<FileGuardedRecovery, String> {
    checkpoint()?;
    let mut recovery = FileOversight::begin_open_guarded_anchored(
        &config.store,
        config.profile.clone(),
        &profile.requirements(config),
        custody.anchor(),
    )
    .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
    .and_then(|run| run.require_publication_profile(publication))
    .map_err(guarded::unconfirmed)?;
    loop {
        checkpoint()?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready {
            return Ok(recovery);
        }
        recovery
            .advance(progress.replayed_events, 1)
            .map_err(guarded::unconfirmed)?;
    }
}

pub(super) fn serve<F>(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    publication: Option<&PublicationProfile>,
    credibility: Option<&Path>,
    profile: &SubmitProfile,
    requests: &[u64],
    generation: u64,
    operator: &operator::Settings,
    governor: &governor::Settings,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    // Own the full selection before clock, socket or canonical mutation. A
    // plain witness profile cannot stand in for a richer selected verifier.
    let validation = publication
        .map(PublicationProfile::simple_limits)
        .transpose()?;
    if generation == 0 || requests.len() < 2 {
        return Err("governed recovery requires a nonzero Tower generation and at least two explicitly scheduled requests".into());
    }
    if profile.campaign_requirement() != Some(governor.campaign_requirement()) {
        return Err("governed recovery requires fa.guarded-submit/2 with the exact independently selected governor campaign limits".into());
    }
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err(
            "governed recovery requires the configured Unix-millisecond clock domain".into(),
        );
    }
    actor.check_host(&config)?;
    reviewer.check_host(&config)?;
    series::validate(requests, actor, reviewer)?;
    let mut endpoints: Vec<_> = requests
        .iter()
        .flat_map(|&request| {
            [
                reviewer.socket(request),
                crate::workflow::control::socket_path(reviewer, request),
            ]
        })
        .collect();
    endpoints.push(governor.socket.clone());
    let paths: Vec<_> = endpoints.iter().map(|path| path.as_path()).collect();
    operator.check_host(&config, actor, reviewer, &paths)?;
    governor.check_host(&config, actor, reviewer, operator, requests)?;
    let custody = profile.admit(&config)?;
    check_destinations(actor, reviewer, operator, governor, requests, &custody)?;

    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(time(), runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let mut control = Control::new(&config, actor.request, Some(reviewer))?;
    let mut pending = false;
    let recovery = ready(
        &config,
        profile,
        &custody,
        FilePublicationRecoveryRequirements {
            validation,
            feed: None,
            joint: None,
        },
        &mut || {
            deadline.check(time())?;
            pending |= control.recovery_pending_peer()?;
            Ok(())
        },
    )?;
    if pending {
        // Complete replay and requirements precede the offer. A pending peer
        // cannot fall through into ordinary live recovery if its exchange fails.
        return stop_recovery(recovery, &custody, &mut control, &deadline, &mut time);
    }

    let now = time();
    deadline.check(now)?;
    let ReconciledGuardedOwner {
        owner,
        roles,
        outcomes,
    } = recovery
        .finish_reconciled(|| now)
        .map_err(guarded::unconfirmed)?;
    let revision = owner.revision();
    if let Err(error) = require_settled(&owner, &outcomes)
        .and_then(|_| check_deployment(&config, &owner, publication))
    {
        return Err(retain_failure(&custody, &owner, revision, error));
    }
    let fresh_governor = match roles.policy_governor {
        Some(role) => role,
        None => {
            return Err(retain_failure(
                &custody,
                &owner,
                revision,
                "guarded recovery did not return the required fresh governor".into(),
            ));
        }
    };
    let (port, mut driver) = owner.into_supervised_driver();
    let prepared = (|| {
        deadline.check(time())?;
        if control.checkpoint(&mut driver, &roles.human, &deadline, &mut time)? {
            return Err(
                "original native Stop acknowledged before live actor intake; no new work admitted"
                    .into(),
            );
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        let retained = match driver.supervisor().host() {
            Ok(owner) => retain_failure(&custody, &owner, revision, error),
            Err(failure) => guarded::acknowledged(
                revision,
                &format!(
                    "{error}; history anchor retention unavailable: {}",
                    debug(failure)
                ),
            ),
        };
        return Err(retained);
    }

    // The same source/actor/helper/human workflow acquires current evidence for
    // new work. Historical requests and old promotion permits are not upgraded.
    let result = series::serve_ready_driver_with_governor(
        config,
        actor,
        reviewer,
        publication,
        series::Options {
            requests,
            open: true,
            credibility,
        },
        Some(generation),
        Some(operator),
        socket,
        port,
        driver,
        roles.human,
        None,
        OwnerStart::Reconciled,
        Some(control),
        Some((governor, fresh_governor)),
        deadline,
        time,
        |owner, _, _| guarded::retain(&custody, owner),
    );
    result.map_err(|error| guarded::acknowledged(revision, &error))
}

fn retain_failure(
    custody: &Custody,
    owner: &FileOversight,
    revision: u64,
    error: String,
) -> String {
    let message = match guarded::retain(custody, owner) {
        Ok(()) => error,
        Err(retention) => format!("{error}; {retention}"),
    };
    guarded::acknowledged(revision, &message)
}

fn stop_recovery<F>(
    ready: FileGuardedRecovery,
    custody: &Custody,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let target = ready
        .into_recovery_stop(control.recovery_operation()?)
        .map_err(debug)?;
    let outcome = control.stop_recovery(target, deadline, time)?;
    let mut terminal = outcome.terminal_message();
    if let Ok(stopped) = &outcome.application.result {
        if let Err(error) = custody.retain(stopped.history_anchor()) {
            terminal.push_str(&format!("; history anchor retention failed: {error}; keep the previous archive and select a new output path; no Stop retry was sent"));
        }
    }
    Err(terminal)
}

fn check_destinations(
    actor: &Profile,
    reviewer: &PeerProfile,
    operator: &operator::Settings,
    governor: &governor::Settings,
    requests: &[u64],
    custody: &Custody,
) -> Result<(), String> {
    let mut sockets = vec![
        canonical_destination(&actor.socket)?,
        canonical_destination(&operator.socket)?,
        canonical_destination(&governor.socket)?,
    ];
    for &request in requests {
        sockets.push(canonical_destination(&reviewer.socket(request))?);
        sockets.push(canonical_destination(
            &crate::workflow::control::socket_path(reviewer, request),
        )?);
    }
    if sockets
        .iter()
        .enumerate()
        .any(|(index, path)| sockets[..index].contains(path))
    {
        return Err(
            "actor, reviewer, Stop, operator and governor endpoints must be distinct".into(),
        );
    }
    if custody
        .output_path()
        .is_some_and(|output| sockets.iter().any(|socket| socket.as_path() == output))
    {
        return Err(
            "history anchor output and every scheduled service endpoint must be distinct".into(),
        );
    }
    Ok(())
}

fn canonical_destination(path: &Path) -> Result<PathBuf, String> {
    let parent = fs::canonicalize(path.parent().ok_or("endpoint needs a parent directory")?)
        .map_err(debug)?;
    Ok(parent.join(path.file_name().ok_or("endpoint needs a filename")?))
}

#[cfg(test)]
#[path = "policy_recovery/tests.rs"]
mod tests;
