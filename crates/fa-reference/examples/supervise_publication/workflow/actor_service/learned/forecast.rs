//! Closed, independently supplied forecasts for the original learned owner.
//! Registration probabilities are provisioning data, never calibration evidence.
//! Model identities declare comparison contracts, not asset authentication.
//! Parsing and actual model/source binding complete before a durable store opens.
use super::fields::{Assets, Fields, MAX_DATA_BYTES, hex};
use crate::config::debug;
use fa_reference::action::consequence::activation::{
    CaptureProfile,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration, MAX_EVENT_PREFIX_BYTES},
    monitor::learned::LearnedMonitorBudget,
    tensor::kv::{
        decoder::{DecoderIdentity, DecoderModel, MAX_DECODER_HIDDEN},
        experiment::KvSide,
    },
};
use fa_reference::action::consequence::delivery::{
    persistent::observed::{
        consistency::{FileConsistencyConfig, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        decoder::learned::FileLearnedConfig,
    },
    stream::StreamProfile,
};
use std::path::Path;

enum RequiredForecast {
    Raw(FileConsistencyConfig),
    Owned(FileLearnedConsistencyConfig),
}

pub(super) struct ForecastInput {
    identity: DecoderIdentity,
    layer: u64,
    side: Option<KvSide>,
    profile: CaptureProfile,
    dimensions: usize,
    stream: u64,
    publication: StreamProfile,
    required: RequiredForecast,
}

impl ForecastInput {
    pub(super) fn load(path: &Path, assets: &mut Assets) -> Result<Self, String> {
        Self::decode(&assets.read(path, MAX_DATA_BYTES)?)
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_DATA_BYTES)?;
        if root.text("schema")? != "fa.learned-forecast/1" {
            return Err("unsupported learned forecast artifact".into());
        }
        let identity = identity(root.object("model_identity")?)?;
        let mut source = root.object("source")?;
        let owned = match source.text("kind")?.as_str() {
            "raw_residual" => false,
            "owned_learned_kv" => true,
            _ => return Err("unsupported learned forecast source".into()),
        };
        let layer = source.number("layer")?;
        let side = if owned { Some(match source.text("side")?.as_str() {
            "key" => KvSide::Key,
            "value" => KvSide::Value,
            _ => return Err("learned forecast side must be key or value".into()),
        }) } else { None };
        let profile = profile(source.object("profile")?)?;
        source.end()?;
        let mut probe = root.object("probe")?;
        let probe_id = probe.number("id")?;
        let probe_generation = probe.number("generation")?;
        let weights = probe.array("weights_bits", MAX_DECODER_HIDDEN)?.into_iter().map(|word| {
            let bits = word.as_u64().and_then(|value| u32::try_from(value).ok())
                .ok_or("forecast coefficients must be binary32 bit words")?;
            finite(bits)
        }).collect::<Result<Vec<_>, String>>()?;
        let bias = finite(word(&mut probe, "bias_bits")?)?;
        let threshold = finite(word(&mut probe, "threshold_bits")?)?;
        probe.end()?;
        let dimensions = weights.len();
        let mut registration = root.object("registration")?;
        let forecast = ForecastRegistration {
            domain: registration.number("domain")?,
            generation: registration.number("generation")?,
            policy_generation: registration.number("policy_generation")?,
            event_prefix: hex(&registration.text("event_prefix_hex")?, MAX_EVENT_PREFIX_BYTES)?,
            negative: probability(registration.object("negative")?)?,
            at_threshold: probability(registration.object("at_threshold")?)?,
            positive: probability(registration.object("positive")?)?,
        };
        registration.end()?;
        let mut alpha = root.object("alpha")?;
        let error_budget = ErrorBudget::new(alpha.number("numerator")?, alpha.number("denominator")?)
            .map_err(debug)?;
        alpha.end()?;
        let stream = root.number("stream")?;
        let max_predictions = root.size("max_predictions")?;
        let max_prediction_age_ticks = root.number("max_prediction_age_ticks")?;
        let publication = publication(root.object("publication_stream")?)?;
        let learned_budget = if owned {
            let mut fields = root.object("learned_budget")?;
            let per_job = budget(fields.object("per_job")?)?;
            let lifetime = budget(fields.object("lifetime")?)?;
            let retained = fields.size("max_retained_source_bytes")?;
            fields.end()?;
            Some((per_job, lifetime, retained))
        } else { None };
        // No unknown, cross-mode or duplicate fields can be hidden in a valid
        // original predictor. All externally selected semantics are explicit.
        root.end()?;
        let consistency = FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id, probe_generation, profile, weights, bias, threshold, forecast,
            alpha: error_budget, stream, max_predictions, max_prediction_age_ticks,
        }).map_err(debug)?.with_stream_messages(publication).map_err(debug)?;
        let required = match (side, learned_budget) {
            (Some(side), Some((per_job, lifetime, retained))) => RequiredForecast::Owned(
                FileLearnedConsistencyConfig::new(consistency, layer, side, per_job, lifetime, retained)
                    .map_err(debug)?.with_owned_generation().map_err(debug)?
                    .with_pre_output_forecast().map_err(debug)?),
            (None, None) => RequiredForecast::Raw(consistency.with_hosted_residual(layer)
                .map_err(debug)?.with_pre_output_forecast().map_err(debug)?),
            _ => return Err("incomplete learned forecast source contract".into()),
        };
        Ok(Self { identity, layer, side, profile, dimensions, stream, publication, required })
    }

    /// Compare the explicit registration against the contracts installed by the
    /// actual imported model. Matching labels alone cannot substitute a K/V tap,
    /// residual width, tokenizer generation or complete-message stream policy.
    pub(super) fn bind(self, model: &DecoderModel, stream: u64, publication: StreamProfile)
        -> Result<BoundForecast, String>
    {
        if self.identity != model.profile().identity() || self.stream != stream
            || self.publication != publication {
            return Err("forecast model identity or stream does not match the learned recipe".into());
        }
        let contract = match self.side {
            None => model.residual_contract(self.layer).map_err(debug)?,
            Some(side) => {
                let layer = model.cache_profile().layers().get(&self.layer)
                    .ok_or("forecast layer is outside the original learned model")?;
                match side { KvSide::Key => layer.keys(), KvSide::Value => layer.values() }
            }
        };
        if contract.profile() != self.profile || contract.dimensions() != self.dimensions {
            return Err("forecast source profile or dimensions do not match the original model tap".into());
        }
        Ok(BoundForecast(self.required))
    }
}

