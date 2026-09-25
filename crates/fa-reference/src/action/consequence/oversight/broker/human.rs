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

/// Trusted lifetime budget for review work, separate from effect reservations.
/// Rejecting or revoking work never refunds presentation capacity. All units
/// are counted once PER ATTEMPT, even when identical evidence is shown once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanWorkBudget {
    pub max_work_items: usize,
    pub max_members_per_work: usize,
    pub max_units_per_work: u64,
    pub max_total_units: u64,
}

impl HumanWorkBudget {
    pub fn validate(self) -> Result<(), Error> {
        if self.max_work_items == 0 || self.max_members_per_work == 0
            || self.max_units_per_work == 0 || self.max_total_units == 0
        {
            return Err(Error::InvalidInput);
        }
        if self.max_work_items > MAX_HUMAN_REQUESTS || self.max_members_per_work > MAX_HUMAN_REQUESTS {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanWorkUsage {
    pub work_items: usize,
    pub requests: usize,
    pub total_units: u64,
}

#[derive(Debug)]
struct WorkBinding {
    issuer: Rc<()>,
    id: u64,
    requests: Vec<HumanRequest>,
    total_units: u64,
    presented_at: ElapsedTick,
}

/// A sealed presentation, not a capability to approve or dispatch anything.
/// Every listed member is a separate proposed effect, NOT a deduplicated effect.
/// Membership never expands after this value has been returned to the reviewer.
#[derive(Clone, Debug)]
pub struct HumanReviewWork {
    binding: Rc<WorkBinding>,
}

impl HumanReviewWork {
    pub fn id(&self) -> u64 { self.binding.id }
    pub fn requests(&self) -> &[HumanRequest] { &self.binding.requests }
    pub fn total_units(&self) -> u64 { self.binding.total_units }
    pub fn presented_at(&self) -> ElapsedTick { self.binding.presented_at }
}

/// Reviewer-owned, bounded aggregation of exactly equivalent pending requests.
/// This consumes the reviewer role; requests and work snapshots cannot recreate
/// it. Budgets and terminal work are retained for this queue's entire lifetime.
/// No serialization, durable reviewer identity or automatic approval is implied.
#[derive(Debug)]
pub struct HumanReviewWorkQueue {
    reviewer: HumanReviewer,
    issuer: Rc<()>,
    budget: HumanWorkBudget,
    works: BTreeMap<u64, Rc<WorkBinding>>,
    enrolled: BTreeSet<u64>,
    total_units: u64,
}

impl HumanReviewer {
    /// Validate the budget before moving the separately provisioned role here.
    /// There is deliberately no conversion back to an unbudgeted reviewer.
    pub fn into_work_queue(self, budget: HumanWorkBudget) -> Result<HumanReviewWorkQueue, Error> {
        budget.validate()?;
        Ok(HumanReviewWorkQueue {
            reviewer: self, issuer: Rc::new(()), budget,
            works: BTreeMap::new(), enrolled: BTreeSet::new(), total_units: 0,
        })
    }
}

impl HumanReviewWorkQueue {
    pub fn usage(&self) -> HumanWorkUsage {
        HumanWorkUsage { work_items: self.works.len(), requests: self.enrolled.len(), total_units: self.total_units }
    }

    /// Present the oldest unqueued request (request-id order) together with as
    /// many equivalent pending requests as fit. Equivalence includes the FULL
    /// frozen action, private witnesses, helper inputs, policy/control/input
    /// generations and expiry. Only attempt/request identity may differ.
    /// Oversized groups are split, never silently broadened or undercharged.
    /// None means no eligible pending work; Limit means explicit backpressure.
    pub fn next_work(&mut self, now: ElapsedTick) -> Result<Option<HumanReviewWork>, Error> {
        let mut state = self.reviewer.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observe(now)?;
        let eligible = |entry: &&Entry| {
            entry.status.disposition == HumanDisposition::Pending
                && !self.enrolled.contains(&entry.binding.request)
                && now >= entry.binding.created_at && now < entry.binding.expires_at
        };
        let Some(first) = state.entries.values().find(eligible) else { return Ok(None); };
        if self.works.len() >= self.budget.max_work_items { return Err(Error::Limit); }
        let remaining = self.budget.max_total_units.checked_sub(self.total_units).ok_or(Error::Limit)?;
        let ceiling = remaining.min(self.budget.max_units_per_work);
        let mut requests = Vec::new();
        let mut total_units = 0_u64;
        for entry in state.entries.values().filter(eligible) {
            let left = &first.binding;
            let right = &entry.binding;
            if left.reviewer != right.reviewer || left.sequence != right.sequence
                || left.epoch != right.epoch || left.policy_generation != right.policy_generation
                || left.input_revision != right.input_revision || left.expires_at != right.expires_at
                || left.inputs.as_ref() != right.inputs.as_ref()
            {
                continue;
            }
            if requests.len() >= self.budget.max_members_per_work { break; }
            let units = entry.binding.inputs.action().spec().units;
            // Subtract before adding: even u64::MAX-sized declarations cannot
            // wrap either the per-work or lifetime review exposure counter.
            if units > ceiling - total_units { break; }
            total_units += units;
            requests.push(HumanRequest { binding: Rc::clone(&entry.binding) });
        }
        if requests.is_empty() { return Err(Error::Limit); }
        let id = u64::try_from(self.works.len()).map_err(|_| Error::Overflow)?
            .checked_add(1).ok_or(Error::Overflow)?;
        let binding = Rc::new(WorkBinding {
            issuer: Rc::clone(&self.issuer), id, requests, total_units, presented_at: now,
        });
        drop(state);
        for request in &binding.requests { self.enrolled.insert(request.id()); }
        self.total_units += total_units;
        self.works.insert(id, Rc::clone(&binding));
        Ok(Some(HumanReviewWork { binding }))
    }

    fn matches_work(&self, work: &HumanReviewWork) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &work.binding.issuer) { return Err(Error::Binding); }
        let retained = self.works.get(&work.id()).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(retained, &work.binding) { return Err(Error::Binding); }
        Ok(())
    }

    /// Atomic all-member approval, returning one ordinary one-use human key
    /// per REQUEST ID. Each still requires its OWN automatic permit and the
    /// original dispatch-time evidence, policy, epoch, fence and expiry checks.
    /// A stale/terminal member prevents issuing ANY keys from this operation.
    pub fn approve(&mut self, work: &HumanReviewWork, now: ElapsedTick) -> Result<BTreeMap<u64, HumanPermit>, Error> {
        self.matches_work(work)?;
        let mut state = self.reviewer.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observe(now)?;
        for request in work.requests() {
            state.matches(&request.binding)?;
            if now < request.created_at() || now >= request.expires_at() { return Err(Error::Stale); }
            if state.entries[&request.id()].status.disposition != HumanDisposition::Pending {
                return Err(Error::WrongState);
            }
        }
        let keys = work.requests().iter().map(|request| {
            (request.id(), HumanPermit { binding: Rc::clone(&request.binding) })
        }).collect();
        for request in work.requests() {
            let status = &mut state.entries.get_mut(&request.id()).expect("validated work member").status;
            status.disposition = HumanDisposition::Approved;
            status.issued_at = Some(now);
        }
        Ok(keys)
    }

    /// Reject the exact displayed member set atomically. Rejected work retains
    /// its budget charge and cannot be presented again to fish for approval.
    pub fn reject(&mut self, work: &HumanReviewWork, now: ElapsedTick) -> Result<(), Error> {
        self.matches_work(work)?;
        let mut state = self.reviewer.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observe(now)?;
        for request in work.requests() {
            state.matches(&request.binding)?;
            if state.entries[&request.id()].status.disposition != HumanDisposition::Pending {
                return Err(Error::WrongState);
            }
        }
        for request in work.requests() {
            let status = &mut state.entries.get_mut(&request.id()).expect("validated work member").status;
            status.disposition = HumanDisposition::Rejected;
            status.finished_at = Some(now);
        }
        Ok(())
    }

    /// Withdraw outstanding members even after another member was dispatched.
    /// Consumed/rejected/revoked entries stay terminal. This does not undo any
    /// effect, release reservations, or manufacture a nonexecution receipt.
    pub fn revoke(&mut self, work: &HumanReviewWork, now: ElapsedTick) -> Result<HumanRevocation, Error> {
        self.matches_work(work)?;
        let mut state = self.reviewer.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observe(now)?;
        for request in work.requests() { state.matches(&request.binding)?; }
        let requests: Vec<_> = work.requests().iter().filter_map(|request| {
            matches!(state.entries[&request.id()].status.disposition,
                HumanDisposition::Pending | HumanDisposition::Approved).then_some(request.id())
        }).collect();
        for id in &requests {
            let status = &mut state.entries.get_mut(id).expect("validated work member").status;
            status.disposition = HumanDisposition::Revoked;
            status.finished_at = Some(now);
        }
        Ok(HumanRevocation { at: now, requests })
    }

    /// Emergency withdrawal includes requests not yet admitted to review work.
    /// It remains available after all presentation budgets have been exhausted.
    pub fn revoke_all(&mut self, now: ElapsedTick) -> Result<HumanRevocation, Error> {
        self.reviewer.revoke_all(now)
    }

    pub fn statuses(&self, work: &HumanReviewWork) -> Result<Vec<HumanStatus>, Error> {
        self.matches_work(work)?;
        let state = self.reviewer.state.try_borrow().map_err(|_| Error::WrongState)?;
        work.requests().iter().map(|request| {
            state.matches(&request.binding)?;
            Ok(state.entries[&request.id()].status)
        }).collect()
    }
}

#[cfg(test)]
mod aggregation_tests {
    use super::*;
    use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
    use crate::action::consequence::oversight::{CommitteeContract, HelperContract, action_frame};
    use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
    use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};

