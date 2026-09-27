//! The ORIGINAL learned generation's checked KV becomes actual helper input.
//! No recapture, refit, second compression, inferred vote or replacement source.
use super::OversightBroker;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvGroup};
use crate::action::consequence::oversight::{CommitteeInput, ObservedSession, ReviewWindow,
    learned_source::LearnedEvidence,
    sidecar::{SidecarCommitteeRound, SidecarCongressBudget, SidecarCongressPlan, SidecarIdentity}};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// Disclosure choices only. The actual codec, original evaluation origin,
/// retained residual inventory and source come from the compulsory learned audit.
#[derive(Clone, Debug)]
pub struct LearnedSidecarRequest {
    pub identity: SidecarIdentity,
    pub priority: Vec<KvGroup>,
    pub budget: SidecarCongressBudget,
}

#[derive(Debug, Default)]
pub(super) struct LearnedSidecarState {
    required: bool,
    // Original monotonically increasing input revisions, not another input or
    // approval ledger. This also prevents reopening a planner to refill budgets.
    registered: BTreeMap<u64, u64>,
}

/// One bounded disclosure sequence from the proposal's exact learned evidence.
/// It retains the latest position's K/V audit, NOT a full-prefix activation trace.
/// Neither the planner nor an executable generation can be extracted or cloned.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::LearnedSidecar;
/// fn reset_budget(sidecar: LearnedSidecar) { sidecar.into_plan(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::oversight::learned_host::sidecar::LearnedSidecar};
/// fn authorize(sidecar: LearnedSidecar) -> Permit { sidecar }
/// ```
pub struct LearnedSidecar {
    issuer: Rc<()>,
    attempt: u64,
    actor_revision: u64,
    input_revision: u64,
    evidence: LearnedEvidence,
    plan: SidecarCongressPlan,
    current: SidecarCommitteeRound,
}
impl fmt::Debug for LearnedSidecar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedSidecar").field("attempt", &self.attempt)
            .field("actor_revision", &self.actor_revision)
            .field("input_revision", &self.input_revision).finish_non_exhaustive()
    }
}
impl LearnedSidecar {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn input_revision(&self) -> u64 { self.input_revision }
    pub fn evidence(&self) -> &LearnedEvidence { &self.evidence }
    pub fn source(&self) -> &CheckedLearnedKv { self.plan.source() }
    /// Historical bytes, not current eligibility. Use current_learned_sidecar
    /// on the original broker before acting on a previously retained handle.
    pub fn round(&self) -> &SidecarCommitteeRound { &self.current }
}

