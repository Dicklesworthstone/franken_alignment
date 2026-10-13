//! Exact retained-prefix continuity for the original source-free stop operation.
//! No recovered owner or candidate anchor escapes before acknowledgment.
use super::{Error, FileHistoryAnchor, FileOversight, FileOversightProfile,
    FileStoppedHistory, FileStoppedRecovery, JournalError, Machine, StopRequest,
    ElapsedTick, Path, journal, storage};

impl FileOversight {
    /// Require an independently retained exact history prefix before recovering
    /// directly into the ORIGINAL stopped state. Counter-equal forks and rollback
    /// behind the anchor refuse before replay, cleanup or the terminal clock.
    /// The returned private anchor and stop observation cover the same durably
    /// acknowledged cut; no live owner, reviewer or permission is returned.
    ///
    /// This is the ordinary source-free replay profile plus exact continuity,
    /// not an alternative learned-model loader or complete guard-set admission.
    /// All original decoding, replay and optional-contract errors remain errors.
    /// Learned histories requiring independently supplied numerical recipes must
    /// use their original guarded learned cursors, not a fallback through here.
    ///
    /// The original Stop -> Fence -> StopProgress transaction consumes the fully
    /// verified machine without another numerical replay or an earlier fence.
    /// Canonical bytes are rechecked before the clock and again after native
    /// preflight, before cleanup/replacement. Invalid time/capacity or an unwind
    /// cannot yield a stopped receipt, anchor or refund. Failed persistence can
    /// still leave a visible replacement; retry the exact original operation.
    ///
    /// A matching drained retry confirms the same image and returns the identical
    /// anchor without a clock call, record or replacement, including at capacity.
    /// Pending/expired effects remain charged. External export and custodian
    /// authentication are separate; keep the previous archive until export is
    /// acknowledged. Joint rollback of journal and archive remains out of scope.
    pub fn recover_stopped_anchored<F>(directory: impl AsRef<Path>, profile: FileOversightProfile,
        request: StopRequest, anchor: &FileHistoryAnchor, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        profile.delivery.limits.check()?;
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        recover_anchored_locked(store, profile, request, anchor, clock)
    }
}

// The same native Store is kept locked through prefix admission, replay,
// original terminal preflight and acknowledgment. This private seam exists for
// the existing deterministic Store barriers, never caller-selected persistence.
fn recover_anchored_locked<F>(store: storage::Store, profile: FileOversightProfile,
    request: StopRequest, anchor: &FileHistoryAnchor, clock: F)
    -> Result<FileStoppedHistory, JournalError>
where F: FnOnce() -> ElapsedTick {
    let canonical = store.read(profile.delivery.limits.bytes)?;
    let events = journal::decode(&profile, store.identity(), &canonical)?;
    anchor.check(&profile, store.identity(), &events)?;
    let machine = Machine::replay(&profile, &events)?;
    let snapshot = machine.snapshot(events.len());
    if let Some(receipt) = &snapshot.stop {
        let previous = receipt.request();
        if previous != request {
            return Err(if previous.operation == request.operation { Error::Binding }
                else { Error::Duplicate }.into());
        }
        let progress = machine.broker.stop_progress()?;
        if progress.drained() {
            if store.read(profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
            store.confirm_and_cleanup()?;
            let anchor = FileHistoryAnchor::from_acknowledged(events.len(), canonical);
            return Ok(FileStoppedHistory::new(
                FileStoppedRecovery::AlreadyDrained { snapshot, progress }, anchor));
        }
    } else if snapshot.control.sequence != request.expected_control_sequence
        || snapshot.control.ledger.epoch != request.expected_authority_epoch {
        return Err(Error::Stale.into());
    }
    if store.read(profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
    // No cleanup before the clock or terminal preflight. The native consuming
    // finalizer rechecks this same canonical image before cleanup/replacement
    // and moves its successful encoded bytes into the returned history anchor.
    let tick = clock();
    let (host, _reviewer) = FileOversight::owner(profile, store, events, machine);
    host.finish_verified_stopped(request, tick, &canonical)
}

#[cfg(test)]
mod tests;
