//! Frozen inputs to the ORIGINAL SafeTensors and monitored-sampler constructors.
//! Parameter bytes and numeric-platform compatibility remain operator trust roots.
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::monitor::decoder::config::{MonitorConfigError, MAX_MONITOR_CONFIG_BYTES};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder,
    config::{SamplingConfigError, MAX_SAMPLING_CONFIG_BYTES}};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::{OutputHead, WeightError, MAX_WEIGHT_FILE_BYTES};
use crate::action::consequence::oversight::decoder_monitoring::{DecoderBindingLimits,
    MAX_BOUND_DECODER_TOKENS, MAX_BOUND_DECODER_SCORE_WORDS};
use crate::Error;
use std::fmt;
use std::rc::Rc;

mod sharded;
pub use sharded::FileDecoderShardInputs;
use sharded::ShardSet;

/// Exact immutable bootstrap data. Equality includes all input bytes, not just
/// supplied model names. This is not a signed manifest or authority to publish.
#[derive(Clone, PartialEq, Eq)]
pub struct FileDecoderConfig {
    profile: DecoderProfile,
    output_head: OutputHead,
    weights: Rc<[u8]>,
    // Single-file bytes are empty in sharded mode; neither layout is inferred.
    shards: Option<Rc<ShardSet>>,
    monitor: Rc<[u8]>,
    sampling: Rc<[u8]>,
    stream: u64,
    pub(in super::super) limits: DecoderBindingLimits,
}
impl fmt::Debug for FileDecoderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDecoderConfig").field("profile", &self.profile)
            .field("output_head", &self.output_head).field("sharded", &self.is_sharded()).field("stream", &self.stream).field("input_bytes", &self.input_bytes()).finish_non_exhaustive()
    }
}
impl FileDecoderConfig {
    /// Legacy and default loading always require an independent lm_head.
    /// A missing head never implicitly selects shared embeddings.
    pub fn new(profile: DecoderProfile, weights: Vec<u8>, monitor: Vec<u8>,
        sampling: Vec<u8>, stream: u64, limits: DecoderBindingLimits) -> Result<Self, Error>
    {
        Self::new_with_output_head(profile, weights, monitor, sampling, stream, limits,
            OutputHead::Independent)
    }

