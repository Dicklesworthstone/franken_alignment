//! Durable request identity over the original publication authority. Journal
//! replay recomputes admission, never imports a caller-asserted outcome or permit.

pub mod actor;

use super::{Event, FileDelivery, FrozenAction, JournalError, Machine, Transition};
use crate::action::{ActionSpec, ActionState, Scope};
use crate::action::consequence::gate::ControlInspection;
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::{Error, Snapshot};
use std::collections::BTreeMap;

pub const MAX_FILE_REQUESTS: usize = 128;
pub const MAX_FILE_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// Supervisor-only disposition. The actor gateway publishes a restricted
/// Knowledge projection, not the internal attempt ID or admission diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRequestDisposition {
    NotAdmitted(Error),
    Admitted { attempt: u64, stage: ActionState },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRequestStatus {
    pub request: u64,
    /// Request-local projection revision, not the shared journal sequence.
    pub generation: u64,
    pub disposition: FileRequestDisposition,
}

struct RequestRecord {
    spec: ActionSpec,
    allocated_attempt: u64,
    status: FileRequestStatus,
}
#[derive(Default)]
pub(super) struct RequestBook {
    records: BTreeMap<u64, RequestRecord>,
    bytes: usize,
}

/// Private preflight data. It cannot admit an effect; only an ORIGINAL broker's
/// proposal result completes it. Both durable profiles use this same request book.
pub(super) struct PreparedRequest {
    request: u64,
    attempt: u64,
    spec: ActionSpec,
    bytes: usize,
}
impl PreparedRequest {
    pub(super) fn attempt(&self) -> u64 { self.attempt }
}
impl RequestBook {
    pub(super) fn len(&self) -> usize { self.records.len() }
    pub(super) fn bytes(&self) -> usize { self.bytes }
    pub(super) fn status(&self, request: u64) -> Result<FileRequestStatus, Error> {
        Ok(self.records.get(&request).ok_or(Error::Missing)?.status)
    }
    pub(super) fn retry(&self, request: u64, spec: &ActionSpec) -> Result<Option<FileRequestStatus>, Error> {
        match self.records.get(&request) {
            Some(record) if &record.spec == spec => Ok(Some(record.status)),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(super) fn prepare(&self, request: u64, spec: &ActionSpec, scope: Scope,
        inspection: &ControlInspection, stopping: bool) -> Result<PreparedRequest, Error>
    {
        if request == 0 { return Err(Error::InvalidInput); }
        if self.records.contains_key(&request) { return Err(Error::Duplicate); }
        if inspection.suspended || stopping { return Err(Error::WrongState); }
        if !spec.required_witnesses.is_empty() { return Err(Error::InvalidInput); }
        // Structure is not policy approval. Policy refusals are retained by
        // finish rather than retried under this key with a different snapshot.
        FrozenAction::freeze(spec.clone())?;
        if spec.scope != scope { return Err(Error::Binding); }
        if self.records.len() >= MAX_FILE_REQUESTS { return Err(Error::Limit); }
        let bytes = self.bytes.checked_add(spec.payload.len()).ok_or(Error::Limit)?;
        if bytes > MAX_FILE_REQUEST_BYTES { return Err(Error::Limit); }
        let maximum = inspection.ledger.stages.keys().copied()
            .chain(self.records.values().map(|row| row.allocated_attempt)).max().unwrap_or(0);
        let attempt = maximum.checked_add(1).ok_or(Error::Overflow)?;
        Ok(PreparedRequest { request, attempt, spec: spec.clone(), bytes })
    }

    /// No policy evaluator or outcome reducer lives here. The original proposal
    /// result supplies both the frozen action and actual initial ledger stage.
    pub(super) fn finish(&mut self, prepared: PreparedRequest, result: Result<Proposal, Error>)
        -> Option<(u64, FrozenAction)>
    {
        let PreparedRequest { request, attempt, spec, bytes } = prepared;
        let (disposition, action) = match result {
            Ok(proposal) => (FileRequestDisposition::Admitted { attempt, stage: proposal.state },
                Some((attempt, proposal.action))),
            Err(error) => (FileRequestDisposition::NotAdmitted(error), None),
        };
        let generation = u64::from(!matches!(disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized, .. }));
        self.records.insert(request, RequestRecord { spec, allocated_attempt: attempt,
            status: FileRequestStatus { request, generation, disposition } });
        self.bytes = bytes;
        action
    }

    /// A projection of original ledger stages, never a second outcome ledger.
    /// Private review/reservation transitions remain the same visible phase.
    pub(super) fn refresh(&mut self, inspection: &ControlInspection) -> Result<(), Error> {
        for record in self.records.values_mut() {
            if let FileRequestDisposition::Admitted { attempt, stage } = record.status.disposition {
                let current = *inspection.ledger.stages.get(&attempt).ok_or(Error::Missing)?;
                if current != stage {
                    if projection_class(current) != projection_class(stage) {
                        record.status.generation = record.status.generation.checked_add(1).ok_or(Error::Overflow)?;
                    }
                    record.status.disposition = FileRequestDisposition::Admitted { attempt, stage: current };
                }
            }
        }
        Ok(())
    }
}

impl FileDelivery {
    /// Exactly one original admission per key, including a recorded refusal.
    /// Exact retry returns its CURRENT durable disposition before checking the
    /// supplied predecessor, clock or snapshot; it performs no new admission.
    /// Changed action fields conflict. A storage fault never returns old state
    /// as a successful retry while a newer journal may already be visible.
    pub fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(status) = self.machine.requests.retry(request, &spec)? { return Ok(status); }
        self.transact(revision, Event::SubmitRequest(request, spec, snapshot))?;
        self.request_status(request)
    }

