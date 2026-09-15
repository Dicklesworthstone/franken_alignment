//! Finite original-token generation through the existing learned decoder guard.
//! Prompt IDs are fixed; continuation choices use the original sampler. Only a
//! completely quiet audit publishes a chosen token and commits its random draw.
use super::{PreparedSample, SampledToken, Sampler, SamplerSnapshot, SamplingBudget, SamplingStart};
use super::super::{DecoderBudget, DecoderModel, DecoderStep, DecoderWork, MAX_DECODER_PRODUCTS};
use super::super::monitoring::{LearnedDecoderEvent, LearnedDecoderPolicy, LearnedDecoderSession};
use super::super::super::model::{ModelKvImage, learned::CompressionReport};
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::model::LearnedModelReport};
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

pub const MAX_GENERATION_TOKENS: usize = 4096;
pub const MAX_GENERATION_SCORES: u64 = 1_073_741_824;

/// Frozen before any inference. Stop IDs apply only to accepted continuations,
/// never to the teacher-forced prompt. No tokenizer or stop-string guesswork.
#[derive(Clone, Debug)]
pub struct GenerationSpec {
    prompt: Vec<u32>,
    max_new_tokens: usize,
    stop_tokens: BTreeSet<u32>,
    sampling: SamplingStart,
}
impl GenerationSpec {
    pub fn new(prompt: Vec<u32>, max_new_tokens: usize, stop_tokens: BTreeSet<u32>, sampling: SamplingStart)
        -> Result<Self, Error>
    {
        if prompt.is_empty() || max_new_tokens == 0 { return Err(Error::InvalidInput); }
        let total = prompt.len().checked_add(max_new_tokens).ok_or(Error::Overflow)?;
        if total > MAX_GENERATION_TOKENS || stop_tokens.len() > sampling.policy.vocabulary() {
            return Err(Error::Limit);
        }
        Ok(Self { prompt, max_new_tokens, stop_tokens, sampling })
    }
    pub fn prompt(&self) -> &[u32] { &self.prompt }
    pub fn max_new_tokens(&self) -> usize { self.max_new_tokens }
    pub fn stop_tokens(&self) -> &BTreeSet<u32> { &self.stop_tokens }
    pub fn sampling(&self) -> &SamplingStart { &self.sampling }
}

/// Whole-run ceilings, checked against the longest declared continuation even
/// when an early stop is likely. Learned preparation/audit caps remain frozen in
/// LearnedDecoderPolicy; these counters do not claim CPU time, RSS or authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationBudget { pub decoder_products: u64, pub vocabulary_scores: u64 }
impl Default for GenerationBudget {
    fn default() -> Self {
        Self { decoder_products: MAX_DECODER_PRODUCTS, vocabulary_scores: MAX_GENERATION_SCORES }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationEstimate {
    pub decoder: DecoderWork,
    pub vocabulary_scores: u64,
    pub audited_positions: usize,
}

/// Reservations are monotone, including a held or failed attempt. The decoder
/// reservation precedes sampling, so it can exceed work actually executed when
/// sampling itself fails. accepted_decoder counts only original committed work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationWork {
    pub admitted_tokens: u64,
    pub reserved_decoder_products: u64,
    pub sampling_attempts: u64,
    pub reserved_vocabulary_scores: u64,
    pub accepted_decoder: DecoderWork,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationPhase { Prompt, Continuation }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationStop { TokenLimit, StopToken(u32) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationStatus {
    Prefilling, Generating, Finished(GenerationStop), Held(MonitorOutcome), Failed(Error),
}
impl GenerationStatus {
    pub fn is_active(self) -> bool { matches!(self, Self::Prefilling | Self::Generating) }
}

/// A held event exposes numerical audit evidence but no candidate ID, sampled
/// random word, or pending logits. It deliberately does not expose the underlying
/// decoder event, whose input-token accessor is appropriate only for forced IDs.
/// This is API publication discipline, not secrecy: replay state and numerical
/// evidence may let callers independently infer a rejected candidate.
#[derive(Clone)]
pub struct GenerationEvent {
    phase: GenerationPhase,
    position: u64,
    status: GenerationStatus,
    evidence: Rc<LearnedDecoderEvent>,
    sample: Option<SampledToken>,
}
impl fmt::Debug for GenerationEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationEvent").field("phase", &self.phase).field("position", &self.position)
            .field("status", &self.status).field("accepted", &self.accepted().is_some()).finish_non_exhaustive()
    }
}
impl GenerationEvent {
    pub fn phase(&self) -> GenerationPhase { self.phase }
    pub fn position(&self) -> u64 { self.position }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn accepted(&self) -> Option<&DecoderStep> { self.evidence.step() }
    pub fn sample(&self) -> Option<&SampledToken> { self.sample.as_ref() }
    pub fn audit(&self) -> &LearnedModelReport { self.evidence.audit() }
    pub fn compression(&self) -> &CompressionReport { self.evidence.compression() }
}

/// One non-cloneable generation path. There is no forced-token override, mutable
/// guard/sampler, unchecked prefix import, hold reset or mid-run budget change.
/// Accepted tokens remain numerical observations, not external-effect permits.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::LearnedGeneration;
/// fn bypass(run: &mut LearnedGeneration) { run.guard_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationEvent};
/// fn authorize(event: GenerationEvent) -> Permit { event }
/// ```
pub struct LearnedGeneration {
    model: DecoderModel,
    guard: LearnedDecoderSession,
    sampler: Sampler,
    spec: GenerationSpec,
    budget: GenerationBudget,
    estimate: GenerationEstimate,
    status: GenerationStatus,
    work: GenerationWork,
    samples: Vec<SampledToken>,
    last_event: Option<Rc<GenerationEvent>>,
}
impl fmt::Debug for LearnedGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The guard's own Debug can include the input ID of a held decoder event.
        // Do not delegate to it for a privately selected continuation candidate.
        f.debug_struct("LearnedGeneration").field("position", &self.position()).field("status", &self.status)
            .field("generated", &self.samples.len()).field("work", &self.work).finish_non_exhaustive()
    }
}
impl DecoderModel {
    pub fn estimate_monitored_generation(&self, spec: &GenerationSpec) -> Result<GenerationEstimate, Error> {
        let vocabulary = self.profile().shape().vocabulary;
        if spec.sampling.policy.vocabulary() != vocabulary { return Err(Error::Binding); }
        if spec.sampling.stream == 0 || spec.prompt.iter().chain(&spec.stop_tokens)
            .any(|token| *token as usize >= vocabulary) { return Err(Error::InvalidInput); }
        let positions = spec.prompt.len().checked_add(spec.max_new_tokens).ok_or(Error::Overflow)?;
        let decoder = self.estimate(0, positions)?;
        let vocabulary_scores = (vocabulary as u64).checked_mul(spec.max_new_tokens as u64).ok_or(Error::Overflow)?;
        if vocabulary_scores > MAX_GENERATION_SCORES { return Err(Error::Limit); }
        Ok(GenerationEstimate { decoder, vocabulary_scores, audited_positions: positions })
    }

