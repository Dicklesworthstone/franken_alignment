//! Finite original-token generation through the existing learned decoder guard.
//! Prompt IDs are fixed; continuation choices use the original sampler. Only a
//! completely quiet audit publishes a chosen token and commits its random draw.
pub mod restart;

use super::{PreparedSample, SampledToken, Sampler, SamplerSnapshot, SamplingBudget, SamplingStart};
use super::super::{DecoderBudget, DecoderModel, DecoderStep, DecoderWork, MAX_DECODER_PRODUCTS};
use super::super::monitoring::{LearnedDecoderAllowance, LearnedDecoderEvent, LearnedDecoderPolicy, LearnedDecoderSession};
use super::super::super::model::{MAX_MODEL_KV_VALUES, ModelKvImage, learned::{CompressionBudget,
    CompressionReport, MAX_COMPRESSION_WORK, MAX_LEARNED_IMAGE_BYTES}};
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::{LearnedMonitorBudget,
    LearnedMonitorWork, MAX_LEARNED_MONITOR_COORDINATES, model::{LearnedAuditPreparationBudget, LearnedModelReport}}};
use crate::action::consequence::activation::probe::learned::{CheckedKvBudget, MAX_CHECKED_KV_BYTES,
    MAX_CHECKED_KV_GROUPS, MAX_CHECKED_KV_PRODUCTS};
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

pub const MAX_GENERATION_TOKENS: usize = 4096;
pub const MAX_GENERATION_SCORES: u64 = 1_073_741_824;
pub const MAX_GENERATION_TELEMETRY_VALUES: u64 = MAX_MODEL_KV_VALUES as u64 * MAX_GENERATION_TOKENS as u64;
pub const MAX_GENERATION_TELEMETRY_BYTES: u64 = MAX_CHECKED_KV_BYTES as u64 * MAX_GENERATION_TOKENS as u64;
pub const MAX_GENERATION_TELEMETRY_PRODUCTS: u64 = MAX_CHECKED_KV_PRODUCTS * MAX_GENERATION_TOKENS as u64;
pub const MAX_GENERATION_TELEMETRY_COORDINATES: u64 = MAX_LEARNED_MONITOR_COORDINATES as u64 * MAX_GENERATION_TOKENS as u64;
pub const MAX_GENERATION_TELEMETRY_REFINEMENTS: u64 = MAX_CHECKED_KV_GROUPS as u64 * MAX_GENERATION_TOKENS as u64;

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

/// Whole-run numerical ceilings, checked against the longest declared continuation
/// even when an early stop is likely. Learned telemetry has its own conserved cap.
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

