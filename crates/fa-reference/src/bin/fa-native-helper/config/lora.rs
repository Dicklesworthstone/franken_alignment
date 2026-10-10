//! Explicit /4 adapter declaration; no adapter is inferred from files or names.
use super::{LaunchError, Manifest, Json, DecoderIdentity, DecoderProfile, object, path, u64_field};
use super::checkpoint::CheckpointFiles;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::lora::{
    LoraMergeBudget, MAX_LORA_WEIGHT_BYTES, adapted_profile,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluator;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{
    NativeAssetReadBudget, NativeFileBootstrap, NativeHelperFiles, NativeHelperFileLimits,
};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::lora::NativeLoraFileBootstrap;
use std::path::PathBuf;

pub(super) struct LoraFiles {
    base_identity: DecoderIdentity,
    configuration: PathBuf,
    weights: PathBuf,
    merge_products: u64,
}
pub(super) fn parse(value: &Json, expected: &DecoderProfile) -> Result<LoraFiles, LaunchError> {
    let object = object(value, &["base_identity", "configuration", "weights", "merge_products"], "adapter")?;
    let id = super::object(&object["base_identity"],
        &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "adapter.base_identity")?;
    let identity = DecoderIdentity { tenant: u64_field(id, "tenant")?, model: u64_field(id, "model")?,
        model_generation: u64_field(id, "model_generation")?,
        tokenizer_generation: u64_field(id, "tokenizer_generation")?,
        profile_generation: u64_field(id, "profile_generation")? };
    let base = DecoderProfile::new(identity, expected.shape(), expected.epsilon(), expected.theta())
        .and_then(|profile| profile.with_rotary_scaling(expected.rotary_scaling())).map_err(LaunchError::Contract)?;
    if adapted_profile(&base, expected.identity()).is_err() {
        return Err(LaunchError::Field("adapter.base_identity"));
    }
    let merge_products = u64_field(object, "merge_products")?;
    LoraMergeBudget::new(merge_products).map_err(|_| LaunchError::Limit)?;
    Ok(LoraFiles { base_identity: identity,
        configuration: path(&object["configuration"], "adapter.configuration")?,
        weights: path(&object["weights"], "adapter.weights")?, merge_products })
}
impl Manifest {
    pub(super) fn load_lora(&self, adapter: &LoraFiles, assets: &mut NativeAssetReadBudget,
        weights: &mut WeightReadBudget) -> Result<NativeEvaluator, LaunchError>
    {
        let CheckpointFiles::Single(weight_path) = &self.checkpoint else {
            return Err(LaunchError::Field("adapter.single_base"));
        };
        // Constructed inside the one original lifetime, never per helper packet.
        let mut products = LoraMergeBudget::new(adapter.merge_products).map_err(|_| LaunchError::Limit)?;
        let base = NativeFileBootstrap { policy: &self.policy, stream: self.stream,
            files: NativeHelperFiles { configuration: &self.files[0], tokenizer: &self.files[1],
                monitoring: &self.files[2], sampling: &self.files[3], weights: weight_path },
            limits: NativeHelperFileLimits::default() };
        NativeEvaluator::from_llama_lora_files(NativeLoraFileBootstrap { base,
            base_identity: adapter.base_identity, adapter_configuration: &adapter.configuration,
            adapter_weights: &adapter.weights, adapter_weight_bytes: MAX_LORA_WEIGHT_BYTES },
            assets, weights, &mut products, self.tokenizer_format)
            .map(|(evaluator, _)| evaluator).map_err(LaunchError::LoraStartup)
    }
}
