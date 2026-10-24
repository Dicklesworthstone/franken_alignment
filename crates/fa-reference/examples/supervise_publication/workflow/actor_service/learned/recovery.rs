//! Original learned replay, complete guard inventory and receipt-only settlement.
#[path = "recovery/cooperative.rs"]
mod cooperative;
#[path = "recovery/readonly.rs"]
pub(super) mod readonly;
#[path = "recovery/selected.rs"]
mod selected;
#[path = "recovery/terminal.rs"]
mod terminal;
use super::{
    ActorWire, Command, Config, Control, Deadline, Duration, ElapsedTick, FileHumanReviewer,
    FileLearnedTextStreamActorPort, FileOversight, FileSupervisedDriver, Instant,
    LearnedTextRelease, Loaded, RecoveryReserve, RunResult, cleanup, debug, encode_command,
};
use cooperative::{Progress, finish_replay};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
};
use fa_reference::action::consequence::delivery::persistent::observed::identity::FileIdentityObserver;
#[cfg(test)]
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStoppedRecovery;
use fa_reference::action::consequence::delivery::persistent::observed::stream::FileStreamProposal;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory},
};

pub(super) type Custody = (
    FileOversight,
    FileHumanReviewer,
    Option<FileConsistencyObserver>,
    Option<FileIdentityObserver>,
);
pub(super) type TopologyCustody = (Custody,
    Option<fa_reference::action::consequence::delivery::persistent::observed::mediation::FileMediationObserver>);

pub(super) fn create_selected(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    if loaded
        .generation
        .required_computed_identity_registration()
        .is_some()
    {
        return create_with_identity(config, loaded);
    }
    if loaded.generation.required_pre_output_forecast().is_some()
        || loaded
            .generation
            .required_owned_pre_output_forecast()
            .is_some()
    {
        // The first original stream image pins the predictor AND terminal
        // reserve. Neither an after-creation enable nor a naked role getter can
        // reconstruct this separate observer custody.
        let (host, human, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
            &config.store,
            config.profile.clone(),
            loaded.generation.clone(),
            RecoveryReserve::terminal(),
        )
        .map_err(debug)?;
        return Ok((host, human, Some(observer), None));
    }
    let (host, human) = create(config, loaded)?;
    Ok((host, human, None, None))
}

// Keep mutually exclusive native constructors out of the ordinary startup
// frame. Each path retains its original reserve, guards and separate roles.
fn create_with_identity(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    let guards = requirements(config, loaded).guards;
    let reserve = RecoveryReserve::terminal();
    let (host, roles) = if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
        FileOversight::create_predictive_guarded_with_learned_generation_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            prediction.clone(),
            None,
            loaded.generation.clone(),
            reserve,
        )
        .map_err(debug)?
    } else if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            prediction.clone(),
            None,
            loaded.generation.clone(),
            reserve,
        )
        .map_err(debug)?
    } else {
        let (host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            loaded.generation.clone(),
            reserve,
        )
        .map_err(debug)?;
        return Ok((host, roles.human, None, roles.identity_observer));
    };
    Ok((
        host,
        roles.oversight.human,
        Some(roles.consistency_observer),
        roles.oversight.identity_observer,
    ))
}

pub(super) fn create(
    config: &Config,
    loaded: &Loaded,
) -> Result<(FileOversight, FileHumanReviewer), String> {
    // The FIRST original image includes stream, numerical recipe, mandatory
    // learned provenance, policy-only source and terminal recovery reserve.
    // Reserve installation must precede the original learned Enable event.
    FileOversight::create_with_learned_text_stream_with_reserve(
        &config.store,
        config.profile.clone(),
        loaded.generation.clone(),
        RecoveryReserve::terminal(),
    )
    .map_err(debug)
}

#[cfg(test)]
pub(super) fn open(
    config: &Config,
    loaded: &Loaded,
) -> Result<(FileOversight, FileHumanReviewer), String> {
    let (host, human, _, _) = open_selected_with_checkpoints(config, loaded, &mut |_| Ok(()))?;
    Ok((host, human))
}

pub(super) fn requirements(config: &Config, loaded: &Loaded) -> FileRecoveryRequirements {
    FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(loaded.stream),
            decoder: None,
            decoder_stop: None,
            source: Some(config.source_policy),
            identity: loaded
                .generation
                .required_computed_identity_registration()
                .cloned(),
            campaigns: None,
            credential: None,
        },
        effective_policy: config.profile.delivery.policy.clone(),
        credential_epoch: None,
        minimum: loaded.floor,
    }
}

/// Each executable invocation independently reloads the exact predictor and
/// numerical recipe. Only the original predictive guard can issue a new role
/// after the same validated replay, terminal reserve and single recovery fence.
/// A previously unanswered forecast becomes lost coverage, never a new attempt.
#[cfg(test)]
pub(super) fn open_selected(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    open_selected_with_checkpoints(config, loaded, &mut |_| Ok(()))
}

