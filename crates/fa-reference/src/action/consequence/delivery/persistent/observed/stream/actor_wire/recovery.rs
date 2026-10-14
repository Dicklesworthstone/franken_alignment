//! Original stream intents through the same permanently non-admitting gate.
use super::{ActorError, ActorOutcome, ActorProposal, ActorRequestPort, FileActorTicket,
    FileOversight, FileStreamActorPort, FileStreamProposal, Knowledge, StreamProfile,
    backend, codec};
use crate::action::consequence::delivery::persistent::requests::actor::FileActorRecoveryPort;
use crate::Error;

/// Recover the original request from its saved MESSAGE INTENT, not a caller's
/// cumulative release frame. Missing keys never enter the builder or admission.
/// Poll/Cancel retain the original outcome and liability rules. Authentication
/// and independently matched owner recovery still belong to the supervisor.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::FileStreamActorRecoveryPort;
/// fn widen(port: FileStreamActorRecoveryPort) { port.into_admitting(); }
/// ```
#[derive(Clone, Debug)]
pub struct FileStreamActorRecoveryPort {
    port: FileActorRecoveryPort<FileOversight>,
    profile: StreamProfile,
}
impl FileStreamActorPort {
    /// Narrow this already provisioned stream capability without changing the
    /// host or snapshot. Copies of an independently retained admitting port are
    /// not revoked. The returned port cannot recover a missing key by creating it.
    pub fn recovery_only(self) -> FileStreamActorRecoveryPort {
        FileStreamActorRecoveryPort { port: self.port.recovery_only(), profile: self.profile }
    }
}
impl FileStreamActorRecoveryPort {
    pub fn profile(&self) -> StreamProfile { self.profile }
    pub fn selected_request(&self) -> Option<u64> { self.port.selected_request() }
    /// Selection is monotone: retain this key or refuse, never switch or widen.
    pub fn for_request(mut self, request: u64) -> Result<Self, Error> {
        self.port = self.port.for_request(request)?;
        Ok(self)
    }
}
impl backend::Sealed for FileStreamActorRecoveryPort {}
impl ActorRequestPort for FileStreamActorRecoveryPort {
    type Ticket = FileActorTicket<FileOversight>;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        // Reuse the exact original decoder, including stream identity, framing,
        // UTF-8 and transport-unit checks. It observes no current stream prefix.
        let message = codec::message(self.profile, proposal)?;
        let intent = FileStreamProposal { target: proposal.target,
            expected_policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline,
            message: message.map(str::to_owned) };
        // This checks the actual immutable request book BEFORE the original
        // stream builder. Existing refusals and old frames remain recoverable
        // after prefix advancement, finish, fencing or expiry without new input.
        self.port.submit_stream(request, &intent)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> { self.port.poll(ticket) }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> { self.port.cancel(ticket) }
}
