//! Portable original fitting inputs. Parsing NEVER produces a fitted codebook.
pub mod reader;
use super::{FitBudget, FitCheckpointData, LearnedKvCodec, LearnedKvFitCheckpoint,
    LearnedKvPolicy, ModelKvImage, ModelKvProfile, check_budget, input_bytes,
    MAX_FIT_CHECKPOINT_INPUT_BYTES, MAX_FIT_CHECKPOINT_WITNESS_BYTES, MAX_TRAINING_SOURCES};
use super::super::{ModelKvDescriptor, MAX_MODEL_KV_VALUES};
use super::super::fit::{MAX_FIT_SCRATCH_VALUES, MAX_FIT_WORK, MAX_TRAINING_ROWS};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const DOMAIN: &[u8; 8] = b"FAKVFIT\x01";
const FIXED_BYTES: usize = 88;
pub const MAX_FIT_ARCHIVE_BYTES: usize = MAX_FIT_CHECKPOINT_INPUT_BYTES
    + MAX_FIT_CHECKPOINT_WITNESS_BYTES + FIXED_BYTES + 24 * MAX_TRAINING_SOURCES;

/// Independently retained intended configuration and complete source inventory.
/// It binds descriptors, NOT scalar payload integrity or source authenticity.
/// Authenticate the entire archive externally where that is required. A caller
/// must not derive this expectation from the same untrusted archive being read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnedKvFitBinding {
    pub policy: LearnedKvPolicy,
    pub budget: FitBudget,
    pub sources: BTreeMap<u64, ModelKvDescriptor>,
}

/// Unverified archive input. The only route to a usable codec executes the
/// original fitter and compares its actual output; parsed reports/coefficients
/// cannot enter the original LearnedKvCodec constructor as model state.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::model::learned::{
///     LearnedKvCodec, replay::archive::LearnedKvFitArchive};
/// fn bypass(parsed: LearnedKvFitArchive) -> LearnedKvCodec { parsed }
/// ```
pub struct LearnedKvFitArchive { data: FitCheckpointData }
impl fmt::Debug for LearnedKvFitArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedKvFitArchive").field("policy", &self.data.policy)
            .field("training_sources", &self.data.training.len())
            .field("input_bytes", &self.data.input_bytes)
            .field("verified", &false).finish_non_exhaustive()
    }
}

impl LearnedKvFitCheckpoint {
    pub fn binding(&self) -> LearnedKvFitBinding {
        LearnedKvFitBinding { policy: self.data.policy, budget: self.data.budget,
            sources: self.data.training.iter().map(|(id, image)| (*id, image.descriptor())).collect() }
    }

    /// Original source images plus opaque comparison material. No parameters,
    /// training labels or fit reports are imported by this encoder. V1 learned
    /// image bytes and all existing journal formats remain unchanged.
    pub fn encode_archive(&self, byte_limit: usize) -> Result<Vec<u8>, Error> {
        check_limit(byte_limit)?;
        let length = archive_len(self.data.training.len(), self.data.input_bytes, self.data.witness.len())?;
        if length > byte_limit { return Err(Error::Limit); }
        let mut out = Vec::new(); out.try_reserve_exact(length).map_err(|_| Error::Limit)?;
        out.extend_from_slice(DOMAIN);
        let p = self.data.policy;
        for n in [p.id(), p.generation(), p.rank() as u64, p.sweeps() as u64] { word(&mut out, n); }
        let b = self.data.budget;
        for n in [b.source_values as u64, b.parameter_values as u64, b.scratch_values as u64, b.work_units] {
            word(&mut out, n);
        }
        word(&mut out, self.data.training.len() as u64);
        // ALL descriptors precede ALL scalar payloads, so import can preflight
        // the complete expected inventory and conserved totals before allocation.
        for (origin, image) in &self.data.training {
            let d = image.descriptor(); let bytes = d.encode()?;
            word(&mut out, *origin); word(&mut out, bytes.len() as u64);
            word(&mut out, d.image_len()? as u64); out.extend_from_slice(&bytes);
        }
        word(&mut out, self.data.witness.len() as u64);
        out.extend_from_slice(&self.data.witness);
        for image in self.data.training.values() {
            let bytes = image.encode()?;
            out.extend_from_slice(&bytes[image.descriptor().descriptor_len()..]);
        }
        if out.len() != length { return Err(Error::Binding); }
        Ok(out)
    }
}

