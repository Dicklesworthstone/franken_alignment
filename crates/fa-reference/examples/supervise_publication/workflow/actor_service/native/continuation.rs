//! Explicit new-message initiation, never a missing-intent recovery fallback.
use super::*;

const FLAG: &str = "--after-message";

pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<u64>), String> {
    let Some(index) = args.iter().position(|arg| arg == FLAG) else { return Ok((args, None)); };
    if !matches!(args.first().map(String::as_str), Some("serve-open" | "serve-open-checked"))
        || index + 2 != args.len() {
        return Err("--after-message requires a native open command and one final request ID".into());
    }
    let value = &args[index + 1];
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("--after-message requires a nonzero decimal request ID".into());
    }
    let id = value.parse::<u64>().map_err(|_| "previous request ID exceeds u64")?;
    if id == 0 { return Err("previous request ID cannot be zero".into()); }
    Ok((&args[..index], Some(id)))
}

pub(super) fn prepare(host: &FileOversight, request: u64, inputs: &Inputs, after: u64)
    -> Result<(u64, FileTextGenerationCommand), String>
{
    if request == after { return Err("continuation requires a new actor request ID".into()); }
    match host.request_status(request) {
        Ok(_) => return Err("actor request already exists; recover without --after-message".into()),
        Err(JournalError::Contract(Error::Missing)) => {},
        Err(error) => return Err(debug(error)),
    }
    let command = host.prepare_decoder_text_continuation(after, inputs.generation,
        inputs.request.clone()).map_err(debug)?;
    Ok((after, command))
}

/// Original qualification has already run. Independent stop and fresh evidence
/// still precede resume and the rechecked text-intent transaction. Starting the
/// next message cannot reset the model, skip outstanding work or publish output.
pub(super) fn start<F>(driver: &mut FileSupervisedDriver, config: &mut Config, inputs: &Inputs,
    prepared: (u64, FileTextGenerationCommand), controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let (after, command) = prepared;
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, time)?;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let n = host.decoder_inspection().map_err(debug)?.numerical;
        let revision = host.revision();
        host.resume_decoder(revision, n.actor_revision, n.position).map_err(debug)?;
        let revision = host.revision();
        host.begin_decoder_text_continuation(revision, after, command).map_err(debug)?;
    }
    // The same loop as first generation and recovery, including every source,
    // clock, monitor and independent-stop boundary and original budget reducer.
    advance_pending(driver, config, inputs, (control, reviewer, deadline), time)
}

#[cfg(test)]
mod tests;
