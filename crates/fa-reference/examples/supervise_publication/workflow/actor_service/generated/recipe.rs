//! Operator-owned data loading; no downloaded code, inferred EOS or raw-output override.
use super::{Config, debug};
use crate::config::read_regular;
use fa_reference::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::config::MAX_SAMPLING_CONFIG_BYTES;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_STOP_TOKENS, text::{TextGenerationRequest, MAX_PREFIX_CONTROLS},
    tokenizer::{ByteBpe, TokenizationBudget, MAX_INPUT_BYTES},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{
    MAX_WEIGHT_HEADER_BYTES, MAX_WEIGHT_FILE_BYTES,
    pretrained::{LlamaConfig, MAX_CONFIG_BYTES},
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::{
    FileDecoderConfig, text::{FileTextGenerationCommand, MAX_FILE_TOKENIZER_BYTES},
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

const MAX_RECIPE_BYTES: usize = 16_384;

pub(super) struct Loaded {
    pub request: u64,
    pub generation: u64,
    pub ttl_ms: u64,
    pub decoder: FileDecoderConfig,
    pub tokenizer: ByteBpe,
    pub stream: StreamProfile,
    pub text: TextGenerationRequest,
}
impl Loaded {
    pub(super) fn check_host(&self, config: &Config) -> Result<(), String> {
        if self.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant
            || !config.profile.delivery.initial_payload.is_empty()
        { return Err("native text requires the configured tenant and an empty initial publication".into()); }
        if self.request == 0 || self.generation == 0 || self.ttl_ms == 0 || self.ttl_ms > 3_600_000
            || self.text.max_new_tokens == 0 || self.text.stop_tokens.is_empty()
        { return Err("native publication requires bounded nonzero request, generation, lifetime and stop IDs".into()); }
        if !self.tokenizer.binds(self.decoder.profile()) {
            return Err("tokenizer is not bound to the exact decoder profile".into());
        }
        // A content stop can suppress a meaningful suffix. Reject it even if a
        // different Control ID would happen to be chosen on this particular run.
        for id in self.text.stop_tokens.iter().chain(&self.text.prefix_controls) {
            if !self.tokenizer.is_control(*id).map_err(debug)? {
                return Err("stop and prefix IDs must be explicit tokenizer controls".into());
            }
        }
        FileTextGenerationCommand::new(self.generation, 0, 0, self.text.clone()).map_err(debug)?;
        Ok(())
    }
}

// Parse every field and path before reading model/input files. A malformed recipe
// cannot partially provision a store or a socket. Immutable constructors below
// still enforce their own full shape, inventory and numeric compatibility laws.
struct Recipe {
    request: u64, generation: u64, ttl_ms: u64,
    identity: DecoderIdentity, context: usize, decoder_stream: u64,
    stream: StreamProfile, limits: DecoderBindingLimits,
    paths: Paths, text: TextGenerationRequest,
}
struct Paths {
    configuration: PathBuf, weights: PathBuf, monitor: PathBuf,
    sampling: PathBuf, tokenizer: PathBuf, prompt: PathBuf,
}
impl Recipe {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::new(strict_json::parse(bytes, Limits {
            max_bytes: MAX_RECIPE_BYTES, max_depth: 5, max_items: 1024, max_string_bytes: 4096,
        }).map_err(debug)?)?;
        if root.text("schema")? != "fa.generated-publication/1" {
            return Err("unsupported native publication recipe".into());
        }
        let request = root.number("request")?;
        let generation = root.number("generation")?;
        let ttl_ms = root.number("ttl_ms")?;
        if request == 0 || generation == 0 || ttl_ms == 0 || ttl_ms > 3_600_000 {
            return Err("invalid native request, generation or lifetime".into());
        }
        let mut model = Fields::new(root.take("model")?)?;
        let mut identity = Fields::new(model.take("identity")?)?;
        let identity_value = DecoderIdentity {
            tenant: identity.number("tenant")?, model: identity.number("model")?,
            model_generation: identity.number("model_generation")?,
            tokenizer_generation: identity.number("tokenizer_generation")?,
            profile_generation: identity.number("profile_generation")?,
        };
        identity.end()?;
        let context = model.size("context")?;
        let decoder_stream = model.number("stream")?;
        model.end()?;
        let mut stream = Fields::new(root.take("publication_stream")?)?;
        let stream_value = StreamProfile::new(stream.number("id")?, stream.number("generation")?,
            stream.size("max_messages")?, stream.size("max_message_bytes")?,
            stream.size("max_total_bytes")?).map_err(debug)?;
        stream.end()?;
        let mut binding = Fields::new(root.take("binding")?)?;
        let limits = DecoderBindingLimits { token_ids: binding.size("token_ids")?,
            score_words: binding.size("score_words")? };
        binding.end()?;
        let mut paths = Fields::new(root.take("files")?)?;
        let paths_value = Paths { configuration: paths.path("model_config")?, weights: paths.path("weights")?,
            monitor: paths.path("monitor")?, sampling: paths.path("sampling")?,
            tokenizer: paths.path("tokenizer")?, prompt: paths.path("prompt")? };
        paths.end()?;
        let mut text = Fields::new(root.take("text")?)?;
        let prefix_controls = text.ids("prefix_controls", MAX_PREFIX_CONTROLS)?;
        let stop_tokens = text.ids("stop_tokens", MAX_STOP_TOKENS)?;
        let max_new_tokens = text.size("max_new_tokens")?;
        let max_output_bytes = text.size("max_output_bytes")?;
        let generation_budget = GenerationBudget { scalar_products: text.number("scalar_products")?,
            sampling_entries: text.number("sampling_entries")? };
        let mut tokenization = Fields::new(text.take("tokenization")?)?;
        let tokenization_budget = TokenizationBudget { input_bytes: tokenization.size("input_bytes")?,
            pair_lookups: tokenization.size("pair_lookups")?, heap_pops: tokenization.size("heap_pops")? };
        tokenization.end()?; text.end()?; root.end()?;
        let text = TextGenerationRequest { prompt: Vec::new(), prefix_controls, stop_tokens,
            max_new_tokens, max_output_bytes, generation: generation_budget, tokenization: tokenization_budget };
        if text.max_new_tokens == 0 || text.stop_tokens.is_empty() {
            return Err("native message requires generation and at least one explicit stop".into());
        }
        FileTextGenerationCommand::new(generation, 0, 0, text.clone()).map_err(debug)?;
        Ok(Self { request, generation, ttl_ms, identity: identity_value, context, decoder_stream,
            stream: stream_value, limits, paths: paths_value, text })
    }
}

pub(super) fn load(path: &Path, config: &Config) -> Result<Loaded, String> {
    let mut r = Recipe::decode(&read_regular(path, MAX_RECIPE_BYTES)?)?;
    if r.identity.tenant != config.profile.delivery.scope.tenant
        || !config.profile.delivery.initial_payload.is_empty()
    { return Err("native text requires the configured tenant and an empty initial publication".into()); }
    let configuration = read_regular(&r.paths.configuration, MAX_CONFIG_BYTES)?;
    let negotiated = LlamaConfig::decode(r.identity, r.context, &configuration).map_err(debug)?;
    // The negotiated head contract is frozen with the original raw weights.
    // Omission is permitted only for explicit tying; conflicting heads refuse.
    let profile = negotiated.profile().clone();
    let tokenizer = ByteBpe::from_bytes(&profile,
        &read_regular(&r.paths.tokenizer, MAX_FILE_TOKENIZER_BYTES)?).map_err(debug)?;
    r.text.prompt = read_regular(&r.paths.prompt, r.text.tokenization.input_bytes.min(MAX_INPUT_BYTES))?;
    let model_bound = 8 + MAX_WEIGHT_HEADER_BYTES + 4 * profile.parameter_count();
    let weight_limit = model_bound.min(MAX_WEIGHT_FILE_BYTES).min(config.profile.delivery.limits.bytes);
    let weights = read_regular(&r.paths.weights, weight_limit)?;
    let monitor = read_regular(&r.paths.monitor, MAX_MONITOR_CONFIG_BYTES)?;
    let sampling = read_regular(&r.paths.sampling, MAX_SAMPLING_CONFIG_BYTES)?;
    let decoder = FileDecoderConfig::new_with_output_head(profile, weights, monitor, sampling,
        r.decoder_stream, r.limits, negotiated.output_head()).map_err(debug)?;
    let loaded = Loaded { request: r.request, generation: r.generation, ttl_ms: r.ttl_ms,
        decoder, tokenizer, stream: r.stream, text: r.text };
    loaded.check_host(config)?;
    Ok(loaded)
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected recipe object".into()) }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing recipe field {name}"))
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn size(&mut self, name: &str) -> Result<usize, String> { usize::try_from(self.number(name)?).map_err(debug) }
    fn path(&mut self, name: &str) -> Result<PathBuf, String> {
        let path = PathBuf::from(self.text(name)?);
        if !path.is_absolute() || path.file_name().is_none()
            || path.as_os_str().as_encoded_bytes().contains(&0)
            || path.components().any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
        { return Err(format!("{name} must be an absolute normalized file path")); }
        Ok(path)
    }
    fn ids(&mut self, name: &str, limit: usize) -> Result<Vec<u32>, String> {
        let value = self.take(name)?;
        let ids = value.as_array().ok_or_else(|| format!("{name} must be a token ID array"))?;
        if ids.len() > limit { return Err(format!("{name} exceeds its bound")); }
        ids.iter().map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok())
            .ok_or_else(|| format!("{name} requires unsigned 32-bit token IDs"))).collect()
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unknown native recipe field".into()) }
    }
}

#[cfg(test)]
mod tests;
