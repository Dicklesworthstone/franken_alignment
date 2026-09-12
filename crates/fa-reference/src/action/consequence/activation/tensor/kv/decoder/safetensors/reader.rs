//! Startup I/O over already authorized readers, not path discovery or a runtime.
//! All shard directories are checked before any tensor body is decoded. Raw
//! parameters use a fixed-size scratch buffer, then move into the original model.

use super::{DecoderModel, DecoderProfile, TensorDescriptor, TensorIssue, TensorLoad,
    WeightError, WeightLoadReceipt, MAX_WEIGHT_HEADER_BYTES, MAX_WEIGHT_FILE_BYTES,
    construct, decode_scalar, ByteOrder, inventory, inspect_directory, issue};
use super::shards::{ShardLoad, ShardPlan, ShardedWeightLoadReceipt, MAX_WEIGHT_SET_BYTES};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read};

pub const MAX_WEIGHT_READ_CALLS: usize = 1_000_000;
pub const MAX_WEIGHT_READ_BYTES: usize = MAX_WEIGHT_SET_BYTES + 1;
pub const WEIGHT_READ_CHUNK_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeightReadStage { Prefix, Header, TensorData, EndOfFile }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeightReadError {
    Refused(WeightError),
    Io { stage: WeightReadStage, kind: io::ErrorKind },
}
impl From<WeightError> for WeightReadError {
    fn from(error: WeightError) -> Self { Self::Refused(error) }
}
impl fmt::Display for WeightReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for WeightReadError {}

/// Actual bytes returned and read attempts, including Interrupted and EOF probes.
/// This is startup work accounting, not effect authority or wall-clock evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeightReadUsage { pub bytes_read: usize, pub read_calls: usize }

/// A shared allowance across ALL files and failed attempts. No loader resets it.
/// An EOF probe needs one byte of remaining allowance even when it returns zero.
/// Read-call bounds cannot stop a single blocking Read; scheduling/timeouts remain
/// the host's responsibility. No blocking network reader is opened implicitly.
#[derive(Debug)]
pub struct WeightReadBudget {
    byte_limit: usize,
    call_limit: usize,
    usage: WeightReadUsage,
}
impl WeightReadBudget {
    pub fn new(byte_limit: usize, call_limit: usize) -> Result<Self, WeightError> {
        if byte_limit == 0 || call_limit == 0 { return Err(WeightError::Model(crate::Error::InvalidInput)); }
        if byte_limit > MAX_WEIGHT_READ_BYTES || call_limit > MAX_WEIGHT_READ_CALLS { return Err(WeightError::Limit); }
        Ok(Self { byte_limit, call_limit, usage: WeightReadUsage::default() })
    }
    pub fn usage(&self) -> WeightReadUsage { self.usage }
    pub fn remaining_bytes(&self) -> usize { self.byte_limit - self.usage.bytes_read }
    pub fn remaining_calls(&self) -> usize { self.call_limit - self.usage.read_calls }

    fn require_bytes(&self, bytes: usize) -> Result<(), WeightReadError> {
        if bytes > self.remaining_bytes() { return Err(WeightError::Limit.into()); }
        Ok(())
    }
    fn read<R: Read + ?Sized>(
        &mut self, source: &mut R, buffer: &mut [u8], stage: WeightReadStage,
    ) -> Result<usize, WeightReadError> {
        let size = buffer.len().min(self.remaining_bytes());
        if size == 0 || self.remaining_calls() == 0 { return Err(WeightError::Limit.into()); }
        self.usage.read_calls += 1;
        let count = source.read(&mut buffer[..size]).map_err(|error| WeightReadError::Io { stage, kind: error.kind() })?;
        // A faulty Read implementation must not overflow counters or offsets.
        if count > size { return Err(WeightReadError::Io { stage, kind: io::ErrorKind::InvalidData }); }
        self.usage.bytes_read += count;
        Ok(count)
    }
}

struct FilePlan {
    header_bytes: usize,
    data_bytes: usize,
    file_bytes: usize,
    tensors: BTreeMap<String, TensorDescriptor>,
}

