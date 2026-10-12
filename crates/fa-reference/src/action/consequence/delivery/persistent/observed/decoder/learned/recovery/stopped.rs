//! Terminal completion of the SAME sealed original learned replay cursor.
//! No second replay, ordinary recovery fence, live intake or role escape.
use super::{FileLearnedRecovery, FileOversight, JournalError};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopRequest,
    persistent::observed::shutdown::recovery::FileStoppedRecovery};

impl FileLearnedRecovery {
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
                return Ok(FileStoppedRecovery::AlreadyDrained { snapshot, progress });
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
