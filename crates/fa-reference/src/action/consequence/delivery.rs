//! Receipt-gated delivery over the existing exact-policy controller.
//!
//! The controller is a bounded, in-memory protocol model. Its endpoint supports
//! memory-only operation and a Unix local-filesystem publication profile. Neither
//! is an authenticated provider or a durable production authority ledger.
//! A dispatched message may be delayed after its caller loses the acknowledgment.
//! A missing status is consequently never a nonexecution proof or a refund.

mod approval;
mod endpoint;
mod mediation_gate;
mod state_gate;
mod stopping;
pub mod fleet;
pub mod stream;
pub use approval::DispatchApproval;
pub use endpoint::PublicationEndpoint;
#[cfg(unix)]
pub use endpoint::filesystem::{self, FileEndpointRecovery, FilePublicationLimits};
pub use mediation_gate::TopologyChange;
pub use state_gate::{PolicySourceChange, MAX_POLICY_STATE_GENERATIONS};
pub use stopping::{StopProgress, StopReceipt, StopRequest, StopSweep};

use super::gate::containment::session::policy::Policy;
use super::gate::containment::session::policy::controller::{
    ControllerConfig, PolicyAuthority, PolicyChange, PolicyReceipt, PolicyReview, PolicySession,
    Proposal,
};
use super::gate::containment::{ActorState, CheckpointHandle, ResetReceipt, ResetRequest};
use super::gate::ControlInspection;
use super::mediation::VerifiedCut;
use super::oversight::policy_state::CapturedSnapshot;
use crate::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, ResolvedTarget, Scope, TrustedOutcome,
};
use crate::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;
use stream::StreamView;

pub const MAX_DELIVERIES: usize = 128;
/// Counts each retained frozen action once, not allocator or transport copies.
pub const MAX_DELIVERY_ACTION_BYTES: usize = 2 * 1_024 * 1_024;

/// Exact execution-bearing fields only. Policy witnesses, helper votes and actor
/// checkpoints never enter the endpoint's request, query or receipt types.
/// It cannot be constructed or edited independently of the consumed permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationRequest {
    version: u32,
    scope: Scope,
    target: ResolvedTarget,
    payload: Vec<u8>,
    policy_epoch: u64,
    deadline: ElapsedTick,
    units: u64,
    approval: Option<DispatchApproval>,
}

impl PublicationRequest {
    pub fn scope(&self) -> Scope { self.scope }
    pub fn target(&self) -> ResolvedTarget { self.target }
    pub fn payload(&self) -> &[u8] { &self.payload }
    pub fn policy_epoch(&self) -> u64 { self.policy_epoch }
    /// The deadline in the original frozen action, never rewritten after review.
    pub fn deadline(&self) -> ElapsedTick { self.deadline }
    pub fn units(&self) -> u64 { self.units }
    pub fn approval(&self) -> Option<DispatchApproval> { self.approval }

    /// A second key can shorten, but never extend, the execution window.
    pub fn execution_deadline(&self) -> ElapsedTick {
        self.approval.map_or(self.deadline, |approval| self.deadline.min(approval.expires_at()))
    }

    fn from_action(action: &FrozenAction) -> Self {
        let spec = action.spec();
        Self {
            version: spec.version, scope: spec.scope, target: spec.target.expect("frozen target"),
            payload: spec.payload.clone(), policy_epoch: spec.policy_epoch,
            deadline: spec.deadline, units: spec.units, approval: None,
        }
    }

    fn matches_action(&self, action: &FrozenAction) -> bool {
        let spec = action.spec();
        self.version == spec.version && self.scope == spec.scope && Some(self.target) == spec.target
            && self.payload == spec.payload && self.policy_epoch == spec.policy_epoch
            && self.deadline == spec.deadline && self.units == spec.units
    }
}

