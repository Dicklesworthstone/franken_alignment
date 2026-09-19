//! Clock-bounded coverage for the original invalidation feed (FA-061/062).
//! Heartbeats are producer observations, not signatures or wall-clock truth.
//! Expiry is anchored to the producer's original tick, NEVER to reread time.
use super::{ChangeState, DeliveryBroker};
use crate::action::ElapsedTick;
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationFreshnessPolicy {
    /// Independently configured elapsed-clock domain shared with the producer.
    pub clock_domain: u64,
    pub max_age_ticks: u64,
}
impl PublicationFreshnessPolicy {
    pub fn check(self) -> Result<(), Error> {
        if self.clock_domain == 0 || self.max_age_ticks == 0 { return Err(Error::InvalidInput); }
        Ok(())
    }
}

/// A producer's closed observation of the feed, not a replacement for its
/// missing changes. Increment generation whenever ANY field changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationHeartbeat {
    pub source: u64,
    pub clock_domain: u64,
    pub generation: u64,
    pub through: u64,
    pub produced_at: ElapsedTick,
}

/// Current eligibility is recomputed at the owner's clock and dispatcher epoch.
/// A copied status is historical data; no authorization operation accepts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationFreshnessStatus {
    pub policy: PublicationFreshnessPolicy,
    pub heartbeat: Option<PublicationHeartbeat>,
    pub acquired_epoch: Option<u64>,
    pub eligibility: Result<(), Error>,
}

#[derive(Debug)]
pub(super) struct FreshnessState {
    policy: PublicationFreshnessPolicy,
    last: Option<PublicationHeartbeat>,
    acquired_epoch: Option<u64>,
    refusal: Option<Error>,
    // Once two contents claim the same generation, an old quiet copy cannot
    // rehabilitate that generation. Only a monotonic NEW generation can recover.
    conflicted: bool,
}
impl FreshnessState {
    fn new(policy: PublicationFreshnessPolicy) -> Self {
        Self { policy, last: None, acquired_epoch: None, refusal: None, conflicted: false }
    }
    fn withdraw(&mut self) {
        self.acquired_epoch = None;
        self.refusal = None;
        // Retain exact producer generation/tick/coverage floors and conflict.
    }
    fn current(&self, now: Option<ElapsedTick>, epoch: u64, through: u64) -> Result<(), Error> {
        if let Some(error) = self.refusal { return Err(error); }
        let acquired = self.acquired_epoch.ok_or(Error::Incomplete)?;
        // Recovery/fencing changes the existing dispatcher epoch. A saved
        // heartbeat is not a new observation even if its time window is unexpired.
        if acquired != epoch { return Err(Error::Stale); }
        let heartbeat = self.last.ok_or(Error::Incomplete)?;
        if heartbeat.through != through { return Err(Error::Incomplete); }
        let now = now.ok_or(Error::Incomplete)?;
        let expires = heartbeat.produced_at.0.checked_add(self.policy.max_age_ticks).ok_or(Error::Overflow)?;
        if now < heartbeat.produced_at || now.0 >= expires { return Err(Error::Stale); }
        Ok(())
    }
}

