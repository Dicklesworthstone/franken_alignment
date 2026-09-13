//! Durable external keys through the original FULL-INPUT two-key authority.
//! Sharing the request book does not substitute the simpler profile's reducer.
use super::{Machine, Transition};
use super::super::{FileOversight, BaseEvent, Event};
use super::super::super::JournalError;
use super::super::super::requests::{FileRequestDisposition, FileRequestStatus};
use crate::action::{ActionSpec, ActionState, FrozenAction};
use crate::{Error, Snapshot};

impl Machine {
    pub(super) fn apply_request(&mut self, request: u64, spec: &ActionSpec,
        snapshot: &Snapshot) -> Result<Transition, Error>
    {
        let prepared = self.requests.prepare(request, spec, self.scope,
            &self.broker.inspect(), self.broker.stop_receipt().is_some())?;
        let result = self.broker.propose(prepared.attempt(), spec.clone(), snapshot);
        if let Some((attempt, action)) = self.requests.finish(prepared, result) {
            self.actions.insert(attempt, action);
        }
        Ok(Transition::Unit)
    }
}

impl FileOversight {
    /// Persist exactly one original proposal/refusal per external key. A retry
    /// neither reconstructs review sessions nor recovers automatic/human keys.
    /// Snapshot, revision and current-time checks apply only to NEW admissions.
    pub fn submit_request(&mut self, revision: u64, request: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FileRequestStatus, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(status) = self.machine.requests.retry(request, &spec)? { return Ok(status); }
        self.transact(revision, Event::Core(BaseEvent::SubmitRequest(request, spec, snapshot)))?;
        self.request_status(request)
    }

    /// Supervisor data only; a transport must use the existing restricted actor
    /// projection rather than disclose internal IDs or admission diagnostics.
    pub fn request_status(&self, request: u64) -> Result<FileRequestStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.requests.status(request)?)
    }
    pub fn request_action(&self, request: u64) -> Result<&FrozenAction, JournalError> {
        match self.request_status(request)?.disposition {
            FileRequestDisposition::NotAdmitted(_) => Err(Error::WrongState.into()),
            FileRequestDisposition::Admitted { attempt, .. } => {
                self.machine.actions.get(&attempt).ok_or_else(|| Error::Missing.into())
            }
        }
    }
    pub fn cancel_request(&mut self, revision: u64, request: u64) -> Result<(), JournalError> {
        if let FileRequestDisposition::Admitted { attempt, stage } = self.request_status(request)?.disposition {
            if matches!(stage, ActionState::Proposed | ActionState::Prepared
                | ActionState::Reviewing | ActionState::Authorized)
            { return self.cancel(revision, attempt); }
        }
        Ok(())
    }
    pub fn retained_requests(&self) -> usize { self.machine.requests.len() }
    pub fn retained_request_bytes(&self) -> usize { self.machine.requests.bytes() }
}
