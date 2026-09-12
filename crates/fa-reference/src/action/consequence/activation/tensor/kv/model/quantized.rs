//! Bounded lossy KV baseline. Quantized values are experimental data, never
//! SourceFrames, exact restart images or control authority. The original source
//! cut is retained as metadata; imported bytes do not authenticate that cut.

use super::{ModelKvDescriptor, ModelKvImage, MAX_MODEL_DESCRIPTOR_BYTES, MAX_MODEL_KV_VALUES};
use super::super::experiment::{KvCell, KvSide};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

const DOMAIN: &[u8; 8] = b"FAKVI8\0\x01";
pub const QUANTIZED_HEADER_BYTES: usize = 40;
pub const MAX_QUANTIZED_BYTES: usize = QUANTIZED_HEADER_BYTES + MAX_MODEL_DESCRIPTOR_BYTES + 5 * MAX_MODEL_KV_VALUES;

/// V1 is symmetric signed int8 per token, stored head and K/V side. Each group
/// retains its exact binary32 maximum magnitude, followed by codes -127..127.
/// Query-head duplication, clipping ranges, zero-points and stochastic rounding
/// are not inferred. Code -128 is reserved and refuses on import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvQuantization { id: u64, generation: u64 }
impl KvQuantization {
    pub fn new(id: u64, generation: u64) -> Result<Self, Error> {
        if id == 0 || generation == 0 { return Err(Error::InvalidInput); }
        Ok(Self { id, generation })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantizationBudget {
    pub values: usize,
    /// Complete encoded image, including every scale, descriptor and header.
    pub encoded_bytes: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuantizationError {
    pub values: usize,
    pub changed_words: usize,
    pub nonzero_to_zero: usize,
    pub signed_zero_changes: usize,
    /// Rounded descriptive measurements, NOT certified reconstruction bounds.
    pub max_absolute_error: f64,
    pub squared_error_sum: f64,
}
impl QuantizationError {
    fn record(&mut self, source: f32, reconstructed: f32) {
        self.values += 1;
        self.changed_words += usize::from(source.to_bits() != reconstructed.to_bits());
        self.nonzero_to_zero += usize::from(source != 0.0 && reconstructed == 0.0);
        self.signed_zero_changes += usize::from(source == 0.0 && reconstructed == 0.0
            && source.to_bits() != reconstructed.to_bits());
        let delta = f64::from(reconstructed) - f64::from(source);
        self.max_absolute_error = self.max_absolute_error.max(delta.abs());
        self.squared_error_sum += delta * delta;
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct LayerQuantizationError { pub keys: QuantizationError, pub values: QuantizationError }
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizationReport {
    pub policy: KvQuantization,
    pub source: ModelKvDescriptor,
    pub values: usize,
    pub groups: usize,
    pub source_scalar_bytes: usize,
    pub source_image_bytes: usize,
    pub encoded_bytes: usize,
    /// Two original-coordinate visits: peak scan and code/error computation.
    pub source_coordinate_visits: usize,
    pub layers: BTreeMap<u64, LayerQuantizationError>,
}

#[derive(Clone, Debug)]
struct LayerLayout { offset: usize, row_bytes: usize, key_bytes: usize }
#[derive(Clone, Debug)]
struct Layout {
    layers: BTreeMap<u64, LayerLayout>,
    body_bytes: usize,
    values: usize,
    groups: usize,
    encoded_bytes: usize,
}
impl Layout {
    fn new(descriptor: &ModelKvDescriptor) -> Result<Self, Error> {
        descriptor.image_len()?;
        let mut layers = BTreeMap::new();
        let mut body_bytes = 0_usize;
        let mut values = 0_usize;
        let mut groups = 0_usize;
        for (id, layer) in descriptor.layers() {
            let k = layer.contract.keys(); let v = layer.contract.values();
            let key_bytes = k.heads().checked_mul(4 + k.channels()).ok_or(Error::Overflow)?;
            let value_bytes = v.heads().checked_mul(4 + v.channels()).ok_or(Error::Overflow)?;
            let row_bytes = key_bytes.checked_add(value_bytes).ok_or(Error::Overflow)?;
            layers.insert(*id, LayerLayout { offset: body_bytes, row_bytes, key_bytes });
            body_bytes = body_bytes.checked_add(layer.token_count.checked_mul(row_bytes).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            values = values.checked_add(layer.token_count.checked_mul(k.dimensions() + v.dimensions()).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            groups = groups.checked_add(layer.token_count.checked_mul(k.heads() + v.heads()).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        }
        let encoded_bytes = QUANTIZED_HEADER_BYTES.checked_add(descriptor.descriptor_len())
            .and_then(|n| n.checked_add(body_bytes)).ok_or(Error::Overflow)?;
        if values > MAX_MODEL_KV_VALUES || encoded_bytes > MAX_QUANTIZED_BYTES { return Err(Error::Limit); }
        Ok(Self { layers, body_bytes, values, groups, encoded_bytes })
    }
    fn check(&self, budget: QuantizationBudget) -> Result<(), Error> {
        if budget.values > MAX_MODEL_KV_VALUES || budget.encoded_bytes > MAX_QUANTIZED_BYTES
            || self.values > budget.values || self.encoded_bytes > budget.encoded_bytes { return Err(Error::Limit); }
        Ok(())
    }
}

struct QuantizedData {
    policy: KvQuantization,
    descriptor: ModelKvDescriptor,
    layout: Layout,
    body: Vec<u8>,
}
/// Shares compact bytes and descriptors only. No hidden raw-value copy or source
/// owner is retained. Decode returns the same explicitly lossy data type.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvImage, quantized::QuantizedKvImage};
/// fn exact(image: QuantizedKvImage) -> ModelKvImage { image }
/// ```
#[derive(Clone)]
pub struct QuantizedKvImage { data: Rc<QuantizedData> }
impl fmt::Debug for QuantizedKvImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuantizedKvImage").field("policy", &self.policy())
            .field("values", &self.values()).field("encoded_bytes", &self.encoded_len()).finish_non_exhaustive()
    }
}
impl ModelKvImage {
    pub fn quantized_len(&self) -> Result<usize, Error> { Ok(Layout::new(&self.descriptor())?.encoded_bytes) }

    /// Preflight the whole layer inventory and byte/value limits before reading
    /// any scalar. A failure publishes no partial image and never mutates source.
    pub fn quantize(&self, policy: KvQuantization, budget: QuantizationBudget) -> Result<(QuantizedKvImage, QuantizationReport), Error> {
        let descriptor = self.descriptor(); let layout = Layout::new(&descriptor)?;
        layout.check(budget)?;
        let mut body = Vec::new();
        body.try_reserve_exact(layout.body_bytes).map_err(|_| Error::Limit)?;
        let mut layers = BTreeMap::new();
        for (id, layer) in descriptor.layers() {
            let image = self.layer(*id)?;
            let mut keys = QuantizationError::default(); let mut values = QuantizationError::default();
            for offset in 0..layer.token_count {
                let token = image.token(layer.first_position + offset as u64)?;
                for (frame, contract, error) in [(token.key(), layer.contract.keys(), &mut keys),
                    (token.value(), layer.contract.values(), &mut values)]
                {
                    if frame.words.len() != contract.dimensions() { return Err(Error::Binding); }
                    for group in frame.words.chunks_exact(contract.channels()) {
                        encode_group(group, &mut body, error)?;
                    }
                }
            }
            layers.insert(*id, LayerQuantizationError { keys, values });
        }
        if body.len() != layout.body_bytes { return Err(Error::Binding); }
        let report = QuantizationReport {
            policy, source: descriptor.clone(), values: layout.values, groups: layout.groups,
            source_scalar_bytes: descriptor.image_len()? - descriptor.descriptor_len(),
            source_image_bytes: descriptor.image_len()?, encoded_bytes: layout.encoded_bytes,
            source_coordinate_visits: layout.values * 2, layers,
        };
        Ok((QuantizedKvImage { data: Rc::new(QuantizedData { policy, descriptor, layout, body }) }, report))
    }
}
impl QuantizedKvImage {
    pub fn policy(&self) -> KvQuantization { self.data.policy }
    pub fn source_descriptor(&self) -> &ModelKvDescriptor { &self.data.descriptor }
    pub fn values(&self) -> usize { self.data.layout.values }
    pub fn groups(&self) -> usize { self.data.layout.groups }
    pub fn encoded_len(&self) -> usize { self.data.layout.encoded_bytes }
    pub fn compact_scalar_bytes(&self) -> usize { self.data.body.len() }

    /// Numerical inspection remains explicitly approximate. It cannot be used
    /// as a source-checked capture, exact image or restoration receipt.
    pub fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> {
        let descriptor = self.data.descriptor.layers().get(&layer).ok_or(Error::Missing)?;
        let layout = &self.data.layout.layers[&layer];
        let offset = cell.position.checked_sub(descriptor.first_position).ok_or(Error::Missing)?;
        let offset = usize::try_from(offset).map_err(|_| Error::Missing)?;
        if offset >= descriptor.token_count { return Err(Error::Missing); }
        let (contract, side_offset) = match cell.side {
            KvSide::Key => (descriptor.contract.keys(), 0),
            KvSide::Value => (descriptor.contract.values(), layout.key_bytes),
        };
        if cell.head >= contract.heads() || cell.channel >= contract.channels() { return Err(Error::InvalidInput); }
        let start = layout.offset + offset * layout.row_bytes + side_offset + cell.head * (4 + contract.channels());
        let peak = f32::from_bits(read32(&self.data.body, start)?);
        let code = *self.data.body.get(start + 4 + cell.channel).ok_or(Error::Incomplete)?;
        Ok(reconstruct(peak, code).to_bits())
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let descriptor = self.data.descriptor.encode()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(self.encoded_len()).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(DOMAIN);
        bytes.extend_from_slice(&self.policy().id.to_be_bytes());
        bytes.extend_from_slice(&self.policy().generation.to_be_bytes());
        bytes.extend_from_slice(&(descriptor.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes.extend_from_slice(&(self.data.body.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&descriptor); bytes.extend_from_slice(&self.data.body);
        if bytes.len() != self.encoded_len() { return Err(Error::Binding); }
        Ok(bytes)
    }

    /// Expected policy and source metadata are supplied independently. All scales
    /// and codes validate before a compact body is retained. Valid finite edits
    /// may describe different data; this framing does not authenticate payloads.
    pub fn decode(bytes: &[u8], expected: KvQuantization, source: &ModelKvDescriptor,
        budget: QuantizationBudget) -> Result<Self, Error>
    {
        let layout = Layout::new(source)?; layout.check(budget)?;
        if bytes.len() > budget.encoded_bytes { return Err(Error::Limit); }
        if bytes.len() != layout.encoded_bytes || bytes.get(..8) != Some(DOMAIN.as_slice()) { return Err(Error::InvalidInput); }
        if read64(bytes, 8)? != expected.id || read64(bytes, 16)? != expected.generation { return Err(Error::Binding); }
        if read32(bytes, 24)? as usize != source.descriptor_len() || read32(bytes, 28)? != 0
            || read64(bytes, 32)? != layout.body_bytes as u64 { return Err(Error::InvalidInput); }
        let header_end = QUANTIZED_HEADER_BYTES + source.descriptor_len();
        let descriptor = ModelKvDescriptor::decode(&bytes[QUANTIZED_HEADER_BYTES..header_end])?;
        if &descriptor != source { return Err(Error::Binding); }
        let body = &bytes[header_end..];
        for (id, layer) in descriptor.layers() {
            let l = &layout.layers[id];
            for token in 0..layer.token_count {
                let mut start = l.offset + token * l.row_bytes;
                for contract in [layer.contract.keys(), layer.contract.values()] {
                    for _ in 0..contract.heads() {
                        let peak_bits = read32(body, start)?;
                        let peak = f32::from_bits(peak_bits);
                        if !peak.is_finite() || peak_bits >> 31 != 0 { return Err(Error::InvalidInput); }
                        let codes = &body[start + 4..start + 4 + contract.channels()];
                        if codes.contains(&128) || (peak == 0.0 && codes.iter().any(|q| *q != 0))
                            || (peak > 0.0 && !codes.iter().any(|q| *q == 127 || *q == 129))
                        { return Err(Error::InvalidInput); }
                        start += 4 + contract.channels();
                    }
                }
            }
        }
        let mut retained = Vec::new(); retained.try_reserve_exact(body.len()).map_err(|_| Error::Limit)?;
        retained.extend_from_slice(body);
        Ok(Self { data: Rc::new(QuantizedData { policy: expected, descriptor, layout, body: retained }) })
    }
}
fn encode_group(words: &[u32], body: &mut Vec<u8>, error: &mut QuantizationError) -> Result<(), Error> {
    let mut peak = 0.0_f32;
    for word in words {
        let value = f32::from_bits(*word);
        if !value.is_finite() { return Err(Error::InvalidInput); }
        peak = peak.max(value.abs());
    }
    body.extend_from_slice(&peak.to_bits().to_be_bytes());
    for word in words {
        let value = f32::from_bits(*word);
        let code = if peak == 0.0 { 0_i8 }
            else { ((f64::from(value) / f64::from(peak) * 127.0).round().clamp(-127.0, 127.0)) as i8 };
        let code = code.to_ne_bytes()[0];
        body.push(code);
        error.record(value, reconstruct(peak, code));
    }
    Ok(())
}
fn reconstruct(peak: f32, code: u8) -> f32 {
    let q = i8::from_ne_bytes([code]);
    if q == 0 { 0.0 } else { (f64::from(peak) * f64::from(q) / 127.0) as f32 }
}
fn read32(bytes: &[u8], start: usize) -> Result<u32, Error> {
    Ok(u32::from_be_bytes(bytes.get(start..start + 4).ok_or(Error::Incomplete)?.try_into().map_err(|_| Error::Incomplete)?))
}
fn read64(bytes: &[u8], start: usize) -> Result<u64, Error> {
    Ok(u64::from_be_bytes(bytes.get(start..start + 8).ok_or(Error::Incomplete)?.try_into().map_err(|_| Error::Incomplete)?))
}