impl DecoderModel {
    /// Read exactly one complete export from the current reader position to EOF.
    /// No Seek, whole-file raw buffer, path lookup or model-profile inference.
    /// On error the reader position and budget remain consumed; restarting an
    /// import requires explicit host ownership of a fresh/repositioned source.
    pub fn read_safetensors<R: Read + ?Sized>(
        profile: DecoderProfile, source: &mut R, budget: &mut WeightReadBudget,
    ) -> Result<(Self, WeightLoadReceipt), WeightReadError> {
        let plan = read_directory(&inventory(&profile), source, budget, MAX_WEIGHT_FILE_BYTES)?;
        budget.require_bytes(plan.data_bytes.checked_add(1).ok_or(WeightError::Limit)?)?;
        let (mut parameters, source_receipt) = read_body(plan, source, budget)?;
        let receipt = WeightLoadReceipt { normalized_bytes: profile.parameter_count() * 4,
            profile: profile.clone(), file_bytes: source_receipt.file_bytes,
            header_bytes: source_receipt.header_bytes, data_bytes: source_receipt.data_bytes,
            tensors: source_receipt.tensors };
        let model = construct(profile, |name| parameters.remove(name).ok_or(WeightError::Inventory))?;
        Ok((model, receipt))
    }

    /// Index labels select ONLY these already supplied readers. Every source key
    /// and every shard's complete directory is checked before reading any scalar.
    /// All files share one aggregate allowance; none receives a fresh budget.
    pub fn read_safetensors_shards<R: Read>(
        profile: DecoderProfile, index: &[u8], sources: &mut BTreeMap<String, R>,
        budget: &mut WeightReadBudget,
    ) -> Result<(Self, ShardedWeightLoadReceipt), WeightReadError> {
        let index_plan = ShardPlan::parse(&profile, index)?;
        if !index_plan.partitions.keys().eq(sources.keys()) { return Err(WeightError::Inventory.into()); }
        let mut plans = BTreeMap::new(); let mut file_bytes = 0_usize; let mut data_bytes = 0_usize;
        for (name, expected) in &index_plan.partitions {
            let source = sources.get_mut(name).ok_or(WeightError::Inventory)?;
            let plan = read_directory(expected, source, budget, MAX_WEIGHT_SET_BYTES - file_bytes)?;
            file_bytes = file_bytes.checked_add(plan.file_bytes).ok_or(WeightError::Limit)?;
            data_bytes = data_bytes.checked_add(plan.data_bytes).ok_or(WeightError::Limit)?;
            plans.insert(name.clone(), plan);
        }
        if index_plan.total_size.is_some_and(|total| total != data_bytes) { return Err(WeightError::Inventory.into()); }
        // All headers have been consumed. Reserve enough READ allowance for the
        // complete remaining payload and a final one-byte end-of-file probe.
        budget.require_bytes(data_bytes.checked_add(1).ok_or(WeightError::Limit)?)?;
        let mut parameters = BTreeMap::new(); let mut shards = BTreeMap::new();
        for (name, plan) in plans {
            let source = sources.get_mut(&name).ok_or(WeightError::Inventory)?;
            let (values, receipt) = read_body(plan, source, budget)?;
            for (tensor, values) in values {
                if parameters.insert(tensor, values).is_some() { return Err(WeightError::Inventory.into()); }
            }
            shards.insert(name, receipt);
        }
        let receipt = ShardedWeightLoadReceipt { normalized_bytes: profile.parameter_count() * 4,
            profile: profile.clone(), index_bytes: index.len(), file_bytes, data_bytes, shards };
        let model = construct(profile, |name| parameters.remove(name).ok_or(WeightError::Inventory))?;
        Ok((model, receipt))
    }
}

