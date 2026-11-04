//! A closed independent L7 recipe. It contains no live configuration or authority.
use super::super::{fields::{Fields, path, MAX_ASSET_BYTES}, recipe};
use super::super::recipe::weights::WeightsInput;
use super::super::super::tokenizer::TokenizerInput;
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::TokenizationBudget,
    tensor::kv::decoder::{DecoderIdentity, experiment::prompt::{PromptEdit,
        campaign::{PromptCampaignBudget, MAX_PROMPT_TRIALS, MAX_OUTCOME_TOKENS}},
        sampling::monitored::{GenerationBudget, GenerationTelemetryBudget, MAX_GENERATION_TOKENS}},
};
use std::{collections::BTreeSet, path::PathBuf};

pub(super) const MAX_PLAN_BYTES: usize = 64 * 1024;

pub(super) struct Plan {
    pub campaign: u64,
    pub intervention: u64,
    pub runtime_ms: u64,
    pub asset_bytes: usize,
    pub identity: DecoderIdentity,
    pub context: usize,
    pub stream: u64,
    pub evaluation_origin: u64,
    pub files: Files,
    pub max_new_tokens: usize,
    pub stop_tokens: BTreeSet<u32>,
    pub tokenization: TokenizationBudget,
    pub generation: GenerationBudget,
    pub telemetry: GenerationTelemetryBudget,
    pub edit: PromptEdit,
    pub seeds: Vec<u64>,
    pub outcome_tokens: Vec<u32>,
    pub budget: PromptCampaignBudget,
}

pub(super) struct Files {
    pub configuration: PathBuf,
    pub weights: WeightsInput,
    pub tokenizer: TokenizerInput,
    pub sampling: PathBuf,
    pub prompt: PathBuf,
    pub fit_binding: PathBuf,
    pub fit_archive: PathBuf,
    pub monitor: PathBuf,
}

impl Plan {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_PLAN_BYTES)?;
        if root.text("schema")? != "fa.learned-prompt-counterfactual/1" {
            return Err("unsupported prompt counterfactual plan".into());
        }
        let campaign = root.number("campaign")?;
        let intervention = root.number("intervention")?;
        let runtime_ms = root.number("runtime_ms")?;
        let asset_bytes = root.size("asset_bytes")?;
        if campaign == 0 || intervention == 0 || runtime_ms == 0 || runtime_ms > 3_600_000
            || asset_bytes == 0 || asset_bytes > MAX_ASSET_BYTES {
            return Err("prompt experiment requires nonzero identities and bounded input/runtime allowances".into());
        }
        let mut model = root.object("model")?;
        let identity = recipe::identity(model.object("identity")?)?;
        let context = model.size("context")?;
        let stream = model.number("stream")?;
        let evaluation_origin = model.number("evaluation_origin")?;
        model.end()?;
        if context == 0 || context > MAX_GENERATION_TOKENS || stream == 0 || evaluation_origin == 0 {
            return Err("prompt experiment requires an explicit bounded model/source identity".into());
        }
        let mut files = root.object("files")?;
        let files_value = Files { configuration: files.path("model_config")?,
            weights: WeightsInput::parse(files.take("weights")?, true)?,
            tokenizer: TokenizerInput::parse(files.take("tokenizer")?, true)?,
            sampling: files.path("sampling")?, prompt: files.path("prompt")?,
            fit_binding: files.path("fit_binding")?, fit_archive: files.path("fit_archive")?,
            monitor: files.path("monitor")? };
        path(files_value.tokenizer.path())?;
        files.end()?;
        let mut generation = root.object("generation")?;
        let max_new_tokens = generation.size("max_new_tokens")?;
        let stops = generation.ids("stop_tokens", MAX_GENERATION_TOKENS)?;
        let stop_tokens: BTreeSet<_> = stops.iter().copied().collect();
        if max_new_tokens == 0 || max_new_tokens > MAX_GENERATION_TOKENS || stops.len() != stop_tokens.len() {
            return Err("prompt experiment needs a finite token horizon and unique stop IDs".into());
        }
        let tokenization = recipe::tokenization(generation.object("tokenization")?)?;
        let generation_value = GenerationBudget { decoder_products: generation.number("decoder_products")?,
            vocabulary_scores: generation.number("vocabulary_scores")? };
        let telemetry = recipe::telemetry(generation.object("telemetry")?)?;
        generation.end()?;
        let mut edit = root.object("edit")?;
        let edit_value = PromptEdit { start: edit.size("start")?,
            expected: edit.ids("expected", MAX_GENERATION_TOKENS)?,
            replacement: edit.ids("replacement", MAX_GENERATION_TOKENS)? };
        edit.end()?;
        let mut schedule = root.object("schedule")?;
        let seeds = schedule.array("seeds", MAX_PROMPT_TRIALS)?.into_iter().map(|value|
            value.as_u64().ok_or_else(|| "seeds must be unsigned 64-bit integers".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        let outcome_tokens = schedule.ids("outcome_tokens", MAX_OUTCOME_TOKENS)?;
        schedule.end()?;
        if seeds.is_empty() || outcome_tokens.is_empty()
            || seeds.iter().copied().collect::<BTreeSet<_>>().len() != seeds.len() {
            return Err("prompt experiment requires unique ordered seeds and one nonempty exact token query".into());
        }
        let mut budget = root.object("budget")?;
        let budget_value = PromptCampaignBudget { trials: budget.size("trials")?,
            positions: budget.size("positions")?, decoder_products: budget.number("decoder_products")?,
            vocabulary_scores: budget.number("vocabulary_scores")?,
            outcome_comparisons: budget.number("outcome_comparisons")? };
        budget.end()?; root.end()?;
        Ok(Self { campaign, intervention, runtime_ms, asset_bytes, identity, context, stream,
            evaluation_origin, files: files_value, max_new_tokens, stop_tokens, tokenization,
            generation: generation_value, telemetry, edit: edit_value, seeds, outcome_tokens,
            budget: budget_value })
    }
}
