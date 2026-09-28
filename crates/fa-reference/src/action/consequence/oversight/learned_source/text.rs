//! Native byte-BPE text over the ORIGINAL learned generation and live evidence.
//! Completed text is observation, not authorization. No supplied output is used.
use super::{LearnedAvailability, LearnedEvidence, LearnedEvidenceLimits, LearnedSourceConfig,
    ObservedLearnedGeneration};
use crate::action::MAX_PAYLOAD_BYTES;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, TokenizationBudget, TokenizationWork,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderModel, DecoderProfile};
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::LearnedDecoderPolicy;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingStart,
    monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop,
        GenerationTelemetryBudget, GenerationTelemetryWork, GenerationWork}};
use crate::Error;
use std::collections::BTreeSet;
use std::rc::Rc;

/// A finite horizon is not an observed end marker. The supervisor explicitly
/// selects whether token-limit completion may be presented as a whole message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedTextCompletion { StopRequired, StopOrTokenLimit }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedTextOutputPolicy {
    pub max_bytes: usize,
    pub completion: LearnedTextCompletion,
}

/// All input, sampling, monitor and cost choices freeze before the first token.
/// Prompt text is encoded verbatim; no template, normalization or tokenizer-family
/// guessing is inserted. The supplied tokenizer must belong to the model profile.
#[derive(Clone, Debug)]
pub struct LearnedTextConfig {
    pub stream: u64,
    pub evaluation_origin: u64,
    pub monitor_generation: u64,
    pub prompt: String,
    pub tokenization: TokenizationBudget,
    pub max_new_tokens: usize,
    pub stop_tokens: BTreeSet<u32>,
    pub sampling: SamplingStart,
    pub policy: LearnedDecoderPolicy,
    pub budget: GenerationBudget,
    pub telemetry: GenerationTelemetryBudget,
    pub output: LearnedTextOutputPolicy,
}

#[derive(Clone)]
pub(super) struct TextBinding {
    tokenizer: ByteBpe,
    prompt: String,
    tokenization: TokenizationWork,
    output: LearnedTextOutputPolicy,
}

impl DecoderModel {
    /// Uses ByteBpe::encode and the ORIGINAL observed-learned constructor. No
    /// unchecked token prompt, advanced generation or replacement output enters.
    pub fn observed_learned_text_generation(&self, tokenizer: ByteBpe, config: LearnedTextConfig)
        -> Result<ObservedLearnedGeneration, Error>
    {
        check_tokenizer(&tokenizer, self.profile())?;
        if config.output.max_bytes == 0 { return Err(Error::InvalidInput); }
        if config.output.max_bytes > MAX_PAYLOAD_BYTES { return Err(Error::Limit); }
        if config.output.completion == LearnedTextCompletion::StopRequired && config.stop_tokens.is_empty() {
            return Err(Error::InvalidInput);
        }
        // Only declared control IDs may be suppressed, and only as the actual
        // last accepted stop. Ordinary byte tokens never silently disappear.
        for token in &config.stop_tokens {
            if !tokenizer.is_control(*token)? { return Err(Error::Binding); }
        }
        let encoded = tokenizer.encode(config.prompt.as_bytes(), config.tokenization)
            .map_err(|failure| failure.error)?;
        let mut prompt = Vec::new();
        prompt.try_reserve_exact(encoded.tokens().len()).map_err(|_| Error::Limit)?;
        prompt.extend_from_slice(encoded.tokens());
        let spec = GenerationSpec::new(prompt, config.max_new_tokens, config.stop_tokens, config.sampling)?;
        let binding = TextBinding { tokenizer, prompt: config.prompt,
            tokenization: encoded.work(), output: config.output };
        let mut source = self.observed_learned_generation(LearnedSourceConfig {
            stream: config.stream, evaluation_origin: config.evaluation_origin,
            monitor_generation: config.monitor_generation, spec, policy: config.policy,
            budget: config.budget, telemetry: config.telemetry,
        })?;
        source.text = Some(binding);
        Ok(source)
    }
}
fn check_tokenizer(tokenizer: &ByteBpe, profile: &DecoderProfile) -> Result<(), Error> {
    if !tokenizer.binds(profile) { return Err(Error::Binding); }
    Ok(())
}

