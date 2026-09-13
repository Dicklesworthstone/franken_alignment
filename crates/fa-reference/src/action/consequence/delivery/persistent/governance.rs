//! Durable trusted policy changes through the original authority transition.
//! A policy update is data for the supervisor, never an actor or effect permit.

pub(super) mod codec;

use super::{Event, FileDelivery, JournalError, Machine, Transition};
use crate::action::consequence::gate::containment::session::policy::{
    Policy, controller::{PolicyChange, MAX_POLICY_CHANGES},
};
use crate::Error;

/// Exact identity of a supervisor operation. All fields participate in retry
/// binding, including both predecessors and every node/literal of the policy.
/// The journal revision is a concurrency precondition, not part of this identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyUpdate {
    operation: u64,
    expected_control_sequence: u64,
    expected_authority_epoch: u64,
    policy: Policy,
}
impl PolicyUpdate {
    pub fn new(operation: u64, expected_control_sequence: u64,
        expected_authority_epoch: u64, policy: Policy) -> Result<Self, Error>
    {
        if operation == 0 { return Err(Error::InvalidInput); }
        Ok(Self { operation, expected_control_sequence, expected_authority_epoch, policy })
    }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn expected_control_sequence(&self) -> u64 { self.expected_control_sequence }
    pub fn expected_authority_epoch(&self) -> u64 { self.expected_authority_epoch }
    pub fn policy(&self) -> &Policy { &self.policy }
}

/// Acknowledged original change, not proof of external nonexecution. Exact retry
/// may return an older receipt: inspect current_policy separately for live policy.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdateReceipt;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn refund(change: PolicyUpdateReceipt) -> EndpointReceipt { change }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyUpdateReceipt { update: PolicyUpdate, change: PolicyChange }
impl PolicyUpdateReceipt {
    pub fn update(&self) -> &PolicyUpdate { &self.update }
    pub fn change(&self) -> &PolicyChange { &self.change }
}

/// Recomputed from original transitions, never decoded receipt or balance data.
/// Bounded by the original controller's lifetime policy-change allowance.
#[derive(Default)]
pub(super) struct PolicyUpdates { history: Vec<PolicyUpdateReceipt> }
impl PolicyUpdates {
    pub(super) fn receipt(&self, operation: u64) -> Option<&PolicyUpdateReceipt> {
        self.history.iter().find(|receipt| receipt.update.operation == operation)
    }
    pub(super) fn retry(&self, update: &PolicyUpdate) -> Result<Option<PolicyUpdateReceipt>, Error> {
        self.receipt(update.operation).map(|receipt| {
            if receipt.update == *update { Ok(receipt.clone()) } else { Err(Error::Binding) }
        }).transpose()
    }
    pub(super) fn preflight(&mut self, update: &PolicyUpdate) -> Result<(), Error> {
        if self.receipt(update.operation).is_some() { return Err(Error::Duplicate); }
        if self.history.len() >= MAX_POLICY_CHANGES { return Err(Error::Limit); }
        self.history.try_reserve(1).map_err(|_| Error::Limit)
    }
    pub(super) fn record(&mut self, update: &PolicyUpdate, change: PolicyChange) -> PolicyUpdateReceipt {
        let receipt = PolicyUpdateReceipt { update: update.clone(), change };
        self.history.push(receipt.clone());
        receipt
    }
    pub(super) fn current<'a>(&'a self, bootstrap: &'a Policy) -> &'a Policy {
        self.history.last().map_or(bootstrap, |receipt| receipt.change.policy.as_ref())
    }
}

impl Machine {
    pub(super) fn apply_policy_update(&mut self, update: &PolicyUpdate) -> Result<Transition, Error> {
        self.policy_updates.preflight(update)?;
        let change = self.broker.replace_policy(update.expected_control_sequence,
            update.expected_authority_epoch, update.policy.clone())?;
        for id in &change.cancelled { self.permits.remove(id); }
        // An admitted dispatch has a defined serial order before this update.
        // Its envelope and endpoint outcome are NOT revoked or refunded here.
        let receipt = self.policy_updates.record(update, change);
        Ok(Transition::PolicyUpdated(receipt))
    }
}

impl FileDelivery {
    /// One canonical transaction, using the original policy/rights transition.
    /// Neither current helper evidence nor a fresh clock is needed for governance.
    /// Only an exact healthy-owner retry bypasses the supplied journal predecessor.
    pub fn replace_policy(&mut self, revision: u64, update: &PolicyUpdate)
        -> Result<PolicyUpdateReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(receipt) = self.machine.policy_updates.retry(update)? { return Ok(receipt); }
        match self.transact(revision, Event::ReplacePolicy(update.clone()))? {
            Transition::PolicyUpdated(receipt) => Ok(receipt),
            _ => unreachable!("policy update transition"),
        }
    }
    pub fn policy_update_receipt(&self, operation: u64) -> Result<&PolicyUpdateReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.policy_updates.receipt(operation).ok_or_else(|| Error::Missing.into())
    }
    /// Last acknowledged effective policy. A faulted owner refuses this read;
    /// canonical replacement may be newer than its last acknowledged projection.
    pub fn current_policy(&self) -> Result<&Policy, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.policy_updates.current(&self.profile.policy))
    }
}