impl OversightBroker {
    /// Select mandatory sidecar review before the owned generator's FIRST token
    /// or any proposal. There is no disable, late promotion or weaker fallback.
    /// Ordinary residual/external-source configurations are not changed.
    pub fn enable_learned_sidecar_requirement(&mut self) -> Result<(), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() || self.inspect().sequence != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        let host = self.learned_host.as_mut().ok_or(Error::Incomplete)?;
        if host.sidecars.required { return Err(Error::Duplicate); }
        if host.fault.is_some() || host.run.position() != 0
            || !host.run.status().is_active() { return Err(Error::WrongState); }
        host.sidecars.required = true;
        Ok(())
    }
    pub fn learned_sidecar_required(&self) -> bool {
        self.learned_host.as_ref().is_some_and(|host| host.sidecars.required)
    }

    /// Create this proposal's FIRST input from the exact checked source already
    /// retained by its decoder gate. Each attempt gets one planner: replacing or
    /// withdrawing its input cannot restart that planner or refund its spending.
    /// No helper executes and no generation/monitoring work is repeated here.
    pub fn begin_learned_sidecar(&mut self, attempt: u64, actor_revision: u64,
        request: LearnedSidecarRequest) -> Result<LearnedSidecar, Error>
    {
        self.check_learned_sidecar_source(attempt, actor_revision)?;
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.sidecars.registered.contains_key(&attempt) { return Err(Error::Duplicate); }
        if self.input_revision(attempt)? != 0 || self.current_inputs(attempt)?.is_some() {
            return Err(Error::WrongState);
        }
        let evidence = self.learned_decoder_evidence(attempt)?.ok_or(Error::Incomplete)?.clone();
        let source = evidence.audit().source().clone();
        let action = self.inputs.get(&attempt).ok_or(Error::Missing)?.action.clone();
        let mut plan = SidecarCongressPlan::new(source, request.identity, request.priority, request.budget)?;
        let current = plan.initial(&action, &self.contracts)?;
        let revision = self.record_learned_sidecar_input(attempt, 0, current.input())?;
        Ok(LearnedSidecar { issuer: Rc::clone(&self.issuer), attempt, actor_revision,
            input_revision: revision, evidence, plan, current })
    }

    pub fn current_learned_sidecar(&self, sidecar: &LearnedSidecar) -> Result<&CommitteeInput, Error> {
        self.check_learned_sidecar(sidecar)?;
        self.current_inputs(sidecar.attempt)?.ok_or(Error::Incomplete)
    }
    pub fn begin_learned_sidecar_review(&mut self, sidecar: &LearnedSidecar, round: u64,
        root: [u8; 32], window: ReviewWindow, snapshot: &Snapshot) -> Result<ObservedSession, Error>
    {
        self.check_learned_sidecar(sidecar)?;
        self.begin_review(sidecar.attempt, round, root, window, snapshot)
    }

    fn check_learned_sidecar_source(&self, attempt: u64, actor_revision: u64) -> Result<(), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if actor_revision != self.actor_revision() { return Err(Error::Stale); }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.fault.is_some() { return Err(Error::Incomplete); }
        // Source validation must precede input construction; it cannot require
        // the not-yet-created sidecar. Permitting hooks use the complete gate.
        self.check_decoder_source(attempt)?;
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        let action = &self.inputs.get(&attempt).ok_or(Error::Missing)?.action;
        if now >= action.spec().deadline { return Err(Error::Stale); }
        Ok(())
    }
    fn check_learned_sidecar(&self, sidecar: &LearnedSidecar) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &sidecar.issuer) { return Err(Error::Binding); }
        self.check_learned_sidecar_source(sidecar.attempt, sidecar.actor_revision)?;
        if self.input_revision(sidecar.attempt)? != sidecar.input_revision
            || self.current_inputs(sidecar.attempt)? != Some(sidecar.current.input()) {
            return Err(Error::Stale);
        }
        Ok(())
    }

    // Called by the SAME gate used at every existing permitting boundary.
    // Equal-looking caller packets cannot register their own provenance marker.
    pub(in super::super) fn check_required_learned_sidecar(&self, attempt: u64) -> Result<(), Error> {
        let Some(host) = &self.learned_host else { return Ok(()); };
        if !host.sidecars.required { return Ok(()); }
        let expected = host.sidecars.registered.get(&attempt).ok_or(Error::Incomplete)?;
        if self.input_revision(attempt)? != *expected { return Err(Error::Stale); }
        self.current_inputs(attempt)?.ok_or(Error::Incomplete)?;
        Ok(())
    }

    fn record_learned_sidecar_input(&mut self, attempt: u64, expected: u64,
        input: &CommitteeInput) -> Result<u64, Error>
    {
        // Allocate the bounded revision-marker map and input copy BEFORE the
        // original input transaction. Only infallible moves follow its success.
        let mut registered = self.learned_host.as_ref().ok_or(Error::Incomplete)?.sidecars.registered.clone();
        let next = if self.current_inputs(attempt)? == Some(input) { expected }
            else { expected.checked_add(1).ok_or(Error::Overflow)? };
        registered.insert(attempt, next);
        let revision = self.record_inputs(attempt, expected, input.clone())?;
        debug_assert_eq!(revision, next, "original input revision rule");
        self.learned_host.as_mut().expect("retained learned host").sidecars.registered = registered;
        Ok(revision)
    }
}