/// Run-wide telemetry ceilings. These are logical source/value/byte/product
/// allowances, not wall-clock, peak RSS or effect authority. Per-token policy
/// caps still apply; this aggregate can only make later work more restrictive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationTelemetryBudget {
    pub compression_source_values: u64,
    pub compression_encoded_bytes: u64,
    pub compression_work_units: u64,
    pub source_check_values: u64,
    pub source_check_encoded_bytes: u64,
    pub source_check_reconstruction_products: u64,
    pub monitor_encoded_bytes: u64,
    pub monitor_probe_coordinates: u64,
    pub monitor_reconstruction_products: u64,
    pub monitor_materialized_values: u64,
    pub monitor_refinements: u64,
}
impl Default for GenerationTelemetryBudget {
    fn default() -> Self {
        Self {
            compression_source_values: MAX_GENERATION_TELEMETRY_VALUES,
            compression_encoded_bytes: MAX_GENERATION_TELEMETRY_BYTES,
            compression_work_units: MAX_GENERATION_TELEMETRY_PRODUCTS,
            source_check_values: MAX_GENERATION_TELEMETRY_VALUES,
            source_check_encoded_bytes: MAX_GENERATION_TELEMETRY_BYTES,
            source_check_reconstruction_products: MAX_GENERATION_TELEMETRY_PRODUCTS,
            monitor_encoded_bytes: MAX_GENERATION_TELEMETRY_BYTES,
            monitor_probe_coordinates: MAX_GENERATION_TELEMETRY_COORDINATES,
            monitor_reconstruction_products: MAX_GENERATION_TELEMETRY_PRODUCTS,
            monitor_materialized_values: MAX_GENERATION_TELEMETRY_VALUES,
            monitor_refinements: MAX_GENERATION_TELEMETRY_REFINEMENTS,
        }
    }
}
impl GenerationTelemetryBudget {
    fn check(self) -> Result<(), Error> {
        if self.compression_source_values > MAX_GENERATION_TELEMETRY_VALUES
            || self.compression_encoded_bytes > MAX_GENERATION_TELEMETRY_BYTES
            || self.compression_work_units > MAX_GENERATION_TELEMETRY_PRODUCTS
            || self.source_check_values > MAX_GENERATION_TELEMETRY_VALUES
            || self.source_check_encoded_bytes > MAX_GENERATION_TELEMETRY_BYTES
            || self.source_check_reconstruction_products > MAX_GENERATION_TELEMETRY_PRODUCTS
            || self.monitor_encoded_bytes > MAX_GENERATION_TELEMETRY_BYTES
            || self.monitor_probe_coordinates > MAX_GENERATION_TELEMETRY_COORDINATES
            || self.monitor_reconstruction_products > MAX_GENERATION_TELEMETRY_PRODUCTS
            || self.monitor_materialized_values > MAX_GENERATION_TELEMETRY_VALUES
            || self.monitor_refinements > MAX_GENERATION_TELEMETRY_REFINEMENTS
        { return Err(Error::Limit); }
        Ok(())
    }

    fn allowance(self, used: GenerationTelemetryWork, policy: &LearnedDecoderPolicy)
        -> Result<LearnedDecoderAllowance, Error>
    {
        if !used.fits(self) { return Err(Error::Binding); }
        let fixed = policy.allowance();
        Ok(LearnedDecoderAllowance {
            preparation: LearnedAuditPreparationBudget {
                compression: CompressionBudget {
                    source_values: cap_usize(self.compression_source_values - used.compression_source_values,
                        fixed.preparation.compression.source_values),
                    encoded_bytes: cap_usize(self.compression_encoded_bytes - used.compression_encoded_bytes,
                        fixed.preparation.compression.encoded_bytes),
                    work_units: (self.compression_work_units - used.compression_work_units)
                        .min(fixed.preparation.compression.work_units),
                },
                source_check: CheckedKvBudget {
                    source_values: cap_usize(self.source_check_values - used.source_check_values,
                        fixed.preparation.source_check.source_values),
                    encoded_bytes: cap_usize(self.source_check_encoded_bytes - used.source_check_encoded_bytes,
                        fixed.preparation.source_check.encoded_bytes),
                    reconstruction_products: (self.source_check_reconstruction_products - used.source_check_reconstruction_products)
                        .min(fixed.preparation.source_check.reconstruction_products),
                },
            },
            monitoring: LearnedMonitorBudget {
                encoded_bytes: cap_usize(self.monitor_encoded_bytes - used.monitor_encoded_bytes, fixed.monitoring.encoded_bytes),
                probe_coordinates: cap_usize(self.monitor_probe_coordinates - used.monitor_probe_coordinates,
                    fixed.monitoring.probe_coordinates),
                reconstruction_products: (self.monitor_reconstruction_products - used.monitor_reconstruction_products)
                    .min(fixed.monitoring.reconstruction_products),
                materialized_values: cap_usize(self.monitor_materialized_values - used.monitor_materialized_values,
                    fixed.monitoring.materialized_values),
                refinements: cap_usize(self.monitor_refinements - used.monitor_refinements, fixed.monitoring.refinements),
            },
        })
    }
}

