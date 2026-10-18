//! Continue the ORIGINAL mediated domain; never reopen through a weaker profile.
//! Historical requests stay query-only and never consume the next live source.
mod profile;
use profile::Selection;
use super::{Config, Submission, SubmitProfile, PeerProfile, FileOversight, FileGuardSet,
    FileRecoveryRequirements, FileGuardedRecoveryStatus, RecoveryReserve, JournalError,
    Reconciliation, ActionState, ElapsedTick, Deadline, RunResult, Continuation,
    combine_failure, retain, acknowledged, unconfirmed, debug, plus};
use super::super::{checkpoint as source_checkpoint, from_recovered_with_topology_retaining};
#[cfg(test)]
use super::super::from_recovered_with_topology;
use crate::publication::PublicationProfile;
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FilePublicationRecoveryRequirements;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    mediated::FileMediatedRequirements, recovery::mediated::ReconciledMediatedOwner};
use fa_reference::action::consequence::oversight::actor_wire::{Command, decode_command, MAX_FRAME_BYTES};
use std::{io::Write, path::Path, time::{Duration, Instant}};

#[cfg(test)]
fn submit<F>(config: Config, submission: Submission<'_>, topology: &Selection, time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    submit_selected(config, submission, topology, None, time)
}

fn submit_selected<F>(config: Config, submission: Submission<'_>, topology: &Selection,
    publication: Option<&PublicationProfile>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    submit_selected_with_elapsed(config, submission, topology, publication, time, started,
        &mut || started.elapsed())
}

#[cfg(test)]
fn submit_with_elapsed<F>(config: Config, submission: Submission<'_>, topology: &Selection,
    time: F, started: Instant, elapsed: &mut impl FnMut() -> Duration)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    submit_selected_with_elapsed(config, submission, topology, None, time, started, elapsed)
}