/// Only a locally parsed and model-bound original predictor can reach the recipe
/// pin. The original generation constructor still validates its own full bytes.
pub(super) struct BoundForecast(RequiredForecast);
impl BoundForecast {
    pub(super) fn pin(self, generation: FileLearnedConfig) -> Result<FileLearnedConfig, String> {
        match self.0 {
            RequiredForecast::Raw(predictor) => generation.with_required_pre_output_forecast(predictor),
            RequiredForecast::Owned(predictor) => generation.with_required_owned_pre_output_forecast(predictor),
        }.map_err(debug)
    }
}

fn identity(mut fields: Fields) -> Result<DecoderIdentity, String> {
    let identity = DecoderIdentity { tenant: fields.number("tenant")?, model: fields.number("model")?,
        model_generation: fields.number("model_generation")?, tokenizer_generation: fields.number("tokenizer_generation")?,
        profile_generation: fields.number("profile_generation")? };
    fields.end()?;
    Ok(identity)
}

fn profile(mut fields: Fields) -> Result<CaptureProfile, String> {
    let profile = CaptureProfile { tenant: fields.number("tenant")?, model: fields.number("model")?,
        model_generation: fields.number("model_generation")?, tap: fields.number("tap")?,
        layout_generation: fields.number("layout_generation")? };
    fields.end()?;
    Ok(profile)
}

fn publication(mut fields: Fields) -> Result<StreamProfile, String> {
    let profile = StreamProfile::new(fields.number("id")?, fields.number("generation")?,
        fields.size("max_messages")?, fields.size("max_message_bytes")?, fields.size("max_total_bytes")?)
        .map_err(debug)?;
    fields.end()?;
    Ok(profile)
}

fn probability(mut fields: Fields) -> Result<BinaryForecast, String> {
    let probability = BinaryForecast::new(word(&mut fields, "null")?, word(&mut fields, "alternative")?)
        .map_err(debug)?;
    fields.end()?;
    Ok(probability)
}

fn word(fields: &mut Fields, name: &str) -> Result<u32, String> {
    u32::try_from(fields.number(name)?).map_err(debug)
}

fn finite(bits: u32) -> Result<f32, String> {
    let value = f32::from_bits(bits);
    if !value.is_finite() { return Err("forecast coefficients must be finite binary32 words".into()); }
    Ok(value)
}

fn budget(mut fields: Fields) -> Result<LearnedMonitorBudget, String> {
    let budget = LearnedMonitorBudget { encoded_bytes: fields.size("encoded_bytes")?,
        probe_coordinates: fields.size("probe_coordinates")?,
        reconstruction_products: fields.number("reconstruction_products")?,
        materialized_values: fields.size("materialized_values")?, refinements: fields.size("refinements")? };
    fields.end()?;
    Ok(budget)
}
