//! Submit a NEW request to an existing publication journal without resetting its
//! rights, policy, request identities or endpoint history. Recovery of a known
//! request remains query-only, including when its original deadline has elapsed.
//!
//! Opening uses the original owner's durable recovery fence. In particular, the
//! caller must supply the resulting authority epoch in a new proposal; this
//! workflow never edits the actor's frozen bytes to make a stale request pass.

use super::{Deadline, RunResult, cleanup, execute, plus, resume_existing, stop};
use crate::config::{CLOCK_DOMAIN, Config, debug};
use crate::peers::PeerProfile;
use crate::publication::PublicationProfile;
use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::oversight::actor_wire::{
    ActorWire, Command, decode_command, encode_command,
};
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
    mut config: Config,
    document: &[u8],
    peers: Option<&PeerProfile>,
    publication: Option<&PublicationProfile>,
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
    let (mut host, reviewer) = match publication {
        Some(profile) => profile.open(&config.store, config.profile.clone()),
        None => FileOversight::open(&config.store, config.profile.clone()),
    }
    .map_err(debug)?;

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

    // Only the owner's exact Missing result permits a new admission. Storage
    // faults, malformed journals and other failures must not become absence.
    let existing = match host.request_status(request) {
        Ok(_) => true,
        Err(JournalError::Contract(Error::Missing)) => false,
        Err(error) => return Err(debug(error)),
    };
    let deadline = Deadline {
        logical: if existing {
            runtime_deadline
        } else {
            proposal.deadline.min(runtime_deadline)
        },
        started,
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    deadline.check(time())?;
    let admission = if existing {
        // No evidence read, helper launch or human offer on an exact retry.
        None
    } else {
        let observed = host
            .refresh_file_source(host.revision(), &mut config.source, time())
            .map_err(debug)?;
        host.observe_time(host.revision(), time()).map_err(debug)?;
        Some(observed.snapshot().clone())
    };
    let (port, mut driver) = host.into_supervised_driver();
    let mut wire = ActorWire::new(port);
    if let Some(snapshot) = admission {
        let revision = driver.supervisor().host().map_err(debug)?.revision();
        driver
            .supervisor_mut()
            .set_snapshot(revision, Some(snapshot))
            .map_err(debug)?;
    }
    // In particular, do not replace expected_policy_epoch or target version.
    let submitted = wire.exchange(document);
    if submitted.result.is_err() {
        return Ok(RunResult {
            response: submitted,
            failure: Some("original actor gateway refused submission".into()),
            cleanup_pending: 0,
        });
    }
    let work = if existing {
        resume_existing(&mut driver, request, &mut time)
    } else {
        execute(
            &mut driver,
            &reviewer,
            &mut config,
            request,
            &deadline,
            (peers, publication),
            &mut time,
        )
    };
    let failure = match work {
        Ok(()) => None,
        Err(error) => Some(match stop(&mut driver, request, &mut time) {
            Ok(()) => error,
            Err(stopping) => format!("{error}; stop/drain: {stopping}"),
        }),
    };
    let response = wire.exchange(&encode_command(&Command::Poll { request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
