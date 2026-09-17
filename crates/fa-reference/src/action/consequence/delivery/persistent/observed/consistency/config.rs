//! Exact, bounded configuration for the ORIGINAL predictor and likelihood gate.
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::{CaptureProfile, MAX_VALUES};
use crate::action::consequence::activation::consistency::{
    BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration, MAX_EVENT_PREFIX_BYTES, MAX_SAMPLES,
};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::oversight::consistency::ConsistencyConfig;
use crate::Error;
use std::rc::Rc;

pub(super) const MAX_CONFIG_BYTES: usize = MAX_VALUES * 4 + MAX_EVENT_PREFIX_BYTES + 256;
const DOMAIN: &[u8; 8] = b"FACPRED\x01";

/// Trusted bootstrap inputs, not inferred calibration or an actor-supplied score.
/// Floating-point coefficients are retained by their exact binary32 words.
#[derive(Clone, Debug)]
pub struct FileConsistencyParameters {
    pub probe_id: u64,
    pub probe_generation: u64,
    pub profile: CaptureProfile,
    pub weights: Vec<f32>,
    pub bias: f32,
    pub threshold: f32,
    pub forecast: ForecastRegistration,
    pub alpha: ErrorBudget,
    pub stream: u64,
    pub max_predictions: usize,
    pub max_prediction_age_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConsistencyConfig { bytes: Rc<[u8]> }
impl FileConsistencyConfig {
    pub fn new(p: FileConsistencyParameters) -> Result<Self, Error> {
        if p.weights.len() > MAX_VALUES || p.forecast.event_prefix.len() > MAX_EVENT_PREFIX_BYTES {
            return Err(Error::Limit);
        }
        let mut w = Writer::new(MAX_CONFIG_BYTES); w.raw(DOMAIN)?;
        for n in [p.probe_id, p.probe_generation, p.profile.tenant, p.profile.model,
            p.profile.model_generation, p.profile.tap, p.profile.layout_generation] { w.u64(n)?; }
        w.count(p.weights.len())?;
        for value in p.weights { w.u32(value.to_bits())?; }
        w.u32(p.bias.to_bits())?; w.u32(p.threshold.to_bits())?;
        for n in [p.forecast.domain, p.forecast.generation, p.forecast.policy_generation] { w.u64(n)?; }
        w.blob(&p.forecast.event_prefix)?;
        for pair in [p.forecast.negative, p.forecast.at_threshold, p.forecast.positive] {
            w.u32(pair.null_numerator())?; w.u32(pair.alternative_numerator())?;
        }
        w.u64(p.alpha.numerator())?; w.u64(p.alpha.denominator())?; w.u64(p.stream)?;
        w.count(p.max_predictions)?; w.u64(p.max_prediction_age_ticks)?;
        Self::from_bytes(&w.finish())
    }

    pub fn encoded(&self) -> &[u8] { &self.bytes }

    pub(in super::super) fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_CONFIG_BYTES { return Err(Error::Limit); }
        // Validate before copying the candidate configuration into retained data.
        decode(bytes)?;
        Ok(Self { bytes: Rc::from(bytes) })
    }
    pub(in super::super) fn build(&self) -> Result<ConsistencyConfig, Error> { decode(&self.bytes) }
}
fn decode(bytes: &[u8]) -> Result<ConsistencyConfig, Error> {
    let mut r = Reader::new(bytes);
    if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
    let id = r.u64()?; let generation = r.u64()?;
    let profile = CaptureProfile { tenant: r.u64()?, model: r.u64()?, model_generation: r.u64()?,
        tap: r.u64()?, layout_generation: r.u64()? };
    let count = r.count(MAX_VALUES)?;
    let raw = r.take(count.checked_mul(4).ok_or(Error::Limit)?)?;
    let mut weights = Vec::new(); weights.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for word in raw.chunks_exact(4) {
        weights.push(f32::from_bits(u32::from_be_bytes([word[0], word[1], word[2], word[3]])));
    }
    let probe = LinearProbe::new(id, generation, profile, &weights,
        f32::from_bits(r.u32()?), f32::from_bits(r.u32()?))?;
    let domain = r.u64()?; let generation = r.u64()?; let policy_generation = r.u64()?;
    let event_prefix = r.blob(MAX_EVENT_PREFIX_BYTES)?.to_vec();
    let negative = BinaryForecast::new(r.u32()?, r.u32()?)?;
    let at_threshold = BinaryForecast::new(r.u32()?, r.u32()?)?;
    let positive = BinaryForecast::new(r.u32()?, r.u32()?)?;
    let model = ForecastModel::new(probe, ForecastRegistration {
        domain, generation, policy_generation, event_prefix, negative, at_threshold, positive,
    })?;
    let alpha = ErrorBudget::new(r.u64()?, r.u64()?)?;
    let stream = r.u64()?; let max_predictions = r.count(MAX_SAMPLES)?;
    let max_prediction_age_ticks = r.u64()?; r.end()?;
    if stream == 0 || max_predictions == 0 || max_prediction_age_ticks == 0 { return Err(Error::InvalidInput); }
    Ok(ConsistencyConfig { model, alpha, stream, max_predictions, max_prediction_age_ticks })
}
