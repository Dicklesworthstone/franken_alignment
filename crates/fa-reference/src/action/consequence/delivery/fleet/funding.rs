//! Conserved delegation to the ORIGINAL delivery brokers (plan 8.5 and 15.1).
//!
//! One trusted, in-memory parent allocates a fixed budget across bounded child
//! authorities. Funding is not a second permit or outcome algorithm. Children
//! still use the original policy, congress, one-use permit and endpoint receipt.
//! No persistence, cross-process escrow or globally unique bootstrap is claimed.
//!
//! A returned allocation remains visible in its stopped child's historical
//! ledger. The pool subtracts it exactly once. No child can reopen admission,
//! and delayed envelopes remain charged until original endpoint reconciliation.

use super::MAX_FLEET_DOMAINS;
use super::super::{
    DeliveryBroker, DispatchEnvelope, EndpointReceipt, PublicationEndpoint, StopReceipt,
    StopRequest, StopSweep,
};
use crate::action::consequence::gate::containment::session::policy::controller::{
    ControllerConfig, PolicyReceipt, PolicyReview, PolicySession, Proposal,
};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, Scope};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;

#[derive(Debug)]
struct Allocation {
    scope: Scope,
    issued: u64,
    returned: u64,
    broker: DeliveryBroker,
}

/// Historical accounting, not a spendable delegation token or a restart image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocationInspection {
    pub scope: Scope,
    pub issued: u64,
    pub returned: u64,
    /// Available at the child, excluding amounts already returned to the parent.
    pub available: u64,
    pub reserved: u64,
    pub charged: u64,
    pub stopped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FundingInspection {
    pub tenant: u64,
    pub authority: u64,
    pub revision: u64,
    pub total: u64,
    pub unallocated: u64,
    pub available_in_domains: u64,
    pub reserved: u64,
    pub charged: u64,
    pub allocations: BTreeMap<u64, AllocationInspection>,
}

impl FundingInspection {
    pub fn conserved(&self) -> bool {
        self.unallocated.checked_add(self.available_in_domains)
            .and_then(|sum| sum.checked_add(self.reserved))
            .and_then(|sum| sum.checked_add(self.charged)) == Some(self.total)
    }
}

/// Accounting receipt only. Copying this cannot construct a child or a permit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FundingReturn {
    pub revision: u64,
    pub authority: u64,
    pub units: u64,
    pub returned_total: u64,
    pub unallocated: u64,
}

/// Owns every funded broker. No mutable broker accessor, removal, import,
/// budget top-up or Clone is provided. Dropping a borrowed domain returns no
/// budget. An independently bootstrapped pool is outside this pool's claim.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::fleet::funding::FundingPool;
/// fn duplicate(pool: FundingPool) { let _ = pool.clone(); }
/// ```
#[derive(Debug)]
pub struct FundingPool {
    tenant: u64,
    authority: u64,
    total: u64,
    unallocated: u64,
    revision: u64,
    max_domains: usize,
    allocations: BTreeMap<u64, Allocation>,
}

impl FundingPool {
    pub fn new(tenant: u64, authority: u64, total: u64, max_domains: usize) -> Result<Self, Error> {
        if tenant == 0 || authority == 0 || total == 0 || max_domains == 0 {
            return Err(Error::InvalidInput);
        }
        if max_domains > MAX_FLEET_DOMAINS { return Err(Error::Limit); }
        Ok(Self {
            tenant, authority, total, unallocated: total, revision: 0, max_domains,
            allocations: BTreeMap::new(),
        })
    }

    pub fn revision(&self) -> u64 { self.revision }

    /// A supervisor allocates config.total, not a new independent budget. All
    /// pool checks precede endpoint attachment. A rejected configuration leaves
    /// the original pool untouched; the original constructor validates policy.
    /// Child authority identities are retained permanently, including on stop.
    pub fn fund_domain(&mut self, expected_revision: u64, config: ControllerConfig,
        endpoint: &mut PublicationEndpoint) -> Result<(), Error>
    {
        if expected_revision != self.revision { return Err(Error::Stale); }
        let scope = config.scope;
        if scope.tenant != self.tenant || scope.authority == self.authority
            || scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if self.allocations.contains_key(&scope.authority) { return Err(Error::Duplicate); }
        if self.allocations.len() >= self.max_domains { return Err(Error::Limit); }
        let issued = config.total;
        let unallocated = self.unallocated.checked_sub(issued).ok_or(Error::Limit)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let broker = DeliveryBroker::new(config, endpoint)?;
        self.allocations.insert(scope.authority, Allocation { scope, issued, returned: 0, broker });
        self.unallocated = unallocated;
        self.revision = revision;
        Ok(())
    }

