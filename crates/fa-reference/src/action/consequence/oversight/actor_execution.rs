//! Trusted execution of actor requests through the original one-shot broker.
//! No method is available on ActorPort; payloads come from the retained action.

use super::{CommitteeInput, actor::ActorSupervisor, human::HumanPermit};
use crate::action::consequence::delivery::{DispatchEnvelope, EndpointReceipt, EndpointStatus, PublicationEndpoint};
use crate::action::{ActionState, Permit};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;

/// Borrowed authorization, never copied rights or a permit inferred from status.
/// The automatic permit and optional human key are independently revalidated.
pub struct DispatchKeys<'a> {
    pub automatic: &'a Permit,
    pub human: Option<&'a HumanPermit>,
}

impl<'a> DispatchKeys<'a> {
    pub fn single(automatic: &'a Permit) -> Self { Self { automatic, human: None } }
    pub fn two(automatic: &'a Permit, human: &'a HumanPermit) -> Self {
        Self { automatic, human: Some(human) }
    }
}

pub type ReconciliationResults = BTreeMap<u64, Result<EndpointStatus, Error>>;

impl ActorSupervisor {
    /// Reserve only through the existing policy, congress, evidence, topology
    /// and identity checks. Only the supervisor receives the resulting permit.
    pub fn authorize_request(
        &mut self, request: u64, current: Option<&CommitteeInput>, snapshot: &Snapshot,
    ) -> Result<Permit, Error> {
        self.synchronize()?;
        let attempt = self.attempt(request)?;
        let result = self.broker_mut().authorize(attempt, current, snapshot);
        self.synchronize()?;
        result
    }

    /// Consume the original permit exactly once and publish uncertainty before
    /// returning a sendable envelope. Even identical actions cannot exchange
    /// permits between request IDs. Errors never trigger automatic re-proposal.
    pub fn dispatch_request(
        &mut self, request: u64, keys: DispatchKeys<'_>,
        current: Option<&CommitteeInput>, snapshot: &Snapshot,
    ) -> Result<DispatchEnvelope, Error> {
        self.synchronize()?;
        let attempt = self.attempt(request)?;
        if keys.automatic.attempt != attempt { return Err(Error::Binding); }
        if self.broker().inspect().ledger.stages.get(&attempt) != Some(&ActionState::Authorized) {
            return Err(Error::WrongState);
        }
        let action = self.action(request)?.clone();
        let result = match (self.broker().human_review_required(), keys.human) {
            (true, Some(human)) => self.broker_mut().dispatch_with_human(keys.automatic, human, &action, current, snapshot),
            (false, None) => self.broker_mut().dispatch(keys.automatic, &action, current, snapshot),
            (true, None) => Err(Error::Incomplete),
            (false, Some(_)) => Err(Error::Binding),
        };
        self.synchronize()?;
        result
    }

    /// Execute once against the existing endpoint (memory or its file profile).
    /// Every endpoint error is conservatively unknown, including a failure after
    /// a visible file publication. The caller must reconcile, never resend here.
    pub fn deliver_request(
        &mut self, request: u64, keys: DispatchKeys<'_>, current: Option<&CommitteeInput>,
        snapshot: &Snapshot, endpoint: &mut PublicationEndpoint,
    ) -> Result<EndpointReceipt, Error> {
        let message = self.dispatch_request(request, keys, current, snapshot)?;
        match endpoint.deliver(&message) {
            Ok(receipt) => {
                if let Err(error) = self.accept_receipt(receipt.clone()) {
                    self.acknowledgment_lost(request)?;
                    return Err(error);
                }
                Ok(receipt)
            }
            Err(error) => {
                self.acknowledgment_lost(request)?;
                Err(error)
            }
        }
    }

    /// Missing acknowledgments never become cancellation or a refund, even if
    /// the actor has subsequently requested cancellation of the original ticket.
    pub fn acknowledgment_lost(&mut self, request: u64) -> Result<(), Error> {
        let attempt = self.attempt(request)?;
        let result = self.broker_mut().acknowledgment_lost(attempt);
        self.synchronize()?;
        result
    }

    /// Only the registered endpoint can mint a receipt the original ledger
    /// accepts. Duplicates remain idempotent, including the actor projection.
    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> {
        let result = self.broker_mut().accept_receipt(receipt);
        self.synchronize()?;
        result
    }

    /// Uses the existing bounded sweep, not a retry loop or another outcome
    /// store. Map keys are privileged ledger attempts; this map never reaches
    /// the actor. Each request receives only its redacted outcome projection.
    pub fn reconcile_pending(&mut self, endpoint: &mut PublicationEndpoint) -> Result<ReconciliationResults, Error> {
        let result = self.broker_mut().reconcile_pending(endpoint);
        self.synchronize()?;
        result
    }
}
