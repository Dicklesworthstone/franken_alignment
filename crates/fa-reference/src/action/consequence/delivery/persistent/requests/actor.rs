//! Actor-only access to one surviving/reopened FileDelivery identity domain.
//! Only the supervisor owns the host or installs a fresh admission snapshot.

use super::{FileRequestDisposition, FileRequestStatus};
use super::super::{FileDelivery, JournalError, MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_ENTRIES};
use crate::action::{ActionSpec, ActionState, FrozenAction, Scope, VERSION, MAX_PAYLOAD_BYTES};
use crate::action::consequence::oversight::actor::{
    ActorBasis, ActorError, ActorOutcome, ActorProposal, BasisSource, Knowledge, UnknownReason,
};
use crate::action::consequence::oversight::actor_wire::{ActorRequestPort, backend};
use crate::{Error, Snapshot};
use std::cell::{Ref, RefCell, RefMut};
use std::fmt;
use std::rc::{Rc, Weak};

type OwnerRef = RefCell<Owner>;
struct Owner { host: FileDelivery, snapshot: Option<Snapshot> }

/// Observation/cancellation handle bound to this particular live gateway.
/// Recovery reacquires it by exact request retry, not by deserializing a permit.
#[derive(Clone)]
pub struct FileActorTicket { owner: Weak<OwnerRef>, request: u64 }
impl FileActorTicket { pub fn request(&self) -> u64 { self.request } }
impl fmt::Debug for FileActorTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileActorTicket").field("request", &self.request).finish_non_exhaustive()
    }
}

/// Weak ownership deliberately cannot keep the file-authority lock alive when
/// its supervisor is dropped. There is no clock, snapshot, review or send method.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
/// fn escape(port: FileActorPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileActorPort { owner: Weak<OwnerRef>, scope: Scope }
impl fmt::Debug for FileActorPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileActorPort { .. }") }
}

/// Trusted owner. It can drive the existing reference congress and the original
/// publication lifecycle; neither a port nor a wire session receives this role.
pub struct FileActorSupervisor { owner: Rc<OwnerRef> }
impl fmt::Debug for FileActorSupervisor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileActorSupervisor { .. }") }
}
impl FileDelivery {
    /// Consume this exact locked host. Opening an archive must already have
    /// completed its original recovery fence. No admission snapshot is restored.
    pub fn into_actor_gateway(self) -> (FileActorPort, FileActorSupervisor) {
        let scope = self.profile.scope;
        let owner = Rc::new(RefCell::new(Owner { host: self, snapshot: None }));
        (FileActorPort { owner: Rc::downgrade(&owner), scope }, FileActorSupervisor { owner })
    }
}
impl FileActorSupervisor {
    pub fn host(&self) -> Result<Ref<'_, FileDelivery>, JournalError> {
        let owner = self.owner.try_borrow().map_err(|_| JournalError::Unavailable)?;
        Ok(Ref::map(owner, |state| &state.host))
    }
    /// Privileged mutation invalidates an unused admission snapshot. This avoids
    /// reusing one across an arbitrary clock, policy or recovery host operation.
    pub fn host_mut(&mut self) -> Result<RefMut<'_, FileDelivery>, JournalError> {
        let mut owner = self.owner.try_borrow_mut().map_err(|_| JournalError::Unavailable)?;
        owner.snapshot = None;
        Ok(RefMut::map(owner, |state| &mut state.host))
    }
    /// One explicitly supplied observation for ONE new submission. It is never
    /// accepted from actor bytes, and exact retries/poll/cancel do not need it.
    /// A refused replacement withdraws the older observation instead of keeping
    /// stale evidence eligible. This is not proof of external-world freshness.
    pub fn set_snapshot(&mut self, expected_revision: u64, next: Option<Snapshot>) -> Result<(), JournalError> {
        let mut owner = self.owner.try_borrow_mut().map_err(|_| JournalError::Unavailable)?;
        owner.snapshot = None;
        if owner.host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if expected_revision != owner.host.revision() { return Err(Error::Stale.into()); }
        if let Some(snapshot) = &next {
            if !snapshot.complete { return Err(Error::Incomplete.into()); }
            if snapshot.values.len() > MAX_SNAPSHOT_ENTRIES { return Err(Error::Limit.into()); }
            let bytes = snapshot.values.values().try_fold(0_usize, |n, value| n.checked_add(value.len()).ok_or(Error::Limit))?;
            if bytes > MAX_SNAPSHOT_BYTES { return Err(Error::Limit.into()); }
        }
        owner.snapshot = next;
        Ok(())
    }
}

