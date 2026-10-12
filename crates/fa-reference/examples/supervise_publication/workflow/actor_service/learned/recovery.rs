//! Original learned replay, complete guard inventory and receipt-only settlement.
use super::{Config, Loaded, FileOversight, FileSupervisedDriver, FileHumanReviewer, ElapsedTick,
    RunResult, ActorWire, Command, FileLearnedTextStreamActorPort, LearnedTextRelease,
    RecoveryReserve, cleanup, debug, encode_command};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
    predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;

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

pub(super) fn open(config: &Config, loaded: &Loaded)
    -> Result<(FileOversight, FileHumanReviewer), String>
{
    let requirements = requirements(config, loaded);
    // Match complete independently loaded model/codec/probe/text/source bytes,
    // full guard inventory and explicit external floors BEFORE cleanup or fence.
    let mut recovery = FileOversight::begin_open_guarded_with_learned_generation(
        &config.store, config.profile.clone(), &requirements, &loaded.generation).map_err(debug)?
        .require_recovery_reserve(RecoveryReserve::terminal()).map_err(debug)?;
    let progress = recovery.progress();
    recovery.advance(progress.replayed_events, progress.total_events).map_err(debug)?;
    let (host, roles) = recovery.finish().map_err(debug)?;
    Ok((host, roles.human))
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
pub(super) fn open_selected(config: &Config, loaded: &Loaded) -> Result<Custody, String> {
    let (host, roles) = if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
        let expected = FilePredictiveRequirements { oversight: requirements(config, loaded),
            prediction: prediction.clone(), evaluation: None };
        let mut recovery = FileOversight::begin_open_predictive_guarded_with_learned_generation(
            &config.store, config.profile.clone(), &expected, &loaded.generation).map_err(debug)?
            .require_recovery_reserve(RecoveryReserve::terminal()).map_err(debug)?;
        let progress = recovery.progress();
        recovery.advance(progress.replayed_events, progress.total_events).map_err(debug)?;
        recovery.finish().map_err(debug)?
    } else if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        let expected = FileOwnedPredictiveRequirements { oversight: requirements(config, loaded),
            prediction: prediction.clone(), evaluation: None };
        let mut recovery = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            &config.store, config.profile.clone(), &expected, &loaded.generation).map_err(debug)?
            .require_recovery_reserve(RecoveryReserve::terminal()).map_err(debug)?;
        let progress = recovery.progress();
        recovery.advance(progress.replayed_events, progress.total_events).map_err(debug)?;
        recovery.finish().map_err(debug)?
    } else {
        let (host, human) = open(config, loaded)?;
        return Ok((host, human, None));
    };
    Ok((host, roles.oversight.human, Some(roles.consistency_observer)))
}

pub(super) fn resume<F>(config: Config, loaded: Loaded, mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let (host, _reviewer, _observer) = open_selected(&config, &loaded)?;
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
    let (host, _reviewer, _observer) = open_selected(&config, &loaded)?;
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
        let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
        return Ok(RunResult { response, cleanup_pending, failure: Some("original learned receipt ticket refused".into()) });
    }
    let failure = super::super::super::resume_existing(&mut driver, loaded.request, time).err();
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
