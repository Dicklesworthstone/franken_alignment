//! Live evidence from an ORIGINAL learned-K/V generation, not imported verdicts.
//! This owner is a prerequisite source for the effect gate, never an authority.
pub(crate) mod host;

use crate::action::consequence::activation::monitor::learned::model::LearnedModelReport;
use crate::action::consequence::activation::probe::SCORE_WORDS;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderModel, DecoderProfile};
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::LearnedDecoderPolicy;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationEvent, GenerationSpec, GenerationStatus, GenerationTelemetryBudget,
    GenerationTelemetryWork, GenerationWork, LearnedGeneration, MAX_GENERATION_TOKENS,
};
use crate::Error;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

pub const MAX_LEARNED_EVIDENCE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_LEARNED_EVIDENCE_SCORE_WORDS: usize = 1_048_576;

/// Original constructor inputs, frozen before any computation. Declared numeric
/// identities are not provider authentication; the owner ties actual objects.
#[derive(Clone, Debug)]
pub struct LearnedSourceConfig {
    pub stream: u64,
    pub evaluation_origin: u64,
    pub monitor_generation: u64,
    pub spec: GenerationSpec,
    pub policy: LearnedDecoderPolicy,
    pub budget: GenerationBudget,
    pub telemetry: GenerationTelemetryBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedAvailability { Empty, InProgress, Ready, Held, Failed, Closed }

/// Logical retained token IDs, exact interval limbs and encoded checked source.
/// Not allocator overhead, monitor coefficients, retained views or peak RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedEvidenceLimits {
    pub token_ids: usize,
    pub score_words: usize,
    pub encoded_bytes: usize,
}
impl Default for LearnedEvidenceLimits {
    fn default() -> Self {
        Self { token_ids: MAX_GENERATION_TOKENS, score_words: MAX_LEARNED_EVIDENCE_SCORE_WORDS,
            encoded_bytes: MAX_LEARNED_EVIDENCE_BYTES }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LearnedEvidenceCost {
    pub token_ids: usize,
    pub score_words: usize,
    pub encoded_bytes: usize,
}
#[derive(Debug)]
struct Prefix { tokens: Vec<u32>, audit: Option<Rc<LearnedModelReport>> }
#[derive(Debug)]
struct Shared {
    profile: DecoderProfile,
    stream: u64,
    evaluation_origin: u64,
    generation: u64,
    availability: Cell<LearnedAvailability>,
    prefix: RefCell<Prefix>,
}

/// A newly constructed original generator is owned from position zero. There is
/// no adoption of advanced, restarted or experimentally edited numerical state.
/// All public computation delegates to the unchanged LearnedGeneration methods.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_source::ObservedLearnedGeneration;
/// fn bypass(run: &mut ObservedLearnedGeneration) { run.generation_mut(); }
/// ```
pub struct ObservedLearnedGeneration { run: LearnedGeneration, shared: Rc<Shared> }
impl fmt::Debug for ObservedLearnedGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservedLearnedGeneration").field("position", &self.position())
            .field("status", &self.status()).finish_non_exhaustive()
    }
}
impl Drop for ObservedLearnedGeneration {
    fn drop(&mut self) { self.shared.availability.set(LearnedAvailability::Closed); }
}
impl DecoderModel {
    pub fn observed_learned_generation(&self, config: LearnedSourceConfig)
        -> Result<ObservedLearnedGeneration, Error>
    {
        if config.monitor_generation == 0 { return Err(Error::InvalidInput); }
        let run = self.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
            config.spec, config.policy, config.budget, config.telemetry)?;
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(run.estimate().audited_positions).map_err(|_| Error::Limit)?;
        let shared = Rc::new(Shared { profile: self.profile().clone(), stream: config.stream,
            evaluation_origin: config.evaluation_origin, generation: config.monitor_generation,
            availability: Cell::new(LearnedAvailability::Empty),
            prefix: RefCell::new(Prefix { tokens, audit: None }) });
        Ok(ObservedLearnedGeneration { run, shared })
    }
}
impl ObservedLearnedGeneration {
    pub fn observation(&self) -> LearnedObservation { LearnedObservation { shared: Rc::clone(&self.shared) } }
    pub fn position(&self) -> u64 { self.run.position() }
    pub fn status(&self) -> GenerationStatus { self.run.status() }
    pub fn work(&self) -> GenerationWork { self.run.work() }
    pub fn telemetry_work(&self) -> GenerationTelemetryWork { self.run.telemetry_work() }
    pub fn accepted_tokens(&self) -> &[u32] { self.run.accepted_tokens() }

