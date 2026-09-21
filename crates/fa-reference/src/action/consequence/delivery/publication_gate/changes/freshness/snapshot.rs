//! Exact current-snapshot fallback for a retained gap; never fake a feed prefix.
//! Only the existing snapshot/whole-input witness types are supported. Every
//! authorization, dispatch and publication still runs their full bounded check.
use super::{ChangeState, DeliveryBroker, FreshnessState, PublicationHeartbeat};
use super::super::{PublicationChangeStatus, PublicationInputCut};
use crate::action::ElapsedTick;
use crate::Error;

impl DeliveryBroker {
    /// Bootstrap-only alternative to refusing ALL work after feed retention loss.
    /// Requires an independently configured heartbeat lease and cut-bound source
    /// captures. Ordinary unbound inputs and legacy sources cannot use it.
    /// No prefix reset, fabricated notice, budget increase or review rebase occurs.
    pub fn enable_publication_snapshot_fallback(&mut self) -> Result<(), Error> {
        let control = self.inspect();
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        if !gate.slots.is_empty() || control.sequence != 0 { return Err(Error::WrongState); }
        let freshness = gate.changes.as_mut().and_then(|state| state.freshness.as_mut())
            .ok_or(Error::Incomplete)?;
        if freshness.snapshot_fallback { return Err(Error::Duplicate); }
        freshness.snapshot_fallback = true;
        // Enabling the profile cannot activate a previously refused observation.
        freshness.withdraw();
        Ok(())
    }

    pub fn publication_snapshot_fallback_enabled(&self) -> Result<bool, Error> {
        let state = self.publication.as_ref().and_then(|gate| gate.changes.as_ref())
            .ok_or(Error::Incomplete)?;
        Ok(state.freshness.as_ref().is_some_and(|freshness| freshness.snapshot_fallback))
    }

    pub(in crate::action::consequence::delivery::publication_gate) fn snapshot_cut_available(
        &self, cut: PublicationInputCut) -> Result<(), Error>
    {
        self.publication.as_ref().and_then(|gate| gate.changes.as_ref()).ok_or(Error::Incomplete)?
            .snapshot_current(self.inspect().ledger.elapsed, self.epoch, Some(cut))
    }
}

impl ChangeState {
    pub(in crate::action::consequence::delivery::publication_gate) fn snapshot_current(
        &self, now: Option<ElapsedTick>, epoch: u64, cut: Option<PublicationInputCut>) -> Result<(), Error>
    {
        // Do not use this path for an allocation-failed/unavailable index, a
        // complete-but-stale lease, or a snapshot preceding the latest known head.
        if self.status.unavailable || self.status.through >= self.status.observed_through {
            return Err(Error::Incomplete);
        }
        let cut = cut.ok_or(Error::Incomplete)?;
        if cut.source != self.status.source { return Err(Error::Binding); }
        if cut.through != self.status.observed_through { return Err(Error::Incomplete); }
        let freshness = self.freshness.as_ref().ok_or(Error::Incomplete)?;
        if !freshness.snapshot_fallback || freshness.conflicted { return Err(Error::Incomplete); }
        if freshness.refusal != Some(Error::Incomplete) {
            return Err(freshness.refusal.unwrap_or(Error::Incomplete));
        }
        let acquired = freshness.snapshot_epoch.ok_or(Error::Incomplete)?;
        if acquired != epoch { return Err(Error::Stale); }
        let heartbeat = freshness.last.ok_or(Error::Incomplete)?;
        if heartbeat.through != cut.through { return Err(Error::Incomplete); }
        freshness.snapshot_time(heartbeat, now)
    }
}

impl FreshnessState {
    pub(super) fn observe_snapshot_candidate(&mut self, heartbeat: PublicationHeartbeat,
        now: Option<ElapsedTick>, epoch: u64, status: PublicationChangeStatus)
    {
        self.snapshot_epoch = None;
        if self.snapshot_fallback && !self.conflicted && self.refusal == Some(Error::Incomplete)
            && !status.unavailable && status.through < status.observed_through
            && self.last == Some(heartbeat) && heartbeat.through == status.observed_through
            && self.snapshot_time(heartbeat, now).is_ok() {
            self.snapshot_epoch = Some(epoch);
        }
    }

    fn snapshot_time(&self, heartbeat: PublicationHeartbeat, now: Option<ElapsedTick>) -> Result<(), Error> {
        let now = now.ok_or(Error::Incomplete)?;
        let expires = heartbeat.produced_at.0.checked_add(self.policy.max_age_ticks).ok_or(Error::Overflow)?;
        if now < heartbeat.produced_at || now.0 >= expires { return Err(Error::Stale); }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
