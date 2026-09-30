//! Byte-exact prompt ingestion and output decoding around the ORIGINAL monitored
//! generation owner. This wrapper owns a fixed tokenizer and cannot swap it
//! halfway through an existing KV history. Output is observation, not publication.

pub mod incremental;

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;

use super::{GenerationBudget, GenerationReport, GenerationRequest, MAX_GENERATION_TOKENS, MAX_STOP_TOKENS};
use super::tokenizer::{ByteBpe, TokenizationBudget, TokenizationCursor, TokenizationProgress,
    TokenizationWork, TokenizedInput, MAX_DECODE_BYTES};
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

    /// Own the decoder while tokenizing the fixed prompt cooperatively. This
    /// consumes ownership even on refusal, like into_generation. The original
    /// numerical admission still runs only after the whole prompt is available.
    pub fn into_generation_admission(self, expected_position: u64, request: TextGenerationRequest)
        -> Result<TextGenerationAdmission, TextGenerationFailure>
    {
        let refuse = |error| TextGenerationFailure {
            error, tokenization: TokenizationWork::default(),
        };
        if self.status() != MonitoringStatus::Ready { return Err(refuse(Error::WrongState)); }
        if self.position() != expected_position { return Err(refuse(Error::Stale)); }
        let preparation = begin_text_request(&self.tokenizer, request)?;
        Ok(TextGenerationAdmission { decoder: self, expected_position, preparation })
    }

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
        self.run_prepared(expected_position, prepared)
    }

    fn run_prepared(&mut self, expected_position: u64, prepared: PreparedText)
        -> Result<TextGenerationReport, Error>
    {
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

/// An owned prompt-admission phase. The original numerical owner is inaccessible
/// while the immutable prompt is being tokenized. No inference or random draw
/// happens in advance; only a complete prompt may enter numerical admission.
/// Initial byte copying and named-control recognition remain synchronous and
/// bounded, as documented by ByteBpe::begin_encode.
///
/// This object is not a completed generation or an authorization. Cancellation
/// destroys the owner rather than returning an owner with erased accounting.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::TextGenerationAdmission;
/// fn bypass(pending: &mut TextGenerationAdmission) { pending.decoder_mut(); }
/// ```
#[must_use = "advance, finish, or cancel prompt admission"]
pub struct TextGenerationAdmission {
    decoder: TextDecoder,
    expected_position: u64,
    preparation: PromptPreparation,
}
impl fmt::Debug for TextGenerationAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextGenerationAdmission")
            .field("position", &self.position())
            .field("tokenization", &self.tokenization_work())
            .field("failure", &self.preparation.failure)
            .finish_non_exhaustive()
    }
}
impl TextGenerationAdmission {
    pub fn position(&self) -> u64 { self.decoder.position() }
    pub fn sampled_draws(&self) -> u64 { self.decoder.sampled_draws() }
    pub fn decoder_work(&self) -> DecoderWork { self.decoder.decoder_work() }
    pub fn monitoring_work(&self) -> MonitoringWork { self.decoder.monitoring_work() }
    pub fn tokenization_work(&self) -> TokenizationWork { self.preparation.cursor.work() }

    /// One original tokenizer step. Terminal failure is sticky and neither
    /// completion nor repeated polling starts inference as a side effect.
    pub fn advance(&mut self) -> Result<TokenizationProgress, TextGenerationFailure> {
        self.preparation.advance()
    }

    /// Run the original synchronous numerical generation ONLY after cooperative
    /// tokenization completed. Returns the same owner and original terminal
    /// report, preserving held/failed latches and all work. A pending finish
    /// refuses and destroys the owner; it does not finish tokenization for you.
    pub fn finish(self) -> Result<(TextDecoder, TextGenerationReport), TextGenerationFailure> {
        let (expected_position, mut decoder, prepared) = self.into_prepared()?;
        let work = prepared.encoded.work();
        let report = decoder.run_prepared(expected_position, prepared)
            .map_err(|error| TextGenerationFailure { error, tokenization: work })?;
        Ok((decoder, report))
    }

