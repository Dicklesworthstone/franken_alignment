//! Persist native topology inputs and consume the original mediation gate.
//! A declared graph cut is not proof of operating-system containment.
mod codec;
pub mod planning;
pub(super) use codec::{read, write};

use super::{Event, FileOversight, JournalError, Transition};
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::{AuthorityGraph, CutCheck, CutProposal, VerifiedCut,
    MAX_CUT_NODES, MAX_NODES, MAX_CHECK_EDGE_VISITS};
use crate::Error;
use std::rc::Rc;

pub const MAX_TOPOLOGY_UPDATES: usize = 128;

/// Independent topology observation, not a replacement authority or verdict.
/// None withdraws coverage. Reinstatement requires newer graph AND inventory
/// generations; an old cut cannot make the unavailable graph live again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediationUpdate {
    pub operation: u64,
    pub expected_generation: u64,
    pub expected_authority_epoch: u64,
    pub next: Option<AuthorityGraph>,
}

/// Last registered graph plus CURRENT availability and native accepted cut.
/// A rejected candidate does not invalidate an earlier cut of the same immutable
/// graph. A topology change clears both the cut and this latest check result.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::mediation::FileMediationSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn grant(evidence: FileMediationSnapshot) -> FilePermit { evidence }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediationSnapshot {
    pub journal_revision: u64,
    pub graph: AuthorityGraph,
    pub available: bool,
    pub accepted: Option<VerifiedCut>,
    pub last_check: Option<Result<CutCheck, Error>>,
    pub retained_updates: usize,
}

/// Separate custody, not authenticated observer identity. No actor port receives
/// this role, and there is no live-owner getter or cloning implementation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::mediation::FileMediationObserver;
/// fn copy(role: FileMediationObserver) { let _ = role.clone(); }
/// ```
#[derive(Debug)]
pub struct FileMediationObserver { pub(super) issuer: Rc<()> }

#[derive(Clone)]
pub(super) enum MediationEvent {
    Enable(AuthorityGraph),
    Check { generation: u64, epoch: u64, gates: Vec<u64>, reachable: Vec<u64>, budget: usize },
    Update(FileMediationUpdate),
}

impl FileOversight {
    /// Before any proposal or external request. First publication is guarded;
    /// the graph starts uncertified and cannot authorize even a proposal yet.
    pub fn enable_mediation(&mut self, revision: u64, graph: AuthorityGraph)
        -> Result<FileMediationObserver, JournalError>
    {
        self.transact(revision, Event::Mediation(MediationEvent::Enable(graph)))?;
        Ok(FileMediationObserver { issuer: Rc::clone(&self.issuer) })
    }
    pub fn mediation_snapshot(&self) -> Result<FileMediationSnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.mediation_snapshot(self.revision())?)
    }
    /// The original cut bound to an actual dispatch, not today's replacement.
    pub fn delivery_mediation(&self, attempt: u64) -> Result<Option<&VerifiedCut>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.delivery_mediation(attempt)?)
    }
    pub fn mediation_update(&self, operation: u64) -> Result<&Option<TopologyChange>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.mediation_update(operation)?)
    }
}

impl FileMediationObserver {
    fn check_owner(&self, host: &FileOversight) -> Result<(), JournalError> {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        Ok(())
    }

    /// Check the proposed partition with the ORIGINAL independent checker. An
    /// inner Err, bypass path or disconnected sink is a committed check result,
    /// never a certificate. There is no API accepting a caller's VerifiedCut.
    pub fn certify(&self, host: &mut FileOversight, revision: u64, epoch: u64,
        proposal: &CutProposal, edge_budget: usize) -> Result<Result<CutCheck, Error>, JournalError>
    {
        self.check_owner(host)?;
        if proposal.gates.len() > MAX_CUT_NODES || proposal.reachable.len() > MAX_NODES
            || edge_budget > MAX_CHECK_EDGE_VISITS { return Err(Error::Limit.into()); }
        if &host.machine.mediation_snapshot(host.revision())?.graph != &proposal.graph {
            return Err(Error::Binding.into());
        }
        let event = MediationEvent::Check { generation: proposal.graph.spec().generation, epoch,
            gates: proposal.gates.clone(), reachable: proposal.reachable.clone(), budget: edge_budget };
        match host.transact(revision, Event::Mediation(event))? {
            Transition::MediationChecked(result) => Ok(*result),
            _ => unreachable!("native cut verification"),
        }
    }

    /// Accepted topology change closes the old admission path BEFORE encoding
    /// or storage can fail. Only acknowledgment clears that live-owner latch.
    /// An exact committed retry returns its old receipt without another fence.
    pub fn update(&self, host: &mut FileOversight, revision: u64, update: &FileMediationUpdate)
        -> Result<Option<TopologyChange>, JournalError>
    {
        self.check_owner(host)?;
        if let Some(receipt) = host.machine.mediation_retry(update)? { return Ok(receipt.clone()); }
        host.transact(revision, Event::Mediation(MediationEvent::Update(update.clone())))?;
        Ok(host.mediation_update(update.operation)?.clone())
    }
}

#[cfg(test)]
mod tests;
