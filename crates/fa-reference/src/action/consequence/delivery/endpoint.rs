//! A separately owned, atomic reference publication endpoint.
//!
//! It owns bounded payload/version state, not an always-OK callback. The optional
//! stream profile only appends complete reviewed messages and seals an explicit
//! finish. No disk, network, provider or OS-crash durability is established.

mod recovery;

use super::*;
use super::stream::{StreamProfile, StreamView};
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
    stream: Option<StreamView>,
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
            stream: None,
        })
    }

    /// Select append-only complete-message semantics before attaching a broker.
    /// The registered resource contract must identify this mode. No mode-change
    /// operation exists. Raw tool arguments are not dispatched by this endpoint.
    pub fn new_stream(
        resource: ResolvedTarget, profile: StreamProfile, retention_ticks: u64, max_deliveries: usize,
    ) -> Result<Self, Error> {
        if profile.max_messages() + 1 > max_deliveries { return Err(Error::Limit); }
        let mut endpoint = Self::new(resource, Vec::new(), retention_ticks, max_deliveries)?;
        endpoint.stream = Some(StreamView::empty(profile));
        Ok(endpoint)
    }

    pub(super) fn attach(&mut self, scope: Scope) -> Result<(), Error> {
        if self.scope.is_some() { return Err(Error::Duplicate); }
        self.scope = Some(scope);
        Ok(())
    }

    pub fn target(&self) -> ResolvedTarget { self.resource }
    pub fn payload(&self) -> &[u8] { &self.payload }
    pub fn execution_count(&self) -> u64 { self.executions }
    /// Actual endpoint-visible messages. A broker's confirmed view can lag it.
    pub fn stream_view(&self) -> Option<&StreamView> { self.stream.as_ref() }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        if self.elapsed.is_some_and(|previous| now_before(tick, previous)) { return Err(Error::Stale); }
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

    /// One compare-and-publish step, or its retained terminal receipt. In stream
    /// mode compare the entire actual prefix AND its message boundaries before
    /// revealing anything. Finishing never deletes a previously visible prefix.
    pub fn deliver(&mut self, message: &DispatchEnvelope) -> Result<EndpointReceipt, Error> {
        let now = self.validate(message)?;
        if message.epoch != self.epoch { return Err(Error::Stale); }
        if now >= message.retained_until { return Err(Error::Stale); }
        // A historical execution wins over later expiry; it cannot be refunded.
        if let Some(receipt) = self.existing(message)? { return Ok(receipt.clone()); }
        if self.receipts.len() >= self.max_deliveries { return Err(Error::Limit); }
        if message.request.approval.is_some_and(|approval| now < approval.issued_at()) {
            return Err(Error::Stale);
        }
        if now >= message.request.execution_deadline() {
            return Ok(self.record(message, EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::DeadlineElapsed,
            }));
        }
        if message.request.target.expected_version != self.resource.expected_version {
            return Ok(self.record(message, EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::VersionConflict,
            }));
        }
        let next_stream = match &self.stream {
            Some(stream) => match stream.advance(&message.request.payload) {
                Ok(next) => Some(next),
                Err(_) => return Ok(self.record(message, EndpointOutcome::NotExecuted {
                    reason: NonExecutionReason::StreamRejected,
                })),
            },
            None => None,
        };
        let version = self.resource.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        let executions = self.executions.checked_add(1).ok_or(Error::Overflow)?;
        let payload = match &next_stream {
            Some(stream) => stream.visible().to_vec(),
            None => message.request.payload.clone(),
        };
        let receipt = self.record(message, EndpointOutcome::Executed { resulting_version: version });
        self.payload = payload;
        self.stream = next_stream;
        self.resource.expected_version = version;
        self.executions = executions;
        Ok(receipt)
    }

    /// A lookup miss is not a terminal receipt: the request may still be queued.
    pub fn status(&self, query: &StatusQuery) -> Result<EndpointStatus, Error> {
        let now = self.validate(&query.0)?;
        if now >= query.0.retained_until { return Ok(EndpointStatus::RetentionExpired); }
        Ok(match self.existing(&query.0)? {
            Some(receipt) => EndpointStatus::Resolved(receipt.clone()),
            None => EndpointStatus::AwaitingResolution,
        })
    }

    /// Establish nonexecution AND prevent every future delivery under this key.
    /// If execution won, return its real receipt. A chunk seal is not stream EOF.
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

fn now_before(tick: ElapsedTick, previous: ElapsedTick) -> bool { tick < previous }
