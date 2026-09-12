//! Bounded actor intake into the existing oversight broker (plan 17.1/9.3; FA-107).
//!
//! This is in-process role separation, not OS isolation or authentication. Only
//! the supervisor owns the broker. Shared state contains actor-supplied proposals
//! and their redacted projection, never helper inputs, votes, policy or permits.

mod stopping;

use super::{CommitteeContract, OversightBroker};
use crate::action::consequence::gate::containment::session::policy::controller::{
    ControllerConfig, Proposal,
};
use crate::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, MAX_PAYLOAD_BYTES,
    ResolvedTarget, Scope, VERSION,
};
use crate::{Error, Snapshot};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::rc::Rc;

pub const MAX_ACTOR_REQUESTS: usize = 128;
pub const MAX_ACTOR_BYTES: usize = 2 * 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntakeLimits {
    pub requests: usize,
    pub payload_bytes: usize,
}

impl Default for IntakeLimits {
    fn default() -> Self {
        Self { requests: MAX_ACTOR_REQUESTS, payload_bytes: MAX_ACTOR_BYTES }
    }
}

/// Execution fields only. Scope and witnesses are supplied by the controller,
/// not the actor. An expected epoch is never silently upgraded on intake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorProposal {
    pub target: ResolvedTarget,
    pub payload: Vec<u8>,
    pub units: u64,
    pub deadline: ElapsedTick,
    pub expected_policy_epoch: u64,
}

impl ActorProposal {
    fn validate(&self) -> Result<(), ActorError> {
        if self.units == 0 || self.deadline.0 == 0 || [
            self.target.adapter, self.target.object, self.target.contract_version,
            self.target.expected_version, self.target.generation,
        ].contains(&0) {
            return Err(ActorError::MalformedProposal);
        }
        if self.payload.len() > MAX_PAYLOAD_BYTES
            || u64::try_from(self.payload.len()).map_err(|_| ActorError::Capacity)? > self.units
        {
            return Err(ActorError::Capacity);
        }
        Ok(())
    }

