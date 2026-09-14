//! Locally source-checked envelopes and optional exact XOR escape blocks.
//! The learned decoder never supplies its own claimed fidelity bound. These
//! objects own compact learned state, envelopes and EXPLICIT optional residuals,
//! not the original source arrays. No origin authentication or live restart is
//! implied. Standalone exported bytes have no unchecked evidence constructor.
use crate::Error;
use crate::action::consequence::activation::FrameIdentity;
use crate::action::consequence::activation::tensor::kv::experiment::{KvCell, KvSide};
use crate::action::consequence::activation::tensor::kv::model::{ModelKvDescriptor, ModelKvImage, MAX_MODEL_KV_VALUES};
use crate::action::consequence::activation::tensor::kv::model::learned::LearnedKvImage;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_CHECKED_KV_GROUPS: usize = 65_536;
pub const MAX_CHECKED_KV_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_CHECKED_KV_PRODUCTS: u64 = 1_073_741_824;
pub const CHECKED_HEADER_BYTES: usize = 24;
pub const CHECKED_GROUP_BYTES: usize = 41;
pub const RESIDUAL_HEADER_BYTES: usize = 41;
const DOMAIN: &[u8; 8] = b"FAKVCK\0\x01";
const RESIDUAL_DOMAIN: &[u8; 8] = b"FAKVRX\0\x01";
const MIN_ORDER: u32 = 0x0080_0000;
const MAX_ORDER: u32 = 0xff7f_ffff;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KvRow { pub layer: u64, pub side: KvSide, pub position: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KvGroup { pub row: KvRow, pub head: usize }
impl KvGroup {
    fn cell(self, channel: usize) -> KvCell {
        KvCell { side: self.row.side, position: self.row.position, head: self.head, channel }
    }
}
#[derive(Clone, Debug)]
pub enum ResidualRetention { None, All, Groups(BTreeSet<KvGroup>) }
impl ResidualRetention {
    fn retains(&self, group: &KvGroup) -> bool {
        match self { Self::None => false, Self::All => true, Self::Groups(groups) => groups.contains(group) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedKvBudget {
    pub source_values: usize,
    /// Entire base export plus ALL retained residual blocks, not just the blocks
    /// that a later monitor chooses to materialize.
    pub encoded_bytes: usize,
    pub reconstruction_products: u64,
}
impl Default for CheckedKvBudget {
    fn default() -> Self {
        Self { source_values: MAX_MODEL_KV_VALUES, encoded_bytes: MAX_CHECKED_KV_BYTES,
            reconstruction_products: MAX_CHECKED_KV_PRODUCTS }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedKvReport {
    pub source_values: usize,
    pub source_coordinate_visits: usize,
    pub reconstruction_products: u64,
    pub groups: usize,
    pub retained_groups: usize,
    pub residual_changed_words: usize,
    pub base_encoded_bytes: usize,
    pub retained_residual_bytes: usize,
    pub total_encoded_bytes: usize,
}
struct Envelope {
    channels: usize,
    /// Maximum distance in the ordered finite-binary32 word space, including
    /// the distinct signed zeros. NOT an absolute-error norm or an MSE estimate.
    radius: u32,
    changed: usize,
    residual: Option<Rc<[u8]>>,
}
struct Data {
    image: LearnedKvImage,
    envelopes: BTreeMap<KvGroup, Envelope>,
    report: CheckedKvReport,
}
/// Source-checking certifies only a relation to the exact supplied ModelKvImage.
/// Its source provenance is still a caller assumption. Complete descriptors,
/// not just dimensions/model names, must agree before any values are read.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::{SourceFrame, probe::learned::CheckedLearnedKv};
/// fn relabel(value: CheckedLearnedKv) -> SourceFrame { value }
/// ```
#[derive(Clone)]
pub struct CheckedLearnedKv { data: Rc<Data> }
impl fmt::Debug for CheckedLearnedKv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedLearnedKv").field("image", &self.data.image)
            .field("report", &self.data.report).finish_non_exhaustive()
    }
}
impl CheckedLearnedKv {
    pub fn new(image: LearnedKvImage, source: &ModelKvImage, retention: ResidualRetention,
        budget: CheckedKvBudget) -> Result<Self, Error>
    {
        if image.source_descriptor() != &source.descriptor() { return Err(Error::Binding); }
        if budget.source_values > MAX_MODEL_KV_VALUES || budget.encoded_bytes > MAX_CHECKED_KV_BYTES
            || budget.reconstruction_products > MAX_CHECKED_KV_PRODUCTS
            || source.normalized_values() > budget.source_values { return Err(Error::Limit); }
        let descriptor = image.source_descriptor(); descriptor.image_len()?;
        let mut count = 0_usize;
        for layer in descriptor.layers().values() {
            count = count.checked_add(layer.token_count.checked_mul(layer.contract.keys().heads()
                + layer.contract.values().heads()).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        }
        if count > MAX_CHECKED_KV_GROUPS { return Err(Error::Limit); }
        let base_bytes = CHECKED_HEADER_BYTES.checked_add(image.encoded_len())
            .and_then(|n| n.checked_add(count.checked_mul(CHECKED_GROUP_BYTES)?)).ok_or(Error::Overflow)?;
        if base_bytes > budget.encoded_bytes { return Err(Error::Limit); }
        let mut envelopes = BTreeMap::new();
        let mut retained_values = 0_usize;
        let mut retained_groups = 0_usize;
        for (layer_id, layer) in descriptor.layers() {
            for offset in 0..layer.token_count {
                for (side, contract) in [(KvSide::Key, layer.contract.keys()), (KvSide::Value, layer.contract.values())] {
                    for head in 0..contract.heads() {
                        let group = KvGroup { row: KvRow { layer: *layer_id, side,
                            position: layer.first_position + offset as u64 }, head };
                        if retention.retains(&group) {
                            retained_values = retained_values.checked_add(contract.channels()).ok_or(Error::Overflow)?;
                            retained_groups += 1;
                        }
                        envelopes.insert(group, Envelope { channels: contract.channels(), radius: 0, changed: 0, residual: None });
                    }
                }
            }
        }
        if let ResidualRetention::Groups(requested) = &retention {
            if requested.len() > MAX_CHECKED_KV_GROUPS { return Err(Error::Limit); }
            if requested.iter().any(|group| !envelopes.contains_key(group)) { return Err(Error::Missing); }
        }
        let visits = source.normalized_values().checked_add(retained_values).ok_or(Error::Overflow)?;
        let products = (visits as u64).checked_mul(image.codec().policy().rank() as u64).ok_or(Error::Overflow)?;
        if products > budget.reconstruction_products { return Err(Error::Limit); }
        // The entire inventory and both passes' arithmetic are admitted before
        // reading the first source scalar. Pass one sizes exact residual storage;
        // no partially checked object or partial success can escape a failure.
        let mut residual_bytes = 0_usize;
        let mut changed_words = 0_usize;
        for (group, envelope) in &mut envelopes {
            let words = source_words(source, *group, envelope.channels)?;
            for (channel, original) in words.iter().copied().enumerate() {
                let approximate = image.bits(group.row.layer, group.cell(channel))?;
                envelope.radius = envelope.radius.max(ordered(original)?.abs_diff(ordered(approximate)?));
                envelope.changed += usize::from(original != approximate);
            }
            if retention.retains(group) {
                let bytes = RESIDUAL_HEADER_BYTES.checked_add(envelope.changed.checked_mul(8).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
                residual_bytes = residual_bytes.checked_add(bytes).ok_or(Error::Overflow)?;
                changed_words = changed_words.checked_add(envelope.changed).ok_or(Error::Overflow)?;
                if base_bytes.checked_add(residual_bytes).ok_or(Error::Overflow)? > budget.encoded_bytes { return Err(Error::Limit); }
            }
        }
        for (group, envelope) in &mut envelopes {
            if !retention.retains(group) { continue; }
            let length = RESIDUAL_HEADER_BYTES + envelope.changed * 8;
            let mut bytes = Vec::new(); bytes.try_reserve_exact(length).map_err(|_| Error::Limit)?;
            bytes.extend_from_slice(RESIDUAL_DOMAIN); put_group(&mut bytes, *group);
            put32(&mut bytes, envelope.channels)?; put32(&mut bytes, envelope.changed)?;
            for (channel, original) in source_words(source, *group, envelope.channels)?.iter().copied().enumerate() {
                let approximate = image.bits(group.row.layer, group.cell(channel))?;
                let xor = original ^ approximate;
                if xor != 0 { put32(&mut bytes, channel)?; bytes.extend_from_slice(&xor.to_be_bytes()); }
            }
            if bytes.len() != length { return Err(Error::Binding); }
            envelope.residual = Some(bytes.into());
        }
        let report = CheckedKvReport { source_values: source.normalized_values(), source_coordinate_visits: visits,
            reconstruction_products: products, groups: count, retained_groups, residual_changed_words: changed_words,
            base_encoded_bytes: base_bytes, retained_residual_bytes: residual_bytes,
            total_encoded_bytes: base_bytes + residual_bytes };
        Ok(Self { data: Rc::new(Data { image, envelopes, report }) })
    }
    pub fn image(&self) -> &LearnedKvImage { &self.data.image }
    pub fn descriptor(&self) -> &ModelKvDescriptor { self.image().source_descriptor() }
    pub fn report(&self) -> &CheckedKvReport { &self.data.report }
    pub fn groups(&self) -> impl Iterator<Item = KvGroup> + '_ { self.data.envelopes.keys().copied() }
    pub fn radius(&self, group: KvGroup) -> Result<u32, Error> { Ok(self.envelope(group)?.radius) }
    pub fn channels(&self, group: KvGroup) -> Result<usize, Error> { Ok(self.envelope(group)?.channels) }
    pub fn residual_bytes(&self, group: KvGroup) -> Result<&[u8], Error> {
        self.envelope(group)?.residual.as_deref().ok_or(Error::Missing)
    }
    /// Local byte-for-byte verification against the source-checked expected
    /// residual. This is NOT an authenticated network importer or a digest oracle.
    pub fn verify_residual(&self, group: KvGroup, bytes: &[u8]) -> Result<CheckedKvResidual, Error> {
        let expected = self.envelope(group)?.residual.as_ref().ok_or(Error::Missing)?;
        if bytes != expected.as_ref() { return Err(Error::Binding); }
        Ok(CheckedKvResidual { owner: self.clone(), group, bytes: Rc::clone(expected) })
    }
    pub fn view(&self) -> LearnedKvView {
        LearnedKvView { source: self.clone(), exact: BTreeMap::new(), revision: 0, values: 0 }
    }
    pub fn row_shape(&self, row: KvRow) -> Result<(FrameIdentity, usize, usize), Error> {
        let layer = self.descriptor().layers().get(&row.layer).ok_or(Error::Missing)?;
        let offset = row.position.checked_sub(layer.first_position).ok_or(Error::Missing)?;
        if offset >= layer.token_count as u64 { return Err(Error::Missing); }
        let contract = match row.side { KvSide::Key => layer.contract.keys(), KvSide::Value => layer.contract.values() };
        Ok((FrameIdentity { profile: contract.profile(), stream: layer.stream,
            sequence: layer.first_sequence.checked_add(offset).ok_or(Error::Overflow)?, position: row.position },
            contract.heads(), contract.channels()))
    }
    pub fn encode_base(&self) -> Result<Vec<u8>, Error> {
        let coarse = self.image().encode()?;
        let mut out = Vec::new(); out.try_reserve_exact(self.report().base_encoded_bytes).map_err(|_| Error::Limit)?;
        out.extend_from_slice(DOMAIN); put64(&mut out, coarse.len() as u64);
        put64(&mut out, self.data.envelopes.len() as u64); out.extend_from_slice(&coarse);
        for (group, envelope) in &self.data.envelopes {
            put_group(&mut out, *group); put32(&mut out, envelope.channels)?;
            out.extend_from_slice(&envelope.radius.to_be_bytes());
            put64(&mut out, envelope.residual.as_ref().map_or(0, |bytes| bytes.len()) as u64);
        }
        if out.len() != self.report().base_encoded_bytes { return Err(Error::Binding); }
        Ok(out)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut out = self.encode_base()?;
        out.try_reserve_exact(self.report().retained_residual_bytes).map_err(|_| Error::Limit)?;
        for envelope in self.data.envelopes.values() {
            if let Some(bytes) = &envelope.residual { out.extend_from_slice(bytes); }
        }
        if out.len() != self.report().total_encoded_bytes { return Err(Error::Binding); }
        Ok(out)
    }
    fn envelope(&self, group: KvGroup) -> Result<&Envelope, Error> {
        self.data.envelopes.get(&group).ok_or(Error::Missing)
    }
}
#[derive(Clone)]
pub struct CheckedKvResidual { owner: CheckedLearnedKv, group: KvGroup, bytes: Rc<[u8]> }
impl fmt::Debug for CheckedKvResidual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedKvResidual").field("group", &self.group)
            .field("encoded_bytes", &self.bytes.len()).finish_non_exhaustive()
    }
}
impl CheckedKvResidual {
    pub fn group(&self) -> KvGroup { self.group }
    pub fn encoded_bytes(&self) -> usize { self.bytes.len() }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvRefinementBudget {
    pub encoded_bytes: usize,
    pub materialized_values: usize,
    pub reconstruction_products: u64,
}
impl Default for KvRefinementBudget {
    fn default() -> Self {
        Self { encoded_bytes: MAX_CHECKED_KV_BYTES, materialized_values: MAX_MODEL_KV_VALUES,
            reconstruction_products: MAX_CHECKED_KV_PRODUCTS }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvRefinementReceipt {
    pub group: KvGroup,
    pub revision: u64,
    pub encoded_bytes: usize,
    pub materialized_values: usize,
    pub reconstruction_products: u64,
}
/// Branch-local exact group materializations; cloning shares immutable arrays,
/// not future promotions. A view is numerical evidence, not a restart checkpoint.
#[derive(Clone)]
pub struct LearnedKvView {
    source: CheckedLearnedKv,
    exact: BTreeMap<KvGroup, Rc<[u32]>>,
    revision: u64,
    values: usize,
}
impl fmt::Debug for LearnedKvView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedKvView").field("revision", &self.revision)
            .field("refined_groups", &self.exact.len()).field("materialized_values", &self.values).finish_non_exhaustive()
    }
}
impl LearnedKvView {
    pub fn source(&self) -> &CheckedLearnedKv { &self.source }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn materialized_values(&self) -> usize { self.values }
    pub fn refined_groups(&self) -> impl Iterator<Item = KvGroup> + '_ { self.exact.keys().copied() }
    pub fn is_refined(&self, group: KvGroup) -> Result<bool, Error> {
        self.source.envelope(group)?; Ok(self.exact.contains_key(&group))
    }
    pub fn interval(&self, group: KvGroup, channel: usize) -> Result<[f32; 2], Error> {
        let envelope = self.source.envelope(group)?;
        if channel >= envelope.channels { return Err(Error::InvalidInput); }
        if let Some(words) = self.exact.get(&group) {
            let value = f32::from_bits(words[channel]); return Ok([value, value]);
        }
        let center = ordered(self.source.image().bits(group.row.layer, group.cell(channel))?)?;
        let low = center.saturating_sub(envelope.radius).max(MIN_ORDER);
        let high = center.saturating_add(envelope.radius).min(MAX_ORDER);
        Ok([f32::from_bits(unordered(low)), f32::from_bits(unordered(high))])
    }
    pub fn refine(&mut self, expected_revision: u64, block: &CheckedKvResidual,
        budget: KvRefinementBudget) -> Result<KvRefinementReceipt, Error>
    {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if !Rc::ptr_eq(&self.source.data, &block.owner.data) { return Err(Error::Binding); }
        if self.exact.contains_key(&block.group) { return Err(Error::Duplicate); }
        let envelope = self.source.envelope(block.group)?;
        let products = (envelope.channels as u64).checked_mul(self.source.image().codec().policy().rank() as u64).ok_or(Error::Overflow)?;
        if budget.encoded_bytes > MAX_CHECKED_KV_BYTES || budget.materialized_values > MAX_MODEL_KV_VALUES
            || budget.reconstruction_products > MAX_CHECKED_KV_PRODUCTS || block.bytes.len() > budget.encoded_bytes
            || envelope.channels > budget.materialized_values || products > budget.reconstruction_products { return Err(Error::Limit); }
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let values = self.values.checked_add(envelope.channels).ok_or(Error::Overflow)?;
        if values > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        let mut words = Vec::new(); words.try_reserve_exact(envelope.channels).map_err(|_| Error::Limit)?;
        for channel in 0..envelope.channels {
            words.push(self.source.image().bits(block.group.row.layer, block.group.cell(channel))?);
        }
        let payload = block.bytes.get(RESIDUAL_HEADER_BYTES..).ok_or(Error::Incomplete)?;
        if !payload.len().is_multiple_of(8) || payload.len() / 8 != envelope.changed { return Err(Error::Binding); }
        let mut previous = None;
        for record in payload.chunks_exact(8) {
            let channel = u32::from_be_bytes(record[..4].try_into().map_err(|_| Error::Incomplete)?) as usize;
            let xor = u32::from_be_bytes(record[4..].try_into().map_err(|_| Error::Incomplete)?);
            if channel >= words.len() || xor == 0 || previous.is_some_and(|before| before >= channel) { return Err(Error::Binding); }
            let original = words[channel] ^ xor;
            if ordered(original)?.abs_diff(ordered(words[channel])?) > envelope.radius { return Err(Error::Binding); }
            words[channel] = original; previous = Some(channel);
        }
        let receipt = KvRefinementReceipt { group: block.group, revision, encoded_bytes: block.bytes.len(),
            materialized_values: envelope.channels, reconstruction_products: products };
        // All numerical work and allocations precede publication. General
        // allocator abort is outside Result-level atomicity, as elsewhere here.
        let words: Rc<[u32]> = words.into();
        self.exact.insert(block.group, words); self.revision = revision; self.values = values;
        Ok(receipt)
    }
}
fn source_words(source: &ModelKvImage, group: KvGroup, channels: usize) -> Result<&[u32], Error> {
    let token = source.layer(group.row.layer)?.token(group.row.position)?;
    let frame = match group.row.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
    let start = group.head.checked_mul(channels).ok_or(Error::Overflow)?;
    frame.words.get(start..start + channels).ok_or(Error::Binding)
}
fn ordered(bits: u32) -> Result<u32, Error> {
    if !f32::from_bits(bits).is_finite() { return Err(Error::InvalidInput); }
    Ok(if bits >> 31 == 0 { bits ^ 0x8000_0000 } else { !bits })
}
fn unordered(word: u32) -> u32 { if word >> 31 == 0 { !word } else { word ^ 0x8000_0000 } }
fn put64(bytes: &mut Vec<u8>, value: u64) { bytes.extend_from_slice(&value.to_be_bytes()); }
fn put32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    bytes.extend_from_slice(&u32::try_from(value).map_err(|_| Error::Overflow)?.to_be_bytes()); Ok(())
}
fn put_group(bytes: &mut Vec<u8>, group: KvGroup) {
    put64(bytes, group.row.layer); bytes.push(if group.row.side == KvSide::Key { 0 } else { 1 });
    put64(bytes, group.row.position); put64(bytes, group.head as u64);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_finite_extremes_and_signed_zeros_roundtrip_without_float_error_estimates() {
        let values = [0xff7f_ffff, 0xbf80_0000, 0x8000_0001, 0x8000_0000, 0, 1, 0x3f80_0000, 0x7f7f_ffff];
        let keys: Vec<_> = values.iter().map(|bits| ordered(*bits).unwrap()).collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(keys[0], MIN_ORDER); assert_eq!(*keys.last().unwrap(), MAX_ORDER);
        for (bits, key) in values.into_iter().zip(keys) { assert_eq!(unordered(key), bits); }
        assert_eq!(ordered(f32::INFINITY.to_bits()), Err(Error::InvalidInput));
        assert_eq!(ordered(f32::NAN.to_bits()), Err(Error::InvalidInput));
    }
}