    /// A limited borrow, never an independently replaceable inner broker.
    pub fn domain(&mut self, authority: u64) -> Result<FundedDomain<'_>, Error> {
        let allocation = self.allocations.get_mut(&authority).ok_or(Error::Missing)?;
        Ok(FundedDomain { broker: &mut allocation.broker })
    }

    /// Collect only actual available rights from a PERMANENTLY stopped child.
    /// A local stop may release undispatched reservations; sent/unknown effects
    /// stay charged. Later NotExecuted receipts can make more rights available.
    /// Repeated collection never transfers the same units twice. The original
    /// child and all of its receipt/attempt tombstones are retained.
    pub fn collect_returned(&mut self, expected_revision: u64, authority: u64)
        -> Result<FundingReturn, Error>
    {
        if expected_revision != self.revision { return Err(Error::Stale); }
        let allocation = self.allocations.get(&authority).ok_or(Error::Missing)?;
        if allocation.broker.stop_receipt().is_none() { return Err(Error::WrongState); }
        let ledger = allocation.broker.inspect().ledger;
        if ledger.reserved != 0 || ledger.available.checked_add(ledger.charged) != Some(allocation.issued) {
            return Err(Error::WrongState);
        }
        let units = ledger.available.checked_sub(allocation.returned).ok_or(Error::WrongState)?;
        let unallocated = self.unallocated.checked_add(units).ok_or(Error::Overflow)?;
        if unallocated > self.total { return Err(Error::WrongState); }
        let revision = if units == 0 { self.revision }
            else { self.revision.checked_add(1).ok_or(Error::Overflow)? };
        let receipt = FundingReturn {
            revision, authority, units, returned_total: ledger.available, unallocated,
        };
        self.allocations.get_mut(&authority).expect("checked allocation").returned = ledger.available;
        self.unallocated = unallocated;
        self.revision = revision;
        Ok(receipt)
    }

    /// Fold original broker accounting, rather than trusting a caller's claimed
    /// spend or refund. Arithmetic failure never produces a partial balance.
    pub fn inspect(&self) -> Result<FundingInspection, Error> {
        let mut allocations = BTreeMap::new();
        let mut available_in_domains = 0_u64;
        let mut reserved = 0_u64;
        let mut charged = 0_u64;
        for (authority, allocation) in &self.allocations {
            let ledger = allocation.broker.inspect().ledger;
            if ledger.available.checked_add(ledger.reserved)
                .and_then(|sum| sum.checked_add(ledger.charged)) != Some(allocation.issued)
            { return Err(Error::WrongState); }
            let available = ledger.available.checked_sub(allocation.returned).ok_or(Error::WrongState)?;
            available_in_domains = available_in_domains.checked_add(available).ok_or(Error::Overflow)?;
            reserved = reserved.checked_add(ledger.reserved).ok_or(Error::Overflow)?;
            charged = charged.checked_add(ledger.charged).ok_or(Error::Overflow)?;
            allocations.insert(*authority, AllocationInspection {
                scope: allocation.scope, issued: allocation.issued, returned: allocation.returned,
                available, reserved: ledger.reserved, charged: ledger.charged,
                stopped: allocation.broker.stop_receipt().is_some(),
            });
        }
        let result = FundingInspection {
            tenant: self.tenant, authority: self.authority, revision: self.revision,
            total: self.total, unallocated: self.unallocated, available_in_domains,
            reserved, charged, allocations,
        };
        if !result.conserved() { return Err(Error::WrongState); }
        Ok(result)
    }
}

/// Mutations delegate to the original broker; none accepts a raw outcome or
/// changes its funding. The read-only accessor cannot be used to replace it.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::fleet::funding::FundedDomain;
/// fn replace(domain: &mut FundedDomain<'_>) { let _ = domain.broker_mut(); }
/// ```
#[derive(Debug)]
pub struct FundedDomain<'a> {
    broker: &'a mut DeliveryBroker,
}

impl FundedDomain<'_> {
    /// Original historical ledger; a stopped child's available balance includes
    /// returned tombstones. Use FundingPool::inspect for spendable pool balances.
    pub fn broker(&self) -> &DeliveryBroker { self.broker }
    pub fn observe_time(&mut self, now: ElapsedTick) -> Result<(), Error> { self.broker.observe_time(now) }

    pub fn confirm_endpoint_fence(&mut self, endpoint: &mut PublicationEndpoint) -> Result<(), Error> {
        let acknowledgment = endpoint.install_fence(self.broker.fence_request())?;
        self.broker.confirm_fence(acknowledgment)
    }

    pub fn propose(&mut self, id: u64, action: ActionSpec, snapshot: &Snapshot) -> Result<Proposal, Error> {
        self.broker.propose(id, action, snapshot)
    }
    pub fn begin_review(&self, id: u64, round: u64, root: [u8; 32], snapshot: &Snapshot)
        -> Result<PolicySession, Error>
    { self.broker.begin_review(id, round, root, snapshot) }
    pub fn apply_review(&mut self, review: PolicyReview, snapshot: &Snapshot) -> Result<PolicyReceipt, Error> {
        self.broker.apply_review(review, snapshot)
    }
    pub fn authorize(&mut self, id: u64, snapshot: &Snapshot) -> Result<Permit, Error> {
        self.broker.authorize(id, snapshot)
    }
    pub fn dispatch(&mut self, permit: &Permit, action: &FrozenAction, snapshot: &Snapshot)
        -> Result<DispatchEnvelope, Error>
    { self.broker.dispatch(permit, action, snapshot) }
    pub fn cancel(&mut self, id: u64) -> Result<(), Error> { self.broker.cancel(id) }
    pub fn acknowledgment_lost(&mut self, id: u64) -> Result<(), Error> { self.broker.acknowledgment_lost(id) }
    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> {
        self.broker.accept_receipt(receipt)
    }
    pub fn abandon_unknown(&mut self, id: u64) -> Result<(), Error> { self.broker.abandon_unknown(id) }
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, Error> {
        self.broker.request_stop(request)
    }
    pub fn progress_stop(&mut self, endpoint: &mut PublicationEndpoint) -> Result<StopSweep, Error> {
        self.broker.progress_stop(endpoint)
    }
}

#[cfg(test)]
mod tests;
