//! Live Tower intake after the ORIGINAL independently anchored mediated replay.
//! Recorded requests stay source-free; new work reuses the native source rebind.
use super::super::{
    BoundSocket, Config, Deadline, Duration, ElapsedTick, FileOversight, Instant, OwnerStart,
    PeerProfile, Profile, PublicationProfile, check_deployment, debug, operator, plus,
    serve_ready_ingress,
};
use crate::recover_stop::guarded::{Custody, SubmitProfile};
use crate::workflow::continuation::{
    checkpoint::{self, AcknowledgedOutput},
    guarded::{
        self,
        mediated::{self, Recovered, profile::Selection},
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
    let mut replay_checkpoint = || {
        if deadline.started.elapsed() >= deadline.wall {
            Err("live mediated recovery replay deadline reached".to_owned())
        } else {
            Ok(())
        }
    };
    replay_checkpoint()?;
    let Recovered {
        native:
            ReconciledMediatedOwner {
                mut owner,
                roles,
                outcomes,
            },
        history,
        checkpoint: saved,
    } = mediated::recover(
        &config,
        recovery,
        &custody,
        topology,
        FilePublicationRecoveryRequirements {
            validation: publication_limits,
            feed: None,
            joint: None,
        },
        &deadline,
        &mut replay_checkpoint,
        &mut time,
    )?;
    let saved = saved.expect("v2 recovery retains original native source custody");
    let revision = owner.revision();
    let prepared = (|| {
        mediated::require_settled(&owner, &outcomes)?;
        check_deployment(&config, &owner, publication)?;
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
        deadline.check(time())?;
        let next = topology.next(&config, history, &mut || deadline.check(time()))?;
        deadline.check(time())?;
        // Original rebind preserves spent work and operation floors and requires
        // BOTH graph and inventory to advance. It revokes, never certifies.
        next.bind(&mut owner, roles.topology_observer).map(Some)
    })();
    let selected = match prepared {
        Ok(selected) => selected,
        Err(error) => {
            let retained = retain_saved(&custody, archive.as_ref(), &saved, &owner);
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
        super::super::series::serve_ready_ingress(
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
            owner,
            roles.oversight.human,
            source,
            OwnerStart::Reconciled,
            deadline,
            time,
            retain,
        )
    } else {
        serve_ready_ingress(
            config,
            actor,
            reviewer,
            publication,
            credibility,
            None,
            Some(generation),
            operator,
            socket,
            owner,
            roles.oversight.human,
            source,
            OwnerStart::Reconciled,
            deadline,
            time,
            retain,
        )
    };
    result.map_err(|error| guarded::acknowledged(revision, &error))
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
