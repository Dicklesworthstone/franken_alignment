//! The same broker owns graph certification, epoch revocation and all rights.
use super::{Machine, Transition};
use super::super::mediation::{FileMediationSnapshot, FileMediationUpdate, MediationEvent, MAX_TOPOLOGY_UPDATES};
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::{AuthorityGraph, CutCheck, CutProposal};
use crate::Error;
use std::collections::BTreeMap;

pub(super) struct MediationState {
    graph: AuthorityGraph,
    last_check: Option<Result<CutCheck, Error>>,
    updates: BTreeMap<u64, (FileMediationUpdate, Option<TopologyChange>)>,
}
impl Machine {
    pub(in super::super) fn mediation_snapshot(&self, revision: u64) -> Result<FileMediationSnapshot, Error> {
        let state = self.mediation.as_ref().ok_or(Error::Incomplete)?;
        Ok(FileMediationSnapshot { journal_revision: revision, graph: state.graph.clone(),
            available: self.broker.mediation_graph().is_some(), accepted: self.broker.mediation_cut().cloned(),
            last_check: state.last_check.clone(), retained_updates: state.updates.len() })
    }
    pub(in super::super) fn mediation_update(&self, operation: u64) -> Result<&Option<TopologyChange>, Error> {
        Ok(&self.mediation.as_ref().ok_or(Error::Incomplete)?.updates.get(&operation).ok_or(Error::Missing)?.1)
    }
    pub(in super::super) fn mediation_retry(&self, update: &FileMediationUpdate)
        -> Result<Option<&Option<TopologyChange>>, Error>
    {
        let state = self.mediation.as_ref().ok_or(Error::Incomplete)?;
        match state.updates.get(&update.operation) {
            Some((previous, receipt)) if previous == update => Ok(Some(receipt)),
            Some(_) => Err(Error::Binding), None => Ok(None),
        }
    }

    /// Structural and predecessor checks only, before the live-owner latch.
    /// Certification and rights mutation remain the original broker's job.
    pub(in super::super) fn preflight_mediation_update(&self, update: &FileMediationUpdate) -> Result<(), Error> {
        let state = self.mediation.as_ref().ok_or(Error::Incomplete)?;
        if update.operation == 0 { return Err(Error::InvalidInput); }
        if state.updates.contains_key(&update.operation) { return Err(Error::Duplicate); }
        if state.updates.len() >= MAX_TOPOLOGY_UPDATES { return Err(Error::Limit); }
        let before = state.graph.spec();
        let control = self.broker.inspect();
        if update.expected_generation != before.generation || update.expected_authority_epoch != control.ledger.epoch {
            return Err(Error::Stale);
        }
        if let Some(next) = &update.next {
            if !next.binds(self.scope, before.target) || next.spec().family.scope != before.family.scope
                || next.spec().family.family != before.family.family { return Err(Error::Binding); }
            if next.spec().generation <= before.generation || next.spec().inventory_generation <= before.inventory_generation {
                return Err(Error::Stale);
            }
        }
        if update.next.is_some() || self.broker.mediation_graph().is_some() {
            control.ledger.epoch.checked_add(1).ok_or(Error::Overflow)?;
        }
        Ok(())
    }

    pub(super) fn apply_mediation(&mut self, event: &MediationEvent) -> Result<Transition, Error> {
        match event {
            MediationEvent::Enable(graph) => {
                if self.mediation.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                self.broker.enable_mediation(graph.clone())?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.mediation = Some(MediationState { graph: graph.clone(), last_check: None, updates: BTreeMap::new() });
            }
            MediationEvent::Check { generation, epoch, gates, reachable, budget } => {
                let graph = self.mediation.as_ref().ok_or(Error::Incomplete)?.graph.clone();
                let proposal = CutProposal { graph, gates: gates.clone(), reachable: reachable.clone() };
                let result = self.broker.certify_mediation(*generation, *epoch, &proposal, *budget);
                self.mediation.as_mut().expect("configured topology").last_check = Some(result.clone());
                return Ok(Transition::MediationChecked(Box::new(result)));
            }
            MediationEvent::Update(update) => {
                self.preflight_mediation_update(update)?;
                let change = match &update.next {
                    Some(next) => Some(self.broker.replace_mediation(update.expected_generation,
                        update.expected_authority_epoch, next.clone())?),
                    None => self.broker.withdraw_mediation(update.expected_generation, update.expected_authority_epoch)?,
                };
                if change.is_some() {
                    self.withdraw_identity()?;
                    self.withdraw_policy_campaigns()?;
                    self.withdraw_keys()?;
                    self.sessions.clear();
                    self.automatic.clear();
                    // Keep sendable envelopes so the ORIGINAL guarded endpoint
                    // can seal old dispatches; no unknown liability is refunded.
                }
                let state = self.mediation.as_mut().expect("configured topology");
                if let Some(next) = &update.next { state.graph = next.clone(); }
                state.last_check = None;
                state.updates.insert(update.operation, (update.clone(), change));
            }
        }
        Ok(Transition::Unit)
    }

    pub(super) fn recover_mediation(&mut self) -> Result<(), Error> {
        if let Some(state) = &mut self.mediation {
            let generation = state.graph.spec().generation;
            let epoch = self.broker.inspect().ledger.epoch;
            self.broker.withdraw_mediation(generation, epoch)?;
            state.last_check = None;
        }
        Ok(())
    }
}
