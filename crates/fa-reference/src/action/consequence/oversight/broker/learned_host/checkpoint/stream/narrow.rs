//! Real pre-output containment at the original paired stream checkpoint.
use super::{HostedLearnedStreamCheckpointHandle, OversightBroker};
use crate::action::ResolvedTarget;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::consequence::gate::{ReviewBinding, TargetCeiling};
use crate::action::consequence::gate::containment::{CheckpointNarrowReceipt, CheckpointNarrowRequest};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct HostedLearnedStreamNarrowRequest {
    pub(crate) checkpoint: HostedLearnedStreamCheckpointHandle,
    pub(crate) expected_control_sequence: u64,
    pub(crate) expected_actor_revision: u64,
    pub(crate) expected_authority_epoch: u64,
    pub(crate) binding: ReviewBinding,
    pub(crate) retained_targets: Vec<ResolvedTarget>,
}

impl OversightBroker {
    pub(crate) fn check_hosted_learned_stream_narrow(&self, request: &HostedLearnedStreamNarrowRequest)
        -> Result<(), Error>
    {
        if !self.owned_learned_forecast_restart_audit_required() || !self.learned_text_stream_required() {
            return Err(Error::Binding);
        }
        self.check_owned_learned_forecast_restart()?;
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if self.started_rounds.contains(&request.binding.round) { return Err(Error::Duplicate); }
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if self.delivery.controller().policy().generation() != state.config.model.policy_generation() {
            return Err(Error::Stale);
        }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&request.checkpoint.inner.issuer, &host.recovery.issuer) { return Err(Error::Binding); }
        if host.fault.is_some() || host.run.observation().availability() != LearnedAvailability::Ready {
            return Err(Error::WrongState);
        }
        let original = host.run.original_generation();
        if original.position() != original.spec().prompt().len() as u64
            || original.status() != GenerationStatus::Generating || original.work().sampling_attempts != 0 {
            return Err(Error::WrongState);
        }
        // Compare the ORIGINAL current cache/sampler and actor, never a public
        // imported checkpoint image. The lower gate also checks exact capture.
        if &host.run.capture_host_actor(host.profile)? != self.delivery.controller().actor() {
            return Err(Error::Binding);
        }
        let saved = self.hosted_learned_stream_checkpoint_basis(request.checkpoint.id())?;
        let (target, view, output_start, continuation) = self.learned_stream_checkpoint_basis()?;
        if target != saved.target || view != saved.view || output_start != saved.output_start
            || continuation != saved.continuation { return Err(Error::Binding); }
        // Multiple expected versions can be selected for later complete message
        // publication and Finish. They may never substitute another audience.
        if request.retained_targets.iter().any(|retained| {
            retained.adapter != target.adapter || retained.object != target.object
                || retained.contract_version != target.contract_version || retained.generation != target.generation
                || retained.expected_version < target.expected_version
        }) { return Err(Error::Binding); }
        self.delivery.check_checkpoint_narrow(&self.original_stream_narrow_request(request)?)
    }

    fn original_stream_narrow_request(&self, request: &HostedLearnedStreamNarrowRequest)
        -> Result<CheckpointNarrowRequest, Error>
    {
        let saved = self.learned_host.as_ref().ok_or(Error::Incomplete)?.recovery.checkpoints
            .get(&request.checkpoint.id()).ok_or(Error::Missing)?;
        Ok(CheckpointNarrowRequest { checkpoint: saved.control.clone(),
            expected_control_sequence: request.expected_control_sequence,
            expected_actor_revision: request.expected_actor_revision,
            expected_authority_epoch: request.expected_authority_epoch, binding: request.binding,
            retained_targets: TargetCeiling::new(&request.retained_targets)?,
        })
    }

    pub(crate) fn narrow_hosted_learned_stream(&mut self, request: HostedLearnedStreamNarrowRequest)
        -> Result<CheckpointNarrowReceipt, Error>
    {
        self.check_hosted_learned_stream_narrow(&request)?;
        let original = self.original_stream_narrow_request(&request)?;
        let receipt = self.delivery.narrow_checkpoint(original)?;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(receipt)
    }
}
