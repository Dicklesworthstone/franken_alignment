//! Pull-based exact-byte output from the ORIGINAL incremental monitored owner.
//! One pull computes at most one token; no output sink or background task exists.
//! Cancellation destroys the unfinished owner, never rewinds or returns its keys.

use super::{PreparedText, TextDecoder, TextGenerationAdmission, TextGenerationFailure,
    TextGenerationReport, TextGenerationRequest, TextOutputError, TokenizationProgress,
    TokenizedInput, append_output};
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
        let mut admission = self.into_generation_admission(expected_position, request)?;
        while admission.advance()? != TokenizationProgress::Complete {}
        admission.finish_incremental()
    }

    // Only the original admission owner can supply this preparation. All
    // numerical request admission remains in the existing GenerationSession.
    fn into_prepared_generation(self, expected_position: u64, prepared: PreparedText)
        -> Result<TextGenerationSession, TextGenerationFailure>
    {
        let work = prepared.encoded.work();
        let PreparedText { encoded, prefix_controls, numerical, output, capacity } = prepared;
        let Self { decoder, tokenizer } = self;
        let numerical = decoder.into_generation(expected_position, numerical)
            .map_err(|error| TextGenerationFailure { error, tokenization: work })?;
        let progress = numerical.progress();
        Ok(TextGenerationSession { numerical, tokenizer, prompt: encoded, prefix_controls,
            progress, output, capacity, decoded_tokens: 0, interrupted: false, output_failure: None })
    }
}

