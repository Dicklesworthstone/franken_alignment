//! Explicit terminal disposition on the same independently guarded private replay.
//! No live owner, role, fresh observation or recovered effect envelope escapes.
use super::{
    BaseEvent, Event, FileGuardedRecovery, FileGuardedRecoveryStatus, FileHistoryAnchor,
    JournalError, Transition, journal,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    abandonment::{FileAbandonReceipt, FileAbandonRequest},
};

/// One acknowledged explicit abandonment and its exact canonical history cut.
/// The receipt describes the supervisor's disposition, not an endpoint outcome.
/// Its original action remains charged and unknown to the actor; a stopped
/// domain still has an unresolved liability. The snapshot may include later
/// acknowledged events when this is an exact retry of an older operation.
///
/// Export is separate from acknowledgment. Keep the independent input archive
/// until the output archive has been durably retained. No live owner or role can
/// be extracted from this value.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileAbandonedHistory;
/// fn no_authority(history: FileAbandonedHistory) { history.owner(); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAbandonedHistory {
    receipt: FileAbandonReceipt,
    snapshot: FileDeliverySnapshot,
    anchor: FileHistoryAnchor,
}

impl FileAbandonedHistory {
    pub fn receipt(&self) -> &FileAbandonReceipt {
        &self.receipt
    }
    pub fn snapshot(&self) -> &FileDeliverySnapshot {
        &self.snapshot
    }
    pub fn history_anchor(&self) -> &FileHistoryAnchor {
        &self.anchor
    }
}

impl FileGuardedRecovery {
    /// Record only an explicit Unknown -> IrrecoverablyUnknown disposition on
    /// this fully replayed and independently checked canonical image. The
    /// original predecessor and operation identity remain mandatory. No clock,
    /// source, helper, human key, endpoint query or endpoint send is acquired.
    ///
    /// A new operation appends one ordinary-work event; it cannot consume the
    /// reserved Fence/Stop/StopProgress tail. An exact historical retry confirms
    /// the unchanged canonical image with no event or replacement, even after
    /// authority epochs advance or ordinary capacity is exhausted. Conflicting
    /// retries and all states other than Unknown refuse before storage cleanup.
    ///
    /// There is no ordinary recovery Fence: this consuming finalizer returns
    /// only observations. Future live recovery still requires its original
    /// Fence and guards. Failure returns no receipt or candidate anchor, even
    /// when an ambiguous replacement may already have become visible.
    pub fn finish_abandoned_with_anchor(
        self,
        request: FileAbandonRequest,
    ) -> Result<FileAbandonedHistory, JournalError> {
        self.finish_abandoned_checked_with_anchor(request, |run| {
            run.expected.check(&run.profile, &run.machine, &run.events)
        })
    }

    // Sealed composition: specialised cursors must supply their COMPLETE
    // original requirements; callers cannot replace this with a weaker check.
    pub(super) fn finish_abandoned_checked_with_anchor(
        mut self,
        request: FileAbandonRequest,
        check: impl FnOnce(&Self) -> Result<(), Error>,
    ) -> Result<FileAbandonedHistory, JournalError> {
        match self.status {
            FileGuardedRecoveryStatus::Ready => {}
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileGuardedRecoveryStatus::Cancelled => return Err(Error::WrongState.into()),
            FileGuardedRecoveryStatus::Replaying => return Err(Error::Incomplete.into()),
        }
        check(&self)?;
        let previous = self.machine.preflight_abandon_unknown(request)?;
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        if let Some(receipt) = previous {
            let snapshot = self.machine.snapshot(self.events.len());
            self.store.confirm_and_cleanup()?;
            let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), self.canonical);
            return Ok(FileAbandonedHistory {
                receipt,
                snapshot,
                anchor,
            });
        }

        let event = Event::Core(BaseEvent::AbandonUnknown(request));
        // Canonical prefix admission enforces ordinary capacity before the
        // private reducer changes state or any staging file is touched.
        let bytes =
            journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let receipt = match self.machine.apply(&event)? {
            Transition::Abandoned(receipt) => receipt,
            _ => unreachable!("abandonment transition"),
        };
        let count = self.events.len().checked_add(1).ok_or(Error::Overflow)?;
        let snapshot = self.machine.snapshot(count);
        // No second replay or exported candidate exists. Recheck the exact
        // canonical bytes after preflight, before cleanup and publication.
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        self.store.confirm_and_cleanup()?;
        self.store.replace(&bytes)?;
        let anchor = FileHistoryAnchor::from_acknowledged(count, bytes);
        Ok(FileAbandonedHistory {
            receipt,
            snapshot,
            anchor,
        })
    }
}

#[cfg(test)]
mod tests;
