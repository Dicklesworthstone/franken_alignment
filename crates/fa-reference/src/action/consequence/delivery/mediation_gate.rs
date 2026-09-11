//! The live reference delivery path consumes verified topology, not graph prose.

use super::DeliveryBroker;
use crate::action::consequence::mediation::{AuthorityGraph, CutCheck, CutProposal, VerifiedCut};
use crate::action::ActionState;
use crate::Error;

#[derive(Debug)]
pub(super) struct MediationState {
    graph: AuthorityGraph,
    accepted: Option<VerifiedCut>,
    unavailable: bool,
}

/// A returned immutable transition receipt. Retaining the journal is the
/// caller's responsibility; this in-memory profile supplies no durable journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyChange {
    pub previous: AuthorityGraph,
    pub current: Option<AuthorityGraph>,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

impl DeliveryBroker {
    /// Trusted bootstrap, before any attempt. There is no later disable method.
    /// A configured but uncertified graph prevents proposals and dispatch.
    pub fn enable_mediation(&mut self, graph: AuthorityGraph) -> Result<(), Error> {
        if self.mediation.is_some() { return Err(Error::Duplicate); }
        let inspection = self.inspect();
        if !inspection.ledger.stages.is_empty() || inspection.sequence != 0 { return Err(Error::WrongState); }
        if !graph.binds(self.scope, self.resource) { return Err(Error::Binding); }
        self.mediation = Some(MediationState { graph, accepted: None, unavailable: false });
        Ok(())
    }

    /// None means absent/unavailable, not an empty complete graph.
    pub fn mediation_graph(&self) -> Option<&AuthorityGraph> {
        self.mediation.as_ref().filter(|state| !state.unavailable).map(|state| &state.graph)
    }

    pub fn mediation_cut(&self) -> Option<&VerifiedCut> {
        self.mediation.as_ref().and_then(|state| state.accepted.as_ref())
    }

    /// Explicit trusted activation. Every candidate is independently checked
    /// against the currently owned graph; an old VerifiedCut is not accepted as
    /// a caller assertion. This never restores a cancelled attempt or old epoch.
    pub fn certify_mediation(
        &mut self, expected_generation: u64, expected_epoch: u64,
        proposal: &CutProposal, edge_budget: usize,
    ) -> Result<CutCheck, Error> {
        self.mediation_predecessor(expected_generation, expected_epoch)?;
        let state = self.mediation.as_ref().expect("checked mediation");
        if state.unavailable { return Err(Error::Incomplete); }
        let result = state.graph.verify_cut(proposal, edge_budget)?;
        if let CutCheck::Verified(cut) = &result {
            self.mediation.as_mut().expect("checked mediation").accepted = Some(cut.clone());
        }
        Ok(result)
    }

    /// Publish a new trusted inventory even when it has a bypass or unknown
    /// completeness. Such a change must close the old admission path, not fail
    /// certification while accidentally keeping the old graph active.
    /// All undispatched attempts are cancelled; dispatched liability is intact.
    pub fn replace_mediation(
        &mut self, expected_generation: u64, expected_epoch: u64, next: AuthorityGraph,
    ) -> Result<TopologyChange, Error> {
        self.mediation_predecessor(expected_generation, expected_epoch)?;
        let state = self.mediation.as_ref().expect("checked mediation");
        if !next.binds(self.scope, self.resource)
            || next.spec().family.scope != state.graph.spec().family.scope
            || next.spec().family.family != state.graph.spec().family.family { return Err(Error::Binding); }
        if next.spec().generation <= state.graph.spec().generation
            || next.spec().inventory_generation <= state.graph.spec().inventory_generation { return Err(Error::Stale); }
        self.transition_mediation(Some(next))
    }

    /// Capture loss withdraws the entire topology prerequisite. Its prior bytes
    /// remain historical, but cannot be recertified until a NEW inventory is
    /// explicitly installed. Repeated loss is an idempotent no-op at the current
    /// predecessor and cannot consume extra revocation floors or refund twice.
    pub fn withdraw_mediation(
        &mut self, expected_generation: u64, expected_epoch: u64,
    ) -> Result<Option<TopologyChange>, Error> {
        self.mediation_predecessor(expected_generation, expected_epoch)?;
        if self.mediation.as_ref().expect("checked mediation").unavailable { return Ok(None); }
        self.transition_mediation(None).map(Some)
    }

    /// The exact cut consumed by this earlier dispatch. Topology updates and
    /// capture loss do not rewrite it, and no graph bytes enter the endpoint.
    pub fn delivery_mediation(&self, attempt: u64) -> Result<Option<&VerifiedCut>, Error> {
        Ok(self.records.get(&attempt).ok_or(Error::Missing)?.mediation.as_ref())
    }

    pub(crate) fn check_mediation(&self) -> Result<(), Error> {
        if let Some(state) = &self.mediation {
            if state.unavailable { return Err(Error::Incomplete); }
            let accepted = state.accepted.as_ref().ok_or(Error::Incomplete)?;
            if !std::ptr::eq(accepted.graph().spec(), state.graph.spec()) || !state.graph.binds(self.scope, self.resource) { return Err(Error::Stale); }
        }
        Ok(())
    }

    fn mediation_predecessor(&self, generation: u64, epoch: u64) -> Result<(), Error> {
        let state = self.mediation.as_ref().ok_or(Error::Incomplete)?;
        if state.graph.spec().generation != generation || self.inspect().ledger.epoch != epoch {
            return Err(Error::Stale);
        }
        Ok(())
    }

    fn transition_mediation(&mut self, next: Option<AuthorityGraph>) -> Result<TopologyChange, Error> {
        let inspection = self.inspect();
        let floor = inspection.ledger.epoch.checked_add(1).ok_or(Error::Overflow)?;
        let cancelled: Vec<_> = inspection.ledger.stages.iter().filter_map(|(id, stage)| {
            matches!(stage, ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized)
                .then_some(*id)
        }).collect();
        let change = TopologyChange {
            previous: self.mediation.as_ref().ok_or(Error::Incomplete)?.graph.clone(),
            current: next.clone(), revocation_floor: floor, cancelled,
            refunded_units: inspection.ledger.reserved,
        };
        // No clock/callback/other writer can run between these steps. The owning
        // ledger checks epoch overflow before mutation; each selected attempt
        // is known to be cancellable. Existing effect accounting is reused.
        self.controller.revoke_epoch()?;
        for id in &change.cancelled {
            self.controller.cancel(*id).expect("prevalidated undispatched attempt");
        }
        let state = self.mediation.as_mut().expect("checked mediation");
        state.accepted = None;
        state.unavailable = next.is_none();
        if let Some(next) = next { state.graph = next; }
        Ok(change)
    }
}
