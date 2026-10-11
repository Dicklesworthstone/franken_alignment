//! Closed token corpus and complete original inference/fitting admission.
use super::{fields::Fields, config::debug};
use fa_reference::action::consequence::activation::tensor::kv::{
    MAX_KV_POSITIONS,
    decoder::{DecoderIdentity, DecoderModel, DecoderProfile, MAX_DECODER_PRODUCTS,
        safetensors::{MAX_WEIGHT_FILE_BYTES, reader::MAX_WEIGHT_READ_CALLS}},
    model::{MAX_MODEL_KV_VALUES, MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES,
        learned::{FitBudget, FitEstimate, LearnedKvPolicy, MAX_TRAINING_ROWS,
            MAX_TRAINING_SOURCES, MAX_LEARNED_PARAMETERS, MAX_FIT_SCRATCH_VALUES, MAX_FIT_WORK,
            replay::{MAX_FIT_CHECKPOINT_INPUT_BYTES, MAX_FIT_CHECKPOINT_WITNESS_BYTES,
                archive::MAX_FIT_ARCHIVE_BYTES,
                training::{CodecTraining, CodecTrainingBudget, CodecTrainingInput, CodecTrainingReport}}}}};
use fa_reference::Error;
use std::collections::BTreeSet;

pub(super) const MAX_PLAN_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy)]
pub(super) struct InputLimits { pub auxiliary_bytes: usize, pub weight_bytes: usize, pub weight_read_calls: usize }
#[derive(Clone, Copy)]
pub(super) struct OutputLimits { pub binding_bytes: usize, pub archive_bytes: usize, pub report_bytes: usize }
#[derive(Clone, Copy, Debug)]
pub(super) struct Estimate {
    pub tokens: usize,
    pub source_values: usize,
    pub decoder_products: u64,
    pub fit: FitEstimate,
    pub input_bytes: usize,
    pub binding_reservation: usize,
    pub archive_reservation: usize,
    pub report_reservation: usize,
}
pub(super) struct Plan {
    pub identity: DecoderIdentity,
    pub context: usize,
    pub inputs: InputLimits,
    pub outputs: OutputLimits,
    pub policy: LearnedKvPolicy,
    pub budget: CodecTrainingBudget,
    pub documents: Vec<CodecTrainingInput>,
}

