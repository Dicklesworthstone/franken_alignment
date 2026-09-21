//! Choose lookup semantics before any work; never reinterpret retained history.
use super::{DeliveryBroker, Error, InvalidationIndex, RoutingLimits, MAX_WITNESSES};
use crate::witness::refinement::index::routing::RoutingStrategy;

impl DeliveryBroker {
    /// Select the bounded subtree strategy before any proposal or change record.
    /// Limits, source identity, original dependencies and exact validation do not
    /// change. There is no live switch back or silent upgrade of a legacy owner.
    pub fn enable_publication_subtree_routing(&mut self) -> Result<(), Error> {
        let control = self.inspect();
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let state = gate.changes.as_mut().ok_or(Error::Incomplete)?;
        if state.index.strategy() == RoutingStrategy::SubtreeV2 { return Err(Error::Duplicate); }
        if !gate.slots.is_empty() || control.sequence != 0 || state.index.registered() != 0
            || state.last.is_some() || state.status.through != state.policy.after
            || state.status.observed_through != state.policy.after || state.status.unavailable {
            return Err(Error::WrongState);
        }
        let next = InvalidationIndex::new_with_strategy(RoutingLimits {
            judgments: gate.limits.bindings, dependencies: gate.limits.bindings * MAX_WITNESSES,
        }, RoutingStrategy::SubtreeV2)?;
        state.index = next;
        Ok(())
    }

    /// The configured traversal/cost version, not an input-validity certificate.
    pub fn publication_routing_strategy(&self) -> Result<RoutingStrategy, Error> {
        let state = self.publication.as_ref().and_then(|gate| gate.changes.as_ref()).ok_or(Error::Incomplete)?;
        Ok(state.index.strategy())
    }
}
