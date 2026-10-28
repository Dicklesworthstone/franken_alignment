//! Preserve original source work across only the selected native abandonment.
//! A retained checkpoint never certifies fresh topology or an endpoint outcome.
use super::{TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, JournalError,
    abandonment::FileAbandonRequest,
    observed::{
        BaseEvent, Event, FileOversightProfile, guarded::recovery::FileAbandonedHistory,
    },
};
use std::path::Path;

impl TopologyCheckpoint {
    // Only completely replayed private cursors can preflight this source cut.
    // Ordinary guarded recovery remains exact; this restricted finalizer may
    // confirm just the same original request's one-event successor for export.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn check_abandoned_start(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        snapshot: &FileDeliverySnapshot,
        request: FileAbandonRequest,
    ) -> Result<(), JournalError> {
        if snapshot.revision != events.len() as u64 {
            return Err(Error::Binding.into());
        }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        self.check_abandoned_suffix(profile, identity, events, request)
    }

    fn check_abandoned_suffix(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        request: FileAbandonRequest,
    ) -> Result<(), JournalError> {
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        match &events[after..] {
            [] => {}
            [Event::Core(BaseEvent::AbandonUnknown(original))] if *original == request => {}
            _ => return Err(Error::Stale.into()),
        }
        let (graph, available) = super::terminal::topology_at(events)?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    /// Advance this independently retained source cut to the same acknowledged
    /// abandonment, or retain its exact retry. Only the unchanged cut or one
    /// exact AbandonUnknown event may follow the saved prefix. Fence, time,
    /// source work, topology changes and any unrelated operation all refuse.
    ///
    /// Original documents, spent work, last source operation and availability
    /// remain unchanged. The new anchor and floors describe the acknowledged
    /// native cut, with no live observer, source read, extra semantic replay,
    /// clock or authority. Use the cursor's with_checkpoint finalizer to admit
    /// this source selection BEFORE mutation; archive writes remain separate.
    pub fn after_abandoned_history(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        history: &FileAbandonedHistory,
    ) -> Result<Self, JournalError> {
        let anchor = history.history_anchor();
        let snapshot = history.snapshot();
        if snapshot.revision != anchor.revision() {
            return Err(Error::Binding.into());
        }
        let request = history.receipt().request();
        let events = anchor.decoded_events(profile, identity)?;
        self.check_abandoned_start(profile, identity, &events, snapshot, request)?;
        if !events.iter().any(|event| {
            matches!(event, Event::Core(BaseEvent::AbandonUnknown(original)) if *original == request)
        }) {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: anchor.clone(),
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available: self.available,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }
}
