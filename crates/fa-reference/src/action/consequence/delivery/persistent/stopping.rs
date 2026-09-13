//! Persist the original terminal admission stop separately from endpoint drain.
//! Neither local-stop evidence nor a clock reading is a nonexecution receipt.
use super::*;
use super::super::StopProgress;

/// Projection of the original stop sweep, acknowledged only after the canonical
/// journal replacement succeeds. The original endpoint outcomes, not a second
/// reducer, determine which resource charges remain outstanding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStopSweep {
    pub outcomes: BTreeMap<u64, Result<Reconciliation, Error>>,
    pub progress: StopProgress,
}

impl Machine {
    pub(super) fn apply_stop(&mut self, request: StopRequest) -> Result<Transition, Error> {
        let receipt = self.broker.request_stop(request)?;
        // This coupled host owns every sendable envelope; none escaped to a
        // separate transport. Withdrawal can therefore discard unsent messages
        // now without claiming that their already-charged attempts did not run.
        // Published outcomes stay in the original endpoint's receipt history.
        self.permits.clear();
        self.envelopes.clear();
        Ok(Transition::Stopped(receipt))
    }

    pub(super) fn apply_stop_progress(&mut self, tick: ElapsedTick) -> Result<Transition, Error> {
        self.broker.stop_receipt().ok_or(Error::Incomplete)?;
        // Recovery's saved tick is not current. The caller explicitly supplies
        // this observation in the independently bound process-independent domain.
        self.broker.observe_time(tick)?;
        self.endpoint.observe_time(tick)?;
        let sweep = self.broker.progress_stop(&mut self.endpoint)?;
        self.clock_ready = true;
        Ok(Transition::StopProgressed(FileStopSweep {
            outcomes: sweep.outcomes.into_iter().map(|(id, result)| (id, result.map(project_status))).collect(),
            progress: sweep.progress,
        }))
    }
}

impl FileDelivery {
    /// Permanently stop this domain through the original controller. Exact
    /// request retries preserve the original stop receipt; this profile has no
    /// resume operation. Only undispatched reservations may be refunded here.
    /// No endpoint fence or outstanding-effect settlement is claimed by this
    /// receipt. No time, policy snapshot or helper ballot is needed to stop.
    pub fn request_stop(&mut self, revision: u64, request: StopRequest) -> Result<StopReceipt, JournalError> {
        match self.transact(revision, Event::Stop(request))? {
            Transition::Stopped(receipt) => Ok(receipt), _ => unreachable!("stop transition"),
        }
    }

    /// Last acknowledged local-stop evidence, which can lag the canonical file
    /// after storage failure. This read-only receipt is never permission to send.
    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.machine.broker.stop_receipt() }

    /// Original stop accounting at the last acknowledged healthy cut. Inspect
    /// every unresolved ID; even a fully drained stop can retain executed charges.
    pub fn stop_progress(&self) -> Result<StopProgress, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.stop_progress()?)
    }

    /// With a NEW explicit trusted clock observation, install the current
    /// endpoint fence and seal the original outstanding requests. Prior executed
    /// receipts win. Retention expiry remains unresolved and charged; it is not
    /// silently refunded. This can run immediately after recovery without using
    /// a historical saved tick as current time. No intake is reopened.
    ///
    /// The original sweep's per-attempt results are committed as one journal
    /// replacement. An outer storage error returns no candidate results/refunds.
    pub fn progress_stop(&mut self, revision: u64, tick: ElapsedTick) -> Result<FileStopSweep, JournalError> {
        match self.transact(revision, Event::StopProgress(tick))? {
            Transition::StopProgressed(sweep) => Ok(sweep), _ => unreachable!("stop-progress transition"),
        }
    }
}
