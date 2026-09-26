//! Bounded operator input; the original model/tokenizer constructors own semantics.
use super::*;
use crate::config::read_regular;
use fa_reference::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::config::MAX_SAMPLING_CONFIG_BYTES;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_GENERATION_TOKENS, MAX_SAMPLING_ENTRIES,
    text::{TextGenerationRequest, MAX_PREFIX_CONTROLS},
    tokenizer::{ByteBpe, TokenizationBudget, MAX_INPUT_BYTES},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{
    MAX_WEIGHT_FILE_BYTES, pretrained::{LlamaConfig, MAX_CONFIG_BYTES},
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::{
    FileDecoderConfig, text::{FileTextGenerationCommand, MAX_FILE_TOKENIZER_BYTES},
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::path::PathBuf;

const MAX_RECIPE_BYTES: usize = 16_384;

pub(super) struct Inputs {
    pub decoder: FileDecoderConfig,
    pub tokenizer: ByteBpe,
    pub stream: StreamProfile,
    pub generation: u64,
    pub request: TextGenerationRequest,
}

/// No output, verdict, cursor, key or callback can be supplied in this document.
pub(super) fn load(path: &Path, tenant: u64, byte_limit: usize) -> Result<Inputs, String> {
    let bytes = read_regular(path, MAX_RECIPE_BYTES.min(byte_limit))?;
    let mut f = Fields::new(strict_json::parse(&bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 4, max_items: 1024, max_string_bytes: 4096,
    }).map_err(debug)?)?;
    if f.text("schema")? != "fa.native-text-service/1" { return Err("unsupported native service recipe".into()); }
    let mut ids = Fields::new(f.take("identity")?)?;
    let identity = DecoderIdentity { tenant: ids.number("tenant")?, model: ids.number("model")?,
        model_generation: ids.number("model_generation")?, tokenizer_generation: ids.number("tokenizer_generation")?,
        profile_generation: ids.number("profile_generation")? };
    ids.end()?;
    if identity.tenant != tenant { return Err("native model belongs to a different tenant".into()); }
    let context = f.count("context")?;
    let cache_stream = f.number("cache_stream")?;
    let generation = f.number("generation")?;
    let mut stream = Fields::new(f.take("stream")?)?;
    let id = stream.number("id")?;
    let epoch = stream.number("generation")?;
    let messages = stream.count("max_messages")?;
    let message_bytes = stream.count("max_message_bytes")?;
    let total_bytes = stream.count("max_total_bytes")?;
    stream.end()?;
    let stream = StreamProfile::new(id, epoch, messages, message_bytes, total_bytes).map_err(debug)?;
    let prefix_controls = f.tokens("prefix_controls", MAX_PREFIX_CONTROLS)?;
    let stop_tokens = f.tokens("stop_tokens", 256)?;
    let max_new_tokens = f.count("max_new_tokens")?;
    let max_output_bytes = f.count("max_output_bytes")?;
    let products = f.number("scalar_products")?;
    let sampling = f.number("sampling_entries")?;
    let names = ["model_config", "weights", "monitor", "sampling", "tokenizer", "prompt"];
    let paths: Vec<_> = names.iter().map(|name| f.text(name)).collect::<Result<_, _>>()?;
    f.end()?; // Full schema admission BEFORE opening any referred file.
    if generation == 0 || cache_stream == 0 || max_new_tokens == 0
        || max_new_tokens > MAX_GENERATION_TOKENS || stop_tokens.is_empty()
        || max_output_bytes == 0 || max_output_bytes > message_bytes
        || products > MAX_DECODER_PRODUCTS || sampling > MAX_SAMPLING_ENTRIES {
        return Err("native generation bounds are invalid".into());
    }
    let base = path.parent().ok_or("recipe has no parent directory")?;
    let mut remaining = byte_limit.checked_sub(bytes.len()).ok_or("recipe exceeds input allowance")?;
    let mut read = |index: usize, limit: usize| -> Result<Vec<u8>, String> {
        let name = &paths[index];
        if name.is_empty() || name.as_bytes().contains(&0) { return Err("invalid native input path".into()); }
        let path = PathBuf::from(name);
        let path = if path.is_absolute() { path } else { base.join(path) };
        let bytes = read_regular(&path, limit.min(remaining))?;
        remaining = remaining.checked_sub(bytes.len()).ok_or("native input allowance exhausted")?;
        Ok(bytes)
    };
    let model = LlamaConfig::decode(identity, context, &read(0, MAX_CONFIG_BYTES)?).map_err(debug)?;
    // Preserve the model's explicit declaration through durable replay. Missing
    // matrices never choose the mode, and contradictory stored heads refuse.
    let profile = model.profile().clone();
    let weights = read(1, MAX_WEIGHT_FILE_BYTES)?;
    let monitor = read(2, MAX_MONITOR_CONFIG_BYTES)?;
    let sampling_config = read(3, MAX_SAMPLING_CONFIG_BYTES)?;
    let tokenizer = ByteBpe::from_bytes(&profile, &read(4, MAX_FILE_TOKENIZER_BYTES)?).map_err(debug)?;
    let prompt = read(5, MAX_INPUT_BYTES)?;
    for token in prefix_controls.iter().chain(&stop_tokens) {
        if !tokenizer.is_control(*token).map_err(debug)? {
            return Err("prefix and stop IDs must be registered Control tokens".into());
        }
    }
    if stop_tokens.iter().enumerate().any(|(i, token)| stop_tokens[..i].contains(token)) {
        return Err("duplicate stop token".into());
    }
    let tokenization = TokenizationBudget::default();
    let encoded = tokenizer.encode(&prompt, tokenization).map_err(debug)?;
    let prompt_tokens = encoded.tokens().len().checked_add(prefix_controls.len()).ok_or("prompt size overflow")?;
    if prompt_tokens == 0 || prompt_tokens.checked_add(max_new_tokens)
        .is_none_or(|n| n > MAX_GENERATION_TOKENS || n > profile.shape().context) {
        return Err("complete prompt and continuation exceed the native context".into());
    }
    let request = TextGenerationRequest { prompt, prefix_controls, max_new_tokens, stop_tokens,
        tokenization, generation: GenerationBudget { scalar_products: products, sampling_entries: sampling }, max_output_bytes };
    FileTextGenerationCommand::new(generation, 0, 0, request.clone()).map_err(debug)?;
    let decoder = FileDecoderConfig::new_with_output_head(profile, weights, monitor, sampling_config, cache_stream,
        DecoderBindingLimits::default(), model.output_head()).map_err(debug)?;
    Ok(Inputs { decoder, tokenizer, stream, generation, request })
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected recipe object".into()) }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing native recipe field {name}"))
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn count(&mut self, name: &str) -> Result<usize, String> { usize::try_from(self.number(name)?).map_err(debug) }
    fn tokens(&mut self, name: &str, limit: usize) -> Result<Vec<u32>, String> {
        let value = self.take(name)?;
        let array = value.as_array().ok_or("token IDs must be an array")?;
        if array.len() > limit { return Err("too many explicit token IDs".into()); }
        array.iter().map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| "invalid token ID".to_owned())).collect()
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unknown native recipe field".into()) }
    }
}
