//! Control-plane cleanup and query-only recovery using the same locked owner.
use super::{FileSupervisedDriver, Job, Phase, admitted, observe, stage};
use super::super::super::{FileStopSweep, JournalError, Reconciliation};
use super::super::super::requests::FileRequestStatus;
use crate::action::consequence::delivery::{StopReceipt, StopRequest};
use crate::action::{ActionState, ElapsedTick};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

impl FileSupervisedDriver {
    /// Resume only an ORIGINAL dispatched obligation, never its helper round,
    /// approvals or publication phase. Works with unavailable helper inputs and
    /// no reviewer role. The next step still needs an explicit fresh clock.
    pub fn resume_reconciliation(&mut self, request: u64) -> Result<(), JournalError> {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        let host = self.supervisor.host()?;
        let (attempt, state) = admitted(host.request_status(request)?)?;
        if !matches!(state, ActionState::Dispatching | ActionState::Unknown) { return Err(Error::WrongState.into()); }
        let action = host.request_action(request)?.clone();
        self.job = Some(Job { issuer: Rc::clone(&host.issuer), request, attempt, action, inputs: None, input_revision: 0,
            control_sequence: None, pool: None, permit: None, phase: Phase::Reconcile });
        Ok(())
    }

    /// Cancel only what the original authority can cancel. A sent obligation
    /// becomes query-only in this driver, not cancelled-before-dispatch and not
    /// refunded. No helper or source call is needed; exact request keys survive.
    pub fn cancel_active(&mut self) -> Result<FileRequestStatus, JournalError> {
        let request = self.job.as_ref().ok_or(Error::Missing)?.request;
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            self.job.as_ref().ok_or(Error::Missing)?.check_owner(&host)?;
            let revision = host.revision();
            host.cancel_request(revision, request)?;
            host.request_status(request)
        })();
        if let Ok(status) = &result {
            let (_, state) = admitted(*status)?;
            let job = self.job.as_mut().expect("retained active request");
            job.permit = None;
            if matches!(state, ActionState::Dispatching | ActionState::Unknown) { job.phase = Phase::Reconcile; }
            else { job.close(); }
        }
        self.reap_helpers();
        if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
        result
    }

    /// Existing obligations are independent of the currently active review.
    /// Return the original per-attempt results only after their durable commit.
    pub fn reconcile_pending(&mut self, now: ElapsedTick)
        -> Result<BTreeMap<u64, Result<Reconciliation, Error>>, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            observe(&mut host, now)?;
            let revision = host.revision();
            host.reconcile_pending(revision)
        })();
        self.reap_helpers();
        result
    }

    /// Stop through the original ledger/journal, not a local driver flag. A
    /// failed preflight preserves a healthy job; ambiguous storage closes its
    /// local drive path without asserting a committed stop, refund or drain.
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, JournalError> {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            let revision = host.revision();
            host.request_stop(revision, request)
        })();
        self.release_stopped_job();
        result
    }

    /// Advance the original endpoint barrier and terminal evidence. The original
    /// host owns time observation and the durable sweep together; no source or
    /// human permit is fetched, and an error returns no candidate refunds.
    pub fn progress_stop(&mut self, now: ElapsedTick) -> Result<FileStopSweep, JournalError> {
        self.release_stopped_job();
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            let revision = host.revision();
            host.progress_stop(revision, now)
        })();
        self.release_stopped_job();
        result
    }

    fn release_stopped_job(&mut self) {
        let closed = self.supervisor.host().is_ok_and(|host| {
            host.storage_failure().is_some() || host.inspect().stop.is_some()
                || self.job.as_ref().is_some_and(|job| {
                    stage(&host, job.request).is_ok_and(|state| !matches!(state,
                        ActionState::Reviewing | ActionState::Authorized | ActionState::Dispatching | ActionState::Unknown))
                })
        });
        if closed { if let Some(job) = &mut self.job { job.close(); } }
        self.reap_helpers();
        if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
    }
}
