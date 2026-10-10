//! Original learned replay, complete guard inventory and receipt-only settlement.
use super::{Config, Loaded, FileOversight, FileSupervisedDriver, FileHumanReviewer, ElapsedTick,
    RunResult, ActorWire, Command, FileLearnedTextStreamActorPort, LearnedTextRelease,
    RecoveryReserve, cleanup, debug, encode_command};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;

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
    let requirements = FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(loaded.stream), decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None, minimum: loaded.floor,
    };
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

pub(super) fn resume<F>(config: Config, loaded: Loaded, mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let (host, _reviewer) = open(&config, &loaded)?;
    let action = host.request_action(loaded.request).map_err(debug)?;
    let frame = ReleaseFrame::decode(&action.spec().payload).map_err(debug)?;
    if frame.profile() != loaded.stream || frame.is_finish() {
        return Err("receipt recovery requires this original learned message request".into());
    }
    // The ORIGINAL retained action supplies the old deadline. ttl_ms cannot
    // renew it, and neither new text nor today's target/epoch enters the retry.
    let proposal = FileLearnedTextStreamActorPort::encode_release(loaded.request,
        LearnedTextRelease::Message, action.spec().deadline).map_err(debug)?;
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
    let failure = super::super::super::resume_existing(&mut driver, loaded.request, &mut time).err();
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
