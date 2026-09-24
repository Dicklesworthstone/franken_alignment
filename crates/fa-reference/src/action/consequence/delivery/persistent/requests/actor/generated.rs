//! Source-only actor intake into the ORIGINAL durable gateway and request book.
//! The actor names retained generation data; only the supervisor owns inference.
mod wire;
use super::{ActorError, ActorOutcome, FileActorPort, FileActorSupervisor, FileActorTicket,
    JournalError, Knowledge, Snapshot, redact};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    driver::FileSupervisedDriver, stream::{FileStreamProposal, generated::FileTextMessageRequest}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::Error;

/// Weak, source-only access to an already bootstrapped native-text stream.
/// No arbitrary-message submit, raw-port getter, generation step, tokenizer,
/// private progress, clock, snapshot, reviewer or effect key is exposed. The
/// caller supplies identities, not output bytes or a claim that inference ended.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileGeneratedTextActorPort;
/// fn bypass(port: FileGeneratedTextActorPort) { port.host_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileGeneratedTextActorPort;
/// fn substitute(port: FileGeneratedTextActorPort) { port.submit_stream(1, "replacement"); }
/// ```
#[derive(Clone, Debug)]
pub struct FileGeneratedTextActorPort {
    port: FileActorPort<FileOversight>,
    profile: StreamProfile,
}

impl FileOversight {
    /// Consume the SAME locked owner, retaining its separately provisioned roles
    /// outside actor access. A legacy stream is refused, not upgraded. Creation
    /// and exclusive recovery must already have established the native-source
    /// requirement; this gateway never installs it or restores an old snapshot.
    pub fn into_generated_text_actor_gateway(self)
        -> Result<(FileGeneratedTextActorPort, FileActorSupervisor<Self>), JournalError>
    {
        if !self.generated_text_stream_required()? { return Err(Error::Binding.into()); }
        let profile = self.stream_snapshot()?.confirmed.profile();
        let (port, supervisor) = self.into_actor_gateway();
        Ok((FileGeneratedTextActorPort { port, profile }, supervisor))
    }

    /// Feed source-only actor requests to the existing full-input/two-key driver.
    /// This creates no new executor or phase machine. The separate human and
    /// other governance roles stay with their original independent custodians.
    pub fn into_generated_text_supervised_driver(self)
        -> Result<(FileGeneratedTextActorPort, FileSupervisedDriver), JournalError>
    {
        let (port, supervisor) = self.into_generated_text_actor_gateway()?;
        Ok((port, FileSupervisedDriver::new(supervisor)))
    }
}

impl FileGeneratedTextActorPort {
    /// Immutable bootstrap limits, not current publication or model state.
    pub fn profile(&self) -> StreamProfile { self.profile }

    /// Request the original complete native output without seeing private output
    /// through this port. Admission uses ONE supervisor-installed snapshot, and
    /// the original source/clock/revision, full-prompt and monitored-stop checks.
    /// An incomplete, cancelled or stale generation cannot become an effect.
    ///
    /// Exact retries need no new snapshot and do not consume one waiting for a
    /// different request. Conflicting source identities and ordinary request IDs
    /// refuse before taking it. Once a new admission is attempted its snapshot
    /// is consumed even on refusal; a failed request cannot reuse that observation.
    /// Recorded policy refusal still returns a ticket with the original redacted
    /// NotAdmitted outcome. No model, source reader or clock callback runs here.
    /// Existing journal replay can recompute historical tokens, not new inference.
    pub fn submit(&self, request: &FileTextMessageRequest)
        -> Result<FileActorTicket<FileOversight>, ActorError>
    {
        request.check().map_err(|error| redact(error.into()))?;
        let owner = self.port.owner.upgrade().ok_or(ActorError::Unavailable)?;
        let mut state = owner.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
        let retry = match state.host.decoder_text_message_request(request.request) {
            Ok(previous) => {
                if previous != request { return Err(ActorError::IdempotencyConflict); }
                true
            }
            Err(JournalError::Contract(Error::Missing)) => false,
            Err(_) => return Err(ActorError::Unavailable),
        };
        let snapshot = if retry { Snapshot::default() } else {
            match state.host.request_status(request.request) {
                Ok(_) => return Err(ActorError::IdempotencyConflict),
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(_) => return Err(ActorError::Unavailable),
            }
            state.snapshot.take().ok_or(ActorError::Unavailable)?
        };
        let revision = state.host.revision();
        state.host.submit_decoder_text_message(revision, request.clone(), snapshot).map_err(redact)?;
        Ok(FileActorTicket { owner: self.port.owner.clone(), request: request.request })
    }

    /// Explicit non-message effect. The ORIGINAL stream builder binds every
    /// confirmed prior message; the actor cannot supply or replace that prefix.
    /// This is still a normal proposal requiring congress and the human key.
    /// It neither cancels generation nor asserts that a pending output is complete.
    pub fn finish(&self, request: u64, target: ResolvedTarget, policy_epoch: u64,
        deadline: ElapsedTick) -> Result<FileActorTicket<FileOversight>, ActorError>
    {
        self.port.submit_stream(request, &FileStreamProposal { target,
            expected_policy_epoch: policy_epoch, deadline, message: None })
    }

    /// Original restricted request disposition, not native output or a review.
    /// An old/foreign ticket cannot be adopted just by knowing its numeric ID.
    pub fn poll(&self, ticket: &FileActorTicket<FileOversight>) -> Knowledge<ActorOutcome> {
        self.port.poll(ticket)
    }

    /// Cancel only the original undispatched effect reservation. After dispatch
    /// this remains the existing no-op: no inferred nonexecution, refund, replay
    /// of output, or cancellation of the source generation is introduced.
    pub fn cancel(&self, ticket: &FileActorTicket<FileOversight>) -> Result<(), ActorError> {
        self.port.cancel(ticket)
    }
}
