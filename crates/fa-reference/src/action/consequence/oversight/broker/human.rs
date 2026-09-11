//! Two-key dispatch (plan 9.11) over the existing policy/congress/effect ledger.
//!
//! The separately held reviewer role issues only a second, revocable key. It
//! cannot mint the effect permit, change policy, or refund resources. This is
//! process-local reference role separation, not human authentication or durable
//! co-signing. Every operation is an atomic model step; no crash claim follows.

use super::{CommitteeInput, OversightBroker};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::{DispatchApproval, DispatchEnvelope};
use crate::action::{ActionState, ElapsedTick, FrozenAction, Permit};
use crate::{Error, Snapshot};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_HUMAN_REQUESTS: usize = 256;
pub const MAX_HUMAN_INPUT_BYTES: usize = 8 * 1_048_576;

/// Trusted bootstrap for ALL effects of this broker's one-resource profile.
/// There is no actor-selectable threshold, reviewer replacement or disable API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanReviewPolicy {
    pub reviewer_id: u64,
    pub max_validity_ticks: u64,
    pub max_requests: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanDisposition {
    Pending,
    Approved,
    Rejected,
    Revoked,
    Consumed,
}

#[derive(Debug)]
struct Binding {
    issuer: Rc<()>,
    request: u64,
    reviewer: u64,
    attempt: u64,
    sequence: u64,
    epoch: u64,
    policy_generation: u64,
    input_revision: u64,
    inputs: Rc<CommitteeInput>,
    created_at: ElapsedTick,
    expires_at: ElapsedTick,
}

/// Immutable evidence for the separately provisioned reviewer. Cloning a
/// request copies only evidence references, never approval or a live permit.
#[derive(Clone, Debug)]
pub struct HumanRequest {
    binding: Rc<Binding>,
}

impl HumanRequest {
    pub fn id(&self) -> u64 { self.binding.request }
    pub fn reviewer_id(&self) -> u64 { self.binding.reviewer }
    pub fn attempt(&self) -> u64 { self.binding.attempt }
    pub fn action(&self) -> &FrozenAction { self.binding.inputs.action() }
    pub fn inputs(&self) -> &CommitteeInput { &self.binding.inputs }
    pub fn control_sequence(&self) -> u64 { self.binding.sequence }
    pub fn input_revision(&self) -> u64 { self.binding.input_revision }
    pub fn policy_generation(&self) -> u64 { self.binding.policy_generation }
    pub fn created_at(&self) -> ElapsedTick { self.binding.created_at }
    pub fn expires_at(&self) -> ElapsedTick { self.binding.expires_at }
}

/// A one-use second key, not an action Permit. Ledger state also prevents reuse
/// when the same Rust value is borrowed twice. A failed dispatch does not spend it.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::human::HumanPermit;
/// fn duplicate(key: HumanPermit) { let _copy = key.clone(); }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::oversight::human::HumanPermit;
/// fn elevate(key: HumanPermit) -> Permit { key }
/// ```
#[derive(Debug)]
pub struct HumanPermit {
    binding: Rc<Binding>,
}

/// Recorded state, not a claim that Approved is still current. Expiry, input,
/// policy, control sequence and revocation are checked again at dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanStatus {
    pub request: u64,
    pub attempt: u64,
    pub reviewer: u64,
    pub disposition: HumanDisposition,
    pub created_at: ElapsedTick,
    pub expires_at: ElapsedTick,
    pub issued_at: Option<ElapsedTick>,
    pub finished_at: Option<ElapsedTick>,
}

/// Exactly the outstanding keys withdrawn by one reviewer operation. It is
/// evidence of local key withdrawal, never a receipt for external nonexecution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HumanRevocation {
    pub at: ElapsedTick,
    pub requests: Vec<u64>,
}

#[derive(Debug)]
struct Entry {
    binding: Rc<Binding>,
    status: HumanStatus,
}

#[derive(Debug)]
struct HumanState {
    issuer: Rc<()>,
    policy: HumanReviewPolicy,
    elapsed: Option<ElapsedTick>,
    entries: BTreeMap<u64, Entry>,
    contexts: BTreeSet<(u64, u64, u64, u64)>,
    retained_bytes: usize,
}

impl HumanState {
    fn observe(&mut self, now: ElapsedTick) -> Result<(), Error> {
        if self.elapsed.is_some_and(|previous| now < previous) { return Err(Error::Stale); }
        self.elapsed = Some(now);
        Ok(())
    }

    fn matches(&self, binding: &Rc<Binding>) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &binding.issuer) { return Err(Error::Binding); }
        let entry = self.entries.get(&binding.request).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(&entry.binding, binding) { return Err(Error::Binding); }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct HumanGate {
    state: Rc<RefCell<HumanState>>,
}