    fn inputs(units: u64, payload: &[u8]) -> Rc<CommitteeInput> {
        let action = FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
            payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(100), units,
        }).unwrap();
        let helper = HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"framed-action-v1".to_vec(),
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0,
        }, 7, b"approve?".to_vec()).unwrap();
        let mut bytes = action_frame(&action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        let contract = CommitteeContract::new(BTreeMap::from([("alice".to_owned(), helper)])).unwrap();
        Rc::new(CommitteeInput::capture(&action, &contract, BTreeMap::from([("alice".to_owned(), view)])).unwrap())
    }

    fn queue(budget: HumanWorkBudget) -> HumanReviewWorkQueue {
        HumanReviewer { state: Rc::new(RefCell::new(HumanState {
            issuer: Rc::new(()), policy: HumanReviewPolicy {
                reviewer_id: 9, max_validity_ticks: 100, max_requests: MAX_HUMAN_REQUESTS,
            }, elapsed: None, entries: BTreeMap::new(), contexts: BTreeSet::new(), retained_bytes: 0,
        })) }.into_work_queue(budget).unwrap()
    }

    fn budget() -> HumanWorkBudget {
        HumanWorkBudget { max_work_items: 4, max_members_per_work: 4, max_units_per_work: 100, max_total_units: 200 }
    }

    fn add(queue: &HumanReviewWorkQueue, id: u64, inputs: Rc<CommitteeInput>, revision: u64) {
        let mut state = queue.reviewer.state.borrow_mut();
        let binding = Rc::new(Binding {
            issuer: Rc::clone(&state.issuer), request: id, reviewer: 9, attempt: id,
            sequence: 0, epoch: 0, policy_generation: 0, input_revision: revision,
            inputs, created_at: ElapsedTick(1), expires_at: ElapsedTick(90),
        });
        let status = HumanStatus {
            request: id, attempt: id, reviewer: 9, disposition: HumanDisposition::Pending,
            created_at: ElapsedTick(1), expires_at: ElapsedTick(90), issued_at: None, finished_at: None,
        };
        assert!(state.entries.insert(id, Entry { binding, status }).is_none());
    }

    #[test]
    fn equivalent_work_has_sealed_membership_and_separate_single_use_keys() {
        let mut queue = queue(budget());
        let evidence = inputs(10, b"publish");
        add(&queue, 1, Rc::clone(&evidence), 1);
        add(&queue, 2, Rc::clone(&evidence), 1);
        let work = queue.next_work(ElapsedTick(2)).unwrap().unwrap();
        assert_eq!(work.requests().len(), 2);
        assert_eq!(work.total_units(), 20);
        add(&queue, 3, evidence, 1);
        let keys = queue.approve(&work.clone(), ElapsedTick(3)).unwrap();
        assert_eq!(keys.keys().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert!(!Rc::ptr_eq(&keys[&1].binding, &keys[&2].binding));
        assert!(matches!(queue.approve(&work, ElapsedTick(3)), Err(Error::WrongState)));
        let late = queue.next_work(ElapsedTick(3)).unwrap().unwrap();
        assert_eq!(late.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![3]);
        assert_eq!(queue.statuses(&late).unwrap()[0].disposition, HumanDisposition::Pending);
    }

    #[test]
    fn scope_evidence_revision_and_unit_budgets_are_not_collapsed() {
        let mut queue = queue(HumanWorkBudget { max_units_per_work: 20, max_total_units: 30, ..budget() });
        for id in 1..=3 { add(&queue, id, inputs(10, b"publish"), 1); }
        add(&queue, 4, inputs(10, b"different-effect"), 1);
        add(&queue, 5, inputs(10, b"publish"), 2);
        let first = queue.next_work(ElapsedTick(2)).unwrap().unwrap();
        assert_eq!(first.requests().len(), 2);
        queue.reject(&first, ElapsedTick(3)).unwrap();
        let second = queue.next_work(ElapsedTick(3)).unwrap().unwrap();
        assert_eq!(second.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![3]);
        assert_eq!(queue.usage(), HumanWorkUsage { work_items: 2, requests: 3, total_units: 30 });
        assert!(matches!(queue.next_work(ElapsedTick(3)), Err(Error::Limit)));
        assert_eq!(queue.revoke_all(ElapsedTick(4)).unwrap().requests, vec![3, 4, 5]);
        assert_eq!(queue.usage().total_units, 30);
    }

    #[test]
    fn mixed_terminal_work_cannot_partially_approve_and_revocation_keeps_consumed() {
        let mut queue = queue(budget());
        add(&queue, 1, inputs(10, b"publish"), 1);
        add(&queue, 2, inputs(10, b"publish"), 1);
        let work = queue.next_work(ElapsedTick(2)).unwrap().unwrap();
        queue.reviewer.state.borrow_mut().entries.get_mut(&2).unwrap().status.disposition = HumanDisposition::Revoked;
        assert!(matches!(queue.approve(&work, ElapsedTick(3)), Err(Error::WrongState)));
        assert_eq!(queue.statuses(&work).unwrap()[0].disposition, HumanDisposition::Pending);
        queue.reviewer.state.borrow_mut().entries.get_mut(&2).unwrap().status.disposition = HumanDisposition::Consumed;
        assert_eq!(queue.revoke(&work, ElapsedTick(4)).unwrap().requests, vec![1]);
        assert_eq!(queue.statuses(&work).unwrap()[1].disposition, HumanDisposition::Consumed);
        assert!(queue.revoke(&work, ElapsedTick(4)).unwrap().requests.is_empty());
    }
}