    // Only the original text paths may attach this private preparation to its
    // original owner. No profile-only rebinding or re-tokenization is permitted.
    fn into_prepared(self) -> Result<(u64, TextDecoder, PreparedText), TextGenerationFailure> {
        let Self { decoder, expected_position, preparation } = self;
        let prepared = preparation.finish()?;
        Ok((expected_position, decoder, prepared))
    }

    pub fn cancel(self) -> CancelledTextAdmission {
        let Self { decoder, preparation, .. } = self;
        let cancelled = CancelledTextAdmission {
            position: decoder.position(), sampled_draws: decoder.sampled_draws(),
            decoder_work: decoder.decoder_work(), monitoring_work: decoder.monitoring_work(),
            tokenization: preparation.cursor.work(), failure: preparation.failure,
        };
        drop(preparation);
        drop(decoder);
        cancelled
    }
}

/// Historical counters after destroying a prompt-admission owner. No prompt
/// prefix, decoder, RNG, completed report or live observation is returned.
#[derive(Debug)]
pub struct CancelledTextAdmission {
    position: u64,
    sampled_draws: u64,
    decoder_work: DecoderWork,
    monitoring_work: MonitoringWork,
    tokenization: TokenizationWork,
    failure: Option<Error>,
}
impl CancelledTextAdmission {
    pub fn position(&self) -> u64 { self.position }
    pub fn sampled_draws(&self) -> u64 { self.sampled_draws }
    pub fn decoder_work(&self) -> DecoderWork { self.decoder_work }
    pub fn monitoring_work(&self) -> MonitoringWork { self.monitoring_work }
    pub fn tokenization_work(&self) -> TokenizationWork { self.tokenization }
    pub fn failure(&self) -> Option<Error> { self.failure }
}

struct PromptPreparation {
    cursor: TokenizationCursor,
    prefix_controls: Vec<u32>,
    max_new_tokens: usize,
    stop_tokens: Vec<u32>,
    generation: GenerationBudget,
    capacity: usize,
    failure: Option<Error>,
}
impl PromptPreparation {
    fn advance(&mut self) -> Result<TokenizationProgress, TextGenerationFailure> {
        self.cursor.advance().map_err(|failure| {
            self.failure = Some(failure.error);
            TextGenerationFailure { error: failure.error, tokenization: failure.work }
        })
    }

    fn finish(self) -> Result<PreparedText, TextGenerationFailure> {
        let Self { cursor, prefix_controls, max_new_tokens, stop_tokens,
            generation, capacity, .. } = self;
        let encoded = cursor.finish().map_err(|failure| TextGenerationFailure {
            error: failure.error, tokenization: failure.work,
        })?;
        let work = encoded.work();
        (|| {
            let count = encoded.tokens().len().checked_add(prefix_controls.len()).ok_or(Error::Limit)?;
            if count > MAX_GENERATION_TOKENS { return Err(Error::Limit); }
            let mut prompt = Vec::new();
            prompt.try_reserve_exact(count).map_err(|_| Error::Limit)?;
            prompt.extend_from_slice(&prefix_controls);
            prompt.extend_from_slice(encoded.tokens());
            // Full output allocation still precedes original numerical admission.
            let mut output = Vec::new();
            output.try_reserve_exact(capacity).map_err(|_| Error::Limit)?;
            Ok(PreparedText { encoded, prefix_controls, output, capacity,
                numerical: GenerationRequest { prompt, max_new_tokens,
                    stop_tokens, budget: generation } })
        })().map_err(|error| TextGenerationFailure { error, tokenization: work })
    }
}

