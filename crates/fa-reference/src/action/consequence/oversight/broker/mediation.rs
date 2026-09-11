//! Topology controls delegate to the SAME lower delivery owner used by both keys.
use super::OversightBroker;
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::{AuthorityGraph, CutCheck, CutProposal, VerifiedCut};
use crate::Error;

impl OversightBroker {
    pub fn enable_mediation(&mut self, graph: AuthorityGraph) -> Result<(), Error> {
        self.delivery.enable_mediation(graph)
    }
    pub fn mediation_graph(&self) -> Option<&AuthorityGraph> { self.delivery.mediation_graph() }
    pub fn mediation_cut(&self) -> Option<&VerifiedCut> { self.delivery.mediation_cut() }
    pub fn certify_mediation(
        &mut self, generation: u64, epoch: u64, proposal: &CutProposal, budget: usize,
    ) -> Result<CutCheck, Error> { self.delivery.certify_mediation(generation, epoch, proposal, budget) }
    pub fn replace_mediation(
        &mut self, generation: u64, epoch: u64, graph: AuthorityGraph,
    ) -> Result<TopologyChange, Error> {
        let change = self.delivery.replace_mediation(generation, epoch, graph)?;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(change)
    }
    pub fn withdraw_mediation(&mut self, generation: u64, epoch: u64) -> Result<Option<TopologyChange>, Error> {
        let change = self.delivery.withdraw_mediation(generation, epoch)?;
        if change.is_some() { for slot in self.inputs.values_mut() { slot.approved = None; } }
        Ok(change)
    }
    pub fn delivery_mediation(&self, attempt: u64) -> Result<Option<&VerifiedCut>, Error> {
        self.delivery.delivery_mediation(attempt)
    }
}
