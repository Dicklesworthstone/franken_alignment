//! Explicit learned stream closure at an exact receipt-confirmed message cut.
//! No method here generates, advances, resets or replaces a numerical source.
use super::{Config, Loaded, FileOversight, FileSupervisedDriver, FileHumanReviewer,
    Control, Deadline, ElapsedTick, GenerationStatus, GenerationStop, JournalError,
    Error, debug, refresh, USAGE};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;

const FLAG: &str = "--finish-after-message";

pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<u64>), String> {
    let Some(index) = args.iter().position(|value| value == FLAG) else { return Ok((args, None)); };
    if args.len() != 6 || index != 4 || args[0] != "create-learned-generated"
        || args.iter().any(|value| value == "--continue-generation") {
        return Err(USAGE.into());
    }
    let text = &args[5];
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(USAGE.into());
    }
    let after = text.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if after == 0 { return Err(USAGE.into()); }
    Ok((&args[..4], Some(after)))
}

pub(super) fn check_ids(request: u64, after: u64) -> Result<(), String> {
    if request == 0 || after == 0 || request == after {
        return Err("learned finish requires a new request and its distinct original message request".into());
    }
    Ok(())
}

/// Read-only admission before live source acquisition, repeated after resume.
/// Publication ahead of its receipt is not an admissible finish predecessor.
pub(super) fn check_fresh(host: &FileOversight, loaded: &Loaded, after: u64) -> Result<(), String> {
    check_ids(loaded.request, after)?;
    match host.request_status(loaded.request) {
        Err(JournalError::Contract(Error::Missing)) => {}
        Ok(_) => return Err("learned finish already exists; use --resume with the same finish predecessor".into()),
        Err(error) => return Err(debug(error)),
    }
    let previous = host.request_action(after).map_err(debug)?;
    let frame = ReleaseFrame::decode(&previous.spec().payload).map_err(debug)?;
    let mut expected_target = previous.spec().target.ok_or("learned message has no original target")?;
    expected_target.expected_version = expected_target.expected_version.checked_add(1)
        .ok_or("learned message target version overflow")?;
    if host.request_resolution(after).map_err(debug)?
        != Some(EndpointOutcome::Executed { resulting_version: expected_target.expected_version }) {
        return Err("learned finish requires the original confirmed Executed message receipt".into());
    }
    let stream = host.stream_snapshot().map_err(debug)?;
    if frame.is_finish() || frame.profile() != loaded.stream
        || stream.confirmed.profile() != loaded.stream
        || stream.confirmed_target != expected_target
        || stream.publication.target != expected_target
        || stream.pending.is_some() || stream.confirmed != stream.published
        || stream.confirmed.finished()
        || !frame.prior_messages().iter().copied().chain(frame.message()).eq(stream.confirmed.messages()) {
        return Err("learned finish predecessor does not match the complete confirmed stream cut".into());
    }
    let current = host.learned_generation_inspection().map_err(debug)?;
    if !matches!(current.numerical.status, GenerationStatus::Finished(GenerationStop::StopToken(_)))
        || current.pending.is_some() || host.pending_learned_reset().map_err(debug)?.is_some()
        || stream.publication.stop.is_some() || stream.publication.control.suspended {
        return Err("learned finish requires completed original generation without pending work or Stop".into());
    }
    Ok(())
}

pub(super) fn prepare<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    config: &mut Config, loaded: &Loaded, after: u64, control: &mut Control,
    deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    check_fresh(&*driver.supervisor().host().map_err(debug)?, loaded, after)?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, deadline, time)?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    deadline.check(time())?;
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let current = host.learned_generation_inspection().map_err(debug)?;
    if current.paused {
        let revision = host.revision();
        host.resume_learned_generation(revision, current.numerical.actor_revision,
            current.numerical.position).map_err(debug)?;
    }
    check_fresh(&host, loaded, after)?;
    Ok(true)
}
