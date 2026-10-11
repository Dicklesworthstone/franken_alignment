//! Independently repackage the original causal-adapter fixture into actual files.
use super::{assets, lora_assets, v2_assets};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::NativeTokenizerFormat;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{NativeAssetReadBudget, NativeHelperFileLimits};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::lora::NativeLoraShardFileBootstrap;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::sharded::{NativeHelperShardFiles, NativeShardFileBootstrap};
use fa_reference::strict_json::{self, Limits};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn frame(prompt: &[u8]) -> Vec<u8> { lora_assets::frame(prompt) }
pub fn base_identity() -> DecoderIdentity {
    DecoderIdentity { tenant: 1, model: 2, model_generation: 3, tokenizer_generation: 4, profile_generation: 5 }
}
pub fn assets_budget() -> NativeAssetReadBudget { NativeAssetReadBudget::for_shards(1_048_576, 4096).unwrap() }
pub fn weights_budget() -> WeightReadBudget { WeightReadBudget::new(1_048_576, 4096).unwrap() }
pub fn header_bytes(bytes: &[u8]) -> usize { 8 + u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize }

pub struct Fixture {
    pub inner: assets::Fixture,
    pub configuration: PathBuf, pub tokenizer: PathBuf, pub monitoring: PathBuf, pub sampling: PathBuf,
    pub index: PathBuf, pub shards: BTreeMap<String, PathBuf>,
    pub adapter_config: PathBuf, pub adapter_weights: PathBuf, pub format: NativeTokenizerFormat,
}
impl Fixture {
    pub fn new(rank: usize, zero: bool, json: bool, alarm: bool) -> Self {
        let mut inner = assets::Fixture::new(alarm); lora_assets::configure(&mut inner, rank, zero, json);
        let file = fs::read(inner.root.join("weights.safetensors")).unwrap();
        let header_end = header_bytes(&file);
        let header = strict_json::parse(&file[8..header_end], Limits::default()).unwrap();
        let body = &file[header_end..]; let mut shards = BTreeMap::new(); let mut mapping = Vec::new();
        let quote = v2_assets::quote;
        for part in 0..2 {
            let label = format!("base-{part}.safetensors");
            let mut entries = Vec::new(); let mut packed = Vec::new();
            for (name, tensor) in header.as_object().unwrap() {
                if name.starts_with("model.layers.") != (part == 1) { continue; }
                let offsets = tensor.get("data_offsets").unwrap().as_array().unwrap();
                let begin = offsets[0].as_u64().unwrap() as usize; let end = offsets[1].as_u64().unwrap() as usize;
                let start = packed.len(); packed.extend_from_slice(&body[begin..end]);
                let shape = tensor.get("shape").unwrap().as_array().unwrap().iter().map(|v| v.as_u64().unwrap().to_string()).collect::<Vec<_>>().join(",");
                let dtype = tensor.get("dtype").unwrap().as_str().unwrap();
                entries.push(format!("{}:{{\"dtype\":{},\"shape\":[{shape}],\"data_offsets\":[{start},{}]}}", quote(name), quote(dtype), packed.len()));
                mapping.push(format!("{}:{}", quote(name), quote(&label)));
            }
            assert!(!entries.is_empty());
            let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
            while !header.len().is_multiple_of(8) { header.push(b' '); }
            let mut bytes = (header.len() as u64).to_le_bytes().to_vec(); bytes.extend_from_slice(&header); bytes.extend_from_slice(&packed);
            let path = inner.root.join(format!("registered-base-{part}.bin")); fs::write(&path, bytes).unwrap();
            shards.insert(label, path);
        }
        let index = inner.root.join("registered-index.json");
        fs::write(&index, format!("{{\"weight_map\":{{{}}}}}", mapping.join(","))).unwrap();
        let declared = shards.iter().map(|(label, path)| format!("{}:{}", quote(label), quote(path.to_str().unwrap()))).collect::<Vec<_>>().join(",");
        let previous = format!("\"weights\":{{\"kind\":\"single\",\"path\":{}}}", quote(inner.root.join("weights.safetensors").to_str().unwrap()));
        let replacement = format!("\"weights\":{{\"kind\":\"sharded\",\"index\":{},\"shards\":{{{declared}}}}}", quote(index.to_str().unwrap()));
        assert!(inner.manifest.contains(&previous)); inner.manifest = inner.manifest.replacen(&previous, &replacement, 1); inner.save();
        // A fallback to the old monolithic path can no longer pass a positive.
        fs::remove_file(inner.root.join("weights.safetensors")).unwrap();
        Self { configuration: inner.root.join("config.json"), tokenizer: inner.root.join("tokenizer.bin"),
            monitoring: inner.root.join("monitor.json"), sampling: inner.root.join("sampling.json"),
            adapter_config: inner.root.join("adapter_config.json"), adapter_weights: inner.root.join("adapter.safetensors"),
            format: if json { NativeTokenizerFormat::HuggingFaceRawByteLevel } else { NativeTokenizerFormat::NativeArchive },
            index, shards, inner }
    }
    pub fn request(&self) -> NativeLoraShardFileBootstrap<'_> {
        NativeLoraShardFileBootstrap { base: NativeShardFileBootstrap {
            policy: &self.inner.policy, stream: 12,
            files: NativeHelperShardFiles { configuration: &self.configuration, tokenizer: &self.tokenizer,
                monitoring: &self.monitoring, sampling: &self.sampling, index: &self.index, shards: &self.shards },
            limits: NativeHelperFileLimits::default(), index_bytes: 65_536 },
            base_identity: base_identity(), adapter_configuration: &self.adapter_config,
            adapter_weights: &self.adapter_weights, adapter_weight_bytes: 1_048_576 }
    }
    pub fn base_bytes(&self) -> usize { self.shards.values().map(|path| fs::metadata(path).unwrap().len() as usize).sum() }
    pub fn weight_bytes(&self) -> usize { self.base_bytes() + fs::metadata(&self.adapter_weights).unwrap().len() as usize }
    pub fn asset_bytes(&self) -> usize { [&self.configuration, &self.tokenizer, &self.monitoring,
        &self.sampling, &self.index, &self.adapter_config].into_iter().map(|path| fs::metadata(path).unwrap().len() as usize).sum() }
    pub fn all_asset_paths(&self) -> Vec<&Path> {
        [&self.configuration, &self.tokenizer, &self.monitoring, &self.sampling, &self.index,
            &self.adapter_config, &self.adapter_weights].into_iter().chain(self.shards.values()).map(PathBuf::as_path).collect()
    }
}
