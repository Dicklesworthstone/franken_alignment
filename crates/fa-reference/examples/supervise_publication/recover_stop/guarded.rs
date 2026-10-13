//! Offline guarded source-publication recovery using the original sealed cursor.
//! The profile supplies independent floors/custody; CONFIG supplies exact policy
//! and source identity. No current evidence, helper or reviewer is opened.
mod profile;
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
    checkpoint()?;
    let custody = profile.admit(config)?;
    checkpoint()?;
    // This version is EXACTLY the ordinary source-publication profile. None
    // requires absence under the original guard checker, never acceptance of
    // whatever optional guard happens to be stored in the recovered history.
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: profile.minimum(),
    };
    let mut recovery = FileOversight::begin_open_guarded_anchored(
        &config.store, config.profile.clone(), &expected, custody.anchor())
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(unconfirmed)?;
    loop {
        // Ready has its own final checkpoint. A timeout after the last original
        // event must still drop custody without any stop, fence or staging cleanup.
        checkpoint()?;
        let current = recovery.progress();
        if current.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(current.replayed_events, 1).map_err(unconfirmed)?;
    }
    let request = recovery.stop_request(operation).map_err(unconfirmed)?;
    let history = recovery.finish_stopped_with_anchor(request, clock).map_err(unconfirmed)?;
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

fn unconfirmed(error: JournalError) -> String {
    format!("guarded native stop completion is unconfirmed: {}; no effect was resent", debug(error))
}