/// Recovery shares the active command's original deadline and authenticated
/// stop transport. No owner exists yet: admission cancels replay, but cannot
/// apply the durable Stop protocol or acknowledge a Stop receipt.
pub(super) fn open_selected_live<F>(
    config: &Config,
    loaded: &Loaded,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
) -> Result<Custody, String>
where
    F: FnMut() -> ElapsedTick,
{
    open_selected_with_checkpoints(config, loaded, &mut |_| {
        deadline.check(time())?;
        if control.pending_peer()? {
            return Err("learned recovery interrupted by authenticated stop peer; no durable Stop applied or acknowledged".into());
        }
        Ok(())
    })
}

/// Preserve the separately owned topology observer through the SAME one-event
/// guarded replay. No source read or observer is issued before finalization.
pub(super) fn open_selected_live_with_topology<F>(
    config: &Config,
    loaded: &Loaded,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
) -> Result<TopologyCustody, String>
where
    F: FnMut() -> ElapsedTick,
{
    let mut checkpoint = |_: Option<Progress>| {
        deadline.check(time())?;
        if control.pending_peer()? {
            return Err("learned recovery interrupted by authenticated stop peer; no durable Stop applied or acknowledged".into());
        }
        Ok(())
    };
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(
        recovery,
        &mut checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done),
        |run| run.finish_with_topology(),
    )
}

fn open_receipt(
    config: &Config,
    loaded: &Loaded,
    after: Option<u64>,
) -> Result<(Custody, FileStreamProposal), String> {
    let started = Instant::now();
    open_selected_receipt_with_elapsed(config, loaded, after, &mut || started.elapsed())
}

/// The actual receipt path: select the original request AFTER complete guarded
/// replay and BEFORE the sole fence. No second read/replay or unchecked owner is
/// used. An invalid key, release kind or finish predecessor consumes no reserve.
pub(super) fn open_selected_receipt_with_elapsed(
    config: &Config,
    loaded: &Loaded,
    after: Option<u64>,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<(Custody, FileStreamProposal), String> {
    receipt_replay(config, loaded, elapsed, |run| {
        let original = run.receipt(loaded.request, after)?;
        Ok((run.finish()?, original))
    })
}

// Existing elapsed-budget tests deliberately recover histories before any
// request exists. Keep that base-finalizer test surface, never use it for --resume.
#[cfg(test)]
pub(super) fn open_receipt_with_elapsed(
    config: &Config,
    loaded: &Loaded,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<Custody, String> {
    receipt_replay(config, loaded, elapsed, |run| run.finish())
}

/// One finite wall budget, without a new logical clock, evidence read or renewed
/// request deadline. Every original event and Ready cut share the same timer.
fn receipt_replay<T>(
    config: &Config,
    loaded: &Loaded,
    elapsed: &mut impl FnMut() -> Duration,
    finish: impl FnOnce(selected::Cursor) -> Result<T, JournalError>,
) -> Result<T, String> {
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = |_: Option<Progress>| {
        if elapsed() >= wall {
            Err("learned receipt recovery replay deadline reached".into())
        } else {
            Ok(())
        }
    };
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(
        recovery,
        &mut checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done),
        finish,
    )
}

/// Run the SAME guarded owner one original event at a time. None is the
/// pre-open boundary; Some contains historical progress only, never a role or
/// mutable candidate. Checkpoints may veto before any replay event and again
/// after complete verification, before cleanup and the original recovery fence.
/// An error or unwind drops the original exclusive recovery without finishing.
/// One event, framing and final persistence remain synchronous operations.
pub(super) fn open_selected_with_checkpoints(
    config: &Config,
    loaded: &Loaded,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>,
) -> Result<Custody, String> {
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(
        recovery,
        checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done),
        |run| run.finish(),
    )
}