/// A copyable message is data about an already consumed permit. Only the broker
/// constructs it. Re-delivering it to the registered endpoint cannot create a
/// second publication under the same key. It is not an authorization interface.
#[derive(Clone, Debug)]
pub struct DispatchEnvelope {
    binding: Rc<()>,
    epoch: u64,
    attempt: u64,
    request: PublicationRequest,
    retained_until: ElapsedTick,
}

impl DispatchEnvelope {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn request(&self) -> &PublicationRequest { &self.request }
    pub fn retained_until(&self) -> ElapsedTick { self.retained_until }
}

/// Status queries cannot be converted into dispatch messages. Sealing this key
/// at the endpoint prevents a delayed dispatch from executing later.
#[derive(Clone, Debug)]
pub struct StatusQuery(DispatchEnvelope);

impl StatusQuery {
    pub fn attempt(&self) -> u64 { self.0.attempt }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonExecutionReason {
    Sealed,
    VersionConflict,
    /// First execution reached the frozen action or the second key's deadline.
    DeadlineElapsed,
    StreamRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointOutcome {
    Executed { resulting_version: u64 },
    NotExecuted { reason: NonExecutionReason },
}

/// Opaque terminal evidence minted only by the registered reference endpoint.
/// Its process-local brand is not a signature or a wire authentication scheme.
///
/// ```compile_fail
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn forge() -> EndpointReceipt { EndpointReceipt {} }
/// ```
#[derive(Clone, Debug)]
pub struct EndpointReceipt {
    binding: Rc<()>,
    attempt: u64,
    request: PublicationRequest,
    retained_until: ElapsedTick,
    outcome: EndpointOutcome,
}

impl EndpointReceipt {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn request(&self) -> &PublicationRequest { &self.request }
    pub fn outcome(&self) -> EndpointOutcome { self.outcome }
}

impl PartialEq for EndpointReceipt {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.binding, &other.binding) && self.attempt == other.attempt
            && self.request == other.request && self.retained_until == other.retained_until
            && self.outcome == other.outcome
    }
}

impl Eq for EndpointReceipt {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointStatus {
    /// There is no terminal record YET. A delayed message can still arrive.
    AwaitingResolution,
    /// The declared retention interval no longer supports a status claim.
    RetentionExpired,
    Resolved(EndpointReceipt),
}

#[derive(Clone, Debug)]
pub struct FenceRequest {
    binding: Rc<()>,
    epoch: u64,
}

#[derive(Clone, Debug)]
pub struct FenceAcknowledgment {
    binding: Rc<()>,
    epoch: u64,
}

#[derive(Debug)]
struct DeliveryRecord {
    action: FrozenAction,
    approval: Option<DispatchApproval>,
    mediation: Option<VerifiedCut>,
    policy_state: Option<CapturedSnapshot>,
    retained_until: ElapsedTick,
    resolution: Option<EndpointReceipt>,
}

/// Owns the policy controller. There is no mutable controller accessor, raw
/// dispatch, raw TrustedOutcome input, or automatic retry with a fresh key.
/// This profile accounts resource units as an upper bound on payload bytes.
#[derive(Debug)]
pub struct DeliveryBroker {
    controller: PolicyAuthority,
    binding: Rc<()>,
    scope: Scope,
    resource: ResolvedTarget,
    retention_ticks: u64,
    max_deliveries: usize,
    epoch: u64,
    fenced: bool,
    records: BTreeMap<u64, DeliveryRecord>,
    action_bytes: usize,
    stream: Option<StreamView>,
    stream_pending: Option<u64>,
    fleet: Option<fleet::FleetDomain>,
    mediation: Option<mediation_gate::MediationState>,
    policy_state: Option<state_gate::CapturedStateGate>,
    stop: Option<StopReceipt>,
}

impl DeliveryBroker {
    pub fn new(config: ControllerConfig, endpoint: &mut PublicationEndpoint) -> Result<Self, Error> {
        let scope = config.scope;
        let controller = PolicyAuthority::new(config)?;
        endpoint.attach(scope)?;
        Ok(Self {
            controller,
            binding: Rc::clone(&endpoint.binding),
            scope,
            resource: endpoint.target(),
            retention_ticks: endpoint.retention_ticks,
            max_deliveries: endpoint.max_deliveries,
            epoch: 0,
            fenced: false,
            records: BTreeMap::new(),
            action_bytes: 0,
            stream: endpoint.stream_view().cloned(),
            stream_pending: None,
            fleet: None,
            mediation: None,
            policy_state: None,
            stop: None,
        })
    }