    pub fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.requests.status(request)?)
    }

    /// Frozen data for the trusted review/authorization owner. No key or
    /// sendable envelope is reconstructed by looking up this action.
    pub fn request_action(&self, request: u64) -> Result<&FrozenAction, JournalError> {
        let status = self.request_status(request)?;
        match status.disposition {
            FileRequestDisposition::NotAdmitted(_) => Err(Error::WrongState.into()),
            FileRequestDisposition::Admitted { attempt, .. } => {
                self.machine.actions.get(&attempt).ok_or_else(|| Error::Missing.into())
            }
        }
    }

    /// A cancellation request can release only an original undispatched
    /// reservation. Once sent, cancellation is an idempotent no-op: it does not
    /// assert nonexecution, release a charge, delete a key or create a resend.
    pub fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError> {
        let status = self.request_status(request)?;
        if let FileRequestDisposition::Admitted { attempt, stage } = status.disposition {
            if matches!(stage, ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized)
            { return self.cancel(revision, attempt); }
        }
        Ok(())
    }

    pub fn retained_requests(&self) -> usize { self.machine.requests.len() }
    pub fn retained_request_bytes(&self) -> usize { self.machine.requests.bytes() }
}

impl Machine {
    pub(super) fn apply_request(&mut self, request: u64, spec: &ActionSpec,
        snapshot: &Snapshot) -> Result<Transition, Error>
    {
        let prepared = self.requests.prepare(request, spec, self.broker.scope,
            &self.broker.inspect(), self.broker.stop_receipt().is_some())?;
        let result = self.broker.propose(prepared.attempt(), spec.clone(), snapshot);
        if let Some((attempt, action)) = self.requests.finish(prepared, result) {
            self.actions.insert(attempt, action);
        }
        Ok(Transition::Unit)
    }

    pub(super) fn refresh_requests(&mut self) -> Result<(), Error> {
        self.requests.refresh(&self.broker.inspect())
    }
}

fn projection_class(stage: ActionState) -> u8 {
    match stage {
        ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized => 0,
        ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => 1,
        ActionState::Confirmed => 2,
        ActionState::Denied => 3,
        ActionState::Cancelled => 4,
        ActionState::ConfirmedNotExecuted => 5,
    }
}