impl LearnedKvFitArchive {
    /// Parse one exact archive under an independently supplied binding. Structural
    /// admission reads no scalar before every descriptor, original cap, source
    /// identity, aggregate value/row/byte bound and exact framing has passed.
    /// Saved result bytes remain opaque and cannot provide a usable codec.
    pub fn decode(bytes: &[u8], expected: &LearnedKvFitBinding, byte_limit: usize)
        -> Result<Self, Error>
    {
        check_limit(byte_limit)?;
        if bytes.len() > byte_limit { return Err(Error::Limit); }
        validate_binding(expected)?;
        let mut r = Reader { bytes, offset: 0 };
        if r.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
        let policy = LearnedKvPolicy::new(r.word()?, r.word()?, r.count(128)?, r.count(32)?)?;
        let budget = FitBudget { source_values: r.count(MAX_MODEL_KV_VALUES)?,
            parameter_values: r.count(super::super::MAX_LEARNED_PARAMETERS)?,
            scratch_values: r.count(MAX_FIT_SCRATCH_VALUES)?, work_units: r.word()? };
        if policy != expected.policy || budget != expected.budget { return Err(Error::Binding); }
        let count = r.count(MAX_TRAINING_SOURCES)?;
        if count != expected.sources.len() { return Err(Error::Binding); }
        let mut descriptors = Vec::new(); descriptors.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut total = 0_usize;
        for (origin, descriptor) in &expected.sources {
            if r.word()? != *origin { return Err(Error::Binding); }
            let header = r.count(super::super::super::MAX_MODEL_DESCRIPTOR_BYTES)?;
            let length = r.count(super::super::super::MAX_MODEL_IMAGE_BYTES)?;
            let encoded = r.take(header)?;
            let actual = ModelKvDescriptor::decode(encoded)?;
            if actual != *descriptor || length != actual.image_len()? { return Err(Error::Binding); }
            total = total.checked_add(length).ok_or(Error::Limit)?;
            if total > MAX_FIT_CHECKPOINT_INPUT_BYTES { return Err(Error::Limit); }
            descriptors.push((*origin, actual, encoded, length - header));
        }
        let witness_len = r.count(MAX_FIT_CHECKPOINT_WITNESS_BYTES)?;
        if witness_len == 0 { return Err(Error::Incomplete); }
        if archive_len(count, total, witness_len)? != bytes.len() { return Err(Error::InvalidInput); }
        let witness = r.take(witness_len)?;
        // Framing and the whole inventory have passed. Construct original images
        // with their existing decoder; never import a live ModelKvCapture.
        let mut training = BTreeMap::new();
        for (origin, descriptor, header, payload_len) in descriptors {
            let payload = r.take(payload_len)?;
            let mut framed = Vec::new();
            framed.try_reserve_exact(header.len() + payload.len()).map_err(|_| Error::Limit)?;
            framed.extend_from_slice(header); framed.extend_from_slice(payload);
            training.insert(origin, ModelKvImage::decode(&framed, &descriptor)?);
        }
        if r.offset != bytes.len() || input_bytes(&training)? != total { return Err(Error::Binding); }
        let mut retained = Vec::new(); retained.try_reserve_exact(witness_len).map_err(|_| Error::Limit)?;
        retained.extend_from_slice(witness);
        Ok(Self { data: FitCheckpointData { policy, budget, training, input_bytes: total, witness: retained } })
    }

    /// Produce a NEW original fit and compare all result words before returning
    /// its codec and verified checkpoint. Neither the result witness nor its
    /// descriptive training statistics is loaded as state. Each successful call
    /// costs a fresh fit; allocation/replay failure cannot expose a partial codec.
    pub fn replay(self, budget: FitBudget) -> Result<(LearnedKvCodec, LearnedKvFitCheckpoint), Error> {
        check_budget(budget, self.data.budget)?;
        let (codec, checkpoint) = LearnedKvCodec::fit_with_checkpoint(self.data.policy, &self.data.training, budget)?;
        if checkpoint.data.witness != self.data.witness { return Err(Error::Binding); }
        Ok((codec, checkpoint))
    }
}

fn validate_binding(binding: &LearnedKvFitBinding) -> Result<(), Error> {
    if binding.sources.is_empty() || binding.sources.contains_key(&0) { return Err(Error::InvalidInput); }
    if binding.sources.len() > MAX_TRAINING_SOURCES { return Err(Error::Limit); }
    check_budget(binding.budget, FitBudget::default())?;
    let mut profile: Option<&ModelKvProfile> = None;
    let mut streams = BTreeSet::new(); let mut values = 0_usize; let mut rows = 0_usize;
    for descriptor in binding.sources.values() {
        descriptor.image_len()?;
        if profile.is_some_and(|p| p != descriptor.profile()) { return Err(Error::Binding); }
        profile = Some(descriptor.profile());
        let source = descriptor.layers().values().next().ok_or(Error::Incomplete)?;
        if source.token_count == 0 { return Err(Error::InvalidInput); }
        if !streams.insert((source.stream, source.source_batch)) { return Err(Error::Duplicate); }
        rows = rows.checked_add(source.token_count).ok_or(Error::Limit)?;
        values = values.checked_add(source.token_count.checked_mul(descriptor.profile().values_per_token())
            .ok_or(Error::Limit)?).ok_or(Error::Limit)?;
        if rows > MAX_TRAINING_ROWS || values > binding.budget.source_values { return Err(Error::Limit); }
    }
    if rows < 2 { return Err(Error::Incomplete); }
    // The original fitter remains the authority for parameter, scratch and loop
    // budgets; here only the import's scalar/descriptor allocation is admitted.
    if binding.budget.work_units > MAX_FIT_WORK { return Err(Error::Limit); }
    Ok(())
}
fn check_limit(limit: usize) -> Result<(), Error> {
    if limit == 0 || limit > MAX_FIT_ARCHIVE_BYTES { Err(Error::Limit) } else { Ok(()) }
}
fn archive_len(sources: usize, input: usize, witness: usize) -> Result<usize, Error> {
    if sources > MAX_TRAINING_SOURCES || input > MAX_FIT_CHECKPOINT_INPUT_BYTES
        || witness > MAX_FIT_CHECKPOINT_WITNESS_BYTES { return Err(Error::Limit); }
    FIXED_BYTES.checked_add(sources.checked_mul(24).ok_or(Error::Limit)?)
        .and_then(|n| n.checked_add(input)).and_then(|n| n.checked_add(witness)).ok_or(Error::Limit)
}
fn word(bytes: &mut Vec<u8>, value: u64) { bytes.extend_from_slice(&value.to_be_bytes()); }
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Limit)?;
        let part = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end; Ok(part)
    }
    fn word(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let value = usize::try_from(self.word()?).map_err(|_| Error::Limit)?;
        if value > limit { Err(Error::Limit) } else { Ok(value) }
    }
}
