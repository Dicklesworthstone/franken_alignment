//! Real imported helper models, fixed before durable startup. No supplied votes.
use super::fields::{Assets, Fields, path, MAX_DATA_BYTES};
use super::recipe::{identity, tokenization};
use super::super::tokenizer::TokenizerInput;
use crate::config::{Config, debug};
use fa_reference::action::consequence::activation::monitor::decoder::{
    config::MAX_MONITOR_CONFIG_BYTES,
    sampled::{MonitoredSampledDecoder, config::MAX_SAMPLING_CONFIG_BYTES,
        generation::{GenerationBudget, MAX_STOP_TOKENS, text::TextDecoder,
            tokenizer::{ByteBpe, TokenizationBudget}}},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel, safetensors::{MAX_WEIGHT_FILE_BYTES, MAX_WEIGHT_HEADER_BYTES,
        pretrained::{LlamaConfig, MAX_CONFIG_BYTES}},
};
use fa_reference::action::consequence::oversight::helper_client::native::{NativeEvaluator, NativeHelperPolicy};
use fa_reference::full_input::InputProfileBinding;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) struct NativeRoster { members: BTreeMap<String, NativeModel> }
struct NativeInput {
    identity: fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity,
    context: usize, stream: u64, configuration: PathBuf, weights: PathBuf,
    monitoring: PathBuf, sampling: PathBuf, tokenizer: TokenizerInput,
    max_new_tokens: usize, max_output_bytes: usize, stop_tokens: Vec<u32>,
    tokenization: TokenizationBudget, generation: GenerationBudget,
}
struct NativeModel {
    model: DecoderModel, tokenizer: ByteBpe, monitoring: Vec<u8>, sampling: Vec<u8>,
    stream: u64, max_new_tokens: usize, max_output_bytes: usize, stop_tokens: Vec<u32>,
    tokenization: TokenizationBudget, generation: GenerationBudget,
}
impl NativeRoster {
    pub(super) fn load(file: &Path, config: &Config, assets: &mut Assets) -> Result<Self, String> {
        let mut root = Fields::parse(&assets.read(file, MAX_DATA_BYTES)?, MAX_DATA_BYTES)?;
        if root.text("schema")? != "fa.learned-native-roster/1" {
            return Err("unsupported learned native roster".into());
        }
        let members = root.object("members")?.entries();
        root.end()?;
        if !members.keys().eq(config.profile.committee.members().keys()) {
            return Err("native model roster must cover the exact independently configured congress".into());
        }
        // Finish closed schema/path admission for ALL members before model reads.
        let inputs = members.into_iter().map(|(name, value)| Ok((name, NativeInput::decode(Fields::new(value)?)?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let mut members = BTreeMap::new();
        for (name, input) in inputs {
            let model = input.load(assets)?;
            // This original empty evaluator validates full tokenizer/control,
            // monitor and sampling contracts before an authority store exists.
            // Epoch zero is just the independently registered provisional input
            // profile: the real review binds its actual action epoch below.
            drop(model.build(config.profile.committee.members()[&name].profile_at(0))?);
            members.insert(name, model);
        }
        Ok(Self { members })
    }

    pub(super) fn check_limits(&self, limits: &fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::native::NativeReviewLimits,
        rounds: usize)
        -> Result<(), String>
    {
        let mut products = 0_u64;
        let mut entries = 0_u64;
        for model in self.members.values() {
            products = products.checked_add(model.generation.scalar_products).ok_or("native product reservation overflow")?;
            entries = entries.checked_add(model.generation.sampling_entries).ok_or("native sampler reservation overflow")?;
        }
        // Every future evaluator is provisioned before the first answer. An
        // unused or cancelled round cannot refund its admitted reservation.
        let evaluations = self.members.len().checked_mul(rounds).ok_or("native evaluation reservation overflow")?;
        products = products.checked_mul(rounds as u64).ok_or("native product reservation overflow")?;
        entries = entries.checked_mul(rounds as u64).ok_or("native sampler reservation overflow")?;
        if rounds == 0 || evaluations > limits.native.evaluations || products > limits.native.scalar_products
            || entries > limits.native.sampling_entries {
            return Err("native review allowance cannot admit the complete registered model roster".into());
        }
        Ok(())
    }

    pub(super) fn evaluator(&self, name: &str, profile: InputProfileBinding) -> Result<NativeEvaluator, String> {
        self.members.get(name).ok_or("missing registered native model")?.build(profile)
    }
}
impl NativeInput {
    fn decode(mut fields: Fields) -> Result<Self, String> {
        let mut model = fields.object("model")?;
        let identity = identity(model.object("identity")?)?;
        let context = model.size("context")?;
        let stream = model.number("stream")?;
        model.end()?;
        let mut files = fields.object("files")?;
        let configuration = files.path("model_config")?;
        let weights = files.path("weights")?;
        let monitoring = files.path("monitor")?;
        let sampling = files.path("sampling")?;
        let tokenizer = TokenizerInput::parse(files.take("tokenizer")?, true)?;
        path(tokenizer.path())?;
        files.end()?;
        let mut text = fields.object("text")?;
        let max_new_tokens = text.size("max_new_tokens")?;
        let max_output_bytes = text.size("max_output_bytes")?;
        let stop_tokens = text.ids("stop_tokens", MAX_STOP_TOKENS)?;
        let tokenization = tokenization(text.object("tokenization")?)?;
        let generation = GenerationBudget { scalar_products: text.number("scalar_products")?,
            sampling_entries: text.number("sampling_entries")? };
        text.end()?; fields.end()?;
        if stream == 0 || max_new_tokens == 0 || stop_tokens.is_empty() || max_output_bytes == 0 {
            return Err("native review requires explicit finite generation and control stops".into());
        }
        Ok(Self { identity, context, stream, configuration, weights, monitoring, sampling,
            tokenizer, max_new_tokens, max_output_bytes, stop_tokens, tokenization, generation })
    }
    fn load(self, assets: &mut Assets) -> Result<NativeModel, String> {
        let configuration = assets.read(&self.configuration, MAX_CONFIG_BYTES)?;
        let negotiated = LlamaConfig::decode(self.identity, self.context, &configuration).map_err(debug)?;
        let tokenizer = self.tokenizer.decode(negotiated.profile(),
            &assets.read(Path::new(self.tokenizer.path()), self.tokenizer.byte_limit())?)?;
        let monitoring = assets.read(&self.monitoring, MAX_MONITOR_CONFIG_BYTES)?;
        let sampling = assets.read(&self.sampling, MAX_SAMPLING_CONFIG_BYTES)?;
        let bound = (8 + MAX_WEIGHT_HEADER_BYTES + 4 * negotiated.profile().parameter_count())
            .min(MAX_WEIGHT_FILE_BYTES);
        let weights = assets.read(&self.weights, bound)?;
        let (model, _) = DecoderModel::from_llama_safetensors(self.identity, self.context,
            &configuration, &weights).map_err(debug)?;
        Ok(NativeModel { model, tokenizer, monitoring, sampling, stream: self.stream,
            max_new_tokens: self.max_new_tokens, max_output_bytes: self.max_output_bytes,
            stop_tokens: self.stop_tokens, tokenization: self.tokenization, generation: self.generation })
    }
}
impl NativeModel {
    fn build(&self, input_profile: InputProfileBinding) -> Result<NativeEvaluator, String> {
        let policy = NativeHelperPolicy { input_profile, decoder_profile: self.model.profile().clone(),
            max_new_tokens: self.max_new_tokens, stop_tokens: self.stop_tokens.clone(),
            tokenization: self.tokenization, generation: self.generation, max_output_bytes: self.max_output_bytes };
        let decoder = MonitoredSampledDecoder::from_json(self.model.clone(), self.stream,
            &self.monitoring, &self.sampling).map_err(debug)?;
        NativeEvaluator::new(TextDecoder::new(decoder, self.tokenizer.clone()).map_err(debug)?, policy).map_err(debug)
    }
}
