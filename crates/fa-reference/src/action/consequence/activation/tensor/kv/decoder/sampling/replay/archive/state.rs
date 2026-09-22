//! Untrusted numerical expectations. No saved tensor, draw or counter is installed.
use super::{ArchiveLimits, Recipe, State, wire::{Reader, Writer}};
use super::super::{GenerationStatus, GenerationTelemetryWork, GenerationWork,
    SampledToken, SamplerSnapshot, SAMPLER_SNAPSHOT_BYTES, copy_slice};
use super::super::super::{SamplingWork, monitored::GenerationStop};
use super::super::super::super::DecoderWork;
use super::super::super::super::super::model::{MODEL_DESCRIPTOR_HEADER_BYTES, MODEL_LAYER_DESCRIPTOR_BYTES};
use crate::Error;

const COMPONENTS: u64 = 63;
const FIXED: usize = 352;

pub(super) fn size(state: &State) -> Result<usize, Error> {
    layout(state.tokens.len(), state.samples.len(), state.logits.as_ref().map_or(0, Vec::len), state.cache.len())
}
fn layout(tokens: usize, samples: usize, logits: usize, cache: usize) -> Result<usize, Error> {
    FIXED.checked_add(tokens.checked_mul(4).ok_or(Error::Limit)?)
        .and_then(|n| n.checked_add(samples.checked_mul(68)?))
        .and_then(|n| n.checked_add(logits.checked_mul(4)?))
        .and_then(|n| n.checked_add(cache)).ok_or(Error::Limit)
}
pub(super) fn write(w: &mut Writer<'_>, state: &State) -> Result<(), Error> {
    if state.logical_bytes != size(state)? { return Err(Error::Binding); }
    w.u64(COMPONENTS)?; w.size(state.logical_bytes)?; w.size(state.tokens.len())?;
    let (tag, stop) = match state.status {
        GenerationStatus::Prefilling => (0, 0),
        GenerationStatus::Generating => (1, 0),
        GenerationStatus::Finished(GenerationStop::TokenLimit) => (2, 0),
        GenerationStatus::Finished(GenerationStop::StopToken(token)) => (3, u64::from(token)),
        _ => return Err(Error::WrongState),
    };
    w.u64(tag)?; w.u64(stop)?;
    for count in [state.tokens.len(), state.samples.len(), state.logits.as_ref().map_or(0, Vec::len), state.cache.len()] {
        w.size(count)?;
    }
    let g = state.work; let d = g.accepted_decoder;
    for value in [g.admitted_tokens, g.reserved_decoder_products, g.sampling_attempts,
        g.reserved_vocabulary_scores, d.tokens, d.matrix_products, d.attention_products,
        d.attention_exponentials, d.normalization_coordinates, d.rotary_pairs, d.gate_coordinates,
        d.cache_values_appended] { w.u64(value)?; }
    let t = state.telemetry;
    for value in [t.compression_source_values, t.compression_encoded_bytes, t.compression_work_units,
        t.source_check_values, t.source_check_encoded_bytes, t.source_check_reconstruction_products,
        t.monitor_encoded_bytes, t.monitor_probe_coordinates, t.monitor_reconstruction_products,
        t.monitor_materialized_values, t.monitor_refinements] { w.u64(value)?; }
    w.bytes(&state.sampler.encode())?;
    for token in &state.tokens { w.u32(*token)?; }
    for sample in &state.samples {
        w.u32(sample.token)?;
        for value in [sample.stream, sample.draw, sample.random_word, sample.probability.to_bits()] { w.u64(value)?; }
        for count in [sample.work.logits_scanned, sample.work.exponentials,
            sample.work.retained_candidates, sample.work.zero_weights] { w.size(count)?; }
    }
    for logit in state.logits.iter().flatten() { w.u32(*logit)?; }
    w.bytes(&state.cache)
}

