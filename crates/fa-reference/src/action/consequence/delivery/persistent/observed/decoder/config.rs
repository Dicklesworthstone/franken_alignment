//! Frozen inputs to the ORIGINAL SafeTensors and monitored-sampler constructors.
//! Parameter bytes and numeric-platform compatibility remain operator trust roots.
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::monitor::decoder::config::{MonitorConfigError, MAX_MONITOR_CONFIG_BYTES};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder,
    config::{SamplingConfigError, MAX_SAMPLING_CONFIG_BYTES}};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::{WeightError, MAX_WEIGHT_FILE_BYTES};
use crate::action::consequence::oversight::decoder_monitoring::{DecoderBindingLimits,
    MAX_BOUND_DECODER_TOKENS, MAX_BOUND_DECODER_SCORE_WORDS};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// Exact immutable bootstrap data. Equality includes all input bytes, not just
/// supplied model names. This is not a signed manifest or authority to publish.
#[derive(Clone, PartialEq, Eq)]
pub struct FileDecoderConfig {
    profile: DecoderProfile,
    weights: Rc<[u8]>,
    monitor: Rc<[u8]>,
    sampling: Rc<[u8]>,
    stream: u64,
    pub(in super::super) limits: DecoderBindingLimits,
}
impl fmt::Debug for FileDecoderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDecoderConfig").field("profile", &self.profile)
            .field("stream", &self.stream).field("input_bytes", &self.input_bytes()).finish_non_exhaustive()
    }
}
impl FileDecoderConfig {
    pub fn new(profile: DecoderProfile, weights: Vec<u8>, monitor: Vec<u8>,
        sampling: Vec<u8>, stream: u64, limits: DecoderBindingLimits) -> Result<Self, Error>
    {
        let config = Self { profile, weights: weights.into(), monitor: monitor.into(),
            sampling: sampling.into(), stream, limits };
        config.check_bounds()?;
        // Validate the actual native constructors before handing out configuration.
        // No token is computed and no active numerical session is imported.
        config.build()?;
        Ok(config)
    }
    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn input_bytes(&self) -> usize { self.weights.len() + self.monitor.len() + self.sampling.len() }
    fn check_bounds(&self) -> Result<(), Error> {
        if self.stream == 0 || self.limits.token_ids == 0 || self.limits.score_words == 0 { return Err(Error::InvalidInput); }
        if self.weights.len() > MAX_WEIGHT_FILE_BYTES || self.monitor.len() > MAX_MONITOR_CONFIG_BYTES
            || self.sampling.len() > MAX_SAMPLING_CONFIG_BYTES
            || self.limits.token_ids > MAX_BOUND_DECODER_TOKENS || self.limits.score_words > MAX_BOUND_DECODER_SCORE_WORDS
        { return Err(Error::Limit); }
        Ok(())
    }
    pub(in super::super) fn build(&self) -> Result<MonitoredSampledDecoder, Error> {
        self.check_bounds()?;
        let (model, _) = DecoderModel::from_safetensors(self.profile.clone(), &self.weights)
            .map_err(|e| match e { WeightError::Limit => Error::Limit, WeightError::Model(e) => e, _ => Error::InvalidInput })?;
        MonitoredSampledDecoder::from_json(model, self.stream, &self.monitor, &self.sampling)
            .map_err(|e| match e {
                SamplingConfigError::Limit | SamplingConfigError::Monitor(MonitorConfigError::Limit) => Error::Limit,
                SamplingConfigError::Sampling(e) | SamplingConfigError::Monitor(MonitorConfigError::Monitor(e)) => e,
                _ => Error::InvalidInput,
            })
    }
    pub(super) fn write(&self, w: &mut Writer) -> Result<(), Error> {
        self.check_bounds()?;
        let id = self.profile.identity(); let s = self.profile.shape();
        for value in [id.tenant, id.model, id.model_generation, id.tokenizer_generation,
            id.profile_generation, self.profile.epsilon().to_bits(), self.profile.theta().to_bits(), self.stream] { w.u64(value)?; }
        for value in [s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads,
            s.cache_heads, s.context, self.limits.token_ids, self.limits.score_words] { w.count(value)?; }
        w.blob(&self.weights)?; w.blob(&self.monitor)?; w.blob(&self.sampling)
    }
    pub(super) fn read(r: &mut Reader<'_>) -> Result<Self, Error> {
        let identity = DecoderIdentity { tenant: r.u64()?, model: r.u64()?, model_generation: r.u64()?,
            tokenizer_generation: r.u64()?, profile_generation: r.u64()? };
        let epsilon = f64::from_bits(r.u64()?); let theta = f64::from_bits(r.u64()?); let stream = r.u64()?;
        let shape = DecoderShape { vocabulary: r.count(65_536)?, hidden: r.count(2_048)?, intermediate: r.count(8_192)?,
            layers: r.count(128)?, query_heads: r.count(2_048)?, cache_heads: r.count(2_048)?, context: r.count(1_048_576)? };
        let profile = DecoderProfile::new(identity, shape, epsilon, theta)?;
        let limits = DecoderBindingLimits { token_ids: r.count(MAX_BOUND_DECODER_TOKENS)?, score_words: r.count(MAX_BOUND_DECODER_SCORE_WORDS)? };
        let config = Self { profile, limits, stream, weights: Rc::from(r.blob(MAX_WEIGHT_FILE_BYTES)?),
            monitor: Rc::from(r.blob(MAX_MONITOR_CONFIG_BYTES)?), sampling: Rc::from(r.blob(MAX_SAMPLING_CONFIG_BYTES)?) };
        config.check_bounds()?;
        // Semantic replay calls the same native build; decoding does not retain a
        // second model or call unchecked constructors to import a running session.
        Ok(config)
    }
}
