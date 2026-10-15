//! Recover only ORIGINAL retained requests through the existing actor protocol.
//! No request allocation, evidence acquisition or alternative outcome reducer.
use super::{ActorError, ActorOutcome, ActorProposal, ActorRequestPort, Error,
    FileActorPort, FileActorTicket, FileDelivery, FileRequestHost, JournalError,
    Knowledge, backend, fmt};

/// A permanently non-admitting gateway. Submit means exact ticket recovery for
/// an existing record, never permission to create one. Poll and Cancel retain
/// their original laws; therefore this is NOT an entirely read-only capability.
/// The receiver must still authenticate its actor connection independently.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorRecoveryPort;
/// fn widen(port: FileActorRecoveryPort) { port.into_admitting(); }
/// ```
pub struct FileActorRecoveryPort<H: FileRequestHost = FileDelivery> {
    port: FileActorPort<H>,
    selected: Option<u64>,
}
impl<H: FileRequestHost> Clone for FileActorRecoveryPort<H> {
    fn clone(&self) -> Self { Self { port: self.port.clone(), selected: self.selected } }
}
impl<H: FileRequestHost> fmt::Debug for FileActorRecoveryPort<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileActorRecoveryPort").field("selected", &self.selected).finish_non_exhaustive()
    }
}
impl<H: FileRequestHost> FileActorPort<H> {
    /// Narrow this port without changing the host, discarding its admission
    /// snapshot, stopping the actor or claiming anything about endpoint outcomes.
    /// The result works with the ORIGINAL wire, channel, socket and peer session.
    /// Other independently retained ordinary ports are not revoked by narrowing.
    pub fn recovery_only(self) -> FileActorRecoveryPort<H> {
        FileActorRecoveryPort { port: self, selected: None }
    }
}
impl<H: FileRequestHost> FileActorRecoveryPort<H> {
    /// Further narrow to one externally selected request. Repeated selection can
    /// retain that same key but never widen or retarget the resulting capability.
    pub fn for_request(mut self, request: u64) -> Result<Self, Error> {
        if request == 0 { return Err(Error::InvalidInput); }
        if self.selected.is_some_and(|selected| selected != request) { return Err(Error::Binding); }
        self.selected = Some(request);
        Ok(self)
    }
    pub fn selected_request(&self) -> Option<u64> { self.selected }
    fn allowed(&self, request: u64) -> bool {
        request != 0 && self.selected.is_none_or(|selected| selected == request)
    }
    // Keep the original owner alive across validation and the native retry. The
    // sealed books never remove/reassign keys; no callback or yield intervenes.
    fn existing(&self, request: u64) -> Result<super::Rc<super::OwnerRef<H>>, ActorError> {
        if !self.allowed(request) { return Err(ActorError::Withheld); }
        let owner = self.port.owner.upgrade().ok_or(ActorError::Unavailable)?;
        {
            let state = owner.try_borrow().map_err(|_| ActorError::Unavailable)?;
            match state.host.request_status(request) {
                Ok(_) => {},
                // Do not manufacture an Absent/NotAdmitted receipt from failure
                // to find a record. No one-use intake snapshot is consumed here.
                Err(JournalError::Contract(Error::Missing)) => return Err(ActorError::Withheld),
                Err(_) => return Err(ActorError::Unavailable),
            }
        }
        Ok(owner)
    }

}
impl<H: FileRequestHost> backend::Sealed for FileActorRecoveryPort<H> {}
impl<H: FileRequestHost> ActorRequestPort for FileActorRecoveryPort<H> {
    type Ticket = FileActorTicket<H>;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        let _owner = self.existing(request)?;
        // These sealed hosts never delete/reassign a retained request. The same
        // non-Send gateway still owns this host; no callback or yield intervenes.
        // Use the ORIGINAL exact-byte retry (including retained admission refusal)
        // rather than constructing a ticket from its numeric ID or action alone.
        // It rechecks availability and cannot enter a new admission for this key.
        self.port.submit(request, proposal)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> {
        if !self.allowed(ticket.request()) {
            return Knowledge::Withheld { authority_required: "own_request" };
        }
        self.port.poll(ticket)
    }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> {
        if !self.allowed(ticket.request()) { return Err(ActorError::Withheld); }
        self.port.cancel(ticket)
    }
}

// One projection for live polling and independently verified historical replay.
// Callers still validate request identity and their canonical cut; a status is
// not a ticket, an endpoint proof, or permission to create/retry an effect.
impl super::FileRequestStatus {
    pub(in crate::action::consequence::delivery::persistent) fn actor_observation(self)
        -> Knowledge<ActorOutcome>
    {
        super::projection(self)
    }
}

impl FileActorRecoveryPort<super::super::super::observed::FileOversight> {
    /// The original intent-to-retained-frame path, but only after the same
    /// non-admitting gate used for raw requests. Never rebuild from a new prefix.
    pub(in crate::action::consequence::delivery::persistent) fn submit_stream(
        &self, request: u64,
        intent: &super::super::super::observed::stream::FileStreamProposal,
    ) -> Result<FileActorTicket<super::super::super::observed::FileOversight>, ActorError> {
        let _owner = self.existing(request)?;
        self.port.submit_stream(request, intent)
    }
}