    fn action(&self, scope: Scope) -> ActionSpec {
        ActionSpec {
            version: VERSION, scope, target: Some(self.target), payload: self.payload.clone(),
            required_witnesses: Vec::new(), policy_epoch: self.expected_policy_epoch,
            deadline: self.deadline, units: self.units,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorError {
    MalformedProposal,
    IdempotencyConflict,
    Capacity,
    Unavailable,
    Withheld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorOutcome {
    NotAdmitted,
    Denied,
    CancelledBeforeDispatch,
    Executed,
    ConfirmedNotExecuted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason { ControllerUnavailable, OutcomeUnknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasisSource { Intake, ControlLedger }

/// Reference-only provenance. It makes no signature, durable-replay or host
/// completeness claim. Request-local generations disclose no congress sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorBasis {
    pub request: u64,
    pub generation: u64,
    pub source: BasisSource,
}

/// Epistemic projection at this reference boundary, not the production wire
/// format. Known is emitted only for terminal outcomes; Approved is not a value.
/// The unused Stale/Absent forms remain distinct from unknown or withheld data.
/// None of these caller-copyable observations is accepted as an authority input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Knowledge<T> {
    Known { value: T, basis: ActorBasis },
    Pending { request: u64 },
    Unknown { reason: UnknownReason },
    Withheld { authority_required: &'static str },
    Stale { generation: u64, current: u64 },
    Absent { closed_domain: u64, frontier: u64 },
}

/// A request-local observation/cancellation handle, never an effect permit.
#[derive(Clone)]
pub struct ActorTicket { issuer: Rc<()>, request: u64 }

impl ActorTicket {
    pub fn request(&self) -> u64 { self.request }
}

impl fmt::Debug for ActorTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorTicket").field("request", &self.request).finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Projection {
    Pending,
    Unknown,
    Terminal(ActorOutcome, BasisSource),
}

struct Entry {
    proposal: ActorProposal,
    projection: Projection,
    generation: u64,
    cancel_requested: bool,
}

impl Entry {
    fn project(&mut self, next: Projection) {
        if self.projection != next {
            // Only Pending -> Unknown/Terminal and Unknown -> Terminal occur.
            self.generation += 1;
            self.projection = next;
        }
    }
}

struct Mailbox {
    issuer: Rc<()>,
    limits: IntakeLimits,
    live: bool,
    accepting: bool,
    payload_bytes: usize,
    entries: BTreeMap<u64, Entry>,
    queued: VecDeque<u64>,
}

impl Mailbox {
    /// Queued requests have never acquired ledger attempts or effect permits.
    /// Preserve their keys and observations while closing only new admission.
    fn close_intake(&mut self) {
        self.accepting = false;
        while let Some(id) = self.queued.pop_front() {
            self.entries.get_mut(&id).expect("queued actor request retained").project(
                Projection::Terminal(ActorOutcome::CancelledBeforeDispatch, BasisSource::Intake),
            );
        }
    }
}

/// Give only this handle to a cooperative actor. Clones share one bounded queue
/// and idempotency domain. No broker, reviewer, endpoint or permit is reachable.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::{OversightBroker, actor::ActorPort};
/// fn escape(port: ActorPort) -> OversightBroker { port }
/// ```
#[derive(Clone)]
pub struct ActorPort { mailbox: Rc<RefCell<Mailbox>> }

impl fmt::Debug for ActorPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActorPort { .. }")
    }
}

impl ActorPort {
    /// Exact duplicates return the original handle without queueing more work.
    /// Tombstones and original proposals remain until this whole port is dropped;
    /// capacity exhaustion cannot evict a key and make an old request fresh.
    pub fn submit(&self, key: u64, proposal: &ActorProposal) -> Result<ActorTicket, ActorError> {
        if key == 0 { return Err(ActorError::MalformedProposal); }
        proposal.validate()?;
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
        if !state.live { return Err(ActorError::Unavailable); }
        if let Some(previous) = state.entries.get(&key) {
            if previous.proposal != *proposal { return Err(ActorError::IdempotencyConflict); }
            return Ok(ActorTicket { issuer: Rc::clone(&state.issuer), request: key });
        }
        if !state.accepting { return Err(ActorError::Unavailable); }
        let bytes = state.payload_bytes.checked_add(proposal.payload.len()).ok_or(ActorError::Capacity)?;
        if state.entries.len() >= state.limits.requests || bytes > state.limits.payload_bytes {
            return Err(ActorError::Capacity);
        }
        state.entries.insert(key, Entry {
            proposal: proposal.clone(), projection: Projection::Pending,
            generation: 0, cancel_requested: false,
        });
        state.queued.push_back(key);
        state.payload_bytes = bytes;
        Ok(ActorTicket { issuer: Rc::clone(&state.issuer), request: key })
    }

    /// Reads only the last published projection. Pending is not permission and
    /// does not assert that execution has not started since the last publication.
    pub fn poll(&self, ticket: &ActorTicket) -> Knowledge<ActorOutcome> {
        let Ok(state) = self.mailbox.try_borrow() else {
            return Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
        };
        if !Rc::ptr_eq(&state.issuer, &ticket.issuer) {
            return Knowledge::Withheld { authority_required: "own_request" };
        }
        let Some(entry) = state.entries.get(&ticket.request) else {
            return Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
        };
        if let Projection::Terminal(value, source) = entry.projection {
            return Knowledge::Known { value, basis: ActorBasis {
                request: ticket.request, generation: entry.generation, source,
            } };
        }
        if !state.live {
            return Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
        }
        match entry.projection {
            Projection::Pending => Knowledge::Pending { request: ticket.request },
            Projection::Unknown => Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown },
            Projection::Terminal(_, _) => unreachable!("terminal projection handled above"),
        }
    }

    /// This is a request to stop, not a nonexecution receipt. The supervisor
    /// applies it through the original ledger; in-flight effects remain charged.
    pub fn cancel(&self, ticket: &ActorTicket) -> Result<(), ActorError> {
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| ActorError::Unavailable)?;
        if !Rc::ptr_eq(&state.issuer, &ticket.issuer) { return Err(ActorError::Withheld); }
        if !state.live { return Err(ActorError::Unavailable); }
        let entry = state.entries.get_mut(&ticket.request).ok_or(ActorError::Withheld)?;
        entry.cancel_requested = true;
        Ok(())
    }
}

struct Accepted {
    attempt: u64,
    action: FrozenAction,
}

/// Privileged processing result. This never enters the actor's mailbox.
#[derive(Debug)]
pub struct IntakeResult {
    pub request: u64,
    pub attempt: Option<u64>,
    pub result: Result<Option<Proposal>, Error>,
}

/// The trusted owner drives review and dispatch through its existing broker.
/// This is synchronous handoff, not a replacement task runtime or wire server.
pub struct ActorSupervisor {
    broker: OversightBroker,
    scope: Scope,
    mailbox: Rc<RefCell<Mailbox>>,
    accepted: BTreeMap<u64, Accepted>,
    next_attempt: u64,
}

