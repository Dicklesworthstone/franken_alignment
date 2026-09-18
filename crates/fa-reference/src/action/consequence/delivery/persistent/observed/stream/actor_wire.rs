//! Complete-message intents over the ORIGINAL actor wire and Unix transports.
//! The trusted connection selects this profile; ordinary raw-submit ports keep
//! their existing semantics. Intent bytes are not an execution-bearing frame.
mod codec;
mod source;
pub mod process;
#[cfg(target_os = "linux")]
pub use source::{FileStreamPeerDrive, FileStreamPeerDriveError};
pub use codec::{STREAM_INTENT_HEADER_BYTES, encode_stream_proposal};

use super::{FileOversight, FileStreamProposal, JournalError, StreamProfile};
use super::super::super::requests::actor::{FileActorPort, FileActorSupervisor, FileActorTicket};
use crate::action::consequence::oversight::actor::{ActorError, ActorOutcome, ActorProposal, Knowledge};
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, backend};

/// Weak actor-only access. There is no constructor from an arbitrary backend,
/// raw-port getter, mutable host, current-prefix view, evidence or effect key.
/// Use the unchanged ActorWire/ActorChannel/UnixActorConnection/PeerSession with
/// this port; only complete message intents can reach its submit operation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::FileStreamActorPort;
/// fn bypass(port: FileStreamActorPort) { port.host_mut(); }
/// ```
#[derive(Clone, Debug)]
pub struct FileStreamActorPort {
    pub(in crate::action::consequence::delivery::persistent) port: FileActorPort<FileOversight>,
    profile: StreamProfile,
}

impl FileOversight {
    /// Consume this actual stream owner; keep its separately provisioned roles
    /// out of the returned actor port. No admission observation is installed.
    /// Call after independently configured creation/recovery, never instead of
    /// it. A non-stream or unavailable owner is refused, not converted in place.
    pub fn into_stream_actor_gateway(self)
        -> Result<(FileStreamActorPort, FileActorSupervisor<Self>), JournalError>
    {
        let profile = self.stream_snapshot()?.confirmed.profile();
        let (port, supervisor) = self.into_actor_gateway();
        Ok((FileStreamActorPort { port, profile }, supervisor))
    }
}

impl FileStreamActorPort {
    /// Immutable receiver contract, not current actor or publication state.
    pub fn profile(&self) -> StreamProfile { self.profile }

    // Validate without allocation or source access, before the original trusted
    // intake hook can refresh a file or clock. Native retry checks still follow.
    fn validate_proposal(&self, proposal: &ActorProposal) -> Result<(), ActorError> {
        codec::message(self.profile, proposal).map(|_| ())
    }
}

impl backend::Sealed for FileStreamActorPort {}
impl ActorRequestPort for FileStreamActorPort {
    type Ticket = FileActorTicket<FileOversight>;

    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        let message = codec::message(self.profile, proposal)?;
        let intent = FileStreamProposal { target: proposal.target,
            expected_policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline,
            message: message.map(str::to_owned) };
        // The original builder retains every confirmed message/boundary, charges
        // the whole frame, and uses the recorded frame on exact retries. No wire
        // packet, claimed units or invented history replaces that native path.
        self.port.submit_stream(request, &intent)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> { self.port.poll(ticket) }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> { self.port.cancel(ticket) }
}
