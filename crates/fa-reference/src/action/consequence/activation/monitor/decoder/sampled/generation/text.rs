//! Byte-exact prompt ingestion and output decoding around the ORIGINAL monitored
//! generation owner. This wrapper owns a fixed tokenizer and cannot swap it
//! halfway through an existing KV history. Output is observation, not publication.

pub mod incremental;

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;

use super::{GenerationBudget, GenerationReport, GenerationRequest, MAX_GENERATION_TOKENS, MAX_STOP_TOKENS};
use super::tokenizer::{ByteBpe, TokenizationBudget, TokenizationWork, TokenizedInput, MAX_DECODE_BYTES};
use super::super::{MonitoredSampledDecoder, MonitoringStatus, MonitoringWork};
use crate::action::consequence::activation::monitor::decoder::observation::DecoderObservation;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderProfile, DecoderWork};
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

pub const MAX_PREFIX_CONTROLS: usize = 16;

/// A new prompt, not a serialized/re-tokenized version of the owner's old history.
/// The caller explicitly names any BOS-like controls; text never recognizes their
/// spelling. During sampling ALL non-text controls must be in stop_tokens, so
/// decoding cannot silently discard an unexplained special token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextGenerationRequest {
    pub prompt: Vec<u8>,
    pub prefix_controls: Vec<u32>,
    pub max_new_tokens: usize,
    pub stop_tokens: Vec<u32>,
    pub tokenization: TokenizationBudget,
    pub generation: GenerationBudget,
    /// Conservative worst-case output capacity must fit before inference starts.
    /// This is not permission to truncate text after the model has computed it.
    pub max_output_bytes: usize,
}

/// Admission failure; no inference from this request has begun. Tokenization work
/// is retained even when the subsequent numerical admission rejects the prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextGenerationFailure {
    pub error: Error,
    pub tokenization: TokenizationWork,
}
impl fmt::Display for TextGenerationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TextGenerationFailure {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOutputError {
    Contract(Error),
    InvalidUtf8(std::str::Utf8Error),
}
impl fmt::Display for TextOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TextOutputError {}

/// Retains the ORIGINAL numerical report, including holds, errors, admitted work,
/// stop consumption and position. Only its released IDs are decoded. Failure to
/// interpret output never erases that numerical history or yields an empty success.
/// Previously released quiet bytes may precede a later Held/Failed finish, exactly
/// as in GenerationReport. Quietness is not semantic safety or effect authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::TextGenerationReport;
/// fn authorize(report: TextGenerationReport) -> Permit { report }
/// ```
#[derive(Clone)]
pub struct TextGenerationReport {
    prompt: TokenizedInput,
    prefix_controls: Vec<u32>,
    generation: GenerationReport,
    output: Result<Vec<u8>, Error>,
}
impl TextGenerationReport {
    pub fn prompt(&self) -> &TokenizedInput { &self.prompt }
    pub fn prefix_controls(&self) -> &[u32] { &self.prefix_controls }
    pub fn generation(&self) -> &GenerationReport { &self.generation }
    pub fn bytes(&self) -> Result<&[u8], Error> { self.output.as_deref().map_err(|error| *error) }
    /// Strict conversion only. Split or invalid UTF-8 remains available as exact
    /// bytes with the original token report, never a replacement-character string.
    pub fn utf8(&self) -> Result<&str, TextOutputError> {
        std::str::from_utf8(self.bytes().map_err(TextOutputError::Contract)?)
            .map_err(TextOutputError::InvalidUtf8)
    }
}
impl fmt::Debug for TextGenerationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextGenerationReport").field("prompt", &self.prompt)
            .field("prefix_controls", &self.prefix_controls.len())
            .field("generation", &self.generation)
            .field("output_bytes", &self.output.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

/// Own a fresh original monitored decoder and one immutable exact tokenizer.
/// There is no mutable decoder/tokenizer accessor, seed replacement, reset,
/// unreviewed output, model checkpoint or effect handle. A non-ready numerical
/// owner stays non-ready; a failed monitor cannot reroll through text generation.
/// The underlying model remains an operator input, not an OS isolation claim.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::TextDecoder;
/// fn bypass(run: &mut TextDecoder) { run.decoder_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::TextDecoder;
/// fn duplicate(run: TextDecoder) { let _copy = run.clone(); }
/// ```
pub struct TextDecoder {
    decoder: MonitoredSampledDecoder,
    tokenizer: ByteBpe,
}
impl fmt::Debug for TextDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextDecoder").field("decoder", &self.decoder)
            .field("tokenizer", &self.tokenizer).finish_non_exhaustive()
    }
}
impl TextDecoder {
    pub fn new(decoder: MonitoredSampledDecoder, tokenizer: ByteBpe) -> Result<Self, Error> {
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding); }
        if decoder.position() != 0 || decoder.sampled_draws() != 0
            || decoder.status() != MonitoringStatus::Ready
        { return Err(Error::WrongState); }
        Ok(Self { decoder, tokenizer })
    }
    pub fn profile(&self) -> &DecoderProfile { self.decoder.profile() }
    pub fn tokenizer(&self) -> &ByteBpe { &self.tokenizer }
    pub fn position(&self) -> u64 { self.decoder.position() }
    pub fn status(&self) -> MonitoringStatus { self.decoder.status() }
    pub fn observation(&self) -> DecoderObservation { self.decoder.observation() }
    pub fn sampled_draws(&self) -> u64 { self.decoder.sampled_draws() }
    pub fn monitoring_work(&self) -> MonitoringWork { self.decoder.monitoring_work() }
    pub fn decoder_work(&self) -> DecoderWork { self.decoder.decoder_work() }

    /// Encode ALL prompt bytes and admit all output capacity before delegating to
    /// the SAME GenerationCursor/monitor/sampler. No direct numerical step or
    /// special-token bypass is implemented here. Predictable prompt/context/budget
    /// refusals do not run inference; after admission, the original report retains
    /// every quiet prefix, hold, failure and consumed draw without a rewind.
    pub fn generate(&mut self, expected_position: u64, request: TextGenerationRequest)
        -> Result<TextGenerationReport, TextGenerationFailure>
    {
        let mut work = TokenizationWork::default();
        self.generate_inner(expected_position, request, &mut work)
            .map_err(|error| TextGenerationFailure { error, tokenization: work })
    }

    fn generate_inner(&mut self, expected_position: u64, request: TextGenerationRequest,
        work: &mut TokenizationWork) -> Result<TextGenerationReport, Error>
    {
        let prepared = self.prepare_text(expected_position, request, work)?;
        let PreparedText { encoded, prefix_controls, numerical, mut output, capacity } = prepared;
        let generation = self.decoder.generate(expected_position, numerical)?;
        let decoded = append_output(&self.tokenizer, &mut output, generation.tokens(), capacity)
            .map(|()| output);
        Ok(TextGenerationReport { prompt: encoded, prefix_controls, generation, output: decoded })
    }

    // One text admission path for borrowed one-shot and owned incremental runs.
    // The original numerical owner still admits the ENTIRE request before work.
    fn prepare_text(&self, expected_position: u64, request: TextGenerationRequest,
        work: &mut TokenizationWork) -> Result<PreparedText, Error>
    {
        if self.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        if self.position() != expected_position { return Err(Error::Stale); }
        prepare_text_request(&self.tokenizer, request, work)
    }
}

