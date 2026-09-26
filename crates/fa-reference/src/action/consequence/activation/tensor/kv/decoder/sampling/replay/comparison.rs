//! Paired learned-monitor policy experiments over identical numerical inputs.
//!
//! Both arms use the original model, prompt, stop IDs, sampling recipe and budget
//! ceilings. Only the learned monitoring policy differs. Every accepted pair
//! compares exact cache/logit words and random draws, not just output token IDs.
//! A decision difference, missing audit, failure or bound ends the experiment.
//! This L7 diagnostic has no permit, mutable arm or executable-owner accessor.

use super::{CheckpointLimits, Recipe, ReplayableGeneration, State, MAX_REPLAY_STATE_BYTES, same_sample};
use super::super::monitored::{
    GenerationEvent, GenerationStatus, GenerationStop, GenerationTelemetryWork, GenerationWork,
    LearnedGeneration, MAX_GENERATION_SCORES, MAX_GENERATION_TOKENS,
};
use super::super::super::{DecoderStep, MAX_DECODER_PRODUCTS, monitoring::LearnedDecoderPolicy};
use crate::Error;
use crate::action::consequence::activation::monitor::learned::model::LearnedModelReport;
use std::fmt;
use std::rc::Rc;

/// A bound on attempted positions in EACH arm and on EACH temporary comparison
/// state. There are at most two such states at once, in addition to the owners.
/// The original numerical and telemetry ceilings apply separately to each arm;
/// neither an early hold nor dropping this object refunds their consumed work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonLimits {
    pub positions: usize,
    pub state_bytes: usize,
    /// Fresh numerical allowance for BOTH experimental arms together.
    pub decoder_products: u64,
    pub vocabulary_scores: u64,
}
impl Default for ComparisonLimits {
    fn default() -> Self {
        Self { positions: MAX_GENERATION_TOKENS, state_bytes: MAX_REPLAY_STATE_BYTES,
            decoder_products: MAX_DECODER_PRODUCTS.saturating_mul(2),
            vocabulary_scores: MAX_GENERATION_SCORES.saturating_mul(2) }
    }
}
impl ComparisonLimits {
    fn check(self) -> Result<(), Error> {
        if self.positions > MAX_GENERATION_TOKENS || self.state_bytes > MAX_REPLAY_STATE_BYTES
            || self.decoder_products > MAX_DECODER_PRODUCTS.saturating_mul(2)
            || self.vocabulary_scores > MAX_GENERATION_SCORES.saturating_mul(2) {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Both arms retain ONE original observation lineage. A comparison is not two
/// independent observations, a new live capture, or a policy promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonLineage {
    pub stream: u64,
    pub evaluation_origin: u64,
    pub source_position: u64,
    pub source_status: GenerationStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonStatus {
    Active,
    MatchedStop(GenerationStop),
    DecisionDifference { position: u64 },
    BothHeld { position: u64 },
    Exhausted,
    Failed(Error),
}

/// Reservations include unsuccessful attempts. On a generation error, telemetry
/// may have done work without returning its full report; the flag makes that
/// incompleteness explicit rather than turning absent accounting into zero work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonWork {
    pub baseline: GenerationWork,
    pub candidate: GenerationWork,
    pub baseline_telemetry: GenerationTelemetryWork,
    pub candidate_telemetry: GenerationTelemetryWork,
    pub all_attempted_telemetry_reported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonReport {
    pub lineage: ComparisonLineage,
    pub status: ComparisonStatus,
    pub attempted_positions: usize,
    pub matched_positions: usize,
    /// Sum of logical bytes in completed paired state comparisons, not physical
    /// byte movement, allocated memory, or a count of independent observations.
    pub state_bytes_compared: u64,
    pub work: ComparisonWork,
}

/// Original audit observations from one paired attempt. An accepted decoder step
/// is exposed ONLY when both arms passed and their complete numerical states
/// matched. A permissive candidate cannot disclose the other arm's held token.
#[derive(Clone)]
pub struct ComparisonStep {
    position: u64,
    baseline: Result<Rc<GenerationEvent>, Error>,
    candidate: Result<Rc<GenerationEvent>, Error>,
    jointly_accepted: bool,
}
impl fmt::Debug for ComparisonStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComparisonStep")
            .field("position", &self.position)
            .field("baseline", &self.baseline_status())
            .field("candidate", &self.candidate_status())
            .field("jointly_accepted", &self.jointly_accepted).finish_non_exhaustive()
    }
}
impl ComparisonStep {
    pub fn position(&self) -> u64 { self.position }
    pub fn baseline_status(&self) -> GenerationStatus {
        self.baseline.as_ref().map_or_else(|error| GenerationStatus::Failed(*error), |event| event.status())
    }
    pub fn candidate_status(&self) -> GenerationStatus {
        self.candidate.as_ref().map_or_else(|error| GenerationStatus::Failed(*error), |event| event.status())
    }
    pub fn baseline_audit(&self) -> Option<&LearnedModelReport> {
        self.baseline.as_ref().ok().map(|event| event.audit())
    }
    pub fn candidate_audit(&self) -> Option<&LearnedModelReport> {
        self.candidate.as_ref().ok().map(|event| event.audit())
    }
    pub fn accepted(&self) -> Option<&DecoderStep> {
        if !self.jointly_accepted { return None; }
        self.baseline.as_ref().ok().and_then(|event| event.accepted())
    }
}

/// A non-cloneable experiment owner; comparison never installs candidate policy
/// in the source generation and never exposes either executable arm.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::PolicyComparison;
/// fn escape(pair: PolicyComparison) { let _run = pair.into_generation(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::PolicyComparison;
/// fn bypass(pair: &mut PolicyComparison) { pair.candidate_mut(); }
/// ```
pub struct PolicyComparison {
    baseline: LearnedGeneration,
    candidate: LearnedGeneration,
    lineage: ComparisonLineage,
    limits: ComparisonLimits,
    status: ComparisonStatus,
    attempted: usize,
    matched: usize,
    state_bytes_compared: u64,
    last: Option<Rc<ComparisonStep>>,
}
impl fmt::Debug for PolicyComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyComparison").field("report", &self.report()).finish_non_exhaustive()
    }
}