fn submit_selected_with_elapsed<F>(config: Config, submission: Submission<'_>, topology: &Selection,
    publication: Option<&PublicationProfile>, mut time: F, started: Instant,
    elapsed: &mut impl FnMut() -> Duration) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    // Checked mode is independently selected, never inferred from the journal.
    // A richer profile cannot be reduced to just its numerical witness limits.
    // This preflight reads no capture, clock, topology source or canonical store.
    let publication_limits = publication.map(PublicationProfile::simple_limits).transpose()?;
    let publication_requirements = FilePublicationRecoveryRequirements {
        validation: publication_limits, feed: None, joint: None,
    };
    let Submission { document, profile, peers, credibility } = submission;
    let Command::Submit { request, proposal } = decode_command(document).map_err(debug)? else {
        return Err("mediated submit requires an original submit document".into());
    };
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err("mediated submit requires the configured Unix-millisecond clock domain".into());
    }
    if let Some(peers) = peers { peers.check_host(&config)?; }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall { Err("mediated submission recovery replay deadline reached".to_owned()) }
        else { Ok(()) }
    };
    checkpoint()?;
    let runtime_deadline = plus(time(), config.timing.runtime_ms)?;
    let custody = profile.admit(&config)?;
    checkpoint()?;
    // Only independent historical contracts are loaded before recovery. A known
    // request never needs the next live files, source read, helper or human key.
    let history = topology.history(&config, &mut checkpoint)?;
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
                source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
            effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
            minimum: history.minimum(custody.anchor(), profile.minimum())?,
        },
        topology: history.requirement.clone(), prediction: None, evaluation: None,
    };
    let mut recovery = FileOversight::begin_open_mediated_guarded_anchored(
        &config.store, config.profile.clone(), &expected, custody.anchor())
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
        .and_then(|run| run.require_publication_profile(publication_requirements)).map_err(unconfirmed)?;
    history.check_extent(recovery.progress().total_events)?;
    loop {
        checkpoint()?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(progress.replayed_events, 1).map_err(unconfirmed)?;
    }
    let now = time();
    Deadline { logical: runtime_deadline, started, wall }.check(now)?;
    let ReconciledMediatedOwner { mut owner, roles, outcomes } =
        recovery.finish_reconciled(|| now).map_err(unconfirmed)?;
    let revision = owner.revision();
    let prepared = (|| {
        let outstanding = owner.inspect().control.ledger.stages.values().any(|stage|
            matches!(stage, ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
        if outstanding || outcomes.values().any(|result| !matches!(result, Ok(Reconciliation::Resolved(_)))) {
            return Err("unresolved prior deliveries; no new work admitted; charges remain in the original ledger".into());
        }
        if owner.publication_validation_profile().map_err(debug)? != publication_limits {
            return Err("mediated submit requires its matching explicit publication profile".into());
        }
        match owner.request_status(request) {
            Ok(_) => {
                if topology.retention_requested() {
                    return Err("receipt-only recovery cannot capture a fresh source checkpoint; omit retain_checkpoint".into());
                }
                return Ok(None);
            }
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(debug(error)),
        }
        if owner.inspect().stop.is_some() { return Err("the original domain is terminally stopped".into()); }
        let deadline = Deadline { logical: proposal.deadline.min(runtime_deadline), started, wall };
        deadline.check(time())?;
        let next = topology.next(&config, history, &mut checkpoint)?;
        deadline.check(time())?;
        // Restore independently retained source work on unavailable coverage.
        // Native rebind advances generations and revokes authority, not certifies.
        let selected = next.bind(&mut owner, roles.topology_observer)?;
        // Transfer custody before the shared post-rebind deadline check so an
        // expiry can still retain the acknowledged rebind and source counters.
        Ok::<_, String>(Some(selected))
    })();
    let selected = match prepared {
        Ok(selected) => selected,
        Err(error) => {
            let error = combine_failure(Some(error), retain(&custody, &owner).err()).expect("preparation refusal");
            return Err(acknowledged(revision, &error));
        }
    };
    let (selected, checkpoint_output) = match selected {
        Some((source, output)) => (Some(source), output),
        None => (None, None),
    };
    // SAME source acquisition, initial native certification, post-read clock,
    // helper congress, human approval, final automatic topology check and cleanup
    // as creation. No actor epoch, target, payload or deadline is rewritten.
    let result = from_recovered_with_topology_retaining(config, owner, roles.oversight.human, Continuation {
        document, request, proposal, peers, publication, credibility, runtime_deadline, started,
    }, selected, time, |host, topology| source_checkpoint::combine(retain(&custody, host),
        checkpoint_output.as_ref().map_or(Ok(()), |output| output.retain(host, topology))));
    match result {
        Ok(mut result) => {
            result.failure = result.failure.map(|error| acknowledged(revision, &error));
            Ok(result)
        }
        Err(error) => Err(acknowledged(revision, &error)),
    }
}

const USAGE: &str = "usage: supervise_publication submit CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE --topology-profile TOPOLOGY_PROFILE [--publication-profile WITNESS_PROFILE] [--reviewer-profile REVIEWER_JSON] [--credibility-activation EVIDENCE_FILE]";

pub(super) fn command(args: &[String], credibility: Option<&Path>, out: &mut impl Write) -> Result<(), String> {
    let (args, publication_path) = publication_option(args)?;
    let (recovery_path, topology_path, peer_path) = arguments(&args)?;
    let config = Config::read(Path::new(&args[1]))?;
    let document = crate::config::read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
    let recovery = SubmitProfile::read(recovery_path)?;
    let topology = Selection::read(topology_path)?;
    let peers = peer_path.map(PeerProfile::read).transpose()?;
    let publication = publication_path.map(PublicationProfile::read).transpose()?;
    let result = submit_selected(config, Submission { document: &document, profile: &recovery,
        peers: peers.as_ref(), credibility }, &topology, publication.as_ref(), crate::workflow::clock)?;
    super::emit(result, out)
}

// Validate the entire pair grammar before removing the one optional profile.
// A missing option value cannot consume another flag or be repaired by removal.
fn publication_option(args: &[String]) -> Result<(Vec<String>, Option<&Path>), String> {
    if !matches!(args.len(), 7 | 9 | 11) || args[0] != "submit"
        || args[1..3].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    let mut stripped = args[..3].to_vec();
    let mut publication = None;
    for pair in args[3..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") { return Err(USAGE.into()); }
        if pair[0] == "--publication-profile" {
            if publication.is_some() { return Err(USAGE.into()); }
            publication = Some(Path::new(&pair[1]));
        } else {
            stripped.extend_from_slice(pair);
        }
    }
    // Own all duplicate/unknown flags before opening a single input file.
    arguments(&stripped)?;
    Ok((stripped, publication))
}

fn arguments(args: &[String]) -> Result<(&Path, &Path, Option<&Path>), String> {
    if !matches!(args.len(), 7 | 9) || args[0] != "submit" { return Err(USAGE.into()); }
    let (mut recovery, mut topology, mut peers) = (None, None, None);
    for pair in args[3..].chunks_exact(2) {
        let selected = match pair[0].as_str() {
            "--guarded" => &mut recovery,
            "--topology-profile" => &mut topology,
            "--reviewer-profile" => &mut peers,
            _ => return Err(USAGE.into()),
        };
        if selected.is_some() || pair[1].is_empty() || pair[1].starts_with("--") { return Err(USAGE.into()); }
        *selected = Some(Path::new(&pair[1]));
    }
    Ok((recovery.ok_or(USAGE)?, topology.ok_or(USAGE)?, peers))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "mediated/checked.rs"]
mod checked;
