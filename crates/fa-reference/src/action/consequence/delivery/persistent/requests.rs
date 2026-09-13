//! Durable request identity over the original publication authority. Journal
//! replay recomputes admission, never imports a caller-asserted outcome or permit.

use super::{Event, FileDelivery, FrozenAction, JournalError, Machine, Transition};
use crate::action::{ActionSpec, ActionState};
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
        if let Some(record) = self.machine.requests.records.get(&request) {
            if record.spec != spec { return Err(Error::Binding.into()); }
            return Ok(record.status);
        }
        self.transact(revision, Event::SubmitRequest(request, spec, snapshot))?;
        self.request_status(request)
    }

    pub fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.requests.records.get(&request).ok_or(Error::Missing)?.status)
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

    pub fn retained_requests(&self) -> usize { self.machine.requests.records.len() }
    pub fn retained_request_bytes(&self) -> usize { self.machine.requests.bytes }
}

impl Machine {
    pub(super) fn apply_request(&mut self, request: u64, spec: &ActionSpec,
        snapshot: &Snapshot) -> Result<Transition, Error>
    {
        if request == 0 { return Err(Error::InvalidInput); }
        if self.requests.records.contains_key(&request) { return Err(Error::Duplicate); }
        if self.broker.inspect().suspended || self.broker.stop_receipt().is_some() {
            return Err(Error::WrongState);
        }
        if !spec.required_witnesses.is_empty() { return Err(Error::InvalidInput); }
        // Validate structure without pretending it is policy approval. A
        // structurally valid policy refusal is retained below, not re-rollable.
        FrozenAction::freeze(spec.clone())?;
        if spec.scope != self.broker.scope { return Err(Error::Binding); }
        if self.requests.records.len() >= MAX_FILE_REQUESTS { return Err(Error::Limit); }
        let bytes = self.requests.bytes.checked_add(spec.payload.len()).ok_or(Error::Limit)?;
        if bytes > MAX_FILE_REQUEST_BYTES { return Err(Error::Limit); }
        // Actor keys never select ledger IDs. Include refused allocations so
        // they are not recycled, and avoid operator-created original attempts.
        let inspection = self.broker.inspect();
        let maximum = inspection.ledger.stages.keys().copied()
            .chain(self.requests.records.values().map(|row| row.allocated_attempt)).max().unwrap_or(0);
        let attempt = maximum.checked_add(1).ok_or(Error::Overflow)?;
        let disposition = match self.broker.propose(attempt, spec.clone(), snapshot) {
            Ok(proposal) => {
                self.actions.insert(attempt, proposal.action);
                FileRequestDisposition::Admitted { attempt, stage: proposal.state }
            }
            Err(error) => FileRequestDisposition::NotAdmitted(error),
        };
        let generation = u64::from(!matches!(disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized, .. }));
        self.requests.records.insert(request, RequestRecord { spec: spec.clone(), allocated_attempt: attempt,
            status: FileRequestStatus { request, generation, disposition } });
        self.requests.bytes = bytes;
        Ok(Transition::Unit)
    }

    /// Fold each projection from the ORIGINAL ledger after every committed
    /// transition and during pure replay. No second outcome reducer is used.
    pub(super) fn refresh_requests(&mut self) -> Result<(), Error> {
        let inspection = self.broker.inspect();
        for record in self.requests.records.values_mut() {
            if let FileRequestDisposition::Admitted { attempt, stage } = record.status.disposition {
                let current = *inspection.ledger.stages.get(&attempt).ok_or(Error::Missing)?;
                if current != stage {
                    // Review/authorization changes remain one actor-visible
                    // Pending phase and disclose no congress activity count.
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
