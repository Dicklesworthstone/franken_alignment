//! Terminal completion of the original cooperative, independently guarded replay.
//! No ordinary recovery fence, second replay, live owner or role is returned.
use super::{FileGuardedRecovery, FileGuardedRecoveryStatus, FileOversight, JournalError};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopRequest,
    persistent::observed::{guarded::anchored::FileHistoryAnchor,
        shutdown::recovery::{FileAdmissionStoppedHistory, FileStopRecovery,
            FileStoppedHistory, FileStoppedRecovery}}};

impl FileGuardedRecovery {
    fn check_stopped_ready_with(&self, check: impl FnOnce(&Self) -> Result<(), Error>)
        -> Result<(), JournalError> {
        match self.status {
            FileGuardedRecoveryStatus::Ready => {}
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        check(self)?;
        Ok(())
    }

    /// Select the original stop predecessor only after complete replay AND the
    /// original full guard, effective-policy, credential and floor checks.
    /// A matching retained operation reuses its exact request; another cannot
    /// take it over. This selection grants no authority and does no file I/O.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.stop_request_checked(operation,
            |run| run.expected.check(&run.profile, &run.machine, &run.events))
    }

    // Sealed composition only. Specialised profiles supply their COMPLETE
    // native requirements, never a public caller-selected acceptance predicate.
    pub(super) fn stop_request_checked(&self, operation: u64,
        check: impl FnOnce(&Self) -> Result<(), Error>) -> Result<StopRequest, JournalError>
    {
        self.check_stopped_ready_with(check)?;
        if operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = self.machine.snapshot(self.events.len());
        if let Some(receipt) = snapshot.stop {
            let request = receipt.request();
            if request.operation != operation { return Err(Error::Duplicate.into()); }
            return Ok(request);
        }
        Ok(StopRequest { operation, expected_control_sequence: snapshot.control.sequence,
            expected_authority_epoch: snapshot.control.ledger.epoch })
    }

    /// Consume the SAME fully verified cursor into the original fixed
    /// Stop -> Fence -> StopProgress transaction, with no ordinary fence first.
    /// Only stopped observations escape. Outstanding effects remain original
    /// liabilities; no helper, current evidence or replacement key is acquired.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.finish_stopped_with_anchor(request, clock).map(|history| history.into_parts().0)
    }

    /// Return the native stop observation and exact acknowledged history anchor.
    /// All independent requirements are checked again before terminal selection.
    /// Anchored construction keeps its original prefix; reserve selection and
    /// every event/byte limit remain those of the same original cursor.
    ///
    /// Exact drained retries confirm the unchanged canonical image without a
    /// clock, event or replacement, even at capacity. Otherwise the clock is
    /// called once, after intent/canonical checks. The original consuming
    /// finalizer rechecks canonical bytes after native preflight and before
    /// cleanup/replacement, without replaying the already verified machine.
    ///
    /// Errors/unwinds return no owner, roles, stopped receipt or candidate anchor.
    /// Persistence failure may nevertheless leave a visible terminal replacement.
    /// Export is separate: keep the old archive until new custody is acknowledged.
    /// This is the base guarded profile, not admission of a learned, evaluated,
    /// predictive or mediated profile through a weaker recovery entry point.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.finish_stopped_checked_with_anchor(request, clock,
            |run| run.expected.check(&run.profile, &run.machine, &run.events))
    }

    /// Stop admission without sampling time, while retaining this cursor's
    /// complete independent guard/policy/credential/floor and anchor checks.
    /// A new stop uses only the original Stop/Fence events; an existing matching
    /// stop is confirmed unchanged. No sweep, live owner or role is returned.
    /// This does not load learned recipes or relax specialised recovery profiles.
    pub fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.finish_admission_stop_checked(request,
            |run| run.expected.check(&run.profile, &run.machine, &run.events))
    }

    /// Drain an already matching stop with one original StopProgress event.
    /// Recheck the independently selected profile before any clock or mutation.
    /// The original terminal consumer refuses active domains, rechecks canonical
    /// bytes around the fallible clock and skips clock/write for drained retries.
    pub fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.finish_stopped_drain_checked(request, clock,
            |run| run.expected.check(&run.profile, &run.machine, &run.events))
    }

    // These checks are sealed, like the original combined-stop checker. A
    // mediated wrapper supplies its complete original requirements, never the
    // weaker base predicate or a caller-controlled acceptance callback.
    pub(super) fn finish_admission_stop_checked(self, request: StopRequest,
        check: impl FnOnce(&Self) -> Result<(), Error>)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.into_terminal_checked(check)?.finish_admission_stop(request)
    }

    pub(super) fn finish_stopped_drain_checked<F>(self, request: StopRequest, clock: F,
        check: impl FnOnce(&Self) -> Result<(), Error>) -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.into_terminal_checked(check)?.finish_stopped_drain(request, clock)
    }

    fn into_terminal_checked(self, check: impl FnOnce(&Self) -> Result<(), Error>)
        -> Result<FileStopRecovery, JournalError>
    {
        self.check_stopped_ready_with(check)?;
        let Self { profile, store, canonical, events, machine, .. } = self;
        Ok(FileStopRecovery::from_verified_terminal(profile, store, canonical, events, machine))
    }

    // Reuse the exact original stop selection, no-clock retry and consuming
    // finalizer for sealed profiles without weakening their independent guards.
    pub(super) fn finish_stopped_checked_with_anchor<F>(self, request: StopRequest, clock: F,
        check: impl FnOnce(&Self) -> Result<(), Error>) -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_stopped_ready_with(check)?;
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = self.machine.snapshot(self.events.len());
        if let Some(receipt) = &snapshot.stop {
            let previous = receipt.request();
            if previous != request {
                return Err(if previous.operation == request.operation { Error::Binding }
                    else { Error::Duplicate }.into());
            }
            let progress = self.machine.broker.stop_progress()?;
            if progress.drained() {
                if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
                    return Err(Error::Binding.into());
                }
                self.store.confirm_and_cleanup()?;
                let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), self.canonical);
                return Ok(FileStoppedHistory::new(
                    FileStoppedRecovery::AlreadyDrained { snapshot, progress }, anchor));
            }
        } else if snapshot.control.sequence != request.expected_control_sequence
            || snapshot.control.ledger.epoch != request.expected_authority_epoch {
            return Err(Error::Stale.into());
        }
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let tick = clock();
        let Self { profile, store, events, machine, canonical, .. } = self;
        let (host, _reviewer) = FileOversight::owner(profile, store, events, machine);
        host.finish_verified_stopped(request, tick, &canonical)
    }
}

#[cfg(test)]
#[path = "stopped/phases_tests.rs"]
mod phases_tests;
