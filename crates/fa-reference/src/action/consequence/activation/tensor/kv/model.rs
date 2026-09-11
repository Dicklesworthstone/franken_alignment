//! All-or-none capture over an explicitly registered inventory of KV layers.
//!
//! Completeness is relative to this inventory, not inferred from a model name.
//! Host temporal coherence and buffer identities are still trusted inputs. No
//! model executes here, and no cache image contains an actor's effect authority.

mod archive;
mod restore;
pub use archive::{
    ModelKvDescriptor, MAX_MODEL_DESCRIPTOR_BYTES, MAX_MODEL_IMAGE_BYTES,
    MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
};
pub use restore::{LayerRestore, LayerRestoreReceipt, ModelKvRestoreReceipt, PreparedModelKvRestore};

use super::{KvAppend, KvAppendReceipt, KvBudget, KvCapture, KvContract, MAX_KV_POSITIONS, MAX_KV_VALUES};
use super::image::KvImage;
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_MODEL_KV_LAYERS: usize = 128;
pub const MAX_MODEL_KV_VALUES: usize = 16_777_216;
pub const MAX_MODEL_BUFFER_IDENTITIES: usize = 4096;

/// A frozen, nonempty roster. Each layer has distinct K/V tap identities and
/// all layers inhabit the same tenant/model/generation. Physical layouts and
/// scalar formats may differ. This does not authenticate a host's inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelKvProfile {
    id: u64,
    generation: u64,
    layers: BTreeMap<u64, KvContract>,
    values_per_token: usize,
    bytes_per_token: usize,
}

impl ModelKvProfile {
    pub fn new(id: u64, generation: u64, layers: BTreeMap<u64, KvContract>) -> Result<Self, Error> {
        if id == 0 || generation == 0 || layers.is_empty() || layers.contains_key(&0) {
            return Err(Error::InvalidInput);
        }
        if layers.len() > MAX_MODEL_KV_LAYERS { return Err(Error::Limit); }
        let first = layers.values().next().expect("nonempty roster").keys().profile();
        let mut taps = BTreeSet::new();
        let mut values_per_token = 0_usize;
        let mut bytes_per_token = 0_usize;
        for layer in layers.values() {
            for tensor in [layer.keys(), layer.values()] {
                let profile = tensor.profile();
                if (profile.tenant, profile.model, profile.model_generation)
                    != (first.tenant, first.model, first.model_generation)
                { return Err(Error::Binding); }
                if !taps.insert(profile.tap) { return Err(Error::Duplicate); }
                values_per_token = values_per_token.checked_add(tensor.dimensions()).ok_or(Error::Overflow)?;
                bytes_per_token = bytes_per_token.checked_add(
                    tensor.dimensions().checked_mul(tensor.encoding().bytes()).ok_or(Error::Overflow)?
                ).ok_or(Error::Overflow)?;
            }
        }
        if values_per_token > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        Ok(Self { id, generation, layers, values_per_token, bytes_per_token })
    }

    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn layers(&self) -> &BTreeMap<u64, KvContract> { &self.layers }
    pub fn values_per_token(&self) -> usize { self.values_per_token }
    pub fn bytes_per_token(&self) -> usize { self.bytes_per_token }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelKvBudget {
    pub positions: usize,
    /// Aggregate across ALL layers, in addition to each original layer's cap.
    pub normalized_values: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelKvAppendReceipt {
    pub revision: u64,
    pub first_position: u64,
    pub next_position: u64,
    pub first_sequence: u64,
    pub next_sequence: u64,
    pub token_count: usize,
    pub normalized_values: usize,
    pub source_bytes_read: usize,
    pub layers: BTreeMap<u64, KvAppendReceipt>,
}

/// No mutable layer accessor exists: a layer cannot independently advance the
/// shared frontier. Each append stages every layer through KvCapture's original
/// capture path before publishing any. Failed work is not a successful read cost.
pub struct ModelKvCapture {
    profile: Rc<ModelKvProfile>,
    layers: BTreeMap<u64, KvCapture>,
    budget: ModelKvBudget,
    revision: u64,
    positions: usize,
    next_position: u64,
    next_sequence: u64,
    normalized_values: usize,
    source_bytes_read: usize,
    generations: BTreeMap<u64, u64>,
    receipts: Vec<ModelKvAppendReceipt>,
}

impl fmt::Debug for ModelKvCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelKvCapture").field("profile", &self.profile.id)
            .field("revision", &self.revision).field("positions", &self.positions)
            .field("normalized_values", &self.normalized_values).finish_non_exhaustive()
    }
}

