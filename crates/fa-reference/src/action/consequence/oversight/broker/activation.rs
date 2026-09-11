//! Mandatory activation capture in the existing owning oversight broker.
//! Host provenance remains a declared assumption; tensors are actual byte reads.

use super::OversightBroker;
use crate::action::ActionState;
use crate::action::consequence::activation::{FrameIdentity, SourceFrame};
use crate::action::consequence::activation::monitor::{MonitorOutcome, RefinementMonitor, RefinementReport};
use crate::action::consequence::activation::probe::SCORE_WORDS;
use crate::action::consequence::activation::tensor::{HostTensor, TensorCaptureReceipt, TensorContract, TokenSelection};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

const MAX_CAPTURES: usize = 512;
const MAX_RETAINED_SCORE_WORDS: usize = 1_048_576;

#[derive(Debug)]
struct CaptureBasis {
    input_revision: u64,
    actor_revision: u64,
    policy_epoch: u64,
    frame: FrameIdentity,
}

#[derive(Debug)]
struct CaptureRecord {
    basis: CaptureBasis,
    report: Rc<RefinementReport>,
    tensor: Option<TensorCaptureReceipt>,
}

#[derive(Debug)]
struct TensorIngress {
    contract: TensorContract,
    batch: usize,
    generations: BTreeMap<u64, u64>,
}

#[derive(Debug)]
pub(super) struct ActivationState {
    monitor: RefinementMonitor,
    stream: u64,
    max_captures: usize,
    captures: usize,
    last_sequence: u64,
    score_words: usize,
    records: BTreeMap<u64, CaptureRecord>,
    tensor: Option<TensorIngress>,
}

impl OversightBroker {
    /// Freeze before proposals. No disable, coefficient edit or budget widening.
    pub fn enable_activation_tripwire(
        &mut self, monitor: RefinementMonitor, stream: u64, max_captures: usize,
    ) -> Result<(), Error> {
        if self.activation.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if stream == 0 || max_captures == 0 { return Err(Error::InvalidInput); }
        if max_captures > MAX_CAPTURES { return Err(Error::Limit); }
        if monitor.profile().model_generation != self.delivery.controller().actor().profile().model_generation {
            return Err(Error::Binding);
        }
        self.activation = Some(ActivationState { monitor, stream, max_captures, captures: 0,
            last_sequence: 0, score_words: 0, records: BTreeMap::new(), tensor: None });
        Ok(())
    }

    /// Freeze a checked CPU-tensor ingress AND its selected batch. A caller
    /// cannot switch back to supplying an arbitrary flattened SourceFrame.
    pub fn enable_tensor_activation_tripwire(
        &mut self, monitor: RefinementMonitor, contract: TensorContract,
        stream: u64, batch: usize, max_captures: usize,
    ) -> Result<(), Error> {
        if contract.profile() != monitor.profile() || contract.dimensions() != monitor.dimensions()
            || contract.profile().tenant != self.scope.tenant
        { return Err(Error::Binding); }
        self.enable_activation_tripwire(monitor, stream, max_captures)?;
        self.activation.as_mut().expect("configured lane").tensor = Some(TensorIngress {
            contract, batch, generations: BTreeMap::new(),
        });
        Ok(())
    }

    pub fn activation_tripwire_required(&self) -> bool { self.activation.is_some() }

    pub fn record_activation(
        &mut self, attempt: u64, expected_input_revision: u64,
        expected_actor_revision: u64, source: &SourceFrame,
    ) -> Result<Rc<RefinementReport>, Error> {
        if self.activation.as_ref().ok_or(Error::Incomplete)?.tensor.is_some() {
            return Err(Error::Binding);
        }
        let basis = self.admit_activation(attempt, expected_input_revision,
            expected_actor_revision, source.identity(), source.dimensions())?;
        self.complete_activation(attempt, basis, source, None)
    }

    /// Once the context is admitted, every byte/shape/nonfinite/work failure
    /// leaves the new basis held. Inspect input_revision after Err. The old
    /// observation is removed before any host-buffer read or conversion.
    pub fn record_tensor_activation(
        &mut self, attempt: u64, expected_input_revision: u64,
        expected_actor_revision: u64, tensor: HostTensor<'_>, selected: TokenSelection,
    ) -> Result<Rc<RefinementReport>, Error> {
        let ingress = self.activation.as_ref().ok_or(Error::Incomplete)?
            .tensor.as_ref().ok_or(Error::Incomplete)?;
        if selected.batch != ingress.batch { return Err(Error::Binding); }
        if tensor.identity.object == 0 || tensor.identity.generation == 0 { return Err(Error::InvalidInput); }
        if ingress.generations.get(&tensor.identity.object).is_some_and(|floor| tensor.identity.generation < *floor) {
            return Err(Error::Stale);
        }
        let contract = ingress.contract.clone();
        let identity = contract.frame_identity(selected)?;
        let basis = self.admit_activation(attempt, expected_input_revision,
            expected_actor_revision, identity, contract.dimensions())?;
        self.activation.as_mut().expect("configured lane").tensor.as_mut().expect("tensor ingress")
            .generations.insert(tensor.identity.object, tensor.identity.generation);
        let captured = contract.capture(tensor, selected)?;
        self.complete_activation(attempt, basis, captured.source(), Some(captured.receipt().clone()))
    }

