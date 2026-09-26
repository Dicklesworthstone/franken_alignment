//! Bind effect admission to one live compulsory-monitoring numerical owner.
//! This is an additional prerequisite, never another authority or congress.

use super::OversightBroker;
use crate::action::consequence::activation::monitor::decoder::observation::{DecoderEvidence, DecoderObservation};
use crate::action::consequence::activation::probe::SCORE_WORDS;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderProfile;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::MAX_GENERATION_TOKENS;
use crate::action::consequence::oversight::learned_source::{
    LearnedEvidence, LearnedEvidenceLimits, LearnedObservation,
    MAX_LEARNED_EVIDENCE_BYTES, MAX_LEARNED_EVIDENCE_SCORE_WORDS,
};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_BOUND_DECODER_TOKENS: usize = 1_048_576;
pub const MAX_BOUND_DECODER_SCORE_WORDS: usize = 1_048_576;

/// Cumulative retained original IDs and exact probe-score words. This is not an
/// RSS/allocator bound. Reusing a report is conservatively counted per proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderBindingLimits { pub token_ids: usize, pub score_words: usize }
impl Default for DecoderBindingLimits {
    fn default() -> Self {
        Self { token_ids: MAX_BOUND_DECODER_TOKENS, score_words: MAX_BOUND_DECODER_SCORE_WORDS }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoderBindingUsage { pub token_ids: usize, pub score_words: usize }

/// The existing token/score limits plus a cumulative checked-source byte cap.
/// This does not include monitor coefficients, view metadata or peak RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedDecoderBindingLimits {
    pub evidence: DecoderBindingLimits,
    pub encoded_bytes: usize,
}
impl Default for LearnedDecoderBindingLimits {
    fn default() -> Self {
        Self { evidence: DecoderBindingLimits::default(), encoded_bytes: MAX_LEARNED_EVIDENCE_BYTES }
    }
}

#[derive(Debug)]
enum Source { Residual(DecoderObservation), Learned(LearnedObservation) }
impl Source {
    fn profile(&self) -> &DecoderProfile {
        match self { Self::Residual(source) => source.profile(), Self::Learned(source) => source.profile() }
    }
    fn validate(&self, evidence: &Evidence) -> Result<(), Error> {
        match (self, evidence) {
            (Self::Residual(source), Evidence::Residual(evidence)) => source.validate(evidence),
            (Self::Learned(source), Evidence::Learned(evidence)) => source.validate(evidence),
            _ => Err(Error::Binding),
        }
    }
}
#[derive(Debug)]
enum Evidence { Residual(DecoderEvidence), Learned(LearnedEvidence) }
impl Evidence {
    fn profile(&self) -> &DecoderProfile {
        match self { Self::Residual(e) => e.profile(), Self::Learned(e) => e.profile() }
    }
    fn tokens(&self) -> &[u32] {
        match self { Self::Residual(e) => e.tokens(), Self::Learned(e) => e.tokens() }
    }
    fn next_position(&self) -> u64 {
        match self { Self::Residual(e) => e.next_position(), Self::Learned(e) => e.next_position() }
    }
}

#[derive(Debug)]
pub(super) struct CapturedDecoder {
    evidence: Evidence,
    actor_revision: u64,
    policy_epoch: u64,
    cost: DecoderBindingUsage,
    encoded_bytes: usize,
}
#[derive(Debug)]
pub(super) struct DecoderGate {
    source: Source,
    encoded_limit: usize,
    encoded_used: usize,
    limits: DecoderBindingLimits,
    usage: DecoderBindingUsage,
    records: BTreeMap<u64, CapturedDecoder>,
}

impl DecoderGate {
    /// Private paired-reset preparation, never a public source replacement API.
    pub(super) fn validate_successor(&self, source: &DecoderObservation) -> Result<(), Error> {
        let Source::Residual(previous) = &self.source else { return Err(Error::Binding); };
        if source.profile() != previous.profile() || source.generation() != previous.generation()
            || source.stream() <= previous.stream() { return Err(Error::Binding); }
        source.capture()?;
        Ok(())
    }
    /// Only the owning hosted-reset path calls this after the ORIGINAL authority
    /// reset and its host synchronization. Historical records/costs stay intact.
    pub(super) fn publish_successor(&mut self, source: DecoderObservation) {
        assert!(matches!(&self.source, Source::Residual(_)), "validated residual successor");
        self.source = Source::Residual(source);
    }
}

impl OversightBroker {
    /// Trusted bootstrap only, before any proposal/review/control transition.
    /// Freeze this exact source, model profile and monitor generation. Empty
    /// sources can be installed before input inference, but cannot admit work.
    /// There is no public replacement-source, raw-report or restored-key path.
    pub fn enable_decoder_monitoring(
        &mut self, source: DecoderObservation, limits: DecoderBindingLimits,
    ) -> Result<(), Error> {
        self.install_decoder_source(Source::Residual(source), limits, 0)
    }