impl ActorSupervisor {
    pub fn new(
        config: ControllerConfig, endpoint: &mut crate::action::consequence::delivery::PublicationEndpoint,
        contracts: CommitteeContract, limits: IntakeLimits,
    ) -> Result<(ActorPort, Self), Error> {
        if limits.requests == 0 || limits.payload_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.requests > MAX_ACTOR_REQUESTS || limits.payload_bytes > MAX_ACTOR_BYTES { return Err(Error::Limit); }
        let scope = config.scope;
        let broker = OversightBroker::new(config, endpoint, contracts)?;
        let mailbox = Rc::new(RefCell::new(Mailbox {
            issuer: Rc::new(()), limits, live: true, accepting: true, payload_bytes: 0,
            entries: BTreeMap::new(), queued: VecDeque::new(),
        }));
        Ok((ActorPort { mailbox: Rc::clone(&mailbox) }, Self {
            broker, scope, mailbox, accepted: BTreeMap::new(), next_attempt: 1,
        }))
    }

    /// Trusted integration only. Never pass the supervisor to the actor.
    pub fn broker(&self) -> &OversightBroker { &self.broker }
    pub fn broker_mut(&mut self) -> &mut OversightBroker { &mut self.broker }

    /// Only the supervisor can translate a request key to a control-ledger ID.
    pub fn attempt(&self, request: u64) -> Result<u64, Error> {
        Ok(self.accepted.get(&request).ok_or(Error::Missing)?.attempt)
    }
    pub fn action(&self, request: u64) -> Result<&FrozenAction, Error> {
        Ok(&self.accepted.get(&request).ok_or(Error::Missing)?.action)
    }

    /// Process FIFO arrival order, not actor-chosen key order. A refused intake
    /// is terminal for that key. New evidence requires an explicitly new request.
    pub fn accept_next(&mut self, snapshot: &Snapshot) -> Result<Option<IntakeResult>, Error> {
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if self.broker.stop_receipt().is_some() {
            state.close_intake();
            return Ok(None);
        }
        let Some(request) = state.queued.front().copied() else { return Ok(None); };
        let entry = state.entries.get_mut(&request).expect("queued request retained");
        if entry.cancel_requested {
            entry.project(Projection::Terminal(ActorOutcome::CancelledBeforeDispatch, BasisSource::Intake));
            state.queued.pop_front();
            return Ok(Some(IntakeResult { request, attempt: None, result: Ok(None) }));
        }
        let occupied = self.broker.inspect().ledger.stages;
        let mut attempt = self.next_attempt;
        while occupied.contains_key(&attempt) { attempt = attempt.checked_add(1).ok_or(Error::Overflow)?; }
        let next_attempt = attempt.checked_add(1).ok_or(Error::Overflow)?;
        let result = self.broker.propose(attempt, entry.proposal.action(self.scope), snapshot);
        self.next_attempt = next_attempt;
        match &result {
            Ok(proposal) => {
                self.accepted.insert(request, Accepted { attempt, action: proposal.action.clone() });
                entry.project(project(proposal.state));
            }
            Err(_) => entry.project(Projection::Terminal(ActorOutcome::NotAdmitted, BasisSource::Intake)),
        }
        state.queued.pop_front();
        Ok(Some(IntakeResult { request, attempt: result.as_ref().ok().map(|_| attempt), result: result.map(Some) }))
    }

    /// Publish only terminal outcomes or explicit uncertainty. Review approval,
    /// exact disqualifiers, votes, budgets and controller sequences stay private.
    /// Cancellation after dispatch never becomes cancellation-before-dispatch.
    pub fn synchronize(&mut self) -> Result<(), Error> {
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if self.broker.stop_receipt().is_some() { state.close_intake(); }
        let before = self.broker.inspect();
        for (request, accepted) in &self.accepted {
            let stage = *before.ledger.stages.get(&accepted.attempt).ok_or(Error::Missing)?;
            let entry = state.entries.get(request).expect("accepted request retained");
            if entry.cancel_requested && matches!(stage,
                ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized)
            {
                self.broker.cancel(accepted.attempt)?;
            }
        }
        let after = self.broker.inspect();
        for (request, accepted) in &self.accepted {
            let stage = *after.ledger.stages.get(&accepted.attempt).ok_or(Error::Missing)?;
            state.entries.get_mut(request).expect("accepted request retained").project(project(stage));
        }
        Ok(())
    }
}

impl Drop for ActorSupervisor {
    fn drop(&mut self) {
        // No mailbox borrow or guard is ever exposed by either public role.
        if let Ok(mut state) = self.mailbox.try_borrow_mut() { state.live = false; }
    }
}

fn project(stage: ActionState) -> Projection {
    let outcome = match stage {
        ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized => return Projection::Pending,
        ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => return Projection::Unknown,
        ActionState::Confirmed => ActorOutcome::Executed,
        ActionState::Denied => ActorOutcome::Denied,
        ActionState::Cancelled => ActorOutcome::CancelledBeforeDispatch,
        ActionState::ConfirmedNotExecuted => ActorOutcome::ConfirmedNotExecuted,
    };
    Projection::Terminal(outcome, BasisSource::ControlLedger)
}
