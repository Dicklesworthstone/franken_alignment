//! Own the delivery path and its exact empirical-input dependency.
//! Capture and evaluator authenticity remain explicit reference assumptions.

mod session;
mod reliability;
mod activation;
mod stream;
mod fleet;
mod mediation;
mod state_source;
mod stopping;
pub mod consistency;
pub mod human;
pub mod policy_governance;
pub mod identity;
pub mod decoder_gate;
pub mod decoder_host;
pub use session::{ObservedReview, ObservedSession, ReviewWindow};

use super::{CommitteeContract, CommitteeInput};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::{DeliveryBroker, DispatchEnvelope, EndpointReceipt, EndpointStatus, FenceAcknowledgment, FenceRequest, PublicationEndpoint, StatusQuery};
use crate::action::consequence::gate::ControlInspection;
use crate::action::consequence::gate::containment::{ActorState, CheckpointHandle, ResetReceipt, ResetRequest};
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::gate::containment::session::policy::controller::{ControllerConfig, PolicyChange, PolicyReceipt, Proposal};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Scope};
use crate::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_OBSERVED_ROUNDS: usize = 512;
pub const MAX_CAPTURED_INPUT_BYTES: usize = 8 * 1_048_576;

#[derive(Debug)]
struct InputSlot { action: FrozenAction, revision: u64, current: Option<Rc<CommitteeInput>>, approved: Option<u64> }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedReceipt {
    pub policy: PolicyReceipt,
    pub input_revision: u64,
    pub inputs: Rc<CommitteeInput>,
    pub window: ReviewWindow,
    pub started_at: ElapsedTick,
    pub completed_at: ElapsedTick,
}

#[derive(Debug)]
pub struct OversightBroker {
    delivery: DeliveryBroker,
    scope: Scope,
    contracts: CommitteeContract,
    issuer: Rc<()>,
    inputs: BTreeMap<u64, InputSlot>,
    started_rounds: BTreeSet<u64>,
    captured_bytes: usize,
    credibility: Option<reliability::EvaluationState>,
    human: Option<human::HumanGate>,
    activation: Option<activation::ActivationState>,
    consistency: Option<consistency::ConsistencyState>,
    policy_campaigns: Option<policy_governance::PolicyCampaignGate>,
    identity: Option<identity::IdentityGate>,
    decoder: Option<decoder_gate::DecoderGate>,
    decoder_host: Option<decoder_host::DecoderHost>,
}

impl OversightBroker {
    pub fn new(config: ControllerConfig, endpoint: &mut PublicationEndpoint, contracts: CommitteeContract) -> Result<Self, Error> {
        if !contracts.members().keys().eq(config.congress.members.keys()) { return Err(Error::Binding); }
        let scope = config.scope;
        Ok(Self { delivery: DeliveryBroker::new(config, endpoint)?, scope, contracts, issuer: Rc::new(()),
            inputs: BTreeMap::new(), started_rounds: BTreeSet::new(), captured_bytes: 0, credibility: None,
            human: None, activation: None, consistency: None, policy_campaigns: None, identity: None, decoder: None, decoder_host: None })
    }
    pub fn inspect(&self) -> ControlInspection { self.delivery.inspect() }
    pub fn contracts(&self) -> &CommitteeContract { &self.contracts }
    pub fn captured_input_bytes(&self) -> usize { self.captured_bytes }
    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> { self.delivery.observe_time(tick) }

