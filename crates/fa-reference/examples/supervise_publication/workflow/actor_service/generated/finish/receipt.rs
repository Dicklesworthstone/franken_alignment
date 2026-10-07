//! Recover the native finish request, not a new finish against today's prefix.
use super::*;

pub(in super::super) fn resume<F>(config: Config, loaded: Loaded, request: u64,
    mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    if request == 0 || request == loaded.request {
        return Err("finish receipt needs a distinct nonzero request".into());
    }
    let (host, _reviewer) = recovery::open(&config, &loaded)?;
    check_source(&host, &loaded)?;
    // This original selector also accepts recorded refusal/cancellation. It
    // compares the full retained finish prefix and target to the named native
    // message's original execution receipt, not today's potentially newer head.
    let original = host.decoder_text_finish_request(loaded.request, request).map_err(debug)?;
    let proposal = FileGeneratedTextActorPort::encode_finish(request, original.target,
        original.expected_policy_epoch, original.deadline).map_err(debug)?;
    let document = encode_command(&Command::Submit { request, proposal }).map_err(debug)?;
    let revision = host.revision();
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    // A fresh wire needs its own ticket. The EXACT recorded retry supplies it
    // without an admission snapshot, current source, time or journal transition.
    // Never route this through the source-acquiring new-request entry point.
    let attached = wire.exchange(&document);
    if attached.result.is_err() || driver.supervisor().host().map_err(debug)?.revision() != revision {
        let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
        return Ok(RunResult { response: attached,
            failure: Some("original finish receipt ticket could not be restored read-only".into()), cleanup_pending });
    }
    // Query only. Unknown dispatch remains charged unless the original endpoint
    // evidence settles it. There is no human listener, key recovery or resend.
    let failure = super::super::super::super::resume_existing(&mut driver, request, &mut time).err();
    let response = wire.exchange(&encode_command(&Command::Poll { request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}
