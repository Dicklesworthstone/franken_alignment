//! Receipt recovery for a source-linked native publication, never regeneration.
use super::{ActorWire, Command, Config, ElapsedTick, FileOversight, FileSupervisedDriver,
    Loaded, RecoveryReserve, RunResult, cleanup, debug, encode_command};
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer;

/// Pin the SAME immutable generated-stream/model/tokenizer/reserve bootstrap.
/// The original opener replays numerical witnesses and fences old sendable keys.
/// No plain-stream, unguarded, reserve-free or replacement-model fallback exists.
pub(super) fn open(config: &Config, loaded: &Loaded)
    -> Result<(FileOversight, FileHumanReviewer), String>
{
    loaded.check_host(config)?;
    let (host, reviewer) = FileOversight::open_generated_text_stream_with_reserve(
        &config.store, config.profile.clone(), loaded.stream, &loaded.decoder,
        &loaded.tokenizer, RecoveryReserve::terminal()).map_err(debug)?;
    if host.file_source_status().map(|status| status.policy) != Some(config.source_policy)
        || host.publication_validation_profile().map_err(debug)?.is_some()
        || !host.publication_guard_required()
        || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
    { return Err("stored native deployment does not match the explicit source/publication profile".into()); }
    Ok((host, reviewer))
}

/// Select only the historical source-linked request named by this exact recipe.
/// No submit frame is rebuilt against today's target, epoch, prefix or deadline.
/// In particular, ttl_ms cannot extend an old effect's lifetime during recovery.
pub(super) fn resume<F>(config: Config, loaded: Loaded, mut time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let (host, _reviewer) = open(&config, &loaded)?;
    let source = host.decoder_text_message_request(loaded.request).map_err(debug)?;
    if source.generation != loaded.generation {
        return Err("recorded native publication belongs to a different generation".into());
    }
    let generation = host.decoder_text_generation(source.generation).map_err(debug)?;
    if generation.command().request() != &loaded.text {
        return Err("recorded native generation does not match the exact text recipe".into());
    }
    // Validate source, result revision and disposition together. This getter
    // interprets acknowledged tokens; it neither computes nor releases new ones.
    host.decoder_text_message_snapshot(loaded.request).map_err(debug)?;
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    // Reuse the original query-only settlement: it supplies no evidence provider,
    // helper roster, reviewer transport, approval or sendable effect envelope.
    let failure = super::super::super::resume_existing(&mut driver, loaded.request, &mut time).err();
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
