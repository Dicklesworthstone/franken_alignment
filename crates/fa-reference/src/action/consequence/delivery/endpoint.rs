//! A separately owned, atomic reference publication endpoint.
//!
//! It owns actual bounded payload/version state in this model, not an always-OK
//! callback. Both the endpoint and control state are assumed to survive a
//! dispatcher interruption. No disk, network, provider or crash durability is
//! established. Time inputs share a declared logical clock domain.

mod recovery;

use super::*;
use crate::action::MAX_PAYLOAD_BYTES;

#[derive(Debug)]
pub struct PublicationEndpoint {
    pub(super) binding: Rc<()>,
    pub(super) retention_ticks: u64,
    pub(super) max_deliveries: usize,
    resource: ResolvedTarget,
    payload: Vec<u8>,
    scope: Option<Scope>,
    epoch: u64,
    elapsed: Option<ElapsedTick>,
    receipts: BTreeMap<u64, EndpointReceipt>,
    executions: u64,
}

impl PublicationEndpoint {
    pub fn new(
        resource: ResolvedTarget, payload: Vec<u8>, retention_ticks: u64, max_deliveries: usize,
    ) -> Result<Self, Error> {
        if [resource.adapter, resource.object, resource.contract_version,
            resource.expected_version, resource.generation, retention_ticks].contains(&0)
        {
            return Err(Error::InvalidInput);
        }
        if max_deliveries == 0 || max_deliveries > MAX_DELIVERIES || payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Limit);
        }
        Ok(Self {
            binding: Rc::new(()), retention_ticks, max_deliveries, resource, payload,
            scope: None, epoch: 0, elapsed: None, receipts: BTreeMap::new(), executions: 0,
        })
    }

    pub(super) fn attach(&mut self, scope: Scope) -> Result<(), Error> {
        if self.scope.is_some() { return Err(Error::Duplicate); }
        self.scope = Some(scope);
        Ok(())
    }

    pub fn target(&self) -> ResolvedTarget { self.resource }
    pub fn payload(&self) -> &[u8] { &self.payload }
    pub fn execution_count(&self) -> u64 { self.executions }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        if self.elapsed.is_some_and(|previous| tick < previous) { return Err(Error::Stale); }
        self.elapsed = Some(tick);
        Ok(())
    }

    /// Repeating a fence is idempotent. A delayed old fence never lowers it.
    /// Installing it does not claim that previously accepted effects vanished.
    pub fn install_fence(&mut self, request: FenceRequest) -> Result<FenceAcknowledgment, Error> {
        if !Rc::ptr_eq(&self.binding, &request.binding) || self.scope.is_none() {
            return Err(Error::Binding);
        }
        if request.epoch < self.epoch { return Err(Error::Stale); }
        self.epoch = request.epoch;
        Ok(FenceAcknowledgment { binding: Rc::clone(&self.binding), epoch: self.epoch })
    }

    /// Apply one compare-and-publish operation, or replay its terminal receipt.
    /// A terminal nonexecution record also blocks delayed/duplicated messages.
    pub fn deliver(&mut self, message: &DispatchEnvelope) -> Result<EndpointReceipt, Error> {
        let now = self.validate(message)?;
        if message.epoch != self.epoch { return Err(Error::Stale); }
        if now >= message.retained_until { return Err(Error::Stale); }
        if let Some(receipt) = self.existing(message)? { return Ok(receipt.clone()); }
        if self.receipts.len() >= self.max_deliveries { return Err(Error::Limit); }
        if now >= message.request.deadline {
            return Ok(self.record(message, EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::DeadlineElapsed,
            }));
        }
        if message.request.target.expected_version != self.resource.expected_version {
            return Ok(self.record(message, EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::VersionConflict,
            }));
        }
        let version = self.resource.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        let executions = self.executions.checked_add(1).ok_or(Error::Overflow)?;
        let payload = message.request.payload.clone();
        let receipt = self.record(message, EndpointOutcome::Executed { resulting_version: version });
        self.payload = payload;
        self.resource.expected_version = version;
        self.executions = executions;
        Ok(receipt)
    }

    /// A lookup miss is deliberately not a terminal receipt: the request may
    /// still be queued. Expired status cannot be relabeled as nonexecution.
    pub fn status(&self, query: &StatusQuery) -> Result<EndpointStatus, Error> {
        let now = self.validate(&query.0)?;
        if now >= query.0.retained_until { return Ok(EndpointStatus::RetentionExpired); }
        Ok(match self.existing(&query.0)? {
            Some(receipt) => EndpointStatus::Resolved(receipt.clone()),
            None => EndpointStatus::AwaitingResolution,
        })
    }

    /// Atomically establish nonexecution AND prevent execution of every future
    /// delivery under this exact key. If execution won the race, return its real
    /// receipt instead. This operation is stronger than querying for absence.
    pub fn seal_unexecuted(&mut self, query: &StatusQuery) -> Result<EndpointReceipt, Error> {
        let now = self.validate(&query.0)?;
        if query.0.epoch != self.epoch || now >= query.0.retained_until { return Err(Error::Stale); }
        if let Some(receipt) = self.existing(&query.0)? { return Ok(receipt.clone()); }
        if self.receipts.len() >= self.max_deliveries { return Err(Error::Limit); }
        Ok(self.record(&query.0, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }))
    }

    fn validate(&self, message: &DispatchEnvelope) -> Result<ElapsedTick, Error> {
        if !Rc::ptr_eq(&self.binding, &message.binding) { return Err(Error::Binding); }
        let scope = self.scope.ok_or(Error::Incomplete)?;
        if message.request.scope != scope || !same_resource(message.request.target, self.resource) {
            return Err(Error::Binding);
        }
        let bytes = u64::try_from(message.request.payload.len()).map_err(|_| Error::Limit)?;
        if bytes > message.request.units || message.request.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Limit);
        }
        self.elapsed.ok_or(Error::Incomplete)
    }

    fn existing(&self, message: &DispatchEnvelope) -> Result<Option<&EndpointReceipt>, Error> {
        let receipt = self.receipts.get(&message.attempt);
        if let Some(receipt) = receipt {
            if receipt.request != message.request || receipt.retained_until != message.retained_until {
                return Err(Error::Binding);
            }
        }
        Ok(receipt)
    }

    fn record(&mut self, message: &DispatchEnvelope, outcome: EndpointOutcome) -> EndpointReceipt {
        let receipt = EndpointReceipt {
            binding: Rc::clone(&self.binding), attempt: message.attempt,
            request: message.request.clone(), retained_until: message.retained_until, outcome,
        };
        self.receipts.insert(message.attempt, receipt.clone());
        receipt
    }
}