impl FileActorPort {
    pub fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<FileActorTicket, ActorError> {
        if request == 0 { return Err(ActorError::MalformedProposal); }
        if proposal.payload.len() > MAX_PAYLOAD_BYTES || proposal.payload.len() as u64 > proposal.units {
            return Err(ActorError::Capacity);
        }
        let spec = ActionSpec { version: VERSION, scope: self.scope, target: Some(proposal.target),
            payload: proposal.payload.clone(), required_witnesses: Vec::new(),
            policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline, units: proposal.units };
        FrozenAction::freeze(spec.clone()).map_err(|_| ActorError::MalformedProposal)?;
        let owner = self.owner.upgrade().ok_or(ActorError::Unavailable)?;
        let mut state = owner.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
        let snapshot = match state.host.request_status(request) {
            Ok(_) => Snapshot::default(), // exact binding is still checked by submit_request
            Err(JournalError::Contract(Error::Missing)) => state.snapshot.take().ok_or(ActorError::Unavailable)?,
            Err(_) => return Err(ActorError::Unavailable),
        };
        let revision = state.host.revision();
        state.host.submit_request(revision, request, spec, snapshot).map_err(redact)?;
        Ok(FileActorTicket { owner: self.owner.clone(), request })
    }
    pub fn poll(&self, ticket: &FileActorTicket) -> Knowledge<ActorOutcome> {
        if !Weak::ptr_eq(&self.owner, &ticket.owner) {
            return Knowledge::Withheld { authority_required: "own_request" };
        }
        let Some(owner) = self.owner.upgrade() else { return unavailable(); };
        let Ok(state) = owner.try_borrow() else { return unavailable(); };
        match state.host.request_status(ticket.request) {
            Ok(status) => projection(status), Err(_) => unavailable(),
        }
    }
    pub fn cancel(&self, ticket: &FileActorTicket) -> Result<(), ActorError> {
        if !Weak::ptr_eq(&self.owner, &ticket.owner) { return Err(ActorError::Withheld); }
        let owner = self.owner.upgrade().ok_or(ActorError::Unavailable)?;
        let mut state = owner.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
        let revision = state.host.revision();
        let result = state.host.cancel_request(revision, ticket.request);
        if state.host.revision() != revision || state.host.storage_failure().is_some() {
            state.snapshot = None;
        }
        result.map_err(redact)
    }
}
impl backend::Sealed for FileActorPort {}
impl ActorRequestPort for FileActorPort {
    type Ticket = FileActorTicket;
    fn submit(&self, key: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> { FileActorPort::submit(self, key, proposal) }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> { FileActorPort::poll(self, ticket) }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> { FileActorPort::cancel(self, ticket) }
}

fn unavailable() -> Knowledge<ActorOutcome> {
    Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
}
fn redact(error: JournalError) -> ActorError {
    match error {
        JournalError::Contract(Error::Binding) => ActorError::IdempotencyConflict,
        JournalError::Contract(Error::Limit | Error::Overflow) => ActorError::Capacity,
        JournalError::Contract(Error::InvalidInput) => ActorError::MalformedProposal,
        _ => ActorError::Unavailable,
    }
}
fn projection(status: FileRequestStatus) -> Knowledge<ActorOutcome> {
    let (value, source) = match status.disposition {
        FileRequestDisposition::NotAdmitted(_) => (ActorOutcome::NotAdmitted, BasisSource::Intake),
        FileRequestDisposition::Admitted { stage, .. } => {
            let value = match stage {
                ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized => {
                    return Knowledge::Pending { request: status.request };
                }
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => {
                    return Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown };
                }
                ActionState::Confirmed => ActorOutcome::Executed,
                ActionState::Denied => ActorOutcome::Denied,
                ActionState::Cancelled => ActorOutcome::CancelledBeforeDispatch,
                ActionState::ConfirmedNotExecuted => ActorOutcome::ConfirmedNotExecuted,
            };
            (value, BasisSource::ControlLedger)
        }
    };
    Knowledge::Known { value, basis: ActorBasis { request: status.request, generation: status.generation, source } }
}