    fn admit_activation(
        &mut self, attempt: u64, expected_input_revision: u64,
        expected_actor_revision: u64, identity: FrameIdentity, dimensions: usize,
    ) -> Result<CaptureBasis, Error> {
        let state = self.activation.as_ref().ok_or(Error::Incomplete)?;
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_input_revision || self.actor_revision() != expected_actor_revision {
            return Err(Error::Stale);
        }
        if slot.current.is_none() { return Err(Error::Incomplete); }
        let inspection = self.inspect();
        if inspection.suspended
            || !matches!(inspection.ledger.stages.get(&attempt), Some(ActionState::Reviewing | ActionState::Authorized))
        { return Err(Error::WrongState); }
        if slot.action.spec().policy_epoch != inspection.ledger.epoch
            || inspection.ledger.elapsed.ok_or(Error::Incomplete)? >= slot.action.spec().deadline
        { return Err(Error::Stale); }
        let last_position = self.delivery.controller().actor().next_position().checked_sub(1).ok_or(Error::Incomplete)?;
        if identity.profile != state.monitor.profile() || identity.stream != state.stream
            || identity.profile.tenant != slot.action.spec().scope.tenant
            || identity.position != last_position || dimensions != state.monitor.dimensions()
        { return Err(Error::Binding); }
        if identity.sequence <= state.last_sequence { return Err(Error::Stale); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        let slot = self.inputs.get_mut(&attempt).expect("validated input slot");
        slot.revision = revision;
        slot.approved = None;
        let state = self.activation.as_mut().expect("configured activation lane");
        state.records.remove(&attempt);
        state.last_sequence = identity.sequence;
        if state.captures >= state.max_captures { return Err(Error::Limit); }
        state.captures += 1;
        Ok(CaptureBasis { input_revision: revision, actor_revision: expected_actor_revision,
            policy_epoch: inspection.ledger.epoch, frame: identity })
    }

    fn complete_activation(
        &mut self, attempt: u64, basis: CaptureBasis, source: &SourceFrame,
        tensor: Option<TensorCaptureReceipt>,
    ) -> Result<Rc<RefinementReport>, Error> {
        if source.identity() != basis.frame { return Err(Error::Binding); }
        let state = self.activation.as_mut().ok_or(Error::Incomplete)?;
        let report = state.monitor.analyze(source)?;
        let words = report.steps().iter().map(|step| step.observations.len() * 2 * SCORE_WORDS).sum::<usize>();
        let retained = state.score_words.checked_add(words).ok_or(Error::Limit)?;
        if retained > MAX_RETAINED_SCORE_WORDS { return Err(Error::Limit); }
        let report = Rc::new(report);
        state.records.insert(attempt, CaptureRecord { basis, report: Rc::clone(&report), tensor });
        state.score_words = retained;
        Ok(report)
    }

    /// Report failure even when a valid descriptor or buffer could not be built.
    /// Reservations and external effects are never refunded by observation loss.
    pub fn activation_unavailable(&mut self, attempt: u64, expected_revision: u64) -> Result<u64, Error> {
        let state = self.activation.as_mut().ok_or(Error::Incomplete)?;
        let slot = self.inputs.get_mut(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_revision { return Err(Error::Stale); }
        if !state.records.contains_key(&attempt) { return Ok(slot.revision); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        state.records.remove(&attempt);
        slot.revision = revision;
        slot.approved = None;
        Ok(revision)
    }

    /// Historical evidence; presence is not a freshness certificate.
    pub fn activation_report(&self, attempt: u64) -> Result<Option<&RefinementReport>, Error> {
        let state = self.activation.as_ref().ok_or(Error::Incomplete)?;
        self.inputs.get(&attempt).ok_or(Error::Missing)?;
        Ok(state.records.get(&attempt).map(|record| record.report.as_ref()))
    }

    pub fn tensor_capture_receipt(&self, attempt: u64) -> Result<Option<&TensorCaptureReceipt>, Error> {
        let state = self.activation.as_ref().ok_or(Error::Incomplete)?;
        self.inputs.get(&attempt).ok_or(Error::Missing)?;
        Ok(state.records.get(&attempt).and_then(|record| record.tensor.as_ref()))
    }

    pub(super) fn check_activation(&self, attempt: u64) -> Result<(), Error> {
        let Some(state) = &self.activation else { return Ok(()); };
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        let record = state.records.get(&attempt).ok_or(Error::Incomplete)?;
        if record.basis.input_revision != slot.revision || record.basis.actor_revision != self.actor_revision()
            || record.basis.policy_epoch != self.inspect().ledger.epoch
        { return Err(Error::Stale); }
        if record.report.outcome() != MonitorOutcome::NoAlarm { return Err(Error::Incomplete); }
        Ok(())
    }
}
