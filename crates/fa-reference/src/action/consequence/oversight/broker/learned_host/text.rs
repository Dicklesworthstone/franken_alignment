//! Publication payloads derived from THIS original broker-owned text generation.
//! This opt-in mode narrows generic effect admission; it grants no authority.
pub mod stream;
use super::{OversightBroker, LearnedDecoderBindingLimits};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::action::consequence::oversight::learned_source::{LearnedEvidenceLimits,
    text::{LearnedTextConfig, LearnedTextMessage}};
use crate::{Error, ReadWitness, Snapshot};

/// Destination and policy prerequisites only. Payload, tenant/scope and version
/// come from the original owned generation and broker, not caller output text.
///
/// ```compile_fail,E0609
/// use fa_reference::action::consequence::oversight::learned_host::text::LearnedTextTarget;
/// fn override_output(target: &mut LearnedTextTarget) { target.payload = b"override".to_vec(); }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedTextTarget {
    pub target: ResolvedTarget,
    pub required_witnesses: Vec<ReadWitness>,
    pub policy_epoch: u64,
    pub deadline: ElapsedTick,
    pub units: u64,
}

impl OversightBroker {
    /// Select at bootstrap, before any inference or proposal. The whole native
    /// tokenizer is pinned in the original source; no late binding, replacement
    /// tokenizer, advanced-run adoption or disable switch exists. Sidecar/human
    /// policies remain separately configured before the first token as usual.
    pub fn own_learned_text_generation(&mut self, model: DecoderModel, tokenizer: ByteBpe,
        config: LearnedTextConfig, limits: LearnedDecoderBindingLimits) -> Result<(), Error>
    {
        self.check_learned_bootstrap()?;
        // This profile publishes one raw text message, not a framed cumulative
        // stream command. Do not install an incompatible, unusable text mode.
        if self.stream_state().is_some() { return Err(Error::Binding); }
        let source = model.observed_learned_text_generation(tokenizer, config)?;
        self.install_fresh_learned_host(source, limits)
    }

    pub fn learned_text_required(&self) -> bool {
        self.learned_host.as_ref().is_some_and(|host| host.run.text_bound())
    }

    /// Immutable output observation, not a proposal or a permit. Inspection does
    /// not evade suspension or hide a failed actor synchronization.
    pub fn hosted_learned_text_message(&self, limits: LearnedEvidenceLimits)
        -> Result<LearnedTextMessage, Error>
    {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.fault.is_some() { return Err(Error::Incomplete); }
        host.run.text_message(limits)
    }

    /// Propose the original completed text through the SAME broker admission.
    /// Generic propose is also constrained in this mode, so this is convenience,
    /// not a privileged bypass. Normal congress/keys/witnesses still decide.
    pub fn propose_learned_text(&mut self, id: u64, request: LearnedTextTarget, snapshot: &Snapshot)
        -> Result<Proposal, Error>
    {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.fault.is_some() { return Err(Error::Incomplete); }
        let (payload, _, _) = host.run.decode_text_output()?;
        self.propose(id, ActionSpec { version: VERSION, scope: self.scope, target: Some(request.target),
            payload, required_witnesses: request.required_witnesses, policy_epoch: request.policy_epoch,
            deadline: request.deadline, units: request.units }, snapshot)
    }

    // Called before the original proposal ledger/evidence-retention commit.
    // Generic effects outside this explicit text profile retain their semantics.
    pub(in super::super) fn check_learned_text_spec(&self, spec: &ActionSpec) -> Result<(), Error> {
        let Some(host) = &self.learned_host else { return Ok(()); };
        if !host.run.text_bound() { return Ok(()); }
        if host.fault.is_some() { return Err(Error::Incomplete); }
        if host.text_stream.is_some() { return self.check_learned_text_stream_spec(spec); }
        let (original, _, _) = host.run.decode_text_output()?;
        if original.as_slice() != spec.payload.as_slice() { return Err(Error::Binding); }
        Ok(())
    }

    // Reached through the original decoder gate at review/apply/authorize and
    // both dispatch routes. No second approval or output-history ledger exists.
    pub(in super::super) fn check_learned_text_action(&self, attempt: u64) -> Result<(), Error> {
        if !self.learned_text_required() { return Ok(()); }
        let action = &self.inputs.get(&attempt).ok_or(Error::Missing)?.action;
        if self.learned_text_stream_required() && self.stream_pending() == Some(attempt) {
            return self.check_dispatched_learned_text_stream_spec(attempt, action.spec());
        }
        self.check_learned_text_spec(action.spec())
    }
}
