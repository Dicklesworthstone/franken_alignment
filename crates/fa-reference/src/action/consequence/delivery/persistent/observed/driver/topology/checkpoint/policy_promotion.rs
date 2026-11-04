//! Preserve independently retained source expenditure through exactly one
//! original atomic campaign promotion. No source or numerical work is refreshed.
use super::{TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, JournalError,
    observed::{
        Event, FileOversightProfile,
        decoder::learned::{FileLearnedPolicyPromotionHistory, FileLearnedPolicyPromotionRequest},
        governance::campaigns::CampaignEvent,
    },
};
use std::path::Path;

impl TopologyCheckpoint {
    // Called only by the sealed mediated campaign cursor before numerical
    // replay. This admits an exact retained cut or its one matching atomic
    // Request/Approve/Promote suffix; ordinary recovery still requires equality.
    // Full original replay and acceptance-bit binding remain mandatory.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn policy_promotion_successor_available(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        request: &FileLearnedPolicyPromotionRequest,
    ) -> Result<bool, Error> {
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding);
        }
        match &events[after..] {
            [] => {}
            [
                Event::Campaign(CampaignEvent::Request(update)),
                Event::Campaign(CampaignEvent::Approve(approved, _)),
                Event::Campaign(CampaignEvent::Promote(promoted)),
            ] if self.history.revision() == request.revision()
                && update == request.update()
                && *approved == update.operation()
                && *promoted == update.operation() => {}
            _ => return Err(Error::Stale),
        }
        // A policy promotion revokes approval and authority through the
        // original gate. It performs no graph update or recovery Fence.
        let (graph, available) = super::terminal::topology_at(events)?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding);
        }
        Ok(available)
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn check_policy_promotion_start(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        snapshot: &FileDeliverySnapshot,
        request: &FileLearnedPolicyPromotionRequest,
    ) -> Result<(), JournalError> {
        if snapshot.revision != events.len() as u64 {
            return Err(Error::Binding.into());
        }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        self.policy_promotion_successor_available(profile, identity, events, request)?;
        Ok(())
    }

    /// Pair the original source documents, spent work and last operation with
    /// the exact acknowledged promotion. Only an unchanged retained cut or its
    /// complete matching Request/Approve/Promote suffix is accepted. No Fence,
    /// source refresh, numerical event or unrelated operation can explain a gap.
    ///
    /// The sealed native receipt proves acknowledgment; this bounded conversion
    /// does not reopen the store, rerun inference, certify a graph cut or issue
    /// any role. Use the mediated promotion cursor to check this custody BEFORE
    /// mutation. Retaining both output archives remains a separate obligation.
    pub fn after_promoted_policy(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        history: &FileLearnedPolicyPromotionHistory,
    ) -> Result<Self, JournalError> {
        let anchor = history.history_anchor();
        let snapshot = history.snapshot();
        let request = history.review().request();
        if snapshot.revision != anchor.revision() || history.receipt().update() != request.update() {
            return Err(Error::Binding.into());
        }
        let events = anchor.decoded_events(profile, identity)?;
        self.check_policy_promotion_start(profile, identity, &events, snapshot, request)?;
        let at = usize::try_from(request.revision()).map_err(|_| Error::Limit)?;
        let end = at.checked_add(3).ok_or(Error::Overflow)?;
        match events.get(at..end) {
            Some([
                Event::Campaign(CampaignEvent::Request(update)),
                Event::Campaign(CampaignEvent::Approve(approved, accepted)),
                Event::Campaign(CampaignEvent::Promote(promoted)),
            ]) if update == request.update()
                && *approved == update.operation()
                && *promoted == update.operation()
                && *accepted == history.accepted_newly_reviewable() => {}
            _ => return Err(Error::Binding.into()),
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
