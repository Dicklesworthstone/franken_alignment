//! Live Tower intake after the ORIGINAL independently anchored mediated replay.
//! Recorded requests stay source-free; new work reuses the native source rebind.
use super::super::{
    BoundSocket, Config, Deadline, Duration, ElapsedTick, FileOversight, FileSupervisedDriver,
    Instant, OwnerStart, PeerProfile, Profile, PublicationProfile, check_deployment,
    debug, operator, plus, serve_ready_driver,
};
use crate::recover_stop::guarded::{Custody, SubmitProfile};
use crate::workflow::{FileHumanReviewer, control::Control};
use crate::workflow::continuation::{
    checkpoint::{self, AcknowledgedOutput},
    guarded::{
        self,
        mediated::{self, ReadyRecovery, Recovered, profile::Selection},
    },
};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::{
        driver::topology::{FileDriverTopology, checkpoint::TopologyCheckpoint},
        guarded::recovery::{
            FilePublicationRecoveryRequirements, mediated::ReconciledMediatedOwner,
        },
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(in super::super) fn serve<F>(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    publication: Option<&PublicationProfile>,
    credibility: Option<&Path>,
    recovery: &SubmitProfile,
    topology: &Selection,
    generation: u64,
    operator: Option<&operator::Settings>,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_selected(
        config,
        actor,
        reviewer,
        publication,
        credibility,
        recovery,
        topology,
        generation,
        operator,
        None,
        time,
    )
}

/// Resume one explicit schedule on the original independently anchored owner.
/// A schedule with only retained requests needs no new source; a schedule with
/// new work rebinds the original cumulative source once for its entire lifetime.
pub(in super::super) fn serve_sequence<F>(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    publication: Option<&PublicationProfile>,
    credibility: Option<&Path>,
    recovery: &SubmitProfile,
    topology: &Selection,
    generation: u64,
    operator: Option<&operator::Settings>,
    requests: &[u64],
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_selected(
        config,
        actor,
        reviewer,
        publication,
        credibility,
        recovery,
        topology,
        generation,
        operator,
        Some(requests),
        time,
    )
}

fn serve_selected<F>(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    publication: Option<&PublicationProfile>,
    credibility: Option<&Path>,
    recovery: &SubmitProfile,
    topology: &Selection,
    generation: u64,
    operator: Option<&operator::Settings>,
    requests: Option<&[u64]>,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    // Own the complete bootstrap before any source, clock, socket or recovery
    // mutation. Richer publication contracts cannot collapse to plain limits.
    let publication_limits = publication
        .map(PublicationProfile::simple_limits)
        .transpose()?;
    if generation == 0 || !topology.checkpoint_selected() {
        return Err("live mediated Tower recovery requires a nonzero generation and an exact fa.mediated-submit/2 source checkpoint".into());
    }
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err(
            "live mediated recovery requires the configured Unix-millisecond clock domain".into(),
        );
    }
    actor.check_host(&config)?;
    reviewer.check_host(&config)?;
    let scheduled = requests.unwrap_or(std::slice::from_ref(&actor.request));
    super::super::series::validate(scheduled, actor, reviewer)?;
    if let Some(settings) = operator {
        let endpoints: Vec<_> = scheduled
            .iter()
            .flat_map(|&request| {
                [
                    reviewer.socket(request),
                    crate::workflow::control::socket_path(reviewer, request),
                ]
            })
            .collect();
        let paths: Vec<_> = endpoints.iter().map(|path| path.as_path()).collect();
        settings.check_host(&config, actor, reviewer, &paths)?;
    }
    if actor.socket == reviewer.socket(actor.request)
        || actor.socket == crate::workflow::control::socket_path(reviewer, actor.request)
    {
        return Err("actor, human-review and stop sockets must be distinct".into());
    }
    let custody = recovery.admit(&config)?;
    let archive = topology
        .acknowledged_output()
        .map(|path| AcknowledgedOutput::admit(&config, path))
        .transpose()?;
    check_destinations(
        actor,
        reviewer,
        operator,
        scheduled,
        &custody,
        archive.as_ref(),
    )?;
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let deadline = Deadline {
        logical: plus(time(), runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    // Existing paths are never removed to make recovery start. The actor owns
    // no frames until complete native replay, fencing and fresh checks finish.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let mut control = Control::new(&config, actor.request, Some(reviewer))?;
    let mut pending = false;
    let ready = {
        let mut replay_checkpoint = || {
            deadline.check(time())?;
            pending |= control.recovery_pending_peer()?;
            Ok(())
        };
        replay_checkpoint()?;
        mediated::ready(
            &config,
            recovery,
            &custody,
            topology,
            FilePublicationRecoveryRequirements {
                validation: publication_limits,
                feed: None,
                joint: None,
            },
            &mut replay_checkpoint,
        )?
    };
    // Ready has its own final cooperative checkpoint. A retained candidate
    // selects only the original no-clock admission Stop, before ordinary Fence,
    // reconciliation, source acquisition or any live owner role is published.
    if pending {
        return stop_recovery(&config, &custody, archive.as_ref(), ready, &mut control,
            &deadline, &mut time);
    }
    let Recovered {
        native: ReconciledMediatedOwner { owner, roles, outcomes },
        history,
        checkpoint: saved,
    } = ready.finish(&deadline, &mut time)?;
    let saved = saved.expect("v2 recovery retains original native source custody");
    let revision = owner.revision();
    let (port, mut driver) = owner.into_supervised_driver();
    let prepared = (|| {
        {
            let owner = driver.supervisor().host().map_err(debug)?;
            mediated::require_settled(&owner, &outcomes)?;
            check_deployment(&config, &owner, publication)?;
        }
        checkpoint_live(&mut control, &mut driver, &roles.oversight.human,
            &deadline, &mut time)?;
        let owner = driver.supervisor().host().map_err(debug)?;
        let mut has_new_work = false;
        for &request in scheduled {
            match owner.request_status(request) {
                Ok(_) => {}
                Err(JournalError::Contract(Error::Missing)) => has_new_work = true,
                Err(error) => return Err(debug(error)),
            }
        }
        if !has_new_work {
            return Ok(None);
        }
        if owner.inspect().stop.is_some() {
            return Err("the original domain is terminally stopped".into());
        }
        drop(owner);
        let next = topology.next(&config, history, &mut || {
            checkpoint_live(&mut control, &mut driver, &roles.oversight.human,
                &deadline, &mut time)
        })?;
        checkpoint_live(&mut control, &mut driver, &roles.oversight.human,
            &deadline, &mut time)?;
        // Original rebind preserves spent work and operation floors and requires
        // BOTH graph and inventory to advance. It revokes, never certifies.
        let mut owner = driver.supervisor_mut().host_mut().map_err(debug)?;
        next.bind(&mut owner, roles.topology_observer).map(Some)
    })();
    let selected = match prepared {
        Ok(selected) => selected,
        Err(error) => {
            let retained = match driver.supervisor().host() {
                Ok(owner) => retain_saved(&custody, archive.as_ref(), &saved, &owner),
                Err(error) => Err(debug(error)),
            };
            let error = match retained {
                Ok(()) => error,
                Err(retention) => format!("{error}; {retention}"),
            };
            return Err(guarded::acknowledged(revision, &error));
        }
    };
    let (source, capture) = match selected {
        Some((source, capture)) => (Some(source), capture),
        None => (None, None),
    };
    // The SAME service installs the original source, performs its initial check,
    // acquires current actor evidence and uses the driver's final cut before
    // publication. Neither target version nor expected actor epoch is rewritten.
    let retain = |host: &FileOversight, source: Option<&FileDriverTopology>, _succeeded| {
        let history = guarded::retain(&custody, host);
        let source = match (archive.as_ref(), capture.as_ref()) {
            (Some(archive), Some(capture)) => capture
                .capture(host, source)
                .and_then(|saved| archive.retain(&saved)),
            (Some(archive), None) => retain_unchanged(archive, &saved, host),
            (None, _) => Ok(()),
        };
        // Acknowledged failures keep custody too, before releasing the
        // original driver; each independent export is attempted exactly once.
        checkpoint::combine(history, source)
    };
    let result = if let Some(requests) = requests {
        super::super::series::serve_ready_driver(
            config,
            actor,
            reviewer,
            publication,
            super::super::series::Options {
                requests,
                open: true,
                credibility,
            },
            Some(generation),
            operator,
            socket,
            port,
            driver,
            roles.oversight.human,
            source,
            OwnerStart::Reconciled,
            Some(control),
            deadline,
            time,
            retain,
        )
    } else {
        serve_ready_driver(
            config,
            actor,
            reviewer,
            publication,
            credibility,
            None,
            Some(generation),
            operator,
            socket,
            port,
            driver,
            roles.oversight.human,
            source,
            OwnerStart::Reconciled,
            Some(control),
            deadline,
            time,
            retain,
        )
    };
    result.map_err(|error| guarded::acknowledged(revision, &error))
}

fn checkpoint_live<F>(control: &mut Control, driver: &mut FileSupervisedDriver,
    reviewer: &FileHumanReviewer, deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? {
        return Err("original native Stop acknowledged before live actor intake; no new work admitted".into());
    }
    Ok(())
}

fn stop_recovery<F>(config: &Config, custody: &Custody,
    archive: Option<&AcknowledgedOutput>, ready: ReadyRecovery, control: &mut Control,
    deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    // Pin original store identity before native acknowledgment. Only the
    // selected historical checkpoint owns source custody; no next files open.
    let identity = fs::canonicalize(&config.store).map_err(debug)?;
    let target = ready.recovery.into_recovery_stop(control.recovery_operation()?)
        .map_err(debug)?;
    let outcome = control.stop_recovery(target, deadline, time)?;
    let mut terminal = outcome.terminal_message();
    if let Ok(stopped) = &outcome.application.result {
        let history = custody.retain(stopped.history_anchor()).map_err(|error| format!(
            "history anchor retention failed: {error}; keep the previous archive and select a new output path"));
        let source = archive.map_or(Ok(()), |archive| {
            let saved = ready.history.checkpoint()
                .ok_or("admission Stop requires the selected exact source checkpoint")?;
            let stopped = saved.after_admission_stop(&config.profile, &identity, stopped)
                .map_err(debug)?;
            archive.retain(&stopped)
        }).map_err(|error: String| format!(
            "terminal topology checkpoint retention failed: {error}; keep previous paired custody and use terminal checkpoint recovery with new output paths"));
        // A late deadline or lost reply cannot suppress either independent
        // export after acknowledgment, and an export failure never retries Stop.
        if let Err(error) = checkpoint::combine(history, source) {
            terminal.push_str("; ");
            terminal.push_str(&error);
        }
    }
    Err(terminal)
}

fn retain_saved(
    custody: &Custody,
    archive: Option<&AcknowledgedOutput>,
    saved: &TopologyCheckpoint,
    owner: &FileOversight,
) -> Result<(), String> {
    checkpoint::combine(
        guarded::retain(custody, owner),
        archive.map_or(Ok(()), |archive| retain_unchanged(archive, saved, owner)),
    )
}

fn retain_unchanged(
    archive: &AcknowledgedOutput,
    saved: &TopologyCheckpoint,
    owner: &FileOversight,
) -> Result<(), String> {
    if &owner.history_anchor().map_err(debug)? != saved.history_anchor() {
        return Err("source checkpoint retention refused: the original owner advanced after receipt reconciliation; the saved source cut cannot describe later Stop or other work; retain the new history and use native terminal source-checkpoint recovery".into());
    }
    archive.retain(saved)
}

// Every archive is admitted create-new and outside the authority store before
// fencing. Pin canonical parents so an alias cannot become a retired endpoint.
fn check_destinations(
    actor: &Profile,
    reviewer: &PeerProfile,
    operator: Option<&operator::Settings>,
    requests: &[u64],
    custody: &Custody,
    archive: Option<&AcknowledgedOutput>,
) -> Result<(), String> {
    let mut sockets = vec![canonical_destination(&actor.socket)?];
    for &request in requests {
        sockets.push(canonical_destination(&reviewer.socket(request))?);
        sockets.push(canonical_destination(
            &crate::workflow::control::socket_path(reviewer, request),
        )?);
    }
    if let Some(operator) = operator {
        sockets.push(canonical_destination(&operator.socket)?);
    }
    if sockets
        .iter()
        .enumerate()
        .any(|(index, path)| sockets[..index].contains(path))
    {
        return Err("actor, reviewer, Stop and operator endpoints must be distinct".into());
    }
    let outputs: Vec<_> = [custody.output_path(), archive.map(AcknowledgedOutput::path)]
        .into_iter()
        .flatten()
        .collect();
    if outputs.len() == 2 && outputs[0] == outputs[1] {
        return Err(
            "history anchor and source checkpoint outputs must be distinct new files".into(),
        );
    }
    if outputs
        .iter()
        .any(|output| sockets.iter().any(|socket| socket.as_path() == *output))
    {
        return Err(
            "recovery archives and actor, reviewer, Stop or operator sockets must be distinct"
                .into(),
        );
    }
    Ok(())
}

fn canonical_destination(path: &Path) -> Result<PathBuf, String> {
    let parent = fs::canonicalize(path.parent().ok_or("endpoint needs a parent directory")?)
        .map_err(debug)?;
    Ok(parent.join(path.file_name().ok_or("endpoint needs a filename")?))
}
