//! Finite, permanently non-admitting access to selected retained requests.
use super::{
    ActorError, ActorOutcome, ActorProposal, ActorRequestPort, Error, FileActorRecoveryPort,
    FileActorTicket, FileDelivery, FileRequestHost, Knowledge, backend, fmt,
};

/// Maximum number of request identities in one recovery capability.
/// This bounds selection validation and storage independently of the request book.
pub const MAX_RECOVERY_REQUESTS: usize = 64;

/// A finite selection from one original non-admitting recovery port.
/// A selected missing request remains Withheld; selection never reserves a key.
/// Clones share the original weak owner and cannot keep its authority lock alive.
///
/// Submit still compares the complete original proposal, Poll still requires a
/// ticket from that owner, and Cancel still obeys the original liability rules.
/// No admitting port or authority-bearing host can be recovered from this value.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorRecoverySetPort;
/// fn widen(port: FileActorRecoverySetPort) { port.into_admitting(); }
/// ```
pub struct FileActorRecoverySetPort<H: FileRequestHost = FileDelivery> {
    port: FileActorRecoveryPort<H>,
    selected: Vec<u64>,
}
impl<H: FileRequestHost> Clone for FileActorRecoverySetPort<H> {
    fn clone(&self) -> Self {
        Self {
            port: self.port.clone(),
            selected: self.selected.clone(),
        }
    }
}
impl<H: FileRequestHost> fmt::Debug for FileActorRecoverySetPort<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileActorRecoverySetPort")
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}
impl<H: FileRequestHost> FileActorRecoveryPort<H> {
    /// Narrow to a nonempty finite set of externally selected request keys.
    /// A previously selected single key cannot be widened or retargeted.
    /// Empty, zero or duplicate keys are invalid; overlong lists exceed the
    /// selection limit. All checks precede allocation or access to the owner.
    pub fn for_requests(self, requests: &[u64]) -> Result<FileActorRecoverySetPort<H>, Error> {
        validate(requests)?;
        if requests.iter().any(|request| !self.allowed(*request)) {
            return Err(Error::Binding);
        }
        let mut selected = requests.to_vec();
        selected.sort_unstable();
        Ok(FileActorRecoverySetPort {
            port: self,
            selected,
        })
    }
}
impl<H: FileRequestHost> FileActorRecoverySetPort<H> {
    /// Selected identities in ascending order, independent of host availability.
    /// This is the capability's selection, not a list of retained request records.
    pub fn selected_requests(&self) -> &[u64] {
        &self.selected
    }

    /// Further narrow this selection. Omitted keys cannot be recovered later
    /// from this value, and no key outside the current selection can be added.
    pub fn for_requests(mut self, requests: &[u64]) -> Result<Self, Error> {
        validate(requests)?;
        if requests.iter().any(|request| !self.allowed(*request)) {
            return Err(Error::Binding);
        }
        let mut selected = requests.to_vec();
        selected.sort_unstable();
        self.selected = selected;
        Ok(self)
    }

    /// Further narrow to one of the already selected keys.
    pub fn for_request(self, request: u64) -> Result<Self, Error> {
        self.for_requests(&[request])
    }

    fn allowed(&self, request: u64) -> bool {
        self.selected.binary_search(&request).is_ok()
    }
}
impl<H: FileRequestHost> backend::Sealed for FileActorRecoverySetPort<H> {}
impl<H: FileRequestHost> ActorRequestPort for FileActorRecoverySetPort<H> {
    type Ticket = FileActorTicket<H>;

    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        if !self.allowed(request) {
            return Err(ActorError::Withheld);
        }
        self.port.submit(request, proposal)
    }

    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> {
        if !self.allowed(ticket.request()) {
            return Knowledge::Withheld {
                authority_required: "own_request",
            };
        }
        self.port.poll(ticket)
    }

    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> {
        if !self.allowed(ticket.request()) {
            return Err(ActorError::Withheld);
        }
        self.port.cancel(ticket)
    }
}

// Validate bounded borrowed input before allocating a selected-key collection.
// A quadratic duplicate check is at most 64 * 63 / 2 integer comparisons.
fn validate(requests: &[u64]) -> Result<(), Error> {
    if requests.len() > MAX_RECOVERY_REQUESTS {
        return Err(Error::Limit);
    }
    if requests.is_empty() {
        return Err(Error::InvalidInput);
    }
    for (index, request) in requests.iter().enumerate() {
        if *request == 0 || requests[..index].contains(request) {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
