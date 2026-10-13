//! Original learned replay, complete guard inventory and receipt-only settlement.
mod cooperative;
mod selected;
use cooperative::{Progress, finish_replay};
use super::{Config, Loaded, FileOversight, FileSupervisedDriver, FileHumanReviewer, ElapsedTick,
    RunResult, ActorWire, Command, FileLearnedTextStreamActorPort, LearnedTextRelease,
    RecoveryReserve, Control, Deadline, Duration, Instant, cleanup, debug, encode_command};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::shutdown::recovery::{FileStoppedHistory, FileStoppedRecovery}};

type Custody = (FileOversight, FileHumanReviewer, Option<FileConsistencyObserver>);

pub(super) fn create_selected(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    if loaded.generation.required_pre_output_forecast().is_some()
        || loaded.generation.required_owned_pre_output_forecast().is_some() {
        // The first original stream image pins the predictor AND terminal
        // reserve. Neither an after-creation enable nor a naked role getter can
        // reconstruct this separate observer custody.
        let (host, human, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
            &config.store, config.profile.clone(), loaded.generation.clone(),
            RecoveryReserve::terminal()).map_err(debug)?;
        return Ok((host, human, Some(observer)));
    }
    let (host, human) = create(config, loaded)?;
    Ok((host, human, None))
}

pub(super) fn create(config: &Config, loaded: &Loaded)
    -> Result<(FileOversight, FileHumanReviewer), String>
{
    // The FIRST original image includes stream, numerical recipe, mandatory
    // learned provenance, policy-only source and terminal recovery reserve.
    // Reserve installation must precede the original learned Enable event.
    FileOversight::create_with_learned_text_stream_with_reserve(
        &config.store, config.profile.clone(), loaded.generation.clone(),
        RecoveryReserve::terminal()).map_err(debug)
}

#[cfg(test)]
pub(super) fn open(config: &Config, loaded: &Loaded)
    -> Result<(FileOversight, FileHumanReviewer), String>
{
    let (host, human, _) = open_selected_with_checkpoints(config, loaded, &mut |_| Ok(()))?;
    Ok((host, human))
}