impl ModelKvCapture {
    pub fn new(
        profile: ModelKvProfile, stream: u64, batch: usize, first_position: u64,
        first_sequence: u64, budget: ModelKvBudget,
    ) -> Result<Self, Error> {
        if stream == 0 || first_sequence == 0 || budget.positions == 0 || budget.normalized_values == 0 {
            return Err(Error::InvalidInput);
        }
        if budget.positions > MAX_KV_POSITIONS || budget.normalized_values > MAX_MODEL_KV_VALUES
            || budget.normalized_values < profile.values_per_token
        { return Err(Error::Limit); }
        let mut layers = BTreeMap::new();
        for (id, contract) in &profile.layers {
            layers.insert(*id, KvCapture::new(contract.clone(), stream, batch, first_position, first_sequence,
                KvBudget { positions: budget.positions, normalized_values: budget.normalized_values.min(MAX_KV_VALUES) })?);
        }
        Ok(Self { profile: Rc::new(profile), layers, budget, revision: 0, positions: 0,
            next_position: first_position, next_sequence: first_sequence, normalized_values: 0,
            source_bytes_read: 0, generations: BTreeMap::new(), receipts: Vec::new() })
    }

    pub fn profile(&self) -> &ModelKvProfile { &self.profile }
    pub fn layer(&self, id: u64) -> Result<&KvCapture, Error> { self.layers.get(&id).ok_or(Error::Missing) }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn len(&self) -> usize { self.positions }
    pub fn is_empty(&self) -> bool { self.positions == 0 }
    pub fn next_position(&self) -> u64 { self.next_position }
    pub fn next_sequence(&self) -> u64 { self.next_sequence }
    pub fn normalized_values(&self) -> usize { self.normalized_values }
    pub fn source_bytes_read(&self) -> usize { self.source_bytes_read }
    pub fn receipts(&self) -> &[ModelKvAppendReceipt] { &self.receipts }

    pub fn append(
        &mut self, expected_revision: u64, requests: BTreeMap<u64, KvAppend<'_>>,
    ) -> Result<ModelKvAppendReceipt, Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if !requests.keys().eq(self.layers.keys()) { return Err(Error::Binding); }
        let count = requests.values().next().expect("complete nonempty roster").token_count;
        if count == 0 { return Err(Error::InvalidInput); }
        let positions = self.positions.checked_add(count).ok_or(Error::Overflow)?;
        let new_values = self.profile.values_per_token.checked_mul(count).ok_or(Error::Overflow)?;
        let normalized_values = self.normalized_values.checked_add(new_values).ok_or(Error::Overflow)?;
        if positions > self.budget.positions || normalized_values > self.budget.normalized_values {
            return Err(Error::Limit);
        }
        let read_bytes = self.profile.bytes_per_token.checked_mul(count).ok_or(Error::Overflow)?;
        let source_bytes_read = self.source_bytes_read.checked_add(read_bytes).ok_or(Error::Overflow)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let count64 = u64::try_from(count).map_err(|_| Error::Overflow)?;
        let next_position = self.next_position.checked_add(count64).ok_or(Error::Overflow)?;
        let next_sequence = self.next_sequence.checked_add(count64).ok_or(Error::Overflow)?;
        let mut generations = self.generations.clone();
        let mut spans: Vec<super::super::HostTensor<'_>> = Vec::new();
        spans.try_reserve_exact(2 * self.layers.len()).map_err(|_| Error::Limit)?;
        for (id, request) in &requests {
            let first = request.buffer_first_position.checked_add(
                u64::try_from(request.first_token).map_err(|_| Error::Overflow)?
            ).ok_or(Error::Overflow)?;
            if request.token_count != count || first != self.next_position || request.first_sequence != self.next_sequence {
                return Err(Error::Stale);
            }
            let layer = &self.layers[id];
            // Validate every layer's structure and global alias/generation rules
            // before any layer copies numerical values.
            layer.contract().keys().validate_tensor(request.keys)?;
            layer.contract().values().validate_tensor(request.values)?;
            for tensor in [request.keys, request.values] {
                if self.generations.get(&tensor.identity.object).is_some_and(|floor| tensor.identity.generation < *floor) {
                    return Err(Error::Stale);
                }
                for previous in &spans {
                    if previous.identity.object == tensor.identity.object {
                        if previous.identity.generation != tensor.identity.generation
                            || !std::ptr::eq(previous.bytes, tensor.bytes)
                        { return Err(Error::Binding); }
                        let left = previous.layout.byte_range();
                        let right = tensor.layout.byte_range();
                        if left.start < right.end && right.start < left.end { return Err(Error::Binding); }
                    }
                }
                generations.insert(tensor.identity.object, tensor.identity.generation);
                if generations.len() > MAX_MODEL_BUFFER_IDENTITIES { return Err(Error::Limit); }
                spans.push(tensor);
            }
        }
        self.receipts.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut plans = Vec::new();
        plans.try_reserve_exact(self.layers.len()).map_err(|_| Error::Limit)?;
        let mut layer_receipts = BTreeMap::new();
        for (id, layer) in &mut self.layers {
            let plan = layer.prepare_append(expected_revision, requests[id])?;
            layer_receipts.insert(*id, plan.receipt().clone());
            plans.push(plan);
        }
        let receipt = ModelKvAppendReceipt {
            revision, first_position: self.next_position, next_position,
            first_sequence: self.next_sequence, next_sequence, token_count: count,
            normalized_values: new_values, source_bytes_read: read_bytes, layers: layer_receipts,
        };
        let retained_receipt = receipt.clone();
        // Every layer and destination capacity has been staged. No fallible
        // operation/callback now separates layer commits and the shared frontier.
        for plan in plans { plan.commit(); }
        self.revision = revision;
        self.positions = positions;
        self.next_position = next_position;
        self.next_sequence = next_sequence;
        self.normalized_values = normalized_values;
        self.source_bytes_read = source_bytes_read;
        self.generations = generations;
        self.receipts.push(retained_receipt);
        Ok(receipt)
    }

    pub fn snapshot(&self, expected_revision: u64) -> Result<ModelKvImage, Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        let mut layers = BTreeMap::new();
        for (id, layer) in &self.layers { layers.insert(*id, layer.snapshot(expected_revision)?); }
        ModelKvImage::checked(Rc::clone(&self.profile), layers)
    }
}

