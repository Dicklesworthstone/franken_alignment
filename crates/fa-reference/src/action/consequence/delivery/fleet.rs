//! FA-120 bounded fleet admission fences over existing independently funded domains.
//!
//! Messages and observations are process-local capabilities, not signatures.
//! A shared reference scheduler supplies event order and a logical lease clock;
//! it does NOT establish a distributed order or clock-synchronization protocol.
//! Issuing a command never installs it at a domain or settles an external effect.

mod domain;

use super::{ActionState, ElapsedTick, EndpointOutcome, Scope};
use crate::Error;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_FLEET_DOMAINS: usize = 64;
pub const MAX_FLEET_FENCES: usize = 128;

#[derive(Debug, Default)]
struct ModelOrder {
    order: Cell<u64>,
    time: Cell<Option<ElapsedTick>>,
}

impl ModelOrder {
    fn next(&self) -> Result<u64, Error> {
        self.order.get().checked_add(1).ok_or(Error::Overflow)
    }
    fn check_time(&self, now: ElapsedTick) -> Result<(), Error> {
        if self.time.get().is_some_and(|previous| now < previous) { return Err(Error::Stale); }
        Ok(())
    }
}

/// Registered selection only. Empty/unknown domain sets never mean All.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FleetScope {
    All,
    Tenant(u64),
    Principal { tenant: u64, principal: u64 },
    Adapter { adapter: u64, contract_version: u64 },
    Domains(BTreeSet<u64>),
}

#[derive(Debug)]
struct Registration {
    domain: u64,
    scope: Scope,
    adapter: u64,
    contract_version: u64,
    lease_until: ElapsedTick,
}

#[derive(Debug)]
struct Command {
    issuer: Rc<()>,
    id: u64,
    floor: u64,
    scope: FleetScope,
    issued_order: u64,
    issued_at: ElapsedTick,
    targets: BTreeMap<u64, Rc<Registration>>,
}

/// Copying a command does not acknowledge or apply it. Only enrolled domains can
/// construct an acknowledgment, after fencing their actual action authority.
#[derive(Clone, Debug)]
pub struct FleetFence(Rc<Command>);

