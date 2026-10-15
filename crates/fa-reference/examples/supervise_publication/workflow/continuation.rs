//! Submit a NEW request to an existing publication journal without resetting its
//! rights, policy, request identities or endpoint history. Recovery of a known
//! request remains query-only, including when its original deadline has elapsed.
//!
//! Opening uses the original owner's durable recovery fence. In particular, the
//! caller must supply the resulting authority epoch in a new proposal; this
//! workflow never edits the actor's frozen bytes to make a stale request pass.

#[path = "continuation/guarded.rs"]
pub(crate) mod guarded;
#[path = "continuation/creation.rs"]
pub(crate) mod creation;
#[path = "continuation/andon.rs"]
pub(crate) mod andon;
#[path = "continuation/checkpoint.rs"]
mod checkpoint;

use super::{Deadline, RunResult, cleanup, execute, plus, resume_existing, stop};
use crate::config::{CLOCK_DOMAIN, Config, debug};
use crate::peers::PeerProfile;
use crate::publication::PublicationProfile;
use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::FileDriverTopology;
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::actor_wire::{
    ActorWire, Command, decode_command, encode_command,
};
use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "continuation_tests.rs"]
mod tests;

/// Open only an existing store, then submit through the ORIGINAL actor gateway.
/// A stored key (successful, refused or interrupted) is never a new admission:
/// exact bytes retrieve/reconcile its result and different bytes conflict.
///
/// This is an operator-owned synchronous reference workflow, not a daemon or an
/// actor capability to open journals. The supplied bootstrap profile must still
/// match disk, and all existing source, helper, human-key and publication guards
/// remain mandatory. There is no create-on-missing or checked-to-legacy fallback.
pub fn submit_existing<F>(
    config: Config,
    document: &[u8],
    peers: Option<&PeerProfile>,
    publication: Option<&PublicationProfile>,
    time: F,
) -> Result<RunResult, String>
where
    F: FnMut() -> ElapsedTick,
{
    submit_with_credibility(config, document, peers, publication, None, time)
}

/// Optional OFFLINE evidence activation in the SAME recovered owner that will
/// supervise this new request. Historical actor requests remain query-only:
/// their retry never opens this evidence file or reapplies any qualification.
/// A supplied file is mandatory for a new request; no legacy fallback follows
/// a read, decode, activation, freshness or original proposal-binding refusal.
pub fn submit_with_credibility<F>(
    config: Config,
    document: &[u8],
    peers: Option<&PeerProfile>,
    publication: Option<&PublicationProfile>,
    credibility: Option<&Path>,
    mut time: F,
) -> Result<RunResult, String>
where
    F: FnMut() -> ElapsedTick,
{
    if let Some(profile) = peers {
        profile.check_host(&config)?;
    }
    let Command::Submit { request, proposal } = decode_command(document).map_err(debug)? else {
        return Err("expected an original submit document".into());
    };
    let start = time();
    let started = Instant::now();
    let runtime_deadline = plus(start, config.timing.runtime_ms)?;
    let (host, reviewer) = match publication {
        Some(profile) => profile.open(&config.store, config.profile.clone()),
        None => FileOversight::open(&config.store, config.profile.clone()),
    }
    .map_err(debug)?;

    from_recovered(config, host, reviewer, Continuation {
        document, request, proposal, peers, publication, credibility, runtime_deadline, started,
    }, time, |_| Ok(()))
}

// Both openers transfer the ORIGINAL owner and separate reviewer, never keys.
// The actor document and expected epoch/target are retained without rewriting.
struct Continuation<'a> {
    document: &'a [u8],
    request: u64,
    proposal: ActorProposal,
    peers: Option<&'a PeerProfile>,
    publication: Option<&'a PublicationProfile>,
    credibility: Option<&'a Path>,
    runtime_deadline: ElapsedTick,
    started: Instant,
}

// One implementation of fresh source -> original helper review -> human key ->
// publication. Retention runs exactly once while the acknowledged owner remains
// held, including on preparation/refusal errors, and BEFORE child cleanup.
fn from_recovered<F, R>(config: Config, host: FileOversight, reviewer: FileHumanReviewer,
    context: Continuation<'_>, time: F, retain: R) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick, R: FnOnce(&FileOversight) -> Result<(), String> {
    from_recovered_with_topology(config, host, reviewer, context, None, time, retain)
}

// Initial mediated creation transfers its original source custody, never a
// caller's certificate. Legacy/recovery callers keep their original None path.
fn from_recovered_with_topology<F, R>(config: Config, host: FileOversight,
    reviewer: FileHumanReviewer, context: Continuation<'_>, topology: Option<FileDriverTopology>,
    time: F, retain: R) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick, R: FnOnce(&FileOversight) -> Result<(), String> {
    from_recovered_with_topology_retaining(config, host, reviewer, context, topology,
        time, |host, _| retain(host))
}