/// One immutable, complete registered layer set. The image owns no control
/// state and is not a qualified native restart. Clones share arrays and metadata.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::model::ModelKvImage;
/// use fa_reference::action::Permit;
/// fn grant(image: ModelKvImage) -> Permit { image }
/// ```
#[derive(Clone)]
pub struct ModelKvImage {
    profile: Rc<ModelKvProfile>,
    layers: Rc<BTreeMap<u64, KvImage>>,
}

impl fmt::Debug for ModelKvImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelKvImage").field("profile", &self.profile.id)
            .field("layer_count", &self.layers.len()).field("positions", &self.len()).finish_non_exhaustive()
    }
}

impl ModelKvImage {
    /// Useful for independently retained per-layer images. Checks the entire
    /// roster and common cut, not just equal dimensions. Host simultaneity and
    /// authenticity are NOT established by matching supplied identities.
    pub fn from_layers(profile: ModelKvProfile, layers: BTreeMap<u64, KvImage>) -> Result<Self, Error> {
        Self::checked(Rc::new(profile), layers)
    }

    fn checked(profile: Rc<ModelKvProfile>, layers: BTreeMap<u64, KvImage>) -> Result<Self, Error> {
        if !layers.keys().eq(profile.layers.keys()) { return Err(Error::Binding); }
        let first = layers.values().next().ok_or(Error::Incomplete)?.descriptor();
        let count = first.token_count.checked_mul(profile.values_per_token).ok_or(Error::Overflow)?;
        if count > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        for (id, image) in &layers {
            let descriptor = image.descriptor();
            descriptor.encoded_len()?;
            if descriptor.contract != profile.layers[id]
                || (descriptor.stream, descriptor.source_batch, descriptor.first_position,
                    descriptor.first_sequence, descriptor.source_revision, descriptor.token_count)
                    != (first.stream, first.source_batch, first.first_position,
                        first.first_sequence, first.source_revision, first.token_count)
            { return Err(Error::Binding); }
        }
        Ok(Self { profile, layers: Rc::new(layers) })
    }

    pub fn profile(&self) -> &ModelKvProfile { &self.profile }
    pub fn layer(&self, id: u64) -> Result<&KvImage, Error> { self.layers.get(&id).ok_or(Error::Missing) }
    pub fn len(&self) -> usize {
        self.layers.values().next().expect("validated layer set").len()
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn normalized_values(&self) -> usize { self.len() * self.profile.values_per_token }
}
