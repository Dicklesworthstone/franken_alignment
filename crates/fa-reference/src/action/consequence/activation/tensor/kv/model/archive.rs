//! Portable complete-layer images using the existing per-layer scalar format.
//! The independently retained descriptor binds metadata, NOT payload integrity.

use super::{ModelKvImage, ModelKvProfile, MAX_MODEL_KV_LAYERS, MAX_MODEL_KV_VALUES};
use super::super::image::{KvImage, KvImageDescriptor, IMAGE_HEADER_BYTES};
use crate::Error;
use std::collections::BTreeMap;

const DOMAIN: &[u8; 8] = b"FAMKVIM\x01";
pub const MODEL_DESCRIPTOR_HEADER_BYTES: usize = 28;
pub const MODEL_LAYER_DESCRIPTOR_BYTES: usize = 8 + IMAGE_HEADER_BYTES;
pub const MAX_MODEL_DESCRIPTOR_BYTES: usize = MODEL_DESCRIPTOR_HEADER_BYTES
    + MAX_MODEL_KV_LAYERS * MODEL_LAYER_DESCRIPTOR_BYTES;
pub const MAX_MODEL_IMAGE_BYTES: usize = MAX_MODEL_DESCRIPTOR_BYTES + 4 * MAX_MODEL_KV_VALUES;

/// Independent intended-context data, not a signature, hash or restart grade.
/// Only exact declared layer inventories with a common source cut are admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelKvDescriptor {
    profile: ModelKvProfile,
    layers: BTreeMap<u64, KvImageDescriptor>,
}

impl ModelKvDescriptor {
    fn checked(id: u64, generation: u64, layers: BTreeMap<u64, KvImageDescriptor>) -> Result<Self, Error> {
        let contracts = layers.iter().map(|(id, layer)| (*id, layer.contract.clone())).collect();
        let profile = ModelKvProfile::new(id, generation, contracts)?;
        let first = layers.values().next().ok_or(Error::Incomplete)?;
        let total = first.token_count.checked_mul(profile.values_per_token()).ok_or(Error::Overflow)?;
        if total > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        for layer in layers.values() {
            layer.encoded_len()?;
            if (layer.stream, layer.source_batch, layer.first_position, layer.first_sequence,
                layer.source_revision, layer.token_count)
                != (first.stream, first.source_batch, first.first_position, first.first_sequence,
                    first.source_revision, first.token_count)
            { return Err(Error::Binding); }
        }
        Ok(Self { profile, layers })
    }

    pub fn profile(&self) -> &ModelKvProfile { &self.profile }
    pub fn layers(&self) -> &BTreeMap<u64, KvImageDescriptor> { &self.layers }
    pub fn descriptor_len(&self) -> usize {
        MODEL_DESCRIPTOR_HEADER_BYTES + self.layers.len() * MODEL_LAYER_DESCRIPTOR_BYTES
    }

    pub fn image_len(&self) -> Result<usize, Error> {
        let mut length = self.descriptor_len();
        for layer in self.layers.values() {
            let payload = layer.encoded_len()?.checked_sub(IMAGE_HEADER_BYTES).ok_or(Error::Binding)?;
            length = length.checked_add(payload).ok_or(Error::Overflow)?;
        }
        if length > MAX_MODEL_IMAGE_BYTES { return Err(Error::Limit); }
        Ok(length)
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.image_len()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(self.descriptor_len()).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(DOMAIN);
        bytes.extend_from_slice(&self.profile.id().to_be_bytes());
        bytes.extend_from_slice(&self.profile.generation().to_be_bytes());
        bytes.extend_from_slice(&u32::try_from(self.layers.len()).map_err(|_| Error::Limit)?.to_be_bytes());
        for (id, layer) in &self.layers {
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.extend_from_slice(&layer.encode()?);
        }
        if bytes.len() != self.descriptor_len() { return Err(Error::Binding); }
        Ok(bytes)
    }

