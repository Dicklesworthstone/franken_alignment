//! Portable stochastic state is untrusted until the original decoder replays it.
//! Parsing cannot manufacture a DecoderCheckpoint or restore a saved live cache.

use super::{SampleBudget, SampledCheckpoint, SampledSession, same_logits};
use super::super::{SamplerSnapshot, SamplingPolicy, SAMPLER_SNAPSHOT_BYTES};
use super::super::super::{DecoderModel, DecoderProfile, DecoderWork};
use super::super::super::super::{MAX_KV_POSITIONS, model::{
    ModelKvDescriptor, ModelKvImage, MAX_MODEL_KV_VALUES, MAX_MODEL_IMAGE_BYTES,
    MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
}};
use crate::Error;
use std::fmt;

const DOMAIN: &[u8; 8] = b"FASDCP\0\x01";
const REQUIRED_COMPONENTS: u32 = 63;
pub const SAMPLED_ARCHIVE_HEADER_BYTES: usize = 128;
pub const MAX_SAMPLED_ARCHIVE_BYTES: usize = SAMPLED_ARCHIVE_HEADER_BYTES
    + 2 * SAMPLER_SNAPSHOT_BYTES + 12 * MAX_KV_POSITIONS
    + 4 * super::super::super::MAX_DECODER_VOCABULARY + MAX_MODEL_IMAGE_BYTES;

/// Smaller per-import/export limits; zero tokens/cache values permit an empty
/// checkpoint. These are allocation/data bounds, not restored effect rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub bytes: usize,
    pub tokens: usize,
    pub cache_values: usize,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self { bytes: MAX_SAMPLED_ARCHIVE_BYTES, tokens: MAX_KV_POSITIONS,
            cache_values: MAX_MODEL_KV_VALUES }
    }
}
impl ArchiveLimits {
    fn check(self) -> Result<(), Error> {
        if self.bytes > MAX_SAMPLED_ARCHIVE_BYTES || self.tokens > MAX_KV_POSITIONS
            || self.cache_values > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        Ok(())
    }
}

/// Parsed claims about a numerical history. There is deliberately no conversion
/// to a trusted checkpoint, direct cache install, mutable accessor or permit.
/// Verification recomputes the history rather than assigning these saved arrays.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
///     SampledCheckpoint, archive::SampledArchive,
/// };
/// fn trust(parsed: SampledArchive) -> SampledCheckpoint { parsed }
/// ```
pub struct SampledArchive {
    profile: DecoderProfile,
    source_stream: u64,
    initial: SamplerSnapshot,
    sampler: SamplerSnapshot,
    tokens: Vec<u32>,
    sampled_positions: Vec<u64>,
    logits: Option<Vec<f32>>,
    cache: ModelKvImage,
    encoded_bytes: usize,
}
impl fmt::Debug for SampledArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SampledArchive").field("profile", &self.profile)
            .field("source_stream", &self.source_stream).field("tokens", &self.tokens.len())
            .field("draws", &self.sampled_positions.len()).finish_non_exhaustive()
    }
}

/// Successful consistency check against the explicitly supplied model. This is
/// NOT authentication of the file, original host, parameter provenance or time.
/// The returned session is a NEW computation with its own capture stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveReplayReceipt {
    pub profile: DecoderProfile,
    pub source: ModelKvDescriptor,
    pub replay_stream: u64,
    pub initial_sampler: SamplerSnapshot,
    pub final_sampler: SamplerSnapshot,
    pub tokens_compared: usize,
    pub sampled_tokens_compared: usize,
    pub cache_values_compared: usize,
    pub logits_compared: usize,
    pub encoded_bytes: usize,
    pub recomputation: DecoderWork,
}