    /// Preflight the full declared run before any token computation. The largest
    /// single-token decoder cost must also fit the existing frozen guard policy.
    pub fn monitored_generation(&self, stream: u64, evaluation_origin: u64, spec: GenerationSpec,
        policy: LearnedDecoderPolicy, budget: GenerationBudget) -> Result<LearnedGeneration, Error>
    {
        let estimate = self.estimate_monitored_generation(&spec)?;
        estimate.decoder.check(DecoderBudget { scalar_products: budget.decoder_products })?;
        if budget.vocabulary_scores > MAX_GENERATION_SCORES || estimate.vocabulary_scores > budget.vocabulary_scores {
            return Err(Error::Limit);
        }
        self.estimate(estimate.audited_positions - 1, 1)?.check(policy.inference())?;
        let sampler = Sampler::seeded(spec.sampling.policy.clone(), spec.sampling.stream, spec.sampling.seed)?;
        let mut samples = Vec::new();
        samples.try_reserve_exact(spec.max_new_tokens).map_err(|_| Error::Limit)?;
        let guard = self.monitored_session(stream, evaluation_origin, policy)?;
        Ok(LearnedGeneration { model: self.clone(), guard, sampler, spec, budget, estimate,
            status: GenerationStatus::Prefilling, work: GenerationWork::default(), samples, last_event: None })
    }
}
impl LearnedGeneration {
    pub fn spec(&self) -> &GenerationSpec { &self.spec }
    pub fn policy(&self) -> &LearnedDecoderPolicy { self.guard.policy() }
    pub fn evaluation_origin(&self) -> u64 { self.guard.evaluation_origin() }
    pub fn budget(&self) -> GenerationBudget { self.budget }
    pub fn estimate(&self) -> GenerationEstimate { self.estimate }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn position(&self) -> u64 { self.guard.position() }
    pub fn work(&self) -> GenerationWork { self.work }
    pub fn accepted_tokens(&self) -> &[u32] { self.guard.accepted_tokens() }
    pub fn generated_tokens(&self) -> &[u32] {
        self.accepted_tokens().get(self.spec.prompt.len()..).unwrap_or(&[])
    }
    pub fn samples(&self) -> &[SampledToken] { &self.samples }
    pub fn sampler_state(&self) -> SamplerSnapshot { self.sampler.snapshot() }
    pub fn accepted_logits(&self) -> Result<&[f32], Error> { self.guard.accepted_logits() }
    /// Explicit diagnostic export; never invoked by the incremental hot path.
    pub fn accepted_cache_image(&self) -> Result<ModelKvImage, Error> { self.guard.accepted_cache_image() }
    /// Last completed audit event, which can precede a subsequent Failed status.
    pub fn last_event(&self) -> Option<&GenerationEvent> { self.last_event.as_deref() }