// The same completion boundary can retain ACTUAL source expenditure alongside
// history before cleanup drops custody. No extra source read or state mutation.
fn from_recovered_with_topology_retaining<F, R>(mut config: Config, mut host: FileOversight,
    reviewer: FileHumanReviewer, context: Continuation<'_>, topology: Option<FileDriverTopology>,
    mut time: F, retain: R) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick, R: FnOnce(&FileOversight, Option<&FileDriverTopology>) -> Result<(), String> {
    let Continuation { document, request, proposal, peers, publication, credibility,
        runtime_deadline, started } = context;
    let prepared = (|| {
        if host.publication_validation_profile().map_err(debug)?
            != publication.map(|profile| profile.limits)
        {
            return Err("stored witness profile requires its matching explicit checked mode".into());
        }
        if host.file_source_status().map(|status| status.policy) != Some(config.source_policy)
            || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
            || !host.publication_guard_required()
            || config.profile.delivery.clock_domain != CLOCK_DOMAIN
        {
            return Err("stored deployment does not match this mandatory source/recovery profile".into());
        }
        // Only original Missing admits a new request. Existing keys remain
        // query-only even when their proposal deadline has already elapsed.
        let existing = match host.request_status(request) {
            Ok(_) => true,
            Err(JournalError::Contract(Error::Missing)) => false,
            Err(error) => return Err(debug(error)),
        };
        if topology.is_some() && existing {
            return Err("initial topology selection cannot recover an existing request".into());
        }
        let deadline = Deadline {
            logical: if existing { runtime_deadline } else { proposal.deadline.min(runtime_deadline) },
            started, wall: Duration::from_millis(config.timing.runtime_ms),
        };
        deadline.check(time())?;
        let admission = if existing { None } else {
            if let Some(path) = credibility {
                crate::qualification::activate(&mut host, &config, &proposal, path)?;
                deadline.check(time())?;
            }
            let observed = host.refresh_file_source(host.revision(), &mut config.source, time()).map_err(debug)?;
            host.observe_time(host.revision(), time()).map_err(debug)?;
            Some(observed.snapshot().clone())
        };
        Ok::<_, String>((existing, deadline, admission))
    })();
    let (existing, deadline, admission) = match prepared {
        Ok(value) => value,
        Err(error) => return Err(combine_failure(Some(error), retain(&host, topology.as_ref()).err()).expect("original failure")),
    };
    let (port, mut driver) = host.into_supervised_driver();
    let mut wire = ActorWire::new(port);
    let result = (|| {
        if let Some(topology) = topology {
            let revision = driver.supervisor().host().map_err(debug)?.revision();
            driver.require_topology(revision, topology).map_err(debug)?;
            deadline.check(time())?;
            let revision = driver.supervisor().host().map_err(debug)?.revision();
            let refreshed = driver.refresh_topology(revision).map_err(debug)?;
            if refreshed.refusal.is_some()
                || !matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_))))
            {
                return Err("initial topology was not certified by the native checker".into());
            }
            // A slow topology read must not freeze the earlier policy lease or
            // action deadline. The original gateway observes this later time.
            let now = time();
            deadline.check(now)?;
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            let revision = host.revision();
            host.observe_time(revision, now).map_err(debug)?;
        }
        if let Some(snapshot) = admission {
            let revision = driver.supervisor().host().map_err(debug)?.revision();
            driver.supervisor_mut().set_snapshot(revision, Some(snapshot)).map_err(debug)?;
        }
        let submitted = wire.exchange(document);
        if submitted.result.is_err() {
            return Ok((submitted, Some("original actor gateway refused submission".to_owned())));
        }
        let work = if existing { resume_existing(&mut driver, request, &mut time) }
            else { execute(&mut driver, &reviewer, &mut config, request, &deadline,
                (peers, publication), &mut time) };
        let failure = match work {
            Ok(()) => None,
            Err(error) => Some(match stop(&mut driver, request, &mut time) {
                Ok(()) => error,
                Err(stopping) => format!("{error}; stop/drain: {stopping}"),
            }),
        };
        let response = wire.exchange(&encode_command(&Command::Poll { request }).map_err(debug)?);
        Ok::<_, String>((response, failure))
    })();
    let retained = match driver.supervisor().host() {
        Ok(host) => retain(&host, driver.topology()),
        Err(error) => Err(debug(error)),
    };
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    match result {
        Ok((response, failure)) => Ok(RunResult {
            response, failure: combine_failure(failure, retained.err()), cleanup_pending,
        }),
        Err(error) => {
            let error = combine_failure(Some(error), retained.err()).expect("original failure");
            if cleanup_pending == 0 { Err(error) }
            else { Err(format!("{error}; {cleanup_pending} direct helper children lack reaping confirmation")) }
        }
    }
}

fn combine_failure(original: Option<String>, additional: Option<String>) -> Option<String> {
    match (original, additional) {
        (Some(original), Some(additional)) => Some(format!("{original}; {additional}")),
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}
