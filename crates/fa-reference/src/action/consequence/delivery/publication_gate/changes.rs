//! Change-feed invalidation in the ORIGINAL publication owner. Notifications
//! withdraw current observations; they never replace reviewed requirements or
//! excuse exact final-cut validation. A known missing tail blocks publication.
pub mod freshness;
pub use super::source::{PublicationInputCut, PublicationInputCutStatus};
use super::{DeliveryBroker, PublicationGate, PublicationJudgment, MAX_PUBLICATION_BINDINGS};
use crate::witness::refinement::index::routing::{InvalidationIndex, RoutingBudget, RoutingLimits, WitnessChange};
use crate::witness::MAX_WITNESSES;
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationChangePolicy {
    pub source: u64,
    /// Independently declared bootstrap cut, not inferred from the first notice.
    pub after: u64,
    pub lookup: RoutingBudget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationChange {
    pub source: u64,
    /// Exactly one change record at this sequence. Multi-key changes can use a
    /// covering Range, Domain or All; never drop changes to fit a smaller record.
    pub sequence: u64,
    pub change: WitnessChange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationChangeStatus {
    pub source: u64,
    pub through: u64,
    pub observed_through: u64,
    pub unavailable: bool,
}
impl PublicationChangeStatus {
    pub fn complete(self) -> bool { !self.unavailable && self.through == self.observed_through }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeRouting {
    Indexed,
    /// Missing predecessors are NOT covered by a later notification.
    MissingTail,
    /// Recovery from missing coverage withdraws ALL observations again, including
    /// observations captured while the missing tail was being repaired.
    RecoveringTail,
    /// Includes exhausted lookup budgets and malformed range notifications.
    Conservative(Error),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationChangeReport {
    pub status: PublicationChangeStatus,
    pub routing: ChangeRouting,
    pub affected: Vec<u64>,
    pub spent: RoutingBudget,
}

#[derive(Debug)]
pub(super) struct ChangeState {
    policy: PublicationChangePolicy,
    status: PublicationChangeStatus,
    index: InvalidationIndex,
    last: Option<Rc<PublicationChangeReport>>,
    freshness: Option<freshness::FreshnessState>,
}
impl ChangeState {
    pub(super) fn complete(&self) -> bool { self.status.complete() }
    pub(super) fn register(&mut self, attempt: u64, judgment: &PublicationJudgment) -> Result<(), Error> {
        judgment.register_invalidation(attempt, &mut self.index)
    }
}

impl DeliveryBroker {
    /// One bootstrap profile, before every proposal. No later disable, source
    /// replacement, sequence reset or budget widening exists for this owner.
    pub fn enable_publication_changes(&mut self, policy: PublicationChangePolicy) -> Result<(), Error> {
        if policy.source == 0 { return Err(Error::InvalidInput); }
        let control = self.inspect();
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        if gate.changes.is_some() { return Err(Error::Duplicate); }
        if !gate.slots.is_empty() || control.sequence != 0 { return Err(Error::WrongState); }
        let index = InvalidationIndex::new(RoutingLimits {
            judgments: gate.limits.bindings, dependencies: gate.limits.bindings * MAX_WITNESSES,
        })?;
        gate.changes = Some(ChangeState { policy, status: PublicationChangeStatus {
            source: policy.source, through: policy.after, observed_through: policy.after, unavailable: false,
        }, index, last: None, freshness: None });
        Ok(())
    }

    pub fn publication_change_status(&self) -> Result<PublicationChangeStatus, Error> {
        Ok(self.publication.as_ref().and_then(|gate| gate.changes.as_ref()).ok_or(Error::Incomplete)?.status)
    }
    pub fn publication_change_report(&self) -> Result<Option<Rc<PublicationChangeReport>>, Error> {
        Ok(self.publication.as_ref().and_then(|gate| gate.changes.as_ref()).ok_or(Error::Incomplete)?.last.clone())
    }
    /// Side-effect-free ownership/predecessor checks for a durable consumer.
    /// Out-of-order future notices pass: their coverage loss must be COMMITTED,
    /// not thrown away as a stale speculative candidate.
    pub fn preflight_publication_change(&self, notice: PublicationChange) -> Result<(), Error> {
        let state = self.publication_change_status()?;
        if notice.source != state.source { return Err(Error::Binding); }
        if notice.sequence <= state.through { return Err(Error::Stale); }
        Ok(())
    }
    pub fn record_publication_change(&mut self, notice: PublicationChange) -> Result<Rc<PublicationChangeReport>, Error> {
        self.preflight_publication_change(notice)?;
        self.publication.as_mut().ok_or(Error::Incomplete)?.apply_change(notice)
    }
}

impl PublicationGate {
    fn apply_change(&mut self, notice: PublicationChange) -> Result<Rc<PublicationChangeReport>, Error> {
        let state = self.changes.as_mut().ok_or(Error::Incomplete)?;
        let old = state.status;
        // Once a current-source notice is accepted, allocation/overflow/unwind
        // cannot leave the previous observations eligible in this live owner.
        state.status.unavailable = true;
        state.status.observed_through = old.observed_through.max(notice.sequence);
        let contiguous = old.through.checked_add(1) == Some(notice.sequence);
        let mut spent = RoutingBudget::default();
        let mut selected = [false; MAX_PUBLICATION_BINDINGS];
        let routing = if !contiguous {
            ChangeRouting::MissingTail
        } else if !old.complete() {
            ChangeRouting::RecoveringTail
        } else {
            let report = state.index.affected(notice.change, state.policy.lookup);
            spent = report.spent;
            match report.candidates {
                Ok(ids) => {
                    // At most 16 slots: bounded set union, no value comparisons.
                    for (position, (id, slot)) in self.slots.iter().enumerate() {
                        selected[position] = slot.judgment.is_none() || ids.contains(id);
                    }
                    ChangeRouting::Indexed
                }
                Err(error) => ChangeRouting::Conservative(error),
            }
        };
        if routing != ChangeRouting::Indexed { selected.fill(true); }
        let mut affected = Vec::new();
        affected.try_reserve_exact(self.slots.len()).map_err(|_| Error::Limit)?;
        for (position, (id, slot)) in self.slots.iter().enumerate() {
            if selected[position] {
                slot.revision.checked_add(1).ok_or(Error::Overflow)?;
                affected.push(*id);
            }
        }
        let status = PublicationChangeStatus { source: old.source,
            through: if contiguous { notice.sequence } else { old.through },
            observed_through: old.observed_through.max(notice.sequence), unavailable: false };
        let report = Rc::new(PublicationChangeReport { status, routing, affected, spent });
        for (position, slot) in self.slots.values_mut().enumerate() {
            if selected[position] {
                // None preserves snapshot and producer high-water marks. It
                // revokes source freshness, not either authority key or rights.
                slot.record_inputs(slot.revision, None).expect("preflighted withdrawal revision");
                slot.require_capture_through(status.observed_through);
                slot.last = None;
            }
        }
        state.status = status;
        state.last = Some(Rc::clone(&report));
        Ok(report)
    }
}