impl ReplayableGeneration {
    /// Recompute a paired experiment FROM POSITION ZERO, not from this owner's
    /// current position. The source may already be stopped or held; it is only
    /// borrowed and cannot be resumed, reset or weakened by this operation.
    pub fn compare_policy_from_start(&self, candidate: LearnedDecoderPolicy, limits: ComparisonLimits)
        -> Result<PolicyComparison, Error>
    {
        PolicyComparison::start(&self.recipe, candidate, limits, ComparisonLineage {
            stream: self.recipe.stream,
            evaluation_origin: self.recipe.evaluation_origin,
            source_position: self.run.position(),
            source_status: self.run.status(),
        })
    }
}

impl PolicyComparison {
    fn start(recipe: &Recipe, policy: LearnedDecoderPolicy, limits: ComparisonLimits,
        lineage: ComparisonLineage) -> Result<Self, Error>
    {
        limits.check()?;
        // Reserve fresh work for BOTH arms before either starts. No early-stop
        // discount is assumed and no original run's spent budget is refunded.
        let positions = limits.positions.min(recipe.spec.prompt().len() + recipe.spec.max_new_tokens());
        let products = recipe.model.estimate(0, positions)?.scalar_products()?
            .checked_mul(2).ok_or(Error::Overflow)?;
        let samples = positions.saturating_sub(recipe.spec.prompt().len());
        let scores = (samples as u64).checked_mul(recipe.spec.sampling().policy.vocabulary() as u64)
            .and_then(|value| value.checked_mul(2)).ok_or(Error::Overflow)?;
        if products > limits.decoder_products || scores > limits.vocabulary_scores { return Err(Error::Limit); }
        // Use the original constructors and all their profile, roster and budget
        // admission checks; neither an unchecked cache nor a saved RNG is adopted.
        let baseline = recipe.start()?;
        let candidate = recipe.model.monitored_generation_with_telemetry(
            recipe.stream, recipe.evaluation_origin, recipe.spec.clone(), policy,
            recipe.budget, recipe.telemetry)?;
        Ok(Self { baseline, candidate, lineage, limits, status: ComparisonStatus::Active,
            attempted: 0, matched: 0, state_bytes_compared: 0, last: None })
    }

    pub fn status(&self) -> ComparisonStatus { self.status }
    pub fn position(&self) -> u64 { self.attempted as u64 }
    pub fn last_step(&self) -> Option<&ComparisonStep> { self.last.as_deref() }
    pub fn baseline_policy(&self) -> &LearnedDecoderPolicy { self.baseline.policy() }
    pub fn candidate_policy(&self) -> &LearnedDecoderPolicy { self.candidate.policy() }

    /// Only the common, verified prefix is visible. One arm may have accepted
    /// one more token at the terminal disagreement; that token remains hidden.
    pub fn matched_tokens(&self) -> &[u32] { &self.baseline.accepted_tokens()[..self.matched] }