/// Provision this once to a separate reviewer. The broker does not expose a
/// getter or cloning/conversion path to recover this role from a request/key.
/// The caller is responsible for authenticating and isolating the real human.
#[derive(Debug)]
pub struct HumanReviewer {
    state: Rc<RefCell<HumanState>>,
}

impl HumanReviewer {
    pub fn approve(&self, request: &HumanRequest, now: ElapsedTick) -> Result<HumanPermit, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(&request.binding)?;
        state.observe(now)?;
        if now < request.created_at() || now >= request.expires_at() { return Err(Error::Stale); }
        let entry = state.entries.get_mut(&request.id()).expect("validated request");
        if entry.status.disposition != HumanDisposition::Pending { return Err(Error::WrongState); }
        entry.status.disposition = HumanDisposition::Approved;
        entry.status.issued_at = Some(now);
        Ok(HumanPermit { binding: Rc::clone(&entry.binding) })
    }

    pub fn reject(&self, request: &HumanRequest, now: ElapsedTick) -> Result<(), Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(&request.binding)?;
        state.observe(now)?;
        let entry = state.entries.get_mut(&request.id()).expect("validated request");
        match entry.status.disposition {
            HumanDisposition::Rejected => return Ok(()),
            HumanDisposition::Pending => {}
            _ => return Err(Error::WrongState),
        }
        entry.status.disposition = HumanDisposition::Rejected;
        entry.status.finished_at = Some(now);
        Ok(())
    }

    /// Prevent a pending/approved key from being used. This never cancels an
    /// already dispatched effect or releases the automatic permit's reservation.
    pub fn revoke(&self, request: &HumanRequest, now: ElapsedTick) -> Result<(), Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(&request.binding)?;
        state.observe(now)?;
        let entry = state.entries.get_mut(&request.id()).expect("validated request");
        match entry.status.disposition {
            HumanDisposition::Revoked => return Ok(()),
            HumanDisposition::Pending | HumanDisposition::Approved => {}
            _ => return Err(Error::WrongState),
        }
        entry.status.disposition = HumanDisposition::Revoked;
        entry.status.finished_at = Some(now);
        Ok(())
    }

    /// Withdraw every currently pending or approved key atomically. Terminal
    /// outcomes stay terminal. This does not claim to stop an envelope already
    /// returned by dispatch; seal/reconcile it through the endpoint protocol.
    pub fn revoke_all(&self, now: ElapsedTick) -> Result<HumanRevocation, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observe(now)?;
        let requests: Vec<_> = state.entries.iter().filter_map(|(id, entry)| {
            matches!(entry.status.disposition, HumanDisposition::Pending | HumanDisposition::Approved)
                .then_some(*id)
        }).collect();
        for id in &requests {
            let entry = state.entries.get_mut(id).expect("retained outstanding key");
            entry.status.disposition = HumanDisposition::Revoked;
            entry.status.finished_at = Some(now);
        }
        Ok(HumanRevocation { at: now, requests })
    }
}

impl OversightBroker {
    /// Must be configured before any proposal or control transition. The mode
    /// cannot subsequently be disabled, including after reset or reviewer loss.
    pub fn enable_human_review(&mut self, policy: HumanReviewPolicy) -> Result<HumanReviewer, Error> {
        if self.human.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if policy.reviewer_id == 0 || policy.max_validity_ticks == 0 || policy.max_requests == 0 {
            return Err(Error::InvalidInput);
        }
        if policy.max_requests > MAX_HUMAN_REQUESTS { return Err(Error::Limit); }
        let state = Rc::new(RefCell::new(HumanState {
            issuer: Rc::clone(&self.issuer), policy, elapsed: None,
            entries: BTreeMap::new(), contexts: BTreeSet::new(), retained_bytes: 0,
        }));
        self.human = Some(HumanGate { state: Rc::clone(&state) });
        Ok(HumanReviewer { state })
    }

    pub fn human_review_required(&self) -> bool { self.human.is_some() }