impl SampledCheckpoint {
    /// Save the complete restricted-engine state. No weights or authority are
    /// copied. Source-generation metadata is retained, not used as authentication.
    pub fn encode_archive(&self, limits: ArchiveLimits) -> Result<Vec<u8>, Error> {
        let model = self.numerical.model();
        let layout = Layout::new(model, self.numerical.tokens().len(), self.sampled_positions.len(), limits)?;
        let cache = self.numerical.cache();
        let descriptor = cache.descriptor();
        check_descriptor(&descriptor, model, self.numerical.stream(), layout.tokens)?;
        check_history(&self.initial, &self.sampler, &self.sampled_positions, layout.tokens)?;
        if self.numerical.logits().map_or(0, <[f32]>::len) != layout.logits {
            return Err(Error::Binding);
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(layout.length).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(DOMAIN);
        bytes.extend_from_slice(&profile_bytes(model.profile()));
        bytes.extend_from_slice(&self.numerical.stream().to_be_bytes());
        for count in [layout.tokens, layout.samples, layout.logits, layout.descriptor] {
            bytes.extend_from_slice(&(count as u32).to_be_bytes());
        }
        bytes.extend_from_slice(&(layout.image as u64).to_be_bytes());
        bytes.extend_from_slice(&REQUIRED_COMPONENTS.to_be_bytes());
        bytes.extend_from_slice(&self.initial.encode());
        bytes.extend_from_slice(&self.sampler.encode());
        for token in self.numerical.tokens() { bytes.extend_from_slice(&token.to_be_bytes()); }
        for position in self.sampled_positions.iter() { bytes.extend_from_slice(&position.to_be_bytes()); }
        for value in self.numerical.logits().into_iter().flatten() {
            bytes.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        let encoded_cache = cache.encode()?;
        if encoded_cache.len() != layout.image { return Err(Error::Binding); }
        bytes.extend_from_slice(&encoded_cache);
        if bytes.len() != layout.length { return Err(Error::Binding); }
        Ok(bytes)
    }
}

impl SampledArchive {
    /// The caller supplies the intended model/profile and sampling policy.
    /// Their identities cannot be chosen by the file. Entire framing, component
    /// sizes, roster and cut are checked before allocating numerical cache rows.
    pub fn decode(bytes: &[u8], model: &DecoderModel, expected: &SamplingPolicy,
        limits: ArchiveLimits) -> Result<Self, Error>
    {
        limits.check()?;
        if bytes.len() > limits.bytes { return Err(Error::Limit); }
        let mut r = Reader { bytes, at: 0 };
        if r.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
        if r.take(84)? != profile_bytes(model.profile()).as_slice() { return Err(Error::Binding); }
        if expected.vocabulary() != model.profile().shape().vocabulary { return Err(Error::Binding); }
        let stream = r.u64()?;
        if stream == 0 { return Err(Error::InvalidInput); }
        let tokens = r.u32()? as usize;
        let samples = r.u32()? as usize;
        let logits = r.u32()? as usize;
        let descriptor = r.u32()? as usize;
        let image = usize::try_from(r.u64()?).map_err(|_| Error::Limit)?;
        if r.u32()? != REQUIRED_COMPONENTS { return Err(Error::Incomplete); }
        let layout = Layout::new(model, tokens, samples, limits)?;
        if logits != layout.logits || descriptor != layout.descriptor || image != layout.image
            || bytes.len() != layout.length { return Err(Error::Binding); }
        let initial = SamplerSnapshot::decode(r.take(SAMPLER_SNAPSHOT_BYTES)?, expected)?;
        let sampler = SamplerSnapshot::decode(r.take(SAMPLER_SNAPSHOT_BYTES)?, expected)?;
        let cache_descriptor = ModelKvDescriptor::decode(&bytes[layout.cache_at..layout.cache_at + descriptor])?;
        check_descriptor(&cache_descriptor, model, stream, tokens)?;
        if cache_descriptor.image_len()? != image { return Err(Error::Binding); }
        let mut original = Vec::new();
        original.try_reserve_exact(tokens).map_err(|_| Error::Limit)?;
        for _ in 0..tokens {
            let token = r.u32()?;
            if token as usize >= model.profile().shape().vocabulary { return Err(Error::InvalidInput); }
            original.push(token);
        }
        let mut positions = Vec::new();
        positions.try_reserve_exact(samples).map_err(|_| Error::Limit)?;
        for _ in 0..samples { positions.push(r.u64()?); }
        check_history(&initial, &sampler, &positions, tokens)?;
        let mut scores = Vec::new();
        scores.try_reserve_exact(logits).map_err(|_| Error::Limit)?;
        for _ in 0..logits {
            let score = f32::from_bits(r.u32()?);
            if !score.is_finite() { return Err(Error::InvalidInput); }
            scores.push(score);
        }
        if r.at != layout.cache_at { return Err(Error::Binding); }
        let cache = ModelKvImage::decode(r.take(image)?, &cache_descriptor)?;
        if r.at != bytes.len() { return Err(Error::Binding); }
        Ok(Self { profile: model.profile().clone(), source_stream: stream, initial, sampler,
            tokens: original, sampled_positions: positions, logits: (logits != 0).then_some(scores),
            cache, encoded_bytes: bytes.len() })
    }

    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn source_stream(&self) -> u64 { self.source_stream }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn sampled_positions(&self) -> &[u64] { &self.sampled_positions }
    pub fn initial_sampler(&self) -> &SamplerSnapshot { &self.initial }
    pub fn final_sampler(&self) -> &SamplerSnapshot { &self.sampler }
    pub fn cache_descriptor(&self) -> ModelKvDescriptor { self.cache.descriptor() }

    /// Recompute with the original execution/selection path shared by in-memory
    /// checkpoint checking. No saved KV/logit/RNG ending state is installed.
    /// Passing proves consistency of this finite history, not all model weights
    /// or original-source authenticity. Coordinated valid rewrites remain possible.
    pub fn recompute(&self, model: &DecoderModel, replay_stream: u64, budget: SampleBudget)
        -> Result<(SampledSession, ArchiveReplayReceipt), Error>
    {
        if model.profile() != &self.profile || model.cache_profile() != self.cache.profile() {
            return Err(Error::Binding);
        }
        if replay_stream == 0 || replay_stream == self.source_stream { return Err(Error::InvalidInput); }
        let replay = model.replay_sampled_history(replay_stream, &self.tokens, &self.initial,
            &self.sampled_positions, budget)?;
        if replay.sampler_state() != self.sampler || replay.sampled_positions() != self.sampled_positions.as_slice()
            || !super::super::super::checkpoint::same_values(&replay.cache_image()?, &self.cache)?
            || !same_logits(replay.logits().ok(), self.logits.as_deref()) { return Err(Error::Binding); }
        let receipt = ArchiveReplayReceipt {
            profile: self.profile.clone(), source: self.cache.descriptor(), replay_stream,
            initial_sampler: self.initial.clone(), final_sampler: self.sampler.clone(),
            tokens_compared: self.tokens.len(), sampled_tokens_compared: self.sampled_positions.len(),
            cache_values_compared: self.cache.normalized_values(), logits_compared: self.logits.as_ref().map_or(0, Vec::len),
            encoded_bytes: self.encoded_bytes, recomputation: replay.work(),
        };
        Ok((replay, receipt))
    }
}

fn check_history(initial: &SamplerSnapshot, final_state: &SamplerSnapshot, positions: &[u64], count: usize)
    -> Result<(), Error>
{
    if initial.draws() != 0 || initial.stream() != final_state.stream()
        || initial.policy() != final_state.policy() || final_state.draws() != positions.len() as u64
        || positions.windows(2).any(|pair| pair[0] >= pair[1])
        || positions.iter().any(|p| *p == 0 || *p >= count as u64) { return Err(Error::Binding); }
    Ok(())
}
fn check_descriptor(d: &ModelKvDescriptor, model: &DecoderModel, stream: u64, count: usize) -> Result<(), Error> {
    if d.profile() != model.cache_profile() { return Err(Error::Binding); }
    for layer in d.layers().values() {
        if (layer.stream, layer.source_batch, layer.first_position, layer.first_sequence, layer.token_count)
            != (stream, 0, 0, 1, count) { return Err(Error::Binding); }
    }
    Ok(())
}

struct Layout { tokens: usize, samples: usize, logits: usize, descriptor: usize, image: usize,
    cache_at: usize, length: usize }
impl Layout {
    fn new(model: &DecoderModel, tokens: usize, samples: usize, limits: ArchiveLimits) -> Result<Self, Error> {
        limits.check()?;
        let shape = model.profile().shape();
        if tokens > limits.tokens || tokens > shape.context { return Err(Error::Limit); }
        if samples > tokens { return Err(Error::Binding); }
        let values = tokens.checked_mul(model.cache_profile().values_per_token()).ok_or(Error::Overflow)?;
        if values > limits.cache_values { return Err(Error::Limit); }
        let logits = if tokens == 0 { 0 } else { shape.vocabulary };
        let descriptor = MODEL_DESCRIPTOR_HEADER_BYTES + shape.layers * MODEL_LAYER_DESCRIPTOR_BYTES;
        let image = descriptor.checked_add(values.checked_mul(4).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        let cache_at = SAMPLED_ARCHIVE_HEADER_BYTES + 2 * SAMPLER_SNAPSHOT_BYTES
            + 4 * tokens + 8 * samples + 4 * logits;
        let length = cache_at.checked_add(image).ok_or(Error::Overflow)?;
        if length > limits.bytes || image > MAX_MODEL_IMAGE_BYTES { return Err(Error::Limit); }
        Ok(Self { tokens, samples, logits, descriptor, image, cache_at, length })
    }
}
fn profile_bytes(profile: &DecoderProfile) -> [u8; 84] {
    let mut out = [0; 84];
    let id = profile.identity();
    for (i, value) in [id.tenant, id.model, id.model_generation, id.tokenizer_generation, id.profile_generation]
        .into_iter().enumerate() { out[i * 8..i * 8 + 8].copy_from_slice(&value.to_be_bytes()); }
    let s = profile.shape();
    for (i, value) in [s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads, s.cache_heads, s.context]
        .into_iter().enumerate() { out[40 + i * 4..44 + i * 4].copy_from_slice(&(value as u32).to_be_bytes()); }
    out[68..76].copy_from_slice(&profile.epsilon().to_bits().to_be_bytes());
    out[76..84].copy_from_slice(&profile.theta().to_bits().to_be_bytes());
    out
}
struct Reader<'a> { bytes: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).ok_or(Error::Overflow)?;
        let value = self.bytes.get(self.at..end).ok_or(Error::Incomplete)?;
        self.at = end; Ok(value)
    }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?)) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?)) }
}