    pub fn report(&self) -> ComparisonReport {
        ComparisonReport { lineage: self.lineage, status: self.status,
            attempted_positions: self.attempted, matched_positions: self.matched,
            state_bytes_compared: self.state_bytes_compared,
            work: ComparisonWork {
                baseline: self.baseline.work(), candidate: self.candidate.work(),
                baseline_telemetry: self.baseline.telemetry_work(),
                candidate_telemetry: self.candidate.telemetry_work(),
                all_attempted_telemetry_reported: self.baseline.work().admitted_tokens == self.attempted as u64
                    && self.candidate.work().admitted_tokens == self.attempted as u64
                    && !matches!(self.baseline.status(), GenerationStatus::Failed(_))
                    && !matches!(self.candidate.status(), GenerationStatus::Failed(_)),
            },
        }
    }

    /// Attempt exactly one position in each arm. Stale calls have no effect;
    /// errors and caught unwinds latch the experiment before another attempt.
    /// Both ordinary Result outcomes are retained, even when one arm fails.
    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<ComparisonStep>, Error> {
        if self.status != ComparisonStatus::Active { return Err(Error::WrongState); }
        if expected_position != self.position() { return Err(Error::Stale); }
        if self.attempted == self.limits.positions {
            self.status = ComparisonStatus::Exhausted;
            return Err(Error::Limit);
        }
        self.status = ComparisonStatus::Failed(Error::Incomplete);
        self.attempted += 1;
        let baseline = self.baseline.advance(expected_position);
        let candidate = self.candidate.advance(expected_position);
        let mut step = ComparisonStep { position: expected_position, baseline, candidate, jointly_accepted: false };
        let result = self.classify(&mut step);
        let step = Rc::new(step);
        self.last = Some(Rc::clone(&step));
        match result {
            Ok(status) => { self.status = status; Ok(step) }
            Err(error) => { self.status = ComparisonStatus::Failed(error); Err(error) }
        }
    }

    fn classify(&mut self, step: &mut ComparisonStep) -> Result<ComparisonStatus, Error> {
        let baseline = step.baseline.as_ref().map_err(|error| *error)?;
        let candidate = step.candidate.as_ref().map_err(|error| *error)?;
        let left = baseline.accepted().is_some();
        let right = candidate.accepted().is_some();
        if left != right {
            return Ok(ComparisonStatus::DecisionDifference { position: step.position });
        }
        if !left {
            if !matches!(baseline.status(), GenerationStatus::Held(_))
                || !matches!(candidate.status(), GenerationStatus::Held(_)) { return Err(Error::Binding); }
            return Ok(ComparisonStatus::BothHeld { position: step.position });
        }
        if !baseline.audit().complete_quiet() || !candidate.audit().complete_quiet() {
            return Err(Error::Binding);
        }
        // Reuse the original exact checkpoint capture, not a second numerical
        // algorithm. Telemetry is deliberately NOT an equality requirement:
        // policy-dependent compression/refinement costs are the experiment.
        let limits = CheckpointLimits { positions: self.limits.positions, state_bytes: self.limits.state_bytes };
        let a = State::capture(&self.baseline, limits)?;
        let b = State::capture(&self.candidate, limits)?;
        if !same_numerical_state(&a, &b) { return Err(Error::Binding); }
        let bytes = u64::try_from(a.logical_bytes).map_err(|_| Error::Overflow)?
            .checked_add(u64::try_from(b.logical_bytes).map_err(|_| Error::Overflow)?).ok_or(Error::Overflow)?;
        self.state_bytes_compared = self.state_bytes_compared.checked_add(bytes).ok_or(Error::Overflow)?;
        self.matched += 1;
        step.jointly_accepted = true;
        match a.status {
            GenerationStatus::Prefilling | GenerationStatus::Generating => Ok(ComparisonStatus::Active),
            GenerationStatus::Finished(stop) => Ok(ComparisonStatus::MatchedStop(stop)),
            _ => Err(Error::Binding),
        }
    }

    pub fn run_to_stop(&mut self) -> Result<ComparisonStatus, Error> {
        while self.status == ComparisonStatus::Active { self.advance(self.position())?; }
        match self.status {
            ComparisonStatus::Failed(error) => Err(error),
            ComparisonStatus::Exhausted => Err(Error::Limit),
            status => Ok(status),
        }
    }
}

fn same_numerical_state(a: &State, b: &State) -> bool {
    a.status == b.status && a.work == b.work && a.tokens == b.tokens
        && a.sampler == b.sampler && a.logits == b.logits && a.cache == b.cache
        && a.logical_bytes == b.logical_bytes && a.samples.len() == b.samples.len()
        && a.samples.iter().zip(&b.samples).all(|(left, right)| same_sample(left, right))
}
