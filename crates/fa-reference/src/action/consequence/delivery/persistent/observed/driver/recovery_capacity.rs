//! Explicit terminal recovery when ordinary journal work is no longer admitted.
//! Uses the original durable stop first, then its separate endpoint sweep.
use super::{FileSupervisedDriver, JournalError};
use super::super::super::FileStopSweep;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopProgress, StopReceipt, StopRequest};
use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapacityDrain {
    /// Current native progress already has an acknowledged fence and no pending
    /// effects. No clock or journal event was required by this exact retry.
    AlreadyDrained(StopProgress),
    Advanced(FileStopSweep),
}

/// The local stop succeeded durably even if the subsequent drain failed. An
/// outer Err instead means this operation returned no acknowledged stop receipt.
/// No value here constitutes a new effect permit or a helper-reaping assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapacityStop {
    pub stop: StopReceipt,
    pub drain: Result<CapacityDrain, JournalError>,
}

impl FileSupervisedDriver {
    /// Operator-requested terminal handling of journal pressure, not an automatic
    /// reaction to every generic Limit error. Requires an explicitly installed
    /// recovery reserve; ordinary work can never spend that tail. Exact retries
    /// reuse the native stop receipt and do not burn another Stop event.
    ///
    /// Stop, retire the abandoned job and request direct-child cleanup BEFORE
    /// sampling the fallible clock. A stale clock or failed sweep leaves the
    /// original stop in force and uncertain effects charged. The caller must
    /// inspect the nested drain result and continue polling owned child cleanup.
    pub fn stop_with_recovery_reserve<F>(&mut self, request: StopRequest, mut clock: F)
        -> Result<CapacityStop, JournalError>
    where F: FnMut() -> ElapsedTick {
        let prepared = (|| {
            let host = self.supervisor.host()?;
            if let Some(job) = &self.job { job.check_owner(&host)?; }
            if host.journal_capacity()?.reserve().is_none() { return Err(Error::Incomplete.into()); }
            if request.operation == 0 { return Err(Error::InvalidInput.into()); }
            if let Some(receipt) = host.inspect().stop {
                if receipt.request() != request {
                    let error = if receipt.request().operation == request.operation { Error::Binding } else { Error::Duplicate };
                    return Err(JournalError::Contract(error));
                }
                return Ok(Some(receipt));
            }
            Ok(None)
        })();
        let previous = match prepared {
            Ok(previous) => previous,
            Err(error) => { self.reap_helpers(); return Err(error); }
        };
        let stop = match previous {
            Some(receipt) => {
                // A stop may have been issued through the lower-level host.
                // Its current ledger is already terminal; do not append again.
                if let Some(job) = &mut self.job { job.close(); }
                self.reap_helpers();
                self.job = None;
                receipt
            }
            None => self.request_stop(request)?,
        };
        let drain = (|| {
            let progress = self.supervisor.host()?.stop_progress()?;
            if progress.drained() { return Ok(CapacityDrain::AlreadyDrained(progress)); }
            let now = clock();
            self.progress_stop(now).map(CapacityDrain::Advanced)
        })();
        self.reap_helpers();
        Ok(CapacityStop { stop, drain })
    }
}
