//! Exact comparison material for durable numerical replay. Never decoded into
//! a live decoder, cache, monitor judgment, sampler or effect capability.
use super::MonitoredSampledDecoder;
use super::super::super::{DecoderReview, MonitoringStatus};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::probe::{ExactScore, ProbeIdentity, ProbeOutcome};
use crate::Error;
use std::cmp::Ordering;

pub(crate) const MAX_REPLAY_BYTES: usize = 16 * 1024 * 1024;

impl MonitoredSampledDecoder {
    /// Called only by the trusted host. Includes held state, which must never be
    /// exposed as an ordinary released token or accepted from a saved image.
    pub(crate) fn replay_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut w = ReplayWriter(Vec::new());
        w.raw(b"FANREP\0\x01")?;
        let p = self.profile(); let id = p.identity(); let s = p.shape();
        for value in [id.tenant, id.model, id.model_generation, id.tokenizer_generation,
            id.profile_generation, p.epsilon().to_bits(), p.theta().to_bits()] { w.u64(value)?; }
        for value in [s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads,
            s.cache_heads, s.context] { w.count(value)?; }
        w.u64(self.monitored.generation)?; w.u64(self.monitored.stream)?;
        w.count(self.monitored.budget.encoded_bytes)?;
        w.count(self.monitored.budget.probe_coordinates)?;
        match self.status() {
            MonitoringStatus::Ready => w.u8(0)?,
            MonitoringStatus::Held => w.u8(1)?,
            MonitoringStatus::Failed(error) => { w.u8(2)?; w.u8(error_tag(error))?; }
        }
        let tokens = self.monitored.session.tokens(); w.count(tokens.len())?;
        for token in tokens { w.raw(&token.to_be_bytes())?; }
        w.blob(&self.monitored.session.cache_image()?.encode()?)?;
        w.blob(&self.sampler.snapshot().encode())?;
        if tokens.is_empty() { w.u8(0)?; } else {
            w.u8(1)?;
            let logits = self.monitored.session.logits()?; w.count(logits.len())?;
            for value in logits { w.raw(&value.to_bits().to_be_bytes())?; }
        }
        let work = self.decoder_work();
        for value in [work.tokens, work.matrix_products, work.attention_products,
            work.attention_exponentials, work.normalization_coordinates, work.rotary_pairs,
            work.gate_coordinates, work.cache_values_appended] { w.u64(value)?; }
        let work = self.monitoring_work(); w.u64(work.frame_reviews)?;
        for value in [work.encoded_bytes, work.probe_coordinates, work.codec_coordinates] { w.count(value)?; }
        match self.last_review() {
            None => w.u8(0)?,
            Some(review) => { w.u8(1)?; w.review(review)?; }
        }
        Ok(w.0)
    }
}

/// Reuse the exact original review encoding for each generation step. This is
/// comparison-only output; there is no decoder for importing a stored verdict.
pub(crate) fn review_bytes(review: &DecoderReview) -> Result<Vec<u8>, Error> {
    let mut w = ReplayWriter(Vec::new());
    w.review(review)?;
    Ok(w.0)
}

pub(crate) fn error_tag(error: Error) -> u8 {
    match error {
        Error::InvalidInput => 0, Error::Incomplete => 1, Error::Limit => 2,
        Error::Overflow => 3, Error::Duplicate => 4, Error::Missing => 5,
        Error::WrongState => 6, Error::Stale => 7, Error::Binding => 8,
    }
}

struct ReplayWriter(Vec<u8>);
impl ReplayWriter {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.0.len().checked_add(bytes.len()).ok_or(Error::Limit)? > MAX_REPLAY_BYTES { return Err(Error::Limit); }
        self.0.try_reserve(bytes.len()).map_err(|_| Error::Limit)?;
        self.0.extend_from_slice(bytes); Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<(), Error> { self.raw(&[value]) }
    fn u64(&mut self, value: u64) -> Result<(), Error> { self.raw(&value.to_be_bytes()) }
    fn count(&mut self, value: usize) -> Result<(), Error> { self.u64(u64::try_from(value).map_err(|_| Error::Limit)?) }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), Error> { self.count(bytes.len())?; self.raw(bytes) }
    fn profile(&mut self, p: CaptureProfile) -> Result<(), Error> {
        for value in [p.tenant, p.model, p.model_generation, p.tap, p.layout_generation] { self.u64(value)?; } Ok(())
    }
    fn frame(&mut self, f: FrameIdentity) -> Result<(), Error> {
        self.profile(f.profile)?;
        for value in [f.stream, f.sequence, f.position] { self.u64(value)?; } Ok(())
    }
    fn probe(&mut self, p: ProbeIdentity) -> Result<(), Error> {
        self.u64(p.id)?; self.u64(p.generation)?; self.profile(p.profile)?; self.count(p.dimensions)
    }
    fn outcome(&mut self, outcome: MonitorOutcome) -> Result<(), Error> {
        self.u8(match outcome { MonitorOutcome::NoAlarm => 0, MonitorOutcome::Alarm => 1,
            MonitorOutcome::AtThreshold => 2, MonitorOutcome::BudgetExhausted => 3, MonitorOutcome::Unresolved => 4 })
    }
    fn score(&mut self, score: &ExactScore) -> Result<(), Error> {
        self.u8(match score.sign() { Ordering::Less => 0, Ordering::Equal => 1, Ordering::Greater => 2 })?;
        for word in score.magnitude_words() { self.u64(*word)?; } Ok(())
    }
    fn review(&mut self, review: &DecoderReview) -> Result<(), Error> {
        self.u64(review.generation())?; self.u64(review.stream())?; self.u64(review.position())?;
        self.count(review.required_layers())?; self.outcome(review.outcome())?;
        self.count(review.layers().len())?;
        for layer in review.layers() {
            self.u64(layer.layer)?; let r = &layer.report; self.frame(r.frame())?;
            self.count(r.probes().len())?;
            for probe in r.probes() { self.probe(*probe)?; }
            self.outcome(r.outcome())?;
            for count in [r.raw_bytes(), r.encoded_bytes(), r.probe_coordinates(), r.codec_coordinates()] { self.count(count)?; }
            self.count(r.steps().len())?;
            for step in r.steps() {
                self.u8(step.mantissa_bits)?; self.count(step.encoded_bytes)?; self.count(step.observations.len())?;
                for o in &step.observations {
                    self.frame(o.frame())?; self.probe(o.probe())?; self.u8(o.mantissa_bits())?;
                    self.score(&o.interval().lower)?; self.score(&o.interval().upper)?;
                    self.u8(match o.outcome() { ProbeOutcome::CertifiedAlarm => 0, ProbeOutcome::CertifiedQuiet => 1,
                        ProbeOutcome::NeedsRefinement => 2, ProbeOutcome::AtThreshold => 3 })?;
                }
            }
        }
        Ok(())
    }
}
