//! Drive targeted settlement without replacing the original owner or child duty.

use super::{ElapsedTick, Error, FileRequestStatus, FileSupervisedDriver, JournalError};

impl FileSupervisedDriver {
    /// Settle a durable request through this driver's ORIGINAL two-key owner.
    /// The ID may name the current job or an earlier dispatched obligation.
    /// No provider callback, helper exchange, new approval, credential exercise
    /// or retry of the effect occurs. The original owner returns a receipt-backed
    /// disposition, not this driver's local phase or an invented refund.
    ///
    /// After the call, current owner state decides which active job retires.
    /// Settling an older request (including an idempotent terminal retry) cannot
    /// close a different healthy review. A normal preflight refusal preserves
    /// that review. A storage fault closes its local drive path without claiming
    /// a committed cancellation; exclusive recovery must inspect the real cut.
    ///
    /// Original cleanup closes helper sockets and retains owned child cohorts
    /// for nonblocking reaping. Idle is not proof that children have exited:
    /// keep polling helpers_reaped/reap_helpers before starting another cohort
    /// or use the existing explicit release handoff. No children are detached.
    pub fn cancel_and_resolve_request(
        &mut self,
        revision: u64,
        request: u64,
        observed_tick: ElapsedTick,
    ) -> Result<FileRequestStatus, JournalError> {
        let result = {
            // Refuse conflicting borrows before touching the healthy job/pool.
            let mut host = self.supervisor.host_mut()?;
            if let Some(job) = &self.job {
                job.check_owner(&host)?;
            }
            host.cancel_and_resolve_request(revision, request, observed_tick)
        };
        // Never infer active-job termination from another request's result.
        // This also preserves the original pre-I/O fault latch and reap duty.
        self.release_stopped_job();
        result
    }

    /// Clocked settlement of the active job, unlike cancel_active's deliberately
    /// query-only behavior after dispatch. Retention/capacity refusal cannot
    /// become success, and an already executed publication remains charged.
    /// The supplied tick is trusted supervisor input, never an actor timestamp.
    pub fn cancel_and_resolve_active(
        &mut self,
        observed_tick: ElapsedTick,
    ) -> Result<FileRequestStatus, JournalError> {
        let request = self.job.as_ref().ok_or(Error::Missing)?.request;
        let revision = self.supervisor.host()?.revision();
        self.cancel_and_resolve_request(revision, request, observed_tick)
    }
}
