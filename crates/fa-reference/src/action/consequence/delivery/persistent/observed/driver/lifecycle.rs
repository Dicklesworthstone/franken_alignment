//! Control-plane cleanup and query-only recovery using the same locked owner.
mod settlement;
use super::{FileSupervisedDriver, Job, Phase, admitted, stage};
use super::super::super::{FileStopSweep, JournalError, Reconciliation};
use super::super::super::governance::{PolicyUpdate, PolicyUpdateReceipt};
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
    /// Commit the trusted clock and native sweep in ONE canonical replacement.
    /// Both original event slots must fit before either advances. A failed sweep
    /// cannot consume the final slot by leaving behind a separate Time event.
    /// Per-attempt unresolved results remain explicit; nothing is resent.
    pub fn reconcile_pending(&mut self, now: ElapsedTick)
        -> Result<BTreeMap<u64, Result<Reconciliation, Error>>, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            let revision = host.revision();
            host.reconcile_publications_at(revision, now)
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

impl FileSupervisedDriver {
    /// Commit trusted governance without asking the actor, helper or human-effect
    /// reviewer to change policy. Its original owner cancels undispatched work;
    /// then existing maintenance retires its workers before any later clock/I/O.
    /// A rejected update leaves a healthy job intact. An exact historical retry
    /// does not retire a newer job merely because the OLD receipt lists cancelled
    /// attempts. Original live dispositions, not receipt contents, drive cleanup.
    ///
    /// Earlier dispatches retain their publication/reconciliation phases and
    /// charges. An enabled first-publication guard still revalidates at execution;
    /// policy updates do not disable it. This method never resends an effect.
    pub fn replace_policy(&mut self, revision: u64, update: &PolicyUpdate)
        -> Result<PolicyUpdateReceipt, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            if let Some(job) = &self.job { job.check_owner(&host)?; }
            host.replace_policy(revision, update)
        })();
        self.reap_helpers();
        result
    }

    /// Restore through the original durable containment operation. Current
    /// ledger state, not an old retry receipt, decides which workers retire.
    /// The next driver step retains its original Stopped event. No clock,
    /// observation provider, helper vote or new human approval is acquired here.
    /// A sent effect retains its charged outcome/publication obligations.
    pub fn reset_actor(&mut self, checkpoint: &super::super::containment::FileCheckpoint,
        request: super::super::containment::FileResetRequest)
        -> Result<crate::action::consequence::gate::containment::ResetReceipt, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            if let Some(job) = &self.job { job.check_owner(&host)?; }
            let revision = host.revision();
            host.reset_actor(revision, checkpoint, request)
        })();
        // Also retire owned children after ambiguous storage, while leaving
        // an ordinary stale-predecessor refusal's healthy review untouched.
        self.reap_helpers();
        result
    }

    /// Recover the registered source through the same durable owner. Maintenance
    /// uses current ledger dispositions, not a historical replacement receipt,
    /// so an exact retry cannot terminate a newer helper cohort. Dispatched work
    /// keeps its guarded publication/reconciliation phase and charged liability.
    /// This method does not read evidence, sample time or issue an approval.
    pub fn replace_file_source(&mut self, request: super::super::source::FileSourceReplacement)
        -> Result<crate::action::consequence::delivery::PolicySourceChange, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            if let Some(job) = &self.job { job.check_owner(&host)?; }
            let revision = host.revision();
            host.replace_file_source(revision, request)
        })();
        self.reap_helpers();
        result
    }
}

impl FileSupervisedDriver {
    /// Retire only a terminal ORIGINAL request and its fully reaped direct
    /// children, without reopening the authority, resetting quotas or cancelling
    /// any other request. False means retain this driver and poll again; it is
    /// not permission to start a second cohort. Dispatched/unknown outcomes are
    /// never terminal here, even when this driver's local phase is Idle.
    pub fn retire_completed_request(&mut self, request: u64) -> Result<bool, JournalError> {
        use super::super::super::requests::FileRequestDisposition;
        {
            let host = self.supervisor.host()?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            if let Some(job) = &self.job {
                job.check_owner(&host)?;
                if job.request != request { return Err(Error::Binding.into()); }
            }
            let terminal = match host.request_status(request)?.disposition {
                FileRequestDisposition::NotAdmitted(_) => true,
                FileRequestDisposition::Admitted { stage, .. } => matches!(stage,
                    ActionState::Cancelled | ActionState::Denied
                    | ActionState::Confirmed | ActionState::ConfirmedNotExecuted),
            };
            if !terminal { return Err(Error::WrongState.into()); }
        }
        if let Some(job) = &mut self.job { job.close(); }
        self.reap_helpers();
        if !self.helpers_reaped() { return Ok(false); }
        // The original owner retains all history and liabilities. Only terminal
        // local handles and confirmed-reaped child records are discarded.
        self.ensure_child_slot()?;
        self.job = None;
        Ok(true)
    }
}
