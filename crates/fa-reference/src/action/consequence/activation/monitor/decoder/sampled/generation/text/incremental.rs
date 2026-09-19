//! Pull-based exact-byte output from the ORIGINAL incremental monitored owner.
//! One pull computes at most one token; no output sink or background task exists.
//! Cancellation destroys the unfinished owner, never rewinds or returns its keys.

use super::{PreparedText, TextDecoder, TextGenerationFailure, TextGenerationReport,
    TextGenerationRequest, TextOutputError, TokenizationWork, TokenizedInput, append_output};
use super::super::{GenerationFinish, GenerationWork};
use super::super::incremental::{GenerationProgress, GenerationSession};
use crate::action::consequence::activation::monitor::decoder::MonitoringWork;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderWork;
use crate::Error;
use std::fmt;
use std::ops::Range;

/// A borrowed delta from one successful pull. Empty during prefill, a held/stop
/// token, budget exhaustion, and repeated terminal polling. Offsets name exact
/// cumulative output bytes, not UTF-8 character positions or publication receipts.
/// A borrowed chunk cannot be retained across another mutable pull without an
/// explicit caller-owned copy. Debug omits all token IDs and content bytes.
pub struct TextChunk<'a> {
    position: u64,
    byte_range: Range<usize>,
    bytes: &'a [u8],
    tokens: &'a [u32],
    finish: Option<GenerationFinish>,
    work: GenerationWork,
}
impl TextChunk<'_> {
    pub fn position(&self) -> u64 { self.position }
    pub fn byte_range(&self) -> Range<usize> { self.byte_range.clone() }
    pub fn bytes(&self) -> &[u8] { self.bytes }
    pub fn tokens(&self) -> &[u32] { self.tokens }
    pub fn finish(&self) -> Option<GenerationFinish> { self.finish }
    pub fn work(&self) -> GenerationWork { self.work }
}
impl fmt::Debug for TextChunk<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextChunk").field("position", &self.position)
            .field("byte_range", &self.byte_range).field("tokens", &self.tokens.len())
            .field("finish", &self.finish).field("work", &self.work).finish_non_exhaustive()
    }
}

/// Fixed request plus the sole numerical owner. There is no owner accessor or
/// partial-request export. Yielding retains the exact cache, PRNG, budget and
/// tokenizer; a caller cannot change the prompt or widen the request's allowance.
/// A caught unwind or output failure latches this object before another pull.
/// Quiet output is a numerical observation, never authorization to publish it.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::incremental::TextGenerationSession;
/// fn bypass(run: &mut TextGenerationSession) { run.decoder_mut(); }
/// ```
#[must_use = "advance, complete, or cancel the owned monitored generation"]
pub struct TextGenerationSession {
    numerical: GenerationSession,
    tokenizer: super::ByteBpe,
    prompt: TokenizedInput,
    prefix_controls: Vec<u32>,
    progress: GenerationProgress,
    output: Vec<u8>,
    capacity: usize,
    decoded_tokens: usize,
    interrupted: bool,
    output_failure: Option<Error>,
}
impl fmt::Debug for TextGenerationSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextGenerationSession").field("progress", &self.progress)
            .field("output_bytes", &self.output.len()).field("interrupted", &self.interrupted)
            .field("output_failure", &self.output_failure).finish_non_exhaustive()
    }
}

impl TextDecoder {
    /// Freeze and tokenize the COMPLETE request and allocate output capacity
    /// before the first pull. No inference occurs here. This consumes the supplied
    /// owner even on admission refusal, as the native into_generation API does;
    /// generate remains the borrowed alternative. Only a terminated session can
    /// later return this same TextDecoder, with all original latches preserved.
    pub fn into_generation(self, expected_position: u64, request: TextGenerationRequest)
        -> Result<TextGenerationSession, TextGenerationFailure>
    {
        let mut work = TokenizationWork::default();
        let prepared = self.prepare_text(expected_position, request, &mut work)
            .map_err(|error| TextGenerationFailure { error, tokenization: work })?;
        let PreparedText { encoded, prefix_controls, numerical, output, capacity } = prepared;
        let Self { decoder, tokenizer } = self;
        let numerical = decoder.into_generation(expected_position, numerical)
            .map_err(|error| TextGenerationFailure { error, tokenization: work })?;
        let progress = numerical.progress();
        Ok(TextGenerationSession { numerical, tokenizer, prompt: encoded, prefix_controls,
            progress, output, capacity, decoded_tokens: 0, interrupted: false, output_failure: None })
    }
}

impl TextGenerationSession {
    pub fn prompt(&self) -> &TokenizedInput { &self.prompt }
    pub fn prefix_controls(&self) -> &[u32] { &self.prefix_controls }
    /// Last returned native progress, not a claim about an interrupted operation.
    pub fn progress(&self) -> &GenerationProgress { &self.progress }
    pub fn position(&self) -> u64 { self.numerical.position() }
    pub fn sampled_draws(&self) -> u64 { self.numerical.sampled_draws() }
    pub fn decoder_work(&self) -> DecoderWork { self.numerical.decoder_work() }
    pub fn monitoring_work(&self) -> MonitoringWork { self.numerical.monitoring_work() }
    pub fn interrupted(&self) -> bool { self.interrupted }
    pub fn output_failure(&self) -> Option<Error> { self.output_failure }
    pub fn bytes(&self) -> Result<&[u8], Error> {
        if self.interrupted { return Err(Error::Incomplete); }
        if let Some(error) = self.output_failure { return Err(error); }
        Ok(&self.output)
    }
    /// A token can end inside a Unicode character. Retain those exact bytes;
    /// strict text access succeeds only once the full prefix is valid UTF-8.
    pub fn utf8(&self) -> Result<&str, TextOutputError> {
        std::str::from_utf8(self.bytes().map_err(TextOutputError::Contract)?)
            .map_err(TextOutputError::InvalidUtf8)
    }

