//! Original bounded model, tokenizer, fitted codec and complete monitor admission.
use super::{debug, plan::Plan};
use super::super::{fields::{Assets, MAX_DATA_BYTES}, fit, monitor::MonitorInput};
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::{config::{SamplingConfig, MAX_SAMPLING_CONFIG_BYTES},
        generation::tokenizer::{TokenizationWork, MAX_INPUT_BYTES}},
    probe::training::decoder::kv::MAX_KV_MONITOR_ARTIFACT_BYTES,
    tensor::kv::{decoder::{experiment::prompt::{PromptExperimentConfig, campaign::PromptCampaign},
        safetensors::{MAX_WEIGHT_HEADER_BYTES, MAX_WEIGHT_FILE_BYTES, pretrained::{LlamaConfig, MAX_CONFIG_BYTES}},
        sampling::monitored::GenerationSpec},
        model::learned::replay::archive::MAX_FIT_ARCHIVE_BYTES},
};
use std::path::Path;

pub(super) struct Loaded {
    pub campaign: PromptCampaign,
    pub input_bytes: usize,
    pub tokenization: TokenizationWork,
}

pub(super) fn load(plan: &Plan, plan_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<Loaded, String>
{
    let mut assets = Assets::new(plan.asset_bytes)?;
    assets.charge(plan_bytes)?;
    let binding = fit::binding(&assets.read(&plan.files.fit_binding, MAX_DATA_BYTES)?)?;
    checkpoint()?;
    let monitor = MonitorInput::decode(&assets.read(&plan.files.monitor, MAX_KV_MONITOR_ARTIFACT_BYTES)?)?;
    checkpoint()?;
    let configuration = assets.read(&plan.files.configuration, MAX_CONFIG_BYTES)?;
    let negotiated = LlamaConfig::decode(plan.identity, plan.context, &configuration).map_err(debug)?;
    let cache = negotiated.profile().cache_profile().map_err(debug)?;
    // These source labels are independently selected assumptions, not signatures.
    // A declared fit lineage or source stream cannot also be this evaluation.
    for (origin, descriptor) in &binding.sources {
        if descriptor.profile() != &cache || *origin == plan.evaluation_origin
            || descriptor.layers().values().any(|source| source.stream == plan.stream) {
            return Err("prompt experiment source/model overlaps or mismatches the independent codec fit inventory".into());
        }
    }
    checkpoint()?;
    let tokenizer = plan.files.tokenizer.decode(negotiated.profile(), &assets.read(
        Path::new(plan.files.tokenizer.path()), plan.files.tokenizer.byte_limit())?)?;
    checkpoint()?;
    let sampling = SamplingConfig::decode(&assets.read(&plan.files.sampling, MAX_SAMPLING_CONFIG_BYTES)?,
        negotiated.profile().shape().vocabulary).map_err(debug)?.start();
    checkpoint()?;
    let prompt = String::from_utf8(assets.read(&plan.files.prompt,
        plan.tokenization.input_bytes.min(MAX_INPUT_BYTES))?).map_err(debug)?;
    let encoded = tokenizer.encode(prompt.as_bytes(), plan.tokenization)
        .map_err(|failure| debug(failure.error))?;
    for token in &plan.stop_tokens {
        if !tokenizer.is_control(*token).map_err(debug)? {
            return Err("prompt experiment stop IDs must be independently declared tokenizer controls".into());
        }
    }
    let end = plan.edit.start.checked_add(plan.edit.expected.len()).ok_or("prompt edit span overflow")?;
    if encoded.tokens().get(plan.edit.start..end) != Some(plan.edit.expected.as_slice()) {
        return Err("prompt edit expected token span does not match the original tokenizer output".into());
    }
    let original = GenerationSpec::new(encoded.tokens().to_vec(), plan.max_new_tokens,
        plan.stop_tokens.clone(), sampling).map_err(debug)?;
    checkpoint()?;
    let bound = (8 + MAX_WEIGHT_HEADER_BYTES + 4 * negotiated.profile().parameter_count())
        .min(MAX_WEIGHT_FILE_BYTES);
    let model = plan.files.weights.load(&configuration, &negotiated, &mut assets, bound)?;
    checkpoint()?;
    let codec = fit::replay(&assets.read(&plan.files.fit_archive, MAX_FIT_ARCHIVE_BYTES)?,
        &binding, MAX_FIT_ARCHIVE_BYTES)?;
    checkpoint()?;
    let monitor = monitor.bind_with_tokenizer(&model, &tokenizer, codec)?;
    let intervention = model.prompt_intervention(plan.intervention, PromptExperimentConfig {
        stream: plan.stream, evaluation_origin: plan.evaluation_origin, original,
        policy: monitor.policy, generation: plan.generation, telemetry: plan.telemetry,
    }, plan.edit.clone()).map_err(debug)?;
    let campaign = intervention.begin_campaign(plan.campaign, plan.seeds.clone(),
        plan.outcome_tokens.clone(), plan.budget).map_err(debug)?;
    Ok(Loaded { campaign, input_bytes: assets.consumed(), tokenization: encoded.work() })
}