// Shared by in-memory and durable text owners. This compiles immutable input
// data only: numerical readiness/context and authority admission remain owned by
// each ORIGINAL decoder. No public caller can attach an arbitrary result here.
pub(crate) fn prepare_text_request(tokenizer: &ByteBpe, request: TextGenerationRequest,
    work: &mut TokenizationWork) -> Result<PreparedText, Error>
{
    if request.max_new_tokens > MAX_GENERATION_TOKENS || request.stop_tokens.len() > MAX_STOP_TOKENS
        || request.prefix_controls.len() > MAX_PREFIX_CONTROLS || request.max_output_bytes > MAX_DECODE_BYTES
    { return Err(Error::Limit); }
    for control in &request.prefix_controls {
        if !tokenizer.is_control(*control)? { return Err(Error::Binding); }
    }
    let mut stops = BTreeSet::new();
    for token in &request.stop_tokens {
        tokenizer.is_control(*token)?; // also validate content stop IDs
        if !stops.insert(*token) { return Err(Error::Duplicate); }
    }
    if request.max_new_tokens != 0
        && tokenizer.control_tokens().iter().any(|token| !stops.contains(token))
    { return Err(Error::Incomplete); }
    let capacity = request.max_new_tokens.checked_mul(tokenizer.max_content_bytes()).ok_or(Error::Limit)?;
    if capacity > request.max_output_bytes { return Err(Error::Limit); }
    let encoded = match tokenizer.encode(&request.prompt, request.tokenization) {
        Ok(encoded) => { *work = encoded.work(); encoded }
        Err(failure) => { *work = failure.work; return Err(failure.error); }
    };
    let count = encoded.tokens().len().checked_add(request.prefix_controls.len()).ok_or(Error::Limit)?;
    if count > MAX_GENERATION_TOKENS { return Err(Error::Limit); }
    let mut prompt = Vec::new();
    prompt.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    prompt.extend_from_slice(&request.prefix_controls);
    prompt.extend_from_slice(encoded.tokens());
    // Complete output allocation BEFORE the numerical owner can advance.
    let mut output = Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| Error::Limit)?;
    Ok(PreparedText {
        encoded, prefix_controls: request.prefix_controls, output, capacity,
        numerical: GenerationRequest { prompt, max_new_tokens: request.max_new_tokens,
            stop_tokens: request.stop_tokens, budget: request.generation },
    })
}

pub(crate) struct PreparedText {
    encoded: TokenizedInput,
    prefix_controls: Vec<u32>,
    pub(crate) numerical: GenerationRequest,
    output: Vec<u8>,
    capacity: usize,
}

impl PreparedText {
    pub(crate) fn finish(mut self, generation: GenerationReport) -> TextGenerationReport {
        let output = append_output(self.encoded.tokenizer(), &mut self.output,
            generation.tokens(), self.capacity).map(|()| self.output);
        TextGenerationReport { prompt: self.encoded, prefix_controls: self.prefix_controls,
            generation, output }
    }
}

// Validate the entire new token slice before appending any bytes. Capacity was
// reserved BEFORE inference, so output retention cannot request more allocation.
// This only accepts the original cursor's released IDs, not sampled-but-held IDs.
fn append_output(tokenizer: &ByteBpe, output: &mut Vec<u8>, tokens: &[u32], capacity: usize)
    -> Result<(), Error>
{
    let mut bytes = output.len();
    for token in tokens {
        bytes = bytes.checked_add(tokenizer.content_bytes(*token)?.len()).ok_or(Error::Limit)?;
        if bytes > capacity || bytes > output.capacity() { return Err(Error::Limit); }
    }
    for token in tokens { output.extend_from_slice(tokenizer.content_bytes(*token)?); }
    Ok(())
}