    /// Strict header-only import. All per-layer and aggregate allocation bounds
    /// and common-cut checks finish before any numerical payload is allocated.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_MODEL_DESCRIPTOR_BYTES { return Err(Error::Limit); }
        if bytes.len() < MODEL_DESCRIPTOR_HEADER_BYTES { return Err(Error::Incomplete); }
        if bytes.get(..8) != Some(DOMAIN.as_slice()) { return Err(Error::InvalidInput); }
        let id = read64(bytes, 8)?;
        let generation = read64(bytes, 16)?;
        let count = usize::try_from(read32(bytes, 24)?).map_err(|_| Error::Limit)?;
        if count == 0 { return Err(Error::InvalidInput); }
        if count > MAX_MODEL_KV_LAYERS { return Err(Error::Limit); }
        let length = MODEL_DESCRIPTOR_HEADER_BYTES + count * MODEL_LAYER_DESCRIPTOR_BYTES;
        if bytes.len() != length { return Err(Error::InvalidInput); }
        let mut layers = BTreeMap::new();
        let mut offset = MODEL_DESCRIPTOR_HEADER_BYTES;
        let mut previous_id = 0;
        for _ in 0..count {
            let layer_id = read64(bytes, offset)?;
            if layer_id <= previous_id { return Err(Error::InvalidInput); }
            previous_id = layer_id;
            offset += 8;
            let layer = KvImageDescriptor::decode(&bytes[offset..offset + IMAGE_HEADER_BYTES])?;
            offset += IMAGE_HEADER_BYTES;
            layers.insert(layer_id, layer);
        }
        let descriptor = Self::checked(id, generation, layers)?;
        descriptor.image_len()?;
        Ok(descriptor)
    }
}

impl ModelKvImage {
    pub fn descriptor(&self) -> ModelKvDescriptor {
        ModelKvDescriptor {
            profile: self.profile.as_ref().clone(),
            layers: self.layers.iter().map(|(id, layer)| (*id, layer.descriptor().clone())).collect(),
        }
    }

    /// One layer's canonical scalar encoding is reused at a time. No new
    /// floating conversion, compression or authentication algorithm is supplied.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let descriptor = self.descriptor();
        let length = descriptor.image_len()?;
        let mut bytes = descriptor.encode()?;
        bytes.try_reserve_exact(length - bytes.len()).map_err(|_| Error::Limit)?;
        for layer in self.layers.values() {
            let encoded = layer.encode()?;
            bytes.extend_from_slice(encoded.get(IMAGE_HEADER_BYTES..).ok_or(Error::Binding)?);
        }
        if bytes.len() != length { return Err(Error::Binding); }
        Ok(bytes)
    }

    /// Matching a descriptor establishes the intended metadata and complete
    /// roster, not origin/authenticity. Finite same-size payload edits can still
    /// decode as different data. Authenticate bytes outside this reference codec.
    pub fn decode(bytes: &[u8], expected: &ModelKvDescriptor) -> Result<Self, Error> {
        if bytes.len() > MAX_MODEL_IMAGE_BYTES { return Err(Error::Limit); }
        let header = bytes.get(..expected.descriptor_len()).ok_or(Error::Incomplete)?;
        let descriptor = ModelKvDescriptor::decode(header)?;
        if &descriptor != expected { return Err(Error::Binding); }
        if bytes.len() != descriptor.image_len()? { return Err(Error::InvalidInput); }
        let mut offset = descriptor.descriptor_len();
        let mut layers = BTreeMap::new();
        for (id, layer) in descriptor.layers() {
            let payload_len = layer.encoded_len()? - IMAGE_HEADER_BYTES;
            let end = offset.checked_add(payload_len).ok_or(Error::Overflow)?;
            let payload = bytes.get(offset..end).ok_or(Error::Incomplete)?;
            let mut framed = layer.encode()?;
            framed.try_reserve_exact(payload_len).map_err(|_| Error::Limit)?;
            framed.extend_from_slice(payload);
            layers.insert(*id, KvImage::decode(&framed, layer)?);
            offset = end;
        }
        if offset != bytes.len() { return Err(Error::InvalidInput); }
        Self::from_layers(descriptor.profile, layers)
    }
}

fn read32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let end = offset.checked_add(4).ok_or(Error::Overflow)?;
    let word = bytes.get(offset..end).ok_or(Error::Incomplete)?;
    Ok(u32::from_be_bytes(word.try_into().map_err(|_| Error::Incomplete)?))
}

fn read64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let end = offset.checked_add(8).ok_or(Error::Overflow)?;
    let word = bytes.get(offset..end).ok_or(Error::Incomplete)?;
    Ok(u64::from_be_bytes(word.try_into().map_err(|_| Error::Incomplete)?))
}