fn read_directory<R: Read + ?Sized>(
    expected: &BTreeMap<String, Vec<usize>>, source: &mut R,
    budget: &mut WeightReadBudget, file_allowance: usize,
) -> Result<FilePlan, WeightReadError> {
    if file_allowance < 8 { return Err(WeightError::Limit.into()); }
    let mut prefix = [0; 8];
    read_exact(source, &mut prefix, budget, WeightReadStage::Prefix)?;
    let header_bytes = usize::try_from(u64::from_le_bytes(prefix)).map_err(|_| WeightError::Limit)?;
    if header_bytes > MAX_WEIGHT_HEADER_BYTES { return Err(WeightError::Limit.into()); }
    let fixed = 8_usize.checked_add(header_bytes).ok_or(WeightError::Limit)?;
    if fixed > file_allowance { return Err(WeightError::Limit.into()); }
    budget.require_bytes(header_bytes)?;
    let mut header = Vec::new();
    header.try_reserve_exact(header_bytes).map_err(|_| WeightError::Limit)?;
    header.resize(header_bytes, 0);
    read_exact(source, &mut header, budget, WeightReadStage::Header)?;
    let tensors = inspect_directory(expected, &header)?;
    drop(header);
    let data_bytes = tensors.values().map(|tensor| tensor.end).max().unwrap_or(0);
    let file_bytes = fixed.checked_add(data_bytes).ok_or(WeightError::Limit)?;
    if file_bytes > file_allowance { return Err(WeightError::Limit.into()); }
    Ok(FilePlan { header_bytes, data_bytes, file_bytes, tensors })
}

fn read_body<R: Read + ?Sized>(
    plan: FilePlan, source: &mut R, budget: &mut WeightReadBudget,
) -> Result<(BTreeMap<String, Vec<f32>>, ShardLoad), WeightReadError> {
    let mut ordered: Vec<_> = plan.tensors.into_iter().collect();
    ordered.sort_unstable_by_key(|(_, tensor)| tensor.start);
    let mut parameters = BTreeMap::new(); let mut interpretation = BTreeMap::new();
    let mut scratch = [0; WEIGHT_READ_CHUNK_BYTES];
    for (name, tensor) in ordered {
        let raw_bytes = tensor.end - tensor.start;
        let width = tensor.encoding.bytes();
        let mut values = Vec::new();
        values.try_reserve_exact(raw_bytes / width).map_err(|_| WeightError::Limit)?;
        let mut remaining = raw_bytes;
        while remaining != 0 {
            let length = remaining.min(scratch.len());
            read_exact(source, &mut scratch[..length], budget, WeightReadStage::TensorData)?;
            for word in scratch[..length].chunks_exact(width) {
                values.push(decode_scalar(word, tensor.encoding, ByteOrder::Little)
                    .map_err(|_| issue(&name, TensorIssue::NonFinite))?);
            }
            remaining -= length;
        }
        interpretation.insert(name.clone(), TensorLoad { shape: tensor.shape,
            encoding: tensor.encoding, data_bytes: raw_bytes });
        parameters.insert(name, values);
    }
    probe_eof(source, budget)?;
    Ok((parameters, ShardLoad { file_bytes: plan.file_bytes, header_bytes: plan.header_bytes,
        data_bytes: plan.data_bytes, tensors: interpretation }))
}

fn read_exact<R: Read + ?Sized>(
    source: &mut R, bytes: &mut [u8], budget: &mut WeightReadBudget, stage: WeightReadStage,
) -> Result<(), WeightReadError> {
    let mut position = 0;
    while position < bytes.len() {
        match budget.read(source, &mut bytes[position..], stage) {
            Ok(0) => return Err(WeightReadError::Io { stage, kind: io::ErrorKind::UnexpectedEof }),
            Ok(count) => position += count,
            Err(WeightReadError::Io { kind: io::ErrorKind::Interrupted, .. }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
fn probe_eof<R: Read + ?Sized>(source: &mut R, budget: &mut WeightReadBudget) -> Result<(), WeightReadError> {
    let mut byte = [0];
    loop {
        match budget.read(source, &mut byte, WeightReadStage::EndOfFile) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(WeightError::Header.into()),
            Err(WeightReadError::Io { kind: io::ErrorKind::Interrupted, .. }) => {}
            Err(error) => return Err(error),
        }
    }
}
