//! Terminal completion of the SAME sealed original learned replay cursor.
//! No second replay, ordinary recovery fence, live intake or role escape.
use super::{FileLearnedRecovery, FileOversight, JournalError};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopRequest,
    persistent::observed::{guarded::anchored::FileHistoryAnchor,
        shutdown::recovery::{FileAdmissionStoppedHistory, FileStopRecovery, FileStoppedHistory, FileStoppedRecovery}}};

impl FileLearnedRecovery {
    /// Consume the SAME fully replayed learned cursor into a nonce-bound
    /// Stop-only transport target. No ordinary recovery Fence, live owner,
    /// reviewer, predictor or numerical continuation escapes. Its independently
    /// selected outer wrapper must recheck all original requirements first.
    ///
    /// A retained recovery reserve is mandatory, as on the live Stop transport;
    /// this never installs or refills one. The original fixed Stop/Fence consumer
    /// checks exact capacity and canonical custody only after an explicit request.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(self, operation: u64)
        -> Result<crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError>
    {
        use crate::action::consequence::delivery::persistent::observed::{
            BaseEvent, Event, reviewer::{client::ReviewerExpectation,
                peer::control::FileRecoveryStop},
        };
        let request = self.stop_request(operation)?;
        if !self.events.iter().any(|event| matches!(event,
            Event::Core(BaseEvent::ReserveRecovery(_)))) {
            return Err(Error::Incomplete.into());
        }
        let expected = ReviewerExpectation { reviewer: self.profile.human.reviewer_id,
            scope: self.profile.delivery.scope, clock_domain: self.profile.delivery.clock_domain };
        Ok(FileRecoveryStop::from_verified(self.into_terminal()?, expected, request))
    }

    /// Select the original control predecessor only after complete learned
    /// replay. A matching historical stop retains its exact original request;
    /// another operation cannot take it over. This request grants no live role.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        let machine = self.verified_guarded_machine()?;
        if operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = machine.snapshot(self.events.len());
        if let Some(receipt) = snapshot.stop {
            let request = receipt.request();
            if request.operation != operation { return Err(Error::Duplicate.into()); }
            return Ok(request);
        }
        Ok(StopRequest { operation, expected_control_sequence: snapshot.control.sequence,
            expected_authority_epoch: snapshot.control.ledger.epoch })
    }

    /// Withdraw admission without acquiring time or completing a pending
    /// numerical step. Reuse the original Stop/Fence consumer on this fully
    /// verified learned machine and its SAME exclusive canonical cut. No owner,
    /// predictor, reviewer or numerical continuation is returned. Unresolved and
    /// executed effects keep their original charges; this is not a drain receipt.
    pub fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.into_terminal()?.finish_admission_stop(request)
    }

    /// Settle only an already matching stop through one original StopProgress
    /// event. A failed clock leaves the acknowledged admission stop unchanged.
    /// Exact drained retries require no clock, event or write, even at capacity.
    /// The original terminal consumer checks intent and canonical bytes before
    /// time acquisition and again before replacement; no numerical work repeats.
    pub fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.into_terminal()?.finish_stopped_drain(request, clock)
    }

    /// Fallible-clock combined recovery. Clock refusal returns no candidate
    /// receipt or anchor and precedes cleanup and the original combined write.
    /// This permits a caller to reject a late acquisition without inventing time.
    pub fn try_finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.into_terminal()?.try_finish_stopped_with_anchor(request, clock)
    }

    // Private handoff after complete ORIGINAL learned replay. A specialised
    // wrapper must run its full independent checks before delegating here.
    // Never expose the ordinary cursor or reconstruct an unchecked live owner.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn into_terminal(self) -> Result<FileStopRecovery, JournalError> {
        self.verified_guarded_machine()?;
        let Self { profile, store, canonical, events, machine, last_drain, .. } = self;
        Ok(FileStopRecovery::from_verified_terminal(profile, store, canonical, events, machine, last_drain))
    }

    /// Consume completely verified learned history directly into the ORIGINAL
    /// Stop -> Fence -> StopProgress replacement. There is no ordinary recovery
    /// fence first, numerical replay repetition, helper intake or publication
    /// retry. The original finite terminal reserve and both journal limits apply.
    ///
    /// Only original stopped observations are returned. Old keys are withdrawn
    /// and unresolved effects remain charged. Exact already-drained retries
    /// confirm the same canonical image without a clock call, event or write,
    /// including at full capacity. Otherwise the trusted clock is called once.
    ///
    /// The exclusive lock and exact canonical cut survive through native
    /// preflight and persistence. Invalid intent or changed history performs no
    /// cleanup. Storage failure returns no candidate receipt; a replacement may
    /// nevertheless have become visible. External nonrollback remains required.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.finish_stopped_with_anchor(request, clock).map(|history| history.into_parts().0)
    }

    /// Complete the SAME terminal transaction and retain its exact acknowledged
    /// canonical bytes alongside the stopped observations. No live owner, role,
    /// second replay, extra encoding or storage reopen is required for capture.
    ///
    /// The original intent, canonical-cut and durability checks precede any
    /// result. A failed replacement yields neither receipt nor candidate anchor.
    /// Already-drained retry confirms and returns the identical retained cut,
    /// without a clock call or replacement. The private anchor must subsequently
    /// be retained outside the authority store; that export is not atomic with
    /// the stop and cannot change its outcome or discharge unknown liabilities.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        let machine = self.verified_guarded_machine()?;
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = machine.snapshot(self.events.len());
        if let Some(receipt) = &snapshot.stop {
            let previous = receipt.request();
            if previous != request {
                return Err(if previous.operation == request.operation {
                    Error::Binding
                } else { Error::Duplicate }.into());
            }
            let progress = machine.broker.stop_progress()?;
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