    /// At most one original forced/sampled token per pull. There is no lookahead,
    /// speculative generation, re-tokenization, callback sink, or automatic retry.
    /// Every call checks the expected numerical position before charging work.
    pub fn advance(&mut self, expected_position: u64) -> Result<TextChunk<'_>, Error> {
        self.advance_with(expected_position, || {})
    }

    // Private interruption seam, not a user callback or alternative token source.
    fn advance_with<F: FnOnce()>(&mut self, expected_position: u64, after_native: F)
        -> Result<TextChunk<'_>, Error>
    {
        if self.interrupted || self.output_failure.is_some() { return Err(Error::WrongState); }
        if expected_position != self.position() { return Err(Error::Stale); }
        let byte_start = self.output.len();
        let token_start = self.decoded_tokens;
        self.interrupted = true;
        self.progress = self.numerical.advance(expected_position)?;
        after_native();
        let result = (|| {
            let tokens = self.progress.tokens().get(token_start..).ok_or(Error::Binding)?;
            // Native advance releases at most one token; validate this before
            // appending anything, so an upstream error cannot smuggle a batch.
            if tokens.len() > 1 { return Err(Error::Binding); }
            append_output(&self.tokenizer, &mut self.output, tokens, self.capacity)
        })();
        if let Err(error) = result {
            self.output_failure = Some(error);
            self.interrupted = false;
            return Err(error);
        }
        self.decoded_tokens = self.progress.tokens().len();
        self.interrupted = false;
        Ok(TextChunk { position: self.position(), byte_range: byte_start..self.output.len(),
            bytes: &self.output[byte_start..], tokens: &self.progress.tokens()[token_start..],
            finish: self.progress.finish(), work: self.progress.work() })
    }

    /// Return the same owner ONLY after native termination, including a held or
    /// failed numerical finish whose original terminal latch remains unchanged.
    /// Output failure/interruption cannot export the owner for another draw.
    pub fn into_parts(self) -> Result<(TextDecoder, TextGenerationReport), Error> {
        if self.interrupted || self.output_failure.is_some() || self.progress.report().is_none() {
            return Err(Error::Incomplete);
        }
        let (decoder, generation) = self.numerical.into_parts()?;
        let report = TextGenerationReport { prompt: self.prompt, prefix_controls: self.prefix_controls,
            generation, output: Ok(self.output) };
        Ok((TextDecoder { decoder, tokenizer: self.tokenizer }, report))
    }

    /// Stop without computing another token. Destroy the numerical owner BEFORE
    /// returning evidence, invalidating its live observation handles. No partial
    /// owner, RNG reset, refund or fabricated successful GenerationReport escapes.
    /// The native finish (possibly None) is preserved even on terminal cancellation.
    pub fn cancel(self) -> CancelledTextGeneration {
        let Self { numerical, prompt, prefix_controls, progress, output,
            interrupted, output_failure, .. } = self;
        let decoder_work = numerical.decoder_work();
        let monitoring_work = numerical.monitoring_work();
        let sampled_draws = numerical.sampled_draws();
        drop(numerical);
        CancelledTextGeneration { prompt, prefix_controls, progress, output, interrupted,
            output_failure, decoder_work, monitoring_work, sampled_draws }
    }
}

/// Historical observations of destroyed ownership, not a completed generation or
/// effect outcome. After interruption, progress and output may precede the actual
/// native work counters. The last safely decoded prefix remains exact evidence.
/// Cancellation cannot erase a held/failed finish or restore a live observation.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationReport;
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::text::incremental::CancelledTextGeneration;
/// fn fabricate_done(cancelled: CancelledTextGeneration) -> GenerationReport { cancelled }
/// ```
pub struct CancelledTextGeneration {
    prompt: TokenizedInput,
    prefix_controls: Vec<u32>,
    progress: GenerationProgress,
    output: Vec<u8>,
    interrupted: bool,
    output_failure: Option<Error>,
    decoder_work: DecoderWork,
    monitoring_work: MonitoringWork,
    sampled_draws: u64,
}
impl CancelledTextGeneration {
    pub fn prompt(&self) -> &TokenizedInput { &self.prompt }
    pub fn prefix_controls(&self) -> &[u32] { &self.prefix_controls }
    pub fn progress(&self) -> &GenerationProgress { &self.progress }
    pub fn bytes(&self) -> &[u8] { &self.output }
    pub fn interrupted(&self) -> bool { self.interrupted }
    pub fn output_failure(&self) -> Option<Error> { self.output_failure }
    pub fn decoder_work(&self) -> DecoderWork { self.decoder_work }
    pub fn monitoring_work(&self) -> MonitoringWork { self.monitoring_work }
    pub fn sampled_draws(&self) -> u64 { self.sampled_draws }
}
impl fmt::Debug for CancelledTextGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelledTextGeneration").field("progress", &self.progress)
            .field("output_bytes", &self.output.len()).field("interrupted", &self.interrupted)
            .field("output_failure", &self.output_failure).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
