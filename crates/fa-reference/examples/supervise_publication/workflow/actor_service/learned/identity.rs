//! Original cooperative model identity before learned generation or publication.
//! The separate observer and all admitted preparation work stay owned until
//! canonical completion/cancellation and the workflow's final cleanup.
#[path = "identity/profile.rs"] pub(super) mod profile;
use super::{Config, Loaded, Control, Deadline, ElapsedTick, FileHumanReviewer,
    FileSupervisedDriver, debug, refresh};
use crate::config::read_regular;
use fa_reference::action::consequence::activation::{
    identity::wire, tensor::kv::decoder::DecoderBudget,
};
use fa_reference::action::consequence::delivery::persistent::observed::identity::{
    FileIdentityObserver, FileLearnedIdentityInput,
    decoder::transaction::{ComputedIdentityStatus, FileComputedIdentityRun},
};
use fa_reference::action::consequence::oversight::identity::IdentityOutcome;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(super) struct Selection {
    pub check: u64,
    pub measurement_sequence: u64,
    pub budget: DecoderBudget,
    observed_manifest: PathBuf,
}

pub(super) struct Session {
    observer: Option<FileIdentityObserver>,
    history: Vec<FileComputedIdentityRun>,
}
impl Session {
    pub(super) fn new(observer: FileIdentityObserver) -> Self {
        Self { observer: Some(observer), history: Vec::new() }
    }
    #[cfg(test)]
    pub(super) fn history(&self) -> &[FileComputedIdentityRun] { &self.history }
}