    /// In a configured consistency lane, an observed valid category consumes its
    /// pre-action forecast even when subsequent admission fails. Inspect the
    /// consistency observation after Err; it cannot be rerolled as a new sample.
    pub fn propose(&mut self, id: u64, spec: ActionSpec, snapshot: &Snapshot) -> Result<Proposal, Error> {
        self.observe_predicted_action(id, &spec)?;
        let decoder = self.prepare_decoder()?;
        let proposal = self.delivery.propose(id, spec, snapshot)?;
        self.publish_decoder(id, decoder);
        self.inputs.insert(id, InputSlot { action: proposal.action.clone(), revision: 0, current: None, approved: None });
        Ok(proposal)
    }
    pub fn input_revision(&self, id: u64) -> Result<u64, Error> { Ok(self.inputs.get(&id).ok_or(Error::Missing)?.revision) }
    /// The latest RETAINED observation, not a fresh provider capture or approval.
    /// Trusted persistence consumers compare independently supplied current data
    /// against this exact original value instead of maintaining a shadow input slot.
    pub fn current_inputs(&self, id: u64) -> Result<Option<&CommitteeInput>, Error> {
        Ok(self.inputs.get(&id).ok_or(Error::Missing)?.current.as_deref())
    }
    pub fn record_inputs(&mut self, id: u64, expected: u64, inputs: CommitteeInput) -> Result<u64, Error> {
        let slot = self.inputs.get(&id).ok_or(Error::Missing)?;
        if slot.revision != expected { return Err(Error::Stale); }
        inputs.validate_for(&slot.action, &self.contracts)?;
        if slot.current.as_deref() == Some(&inputs) { return Ok(slot.revision); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        let bytes = self.captured_bytes.checked_add(inputs.logical_bytes()).ok_or(Error::Limit)?;
        if bytes > MAX_CAPTURED_INPUT_BYTES { return Err(Error::Limit); }
        let current = Rc::new(inputs); let slot = self.inputs.get_mut(&id).expect("retained input slot");
        slot.current = Some(current); slot.revision = revision; slot.approved = None; self.captured_bytes = bytes;
        Ok(revision)
    }
    pub fn inputs_unavailable(&mut self, id: u64, expected: u64) -> Result<u64, Error> {
        let slot = self.inputs.get_mut(&id).ok_or(Error::Missing)?;
        if slot.revision != expected { return Err(Error::Stale); }
        if slot.current.is_none() { return Ok(slot.revision); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        slot.current = None; slot.approved = None; slot.revision = revision; Ok(revision)
    }
    pub fn begin_review(&mut self, id: u64, round: u64, root: [u8; 32], window: ReviewWindow, snapshot: &Snapshot) -> Result<ObservedSession, Error> {
        if self.started_rounds.contains(&round) { return Err(Error::Duplicate); }
        if self.started_rounds.len() >= MAX_OBSERVED_ROUNDS { return Err(Error::Limit); }
        let slot = self.inputs.get(&id).ok_or(Error::Missing)?;
        let inputs = Rc::clone(slot.current.as_ref().ok_or(Error::Incomplete)?);
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if !(now < window.commit_by && window.commit_by < window.reveal_by && window.reveal_by <= slot.action.spec().deadline) { return Err(Error::InvalidInput); }
        self.check_decoder(id)?;
        let session = self.delivery.begin_review(id, round, root, snapshot)?;
        let observed = ObservedSession::new(session, Rc::clone(&self.issuer), id, slot.revision, inputs, window, now);
        self.started_rounds.insert(round);
        if let Some(state) = &mut self.credibility { state.started(round, self.delivery.controller().policy().generation()); }
        Ok(observed)
    }
    pub fn apply_review(&mut self, review: ObservedReview, current: Option<&CommitteeInput>, snapshot: &Snapshot) -> Result<ObservedReceipt, Error> {
        if !Rc::ptr_eq(&self.issuer, &review.issuer) { return Err(Error::Binding); }
        if self.inspect().ledger.elapsed.ok_or(Error::Incomplete)? < review.completed_at { return Err(Error::Stale); }
        let slot = self.inputs.get(&review.attempt).ok_or(Error::Missing)?;
        let permitting = review.policy.decision().consequence == Consequence::Continue;
        if permitting {
            self.check_identity()?;
            self.delivery.check_fleet()?;
            self.check_consistency(review.attempt)?;
            self.check_activation(review.attempt)?;
            self.check_decoder(review.attempt)?;
            let supplied = current.ok_or(Error::Incomplete)?;
            if slot.revision != review.revision || slot.current.as_deref() != Some(review.inputs.as_ref()) || supplied != review.inputs.as_ref() { return Err(Error::Stale); }
        }
        // Prepare evidence accounting before mutating the owning authority.
        let evaluated = self.prepare_evaluation(&review)?;
        let policy = self.delivery.apply_review(review.policy, snapshot)?;
        self.inputs.get_mut(&review.attempt).expect("retained input slot").approved = permitting.then_some(review.revision);
        if let Some((round, prepared)) = evaluated { self.credibility.as_mut().expect("enabled ledger").applied(round, prepared); }
        Ok(ObservedReceipt { policy, input_revision: review.revision, inputs: review.inputs, window: review.window,
            started_at: review.started_at, completed_at: review.completed_at })
    }
    pub fn authorize(&mut self, id: u64, current: Option<&CommitteeInput>, snapshot: &Snapshot) -> Result<Permit, Error> {
        self.check_approval(id, current)?; self.delivery.authorize(id, snapshot)
    }
    pub fn dispatch(&mut self, permit: &Permit, action: &FrozenAction, current: Option<&CommitteeInput>, snapshot: &Snapshot) -> Result<DispatchEnvelope, Error> {
        // A configured two-key profile has no one-key fallback, even when the
        // reviewer is unavailable. Reconciliation does not use this entry point.
        if self.human.is_some() { return Err(Error::Incomplete); }
        self.check_approval(permit.attempt, current)?; self.delivery.dispatch(permit, action, snapshot)
    }
    fn check_approval(&self, id: u64, supplied: Option<&CommitteeInput>) -> Result<(), Error> {
        self.delivery.check_mediation()?;
        self.check_identity()?;
        self.delivery.check_fleet()?;
        self.check_consistency(id)?;
        self.check_activation(id)?;
        self.check_decoder(id)?;
        let supplied = supplied.ok_or(Error::Incomplete)?; let slot = self.inputs.get(&id).ok_or(Error::Missing)?;
        let current = slot.current.as_deref().ok_or(Error::Incomplete)?;
        if slot.approved != Some(slot.revision) { return Err(Error::Incomplete); }
        if supplied != current { return Err(Error::Stale); } Ok(())
    }
    // Existing obligations deliberately do not depend on helper/evaluator availability.
    pub fn fence_request(&self) -> FenceRequest { self.delivery.fence_request() }
    pub fn confirm_fence(&mut self, ack: FenceAcknowledgment) -> Result<(), Error> { self.delivery.confirm_fence(ack) }
    pub fn restart_dispatcher(&mut self) -> Result<FenceRequest, Error> { self.delivery.restart_dispatcher() }
    pub fn acknowledgment_lost(&mut self, id: u64) -> Result<(), Error> { self.delivery.acknowledgment_lost(id) }
    pub fn status_query(&self, id: u64) -> Result<StatusQuery, Error> { self.delivery.status_query(id) }
    pub fn pending_reconciliation(&self) -> Result<Vec<StatusQuery>, Error> { self.delivery.pending_reconciliation() }
    /// Reconcile retained obligations without rerunning helpers or minting keys.
    /// Inspect every per-attempt result; successful earlier items are not rolled back.
    pub fn reconcile_pending(&mut self, endpoint: &mut PublicationEndpoint) -> Result<BTreeMap<u64, Result<EndpointStatus, Error>>, Error> {
        self.delivery.reconcile_pending(endpoint)
    }
    pub fn reconcile_status(&mut self, query: &StatusQuery, status: EndpointStatus) -> Result<EndpointStatus, Error> { self.delivery.reconcile_status(query, status) }
    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> { self.delivery.accept_receipt(receipt) }
    pub fn resolution(&self, id: u64) -> Result<Option<&EndpointReceipt>, Error> { self.delivery.resolution(id) }
    pub fn cancel(&mut self, id: u64) -> Result<(), Error> { self.delivery.cancel(id) }
    pub fn deny(&mut self, id: u64) -> Result<(), Error> { self.delivery.deny(id) }
    pub fn revoke_epoch(&mut self) -> Result<(), Error> { self.delivery.revoke_epoch() }
    pub fn abandon_unknown(&mut self, id: u64) -> Result<(), Error> { self.delivery.abandon_unknown(id) }
    pub fn actor_revision(&self) -> u64 { self.delivery.controller().actor_revision() }
    pub fn incident_count(&self) -> u64 { self.delivery.controller().incident_count() }
    pub fn capture_checkpoint(&mut self, id: u64, revision: u64) -> Result<CheckpointHandle, Error> {
        if self.decoder_host.is_some() { return Err(Error::WrongState); }
        self.delivery.capture_checkpoint(id, revision)
    }
    pub fn replace_actor_state(&mut self, revision: u64, actor: ActorState) -> Result<(), Error> {
        if self.decoder_host.is_some() { return Err(Error::WrongState); }
        self.delivery.replace_actor_state(revision, actor)
    }
    pub fn reset(&mut self, request: ResetRequest) -> Result<ResetReceipt, Error> {
        if self.decoder_host.is_some() { return Err(Error::WrongState); }
        let result = self.delivery.reset(request)?; for slot in self.inputs.values_mut() { slot.approved = None; } Ok(result)
    }
    pub fn replace_policy(&mut self, sequence: u64, epoch: u64, next: Policy) -> Result<PolicyChange, Error> {
        if self.policy_campaigns.is_some() { return Err(Error::Incomplete); }
        let result = self.delivery.replace_policy(sequence, epoch, next)?; for slot in self.inputs.values_mut() { slot.approved = None; } Ok(result)
    }
}
