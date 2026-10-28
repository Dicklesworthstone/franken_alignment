//! Restricted terminal disposition after the SAME complete learned replay.
//! Numerical state, source custody and charged exposure never rewind.
use super::{FileLearnedRecovery, JournalError};
use crate::action::consequence::delivery::persistent::{
    abandonment::FileAbandonRequest,
    observed::{
        driver::topology::checkpoint::TopologyCheckpoint,
        guarded::recovery::FileAbandonedHistory,
    },
};

impl FileLearnedRecovery {
    /// Record the original Unknown -> IrrecoverablyUnknown transition without
    /// recovering a live owner, acquiring a clock or issuing a recovery Fence.
    /// Complete original numerical replay and canonical-byte checks precede
    /// the single ordinary-capacity event. Pending numerical/source operations
    /// remain pending; no effect is sent or queried, no rights are refunded and
    /// no stream successor becomes eligible.
    ///
    /// Exact retries return the original receipt and current acknowledged cut
    /// without another event, including at physical capacity. Conflicting
    /// requests, stale predecessors and non-Unknown attempts refuse. A storage
    /// failure exposes no candidate receipt or anchor even when a replacement
    /// may already have become visible. Independent guard wrappers call this
    /// only after rechecking their own complete original requirements.
    pub fn finish_abandoned_with_anchor(
        self,
        request: FileAbandonRequest,
    ) -> Result<FileAbandonedHistory, JournalError> {
        self.verified_guarded_machine()?;
        let Self {
            profile,
            store,
            canonical,
            events,
            machine,
            ..
        } = self;
        FileAbandonedHistory::finish_verified(profile, store, canonical, events, machine, request)
    }

    /// Require independently retained source custody before cleanup or mutation.
    /// Only the exact saved cut or ONE matching AbandonUnknown successor is
    /// accepted; the latter permits an exact retry after interrupted export.
    /// Unrelated work cannot borrow the old source expenditure. The original
    /// topology availability, documents, work and last operation do not change.
    ///
    /// Retain the acknowledged history and use after_abandoned_history to obtain
    /// its matching source checkpoint. Archive retention is separate from the
    /// durable disposition and cannot trigger an effect retry or a refund.
    pub fn finish_abandoned_with_checkpoint(
        self,
        request: FileAbandonRequest,
        saved: &TopologyCheckpoint,
    ) -> Result<FileAbandonedHistory, JournalError> {
        let machine = self.verified_guarded_machine()?;
        saved.check_abandoned_start(
            &self.profile,
            self.store.identity(),
            &self.events,
            &machine.snapshot(self.events.len()),
            request,
        )?;
        self.finish_abandoned_with_anchor(request)
    }
}
