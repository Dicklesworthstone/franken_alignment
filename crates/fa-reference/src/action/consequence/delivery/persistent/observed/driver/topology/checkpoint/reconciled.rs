//! Exact source custody through one acknowledged receipt reconciliation.
//! No source is reopened and no unknown liability is inferred to be settled.
use super::{TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, JournalError,
    observed::{
        BaseEvent, Event, FileOversightProfile, decoder::learned::FileReconciledLearnedReceipt,
    },
};
use std::path::Path;

impl TopologyCheckpoint {
    // The same COMPLETE original cursor calls this before the clock or mutation.
    // No public candidate or caller-supplied snapshot can reach this seam.
    pub(in crate::action::consequence::delivery::persistent::observed) fn check_reconciled_start(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        snapshot: &FileDeliverySnapshot,
    ) -> Result<(), JournalError> {
        check_work(&self.expected, self.work, self.last_operation)?;
        if self.history.revision() != events.len() as u64
            || snapshot.revision != events.len() as u64
        {
            return Err(Error::Stale.into());
        }
        self.history.check(profile, identity, events)?;
        let (graph, available) = super::terminal::topology_at(events)?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        Ok(())
    }

    /// Pair original source expenditure with the exact immutable acknowledged
    /// receipt result. Only the SAME complete pre-reconciliation cut followed by
    /// one Fence/Time/Sweep is accepted. An older source checkpoint, unrelated
    /// maintenance, source work, changed profile/path/graph or divergent prefix
    /// cannot be promoted through this API.
    ///
    /// The original documents, spent work and last source operation are copied
    /// unchanged. The native Fence leaves the graph unavailable. The result says
    /// nothing about unresolved outcomes beyond its original actor observation.
    /// No filesystem read, semantic replay, new clock or live authority occurs.
    /// Use the cursor's with_checkpoint finalizer to admit selected source custody
    /// before mutation; this separate pure conversion does not export archives.
    pub fn after_reconciled_receipt(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        receipt: &FileReconciledLearnedReceipt,
    ) -> Result<Self, JournalError> {
        check_work(&self.expected, self.work, self.last_operation)?;
        let history = receipt.history_anchor();
        let snapshot = receipt.snapshot();
        if self.history.revision() != receipt.reconciled_from()
            || history.revision()
                != receipt
                    .reconciled_from()
                    .checked_add(3)
                    .ok_or(Error::Overflow)?
            || snapshot.revision != history.revision()
        {
            return Err(Error::Stale.into());
        }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        let events = history.decoded_events(profile, identity)?;
        self.history.check(profile, identity, &events)?;
        let after = usize::try_from(receipt.reconciled_from()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        match &events[after..] {
            [
                Event::Core(BaseEvent::Fence),
                Event::Core(BaseEvent::Time(_)),
                Event::Core(BaseEvent::Sweep),
            ] => {}
            _ => return Err(Error::Binding.into()),
        }
        let (graph, available) = super::terminal::topology_at(&events)?;
        if graph != self.expected.graph() || available {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: history.clone(),
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }
}