impl FleetFence {
    pub fn id(&self) -> u64 { self.0.id }
    pub fn floor(&self) -> u64 { self.0.floor }
    pub fn scope(&self) -> &FleetScope { &self.0.scope }
    pub fn issued_order(&self) -> u64 { self.0.issued_order }
    pub fn issued_at(&self) -> ElapsedTick { self.0.issued_at }
    pub fn domains(&self) -> impl Iterator<Item = u64> + '_ { self.0.targets.keys().copied() }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainFenceTransition {
    pub domain: u64,
    pub fleet_floor: u64,
    pub installed_order: u64,
    pub installed_at: ElapsedTick,
    pub control_sequence: u64,
    pub previous_epoch: u64,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

/// No constructor accepts an asserted control sequence or refund. This is still
/// only in-process evidence of the model transition, not a durable signed receipt.
#[derive(Clone, Debug)]
pub struct DomainFenceAcknowledgment {
    command: Rc<Command>,
    registration: Rc<Registration>,
    transition: DomainFenceTransition,
}

impl DomainFenceAcknowledgment {
    pub fn fence_id(&self) -> u64 { self.command.id }
    pub fn transition(&self) -> &DomainFenceTransition { &self.transition }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Admission {
    order: u64,
    at: ElapsedTick,
}

/// Snapshot of the original broker ledger. A sent-but-unresolved message is not
/// an observed external success or nonexecution; outcome remains None.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetDispatchObservation {
    pub attempt: u64,
    pub admitted_order: u64,
    pub admitted_at: ElapsedTick,
    pub state: ActionState,
    pub outcome: Option<EndpointOutcome>,
}

/// Opaque, complete snapshot of this enrolled domain's retained dispatch history.
/// Its order bounds what was observed, not future settlement or network delivery.
#[derive(Clone, Debug)]
pub struct DomainObservation {
    issuer: Rc<()>,
    registration: Rc<Registration>,
    through_order: u64,
    observed_at: ElapsedTick,
    dispatches: Vec<FleetDispatchObservation>,
}

impl DomainObservation {
    pub fn domain(&self) -> u64 { self.registration.domain }
    pub fn through_order(&self) -> u64 { self.through_order }
    pub fn observed_at(&self) -> ElapsedTick { self.observed_at }
    pub fn dispatches(&self) -> &[FleetDispatchObservation] { &self.dispatches }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FencePropagation {
    AwaitingAcknowledgment { lease_until: ElapsedTick },
    /// Enforced by the shared model clock in every enrolled admission path.
    /// NOT an acknowledgment, cancellation receipt, or evidence of drained sends.
    LeaseExpired { at: ElapsedTick },
    Acknowledged(DomainFenceTransition),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FleetDeliveryKnowledge {
    Unobserved,
    Observed {
        through_order: u64,
        observed_at: ElapsedTick,
        /// True only after a known admission stop AND a subsequent domain read.
        /// External outcomes in the retained list may still be unknown.
        post_issue_admissions_complete: bool,
        post_issue: Vec<FleetDispatchObservation>,
        unresolved: Vec<FleetDispatchObservation>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainFenceReport {
    pub scope: Scope,
    pub propagation: FencePropagation,
    pub deliveries: FleetDeliveryKnowledge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetFenceReport {
    pub id: u64,
    pub floor: u64,
    pub issued_order: u64,
    pub issued_at: ElapsedTick,
    pub observed_at: ElapsedTick,
    pub domains: BTreeMap<u64, DomainFenceReport>,
}

#[derive(Debug)]
struct FenceRecord {
    command: Rc<Command>,
    acknowledgments: BTreeMap<u64, DomainFenceAcknowledgment>,
}

/// Registers existing authorities, never constructs or divides effect budgets.
/// Membership freezes at first fence. There is no remove/rejoin, lease extension,
/// fence rollback, resume, or clearing of the retained observation history.
#[derive(Debug)]
pub struct FleetCoordinator {
    issuer: Rc<()>,
    clock: Rc<ModelOrder>,
    max_domains: usize,
    max_fences: usize,
    max_lease_ticks: u64,
    revision: u64,
    floor: u64,
    registrations: BTreeMap<u64, Rc<Registration>>,
    fences: BTreeMap<u64, FenceRecord>,
    observations: BTreeMap<u64, DomainObservation>,
}

impl FleetCoordinator {
    pub fn new(max_domains: usize, max_fences: usize, max_lease_ticks: u64) -> Result<Self, Error> {
        if max_domains == 0 || max_fences == 0 || max_lease_ticks == 0 { return Err(Error::InvalidInput); }
        if max_domains > MAX_FLEET_DOMAINS || max_fences > MAX_FLEET_FENCES { return Err(Error::Limit); }
        Ok(Self {
            issuer: Rc::new(()), clock: Rc::new(ModelOrder::default()), max_domains,
            max_fences, max_lease_ticks, revision: 0, floor: 0,
            registrations: BTreeMap::new(), fences: BTreeMap::new(), observations: BTreeMap::new(),
        })
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn observe_time(&mut self, now: ElapsedTick) -> Result<(), Error> {
        self.clock.check_time(now)?;
        self.clock.time.set(Some(now));
        Ok(())
    }

    pub fn issue_fence(&mut self, id: u64, expected_revision: u64, scope: FleetScope) -> Result<FleetFence, Error> {
        if id == 0 { return Err(Error::InvalidInput); }
        // An exact retry recovers the original command without issuing a new floor.
        if let Some(record) = self.fences.get(&id) {
            return if record.command.scope == scope { Ok(FleetFence(Rc::clone(&record.command))) }
                else { Err(Error::Binding) };
        }
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.fences.len() >= self.max_fences { return Err(Error::Limit); }
        let targets = self.select(&scope)?;
        let issued_at = self.clock.time.get().ok_or(Error::Incomplete)?;
        let issued_order = self.clock.next()?;
        let floor = self.floor.checked_add(1).ok_or(Error::Overflow)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let command = Rc::new(Command { issuer: Rc::clone(&self.issuer), id, floor, scope,
            issued_order, issued_at, targets });
        self.fences.insert(id, FenceRecord { command: Rc::clone(&command), acknowledgments: BTreeMap::new() });
        self.clock.order.set(issued_order);
        self.floor = floor;
        self.revision = revision;
        Ok(FleetFence(command))
    }

    pub fn acknowledge(&mut self, acknowledgment: DomainFenceAcknowledgment) -> Result<bool, Error> {
        if !Rc::ptr_eq(&self.issuer, &acknowledgment.command.issuer) { return Err(Error::Binding); }
        let record = self.fences.get_mut(&acknowledgment.command.id).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(&record.command, &acknowledgment.command) { return Err(Error::Binding); }
        let domain = acknowledgment.registration.domain;
        let registered = record.command.targets.get(&domain).ok_or(Error::Binding)?;
        if !Rc::ptr_eq(registered, &acknowledgment.registration) { return Err(Error::Binding); }
        if let Some(previous) = record.acknowledgments.get(&domain) {
            return if previous.transition == acknowledgment.transition { Ok(false) } else { Err(Error::Binding) };
        }
        record.acknowledgments.insert(domain, acknowledgment);
        Ok(true)
    }

    pub fn observe_domain(&mut self, observation: DomainObservation) -> Result<bool, Error> {
        if !Rc::ptr_eq(&self.issuer, &observation.issuer) { return Err(Error::Binding); }
        let domain = observation.domain();
        let registered = self.registrations.get(&domain).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(registered, &observation.registration) { return Err(Error::Binding); }
        if let Some(previous) = self.observations.get(&domain) {
            if observation.through_order < previous.through_order { return Err(Error::Stale); }
            if observation.through_order == previous.through_order {
                return if observation.dispatches == previous.dispatches && observation.observed_at == previous.observed_at {
                    Ok(false)
                } else { Err(Error::Binding) };
            }
        }
        self.observations.insert(domain, observation);
        Ok(true)
    }

    pub fn report(&self, id: u64) -> Result<FleetFenceReport, Error> {
        let command = &self.fences.get(&id).ok_or(Error::Missing)?.command;
        let now = self.clock.time.get().ok_or(Error::Incomplete)?;
        let mut domains = BTreeMap::new();
        for (domain, registration) in &command.targets {
            // A later acknowledged, stronger fence also establishes this stop,
            // but with its REAL installation order, not the earlier issue order.
            let ack = self.fences.values().filter(|r| r.command.floor >= command.floor)
                .filter_map(|r| r.acknowledgments.get(domain))
                .min_by_key(|a| a.transition.installed_order);
            let propagation = if let Some(ack) = ack {
                FencePropagation::Acknowledged(ack.transition.clone())
            } else if now >= registration.lease_until {
                FencePropagation::LeaseExpired { at: registration.lease_until }
            } else {
                FencePropagation::AwaitingAcknowledgment { lease_until: registration.lease_until }
            };
            let deliveries = match self.observations.get(domain) {
                None => FleetDeliveryKnowledge::Unobserved,
                Some(observation) => {
                    let after_stop = match &propagation {
                        FencePropagation::Acknowledged(transition) => observation.through_order >= transition.installed_order,
                        FencePropagation::LeaseExpired { at } => observation.observed_at >= *at,
                        FencePropagation::AwaitingAcknowledgment { .. } => false,
                    };
                    FleetDeliveryKnowledge::Observed {
                        through_order: observation.through_order, observed_at: observation.observed_at,
                        post_issue_admissions_complete: after_stop && observation.through_order >= command.issued_order,
                        post_issue: observation.dispatches.iter().filter(|d| d.admitted_order > command.issued_order).cloned().collect(),
                        unresolved: observation.dispatches.iter().filter(|d| d.outcome.is_none()).cloned().collect(),
                    }
                }
            };
            domains.insert(*domain, DomainFenceReport { scope: registration.scope, propagation, deliveries });
        }
        Ok(FleetFenceReport { id, floor: command.floor, issued_order: command.issued_order,
            issued_at: command.issued_at, observed_at: now, domains })
    }

    fn select(&self, scope: &FleetScope) -> Result<BTreeMap<u64, Rc<Registration>>, Error> {
        match scope {
            FleetScope::Tenant(0) | FleetScope::Principal { tenant: 0, .. }
            | FleetScope::Principal { principal: 0, .. } | FleetScope::Adapter { adapter: 0, .. }
            | FleetScope::Adapter { contract_version: 0, .. } => return Err(Error::InvalidInput),
            FleetScope::Domains(domains) => {
                if domains.is_empty() { return Err(Error::InvalidInput); }
                if domains.len() > self.max_domains { return Err(Error::Limit); }
                if domains.iter().any(|id| !self.registrations.contains_key(id)) { return Err(Error::Missing); }
            }
            _ => {}
        }
        let selected: BTreeMap<_, _> = self.registrations.iter().filter(|(domain, r)| match scope {
            FleetScope::All => true,
            FleetScope::Tenant(tenant) => r.scope.tenant == *tenant,
            FleetScope::Principal { tenant, principal } => r.scope.tenant == *tenant && r.scope.principal == *principal,
            FleetScope::Adapter { adapter, contract_version } => r.adapter == *adapter && r.contract_version == *contract_version,
            FleetScope::Domains(domains) => domains.contains(*domain),
        }).map(|(id, r)| (*id, Rc::clone(r))).collect();
        if selected.is_empty() { return Err(Error::Missing); }
        Ok(selected)
    }
}

#[derive(Debug)]
pub(super) struct FleetDomain {
    issuer: Rc<()>,
    clock: Rc<ModelOrder>,
    registration: Rc<Registration>,
    installed: Option<DomainFenceAcknowledgment>,
    admissions: BTreeMap<u64, Admission>,
}