impl ChangeState {
    pub(in super::super) fn current(&self, now: Option<ElapsedTick>, epoch: u64) -> Result<(), Error> {
        if !self.complete() { return Err(Error::Incomplete); }
        self.freshness.as_ref().map_or(Ok(()), |freshness| freshness.current(now, epoch, self.status.through))
    }
    fn freshness_status(&self, now: Option<ElapsedTick>, epoch: u64) -> Result<PublicationFreshnessStatus, Error> {
        let freshness = self.freshness.as_ref().ok_or(Error::Incomplete)?;
        Ok(PublicationFreshnessStatus { policy: freshness.policy, heartbeat: freshness.last,
            acquired_epoch: freshness.acquired_epoch, eligibility: self.current(now, epoch) })
    }
    fn observe_heartbeat(&mut self, heartbeat: PublicationHeartbeat, now: Option<ElapsedTick>, epoch: u64)
        -> Result<PublicationFreshnessStatus, Error>
    {
        if heartbeat.source != self.status.source { return Err(Error::Binding); }
        let freshness = self.freshness.as_mut().ok_or(Error::Incomplete)?;
        freshness.withdraw();
        // A recognized observation's REFUSAL is retained, rather than rewinding
        // to a previously permitting candidate. The return status carries it.
        let accepted = (|| {
            if heartbeat.generation == 0 { return Err(Error::InvalidInput); }
            if heartbeat.clock_domain != freshness.policy.clock_domain { return Err(Error::Binding); }
            if let Some(old) = freshness.last {
                if heartbeat.generation < old.generation { return Err(Error::Stale); }
                if heartbeat.generation == old.generation {
                    if heartbeat != old || freshness.conflicted {
                        freshness.conflicted = true;
                        return Err(Error::Binding);
                    }
                } else if heartbeat.produced_at < old.produced_at || heartbeat.through < old.through {
                    return Err(Error::Stale);
                }
            }
            freshness.last = Some(heartbeat);
            freshness.conflicted = false;
            // Seeing the producer ahead exposes a missing tail; it does not
            // advance the complete prefix or synthesize omitted notifications.
            self.status.observed_through = self.status.observed_through.max(heartbeat.through);
            if !self.status.complete() { return Err(Error::Incomplete); }
            if heartbeat.through < self.status.through { return Err(Error::Stale); }
            let now = now.ok_or(Error::Incomplete)?;
            let expires = heartbeat.produced_at.0.checked_add(freshness.policy.max_age_ticks).ok_or(Error::Overflow)?;
            if now < heartbeat.produced_at || now.0 >= expires { return Err(Error::Stale); }
            freshness.acquired_epoch = Some(epoch);
            Ok(())
        })();
        freshness.refusal = accepted.err();
        self.freshness_status(now, epoch)
    }
}

impl DeliveryBroker {
    /// Install after change routing, before any proposal. No disable, age
    /// extension, clock replacement or generation reset is available.
    pub fn enable_publication_change_freshness(&mut self, policy: PublicationFreshnessPolicy) -> Result<(), Error> {
        policy.check()?;
        let control = self.inspect();
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let state = gate.changes.as_mut().ok_or(Error::Incomplete)?;
        if state.freshness.is_some() { return Err(Error::Duplicate); }
        if !gate.slots.is_empty() || control.sequence != 0 { return Err(Error::WrongState); }
        state.freshness = Some(FreshnessState::new(policy));
        Ok(())
    }
    pub fn publication_change_freshness(&self) -> Result<PublicationFreshnessStatus, Error> {
        let state = self.publication.as_ref().and_then(|gate| gate.changes.as_ref()).ok_or(Error::Incomplete)?;
        state.freshness_status(self.inspect().ledger.elapsed, self.epoch)
    }
    /// Before any producer I/O, withdraw feed eligibility, not effect rights.
    /// This does not erase an incomplete tail or any original evidence binding.
    pub fn publication_changes_unavailable(&mut self, source: u64) -> Result<(), Error> {
        let state = self.publication.as_mut().and_then(|gate| gate.changes.as_mut()).ok_or(Error::Incomplete)?;
        if state.status.source != source { return Err(Error::Binding); }
        state.freshness.as_mut().ok_or(Error::Incomplete)?.withdraw();
        Ok(())
    }
    /// Trusted producer observation in the native reference profile. The durable
    /// acquisition adapter owns actual I/O. Inspect status.eligibility: Ok(status)
    /// may acknowledge a restrictive, expired, future or incomplete heartbeat.
    pub fn record_publication_heartbeat(&mut self, heartbeat: PublicationHeartbeat)
        -> Result<PublicationFreshnessStatus, Error>
    {
        let now = self.inspect().ledger.elapsed;
        let epoch = self.epoch;
        self.publication.as_mut().and_then(|gate| gate.changes.as_mut()).ok_or(Error::Incomplete)?
            .observe_heartbeat(heartbeat, now, epoch)
    }
}

#[cfg(test)]
mod tests;
