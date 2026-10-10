//! One static plain-LoRA merge into a fresh ORIGINAL dense decoder.
//! No training, adapter runtime, base-model lookup, live mutation or effect authority.
//! Profiles declare identity; authenticating the base and adapter remains a host duty.
mod config;
mod merge;
pub use config::LoraConfig;

use super::{MAX_WEIGHT_HEADER_BYTES, MAX_WEIGHT_TENSORS};
use super::reader::{self, WeightReadBudget, WeightReadError, MAX_WEIGHT_READ_CALLS};
use super::shards::ShardLoad;
use super::pretrained::ConfigIssue;
use super::super::{DecoderIdentity, DecoderModel, DecoderProfile};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Cursor, Read};

pub const MAX_LORA_CONFIG_BYTES: usize = 65_536;
pub const MAX_LORA_RANK: usize = 64;
pub const MAX_LORA_PARAMETERS: usize = 1_048_576;
pub const MAX_LORA_PRODUCTS: u64 = 1_073_741_824;
pub const MAX_LORA_WEIGHT_BYTES: usize = 8 + MAX_WEIGHT_HEADER_BYTES + 4 * MAX_LORA_PARAMETERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoraTarget { Query, Key, Value, AttentionOutput, Gate, Up, Down }
impl LoraTarget {
    pub fn name(self) -> &'static str {
        match self { Self::Query => "q_proj", Self::Key => "k_proj", Self::Value => "v_proj",
            Self::AttentionOutput => "o_proj", Self::Gate => "gate_proj", Self::Up => "up_proj",
            Self::Down => "down_proj" }
    }
    fn dimensions(self, profile: &DecoderProfile) -> (usize, usize) {
        let s = profile.shape();
        match self {
            Self::Query | Self::AttentionOutput => (s.hidden, s.hidden),
            Self::Key | Self::Value => (profile.cache_width(), s.hidden),
            Self::Gate | Self::Up => (s.intermediate, s.hidden),
            Self::Down => (s.hidden, s.intermediate),
        }
    }
    fn prefix(self, layer: usize) -> String {
        let block = match self { Self::Gate | Self::Up | Self::Down => "mlp", _ => "self_attn" };
        format!("base_model.model.model.layers.{layer}.{block}.{}", self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoraError {
    Configuration { field: &'static str, issue: ConfigIssue },
    Identity,
    Limit,
    Weights(WeightReadError),
    Model(Error),
}
impl fmt::Display for LoraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for LoraError {}

/// Planned B*A terms, not FLOPs, latency, actual allocations or publication rights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoraMergeWork {
    pub scalar_products: u64,
    pub updated_parameters: usize,
    pub adapter_parameters: usize,
}

/// Shared, nonrefundable reservation allowance. Full planned arithmetic is charged
/// after complete adapter scalar/EOF admission and BEFORE model allocation/math.
/// A late allocation/nonfinite failure retains this charge. An invalid header
/// performs no merge arithmetic; its original reader usage remains consumed.
#[derive(Debug)]
pub struct LoraMergeBudget { limit: u64, reserved: u64 }
impl LoraMergeBudget {
    pub fn new(scalar_products: u64) -> Result<Self, LoraError> {
        if scalar_products == 0 || scalar_products > MAX_LORA_PRODUCTS { return Err(LoraError::Limit); }
        Ok(Self { limit: scalar_products, reserved: 0 })
    }
    pub fn reserved_products(&self) -> u64 { self.reserved }
    pub fn remaining_products(&self) -> u64 { self.limit - self.reserved }
    pub fn require(&self, work: LoraMergeWork) -> Result<(), LoraError> {
        if work.scalar_products == 0 || work.scalar_products > self.remaining_products() {
            return Err(LoraError::Limit);
        }
        Ok(())
    }
    fn reserve(&mut self, work: LoraMergeWork) -> Result<(), LoraError> {
        self.require(work)?;
        self.reserved += work.scalar_products;
        Ok(())
    }
}

/// Interpretation/cost receipt, not parameter authentication or training lineage.
/// Complete merged weights remain in the original immutable model and therefore
/// in every original recipe binding that binds all parameters, even unused ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoraMergeReceipt {
    pub base_profile: DecoderProfile,
    pub adapted_profile: DecoderProfile,
    pub configuration: LoraConfig,
    pub adapter: ShardLoad,
    pub work: LoraMergeWork,
}

/// Require a new generation in the same model/tokenizer lineage. Merely changing
/// a display/model/tokenizer label is insufficient: BOTH model and numerical
/// profile generations strictly increase, including for a mathematically zero
/// delta. Original capture/probe/cache contracts therefore become incompatible.
pub fn adapted_profile(base: &DecoderProfile, identity: DecoderIdentity) -> Result<DecoderProfile, LoraError> {
    let old = base.identity();
    if identity.tenant != old.tenant || identity.model != old.model
        || identity.tokenizer_generation != old.tokenizer_generation
        || identity.model_generation <= old.model_generation
        || identity.profile_generation <= old.profile_generation
    { return Err(LoraError::Identity); }
    DecoderProfile::new(identity, base.shape(), base.epsilon(), base.theta())
        .and_then(|profile| profile.with_rotary_scaling(base.rotary_scaling()))
        .map_err(LoraError::Model)
}

impl LoraConfig {
    pub fn estimate(&self, profile: &DecoderProfile) -> Result<LoraMergeWork, LoraError> {
        let mut work = LoraMergeWork::default();
        let layers = profile.shape().layers;
        if layers.checked_mul(self.targets().len()).and_then(|n| n.checked_mul(2))
            .is_none_or(|n| n > MAX_WEIGHT_TENSORS) { return Err(LoraError::Limit); }
        for target in self.targets() {
            let (output, input) = target.dimensions(profile);
            let updated = output.checked_mul(input).and_then(|n| n.checked_mul(layers)).ok_or(LoraError::Limit)?;
            let parameters = output.checked_add(input).and_then(|n| n.checked_mul(self.rank()))
                .and_then(|n| n.checked_mul(layers)).ok_or(LoraError::Limit)?;
            work.updated_parameters = work.updated_parameters.checked_add(updated).ok_or(LoraError::Limit)?;
            work.adapter_parameters = work.adapter_parameters.checked_add(parameters).ok_or(LoraError::Limit)?;
            let products = (updated as u64).checked_mul(self.rank() as u64).ok_or(LoraError::Limit)?;
            work.scalar_products = work.scalar_products.checked_add(products).ok_or(LoraError::Limit)?;
        }
        if work.adapter_parameters > MAX_LORA_PARAMETERS || work.scalar_products > MAX_LORA_PRODUCTS {
            return Err(LoraError::Limit);
        }
        Ok(work)
    }
    fn inventory(&self, profile: &DecoderProfile) -> BTreeMap<String, Vec<usize>> {
        let mut tensors = BTreeMap::new();
        for layer in 0..profile.shape().layers {
            for target in self.targets() {
                let (output, input) = target.dimensions(profile);
                let prefix = target.prefix(layer);
                tensors.insert(format!("{prefix}.lora_A.weight"), vec![self.rank(), input]);
                tensors.insert(format!("{prefix}.lora_B.weight"), vec![output, self.rank()]);
            }
        }
        tensors
    }
}

impl DecoderModel {
    /// Merge a single complete saved adapter. The base remains unchanged, including
    /// all existing sessions/checkpoints. There is no hidden identity increment.
    /// The new owner is not exposed unless every parameter and budget is admitted.
    pub fn with_lora_safetensors(&self, identity: DecoderIdentity, configuration: &[u8],
        adapter: &[u8], products: &mut LoraMergeBudget) -> Result<(Self, LoraMergeReceipt), LoraError>
    {
        if adapter.len() > MAX_LORA_WEIGHT_BYTES { return Err(LoraError::Limit); }
        let mut source = Cursor::new(adapter);
        let mut reads = WeightReadBudget::new(adapter.len().checked_add(1).ok_or(LoraError::Limit)?,
            MAX_WEIGHT_READ_CALLS).map_err(|error| LoraError::Weights(WeightReadError::Refused(error)))?;
        self.read_lora_safetensors(identity, configuration, &mut source, &mut reads, products)
    }

    /// The original scalar reader, header coverage and EOF checks; F32/F16/BF16
    /// are normalized exactly as other checkpoint inputs. Identity/configuration/
    /// aggregate merge-work checks precede any adapter read. Reader positions and
    /// BOTH caller-owned allowances survive failures without hidden retry/reset.
    pub fn read_lora_safetensors<R: Read + ?Sized>(&self, identity: DecoderIdentity,
        configuration: &[u8], source: &mut R, reads: &mut WeightReadBudget,
        products: &mut LoraMergeBudget) -> Result<(Self, LoraMergeReceipt), LoraError>
    {
        let profile = adapted_profile(self.profile(), identity)?;
        let configuration = LoraConfig::decode(configuration)?;
        let work = configuration.estimate(self.profile())?;
        products.require(work)?;
        let maximum = 8 + MAX_WEIGHT_HEADER_BYTES + 4 * work.adapter_parameters;
        let (parameters, adapter) = reader::read_tensor_set(
            &configuration.inventory(self.profile()), source, reads, maximum)
            .map_err(LoraError::Weights)?;
        products.reserve(work)?;
        let model = merge::construct(self, profile.clone(), &configuration, parameters)?;
        Ok((model, LoraMergeReceipt { base_profile: self.profile().clone(),
            adapted_profile: profile, configuration, adapter, work }))
    }
}