    /// Freeze a live original learned-K/V source at the SAME effect gate. This
    /// does not own the actor or grant any approval. It has no replacement API,
    /// fallback to residual-only evidence, or private hosted-reset conversion.
    pub fn enable_learned_decoder_monitoring(&mut self, source: LearnedObservation,
        limits: LearnedDecoderBindingLimits) -> Result<(), Error>
    {
        if limits.encoded_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.encoded_bytes > MAX_LEARNED_EVIDENCE_BYTES { return Err(Error::Limit); }
        self.install_decoder_source(Source::Learned(source), limits.evidence, limits.encoded_bytes)
    }

    fn install_decoder_source(&mut self, source: Source, limits: DecoderBindingLimits,
        encoded_limit: usize) -> Result<(), Error>
    {
        if self.decoder.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if limits.token_ids == 0 || limits.score_words == 0 { return Err(Error::InvalidInput); }
        if limits.token_ids > MAX_BOUND_DECODER_TOKENS || limits.score_words > MAX_BOUND_DECODER_SCORE_WORDS {
            return Err(Error::Limit);
        }
        let identity = source.profile().identity();
        let actor = self.delivery.controller().actor().profile();
        if identity.tenant != self.scope.tenant || identity.model_generation != actor.model_generation
            || identity.tokenizer_generation != actor.tokenizer_generation
        { return Err(Error::Binding); }
        self.decoder = Some(DecoderGate { source, limits, encoded_limit, encoded_used: 0,
            usage: DecoderBindingUsage::default(), records: BTreeMap::new() });
        Ok(())
    }

    pub fn decoder_monitoring_required(&self) -> bool { self.decoder.is_some() }
    pub fn decoder_binding_usage(&self) -> Option<DecoderBindingUsage> {
        self.decoder.as_ref().map(|gate| gate.usage)
    }

    /// Historical frozen proposal basis, NOT a claim of present eligibility or
    /// of dispatch. Changing source/input state cannot rewrite this observation.
    pub fn decoder_evidence(&self, attempt: u64) -> Result<Option<&DecoderEvidence>, Error> {
        self.inputs.get(&attempt).ok_or(Error::Missing)?;
        Ok(self.decoder.as_ref().and_then(|gate| gate.records.get(&attempt)).and_then(|record| {
            match &record.evidence { Evidence::Residual(evidence) => Some(evidence), Evidence::Learned(_) => None }
        }))
    }

    /// The original delivery record must exist before calling a basis consumed.
    /// Reconciliation remains independent of whether this source is still live.
    pub fn dispatched_decoder_evidence(&self, attempt: u64) -> Result<Option<&DecoderEvidence>, Error> {
        self.delivery.status_query(attempt)?;
        self.decoder_evidence(attempt)
    }

    /// Historical learned basis is a separate type, never a fabricated residual
    /// review. Eligibility still requires validation against the live source.
    pub fn learned_decoder_evidence(&self, attempt: u64) -> Result<Option<&LearnedEvidence>, Error> {
        self.inputs.get(&attempt).ok_or(Error::Missing)?;
        Ok(self.decoder.as_ref().and_then(|gate| gate.records.get(&attempt)).and_then(|record| {
            match &record.evidence { Evidence::Learned(evidence) => Some(evidence), Evidence::Residual(_) => None }
        }))
    }
    pub fn dispatched_learned_decoder_evidence(&self, attempt: u64) -> Result<Option<&LearnedEvidence>, Error> {
        self.delivery.status_query(attempt)?;
        self.learned_decoder_evidence(attempt)
    }
    pub fn learned_decoder_binding_bytes(&self) -> Option<usize> {
        self.decoder.as_ref().and_then(|gate| match &gate.source {
            Source::Learned(_) => Some(gate.encoded_used), Source::Residual(_) => None,
        })
    }