// Shared by in-memory and durable text owners. All paths use the same original
// request guards, tokenizer cursor and complete-prompt construction.
pub(crate) fn prepare_text_request(tokenizer: &ByteBpe, request: TextGenerationRequest,
    work: &mut TokenizationWork) -> Result<PreparedText, Error>
{
    let result = (|| {
        let mut preparation = begin_text_request(tokenizer, request)?;
        while preparation.advance()? != TokenizationProgress::Complete {}
        preparation.finish()
    })();
    match result {
        Ok(prepared) => { *work = prepared.encoded.work(); Ok(prepared) }
        Err(failure) => { *work = failure.tokenization; Err(failure.error) }
    }
}

fn begin_text_request(tokenizer: &ByteBpe, request: TextGenerationRequest)
    -> Result<PromptPreparation, TextGenerationFailure>
{
    let capacity = (|| {
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
        Ok(capacity)
    })().map_err(|error| TextGenerationFailure { error, tokenization: TokenizationWork::default() })?;
    let cursor = tokenizer.begin_encode(&request.prompt, request.tokenization)
        .map_err(|failure| TextGenerationFailure { error: failure.error, tokenization: failure.work })?;
    Ok(PromptPreparation { cursor, prefix_controls: request.prefix_controls,
        max_new_tokens: request.max_new_tokens, stop_tokens: request.stop_tokens,
        generation: request.generation, capacity, failure: None })
}

pub(crate) struct PreparedText {
    encoded: TokenizedInput,
    prefix_controls: Vec<u32>,
    pub(crate) numerical: GenerationRequest,
    output: Vec<u8>,
    capacity: usize,
}

impl PreparedText {
    /// Decode only a native released-ID projection into the capacity already
    /// reserved by original text admission. It cannot create a report or owner.
    #[cfg(unix)]
    pub(crate) fn decode_output(mut self, tokens: &[u32]) -> Result<Vec<u8>, Error> {
        append_output(self.encoded.tokenizer(), &mut self.output, tokens, self.capacity)?;
        Ok(self.output)
    }

    #[cfg(unix)]
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

#[cfg(test)]
mod admission_tests {
    use super::*;
    use super::super::GenerationFinish;
    use super::tests::{request, run};

    fn drain(mut admission: TextGenerationAdmission) -> TextGenerationAdmission {
        let position = admission.position();
        let draws = admission.sampled_draws();
        let decoder = admission.decoder_work();
        let monitoring = admission.monitoring_work();
        loop {
            let progress = admission.advance().unwrap();
            assert_eq!(admission.position(), position);
            assert_eq!(admission.sampled_draws(), draws);
            assert_eq!(admission.decoder_work(), decoder);
            assert_eq!(admission.monitoring_work(), monitoring);
            if progress == TokenizationProgress::Complete { return admission; }
        }
    }

    #[test]
    fn cooperative_prompt_admission_matches_original_numerical_generation() {
        let mut synchronous = run(65, None);
        let expected = synchronous.generate(0, request(b"ab", 3)).unwrap();
        let pending = run(65, None).into_generation_admission(0, request(b"ab", 3)).unwrap();
        assert_eq!(pending.tokenization_work().pair_lookups, 0);
        let (owner, actual) = drain(pending).finish().unwrap();
        assert_eq!(actual.prompt().tokens(), &[258]);
        assert_eq!(actual.prompt().spans(), &[0..2]);
        assert_eq!(actual.prompt().work(), expected.prompt().work());
        assert_eq!(actual.bytes().unwrap(), b"AAA");
        assert_eq!(actual.generation().tokens(), expected.generation().tokens());
        assert_eq!(actual.generation().work(), expected.generation().work());
        assert_eq!(owner.decoder_work(), synchronous.decoder_work());
        assert_eq!(owner.monitoring_work(), synchronous.monitoring_work());
        assert_eq!(owner.sampled_draws(), synchronous.sampled_draws());
    }