/// Absence preserves the old workflow, including its clock/callback boundaries.
/// Historical commands never call this function or read an observed manifest.
pub(super) fn prepare<F>(driver: &mut FileSupervisedDriver, config: &mut Config,
    loaded: &Loaded, session: &mut Option<Session>,
    controls: (&mut Control, &FileHumanReviewer, &Deadline), time: &mut F)
    -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let Some(selected) = loaded.identity.as_ref() else {
        if session.is_some() { return Err("unexpected learned identity observer custody".into()); }
        return Ok(true);
    };
    let registration = loaded.generation.required_computed_identity_registration()
        .ok_or("missing exact learned identity registration")?;
    let session = session.as_mut().ok_or("missing learned identity observer custody")?;
    if session.observer.is_none() {
        return Err("learned identity preparation still owns its observer; exclusive recovery required".into());
    }
    if session.history.len() >= registration.policy.max_checks {
        return Err("learned identity session exhausted its original check allowance".into());
    }
    session.history.try_reserve(1).map_err(debug)?;
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    // This is independently observed data. Never fill it from the expected
    // passport; a wrong manifest must reach the original mismatch/containment.
    let observed_manifest = wire::decode_manifest(&read_regular(
        &selected.observed_manifest, wire::MANIFEST_BYTES)?).map_err(debug)?;
    refresh(driver, config, deadline, time)?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    let started = time(); deadline.check(started)?;
    let challenge = {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let revision = host.revision(); host.observe_time(revision, started).map_err(debug)?;
        let state = host.inspect();
        let actor = host.learned_generation_inspection().map_err(debug)?.numerical.actor_revision;
        let revision = host.revision();
        host.begin_identity_check(revision, selected.check, state.control.sequence, actor)
            .map_err(debug)?.map_err(debug)?
    };
    let prepared = {
        let host = driver.supervisor().host().map_err(debug)?;
        let observer = session.observer.take().expect("original observer checked before begin");
        observer.begin_computed_learned(&host, host.revision(), &challenge,
            FileLearnedIdentityInput { measurement_sequence: selected.measurement_sequence,
                budget: selected.budget, observed_manifest }, started)
    };
    let mut run = match prepared {
        Ok(run) => run,
        Err(refused) => {
            session.observer = Some(refused.observer);
            // Setup performs no inference, but Begin already withdrew identity.
            // Close that original pending basis before any usable fresh check.
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            let revision = host.revision();
            let cleanup = host.identity_unavailable(revision, challenge.evidence().basis())
                .map(|_| ()).map_err(debug);
            return Err(with_cleanup(debug(refused.error), cleanup));
        }
    };
    let result = drive(&mut run, driver, control, deadline, time, challenge.evidence().deadline());
    let cleanup = if run.status() == ComputedIdentityStatus::Committed { Ok(()) } else {
        match driver.supervisor_mut().host_mut() {
            Ok(mut host) => { let revision = run.revision(); run.cancel(&mut host, revision).map_err(debug) }
            Err(error) => Err(debug(error)),
        }
    };
    let cleanup = cleanup.and_then(|()| run.take_observer().map(|observer|
        session.observer = Some(observer)).map_err(debug));
    // Canonically completed work is already in the original computed record.
    // Interrupted work remains process-local, retained and reported even when
    // cancellation or durable storage fails; it is never refunded or reset.
    eprintln!("Computed identity check {}: {:?}, work {:?}, interrupted={}",
        selected.check, run.status(), run.work(), run.interrupted());
    let observed = run.result().cloned();
    session.history.push(run);
    let complete = match result {
        Ok(complete) => { cleanup?; complete }
        Err(error) => return Err(with_cleanup(error, cleanup)),
    };
    if !complete {
        // Pending peer admission did not mutate the pinned owner. Preparation
        // has now been cancelled and its unique observer returned, so the
        // original authenticated stop protocol may act on the original host.
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        return Err("learned identity cancelled for an authenticated stop peer".into());
    }
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    let observed = observed.ok_or("computed identity completed without an acknowledged observation")?;
    let report = match observed.observation.measurement {
        Ok(report) => report,
        Err(error) => {
            // A completed comparison can refuse (for example, a stale anchor
            // sequence) while leaving this original challenge pending. Keep
            // the acknowledged work above, and close only its current basis.
            let withdrawal = (|| {
                let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                let basis = host.identity_basis().map_err(debug)?;
                if basis != challenge.evidence().basis() { return Ok(()); }
                let revision = host.revision();
                host.identity_unavailable(revision, basis).map(|_| ()).map_err(debug)
            })();
            return Err(with_cleanup(debug(error), withdrawal));
        }
    };
    if report.outcome != IdentityOutcome::Matched {
        return Err(format!("computed learned identity refused: {:?}; containment {:?}",
            report.outcome, observed.observation.containment));
    }
    // Completion sealed its own receipt time. Installation is a separate
    // original boundary and must see current validity after any peer wait.
    // The cursor is committed and retained before this host mutation.
    let now = time(); deadline.check(now)?;
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let revision = host.revision(); host.observe_time(revision, now).map_err(debug)?;
    let state = host.inspect(); let revision = host.revision();
    host.apply_identity_check(revision, &challenge, state.control.sequence, state.control.ledger.epoch)
        .map_err(debug)?;
    Ok(true)
}

fn with_cleanup(error: String, cleanup: Result<(), String>) -> String {
    match cleanup { Ok(()) => error,
        Err(cleanup) => format!("{error}; original identity withdrawal failed: {cleanup}") }
}

/// Only clock reads and authenticated peer admission occur between tokens.
/// Even an unrelated host transaction would invalidate this native cursor.
fn drive<F>(run: &mut FileComputedIdentityRun, driver: &mut FileSupervisedDriver,
    control: &mut Control, deadline: &Deadline, time: &mut F, abort_at: ElapsedTick)
    -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    loop {
        deadline.check(time())?;
        if control.pending_peer()? { return Ok(false); }
        let mut clock_error = None;
        let clock = || {
            let now = time();
            if let Err(error) = deadline.check(now) {
                clock_error = Some(error); abort_at
            } else { now }
        };
        let revision = run.revision();
        let result = match run.status() {
            ComputedIdentityStatus::Computing => {
                let host = driver.supervisor().host().map_err(debug)?;
                run.advance_with_clock(&host, revision, clock).map(|_| ())
            }
            ComputedIdentityStatus::ReadyToCommit => {
                let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                run.commit_with_clock(&mut host, revision, clock).map(|_| ())
            }
            _ => return Err("unexpected native computed identity preparation state".into()),
        };
        if let Some(error) = clock_error { return Err(error); }
        result.map_err(debug)?;
        if run.status() == ComputedIdentityStatus::Committed { return Ok(true); }
    }
}