    pub(super) fn prepare_decoder(&self) -> Result<Option<CapturedDecoder>, Error> {
        let Some(gate) = &self.decoder else { return Ok(None); };
        let (evidence, score_words, encoded_bytes) = match &gate.source {
            Source::Residual(source) => {
                let evidence = source.capture()?;
                let mut score_words = 0_usize;
                for layer in evidence.review().layers() {
                    for step in layer.report.steps() {
                        let words = step.observations.len().checked_mul(2 * SCORE_WORDS).ok_or(Error::Limit)?;
                        score_words = score_words.checked_add(words).ok_or(Error::Limit)?;
                    }
                }
                (Evidence::Residual(evidence), score_words, 0)
            }
            Source::Learned(source) => {
                let remaining = LearnedEvidenceLimits {
                    token_ids: gate.limits.token_ids.checked_sub(gate.usage.token_ids).ok_or(Error::Binding)?
                        .min(MAX_GENERATION_TOKENS),
                    score_words: gate.limits.score_words.checked_sub(gate.usage.score_words).ok_or(Error::Binding)?
                        .min(MAX_LEARNED_EVIDENCE_SCORE_WORDS),
                    encoded_bytes: gate.encoded_limit.checked_sub(gate.encoded_used).ok_or(Error::Binding)?,
                };
                let evidence = source.capture(remaining)?;
                let cost = evidence.cost();
                (Evidence::Learned(evidence), cost.score_words, cost.encoded_bytes)
            }
        };
        self.check_decoder_actor(&evidence)?;
        let cost = DecoderBindingUsage { token_ids: evidence.tokens().len(), score_words };
        let total = add(gate.usage, cost)?;
        if total.token_ids > gate.limits.token_ids || total.score_words > gate.limits.score_words {
            return Err(Error::Limit);
        }
        Ok(Some(CapturedDecoder { evidence, actor_revision: self.actor_revision(),
            policy_epoch: self.inspect().ledger.epoch, cost, encoded_bytes }))
    }

    /// Called only after the original ledger accepts a fresh proposal. No user
    /// callbacks can run between preparation and publication under &mut self.
    pub(super) fn publish_decoder(&mut self, attempt: u64, captured: Option<CapturedDecoder>) {
        if let Some(captured) = captured {
            let gate = self.decoder.as_mut().expect("prepared decoder gate");
            gate.usage = add(gate.usage, captured.cost).expect("preflighted retained evidence cost");
            gate.encoded_used = gate.encoded_used.checked_add(captured.encoded_bytes)
                .expect("preflighted checked-source retention");
            let previous = gate.records.insert(attempt, captured);
            debug_assert!(previous.is_none(), "original ledger admitted a fresh attempt");
        }
    }

    pub(super) fn check_decoder(&self, attempt: u64) -> Result<(), Error> {
        let Some(gate) = &self.decoder else { return Ok(()); };
        let record = gate.records.get(&attempt).ok_or(Error::Incomplete)?;
        gate.source.validate(&record.evidence)?;
        if record.actor_revision != self.actor_revision() || record.policy_epoch != self.inspect().ledger.epoch {
            return Err(Error::Stale);
        }
        self.check_decoder_actor(&record.evidence)
    }

    fn check_decoder_actor(&self, evidence: &Evidence) -> Result<(), Error> {
        let actor = self.delivery.controller().actor();
        let identity = evidence.profile().identity();
        if identity.tenant != self.scope.tenant || identity.model_generation != actor.profile().model_generation
            || identity.tokenizer_generation != actor.profile().tokenizer_generation
            || evidence.tokens() != actor.tokens() || evidence.next_position() != actor.next_position()
        { return Err(Error::Binding); }
        Ok(())
    }
}

fn add(left: DecoderBindingUsage, right: DecoderBindingUsage) -> Result<DecoderBindingUsage, Error> {
    Ok(DecoderBindingUsage {
        token_ids: left.token_ids.checked_add(right.token_ids).ok_or(Error::Limit)?,
        score_words: left.score_words.checked_add(right.score_words).ok_or(Error::Limit)?,
    })
}