impl Plan {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_PLAN_BYTES)?;
        if root.text("schema")? != "fa.learned-kv-codec-training/1" { return Err("unsupported codec training schema".into()); }
        let mut id = root.object("identity")?;
        let identity = DecoderIdentity { tenant: positive(&mut id, "tenant")?, model: positive(&mut id, "model")?,
            model_generation: positive(&mut id, "model_generation")?, tokenizer_generation: positive(&mut id, "tokenizer_generation")?,
            profile_generation: positive(&mut id, "profile_generation")? };
        id.end()?;
        let context = root.size("context")?;
        if context == 0 || context > MAX_KV_POSITIONS { return Err("invalid original decoder context".into()); }
        let mut value = root.object("input_limits")?;
        let inputs = InputLimits { auxiliary_bytes: value.size("auxiliary_bytes")?,
            weight_bytes: value.size("weight_bytes")?, weight_read_calls: value.size("weight_read_calls")? };
        value.end()?;
        if inputs.auxiliary_bytes == 0 || inputs.auxiliary_bytes > super::fields::MAX_ASSET_BYTES
            || inputs.weight_bytes == 0 || inputs.weight_bytes > MAX_WEIGHT_FILE_BYTES
            || inputs.weight_read_calls == 0 || inputs.weight_read_calls > MAX_WEIGHT_READ_CALLS {
            return Err("invalid original input reader limits".into());
        }
        let mut value = root.object("output_limits")?;
        let outputs = OutputLimits { binding_bytes: value.size("binding_bytes")?,
            archive_bytes: value.size("archive_bytes")?, report_bytes: value.size("report_bytes")? };
        value.end()?;
        if outputs.binding_bytes == 0 || outputs.binding_bytes > super::fields::MAX_DATA_BYTES
            || outputs.archive_bytes == 0 || outputs.archive_bytes > MAX_FIT_ARCHIVE_BYTES
            || outputs.report_bytes == 0 || outputs.report_bytes > MAX_REPORT_BYTES {
            return Err("invalid output artifact limits".into());
        }
        let mut value = root.object("policy")?;
        let policy = LearnedKvPolicy::new(positive(&mut value, "id")?, positive(&mut value, "generation")?,
            value.size("rank")?, value.size("sweeps")?).map_err(debug)?;
        value.end()?;
        let mut value = root.object("budget")?;
        let mut fit = value.object("fitting")?;
        let fitting = FitBudget { source_values: fit.size("source_values")?, parameter_values: fit.size("parameter_values")?,
            scratch_values: fit.size("scratch_values")?, work_units: fit.number("work_units")? };
        fit.end()?;
        let budget = CodecTrainingBudget { tokens: value.size("tokens")?, source_values: value.size("source_values")?,
            decoder_products: value.number("decoder_products")?, fitting };
        value.end()?;
        if budget.tokens > MAX_TRAINING_ROWS || budget.source_values > MAX_MODEL_KV_VALUES
            || budget.decoder_products > MAX_DECODER_PRODUCTS || fitting.source_values > MAX_MODEL_KV_VALUES
            || fitting.parameter_values > MAX_LEARNED_PARAMETERS || fitting.scratch_values > MAX_FIT_SCRATCH_VALUES
            || fitting.work_units > MAX_FIT_WORK { return Err("training budget exceeds the original limits".into()); }
        let mut documents = Vec::new();
        let mut origins = BTreeSet::new(); let mut streams = BTreeSet::new();
        let mut tokens = 0_usize;
        for value in root.array("documents", MAX_TRAINING_SOURCES)? {
            let mut value = Fields::new(value)?;
            let origin = positive(&mut value, "origin")?; let stream = positive(&mut value, "stream")?;
            let ids = value.ids("tokens", context)?;
            value.end()?;
            if ids.is_empty() || !origins.insert(origin) || !streams.insert(stream) {
                return Err("documents require unique original origins/streams and nonempty token histories".into());
            }
            tokens = tokens.checked_add(ids.len()).ok_or("training token count overflow")?;
            if tokens > MAX_TRAINING_ROWS { return Err("training corpus exceeds original row limit".into()); }
            documents.push(CodecTrainingInput { origin, stream, tokens: ids });
        }
        root.end()?;
        if documents.is_empty() || tokens < 2 { return Err("codec fitting requires at least two original rows".into()); }
        documents.sort_by_key(|document| document.origin);
        Ok(Self { identity, context, inputs, outputs, policy, budget, documents })
    }

    pub fn estimate(&self, profile: &DecoderProfile) -> Result<Estimate, Error> {
        if profile.identity() != self.identity || profile.shape().context != self.context { return Err(Error::Binding); }
        let cache = profile.cache_profile()?;
        let mut tokens = 0_usize; let mut products = 0_u64;
        for document in &self.documents {
            if document.tokens.iter().any(|token| *token as usize >= profile.shape().vocabulary) { return Err(Error::InvalidInput); }
            tokens = tokens.checked_add(document.tokens.len()).ok_or(Error::Overflow)?;
            products = products.checked_add(profile.estimate(0, document.tokens.len())?.scalar_products()?).ok_or(Error::Overflow)?;
        }
        let values = tokens.checked_mul(cache.values_per_token()).ok_or(Error::Overflow)?;
        if tokens > self.budget.tokens || values > self.budget.source_values || products > self.budget.decoder_products {
            return Err(Error::Limit);
        }
        let fit = self.policy.estimate_fit(&cache, tokens, self.budget.fitting)?;
        let sources = self.documents.len();
        let descriptor = MODEL_DESCRIPTOR_HEADER_BYTES + profile.shape().layers * MODEL_LAYER_DESCRIPTOR_BYTES;
        let input_bytes = sources.checked_mul(descriptor).and_then(|n| n.checked_add(values.checked_mul(4)?))
            .ok_or(Error::Overflow)?;
        if input_bytes > MAX_FIT_CHECKPOINT_INPUT_BYTES { return Err(Error::Limit); }
        // Conservative serializer reservations, not a second fitting estimator.
        // Complete opaque comparison material includes every original descriptor,
        // parameter and retained rank diagnostic; reserve its framing in full.
        let witness = 256_usize.checked_add(sources.checked_mul(descriptor + 24).ok_or(Error::Overflow)?)
            .and_then(|n| n.checked_add(fit.parameter_values.checked_mul(4)?))
            .and_then(|n| n.checked_add(fit.groups.checked_mul(128 + 8 * self.policy.rank())?))
            .ok_or(Error::Overflow)?;
        if witness > MAX_FIT_CHECKPOINT_WITNESS_BYTES { return Err(Error::Limit); }
        let archive_reservation = 256_usize.checked_add(input_bytes).and_then(|n| n.checked_add(witness))
            .and_then(|n| n.checked_add(sources.checked_mul(24)?)).ok_or(Error::Overflow)?;
        let binding_reservation = 512_usize.checked_add(sources.checked_mul(128 + 2 * descriptor).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)?;
        let report_reservation = 32_768_usize.checked_add(tokens.checked_mul(12).ok_or(Error::Overflow)?)
            .and_then(|n| n.checked_add(sources.checked_mul(4096)?))
            .and_then(|n| n.checked_add(fit.groups.checked_mul(2048 + self.policy.rank() * 24)?)).ok_or(Error::Overflow)?;
        if archive_reservation > self.outputs.archive_bytes || binding_reservation > self.outputs.binding_bytes
            || report_reservation > self.outputs.report_bytes { return Err(Error::Limit); }
        Ok(Estimate { tokens, source_values: values, decoder_products: products, fit, input_bytes,
            binding_reservation, archive_reservation, report_reservation })
    }

    pub fn begin(&self, model: &DecoderModel) -> Result<CodecTraining, Error> {
        let expected = self.estimate(model.profile())?;
        let owner = model.begin_codec_training(self.documents.clone(), self.policy, self.budget)?;
        let original = owner.report();
        if original.declared_tokens != expected.tokens || original.declared_values != expected.source_values
            || original.admitted_decoder_products != expected.decoder_products { return Err(Error::Binding); }
        Ok(owner)
    }

    pub fn check_completed(&self, estimate: &Estimate, actual: &CodecTrainingReport) -> Result<(), Error> {
        let fit = actual.fit.as_ref().ok_or(Error::Incomplete)?;
        if !actual.fit_attempted || !actual.all_inference_reported
            || !actual.documents.values().all(|document| document.complete)
            || fit.rows_per_group != estimate.fit.rows || fit.groups.len() != estimate.fit.groups
            || fit.training_values != estimate.fit.source_values || fit.parameter_values != estimate.fit.parameter_values
            || fit.scratch_values_reserved != estimate.fit.scratch_values || fit.work_units_reserved != estimate.fit.work_units {
            return Err(Error::Binding);
        }
        let mut products = 0_u64; let mut tokens = 0_u64; let mut values = 0_usize;
        for document in actual.documents.values() {
            products = products.checked_add(document.accepted.scalar_products()?).ok_or(Error::Overflow)?;
            tokens = tokens.checked_add(document.accepted.tokens).ok_or(Error::Overflow)?;
            values = values.checked_add(document.captured_values).ok_or(Error::Overflow)?;
        }
        if products != estimate.decoder_products || tokens != estimate.tokens as u64 || values != estimate.source_values {
            return Err(Error::Binding);
        }
        Ok(())
    }
}
fn positive(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let value = fields.number(name)?;
    if value == 0 { Err(format!("{name} must be nonzero")) } else { Ok(value) }
}