    #[test]
    fn cancellation_keeps_existing_work_and_destroys_the_original_source() {
        let mut owner = run(65, None);
        owner.generate(0, request(b"ab", 0)).unwrap();
        let observation = owner.observation();
        assert!(observation.capture().is_ok());
        let position = owner.position();
        let decoder = owner.decoder_work();
        let monitoring = owner.monitoring_work();
        let mut pending = owner.into_generation_admission(position, request(b"abab", 3)).unwrap();
        pending.advance().unwrap();
        let work = pending.tokenization_work();
        let cancelled = pending.cancel();
        assert_eq!(cancelled.position(), position);
        assert_eq!(cancelled.decoder_work(), decoder);
        assert_eq!(cancelled.monitoring_work(), monitoring);
        assert_eq!(cancelled.sampled_draws(), 0);
        assert_eq!(cancelled.tokenization_work(), work);
        assert_eq!(cancelled.failure(), None);
        assert!(observation.capture().is_err());
    }

    #[test]
    fn pending_finish_refuses_without_running_prefill_or_sampling() {
        let owner = run(65, None);
        let observation = owner.observation();
        let pending = owner.into_generation_admission(0, request(b"ab", 2)).unwrap();
        assert_eq!(pending.position(), 0);
        assert_eq!(pending.sampled_draws(), 0);
        let failure = pending.finish().unwrap_err();
        assert_eq!(failure.error, Error::Incomplete);
        assert_eq!(failure.tokenization.input_bytes, 2);
        assert_eq!(failure.tokenization.heap_pops, 0);
        assert!(observation.capture().is_err());
    }

    #[test]
    fn request_guards_precede_tokenization_and_cannot_widen_control_policy() {
        let mut missing = request(b"ab", 2);
        missing.stop_tokens.pop();
        let error = run(65, None).into_generation_admission(0, missing).unwrap_err();
        assert_eq!(error.error, Error::Incomplete);
        assert_eq!(error.tokenization, TokenizationWork::default());
        let stale = run(65, None).into_generation_admission(1, request(b"ab", 2)).unwrap_err();
        assert_eq!(stale.error, Error::Stale);
        assert_eq!(stale.tokenization, TokenizationWork::default());
        let mut small = request(b"ab", 2);
        small.max_output_bytes = 3;
        let error = run(65, None).into_generation_admission(0, small).unwrap_err();
        assert_eq!(error.error, Error::Limit);
        assert_eq!(error.tokenization, TokenizationWork::default());
    }

    #[test]
    fn failed_tokenization_stays_failed_without_numerical_work() {
        let mut input = request(b"ab", 2);
        input.tokenization.heap_pops = 0;
        let mut pending = run(65, None).into_generation_admission(0, input).unwrap();
        let before = pending.decoder_work();
        let failure = loop {
            match pending.advance() {
                Ok(TokenizationProgress::Pending) => {}
                other => break other.unwrap_err(),
            }
        };
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(pending.advance(), Err(failure));
        assert_eq!(pending.decoder_work(), before);
        assert_eq!(pending.sampled_draws(), 0);
        let cancelled = pending.cancel();
        assert_eq!(cancelled.failure(), Some(Error::Limit));
        assert_eq!(cancelled.tokenization_work(), failure.tokenization);
    }

    #[test]
    fn original_monitor_hold_survives_the_admission_handoff() {
        let pending = run(33, Some(33)).into_generation_admission(0, request(b"ab", 3)).unwrap();
        let (owner, report) = drain(pending).finish().unwrap();
        assert_eq!(report.generation().finish(), GenerationFinish::Held);
        assert!(report.bytes().unwrap().is_empty());
        assert_eq!(owner.status(), MonitoringStatus::Held);
        assert_eq!(owner.sampled_draws(), 1);
        let position = owner.position();
        assert_eq!(owner.into_generation_admission(position, request(b"ab", 1))
            .unwrap_err().error, Error::WrongState);
    }
}
