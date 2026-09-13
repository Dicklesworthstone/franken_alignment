//! Pair the original authority checkpoint with private numerical state. A
//! successful reset changes authority FIRST; a replay never restores old rights.

use super::{DecoderHost, OversightBroker};
use crate::action::consequence::activation::monitor::decoder::sampled::host::reset::{NumericalCheckpoint, sum_work};
use crate::action::consequence::activation::monitor::decoder::MonitoringWork;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderWork, MAX_DECODER_PRODUCTS};
use crate::action::consequence::gate::{ReviewBinding, TargetCeiling};
use crate::action::consequence::gate::containment::{
    ActorState, CheckpointHandle, ResetReceipt, ResetRequest, RestartGrade,
    MAX_CACHE_BYTES, MAX_CHECKPOINTS, MAX_RETAINED_STATE_BYTES, MAX_SAMPLER_BYTES,
};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

/// A reference to one paired checkpoint, not its tokens, cache, sampler or rights.
/// Neither an actor port nor a standalone monitored decoder can create this.
#[derive(Clone)]
pub struct HostedCheckpointHandle { issuer: Rc<()>, id: u64 }
impl HostedCheckpointHandle { pub fn id(&self) -> u64 { self.id } }
impl fmt::Debug for HostedCheckpointHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostedCheckpointHandle").field("id", &self.id).finish_non_exhaustive()
    }
}

/// Trusted supervision, using the SAME original reset decision and target
/// ceiling contract. No caller-supplied model, sampler, cache or monitor is read.
#[derive(Clone, Debug)]
pub struct HostedResetRequest {
    pub checkpoint: HostedCheckpointHandle,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub binding: ReviewBinding,
    pub retained_targets: TargetCeiling,
    pub replay_budget: DecoderBudget,
}

/// Logical sampler draws rewind for exact continuation; physical computation
/// and monitoring never rewind. The original reset receipt records its own
/// transition; actor_revision additionally includes the new-stream host sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedResetReceipt {
    pub control: ResetReceipt,
    pub actor_revision: u64,
    pub resumed_stream: Option<u64>,
    pub position: u64,
    pub sampled_draws: u64,
    pub replay_numerical: DecoderWork,
    pub monitoring: MonitoringWork,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostedRecoveryUsage {
    pub checkpoints: usize,
    /// Additional logical numerical storage; the original ledger's state-copy
    /// limits independently remain in force. Neither bound includes allocator RSS.
    pub checkpoint_bytes: usize,
    pub replay_attempts: usize,
    /// Whole-prefix numerical allowance charged before every admitted replay.
    /// A failure or a final original-controller refusal does not return it.
    pub admitted_products: u64,
}

#[derive(Debug)]
struct PairedCheckpoint { control: CheckpointHandle, numerical: NumericalCheckpoint }
#[derive(Debug)]
pub(super) struct RecoveryState {
    issuer: Rc<()>,
    checkpoints: BTreeMap<u64, PairedCheckpoint>,
    usage: HostedRecoveryUsage,
    retired: DecoderWork,
    reset_rounds: BTreeSet<u64>,
}
impl RecoveryState {
    pub(super) fn new() -> Self {
        Self { issuer: Rc::new(()), checkpoints: BTreeMap::new(),
            usage: HostedRecoveryUsage::default(), retired: DecoderWork::default(), reset_rounds: BTreeSet::new() }
    }
    pub(super) fn cumulative(&self, active: DecoderWork) -> Result<DecoderWork, Error> {
        sum_work(self.retired, active)
    }
}

impl OversightBroker {
    pub fn hosted_recovery_usage(&self) -> Result<HostedRecoveryUsage, Error> {
        Ok(self.decoder_host.as_ref().ok_or(Error::Incomplete)?.recovery.usage)
    }

    /// Capture only a currently quiet, nonempty numerical prefix. Every byte of
    /// the ledger's actual actor copy must agree before pairing its checkpoint.
    /// A failed capture inserts neither a numerical handle nor a ledger checkpoint.
    pub fn capture_hosted_checkpoint(&mut self, id: u64, expected_actor_revision: u64)
        -> Result<HostedCheckpointHandle, Error>
    {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        if id == 0 { return Err(Error::InvalidInput); }
        let actor = self.delivery.controller().actor();
        if actor.profile().grade == RestartGrade::AuditOnly { return Err(Error::Incomplete); }
        let host = self.decoder_host.as_ref().ok_or(Error::Incomplete)?;
        if host.recovery.checkpoints.contains_key(&id) { return Err(Error::Duplicate); }
        if host.recovery.checkpoints.len() >= MAX_CHECKPOINTS { return Err(Error::Limit); }
        let (numerical, captured) = host.run.checkpoint_host(MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)?;
        if actor.profile() != host.profile || actor.tokens() != captured.tokens.as_slice()
            || actor.next_position() != captured.position || actor.cache() != captured.cache.as_slice()
            || actor.sampler() != captured.sampler.as_slice() { return Err(Error::Binding); }
        let bytes = host.recovery.usage.checkpoint_bytes.checked_add(numerical.logical_bytes).ok_or(Error::Limit)?;
        if bytes > MAX_RETAINED_STATE_BYTES { return Err(Error::Limit); }
        let control = self.delivery.capture_checkpoint(id, expected_actor_revision)?;
        let host = self.decoder_host.as_mut().expect("validated hosted owner");
        host.recovery.checkpoints.insert(id, PairedCheckpoint { control, numerical });
        host.recovery.usage.checkpoints += 1;
        host.recovery.usage.checkpoint_bytes = bytes;
        Ok(HostedCheckpointHandle { issuer: Rc::clone(&host.recovery.issuer), id })
    }

