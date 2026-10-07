//! Close a confirmed native stream through ORIGINAL actor intake and review.
//! This module contains no generation/advance/cancel call and no output override.
use super::*;
use fa_reference::action::consequence::oversight::actor::ActorProposal;

const FLAG: &str = "--finish-after-message";

#[derive(Clone, Copy)]
pub(super) enum Intent { Existing, Continue(u64), Finish(u64) }
impl Intent {
    pub(super) fn check(self, open: bool, request: u64) -> Result<(), String> {
        match self {
            Self::Continue(after) if !open || after == request =>
                Err("native continuation requires an existing stream and a new actor request ID".into()),
            Self::Finish(after) if !open || after == 0 || after == request =>
                Err("native finish requires an existing stream and a distinct preceding request".into()),
            _ => Ok(()),
        }
    }
}

pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<u64>), String> {
    let Some(index) = args.iter().position(|arg| arg == FLAG) else { return Ok((args, None)); };
    if !matches!(args.first().map(String::as_str), Some("serve-open" | "serve-open-checked"))
        || index + 2 != args.len() || args.iter().any(|arg| arg == "--after-message") {
        return Err("--finish-after-message requires native open, one final request ID and no continuation option".into());
    }
    let value = &args[index + 1];
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("finish predecessor must be a nonzero decimal request ID".into());
    }
    let id = value.parse::<u64>().map_err(|_| "finish predecessor exceeds u64")?;
    if id == 0 { return Err("finish predecessor cannot be zero".into()); }
    Ok((&args[..index], Some(id)))
}

pub(super) fn serve<F>(config: Config, actor: &Profile, reviewer: &PeerProfile, inputs: Inputs,
    selection: (Option<&PublicationProfile>, Option<&Path>, u64), time: F, output: &mut impl Write)
    -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    serve_lifecycle(config, actor, reviewer, inputs,
        (true, selection.0, selection.1, Intent::Finish(selection.2)), time, output)
}

/// The recipe is the preceding message's ORIGINAL intent, not a new prompt or a
/// generated EOS request. All complete model/tokenizer inputs were pinned by open.
/// Recorded finishes are classified first and never need new source/qualification.
pub(super) fn select(host: &FileOversight, request: u64, inputs: &Inputs, after: u64,
    deadline: ElapsedTick) -> Result<Option<ActorProposal>, String>
{
    let source = host.decoder_text_message_request(after).map_err(debug)?;
    let text = host.decoder_text_generation(source.generation).map_err(debug)?;
    if source.generation != inputs.generation || text.command().request() != &inputs.request {
        return Err("finish requires the preceding native message's original recipe".into());
    }
    match host.request_status(request) {
        Ok(_) => {
            let original = host.decoder_text_finish_request(after, request).map_err(debug)?;
            return FileGeneratedTextActorPort::encode_finish(request, original.target,
                original.expected_policy_epoch, original.deadline).map(Some).map_err(debug);
        }
        Err(JournalError::Contract(Error::Missing)) => {},
        Err(error) => return Err(debug(error)),
    }
    host.prepare_decoder_text_finish(after, request, deadline).map_err(debug)?;
    Ok(None)
}

/// Enable the existing effect review path without advancing numerical work.
/// Source acquisition and independent stop surround the original explicit resume;
/// then recheck the confirmed cut before emitting the fixed-size finish reference.
pub(super) fn prepare<F>(driver: &mut FileSupervisedDriver, config: &mut Config,
    request: u64, after: u64, controls: (&mut Control, &FileHumanReviewer, &Deadline), time: &mut F)
    -> Result<Option<ActorProposal>, String>
where F: FnMut() -> ElapsedTick {
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
    refresh(driver, config, time)?;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    stamp(&mut host, deadline, time)?;
    let n = host.decoder_inspection().map_err(debug)?.numerical;
    let revision = host.revision();
    host.resume_decoder(revision, n.actor_revision, n.position).map_err(debug)?;
    let intent = host.prepare_decoder_text_finish(after, request, deadline.logical).map_err(debug)?;
    // The full-context builder validates ordinary currentness too. The actor
    // gateway rebuilds and checks this same frame on authenticated admission.
    let spec = host.stream_finish_spec(intent.deadline).map_err(debug)?;
    if spec.target != Some(intent.target) || spec.policy_epoch != intent.expected_policy_epoch {
        return Err("native finish predecessor changed during preparation".into());
    }
    FileGeneratedTextActorPort::encode_finish(request, intent.target, intent.expected_policy_epoch,
        intent.deadline).map(Some).map_err(debug)
}

#[cfg(test)]
#[path = "finish/tests.rs"]
mod tests;
