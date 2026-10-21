//! Offline guarded source-publication recovery using the original sealed cursor.
//! The profile supplies independent floors/custody; CONFIG supplies exact policy
//! and source identity. No current evidence, helper or reviewer is opened.
mod profile;
mod recovery;
pub(super) mod phases;
pub(crate) use profile::{CreationOutput, Custody, Profile, ResumeProfile, SubmitProfile};
use super::{Config, debug, report};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve,
    observed::{FileOversight, guarded::{FileGuardSet, FileRecoveryRequirements,
        recovery::FileGuardedRecoveryStatus}}};
use std::{io::Write, time::{Duration, Instant}};

pub(crate) fn run<F>(config: &Config, profile: &Profile, operation: u64,
    clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let started = Instant::now();
    run_with_elapsed(config, profile, operation, clock, &mut || started.elapsed(), out)
}

// Deterministic elapsed seam; production always uses one monotonic Instant.
// It never renews the retained effect deadline or exposes the candidate machine.
pub(crate) fn run_with_elapsed<F>(config: &Config, profile: &Profile, operation: u64,
    clock: F, elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall { Err("guarded stop recovery replay deadline reached".to_owned()) }
        else { Ok(()) }
    };
    let (mut recovery, custody) = prepare_recovery(config, profile, &mut checkpoint)?;
    loop {
        // Ready has its own final checkpoint. A timeout after the last original
        // event must still drop custody without any stop, fence or staging cleanup.
        checkpoint()?;
        let current = recovery.progress();
        if current.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(current.replayed_events, 1).map_err(unconfirmed)?;
    }
    let request = recovery.stop_request(operation).map_err(unconfirmed)?;
    // Replay completion is not permission to use a clock result obtained after
    // the original deadline. Both checks stay inside native time acquisition,
    // so a completed receipt still skips the clock and these extra checkpoints.
    let mut timed_out = false;
    let history = recovery.finish_stopped_with_anchor(request, || {
        if checkpoint().is_err() { timed_out = true; return Err(JournalError::Unavailable); }
        let tick = clock();
        if checkpoint().is_err() { timed_out = true; return Err(JournalError::Unavailable); }
        Ok(tick)
    });
    if timed_out {
        return Err("guarded stop recovery acquisition deadline reached; no native completion was acknowledged; no effect was resent".into());
    }
    let history = history.map_err(unconfirmed)?;
    // Native acknowledgment comes first. Export only this exact captured cut,
    // never a reopened owner or a possibly newer canonical image.
    let retained = custody.retain(history.history_anchor()).map_err(|error| format!(
        "native stop acknowledged at revision {}, but history anchor retention failed: {error}; keep the previous archive and select a new output path; no effect was resent",
        history.stopped().snapshot().revision));
    let reported = report(history.stopped(), out);
    match (reported, retained) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(report), Err(retention)) => Err(format!("{report}; {retention}")),
    }
}

// Both combined recovery and the independently selected phases retain this
// exact custody, guard inventory, policy, topology and reserve admission.
fn prepare_recovery(config: &Config, profile: &Profile,
    checkpoint: &mut impl FnMut() -> Result<(), String>)
    -> Result<(recovery::Recovery, Custody), String>
{
    checkpoint()?;
    let custody = profile.admit(config)?;
    checkpoint()?;
    let topology = profile.admit_topology(config, checkpoint)?;
    // Both versions retain the exact source-publication guard inventory. Only
    // v2 adds independently supplied initial/current topology and availability;
    // no optional prediction, evaluation or numerical profile is inferred.
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: profile.minimum(),
    };
    let recovery = recovery::Recovery::begin(config, expected, topology, custody.anchor())
        .map_err(unconfirmed)?;
    Ok((recovery, custody))
}

fn unconfirmed(error: JournalError) -> String {
    format!("guarded native stop completion is unconfirmed: {}; no effect was resent", debug(error))
}

#[cfg(test)]
#[path = "guarded/mediated_tests.rs"]
mod mediated_tests;