    /// Stage real recomputation and every-token review before invoking the
    /// original containment reset. The saved sampler is installed only with an
    /// exactly matching complete cache/logit replay. Old epochs, permits, human
    /// keys, input approvals and observation owners are NEVER restored.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::consequence::oversight::decoder_host::HostedCheckpointHandle;
    /// use fa_reference::action::Permit;
    /// fn grant(checkpoint: HostedCheckpointHandle) -> Permit { checkpoint }
    /// ```
    pub fn reset_hosted_decoder(&mut self, request: HostedResetRequest) -> Result<HostedResetReceipt, Error> {
        if self.enforce_hosted_stop()?.is_some() { return Err(Error::WrongState); }
        let result = self.reset_hosted_inner(request);
        // Replay failure may have poisoned the original numerical owner. Apply
        // an installed stop policy before returning that failure. An intentional
        // incident-driven suspension is not relabelled as a numerical fault.
        if result.is_err() { self.enforce_hosted_stop()?; }
        result
    }

    fn reset_hosted_inner(&mut self, request: HostedResetRequest) -> Result<HostedResetReceipt, Error> {
        let inspection = self.inspect();
        if inspection.suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if request.expected_control_sequence != inspection.sequence
            || request.expected_authority_epoch != inspection.ledger.epoch
            || request.expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        request.expected_actor_revision.checked_add(2).ok_or(Error::Overflow)?;
        if request.binding.round == 0 || request.binding.reducer_generation == 0
            || request.binding.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        let host = self.decoder_host.as_mut().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&request.checkpoint.issuer, &host.recovery.issuer) { return Err(Error::Binding); }
        if self.started_rounds.contains(&request.binding.round) || host.recovery.reset_rounds.contains(&request.binding.round) {
            return Err(Error::Duplicate);
        }
        let saved = host.recovery.checkpoints.get(&request.checkpoint.id).ok_or(Error::Missing)?;
        let products = saved.numerical.products()?;
        if request.replay_budget.scalar_products > MAX_DECODER_PRODUCTS
            || products > request.replay_budget.scalar_products { return Err(Error::Limit); }
        let admitted = host.recovery.usage.admitted_products.checked_add(products).ok_or(Error::Limit)?;
        if host.recovery.usage.replay_attempts >= MAX_CHECKPOINTS || admitted > MAX_DECODER_PRODUCTS {
            return Err(Error::Limit);
        }
        let stream = host.run.observation().stream().checked_add(1).ok_or(Error::Overflow)?;
        let old_work = host.recovery.cumulative(host.run.decoder_work())?;
        let original = ResetRequest { checkpoint: saved.control.clone(),
            expected_control_sequence: request.expected_control_sequence,
            expected_actor_revision: request.expected_actor_revision,
            binding: request.binding, retained_targets: request.retained_targets };
        host.recovery.usage.admitted_products = admitted;
        host.recovery.usage.replay_attempts += 1;
        let attempted = host.run.replay_host(&saved.numerical, stream, request.replay_budget,
            MAX_CACHE_BYTES, MAX_SAMPLER_BYTES);
        // Account successful replay tokens even when its monitoring or the final
        // controller decision refuses. No abandoned run becomes free computation.
        host.recovery.retired = sum_work(host.recovery.retired, attempted.numerical)?;
        let replay_numerical = attempted.numerical;
        let prepared = attempted.result?;
        let source = prepared.run.observation();
        self.decoder.as_ref().ok_or(Error::Incomplete)?.validate_successor(&source)?;
        let state = prepared.state;
        let profile = self.decoder_host.as_ref().expect("retained hosted owner").profile;
        let actor = ActorState::new(profile, state.tokens, state.cache, state.sampler, state.position)?;
        // No callbacks occur across the original authority transition and the
        // prevalidated host synchronization. A restrictive suspension discards
        // the staged owner; it cannot be reopened by installing its quiet source.
        let reset_round = original.binding.round;
        let control = self.delivery.reset(original)?;
        self.decoder_host.as_mut().expect("retained hosted owner").recovery.reset_rounds.insert(reset_round);
        for slot in self.inputs.values_mut() { slot.approved = None; }
        let resumed_stream = if control.restored {
            self.delivery.replace_actor_state(control.actor_revision, actor)
                .expect("prevalidated same-profile post-reset host synchronization");
            self.decoder.as_mut().expect("validated decoder gate").publish_successor(source);
            let host = self.decoder_host.as_mut().expect("retained hosted owner");
            host.run = prepared.run;
            // The new active run now owns the replay work; retire only the prior
            // complete run and earlier failed replay work, without double counting.
            host.recovery.retired = old_work;
            Some(stream)
        } else { None };
        let host: &DecoderHost = self.decoder_host.as_ref().expect("retained hosted owner");
        Ok(HostedResetReceipt { control, actor_revision: self.actor_revision(), resumed_stream,
            position: host.run.position(), sampled_draws: host.run.sampled_draws(),
            replay_numerical, monitoring: host.run.monitoring_work() })
    }
}