/// Completed telemetry that returned source-checked evidence. A preparation error
/// may consume some bounded work without a report, but it permanently fails the
/// generation owner, so that unreported remainder can never be reused by it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationTelemetryWork {
    pub compression_source_values: u64,
    pub compression_encoded_bytes: u64,
    pub compression_work_units: u64,
    pub source_check_values: u64,
    pub source_check_encoded_bytes: u64,
    pub source_check_reconstruction_products: u64,
    pub monitor_encoded_bytes: u64,
    pub monitor_probe_coordinates: u64,
    pub monitor_reconstruction_products: u64,
    pub monitor_materialized_values: u64,
    pub monitor_refinements: u64,
}
impl GenerationTelemetryWork {
    fn from_event(event: &LearnedDecoderEvent) -> Result<Self, Error> {
        let checked = event.audit().source().report();
        let monitor: LearnedMonitorWork = event.audit().work();
        Ok(Self {
            compression_source_values: u64::try_from(checked.source_values).map_err(|_| Error::Overflow)?,
            compression_encoded_bytes: u64::try_from(event.compression().encoded_bytes).map_err(|_| Error::Overflow)?,
            compression_work_units: event.compression().work_units_reserved,
            source_check_values: u64::try_from(checked.source_values).map_err(|_| Error::Overflow)?,
            source_check_encoded_bytes: u64::try_from(checked.total_encoded_bytes).map_err(|_| Error::Overflow)?,
            source_check_reconstruction_products: checked.reconstruction_products,
            monitor_encoded_bytes: u64::try_from(monitor.encoded_bytes).map_err(|_| Error::Overflow)?,
            monitor_probe_coordinates: u64::try_from(monitor.probe_coordinates).map_err(|_| Error::Overflow)?,
            monitor_reconstruction_products: monitor.reconstruction_products,
            monitor_materialized_values: u64::try_from(monitor.materialized_values).map_err(|_| Error::Overflow)?,
            monitor_refinements: u64::try_from(monitor.refinements).map_err(|_| Error::Overflow)?,
        })
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        let add = |a: u64, b: u64| a.checked_add(b).ok_or(Error::Overflow);
        Ok(Self {
            compression_source_values: add(self.compression_source_values, other.compression_source_values)?,
            compression_encoded_bytes: add(self.compression_encoded_bytes, other.compression_encoded_bytes)?,
            compression_work_units: add(self.compression_work_units, other.compression_work_units)?,
            source_check_values: add(self.source_check_values, other.source_check_values)?,
            source_check_encoded_bytes: add(self.source_check_encoded_bytes, other.source_check_encoded_bytes)?,
            source_check_reconstruction_products: add(self.source_check_reconstruction_products, other.source_check_reconstruction_products)?,
            monitor_encoded_bytes: add(self.monitor_encoded_bytes, other.monitor_encoded_bytes)?,
            monitor_probe_coordinates: add(self.monitor_probe_coordinates, other.monitor_probe_coordinates)?,
            monitor_reconstruction_products: add(self.monitor_reconstruction_products, other.monitor_reconstruction_products)?,
            monitor_materialized_values: add(self.monitor_materialized_values, other.monitor_materialized_values)?,
            monitor_refinements: add(self.monitor_refinements, other.monitor_refinements)?,
        })
    }
    fn fits(self, budget: GenerationTelemetryBudget) -> bool {
        self.compression_source_values <= budget.compression_source_values
            && self.compression_encoded_bytes <= budget.compression_encoded_bytes
            && self.compression_work_units <= budget.compression_work_units
            && self.source_check_values <= budget.source_check_values
            && self.source_check_encoded_bytes <= budget.source_check_encoded_bytes
            && self.source_check_reconstruction_products <= budget.source_check_reconstruction_products
            && self.monitor_encoded_bytes <= budget.monitor_encoded_bytes
            && self.monitor_probe_coordinates <= budget.monitor_probe_coordinates
            && self.monitor_reconstruction_products <= budget.monitor_reconstruction_products
            && self.monitor_materialized_values <= budget.monitor_materialized_values
            && self.monitor_refinements <= budget.monitor_refinements
    }
}
fn cap_usize(remaining: u64, fixed: usize) -> usize {
    usize::try_from(remaining).map_or(fixed, |value| value.min(fixed))
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
pub struct LearnedGeneration {
    model: DecoderModel,
    guard: LearnedDecoderSession,
    sampler: Sampler,
    spec: GenerationSpec,
    budget: GenerationBudget,
    telemetry_budget: GenerationTelemetryBudget,
    estimate: GenerationEstimate,
    status: GenerationStatus,
    work: GenerationWork,
    telemetry_work: GenerationTelemetryWork,
    samples: Vec<SampledToken>,
    last_event: Option<Rc<GenerationEvent>>,
}
impl fmt::Debug for LearnedGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedGeneration").field("position", &self.position()).field("status", &self.status)
            .field("generated", &self.samples.len()).field("work", &self.work)
            .field("telemetry_work", &self.telemetry_work).finish_non_exhaustive()
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

    pub fn monitored_generation(&self, stream: u64, evaluation_origin: u64, spec: GenerationSpec,
        policy: LearnedDecoderPolicy, budget: GenerationBudget) -> Result<LearnedGeneration, Error>
    {
        self.monitored_generation_with_telemetry(stream, evaluation_origin, spec, policy, budget,
            GenerationTelemetryBudget::default())
    }

    /// The aggregate telemetry budget is immutable for the run. Unlike the
    /// original numerical estimate, data-dependent residual/refinement costs are
    /// admitted progressively through the remaining allowance at each token.
    pub fn monitored_generation_with_telemetry(&self, stream: u64, evaluation_origin: u64, spec: GenerationSpec,
        policy: LearnedDecoderPolicy, budget: GenerationBudget, telemetry_budget: GenerationTelemetryBudget)
        -> Result<LearnedGeneration, Error>
    {
        telemetry_budget.check()?;
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
        Ok(LearnedGeneration { model: self.clone(), guard, sampler, spec, budget, telemetry_budget, estimate,
            status: GenerationStatus::Prefilling, work: GenerationWork::default(), telemetry_work: GenerationTelemetryWork::default(),
            samples, last_event: None })
    }
}
impl LearnedGeneration {
    pub fn spec(&self) -> &GenerationSpec { &self.spec }
    pub fn policy(&self) -> &LearnedDecoderPolicy { self.guard.policy() }
    pub fn evaluation_origin(&self) -> u64 { self.guard.evaluation_origin() }
    pub fn budget(&self) -> GenerationBudget { self.budget }
    pub fn telemetry_budget(&self) -> GenerationTelemetryBudget { self.telemetry_budget }
    pub fn estimate(&self) -> GenerationEstimate { self.estimate }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn position(&self) -> u64 { self.guard.position() }
    pub fn work(&self) -> GenerationWork { self.work }
    pub fn telemetry_work(&self) -> GenerationTelemetryWork { self.telemetry_work }
    pub fn accepted_tokens(&self) -> &[u32] { self.guard.accepted_tokens() }
    pub fn generated_tokens(&self) -> &[u32] {
        self.accepted_tokens().get(self.spec.prompt.len()..).unwrap_or(&[])
    }
    pub fn samples(&self) -> &[SampledToken] { &self.samples }
    pub fn sampler_state(&self) -> SamplerSnapshot { self.sampler.snapshot() }
    pub fn accepted_logits(&self) -> Result<&[f32], Error> { self.guard.accepted_logits() }
    pub fn accepted_cache_image(&self) -> Result<ModelKvImage, Error> { self.guard.accepted_cache_image() }
    pub fn last_event(&self) -> Option<&GenerationEvent> { self.last_event.as_deref() }

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
        let allowance = self.telemetry_budget.allowance(self.telemetry_work, self.guard.policy())?;
        let evidence = self.guard.advance_with_allowance(position, token, allowance)?;
        let next_telemetry = self.telemetry_work.add(GenerationTelemetryWork::from_event(&evidence)?)?;
        if !next_telemetry.fits(self.telemetry_budget) { return Err(Error::Binding); }
        self.telemetry_work = next_telemetry;
        if evidence.step().is_none() {
            return Ok(GenerationEvent { phase, position, status: GenerationStatus::Held(evidence.audit().outcome()),
                evidence, sample: None });
        }
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