    /// Inspection and starting a review are read-only; no mutable authority leaks.
    pub fn controller(&self) -> &PolicyAuthority { &self.controller }
    pub fn inspect(&self) -> ControlInspection { self.controller.inspect() }
    pub fn dispatcher_epoch(&self) -> u64 { self.epoch }
    pub fn fence_confirmed(&self) -> bool { self.fenced }

    /// Receipt-confirmed prefix and resource version, NOT a fresh remote read.
    /// When pending is Some, the actual audience may already have seen more.
    pub fn stream_state(&self) -> Option<(ResolvedTarget, &StreamView)> {
        self.stream.as_ref().map(|view| (self.resource, view))
    }
    pub fn stream_pending(&self) -> Option<u64> { self.stream_pending }

    pub fn fence_request(&self) -> FenceRequest {
        FenceRequest { binding: Rc::clone(&self.binding), epoch: self.epoch }
    }

    pub fn confirm_fence(&mut self, acknowledgment: FenceAcknowledgment) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.binding, &acknowledgment.binding) {
            return Err(Error::Binding);
        }
        if acknowledgment.epoch != self.epoch {
            return Err(Error::Stale);
        }
        self.fenced = true;
        Ok(())
    }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        self.check_fleet_time(tick)?;
        self.controller.observe_time(tick)?;
        self.publish_fleet_time(tick);
        Ok(())
    }

    pub fn propose(&mut self, id: u64, spec: ActionSpec, snapshot: &Snapshot) -> Result<Proposal, Error> {
        self.check_not_stopping()?;
        let _ = self.check_policy_state(snapshot)?;
        self.check_mediation()?;
        self.check_fleet()?;
        check_resource(&spec, self.scope, self.resource)?;
        self.check_stream(&spec)?;
        self.controller.propose(id, spec, snapshot)
    }

    pub fn begin_review(
        &self, id: u64, round: u64, root: [u8; 32], snapshot: &Snapshot,
    ) -> Result<PolicySession, Error> {
        self.check_not_stopping()?;
        let _ = self.check_policy_state(snapshot)?;
        self.controller.begin_review(id, round, root, snapshot)
    }

    pub fn apply_review(&mut self, review: PolicyReview, snapshot: &Snapshot) -> Result<PolicyReceipt, Error> {
        if review.decision().consequence == super::Consequence::Continue {
            self.check_not_stopping()?;
            let _ = self.check_policy_state(snapshot)?;
            self.check_mediation()?;
            self.check_fleet()?;
        }
        self.controller.apply_review(review, snapshot)
    }

    pub fn authorize(&mut self, id: u64, snapshot: &Snapshot) -> Result<Permit, Error> {
        self.check_not_stopping()?;
        let _ = self.check_policy_state(snapshot)?;
        self.check_mediation()?;
        self.check_fleet()?;
        self.controller.authorize(id, snapshot)
    }

    /// Normal exact-policy dispatch consumes the one-use permit and charges the
    /// original rights before this method returns any sendable envelope.
    pub fn dispatch(
        &mut self, permit: &Permit, action: &FrozenAction, snapshot: &Snapshot,
    ) -> Result<DispatchEnvelope, Error> {
        self.dispatch_bound(permit, action, snapshot, None)
    }

    /// Only the validated two-key oversight path supplies this additional bound.
    /// No public entrypoint accepts caller-asserted approval metadata.
    pub(crate) fn dispatch_with_approval(
        &mut self, permit: &Permit, action: &FrozenAction, snapshot: &Snapshot,
        approval: DispatchApproval,
    ) -> Result<DispatchEnvelope, Error> {
        self.dispatch_bound(permit, action, snapshot, Some(approval))
    }

    fn dispatch_bound(
        &mut self, permit: &Permit, action: &FrozenAction, snapshot: &Snapshot,
        approval: Option<DispatchApproval>,
    ) -> Result<DispatchEnvelope, Error> {
        self.check_not_stopping()?;
        let captured_policy_state = self.check_policy_state(snapshot)?;
        self.check_mediation()?;
        if !self.fenced { return Err(Error::Incomplete); }
        check_resource(action.spec(), self.scope, self.resource)?;
        if self.records.contains_key(&permit.attempt) { return Err(Error::Duplicate); }
        self.check_stream(action.spec())?;
        if self.records.len() >= self.max_deliveries { return Err(Error::Limit); }
        let bytes = self.action_bytes.checked_add(retained_action_bytes(action)?).ok_or(Error::Limit)?;
        if bytes > MAX_DELIVERY_ACTION_BYTES { return Err(Error::Limit); }
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if let Some(approval) = approval { approval.validate_at(now, action.spec().deadline)?; }
        let retained_until = ElapsedTick(now.0.checked_add(self.retention_ticks).ok_or(Error::Overflow)?);
        let envelope = DispatchEnvelope {
            binding: Rc::clone(&self.binding), epoch: self.epoch, attempt: permit.attempt,
            request: PublicationRequest { approval, ..PublicationRequest::from_action(action) },
            retained_until,
        };
        let record = DeliveryRecord { action: action.clone(), approval, mediation: self.mediation_cut().cloned(),
            policy_state: captured_policy_state, retained_until, resolution: None };
        let fleet_admission = self.prepare_fleet_dispatch(permit.attempt)?;
        self.controller.dispatch(permit, action, snapshot)?;
        self.records.insert(permit.attempt, record);
        self.action_bytes = bytes;
        if self.stream.is_some() { self.stream_pending = Some(permit.attempt); }
        self.publish_fleet_dispatch(permit.attempt, fleet_admission);
        Ok(envelope)
    }

    fn check_stream(&self, spec: &ActionSpec) -> Result<(), Error> {
        if let Some(stream) = &self.stream {
            if self.stream_pending.is_some() { return Err(Error::Incomplete); }
            if spec.target != Some(self.resource) { return Err(Error::Stale); }
            stream.advance(&spec.payload)?;
        }
        Ok(())
    }

    pub fn acknowledgment_lost(&mut self, attempt: u64) -> Result<(), Error> {
        self.records.get(&attempt).ok_or(Error::Missing)?;
        match self.inspect().ledger.stages.get(&attempt).copied().ok_or(Error::Missing)? {
            ActionState::Dispatching => self.controller.mark_unknown(attempt),
            ActionState::Unknown => Ok(()),
            _ => Err(Error::WrongState),
        }
    }

    pub fn status_query(&self, attempt: u64) -> Result<StatusQuery, Error> {
        let record = self.records.get(&attempt).ok_or(Error::Missing)?;
        Ok(StatusQuery(DispatchEnvelope {
            binding: Rc::clone(&self.binding), epoch: self.epoch, attempt,
            request: PublicationRequest { approval: record.approval, ..PublicationRequest::from_action(&record.action) },
            retained_until: record.retained_until,
        }))
    }

    /// Only a registered endpoint's terminal receipt reaches the trusted-outcome
    /// operation. Duplicating an identical receipt is idempotent, not a refund.
    /// A stream prefix advances only for executed evidence; missing status,
    /// cancellation, actor rewind and dispatcher restart cannot advance it.
    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> {
        if !Rc::ptr_eq(&self.binding, &receipt.binding) { return Err(Error::Binding); }
        let record = self.records.get(&receipt.attempt).ok_or(Error::Missing)?;
        if !receipt.request.matches_action(&record.action) || receipt.retained_until != record.retained_until
            || receipt.request.approval != record.approval
        {
            return Err(Error::Binding);
        }
        if let Some(previous) = &record.resolution {
            return if previous == &receipt { Ok(false) } else { Err(Error::Binding) };
        }
        let next_stream = if let Some(stream) = &self.stream {
            if self.stream_pending != Some(receipt.attempt) { return Err(Error::WrongState); }
            match receipt.outcome {
                EndpointOutcome::Executed { resulting_version } => {
                    if resulting_version != self.resource.expected_version.checked_add(1).ok_or(Error::Overflow)? {
                        return Err(Error::Binding);
                    }
                    Some((resulting_version, stream.advance(&record.action.spec().payload)?))
                }
                EndpointOutcome::NotExecuted { .. } => None,
            }
        } else { None };
        let outcome = match receipt.outcome {
            EndpointOutcome::Executed { .. } => TrustedOutcome::Executed,
            EndpointOutcome::NotExecuted { .. } => TrustedOutcome::NotExecuted,
        };
        self.controller.record_trusted_outcome(receipt.attempt, outcome)?;
        self.records.get_mut(&receipt.attempt).expect("retained delivery").resolution = Some(receipt);
        if self.stream.is_some() {
            if let Some((version, next)) = next_stream {
                self.resource.expected_version = version;
                self.stream = Some(next);
            }
            self.stream_pending = None;
        }
        Ok(true)
    }

    pub fn resolution(&self, attempt: u64) -> Result<Option<&EndpointReceipt>, Error> {
        Ok(self.records.get(&attempt).ok_or(Error::Missing)?.resolution.as_ref())
    }

    pub fn cancel(&mut self, attempt: u64) -> Result<(), Error> { self.controller.cancel(attempt) }
    pub fn deny(&mut self, attempt: u64) -> Result<(), Error> { self.controller.deny(attempt) }
    pub fn revoke_epoch(&mut self) -> Result<(), Error> { self.controller.revoke_epoch() }

    pub fn abandon_unknown(&mut self, attempt: u64) -> Result<(), Error> {
        self.records.get(&attempt).ok_or(Error::Missing)?;
        self.controller.mark_irrecoverable(attempt)
    }

    pub fn replace_policy(&mut self, sequence: u64, epoch: u64, next: Policy) -> Result<PolicyChange, Error> {
        self.controller.replace_policy(sequence, epoch, next)
    }

    pub fn capture_checkpoint(&mut self, id: u64, revision: u64) -> Result<CheckpointHandle, Error> {
        self.controller.capture_checkpoint(id, revision)
    }

    pub fn replace_actor_state(&mut self, revision: u64, actor: ActorState) -> Result<(), Error> {
        self.controller.replace_actor_state(revision, actor)
    }

    pub fn reset(&mut self, request: ResetRequest) -> Result<ResetReceipt, Error> {
        self.controller.reset(request)
    }
}

fn same_resource(target: ResolvedTarget, resource: ResolvedTarget) -> bool {
    target.adapter == resource.adapter && target.object == resource.object
        && target.contract_version == resource.contract_version && target.generation == resource.generation
}

fn check_resource(spec: &ActionSpec, scope: Scope, resource: ResolvedTarget) -> Result<(), Error> {
    let target = spec.target.ok_or(Error::Incomplete)?;
    if spec.scope != scope || !same_resource(target, resource) { return Err(Error::Binding); }
    let bytes = u64::try_from(spec.payload.len()).map_err(|_| Error::Limit)?;
    if bytes > spec.units { return Err(Error::Limit); }
    Ok(())
}

fn retained_action_bytes(action: &FrozenAction) -> Result<usize, Error> {
    action.spec().required_witnesses.iter().try_fold(action.spec().payload.len(), |sum, witness| {
        let bytes = match witness {
            ReadWitness::Exact { value: Some(value), .. } => value.len(),
            _ => 0,
        };
        sum.checked_add(bytes).ok_or(Error::Limit)
    })
}
