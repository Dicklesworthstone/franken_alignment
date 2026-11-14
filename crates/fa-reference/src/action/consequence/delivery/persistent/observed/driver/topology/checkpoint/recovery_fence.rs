//! Retain the original source-work cut immediately after native guarded recovery.
//! Acknowledged Fence custody needs neither a live source nor a credential guard.
use super::{FileOversight, JournalError, TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event};

impl TopologyCheckpoint {
    /// Advance this independently retained checkpoint across exactly ONE
    /// acknowledged original recovery Fence, before subsequent startup work.
    /// The same healthy owner's profile, canonical identity and complete retained
    /// prefix must agree. The original graph and historical availability must
    /// match that cut; the native successor must retain the graph, withdraw its
    /// availability and preserve the retained control and authority floors.
    ///
    /// This permits a supervisor to retain honest recovery custody before a
    /// fallible source rebind. It does not require credential registration.
    /// Source work, time observations, reconciliation, credential changes, Stop,
    /// multiple fences and all other successors refuse. They cannot silently
    /// inherit older source expenditure. Repeating this call with the ORIGINAL
    /// checkpoint and unchanged healthy owner returns the same successor.
    ///
    /// Original documents, spent work and the last source operation are copied
    /// unchanged. Only acknowledged history, withdrawn availability and native
    /// floors advance. No file access, source check, replay, clock call, journal
    /// write, topology certification or authority is created. Independently
    /// retaining the successor remains the caller's obligation; keep the old
    /// checkpoint until that export has been durably acknowledged.
    pub fn after_recovery_fence(&self, host: &FileOversight) -> Result<Self, JournalError> {
        if host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        check_work(&self.expected, self.work, self.last_operation)?;
        let snapshot = host.inspect();
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        self.history
            .check(&host.profile, host.store.identity(), &host.events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        if !matches!(&host.events[after..], [Event::Core(BaseEvent::Fence)]) {
            return Err(Error::Stale.into());
        }
        let (graph, available) = super::terminal::topology_at(&host.events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        let current = host.mediation_snapshot()?;
        if current.graph != *self.expected.graph() || current.available {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: host.history_anchor()?,
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available: false,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }
}