    /// Stale calls are free. Once actual work starts, a caught unwind or error
    /// cannot leave an older quiet observation eligible at the effect gate.
    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<GenerationEvent>, Error> {
        if !self.run.status().is_active() || !matches!(self.shared.availability.get(),
            LearnedAvailability::Empty | LearnedAvailability::Ready) { return Err(Error::WrongState); }
        if expected_position != self.run.position() { return Err(Error::Stale); }
        self.shared.availability.set(LearnedAvailability::InProgress);
        match self.advance_inner(expected_position) {
            Ok(event) => Ok(event),
            Err(error) => { self.shared.availability.set(LearnedAvailability::Failed); Err(error) }
        }
    }
    fn advance_inner(&mut self, position: u64) -> Result<Rc<GenerationEvent>, Error> {
        let event = self.run.advance(position)?;
        if event.position() != position { return Err(Error::Binding); }
        let Some(step) = event.accepted() else {
            if !matches!(event.status(), GenerationStatus::Held(_)) { return Err(Error::Binding); }
            self.shared.availability.set(LearnedAvailability::Held);
            return Ok(event);
        };
        let audit = event.audit();
        let next = position.checked_add(1).ok_or(Error::Overflow)?;
        if !audit.complete_quiet() || audit.first_position() != position || audit.end_position() != next
            || audit.planned_rows() != self.shared.profile.shape().layers * 2
            || audit.source().descriptor().profile() != self.run.policy().codec().profile()
            || audit.source().descriptor().layers().values().any(|layer| {
                layer.stream != self.shared.stream || layer.first_position != position
                    || layer.first_sequence != next || layer.token_count != 1
            }) { return Err(Error::Binding); }
        let mut prefix = self.shared.prefix.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if prefix.tokens.len() as u64 != position || self.run.position() != next
            || self.run.accepted_tokens().get(..prefix.tokens.len()) != Some(prefix.tokens.as_slice())
            || self.run.accepted_tokens().last() != Some(&step.token) { return Err(Error::Binding); }
        // Allocate/copy evidence while unavailable; only a complete publication
        // below sets Ready. Every earlier position was accepted by this owner.
        let audit = Rc::new(audit.clone());
        prefix.tokens.push(step.token);
        prefix.audit = Some(audit);
        self.shared.availability.set(LearnedAvailability::Ready);
        Ok(event)
    }
    pub fn run_to_stop(&mut self) -> Result<GenerationStatus, Error> {
        while self.run.status().is_active() { self.advance(self.position())?; }
        match self.run.status() { GenerationStatus::Failed(error) => Err(error), status => Ok(status) }
    }
}

/// Read-only liveness handle. Matching numbers from another original generator
/// are insufficient: capture validation also checks the exact process-local owner.
#[derive(Clone, Debug)]
pub struct LearnedObservation { shared: Rc<Shared> }
impl LearnedObservation {
    pub fn profile(&self) -> &DecoderProfile { &self.shared.profile }
    pub fn stream(&self) -> u64 { self.shared.stream }
    pub fn generation(&self) -> u64 { self.shared.generation }
    pub fn evaluation_origin(&self) -> u64 { self.shared.evaluation_origin }
    pub fn availability(&self) -> LearnedAvailability { self.shared.availability.get() }
    pub fn capture(&self, limits: LearnedEvidenceLimits) -> Result<LearnedEvidence, Error> {
        if limits.token_ids > MAX_GENERATION_TOKENS || limits.score_words > MAX_LEARNED_EVIDENCE_SCORE_WORDS
            || limits.encoded_bytes > MAX_LEARNED_EVIDENCE_BYTES { return Err(Error::Limit); }
        if self.availability() != LearnedAvailability::Ready { return Err(Error::Incomplete); }
        let prefix = self.shared.prefix.try_borrow().map_err(|_| Error::Incomplete)?;
        let audit = prefix.audit.as_ref().ok_or(Error::Incomplete)?;
        let mut score_words = 0_usize;
        for row in audit.rows() {
            for step in row.steps() {
                score_words = score_words.checked_add(step.observations.len()
                    .checked_mul(2 * SCORE_WORDS).ok_or(Error::Limit)?).ok_or(Error::Limit)?;
            }
        }
        let cost = LearnedEvidenceCost { token_ids: prefix.tokens.len(), score_words,
            encoded_bytes: audit.source().report().total_encoded_bytes };
        if cost.token_ids > limits.token_ids || cost.score_words > limits.score_words
            || cost.encoded_bytes > limits.encoded_bytes { return Err(Error::Limit); }
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(cost.token_ids).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(&prefix.tokens);
        Ok(LearnedEvidence { shared: Rc::clone(&self.shared), tokens: tokens.into(),
            audit: Rc::clone(audit), cost })
    }
    pub fn validate(&self, evidence: &LearnedEvidence) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.shared, &evidence.shared) { return Err(Error::Binding); }
        if self.availability() != LearnedAvailability::Ready { return Err(Error::Incomplete); }
        let prefix = self.shared.prefix.try_borrow().map_err(|_| Error::Incomplete)?;
        if prefix.tokens.as_slice() != evidence.tokens.as_ref()
            || !prefix.audit.as_ref().is_some_and(|audit| Rc::ptr_eq(audit, &evidence.audit)) {
            return Err(Error::Stale);
        }
        Ok(())
    }
}

/// The latest full K/V audit plus an owner-attested wholly accepted prefix.
/// This does NOT retain every historical audit or authenticate an external host.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::oversight::learned_source::LearnedEvidence};
/// fn grant(evidence: LearnedEvidence) -> Permit { evidence }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedEvidence {
    shared: Rc<Shared>,
    tokens: Rc<[u32]>,
    audit: Rc<LearnedModelReport>,
    cost: LearnedEvidenceCost,
}
impl LearnedEvidence {
    pub fn profile(&self) -> &DecoderProfile { &self.shared.profile }
    pub fn stream(&self) -> u64 { self.shared.stream }
    pub fn generation(&self) -> u64 { self.shared.generation }
    pub fn evaluation_origin(&self) -> u64 { self.shared.evaluation_origin }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn next_position(&self) -> u64 { self.tokens.len() as u64 }
    pub fn audit(&self) -> &LearnedModelReport { &self.audit }
    pub fn cost(&self) -> LearnedEvidenceCost { self.cost }
}
