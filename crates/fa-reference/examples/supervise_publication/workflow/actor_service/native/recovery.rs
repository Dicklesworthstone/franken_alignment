//! Explicit continuation of an ORIGINAL intent, or receipt-only recovery of an
//! ORIGINAL actor request. No restart, new intent, rebudgeting or resend fallback.
use super::*;

/// Validate the complete raw recipe against its original text intent. The exact
/// bootstrap opener already pinned stream, model/monitor/sampler and tokenizer.
/// A recorded actor key keeps its exact source, even after later numerical work;
/// an unsubmitted result must still describe the CURRENT numerical predecessor.
pub(super) fn select(host: &FileOversight, request: u64, inputs: &Inputs)
    -> Result<Option<FileTextMessageRequest>, String>
{
    let text = host.decoder_text_progress(inputs.generation).map_err(debug)?;
    if text.command().request() != &inputs.request {
        return Err("native recovery recipe differs from the original text intent".into());
    }
    match host.request_status(request) {
        Ok(_) => {
            let source = host.decoder_text_message_request(request).map_err(debug)?;
            if source.generation != inputs.generation
                || source.generation_revision != text.generation_revision() {
                return Err("recorded actor request belongs to another native generation".into());
            }
            return Ok(Some(source.clone()));
        }
        Err(JournalError::Contract(Error::Missing)) => {}
        Err(error) => return Err(debug(error)),
    }
    if host.pending_decoder_generation().map_err(debug)?
        .is_some_and(|pending| pending.id() != inputs.generation) {
        return Err("native recovery cannot skip another outstanding generation".into());
    }
    if text.is_complete() {
        if text.finish() != Some(Ok(GenerationFinish::StopToken)) {
            return Err("native recovery cannot restart a held, cancelled, failed or limited generation".into());
        }
        let receipt = text.numerical().receipt().ok_or("missing completed native receipt")?;
        let report = receipt.result().map_err(debug)?;
        let original = text.command();
        let steps = report.end_position().checked_sub(original.position())
            .ok_or("native result precedes its original intent")?;
        let actor_revision = original.actor_revision().checked_add(steps)
            .ok_or("native actor revision overflow")?;
        let numerical = host.decoder_inspection().map_err(debug)?.numerical;
        if report.start_position() != original.position() || numerical.position != report.end_position()
            || numerical.actor_revision != actor_revision {
            return Err("unsubmitted native result no longer describes the current numerical state".into());
        }
    } else if host.pending_decoder_text().map_err(debug)?.as_ref() != Some(text.command()) {
        return Err("native recovery requires the exact current pending text intent".into());
    }
    Ok(None)
}

/// Resume is an explicit consequence of serve-open, never a side effect of a
/// read or a failed create. Independent stop wins before source work and again
/// before original resume. Source/time admission and Ready status are enforced
/// by the original durable decoder; acknowledged work and RNG state survive it.
pub(super) fn resume<F>(driver: &mut FileSupervisedDriver, config: &mut Config, inputs: &Inputs,
    controls: (&mut Control, &FileHumanReviewer, &Deadline), time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, time)?;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let numerical = host.decoder_inspection().map_err(debug)?.numerical;
        let revision = host.revision();
        host.resume_decoder(revision, numerical.actor_revision, numerical.position).map_err(debug)?;
    }
    // Do not reconstruct a new command from the current decoder position: that
    // would lose the original prompt predecessor and cumulative work budget.
    advance_pending(driver, config, inputs, (control, reviewer, deadline), time)
}