    /// Audit exactly one fixed prompt ID or one privately sampled continuation.
    /// Stale calls spend nothing. Every real attempt latches on failure or hold;
    /// there is no opportunity to retry another draw or skip an unaccepted token.
    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<GenerationEvent>, Error> {
        if !self.status.is_active() { return Err(Error::WrongState); }
        if expected_position != self.position() { return Err(Error::Stale); }
        self.status = GenerationStatus::Failed(Error::Incomplete);
        match self.advance_inner() {
            Ok(event) => {
                self.status = event.status;
                let event = Rc::new(event);
                self.last_event = Some(Rc::clone(&event));
                Ok(event)
            }
            Err(error) => { self.status = GenerationStatus::Failed(error); Err(error) }
        }
    }

    /// Drives the same one-token state machine; it never switches to an ordinary
    /// unmonitored prefill or generation implementation. Repeated terminal calls
    /// do no work; failures are returned again rather than relabeled as a stop.
    pub fn run_to_stop(&mut self) -> Result<GenerationStatus, Error> {
        while self.status.is_active() { self.advance(self.position())?; }
        match self.status { GenerationStatus::Failed(error) => Err(error), status => Ok(status) }
    }

    fn advance_inner(&mut self) -> Result<GenerationEvent, Error> {
        let position = self.position();
        let phase = if position < self.spec.prompt.len() as u64 { GenerationPhase::Prompt }
            else { GenerationPhase::Continuation };
        let planned = self.model.estimate(position as usize, 1)?;
        let next_accepted = self.work.accepted_decoder.add(planned)?;
        let mut reserved = self.work;
        reserved.admitted_tokens = reserved.admitted_tokens.checked_add(1).ok_or(Error::Overflow)?;
        reserved.reserved_decoder_products = reserved.reserved_decoder_products
            .checked_add(planned.scalar_products()?).ok_or(Error::Overflow)?;
        if phase == GenerationPhase::Continuation {
            reserved.sampling_attempts = reserved.sampling_attempts.checked_add(1).ok_or(Error::Overflow)?;
            reserved.reserved_vocabulary_scores = reserved.reserved_vocabulary_scores
                .checked_add(self.spec.sampling.policy.vocabulary() as u64).ok_or(Error::Overflow)?;
        }
        if reserved.admitted_tokens > self.estimate.audited_positions as u64
            || reserved.reserved_decoder_products > self.budget.decoder_products
            || reserved.reserved_vocabulary_scores > self.budget.vocabulary_scores { return Err(Error::Limit); }
        self.work = reserved;
        let prepared: Option<PreparedSample> = if phase == GenerationPhase::Continuation {
            Some(self.sampler.prepare(self.guard.accepted_logits()?,
                SamplingBudget { vocabulary: self.spec.sampling.policy.vocabulary() })?)
        } else { None };
        let token = match &prepared {
            Some(prepared) => prepared.sample.token,
            None => self.spec.prompt[position as usize],
        };
        let evidence = self.guard.advance(position, token)?;
        if evidence.step().is_none() {
            return Ok(GenerationEvent { phase, position, status: GenerationStatus::Held(evidence.audit().outcome()),
                evidence, sample: None });
        }
        // The original guarded all-layer transaction has committed. Only
        // infallible assignments and a constructor-preallocated push follow.
        self.work.accepted_decoder = next_accepted;
        let sample = prepared.map(|prepared| {
            self.sampler.snapshot = prepared.next;
            self.samples.push(prepared.sample.clone());
            prepared.sample
        });
        let status = match phase {
            GenerationPhase::Prompt if self.position() < self.spec.prompt.len() as u64 => GenerationStatus::Prefilling,
            GenerationPhase::Prompt => GenerationStatus::Generating,
            GenerationPhase::Continuation if self.spec.stop_tokens.contains(&token) =>
                GenerationStatus::Finished(GenerationStop::StopToken(token)),
            GenerationPhase::Continuation if self.samples.len() == self.spec.max_new_tokens =>
                GenerationStatus::Finished(GenerationStop::TokenLimit),
            GenerationPhase::Continuation => GenerationStatus::Generating,
        };
        Ok(GenerationEvent { phase, position, status, evidence, sample })
    }
}