fn requirements(config: &Config, loaded: &Loaded) -> FileRecoveryRequirements {
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(loaded.stream), decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None, minimum: loaded.floor,
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
pub(super) fn open_selected_live<F>(config: &Config, loaded: &Loaded,
    control: &mut Control, deadline: &Deadline, time: &mut F) -> Result<Custody, String>
where F: FnMut() -> ElapsedTick {
    open_selected_with_checkpoints(config, loaded, &mut |_| {
        deadline.check(time())?;
        if control.pending_peer()? {
            return Err("learned recovery interrupted by authenticated stop peer; no durable Stop applied or acknowledged".into());
        }
        Ok(())
    })
}

fn open_receipt(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    let started = Instant::now();
    open_receipt_with_elapsed(config, loaded, &mut || started.elapsed())
}

/// Receipt replay has its own finite wall budget, without consulting a new
/// logical clock, renewing the retained request deadline or reading evidence.
/// The elapsed supplier keeps every cancellation cut deterministic in tests;
/// production always uses the single Instant captured before guarded open.
pub(super) fn open_receipt_with_elapsed(config: &Config, loaded: &Loaded,
    elapsed: &mut impl FnMut() -> Duration) -> Result<Custody, String>
{
    let wall = Duration::from_millis(config.timing.runtime_ms);
    open_selected_with_checkpoints(config, loaded, &mut |_| {
        if elapsed() >= wall {
            return Err("learned receipt recovery replay deadline reached".into());
        }
        Ok(())
    })
}

/// Run the SAME guarded owner one original event at a time. None is the
/// pre-open boundary; Some contains historical progress only, never a role or
/// mutable candidate. Checkpoints may veto before any replay event and again
/// after complete verification, before cleanup and the original recovery fence.
/// An error or unwind drops the original exclusive recovery without finishing.
/// One event, framing and final persistence remain synchronous operations.
pub(super) fn open_selected_with_checkpoints(config: &Config, loaded: &Loaded,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>) -> Result<Custody, String>
{
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(recovery, checkpoint, |run| run.progress(),
        |run, done| run.advance(done), |run| run.finish())
}

/// Offline stop uses the same independently guarded replay and every wall-time
/// checkpoint, including Ready. It never constructs live service custody, reads
/// current helper/source evidence or consults the terminal clock during replay.
pub(super) fn stop_selected_with_elapsed<F>(config: &Config, loaded: &Loaded,
    operation: u64, clock: F, elapsed: &mut impl FnMut() -> Duration)
    -> Result<FileStoppedRecovery, String>
where F: FnOnce() -> ElapsedTick {
    stop_replay(config, loaded, operation, elapsed, |run| run.finish_stopped(operation, clock))
}

/// Capture the exact acknowledged terminal image on the SAME sealed cursor.
/// All original admission, one-event and Ready checkpoints are shared; only
/// the successful native return value additionally carries its private anchor.
pub(super) fn stop_selected_with_anchor_with_elapsed<F>(config: &Config, loaded: &Loaded,
    operation: u64, clock: F, elapsed: &mut impl FnMut() -> Duration)
    -> Result<FileStoppedHistory, String>
where F: FnOnce() -> ElapsedTick {
    stop_replay(config, loaded, operation, elapsed, |run| run.finish_stopped_with_anchor(operation, clock))
}

fn stop_replay<T>(config: &Config, loaded: &Loaded, operation: u64,
    elapsed: &mut impl FnMut() -> Duration,
    finish: impl FnOnce(selected::Cursor) -> Result<T, JournalError>) -> Result<T, String>
{
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = |_: Option<Progress>| {
        if elapsed() >= wall {
            Err("learned stop recovery replay deadline reached".into())
        } else { Ok(()) }
    };
    checkpoint(None)?;
    let recovery = selected::Cursor::begin(config, loaded)?;
    finish_replay(recovery, &mut checkpoint, |run| run.progress(),
        |run, done| run.advance(done), finish)
}

pub(super) fn resume<F>(config: Config, loaded: Loaded, mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let (host, _reviewer, _observer) = open_receipt(&config, &loaded)?;
    let action = host.request_action(loaded.request).map_err(debug)?;
    let frame = ReleaseFrame::decode(&action.spec().payload).map_err(debug)?;
    if frame.profile() != loaded.stream || frame.is_finish() {
        return Err("receipt recovery requires this original learned message request".into());
    }
    let deadline = action.spec().deadline;
    resume_original(config, loaded, host, LearnedTextRelease::Message, deadline, &mut time)
}

pub(super) fn resume_finish<F>(config: Config, loaded: Loaded, after: u64, mut time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    super::finish::check_ids(loaded.request, after)?;
    let (host, _reviewer, _observer) = open_receipt(&config, &loaded)?;
    // The original pre-policy request survives even if admission never created
    // a FrozenAction. Only the exact predecessor receipt can select this finish.
    let original = host.learned_text_stream_finish_request(after, loaded.request).map_err(debug)?;
    resume_original(config, loaded, host, LearnedTextRelease::Finish, original.deadline, &mut time)
}

fn resume_original<F>(config: Config, loaded: Loaded, host: FileOversight,
    release: LearnedTextRelease, deadline: ElapsedTick, time: &mut F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    // The ORIGINAL retained action supplies the old deadline. ttl_ms cannot
    // renew it, and neither new text nor today's target/epoch enters the retry.
    let proposal = FileLearnedTextStreamActorPort::encode_release(loaded.request,
        release, deadline).map_err(debug)?;
    let document = encode_command(&Command::Submit { request: loaded.request, proposal }).map_err(debug)?;
    let revision = host.revision();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let response = wire.exchange(&document);
    if response.result.is_err() || driver.supervisor().host().map_err(debug)?.revision() != revision {
        let failure = super::anchors::retain(&loaded, &driver,
            Some("original learned receipt ticket refused".into()));
        let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
        return Ok(RunResult { response, cleanup_pending, failure });
    }
    let failure = super::super::super::resume_existing(&mut driver, loaded.request, time).err();
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request }).map_err(debug)?);
    let failure = super::anchors::retain(&loaded, &driver, failure);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