impl ObservedLearnedGeneration {
    pub fn text_bound(&self) -> bool { self.text.is_some() }
    pub fn text_prompt(&self) -> Option<&str> { self.text.as_ref().map(|text| text.prompt.as_str()) }
    pub fn text_tokenizer(&self) -> Option<&ByteBpe> { self.text.as_ref().map(|text| &text.tokenizer) }
    pub fn text_tokenization_work(&self) -> Option<TokenizationWork> {
        self.text.as_ref().map(|text| text.tokenization)
    }
    pub fn text_output_policy(&self) -> Option<LearnedTextOutputPolicy> {
        self.text.as_ref().map(|text| text.output)
    }

    /// Capture ONLY a finished, still-live, wholly accepted continuation. A held
    /// or failed suffix is not a successful shorter message. UTF-8 validation is
    /// exact: incomplete byte sequences never become replacement characters.
    pub fn text_message(&self, limits: LearnedEvidenceLimits) -> Result<LearnedTextMessage, Error> {
        let (bytes, stop, output_tokens) = self.decode_text_output()?;
        let evidence = self.observation().capture(limits)?;
        let text = self.text.as_ref().ok_or(Error::Incomplete)?;
        Ok(LearnedTextMessage { bytes: bytes.into(), evidence, tokenizer: text.tokenizer.clone(),
            tokenization: text.tokenization, output_policy: text.output, stop, output_tokens,
            work: self.work(), telemetry: self.telemetry_work() })
    }

    // Shared by text capture and the owning effect gate. This does not run any
    // inference or change liveness. Each call pays bounded decoding/copy costs.
    pub(crate) fn decode_text_output(&self) -> Result<(Vec<u8>, GenerationStop, usize), Error> {
        let text = self.text.as_ref().ok_or(Error::Incomplete)?;
        if self.observation().availability() != LearnedAvailability::Ready { return Err(Error::Incomplete); }
        let GenerationStatus::Finished(stop) = self.status() else { return Err(Error::Incomplete); };
        let continuation = self.run.generated_tokens();
        let tokens = match stop {
            GenerationStop::StopToken(token) => {
                if !self.run.spec().stop_tokens().contains(&token) || !text.tokenizer.is_control(token)?
                    || continuation.last() != Some(&token) { return Err(Error::Binding); }
                &continuation[..continuation.len() - 1]
            }
            GenerationStop::TokenLimit => {
                if text.output.completion == LearnedTextCompletion::StopRequired { return Err(Error::Incomplete); }
                continuation
            }
        };
        let bytes = text.tokenizer.decode(tokens, text.output.max_bytes)?;
        if bytes.is_empty() { return Err(Error::Incomplete); }
        std::str::from_utf8(&bytes).map_err(|_| Error::InvalidInput)?;
        Ok((bytes, stop, tokens.len()))
    }
}

/// Immutable historical output and its original learned evidence. It cannot be
/// converted to permission, edited, or rebound to another current source.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::oversight::learned_source::text::LearnedTextMessage};
/// fn grant(message: LearnedTextMessage) -> Permit { message }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_source::text::LearnedTextMessage;
/// fn replace(message: &mut LearnedTextMessage) { message.bytes_mut(); }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedTextMessage {
    bytes: Rc<[u8]>,
    evidence: LearnedEvidence,
    tokenizer: ByteBpe,
    tokenization: TokenizationWork,
    output_policy: LearnedTextOutputPolicy,
    stop: GenerationStop,
    output_tokens: usize,
    work: GenerationWork,
    telemetry: GenerationTelemetryWork,
}
impl LearnedTextMessage {
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    pub fn evidence(&self) -> &LearnedEvidence { &self.evidence }
    pub fn tokenizer(&self) -> &ByteBpe { &self.tokenizer }
    pub fn tokenization_work(&self) -> TokenizationWork { self.tokenization }
    pub fn output_policy(&self) -> LearnedTextOutputPolicy { self.output_policy }
    pub fn stop(&self) -> GenerationStop { self.stop }
    pub fn output_tokens(&self) -> usize { self.output_tokens }
    pub fn work(&self) -> GenerationWork { self.work }
    pub fn telemetry_work(&self) -> GenerationTelemetryWork { self.telemetry }
}