/// Offline stop uses the same independently guarded replay and every wall-time
/// checkpoint, including Ready. It never constructs live service custody, reads
/// current helper/source evidence or consults the terminal clock during replay.
#[cfg(test)]
pub(super) fn stop_selected_with_elapsed<F>(
    config: &Config,
    loaded: &Loaded,
    operation: u64,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<FileStoppedRecovery, String>
where
    F: FnOnce() -> ElapsedTick,
{
    stop_replay(config, loaded, operation, elapsed, |run| {
        run.finish_stopped(operation, clock)
    })
}

/// Capture the exact acknowledged terminal image on the SAME sealed cursor.
/// All original admission, one-event and Ready checkpoints are shared; only
/// the successful native return value additionally carries its private anchor.
#[cfg(test)]
pub(super) fn stop_selected_with_anchor_with_elapsed<F>(
    config: &Config,
    loaded: &Loaded,
    operation: u64,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<FileStoppedHistory, String>
where
    F: FnOnce() -> ElapsedTick,
{
    stop_replay(config, loaded, operation, elapsed, |run| {
        run.finish_stopped_with_anchor(operation, clock)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopPhase {
    Combined,
    Admission,
    Drain,
}

pub(super) enum TerminalHistory {
    Admission(FileAdmissionStoppedHistory),
    Completed(FileStoppedHistory),
}

pub(super) const STOP_ACQUISITION_DEADLINE: &str = "learned stop recovery acquisition deadline reached; no additional native completion was acknowledged";

/// All terminal modes consume the SAME complete guarded replay. Admission has
/// no logical-clock dependency. Drain cannot manufacture a missing admission;
/// combined completion stays one atomic native transaction, with no fallback.
pub(super) fn terminal_selected_with_elapsed<F>(
    config: &Config,
    loaded: &Loaded,
    phase: StopPhase,
    operation: u64,
    recover_checkpoint: bool,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<TerminalHistory, String>
where
    F: FnOnce() -> Result<ElapsedTick, JournalError>,
{
    if operation == 0 {
        return Err("stop operation must be nonzero".into());
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = |_: Option<Progress>| {
        if elapsed() >= wall {
            Err("learned stop recovery replay deadline reached".into())
        } else {
            Ok(())
        }
    };
    checkpoint(None)?;
    let run = terminal::Cursor::begin(config, loaded, operation, recover_checkpoint)?;
    let run = finish_replay(
        run,
        &mut checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done),
        Ok,
    )?;
    if phase == StopPhase::Admission {
        return run
            .finish_admission_stop()
            .map(TerminalHistory::Admission)
            .map_err(debug);
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut expired = false;
    let clock = || {
        if elapsed() >= wall {
            expired = true;
            return Err(JournalError::Unavailable);
        }
        let tick = clock()?;
        // A slow or stalled clock callback cannot extend the original runtime
        // allowance. Native finalizers have not published any phase yet.
        if elapsed() >= wall {
            expired = true;
            return Err(JournalError::Unavailable);
        }
        Ok(tick)
    };
    let history = match phase {
        StopPhase::Combined => run.try_finish_stopped_with_anchor(clock),
        StopPhase::Drain => run.finish_stopped_drain(clock),
        StopPhase::Admission => unreachable!("admission returned before clock construction"),
    };
    if expired {
        return Err(STOP_ACQUISITION_DEADLINE.into());
    }
    history.map(TerminalHistory::Completed).map_err(debug)
}

#[cfg(test)]
fn stop_replay<T>(
    config: &Config,
    loaded: &Loaded,
    operation: u64,
    elapsed: &mut impl FnMut() -> Duration,
    finish: impl FnOnce(selected::Cursor) -> Result<T, JournalError>,
) -> Result<T, String> {
    if operation == 0 {
        return Err("stop operation must be nonzero".into());
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = |_: Option<Progress>| {
        if elapsed() >= wall {
            Err("learned stop recovery replay deadline reached".into())
        } else {
            Ok(())
        }
    };
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(
        recovery,
        &mut checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done),
        finish,
    )
}

pub(super) fn resume<F>(config: Config, loaded: Loaded, mut time: F) -> Result<RunResult, String>
where
    F: FnMut() -> ElapsedTick,
{
    // Selection includes policy refusals with no FrozenAction, before fencing.
    let ((host, _reviewer, _observer, _identity), original) = open_receipt(&config, &loaded, None)?;
    resume_original(
        config,
        loaded,
        host,
        LearnedTextRelease::Message,
        original.deadline,
        &mut time,
    )
}

pub(super) fn resume_finish<F>(
    config: Config,
    loaded: Loaded,
    after: u64,
    mut time: F,
) -> Result<RunResult, String>
where
    F: FnMut() -> ElapsedTick,
{
    super::finish::check_ids(loaded.request, after)?;
    // A wrong predecessor cannot spend a fence or clean staged bytes.
    let ((host, _reviewer, _observer, _identity), original) =
        open_receipt(&config, &loaded, Some(after))?;
    resume_original(
        config,
        loaded,
        host,
        LearnedTextRelease::Finish,
        original.deadline,
        &mut time,
    )
}

fn resume_original<F>(
    config: Config,
    loaded: Loaded,
    host: FileOversight,
    release: LearnedTextRelease,
    deadline: ElapsedTick,
    time: &mut F,
) -> Result<RunResult, String>
where
    F: FnMut() -> ElapsedTick,
{
    // The ORIGINAL retained action supplies the old deadline. ttl_ms cannot
    // renew it, and neither new text nor today's target/epoch enters the retry.
    let proposal =
        FileLearnedTextStreamActorPort::encode_release(loaded.request, release, deadline)
            .map_err(debug)?;
    let document = encode_command(&Command::Submit {
        request: loaded.request,
        proposal,
    })
    .map_err(debug)?;
    let revision = host.revision();
    let (port, supervisor) = host
        .into_learned_text_stream_actor_gateway()
        .map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let response = wire.exchange(&document);
    if response.result.is_err() || driver.supervisor().host().map_err(debug)?.revision() != revision
    {
        let failure = super::anchors::retain(
            &loaded,
            &driver,
            Some("original learned receipt ticket refused".into()),
        );
        let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
        return Ok(RunResult {
            response,
            cleanup_pending,
            failure,
        });
    }
    let failure = super::super::super::resume_existing(&mut driver, loaded.request, time).err();
    let response = wire.exchange(
        &encode_command(&Command::Poll {
            request: loaded.request,
        })
        .map_err(debug)?,
    );
    let failure = super::anchors::retain(&loaded, &driver, failure);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult {
        response,
        failure,
        cleanup_pending,
    })
}
