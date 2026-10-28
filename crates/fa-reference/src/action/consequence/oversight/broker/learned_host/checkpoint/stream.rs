//! Versioned same-audience stream containment. The world and rights never rewind.
use super::{HostedLearnedCheckpointHandle, HostedLearnedResetReceipt, HostedLearnedResetRequest,
    KvRestartBudget, OversightBroker, ReviewBinding, TargetCeiling};
use crate::action::ResolvedTarget;
use crate::action::consequence::delivery::stream::StreamView;
use crate::Error;
use std::rc::Rc;

/// Sealed stream-kind identity. It cannot be supplied to the legacy reset API.
#[derive(Clone, Debug)]
pub struct HostedLearnedStreamCheckpointHandle { inner: HostedLearnedCheckpointHandle }
impl HostedLearnedStreamCheckpointHandle { pub fn id(&self) -> u64 { self.inner.id() } }

/// The incident sequence is an independently selected SUPERVISOR observation.
/// Its strict ordering is checked; this type does not authenticate provenance.
#[derive(Clone, Debug)]
pub struct HostedLearnedStreamResetRequest {
    pub checkpoint: HostedLearnedStreamCheckpointHandle,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub binding: ReviewBinding,
    pub retained_targets: TargetCeiling,
    pub restart_budget: KvRestartBudget,
    pub first_incident_sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StreamCheckpointBasis {
    pub(crate) target: ResolvedTarget,
    pub(crate) view: StreamView,
    pub(crate) output_start: usize,
    pub(crate) continuation: bool,
    pub(crate) control_sequence: u64,
}
impl StreamCheckpointBasis {
    pub(super) fn retained_bytes(&self) -> Result<usize, Error> {
        // Five target words, five profile words, offset, capture sequence,
        // count, two flags, message lengths and exact confirmed UTF-8 bytes.
        106_usize.checked_add(self.view.message_count().checked_mul(8).ok_or(Error::Limit)?)
            .and_then(|n| n.checked_add(self.view.visible().len())).ok_or(Error::Limit)
    }
}

impl OversightBroker {
    /// Capture only a quiet active original stream within its unchanged exact
    /// confirmed publication basis. Installed predictors need a separate rearm
    /// contract and are unavailable here. The original restart grade, numerical
    /// snapshot, actor equality and aggregate storage ceilings remain mandatory.
    pub fn capture_hosted_learned_stream_checkpoint(&mut self, id: u64, actor_revision: u64)
        -> Result<HostedLearnedStreamCheckpointHandle, Error>
    {
        if self.action_consistency_required() { return Err(Error::Incomplete); }
        let (target, view, output_start, continuation) = self.learned_stream_checkpoint_basis()?;
        let stream = StreamCheckpointBasis { target, view, output_start, continuation,
            control_sequence: self.inspect().sequence };
        let inner = self.capture_hosted_learned_checkpoint_inner(id, actor_revision, Some(stream))?;
        Ok(HostedLearnedStreamCheckpointHandle { inner })
    }

    pub(crate) fn check_hosted_learned_stream_capture_basis(&self) -> Result<(), Error> {
        if self.action_consistency_required() { return Err(Error::Incomplete); }
        self.learned_stream_checkpoint_basis()?;
        Ok(())
    }

    pub(crate) fn hosted_learned_stream_output_start(&self) -> Result<usize, Error> {
        self.learned_host.as_ref().ok_or(Error::Incomplete)?.run.host_text_output_start()
    }

    pub(crate) fn hosted_learned_stream_checkpoint_basis(&self, id: u64)
        -> Result<&StreamCheckpointBasis, Error>
    {
        self.learned_host.as_ref().ok_or(Error::Incomplete)?.recovery.checkpoints
            .get(&id).ok_or(Error::Missing)?.stream.as_ref().ok_or(Error::Binding)
    }

    pub(crate) fn check_hosted_learned_stream_checkpoint(&self, checkpoint: &HostedLearnedStreamCheckpointHandle,
        first_incident_sequence: u64) -> Result<(), Error>
    {
        self.check_hosted_learned_stream_reset(&checkpoint.inner, first_incident_sequence)
    }

    pub(super) fn check_hosted_learned_stream_reset(&self, checkpoint: &HostedLearnedCheckpointHandle,
        first_incident_sequence: u64) -> Result<(), Error>
    {
        if self.action_consistency_required() { return Err(Error::Incomplete); }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&checkpoint.issuer, &host.recovery.issuer) { return Err(Error::Binding); }
        let saved = self.hosted_learned_stream_checkpoint_basis(checkpoint.id())?;
        if first_incident_sequence == 0 { return Err(Error::InvalidInput); }
        if saved.control_sequence >= first_incident_sequence
            || first_incident_sequence > self.inspect().sequence { return Err(Error::Stale); }
        let (target, view, output_start, continuation) = self.learned_stream_checkpoint_basis()?;
        if target != saved.target || view != saved.view || output_start != saved.output_start
            || continuation != saved.continuation { return Err(Error::Binding); }
        Ok(())
    }

    /// Restore the original numerical checkpoint only inside the SAME confirmed
    /// message boundary. No actor, audience, source or publication ledger is
    /// imported. The original reset performs the fresh audit, budget conservation,
    /// narrowing, epoch advance, incident escalation and stale-key withdrawal.
    /// A fresh accepted token and ordinary review remain required afterward.
    pub fn reset_hosted_learned_stream(&mut self, request: HostedLearnedStreamResetRequest)
        -> Result<HostedLearnedResetReceipt, Error>
    {
        if self.enforce_learned_host_stop()?.is_some() || self.enforce_consistency_stop()?.is_some() {
            return Err(Error::WrongState);
        }
        let original = HostedLearnedResetRequest { checkpoint: request.checkpoint.inner,
            expected_control_sequence: request.expected_control_sequence,
            expected_actor_revision: request.expected_actor_revision,
            expected_authority_epoch: request.expected_authority_epoch, binding: request.binding,
            retained_targets: request.retained_targets, restart_budget: request.restart_budget };
        let result = self.reset_hosted_learned_inner(original, Some(request.first_incident_sequence));
        if result.is_err() { self.enforce_learned_host_stop()?; }
        result
    }
}
