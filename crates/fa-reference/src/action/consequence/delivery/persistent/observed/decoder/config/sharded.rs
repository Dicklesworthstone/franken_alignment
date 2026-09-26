//! Exact physical checkpoint sources retained under the original decoder owner.
//! Bounded parsing/label checks grant no authority and never open an index path.
use super::{DecoderModel, DecoderProfile, Error, OutputHead, Rc, Reader, Writer, weight_error};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::shards::{
    MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SET_BYTES, MAX_WEIGHT_SHARDS, check_shard_labels,
};
use std::collections::BTreeMap;

/// Supervisor-supplied data, not a validated model or a reference to live files.
/// new_sharded validates and retains these exact bytes without concatenating or
/// rewriting the source archives. Output-head semantics come from the operator.
#[derive(Clone, Debug)]
pub struct FileDecoderShardInputs {
    pub index: Vec<u8>,
    pub sources: BTreeMap<String, Vec<u8>>,
}
impl FileDecoderShardInputs {
    /// Run the ORIGINAL complete index/label admission before opening explicitly
    /// selected files. The iterator must be in sorted BTreeMap key order. This
    /// checks no tensor bytes, signatures, filesystem paths or source freshness.
    pub fn check_labels<'a>(profile: &DecoderProfile, index: &[u8],
        labels: impl Iterator<Item = &'a str>, output_head: OutputHead) -> Result<(), Error>
    {
        check_shard_labels(profile, index, labels, output_head).map_err(weight_error)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ShardSet {
    index: Rc<[u8]>,
    sources: BTreeMap<String, Rc<[u8]>>,
    source_bytes: usize,
}
impl ShardSet {
    pub(super) fn new(profile: &DecoderProfile, head: OutputHead, inputs: FileDecoderShardInputs)
        -> Result<Self, Error>
    {
        FileDecoderShardInputs::check_labels(profile, &inputs.index,
            inputs.sources.keys().map(String::as_str), head)?;
        let source_bytes = sum_bytes(inputs.sources.values().map(Vec::len), MAX_WEIGHT_SET_BYTES)?;
        let set = Self { index: inputs.index.into(), source_bytes,
            sources: inputs.sources.into_iter().map(|(label, bytes)| (label, bytes.into())).collect() };
        set.check_bounds()?;
        Ok(set)
    }
    pub(super) fn input_bytes(&self) -> usize { self.index.len() + self.source_bytes }
    pub(super) fn check_bounds(&self) -> Result<(), Error> {
        if self.index.len() > MAX_WEIGHT_INDEX_BYTES || self.sources.len() > MAX_WEIGHT_SHARDS {
            return Err(Error::Limit);
        }
        if self.sources.is_empty() { return Err(Error::InvalidInput); }
        if self.sources.keys().any(|key| key.len() > 256) { return Err(Error::Limit); }
        if sum_bytes(self.sources.values().map(|bytes| bytes.len()), MAX_WEIGHT_SET_BYTES)? != self.source_bytes {
            return Err(Error::Binding);
        }
        Ok(())
    }
    pub(super) fn build(&self, profile: &DecoderProfile, head: OutputHead) -> Result<DecoderModel, Error> {
        let sources = self.sources.iter().map(|(label, bytes)| (label.clone(), bytes.as_ref())).collect();
        DecoderModel::from_safetensors_shards_with_output_head(profile.clone(), &self.index,
            &sources, head).map(|(model, _)| model).map_err(weight_error)
    }
    pub(super) fn write(&self, w: &mut Writer) -> Result<(), Error> {
        self.check_bounds()?;
        w.blob(&self.index)?; w.count(self.sources.len())?;
        for (label, bytes) in &self.sources { w.blob(label.as_bytes())?; w.blob(bytes)?; }
        Ok(())
    }
    pub(super) fn read(r: &mut Reader<'_>, profile: &DecoderProfile, head: OutputHead) -> Result<Self, Error> {
        let index: Rc<[u8]> = Rc::from(r.blob(MAX_WEIGHT_INDEX_BYTES)?);
        let count = r.count(MAX_WEIGHT_SHARDS)?;
        if count == 0 { return Err(Error::InvalidInput); }
        let mut sources: BTreeMap<String, Rc<[u8]>> = BTreeMap::new();
        let mut remaining = MAX_WEIGHT_SET_BYTES;
        for _ in 0..count {
            let label = std::str::from_utf8(r.blob(256)?).map_err(|_| Error::InvalidInput)?;
            // Reject duplicate and noncanonical ordering before retaining bytes.
            if sources.last_key_value().is_some_and(|(last, _)| last.as_str() >= label) {
                return Err(Error::InvalidInput);
            }
            let bytes = r.blob(remaining)?;
            remaining = remaining.checked_sub(bytes.len()).ok_or(Error::Limit)?;
            sources.insert(label.to_owned(), Rc::from(bytes));
        }
        FileDecoderShardInputs::check_labels(profile, &index, sources.keys().map(String::as_str), head)?;
        let set = Self { index, sources, source_bytes: MAX_WEIGHT_SET_BYTES - remaining };
        set.check_bounds()?;
        Ok(set)
    }
}

fn sum_bytes(mut lengths: impl Iterator<Item = usize>, limit: usize) -> Result<usize, Error> {
    lengths.try_fold(0_usize, |sum, length| {
        sum.checked_add(length).filter(|sum| *sum <= limit).ok_or(Error::Limit)
    })
}

#[cfg(test)]
mod tests;