    /// Freeze explicit output-head semantics with the original raw model bytes.
    /// TiedEmbeddings accepts an omitted head; a physically present head must
    /// have exactly the embedding matrix's normalized f32 bits. Validation and
    /// numerical execution remain in the original SafeTensors/decoder owners.
    /// Mode participates in exact recovery equality even when both modes would
    /// produce identical numbers from an archive containing equal matrices.
    pub fn new_with_output_head(profile: DecoderProfile, weights: Vec<u8>, monitor: Vec<u8>,
        sampling: Vec<u8>, stream: u64, limits: DecoderBindingLimits, output_head: OutputHead)
        -> Result<Self, Error>
    {
        let config = Self { profile, output_head, weights: weights.into(), shards: None, monitor: monitor.into(),
            sampling: sampling.into(), stream, limits };
        config.check_bounds()?;
        // Validate the actual native constructors before handing out configuration.
        // No token is computed and no active numerical session is imported.
        config.build()?;
        Ok(config)
    }
    /// Retain the exact index, literal shard labels and physical bytes as one
    /// immutable model input. The ORIGINAL sharded parser verifies assignments,
    /// tensor coverage and tied-head equality; no path is opened or inferred.
    /// Reconstruction still reruns the original decoder and checks its history.
    pub fn new_sharded(profile: DecoderProfile, inputs: FileDecoderShardInputs,
        monitor: Vec<u8>, sampling: Vec<u8>, stream: u64, limits: DecoderBindingLimits,
        output_head: OutputHead) -> Result<Self, Error>
    {
        let shards = ShardSet::new(&profile, output_head, inputs)?;
        let config = Self { profile, output_head, weights: Rc::from(&b""[..]),
            shards: Some(Rc::new(shards)), monitor: monitor.into(), sampling: sampling.into(), stream, limits };
        config.build()?;
        Ok(config)
    }
    /// Storage interpretation only, not model quality, freshness or authority.
    pub fn is_sharded(&self) -> bool { self.shards.is_some() }
    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn output_head(&self) -> OutputHead { self.output_head }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn input_bytes(&self) -> usize {
        self.shards.as_ref().map_or(self.weights.len(), |set| set.input_bytes())
            + self.monitor.len() + self.sampling.len()
    }
    fn check_bounds(&self) -> Result<(), Error> {
        if let Some(set) = &self.shards {
            if !self.weights.is_empty() { return Err(Error::Binding); }
            set.check_bounds()?;
        }
        if self.stream == 0 || self.limits.token_ids == 0 || self.limits.score_words == 0 { return Err(Error::InvalidInput); }
        if self.weights.len() > MAX_WEIGHT_FILE_BYTES || self.monitor.len() > MAX_MONITOR_CONFIG_BYTES
            || self.sampling.len() > MAX_SAMPLING_CONFIG_BYTES
            || self.limits.token_ids > MAX_BOUND_DECODER_TOKENS || self.limits.score_words > MAX_BOUND_DECODER_SCORE_WORDS
        { return Err(Error::Limit); }
        Ok(())
    }
    pub(in super::super) fn build(&self) -> Result<MonitoredSampledDecoder, Error> {
        self.check_bounds()?;
        let model = match &self.shards {
            Some(set) => set.build(&self.profile, self.output_head)?,
            None => DecoderModel::from_safetensors_with_output_head(
                self.profile.clone(), &self.weights, self.output_head).map_err(weight_error)?.0,
        };
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
        match &self.shards {
            Some(set) => set.write(w)?,
            None => w.blob(&self.weights)?,
        }
        w.blob(&self.monitor)?; w.blob(&self.sampling)
    }
    pub(super) fn read(r: &mut Reader<'_>) -> Result<Self, Error> {
        Self::read_with_output_head(r, OutputHead::Independent)
    }
    // Only the decoder event discriminator selects this mode. The legacy body
    // and tag 0 keep their exact meaning; bytes never infer a missing head.
    pub(super) fn read_with_output_head(r: &mut Reader<'_>, output_head: OutputHead) -> Result<Self, Error> {
        Self::read_layout(r, output_head, false)
    }
    pub(super) fn read_sharded(r: &mut Reader<'_>, output_head: OutputHead) -> Result<Self, Error> {
        Self::read_layout(r, output_head, true)
    }
    fn read_layout(r: &mut Reader<'_>, output_head: OutputHead, sharded: bool) -> Result<Self, Error> {
        let identity = DecoderIdentity { tenant: r.u64()?, model: r.u64()?, model_generation: r.u64()?,
            tokenizer_generation: r.u64()?, profile_generation: r.u64()? };
        let epsilon = f64::from_bits(r.u64()?); let theta = f64::from_bits(r.u64()?); let stream = r.u64()?;
        let shape = DecoderShape { vocabulary: r.count(65_536)?, hidden: r.count(2_048)?, intermediate: r.count(8_192)?,
            layers: r.count(128)?, query_heads: r.count(2_048)?, cache_heads: r.count(2_048)?, context: r.count(1_048_576)? };
        let profile = DecoderProfile::new(identity, shape, epsilon, theta)?;
        let limits = DecoderBindingLimits { token_ids: r.count(MAX_BOUND_DECODER_TOKENS)?, score_words: r.count(MAX_BOUND_DECODER_SCORE_WORDS)? };
        let (weights, shards) = if sharded {
            (Rc::from(&b""[..]), Some(Rc::new(ShardSet::read(r, &profile, output_head)?)))
        } else { (Rc::from(r.blob(MAX_WEIGHT_FILE_BYTES)?), None) };
        let config = Self { profile, output_head, limits, stream, weights, shards,
            monitor: Rc::from(r.blob(MAX_MONITOR_CONFIG_BYTES)?), sampling: Rc::from(r.blob(MAX_SAMPLING_CONFIG_BYTES)?) };
        config.check_bounds()?;
        // Semantic replay calls the same native build; decoding does not retain a
        // second model or call unchecked constructors to import a running session.
        Ok(config)
    }
}

fn weight_error(error: WeightError) -> Error {
    match error { WeightError::Limit => Error::Limit, WeightError::Model(error) => error, _ => Error::InvalidInput }
}

#[cfg(test)]
mod tests;
