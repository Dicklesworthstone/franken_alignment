//! Frozen original inputs for durable learned-generation replay.
//! The original generation archive codec provides exact recipe binding, not
//! a deserializer that trusts model weights or monitor verdicts from disk.
mod text;
mod sidecar;
mod automatic_stop;
mod identity;
mod policy_source;

use super::super::super::super::codec::shared::Writer;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::replay::{
    CheckpointLimits, archive::ArchiveLimits,
};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_BYTES;
use crate::action::consequence::oversight::{OversightBroker, learned_source::{
    LearnedSourceConfig, MAX_LEARNED_EVIDENCE_BYTES,
}, decoder_monitoring::{
    LearnedDecoderBindingLimits, MAX_BOUND_DECODER_TOKENS, MAX_BOUND_DECODER_SCORE_WORDS,
}};
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;
use crate::Error;
use std::fmt;
use std::rc::Rc;

pub(super) const MAX_CONFIG_BYTES: usize = MAX_JOURNAL_BYTES;
pub(super) const DOMAIN: &[u8; 8] = b"FALBOOT\x01";

/// Exact original model, fitted codec, probes, prompt, sampler and ceilings.
/// The existing generation-archive encoder binds every recipe parameter. Its
/// empty state is never restored; a NEW original observed generator starts at
/// zero and re-executes each recorded operation. Numeric labels alone do not bind.
///
/// There is no constructor from untrusted archive bytes. Recovery must supply
/// this configuration independently, even for an empty or entirely failed run.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedConfig;
/// fn trust_disk(bytes: &[u8]) { let _ = FileLearnedConfig::decode(bytes); }
/// ```
#[derive(Clone)]
pub struct FileLearnedConfig {
    model: DecoderModel,
    source: LearnedSourceConfig,
    limits: LearnedDecoderBindingLimits,
    bytes: Rc<[u8]>,
    text: Option<text::TextRecipe>,
    sidecar_required: bool,
    automatic_stop: Option<HostedStopPolicy>,
    computed_identity_required: bool,
    policy_source: Option<super::super::super::source::FileSourcePolicy>,
}
impl fmt::Debug for FileLearnedConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedConfig").field("profile", self.model.profile())
            .field("encoded_bytes", &self.bytes.len()).finish_non_exhaustive()
    }
}
impl PartialEq for FileLearnedConfig {
    fn eq(&self, other: &Self) -> bool { self.bytes == other.bytes }
}
impl Eq for FileLearnedConfig {}
impl FileLearnedConfig {
    pub fn new(model: DecoderModel, source: LearnedSourceConfig, limits: LearnedDecoderBindingLimits)
        -> Result<Self, Error>
    {
        if source.monitor_generation == 0 || limits.evidence.token_ids == 0
            || limits.evidence.score_words == 0 || limits.encoded_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.evidence.token_ids > MAX_BOUND_DECODER_TOKENS
            || limits.evidence.score_words > MAX_BOUND_DECODER_SCORE_WORDS
            || limits.encoded_bytes > MAX_LEARNED_EVIDENCE_BYTES { return Err(Error::Limit); }
        let recipe = model.replayable_monitored_generation(source.stream, source.evaluation_origin,
            source.spec.clone(), source.policy.clone(), source.budget, source.telemetry)?;
        let checkpoint = recipe.checkpoint(CheckpointLimits::default())?;
        // Eight domain bytes, four u64 fields and the archive's blob framing.
        let archive = checkpoint.encode_archive(ArchiveLimits { bytes: MAX_CONFIG_BYTES - 48,
            ..ArchiveLimits::default() })?;
        let mut w = Writer::new(MAX_CONFIG_BYTES);
        w.raw(DOMAIN)?;
        w.u64(source.monitor_generation)?;
        w.count(limits.evidence.token_ids)?; w.count(limits.evidence.score_words)?;
        w.count(limits.encoded_bytes)?; w.blob(&archive)?;
        Ok(Self { model, source, limits, bytes: w.finish().into(), text: None, sidecar_required: false, automatic_stop: None, computed_identity_required: false, policy_source: None })
    }
    pub fn encoded_bytes(&self) -> usize { self.bytes.len() }
    pub(super) fn bytes(&self) -> &Rc<[u8]> { &self.bytes }
    pub(in super::super::super) fn install(&self, broker: &mut OversightBroker) -> Result<(), Error> {
        match &self.text {
            Some(text) => match text.stream {
                Some(stream) => {
                    if broker.stream_state().map(|(_, view)| view.profile()) != Some(stream) {
                        return Err(Error::Binding);
                    }
                    broker.own_learned_text_stream(self.model.clone(), text.tokenizer.clone(),
                        text.source.clone(), self.limits)
                }
                None => broker.own_learned_text_generation(self.model.clone(), text.tokenizer.clone(),
                    text.source.clone(), self.limits),
            },
            None => broker.own_learned_generation(self.model.clone(), self.source.clone(), self.limits),
        }?;
        if self.sidecar_required { broker.enable_learned_sidecar_requirement()?; }
        if let Some(policy) = self.automatic_stop { broker.enable_learned_host_stop(policy)?; }
        Ok(())
    }
}