impl TextGenerationAdmission {
    /// Transfer a COMPLETE prompt into the original token-by-token monitored
    /// generation session. No re-tokenization, model inference or sampling runs
    /// during this handoff. Output allocation and original numerical admission
    /// still must succeed before a session is returned.
    ///
    /// Pending tokenization returns Incomplete; a failed tokenizer retains its
    /// first error and performed work. Both refusals consume the original owner,
    /// rather than returning a reusable decoder with erased admission history.
    /// To yield during prompt BPE, call into_generation_admission, advance until
    /// Complete, then finish_incremental. The existing into_generation remains
    /// a synchronous-admission convenience over this same path.
    pub fn finish_incremental(self) -> Result<TextGenerationSession, TextGenerationFailure> {
        let (expected_position, decoder, prepared) = self.into_prepared()?;
        decoder.into_prepared_generation(expected_position, prepared)
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
    capacity: usize,
    decoded_tokens: usize,
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

#[cfg(test)]
mod admission_tests {
    use super::*;
    use super::super::tests::{request, run};
    use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
    use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;

    fn prepared(mut admission: TextGenerationAdmission) -> TextGenerationAdmission {
        let position = admission.position();
        let draws = admission.sampled_draws();
        let decoder = admission.decoder_work();
        let monitoring = admission.monitoring_work();
        for _ in 0..128 {
            let progress = admission.advance().unwrap();
            assert_eq!(admission.position(), position);
            assert_eq!(admission.sampled_draws(), draws);
            assert_eq!(admission.decoder_work(), decoder);
            assert_eq!(admission.monitoring_work(), monitoring);
            if progress == TokenizationProgress::Complete { return admission; }
        }
        panic!("small prompt failed to complete within its bounded step allowance");
    }

    #[test]
    fn cooperative_prompt_to_stream_keeps_exact_work_and_original_output() {
        let mut reference = run(259, None);
        let expected = reference.generate(0, request(b"ab", 3)).unwrap();
        let pending = prepared(run(259, None)
            .into_generation_admission(0, request(b"ab", 3)).unwrap());
        let tokenization = pending.tokenization_work();
        let mut session = pending.finish_incremental().unwrap();
        assert_eq!(session.position(), 0);
        assert_eq!(session.sampled_draws(), 0);
        assert_eq!(session.decoder_work().tokens, 0);
        assert_eq!(session.prompt().work(), tokenization);
        assert_eq!(session.prompt().tokens(), &[258]);
        assert_eq!(session.prompt().spans(), &[0..2]);
        let mut bytes = Vec::new();
        for position in 0..5 {
            let before = session.sampled_draws();
            let chunk = session.advance(position).unwrap();
            assert!(chunk.tokens().len() <= 1);
            assert_eq!(chunk.byte_range(), bytes.len()..bytes.len() + chunk.bytes().len());
            bytes.extend_from_slice(chunk.bytes());
            assert!(session.sampled_draws() - before <= 1);
        }
        assert_eq!(bytes, "ééé".as_bytes());
        assert_eq!(session.progress().finish(), Some(GenerationFinish::TokenLimit));
        assert_eq!(session.decoder_work(), reference.decoder_work());
        assert_eq!(session.monitoring_work(), reference.monitoring_work());
        assert_eq!(session.sampled_draws(), reference.sampled_draws());
        let (_, actual) = session.into_parts().unwrap();
        assert_eq!(actual.bytes(), expected.bytes());
        assert_eq!(actual.generation().tokens(), expected.generation().tokens());
        assert_eq!(actual.generation().work(), expected.generation().work());
        assert_eq!(actual.prompt().work(), tokenization);
    }

    #[test]
    fn pending_handoff_refuses_instead_of_finishing_a_partial_prompt() {
        let owner = run(65, None);
        let source = owner.observation();
        let mut pending = owner.into_generation_admission(0, request(b"ab", 2)).unwrap();
        assert_eq!(pending.advance().unwrap(), TokenizationProgress::Pending);
        let work = pending.tokenization_work();
        assert_eq!(pending.decoder_work().tokens, 0);
        let failure = pending.finish_incremental().unwrap_err();
        assert_eq!(failure.error, Error::Incomplete);
        assert_eq!(failure.tokenization, work);
        assert_eq!(source.availability(), DecoderAvailability::Closed);
    }

    #[test]
    fn cancelling_after_handoff_before_prefill_preserves_prompt_work_not_authority() {
        let owner = run(65, None);
        let source = owner.observation();
        let pending = prepared(owner.into_generation_admission(0, request(b"ab", 2)).unwrap());
        let work = pending.tokenization_work();
        let session = pending.finish_incremental().unwrap();
        let cancelled = session.cancel();
        assert_eq!(source.availability(), DecoderAvailability::Closed);
        assert_eq!(cancelled.prompt().source(), b"ab");
        assert_eq!(cancelled.prompt().tokens(), &[258]);
        assert_eq!(cancelled.prompt().work(), work);
        assert_eq!(cancelled.decoder_work().tokens, 0);
        assert_eq!(cancelled.sampled_draws(), 0);
        assert_eq!(cancelled.progress().reviewed_prompt_tokens(), 0);
        assert!(cancelled.progress().report().is_none());
        assert!(cancelled.bytes().is_empty());
    }

    #[test]
    fn held_output_stays_withheld_through_cooperative_prompt_and_streaming() {
        let pending = prepared(run(33, Some(33))
            .into_generation_admission(0, request(b"ab", 3)).unwrap());
        let mut session = pending.finish_incremental().unwrap();
        for position in 0..3 {
            let chunk = session.advance(position).unwrap();
            assert!(chunk.bytes().is_empty());
            assert!(chunk.tokens().is_empty());
        }
        assert_eq!(session.progress().finish(), Some(GenerationFinish::Held));
        assert_eq!(session.sampled_draws(), 1);
        let (owner, report) = session.into_parts().unwrap();
        assert_eq!(owner.status(), MonitoringStatus::Held);
        assert!(report.bytes().unwrap().is_empty());
        let position = owner.position();
        assert_eq!(owner.into_generation_admission(position, request(b"ab", 1))
            .unwrap_err().error, Error::WrongState);
    }

    #[test]
    fn sticky_tokenization_failure_cannot_be_handed_to_numerical_generation() {
        let owner = run(65, None);
        let source = owner.observation();
        let mut input = request(b"ab", 2);
        input.tokenization.heap_pops = 0;
        let mut pending = owner.into_generation_admission(0, input).unwrap();
        for _ in 0..3 {
            assert_eq!(pending.advance().unwrap(), TokenizationProgress::Pending);
        }
        let failure = pending.advance().unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(failure.tokenization.pair_lookups, 1);
        assert_eq!(failure.tokenization.heap_pops, 0);
        assert_eq!(pending.decoder_work().tokens, 0);
        assert_eq!(pending.finish_incremental().unwrap_err(), failure);
        assert_eq!(source.availability(), DecoderAvailability::Closed);
    }

    #[test]
    fn stale_pull_after_handoff_does_not_charge_or_skip_the_next_valid_step() {
        let pending = prepared(run(65, None)
            .into_generation_admission(0, request(b"ab", 1)).unwrap());
        let mut session = pending.finish_incremental().unwrap();
        let work = session.decoder_work();
        assert_eq!(session.advance(1).unwrap_err(), Error::Stale);
        assert_eq!(session.decoder_work(), work);
        assert_eq!(session.sampled_draws(), 0);
        assert!(!session.interrupted());
        for position in 0..2 {
            assert!(session.advance(position).unwrap().bytes().is_empty());
        }
        assert_eq!(session.advance(2).unwrap().bytes(), b"A");
        let work = session.decoder_work();
        let chunk = session.advance(3).unwrap();
        assert!(chunk.bytes().is_empty());
        assert!(chunk.tokens().is_empty());
        assert_eq!(chunk.finish(), Some(GenerationFinish::TokenLimit));
        assert_eq!(session.decoder_work(), work);
        assert_eq!(session.sampled_draws(), 1);
        assert_eq!(session.into_parts().unwrap().1.bytes().unwrap(), b"A");
    }
}