pub(super) fn read(bytes: &[u8], recipe: &Recipe, limits: ArchiveLimits) -> Result<State, Error> {
    let mut r = Reader::new(bytes);
    if r.u64()? != COMPONENTS { return Err(Error::Incomplete); }
    let logical_bytes = r.count(limits.state.state_bytes)?;
    if logical_bytes != bytes.len() { return Err(Error::Binding); }
    let positions = r.count(limits.state.positions)?;
    let tag = r.u64()?; let stop = r.u64()?;
    let status = match (tag, stop) {
        (0, 0) => GenerationStatus::Prefilling,
        (1, 0) => GenerationStatus::Generating,
        (2, 0) => GenerationStatus::Finished(GenerationStop::TokenLimit),
        (3, token) => GenerationStatus::Finished(GenerationStop::StopToken(
            u32::try_from(token).map_err(|_| Error::InvalidInput)?)),
        _ => return Err(Error::InvalidInput),
    };
    let tokens_len = r.count(limits.state.positions)?;
    let samples_len = r.count(recipe.spec.max_new_tokens())?;
    let vocabulary = recipe.model.profile().shape().vocabulary;
    let logits_len = r.count(vocabulary)?;
    let cache_len = r.count(limits.state.state_bytes)?;
    let maximum_positions = recipe.spec.prompt().len().checked_add(recipe.spec.max_new_tokens()).ok_or(Error::Limit)?;
    let expected_cache = MODEL_DESCRIPTOR_HEADER_BYTES.checked_add(
        recipe.model.cache_profile().layers().len().checked_mul(MODEL_LAYER_DESCRIPTOR_BYTES).ok_or(Error::Limit)?)
        .and_then(|n| n.checked_add(recipe.model.cache_profile().values_per_token().checked_mul(tokens_len)?.checked_mul(4)?))
        .ok_or(Error::Limit)?;
    // Entire declared state size is checked BEFORE any attacker-counted Vec.
    if tokens_len != positions || positions > maximum_positions
        || samples_len != positions.saturating_sub(recipe.spec.prompt().len())
        || logits_len != if positions == 0 { 0 } else { vocabulary }
        || cache_len != expected_cache || layout(tokens_len, samples_len, logits_len, cache_len)? != bytes.len()
    { return Err(Error::Binding); }
    let work = GenerationWork {
        admitted_tokens: r.u64()?, reserved_decoder_products: r.u64()?, sampling_attempts: r.u64()?,
        reserved_vocabulary_scores: r.u64()?, accepted_decoder: DecoderWork {
            tokens: r.u64()?, matrix_products: r.u64()?, attention_products: r.u64()?,
            attention_exponentials: r.u64()?, normalization_coordinates: r.u64()?,
            rotary_pairs: r.u64()?, gate_coordinates: r.u64()?, cache_values_appended: r.u64()?,
        },
    };
    let telemetry = GenerationTelemetryWork {
        compression_source_values: r.u64()?, compression_encoded_bytes: r.u64()?, compression_work_units: r.u64()?,
        source_check_values: r.u64()?, source_check_encoded_bytes: r.u64()?, source_check_reconstruction_products: r.u64()?,
        monitor_encoded_bytes: r.u64()?, monitor_probe_coordinates: r.u64()?, monitor_reconstruction_products: r.u64()?,
        monitor_materialized_values: r.u64()?, monitor_refinements: r.u64()?,
    };
    let sampler = SamplerSnapshot::decode(r.take(SAMPLER_SNAPSHOT_BYTES)?, &recipe.spec.sampling().policy)?;
    if work.admitted_tokens != positions as u64 || work.accepted_decoder.tokens != positions as u64
        || work.sampling_attempts != samples_len as u64 || sampler.draws() != samples_len as u64
        || sampler.stream() != recipe.spec.sampling().stream { return Err(Error::Binding); }
    let mut tokens = Vec::new(); tokens.try_reserve_exact(tokens_len).map_err(|_| Error::Limit)?;
    for _ in 0..tokens_len {
        let token = r.u32()?;
        if token as usize >= vocabulary { return Err(Error::InvalidInput); }
        tokens.push(token);
    }
    let mut samples = Vec::new(); samples.try_reserve_exact(samples_len).map_err(|_| Error::Limit)?;
    for index in 0..samples_len {
        let sample = SampledToken { token: r.u32()?, stream: r.u64()?, draw: r.u64()?, random_word: r.u64()?,
            probability: f64::from_bits(r.u64()?), work: SamplingWork {
                logits_scanned: r.count(vocabulary)?, exponentials: r.count(vocabulary)?,
                retained_candidates: r.count(vocabulary)?, zero_weights: r.count(vocabulary)?,
            },
        };
        if sample.token as usize >= vocabulary || sample.token != tokens[recipe.spec.prompt().len() + index]
            || sample.stream != sampler.stream() || sample.draw != index as u64 + 1
            || !sample.probability.is_finite() || !(0.0..=1.0).contains(&sample.probability)
            || sample.work.logits_scanned != vocabulary || sample.work.retained_candidates == 0
            || sample.work.retained_candidates > sample.work.exponentials
            || sample.work.zero_weights > sample.work.exponentials { return Err(Error::Binding); }
        samples.push(sample);
    }
    let logits = if logits_len == 0 { None } else {
        let mut logits = Vec::new(); logits.try_reserve_exact(logits_len).map_err(|_| Error::Limit)?;
        for _ in 0..logits_len {
            let bits = r.u32()?;
            if !f32::from_bits(bits).is_finite() { return Err(Error::InvalidInput); }
            logits.push(bits);
        }
        Some(logits)
    };
    // Kept as exact comparison bytes, never decoded into the candidate cache.
    let cache = copy_slice(r.take(cache_len)?)?; r.end()?;
    Ok(State { status, work, telemetry, tokens, samples, sampler, logits, cache, logical_bytes })
}
