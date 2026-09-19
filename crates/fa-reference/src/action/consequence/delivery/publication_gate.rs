//! Mandatory final-cut validation in the original delivery owner (FA-062).
//!
//! This bounded opt-in reference profile consumes host-supplied observations,
//! not authenticated adapter captures. Hosts must record current observations
//! (or unavailability) before publication. It makes no wall-clock freshness or
//! persistence claim. Once enabled, no dispatch entry point can skip the lane.

mod source;
pub use source::PublicationSourceStatus;
use super::DeliveryBroker;
use crate::Error;
use crate::action::{ActionState, FrozenAction};
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::action::consequence::oversight::publication::{
    PublicationBasis, PublicationJudgment, PublicationOutcome, PublicationReport,
};
use crate::full_input::ActualHelperInput;
use crate::product_frontier::ProductFrontiers;
use crate::witness::WitnessSnapshot;
use crate::witness::refinement::RefinementBudget;
use std::collections::BTreeMap;

/// Lifetime bound; cancelled attempts are not evicted to reopen capacity or
/// erase their requirements. Each input and judgment has its own type bounds.
pub const MAX_PUBLICATION_BINDINGS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationLimits {
    pub bindings: usize,
    /// Logical work available to each authorize/dispatch revalidation. Zero
    /// fails closed; callers cannot raise this ceiling during an attempt.
    pub validation: RefinementBudget,
}

/// An owned immutable observation, not a report asserting that validation ran.
/// None in either lane is preserved as missing evidence, never a narrower read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationInputs {
    pub structured: Option<(WitnessSnapshot, ProductFrontiers)>,
    pub opaque: Option<ActualHelperInput>,
}

impl PublicationInputs {
    fn basis(&self) -> PublicationBasis<'_> {
        PublicationBasis {
            structured: self.structured.as_ref().map(|(snapshot, frontiers)| (snapshot, frontiers)),
            opaque: self.opaque.as_ref(),
        }
    }
}

#[derive(Debug)]
struct Slot {
    action: FrozenAction,
    judgment: Option<PublicationJudgment>,
    revision: u64,
    current: Option<PublicationInputs>,
    // Unavailability must not erase a previously observed monotonic cut.
    floor: Option<(u64, u64, u64)>,
    last: Option<PublicationReport>,
    source: Option<source::SourceState>,
}

#[derive(Debug)]
pub(super) struct PublicationGate {
    limits: PublicationLimits,
    slots: BTreeMap<u64, Slot>,
}

impl DeliveryBroker {
    /// Trusted bootstrap only, before any proposal. There is no disable, limit
    /// increase, judgment replacement, or mutable accessor to this gate.
    pub fn enable_publication_validation(&mut self, limits: PublicationLimits) -> Result<(), Error> {
        if self.publication.is_some() { return Err(Error::Duplicate); }
        if limits.bindings == 0 || limits.bindings > MAX_PUBLICATION_BINDINGS { return Err(Error::Limit); }
        let state = self.inspect();
        if !state.ledger.stages.is_empty() || state.sequence != 0 { return Err(Error::WrongState); }
        self.publication = Some(PublicationGate { limits, slots: BTreeMap::new() });
        Ok(())
    }

    /// Attach the reviewed requirements once, to the exact controller-produced
    /// action (including its derived policy witnesses), before authorization.
    /// This trusted assertion does not itself run a helper or grant any rights.
    pub fn bind_publication_judgment(&mut self, attempt: u64, judgment: PublicationJudgment) -> Result<(), Error> {
        if self.inspect().ledger.stages.get(&attempt) != Some(&ActionState::Reviewing) {
            return Err(Error::WrongState);
        }
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Missing)?;
        if slot.judgment.is_some() { return Err(Error::Duplicate); }
        if judgment.action() != &slot.action { return Err(Error::Binding); }
        slot.judgment = Some(judgment);
        Ok(())
    }

    /// Replace only the observation, using an exact predecessor. None explicitly
    /// marks all inputs unavailable; it does not remove a retained requirement.
    /// Snapshot/cut/semantic high-water marks survive None and actor resets.
    /// Source-bound slots allow only withdrawal here; positive inputs must come
    /// through record_captured_publication_inputs after that withdrawal.
    pub fn record_publication_inputs(
        &mut self, attempt: u64, expected_revision: u64, inputs: Option<PublicationInputs>,
    ) -> Result<u64, Error> {
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Missing)?;
        slot.record_inputs(expected_revision, inputs)
    }

    pub fn publication_input_revision(&self, attempt: u64) -> Result<u64, Error> {
        let gate = self.publication.as_ref().ok_or(Error::Incomplete)?;
        Ok(gate.slots.get(&attempt).ok_or(Error::Missing)?.revision)
    }

    /// Historical diagnostics only, including work before exhaustion/refusal.
    /// No public authority API accepts this report as a permission token.
    pub fn publication_validation(&self, attempt: u64) -> Result<Option<PublicationReport>, Error> {
        let gate = self.publication.as_ref().ok_or(Error::Incomplete)?;
        Ok(gate.slots.get(&attempt).ok_or(Error::Missing)?.last)
    }

    pub(super) fn prepare_publication_proposal(&self, attempt: u64) -> Result<(), Error> {
        if let Some(gate) = &self.publication {
            if gate.slots.contains_key(&attempt) { return Err(Error::Duplicate); }
            if gate.slots.len() >= gate.limits.bindings { return Err(Error::Limit); }
        }
        Ok(())
    }

    pub(super) fn record_publication_proposal(&mut self, proposal: &Proposal) {
        if let Some(gate) = &mut self.publication {
            gate.slots.insert(proposal.attempt, Slot {
                action: proposal.action.clone(), judgment: None, revision: 0,
                current: None, floor: None, last: None, source: None,
            });
        }
    }

    pub(in crate::action::consequence) fn check_publication(&mut self, attempt: u64, action: Option<&FrozenAction>) -> Result<(), Error> {
        let Some(gate) = &mut self.publication else { return Ok(()); };
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Incomplete)?;
        // A source-bound capture cannot bridge authorization, dispatch and first
        // publication. Each boundary requires its own actual acquisition cycle.
        let fresh = slot.consume_capture();
        let report = match (&slot.judgment, fresh) {
            (Some(judgment), true) => judgment.validate(
                action.unwrap_or(&slot.action),
                slot.current.as_ref().map_or_else(PublicationBasis::default, PublicationInputs::basis),
                gate.limits.validation,
            ),
            _ => PublicationReport {
                outcome: PublicationOutcome::Refused(Error::Incomplete),
                spent: RefinementBudget::default(),
            },
        };
        slot.last = Some(report);
        report.require_valid()
    }
}
