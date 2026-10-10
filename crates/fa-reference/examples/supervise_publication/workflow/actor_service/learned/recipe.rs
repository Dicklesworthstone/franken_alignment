//! Bounded, independently provisioned learned generation and native review inputs.
use super::{fields::{Assets, Fields, path, MAX_DATA_BYTES}, fit, monitor::MonitorInput, native::NativeRoster};
use super::super::tokenizer::TokenizerInput;
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::activation::monitor::{
    learned::model::KvTap,
    decoder::sampled::{config::{SamplingConfig, MAX_SAMPLING_CONFIG_BYTES},
        generation::tokenizer::{TokenizationBudget, MAX_INPUT_BYTES}},
};
use fa_reference::action::consequence::activation::probe::{LinearProbe, learned::LearnedProbeWork};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderModel,
    safetensors::{MAX_WEIGHT_HEADER_BYTES, MAX_WEIGHT_FILE_BYTES, pretrained::{LlamaConfig, MAX_CONFIG_BYTES}},
    sampling::monitored::{GenerationBudget, GenerationTelemetryBudget, MAX_GENERATION_TOKENS},
};
use fa_reference::action::consequence::activation::tensor::kv::model::learned::replay::archive::MAX_FIT_ARCHIVE_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedConfig, guarded::FileRecoveryFloor,
    helpers::learned::native::{NativeReviewCost, NativeReviewLimits},
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::{
    decoder_monitoring::{DecoderBindingLimits, LearnedDecoderBindingLimits},
    learned_source::{LearnedEvidenceLimits, text::{LearnedTextConfig, LearnedTextCompletion, LearnedTextOutputPolicy}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, MAX_SIDECAR_ACCUMULATED_BYTES},
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MAX_RECIPE_BYTES: usize = 64 * 1024;

pub(super) struct Loaded {
    pub request: u64,
    pub ttl_ms: u64,
    pub generation: FileLearnedConfig,
    pub stream: StreamProfile,
    pub floor: FileRecoveryFloor,
    pub evidence: LearnedEvidenceLimits,
    pub sidecar: SidecarIdentity,
    pub disclosure: SidecarCongressBudget,
    pub native_limits: NativeReviewLimits,
    pub probes: BTreeMap<KvTap, Vec<LinearProbe>>,
    pub native: Option<NativeRoster>,
}
struct Recipe {
    request: u64, ttl_ms: u64, assets_bytes: usize,
    identity: DecoderIdentity, context: usize, stream: u64, evaluation_origin: u64, monitor_generation: u64,
    publication: StreamProfile, binding: LearnedDecoderBindingLimits, floor: FileRecoveryFloor,
    files: Files, max_new_tokens: usize, max_output_bytes: usize, stop_tokens: BTreeSet<u32>,
    tokenization: TokenizationBudget, budget: GenerationBudget, telemetry: GenerationTelemetryBudget,
    sidecar: SidecarIdentity, disclosure: SidecarCongressBudget, native_limits: NativeReviewLimits,
}
struct Files {
    configuration: PathBuf, weights: PathBuf, tokenizer: TokenizerInput, sampling: PathBuf,
    prompt: PathBuf, fit_binding: PathBuf, fit_archive: PathBuf, monitor: PathBuf, native: PathBuf,
}

pub(super) fn load(file: &Path, config: &Config, require_native: bool) -> Result<Loaded, String> {
    let recipe = Recipe::decode(&read_regular(file, MAX_RECIPE_BYTES)?)?;
    if recipe.identity.tenant != config.profile.delivery.scope.tenant
        || !config.profile.delivery.initial_payload.is_empty() {
        return Err("learned text requires the configured tenant and an empty initial publication".into());
    }
    let mut assets = Assets::new(recipe.assets_bytes)?;
    // The independent binding is never obtained from the archive it checks.
    // Closed probe/fit data admission precedes any model weight ingestion.
    let binding = fit::binding(&assets.read(&recipe.files.fit_binding, MAX_DATA_BYTES)?)?;
    let monitor = MonitorInput::decode(&assets.read(&recipe.files.monitor, MAX_DATA_BYTES)?)?;
    let configuration = assets.read(&recipe.files.configuration, MAX_CONFIG_BYTES)?;
    let negotiated = LlamaConfig::decode(recipe.identity, recipe.context, &configuration).map_err(debug)?;
    let tokenizer = recipe.files.tokenizer.decode(negotiated.profile(),
        &assets.read(Path::new(recipe.files.tokenizer.path()), recipe.files.tokenizer.byte_limit())?)?;
    let sampling = SamplingConfig::decode(&assets.read(&recipe.files.sampling, MAX_SAMPLING_CONFIG_BYTES)?,
        negotiated.profile().shape().vocabulary).map_err(debug)?.start();
    let prompt = String::from_utf8(assets.read(&recipe.files.prompt,
        recipe.tokenization.input_bytes.min(MAX_INPUT_BYTES))?).map_err(debug)?;
    // Stop IDs are only the independently declared literal controls. No content
    // token may be silently discarded as a successful end marker.
    for token in &recipe.stop_tokens {
        if !tokenizer.is_control(*token).map_err(debug)? { return Err("learned stop IDs must be tokenizer controls".into()); }
    }
    let bound = (8 + MAX_WEIGHT_HEADER_BYTES + 4 * negotiated.profile().parameter_count())
        .min(MAX_WEIGHT_FILE_BYTES).min(config.profile.delivery.limits.bytes);
    let weights = assets.read(&recipe.files.weights, bound)?;
    let (model, _) = DecoderModel::from_llama_safetensors(recipe.identity, recipe.context,
        &configuration, &weights).map_err(debug)?;
    let archive = assets.read(&recipe.files.fit_archive, MAX_FIT_ARCHIVE_BYTES)?;
    let codec = fit::replay(&archive, &binding, MAX_FIT_ARCHIVE_BYTES)?;
    let monitor = monitor.bind(&model, codec)?;
    let source = LearnedTextConfig { stream: recipe.stream, evaluation_origin: recipe.evaluation_origin,
        monitor_generation: recipe.monitor_generation, prompt, tokenization: recipe.tokenization,
        max_new_tokens: recipe.max_new_tokens, stop_tokens: recipe.stop_tokens, sampling,
        policy: monitor.policy, budget: recipe.budget, telemetry: recipe.telemetry,
        output: LearnedTextOutputPolicy { max_bytes: recipe.max_output_bytes, completion: LearnedTextCompletion::StopRequired } };
    let generation = FileLearnedConfig::new_text_stream(model, tokenizer, source, recipe.binding,
        recipe.publication).map_err(debug)?.with_required_sidecar().map_err(debug)?
        .with_required_policy_source(config.source_policy).map_err(debug)?;
    let native = if require_native {
        { let roster = NativeRoster::load(&recipe.files.native, config, &mut assets)?;
            roster.check_limits(&recipe.native_limits)?; Some(roster) }
    } else { None };
    Ok(Loaded { request: recipe.request, ttl_ms: recipe.ttl_ms, generation, stream: recipe.publication,
        floor: recipe.floor, evidence: LearnedEvidenceLimits { token_ids: recipe.binding.evidence.token_ids,
            score_words: recipe.binding.evidence.score_words, encoded_bytes: recipe.binding.encoded_bytes },
        sidecar: recipe.sidecar, disclosure: recipe.disclosure, native_limits: recipe.native_limits,
        probes: monitor.probes, native })
}
impl Recipe {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_RECIPE_BYTES)?;
        if root.text("schema")? != "fa.learned-publication/1" {
            return Err("unsupported learned publication recipe".into());
        }
        let request = root.number("request")?;
        let ttl_ms = root.number("ttl_ms")?;
        let assets_bytes = root.size("asset_bytes")?;
        let _ = Assets::new(assets_bytes)?;
        if request == 0 || ttl_ms == 0 || ttl_ms > 3_600_000 {
            return Err("learned request and bounded lifetime must be nonzero".into());
        }
        let mut model = root.object("model")?;
        let identity = identity(model.object("identity")?)?;
        let context = model.size("context")?;
        let stream = model.number("stream")?;
        let evaluation_origin = model.number("evaluation_origin")?;
        let monitor_generation = model.number("monitor_generation")?;
        model.end()?;
        if stream == 0 || evaluation_origin == 0 || monitor_generation == 0 {
            return Err("learned source identities must be nonzero".into());
        }
        let mut publication = root.object("publication_stream")?;
        let publication_value = StreamProfile::new(publication.number("id")?, publication.number("generation")?,
            publication.size("max_messages")?, publication.size("max_message_bytes")?,
            publication.size("max_total_bytes")?).map_err(debug)?;
        publication.end()?;
        let mut binding = root.object("binding")?;
        let binding_value = LearnedDecoderBindingLimits { evidence: DecoderBindingLimits {
            token_ids: binding.size("token_ids")?, score_words: binding.size("score_words")? },
            encoded_bytes: binding.size("encoded_bytes")? };
        binding.end()?;
        // This one-message command uses these SAME explicit limits for capture.
        // The general cumulative binding admits a larger token inventory than
        // one original learned observation; reject that unusable profile now.
        if binding_value.evidence.token_ids == 0
            || binding_value.evidence.token_ids > MAX_GENERATION_TOKENS {
            return Err("one-message learned capture token limit must be within the original generation horizon".into());
        }
        let mut floor = root.object("recovery_floor")?;
        let floor_value = FileRecoveryFloor { journal_revision: floor.number("journal_revision")?,
            control_sequence: floor.number("control_sequence")?, authority_epoch: floor.number("authority_epoch")? };
        floor.end()?;
        let mut files = root.object("files")?;
        let files_value = Files { configuration: files.path("model_config")?, weights: files.path("weights")?,
            tokenizer: TokenizerInput::parse(files.take("tokenizer")?, true)?, sampling: files.path("sampling")?,
            prompt: files.path("prompt")?, fit_binding: files.path("fit_binding")?, fit_archive: files.path("fit_archive")?,
            monitor: files.path("monitor")?, native: files.path("native_roster")? };
        path(files_value.tokenizer.path())?;
        files.end()?;
        let mut text = root.object("text")?;
        let max_new_tokens = text.size("max_new_tokens")?;
        let max_output_bytes = text.size("max_output_bytes")?;
        let stops = text.ids("stop_tokens", MAX_GENERATION_TOKENS)?;
        let stop_tokens: BTreeSet<_> = stops.iter().copied().collect();
        if stops.is_empty() || stops.len() != stop_tokens.len() || max_new_tokens == 0 || max_output_bytes == 0 {
            return Err("learned completion needs explicit unique stop IDs and nonzero token/byte bounds".into());
        }
        if text.text("completion")? != "stop_required" {
            return Err("this command requires an actual monitored control stop".into());
        }
        let tokenization = tokenization(text.object("tokenization")?)?;
        let budget = GenerationBudget { decoder_products: text.number("decoder_products")?,
            vocabulary_scores: text.number("vocabulary_scores")? };
        let telemetry = telemetry(text.object("telemetry")?)?;
        text.end()?;
        let mut sidecar = root.object("sidecar")?;
        let mut identity_fields = sidecar.object("identity")?;
        let sidecar_identity = SidecarIdentity { object_id: identity_fields.number("object_id")?,
            generation: identity_fields.number("generation")?, transform_id: identity_fields.number("transform_id")? };
        identity_fields.end()?;
        let mut disclosure = sidecar.object("budget")?;
        let disclosure_value = SidecarCongressBudget { rounds: disclosure.size("rounds")?,
            residual_bytes: disclosure.size("residual_bytes")?, committee_bytes: disclosure.size("committee_bytes")? };
        disclosure.end()?; sidecar.end()?;
        // This deliberately bounded command starts one real native round. A
        // need-more outcome never becomes Allow or retries with a fresh budget.
        if sidecar_identity.object_id == 0 || sidecar_identity.generation == 0
            || sidecar_identity.transform_id == 0 || disclosure_value.rounds != 1
            || disclosure_value.residual_bytes == 0 || disclosure_value.committee_bytes == 0
            || disclosure_value.residual_bytes > MAX_SIDECAR_ACCUMULATED_BYTES
            || disclosure_value.committee_bytes > MAX_SIDECAR_ACCUMULATED_BYTES {
            return Err("learned publication requires nonzero sidecar identity and one bounded review round".into());
        }
        let mut native = root.object("native_review")?;
        let native_limits = NativeReviewLimits {
            polls: native.size("polls")?,
            probes: LearnedProbeWork { coordinates: native.size("probe_coordinates")?,
                reconstruction_products: native.number("reconstruction_products")? },
            native: NativeReviewCost { evaluations: native.size("evaluations")?,
                scalar_products: native.number("scalar_products")?, sampling_entries: native.number("sampling_entries")? },
            ..NativeReviewLimits::default()
        };
        native.end()?; root.end()?;
        let caps = NativeReviewLimits::default();
        if native_limits.polls == 0 || native_limits.polls > caps.polls
            || native_limits.probes.coordinates > caps.probes.coordinates
            || native_limits.probes.reconstruction_products > caps.probes.reconstruction_products
            || native_limits.native.evaluations == 0 || native_limits.native.evaluations > caps.native.evaluations
            || native_limits.native.scalar_products > caps.native.scalar_products
            || native_limits.native.sampling_entries > caps.native.sampling_entries {
            return Err("native review requires explicit nonzero finite work bounds".into());
        }
        Ok(Self { request, ttl_ms, assets_bytes, identity, context, stream, evaluation_origin, monitor_generation,
            publication: publication_value, binding: binding_value, floor: floor_value, files: files_value,
            max_new_tokens, max_output_bytes, stop_tokens, tokenization, budget, telemetry,
            sidecar: sidecar_identity, disclosure: disclosure_value, native_limits })
    }
}
pub(super) fn identity(mut fields: Fields) -> Result<DecoderIdentity, String> {
    let identity = DecoderIdentity { tenant: fields.number("tenant")?, model: fields.number("model")?,
        model_generation: fields.number("model_generation")?, tokenizer_generation: fields.number("tokenizer_generation")?,
        profile_generation: fields.number("profile_generation")? };
    fields.end()?; Ok(identity)
}
pub(super) fn tokenization(mut fields: Fields) -> Result<TokenizationBudget, String> {
    let value = TokenizationBudget { input_bytes: fields.size("input_bytes")?,
        pair_lookups: fields.size("pair_lookups")?, heap_pops: fields.size("heap_pops")? };
    fields.end()?; Ok(value)
}
fn telemetry(mut fields: Fields) -> Result<GenerationTelemetryBudget, String> {
    let value = GenerationTelemetryBudget {
        compression_source_values: fields.number("compression_source_values")?,
        compression_encoded_bytes: fields.number("compression_encoded_bytes")?,
        compression_work_units: fields.number("compression_work_units")?,
        source_check_values: fields.number("source_check_values")?,
        source_check_encoded_bytes: fields.number("source_check_encoded_bytes")?,
        source_check_reconstruction_products: fields.number("source_check_reconstruction_products")?,
        monitor_encoded_bytes: fields.number("monitor_encoded_bytes")?,
        monitor_probe_coordinates: fields.number("monitor_probe_coordinates")?,
        monitor_reconstruction_products: fields.number("monitor_reconstruction_products")?,
        monitor_materialized_values: fields.number("monitor_materialized_values")?,
        monitor_refinements: fields.number("monitor_refinements")?,
    };
    fields.end()?; Ok(value)
}