    /// Freeze the actual congress-approved action and complete input basis.
    /// Requests do not reserve or authorize effects; dispatch still reevaluates
    /// the exact policy against fresh supplied state and checks the original key.
    pub fn request_human_approval(
        &mut self, request: u64, attempt: u64,
        current: Option<&CommitteeInput>, expires_at: ElapsedTick,
    ) -> Result<HumanRequest, Error> {
        if request == 0 { return Err(Error::InvalidInput); }
        self.check_approval(attempt, current)?;
        let inspection = self.inspect();
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        if inspection.suspended
            || !matches!(inspection.ledger.stages.get(&attempt), Some(ActionState::Reviewing | ActionState::Authorized))
            || inspection.decisions.get(&attempt) != Some(&Consequence::Continue)
        {
            return Err(Error::WrongState);
        }
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        if slot.action.spec().policy_epoch != inspection.ledger.epoch
            || now >= slot.action.spec().deadline || now >= expires_at
        {
            return Err(Error::Stale);
        }
        if expires_at > slot.action.spec().deadline { return Err(Error::InvalidInput); }
        let gate = self.human.as_ref().ok_or(Error::Incomplete)?;
        let mut state = gate.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if expires_at.0 - now.0 > state.policy.max_validity_ticks { return Err(Error::Limit); }
        let context = (attempt, slot.revision, inspection.sequence, inspection.ledger.epoch);
        if state.entries.contains_key(&request) || state.contexts.contains(&context) { return Err(Error::Duplicate); }
        if state.entries.len() >= state.policy.max_requests { return Err(Error::Limit); }
        let inputs = Rc::clone(slot.current.as_ref().ok_or(Error::Incomplete)?);
        let retained = state.retained_bytes.checked_add(inputs.logical_bytes()).ok_or(Error::Limit)?;
        if retained > MAX_HUMAN_INPUT_BYTES { return Err(Error::Limit); }
        state.observe(now)?;
        let binding = Rc::new(Binding {
            issuer: Rc::clone(&self.issuer), request, reviewer: state.policy.reviewer_id,
            attempt, sequence: inspection.sequence, epoch: inspection.ledger.epoch,
            policy_generation: self.delivery.controller().policy().generation(),
            input_revision: slot.revision, inputs, created_at: now, expires_at,
        });
        let status = HumanStatus {
            request, attempt, reviewer: binding.reviewer, disposition: HumanDisposition::Pending,
            created_at: now, expires_at, issued_at: None, finished_at: None,
        };
        state.entries.insert(request, Entry { binding: Rc::clone(&binding), status });
        state.contexts.insert(context);
        state.retained_bytes = retained;
        Ok(HumanRequest { binding })
    }

    /// Recover the immutable review basis, not a lost reviewer role or key.
    pub fn human_request(&self, request: u64) -> Result<HumanRequest, Error> {
        let gate = self.human.as_ref().ok_or(Error::Incomplete)?;
        let state = gate.state.try_borrow().map_err(|_| Error::WrongState)?;
        let entry = state.entries.get(&request).ok_or(Error::Missing)?;
        Ok(HumanRequest { binding: Rc::clone(&entry.binding) })
    }

    pub fn human_status(&self, request: u64) -> Result<HumanStatus, Error> {
        let gate = self.human.as_ref().ok_or(Error::Incomplete)?;
        let state = gate.state.try_borrow().map_err(|_| Error::WrongState)?;
        Ok(state.entries.get(&request).ok_or(Error::Missing)?.status)
    }

    /// Spend the original effect permit AND the separately issued human key in
    /// one model step. Both are borrowed so a rejected dispatch retains usable
    /// keys; successful consumption is irreversible in their respective ledgers.
    /// The endpoint also enforces this key's expiry before first publication.
    pub fn dispatch_with_human(
        &mut self, permit: &Permit, human: &HumanPermit, action: &FrozenAction,
        current: Option<&CommitteeInput>, snapshot: &Snapshot,
    ) -> Result<DispatchEnvelope, Error> {
        self.check_approval(permit.attempt, current)?;
        let inspection = self.inspect();
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        let gate = self.human.as_ref().ok_or(Error::Incomplete)?;
        let mut state = gate.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(&human.binding)?;
        let binding = &human.binding;
        if binding.attempt != permit.attempt || binding.inputs.action() != action { return Err(Error::Binding); }
        let slot = self.inputs.get(&permit.attempt).ok_or(Error::Missing)?;
        if binding.sequence != inspection.sequence || binding.epoch != inspection.ledger.epoch
            || binding.policy_generation != self.delivery.controller().policy().generation()
            || binding.input_revision != slot.revision
            || slot.current.as_deref() != Some(binding.inputs.as_ref())
        {
            return Err(Error::Stale);
        }
        let status = state.entries[&binding.request].status;
        if status.disposition != HumanDisposition::Approved { return Err(Error::WrongState); }
        let issued_at = status.issued_at.ok_or(Error::Incomplete)?;
        if now < issued_at || now >= binding.expires_at { return Err(Error::Stale); }
        let approval = DispatchApproval::new(binding.request, binding.reviewer, issued_at, binding.expires_at)?;
        state.observe(now)?;
        // No caller callback runs here. Keep the exclusive second-key borrow
        // across the existing issuer/evidence/epoch/fence/one-use dispatch check.
        let message = self.delivery.dispatch_with_approval(permit, action, snapshot, approval)?;
        let entry = state.entries.get_mut(&binding.request).expect("validated retained key");
        entry.status.disposition = HumanDisposition::Consumed;
        entry.status.finished_at = Some(now);
        Ok(message)
    }
}
