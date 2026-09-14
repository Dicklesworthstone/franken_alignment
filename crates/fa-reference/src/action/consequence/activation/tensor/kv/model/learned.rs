//! Trained per-layer/per-stored-head linear KV bottleneck (plan 10.5).
//! Explicitly lossy experimental data, never a live capture or exact checkpoint.
//! The fitted mean and basis own no training/source scalar arrays.
mod fit;
pub use fit::{FitBudget, FitReport, GroupFitReport, TrainingSource, MAX_FIT_WORK};

use super::{ModelKvDescriptor, ModelKvImage, ModelKvProfile, MAX_MODEL_KV_VALUES};
use super::super::experiment::{KvCell, KvSide};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const MAX_LEARNED_CHANNELS: usize = 128;
pub const MAX_LEARNED_PARAMETERS: usize = 1_048_576;
pub const MAX_LEARNED_IMAGE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_COMPRESSION_WORK: u64 = 1_073_741_824;
const DOMAIN: &[u8; 8] = b"FAKVLR\0\x01";
const IMAGE_HEADER_BYTES: usize = 64;
const GROUP_HEADER_BYTES: usize = 25;

/// Fixed before fitting. Cyclic symmetric Jacobi sweeps are bounded iterations,
/// not a claim of convergence or globally optimal rank selection. Every group
/// uses this exact rank; an incompatible width refuses rather than clipping it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedKvPolicy { id: u64, generation: u64, rank: usize, sweeps: usize }
impl LearnedKvPolicy {
    pub fn new(id: u64, generation: u64, rank: usize, sweeps: usize) -> Result<Self, Error> {
        if id == 0 || generation == 0 || rank == 0 || sweeps == 0 { return Err(Error::InvalidInput); }
        if rank > MAX_LEARNED_CHANNELS || sweeps > 32 { return Err(Error::Limit); }
        Ok(Self { id, generation, rank, sweeps })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn rank(self) -> usize { self.rank }
    pub fn sweeps(self) -> usize { self.sweeps }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupKey { pub layer: u64, pub side: KvSide, pub head: usize }

/// Emitted binary32 parameters used by BOTH actual encoding and reconstruction.
/// axes is component-major [rank, channels]. There is no mutable accessor.
#[derive(Clone, Debug)]
pub struct GroupBasis { mean: Vec<f32>, axes: Vec<f32> }
impl GroupBasis {
    pub fn mean(&self) -> &[f32] { &self.mean }
    pub fn axes(&self) -> &[f32] { &self.axes }
    pub fn channels(&self) -> usize { self.mean.len() }
}
struct CodecData {
    policy: LearnedKvPolicy,
    profile: ModelKvProfile,
    groups: BTreeMap<GroupKey, GroupBasis>,
    report: FitReport,
}
#[derive(Clone)]
pub struct LearnedKvCodec { data: Rc<CodecData> }
impl fmt::Debug for LearnedKvCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedKvCodec").field("policy", &self.policy())
            .field("groups", &self.data.groups.len()).field("parameters", &self.fit_report().parameter_values)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressionBudget {
    pub source_values: usize,
    /// Complete export: source descriptors, training lineage, means, bases and
    /// latent words. Shared codebook amortization is not silently assumed.
    pub encoded_bytes: usize,
    /// Conservative arithmetic-loop admission, not time, FLOPs or RSS.
    pub work_units: u64,
}
impl Default for CompressionBudget {
    fn default() -> Self {
        Self { source_values: MAX_MODEL_KV_VALUES, encoded_bytes: MAX_LEARNED_IMAGE_BYTES,
            work_units: MAX_COMPRESSION_WORK }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReconstructionError {
    pub values: usize,
    pub changed_words: usize,
    pub nonzero_to_zero: usize,
    pub signed_zero_changes: usize,
    pub squared_error_sum: f64,
    pub max_absolute_error: f64,
}
impl ReconstructionError {
    fn record(&mut self, original: f32, reconstructed: f32) {
        self.values += 1;
        self.changed_words += usize::from(original.to_bits() != reconstructed.to_bits());
        self.nonzero_to_zero += usize::from(original != 0.0 && reconstructed == 0.0);
        self.signed_zero_changes += usize::from(original == 0.0 && reconstructed == 0.0
            && original.to_bits() != reconstructed.to_bits());
        let delta = f64::from(reconstructed) - f64::from(original);
        self.squared_error_sum += delta * delta;
        self.max_absolute_error = self.max_absolute_error.max(delta.abs());
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct CompressionReport {
    pub policy: LearnedKvPolicy,
    pub source: ModelKvDescriptor,
    pub training_source_overlap: bool,
    pub original_scalar_bytes: usize,
    pub original_image_bytes: usize,
    pub latent_scalar_bytes: usize,
    pub codebook_scalar_bytes: usize,
    pub metadata_bytes: usize,
    pub encoded_bytes: usize,
    pub work_units_reserved: u64,
    pub groups: BTreeMap<GroupKey, ReconstructionError>,
}
struct ImageData {
    codec: LearnedKvCodec,
    descriptor: ModelKvDescriptor,
    latents: BTreeMap<GroupKey, Vec<f32>>,
    encoded_bytes: usize,
}
/// Shares only fitted parameters, latent coordinates and metadata. No original
/// source or training cache is secretly retained. Binary32 reconstruction is
/// explicitly approximate; it cannot be relabeled as observed model state.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvImage, learned::LearnedKvImage};
/// fn exact(image: LearnedKvImage) -> ModelKvImage { image }
/// ```
#[derive(Clone)]
pub struct LearnedKvImage { data: Rc<ImageData> }
impl fmt::Debug for LearnedKvImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedKvImage").field("policy", &self.codec().policy())
            .field("encoded_bytes", &self.encoded_len()).finish_non_exhaustive()
    }
}

impl LearnedKvCodec {
    /// The complete training set is supplied together, keyed by nonzero original
    /// task/lineage identity. Ordering is canonical. Repeated streams or origins
    /// refuse. Identity authenticity and pre-generation split assignment remain
    /// caller obligations; this is not a detector qualification campaign.
    pub fn fit(policy: LearnedKvPolicy, training: &BTreeMap<u64, ModelKvImage>, budget: FitBudget)
        -> Result<Self, Error>
    {
        fit::fit(policy, training, budget)
    }
    pub fn policy(&self) -> LearnedKvPolicy { self.data.policy }
    pub fn profile(&self) -> &ModelKvProfile { &self.data.profile }
    pub fn groups(&self) -> &BTreeMap<GroupKey, GroupBasis> { &self.data.groups }
    pub fn fit_report(&self) -> &FitReport { &self.data.report }

    pub fn training_source_overlap(&self, image: &ModelKvImage) -> bool {
        let descriptor = image.descriptor();
        self.fit_report().sources.values().any(|source| same_stream(&source.descriptor, &descriptor))
    }

    /// A separate numerical evaluation that cannot fit or retune the codebook.
    /// Original task identity AND source stream/batch must be absent from fitting.
    /// Success measures this finite capture, not independence from undisclosed
    /// near-duplicate tasks, unseen attacks, or harmfulness of later tokens.
    pub fn evaluate_held_out(&self, origin: u64, image: &ModelKvImage, budget: CompressionBudget)
        -> Result<(LearnedKvImage, CompressionReport), Error>
    {
        if origin == 0 { return Err(Error::InvalidInput); }
        if self.fit_report().sources.contains_key(&origin) || self.training_source_overlap(image) {
            return Err(Error::Duplicate);
        }
        self.compress(image, budget)
    }

    pub fn encoded_len_for(&self, image: &ModelKvImage) -> Result<usize, Error> {
        Ok(self.layout(image)?.0)
    }

    /// Check the complete image, resource budget and output allocation before
    /// reading scalar values. A numerical failure exposes no partial encoding.
    /// Means, axes and latents are scored AFTER binary32 rounding, not using the
    /// optimizer's higher-precision coefficients to report a better error.
    pub fn compress(&self, image: &ModelKvImage, budget: CompressionBudget)
        -> Result<(LearnedKvImage, CompressionReport), Error>
    {
        let (encoded_bytes, latent_values, work_units) = self.layout(image)?;
        if budget.source_values > MAX_MODEL_KV_VALUES || budget.encoded_bytes > MAX_LEARNED_IMAGE_BYTES
            || budget.work_units > MAX_COMPRESSION_WORK || image.normalized_values() > budget.source_values
            || encoded_bytes > budget.encoded_bytes || work_units > budget.work_units { return Err(Error::Limit); }
        let descriptor = image.descriptor();
        let rank = self.policy().rank;
        let mut latents = BTreeMap::new();
        for key in self.groups().keys() {
            let count = descriptor.layers()[&key.layer].token_count * rank;
            let mut values = Vec::new(); values.try_reserve_exact(count).map_err(|_| Error::Limit)?;
            latents.insert(*key, values);
        }
        let mut errors = BTreeMap::new();
        for (key, basis) in self.groups() {
            let layer = &descriptor.layers()[&key.layer];
            let d = basis.channels();
            let values = latents.get_mut(key).expect("complete latent inventory");
            let mut error = ReconstructionError::default();
            for offset in 0..layer.token_count {
                let words = group_words(image, *key, layer.first_position + offset as u64, d)?;
                for axis in basis.axes.chunks_exact(d) {
                    let mut projection = 0.0_f64;
                    for j in 0..d { projection += f64::from(axis[j]) * (finite(words[j])? - f64::from(basis.mean[j])); }
                    values.push(rounded(projection)?);
                }
                let code = &values[offset * rank..(offset + 1) * rank];
                for (j, word) in words.iter().enumerate() {
                    let original = f32::from_bits(*word);
                    error.record(original, reconstruct(basis, code, j)?);
                }
            }
            errors.insert(*key, error);
        }
        let codebook_scalar_bytes = self.fit_report().parameter_values * 4;
        let latent_scalar_bytes = latent_values * 4;
        let original_image_bytes = descriptor.image_len()?;
        let report = CompressionReport {
            policy: self.policy(), source: descriptor.clone(), training_source_overlap: self.training_source_overlap(image),
            original_scalar_bytes: original_image_bytes - descriptor.descriptor_len(), original_image_bytes,
            latent_scalar_bytes, codebook_scalar_bytes,
            metadata_bytes: encoded_bytes - codebook_scalar_bytes - latent_scalar_bytes,
            encoded_bytes, work_units_reserved: work_units, groups: errors,
        };
        Ok((LearnedKvImage { data: Rc::new(ImageData { codec: self.clone(), descriptor, latents, encoded_bytes }) }, report))
    }

    fn layout(&self, image: &ModelKvImage) -> Result<(usize, usize, u64), Error> {
        if image.profile() != self.profile() { return Err(Error::Binding); }
        let descriptor = image.descriptor(); descriptor.image_len()?;
        let mut bytes = IMAGE_HEADER_BYTES.checked_add(descriptor.descriptor_len()).ok_or(Error::Overflow)?;
        for source in self.fit_report().sources.values() {
            bytes = bytes.checked_add(16 + source.descriptor.descriptor_len()).ok_or(Error::Overflow)?;
        }
        let mut latent_values = 0_usize;
        let mut work = 0_u64;
        for (key, basis) in self.groups() {
            let count = descriptor.layers()[&key.layer].token_count;
            let n = count.checked_mul(self.policy().rank).ok_or(Error::Overflow)?;
            latent_values = latent_values.checked_add(n).ok_or(Error::Overflow)?;
            let scalars = basis.mean.len().checked_add(basis.axes.len()).and_then(|m| m.checked_add(n)).ok_or(Error::Overflow)?;
            bytes = bytes.checked_add(GROUP_HEADER_BYTES).and_then(|m| m.checked_add(scalars.checked_mul(4)?)).ok_or(Error::Overflow)?;
            let units = (count as u64).checked_mul(basis.channels() as u64)
                .and_then(|m| m.checked_mul(4 * self.policy().rank as u64 + 8)).ok_or(Error::Overflow)?;
            work = work.checked_add(units).ok_or(Error::Overflow)?;
        }
        if bytes > MAX_LEARNED_IMAGE_BYTES || work > MAX_COMPRESSION_WORK { return Err(Error::Limit); }
        Ok((bytes, latent_values, work))
    }
}

impl LearnedKvImage {
    pub fn codec(&self) -> &LearnedKvCodec { &self.data.codec }
    pub fn source_descriptor(&self) -> &ModelKvDescriptor { &self.data.descriptor }
    pub fn encoded_len(&self) -> usize { self.data.encoded_bytes }
    pub fn latent_values(&self) -> usize { self.data.latents.values().map(Vec::len).sum() }

    /// Source coordinates retain original absolute position and stored KV head.
    /// Query-head multiplicity is not materialized into duplicate cache arrays.
    pub fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> {
        let descriptor = self.source_descriptor().layers().get(&layer).ok_or(Error::Missing)?;
        let offset = cell.position.checked_sub(descriptor.first_position).ok_or(Error::Missing)?;
        let offset = usize::try_from(offset).map_err(|_| Error::Missing)?;
        if offset >= descriptor.token_count { return Err(Error::Missing); }
        let key = GroupKey { layer, side: cell.side, head: cell.head };
        let basis = self.codec().groups().get(&key).ok_or(Error::InvalidInput)?;
        if cell.channel >= basis.channels() { return Err(Error::InvalidInput); }
        let rank = self.codec().policy().rank;
        let code = self.data.latents.get(&key).and_then(|row| row.get(offset * rank..(offset + 1) * rank)).ok_or(Error::Missing)?;
        Ok(reconstruct(basis, code, cell.channel)?.to_bits())
    }

    /// Actual complete representation, including codebook and source/fit metadata.
    /// Export is not authenticated provenance. V1 deliberately has no import-to-
    /// fitted-codec conversion: parsing bytes must not claim that fitting ran.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new(); out.try_reserve_exact(self.encoded_len()).map_err(|_| Error::Limit)?;
        out.extend_from_slice(DOMAIN);
        let policy = self.codec().policy();
        for n in [policy.id, policy.generation, policy.rank as u64, policy.sweeps as u64] { put64(&mut out, n); }
        put64(&mut out, self.codec().fit_report().sources.len() as u64);
        for (origin, source) in &self.codec().fit_report().sources {
            put64(&mut out, *origin);
            let descriptor = source.descriptor.encode()?;
            put64(&mut out, descriptor.len() as u64); out.extend_from_slice(&descriptor);
        }
        let descriptor = self.source_descriptor().encode()?;
        put64(&mut out, descriptor.len() as u64); out.extend_from_slice(&descriptor);
        put64(&mut out, self.codec().groups().len() as u64);
        for (key, basis) in self.codec().groups() {
            put64(&mut out, key.layer); out.push(if key.side == KvSide::Key { 0 } else { 1 });
            put64(&mut out, key.head as u64); put64(&mut out, basis.channels() as u64);
            for value in basis.mean.iter().chain(&basis.axes).chain(&self.data.latents[key]) {
                out.extend_from_slice(&value.to_bits().to_be_bytes());
            }
        }
        if out.len() != self.encoded_len() { return Err(Error::Binding); }
        Ok(out)
    }
}
fn put64(bytes: &mut Vec<u8>, value: u64) { bytes.extend_from_slice(&value.to_be_bytes()); }
fn rounded(value: f64) -> Result<f32, Error> {
    let output = value as f32;
    if !value.is_finite() || !output.is_finite() { return Err(Error::Overflow); }
    Ok(output)
}
fn finite(bits: u32) -> Result<f64, Error> {
    let value = f32::from_bits(bits);
    if !value.is_finite() { return Err(Error::InvalidInput); }
    Ok(f64::from(value))
}
fn reconstruct(basis: &GroupBasis, code: &[f32], channel: usize) -> Result<f32, Error> {
    let d = basis.channels();
    let mut value = f64::from(basis.mean[channel]);
    for (axis, coefficient) in code.iter().enumerate() {
        value += f64::from(basis.axes[axis * d + channel]) * f64::from(*coefficient);
    }
    rounded(value)
}
fn group_words(image: &ModelKvImage, key: GroupKey, position: u64, width: usize) -> Result<&[u32], Error> {
    let token = image.layer(key.layer)?.token(position)?;
    let frame = match key.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
    let start = key.head.checked_mul(width).ok_or(Error::Overflow)?;
    frame.words.get(start..start + width).ok_or(Error::Binding)
}
fn same_stream(left: &ModelKvDescriptor, right: &ModelKvDescriptor) -> bool {
    let a = left.layers().values().next().expect("complete layer descriptor");
    let b = right.layers().values().next().expect("complete layer descriptor");
    (a.stream, a.source_batch) == (b.stream, b.source_batch)
}
