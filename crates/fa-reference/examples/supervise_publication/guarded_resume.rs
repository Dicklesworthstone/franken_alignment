//! Receipt-only operator recovery with independent history and original settlement.
//! No source read, helper/reviewer launch, new request admission or effect retry.
use crate::config::{CLOCK_DOMAIN, Config, debug};
use crate::recover_stop::guarded::ResumeProfile;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation,
    RecoveryReserve, observed::{FileOversight, guarded::{FileGuardSet, FileRecoveryRequirements,
        recovery::{FileGuardedRecoveryStatus, ReconciledGuardedOwner}}}};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, decode_command};
use std::{io::Write, time::{Duration, Instant}};

pub(crate) fn run<F>(config: &Config, profile: &ResumeProfile, document: &[u8],
    clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let started = Instant::now();
    run_with_elapsed(config, profile, document, clock, &mut || started.elapsed(), out)
}

pub(crate) fn run_with_elapsed<F>(config: &Config, profile: &ResumeProfile, document: &[u8],
    clock: F, elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let Command::Submit { request, .. } = decode_command(document).map_err(debug)? else {
        return Err("guarded resume requires an original submit document".into());
    };
    if config.profile.delivery.clock_domain != CLOCK_DOMAIN {
        return Err("guarded resume requires the configured Unix-millisecond clock domain".into());
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall { Err("guarded receipt recovery replay deadline reached".to_owned()) }
        else { Ok(()) }
    };
    checkpoint()?;
    let custody = profile.admit(config)?;
    checkpoint()?;
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
        checkpoint()?; // Ready is a distinct cancellation boundary.
        let current = recovery.progress();
        if current.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(current.replayed_events, 1).map_err(unconfirmed)?;
    }
    // Check existence on this SAME completely verified cursor before spending
    // any fence. Exact proposal equality is still the original actor gateway's
    // responsibility; knowing an ID cannot recover somebody else's ticket.
    let recovery = recovery.require_existing_request(request).map_err(unconfirmed)?;
    let ReconciledGuardedOwner { owner, roles: _, outcomes } =
        recovery.finish_reconciled(clock).map_err(unconfirmed)?;
    let revision = owner.revision();
    let mut failures = Vec::new();
    let unresolved: Vec<_> = outcomes.iter()
        .filter(|(_, result)| !matches!(result, Ok(Reconciliation::Resolved(_))))
        .map(|(attempt, result)| format!("{attempt}:{result:?}")).collect();
    // A fresh sweep may omit a previously irrecoverable delivery. Its retained
    // ledger state still matters; absence from the map must not imply settlement.
    let state = owner.inspect();
    let outstanding: Vec<_> = state.control.ledger.stages.iter()
        .filter(|(_, stage)| matches!(stage, ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown))
        .map(|(attempt, stage)| format!("{attempt}:{stage:?}")).collect();
    if !unresolved.is_empty() || !outstanding.is_empty() {
        failures.push(format!("unresolved prior deliveries: outcomes [{}], retained stages [{}]; charges remain in the original ledger",
            unresolved.join(", "), outstanding.join(", ")));
    }
    // Export before rendering, while the acknowledged owner is still held.
    // No later replay, disk snapshot or second authority owner supplies bytes.
    if custody.retention_requested() {
        let retained = owner.history_anchor_after(custody.anchor()).map_err(debug)
            .and_then(|anchor| custody.retain(&anchor));
        if let Err(error) = retained {
            failures.push(format!("history anchor retention failed: {error}; keep the previous archive and select a new output path"));
        }
    }
    let mut compatible = match owner.publication_validation_profile() {
        Ok(None) => true,
        Ok(Some(_)) => {
            failures.push("stored witness profile requires its matching explicit checked mode".into()); false
        }
        Err(error) => { failures.push(debug(error)); false }
    };
    if let Err(error) = owner.request_status(request) {
        failures.push(debug(error)); compatible = false;
    }
    if compatible {
        // Only an existing request can reach the original actor submit-retry
        // route. No snapshot is installed and no driver work/approval is run.
        let (port, driver) = owner.into_supervised_driver();
        let mut wire = ActorWire::new(port);
        let response = wire.exchange(document);
        if !matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })) {
            failures.push("the original actor result does not confirm execution; see the JSON response".into());
        }
        let encoded = response.encode();
        if let Err(error) = out.write_all(&encoded).and_then(|_| out.write_all(b"\n")).and_then(|_| out.flush()) {
            failures.push(format!("receipt output failed: {error:?}"));
        }
        // There are no child processes, sockets or review sessions to service.
        // Drop both the actor ticket and the original owner; no custody escapes.
        drop(wire); drop(driver);
    }
    if failures.is_empty() { Ok(()) }
    else { Err(acknowledged(revision, &failures.join("; "))) }
}

fn unconfirmed(error: JournalError) -> String {
    format!("guarded receipt recovery completion is unconfirmed: {}; no effect was resent", debug(error))
}
fn acknowledged(revision: u64, error: &str) -> String {
    format!("guarded receipt recovery acknowledged at revision {revision}; {error}; no effect was resent")
}

#[cfg(test)]
#[path = "guarded_resume/tests.rs"]
mod tests;
