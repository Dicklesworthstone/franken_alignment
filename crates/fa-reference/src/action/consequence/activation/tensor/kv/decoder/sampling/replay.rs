//! Exact reconstruction of learned-audited generation, not restoration of rights.
//!
//! Checkpoints retain the immutable original recipe. Reconstruction runs EVERY
//! prompt/sample through LearnedGeneration again; saved cache words, RNG state,
//! audit outcomes and work counters are comparison material, never installed.
//! This is the token-recomputation restart baseline, not a cheap KV restart.
use super::monitored::{
    GenerationBudget, GenerationEvent, GenerationSpec, GenerationStatus,
    GenerationTelemetryBudget, GenerationTelemetryWork, GenerationWork, LearnedGeneration,
    MAX_GENERATION_SCORES, MAX_GENERATION_TOKENS,
};
use super::super::monitoring::LearnedDecoderPolicy;
use super::super::{DecoderModel, MAX_DECODER_PRODUCTS, MAX_DECODER_VOCABULARY};
use super::super::super::model::MAX_MODEL_IMAGE_BYTES;
use super::{SampledToken, SamplerSnapshot, SAMPLER_SNAPSHOT_BYTES};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// Logical retained state bytes, including the canonical cache, not allocator
/// overhead or peak RSS. The recipe shares immutable model/codebook parameters;
/// monitor definitions and prompt retention remain bounded by their native APIs.
pub const MAX_REPLAY_STATE_BYTES: usize = MAX_MODEL_IMAGE_BYTES
    + 4 * MAX_DECODER_VOCABULARY + 72 * MAX_GENERATION_TOKENS
    + SAMPLER_SNAPSHOT_BYTES + 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointLimits {
    pub positions: usize,
    pub state_bytes: usize,
}
impl Default for CheckpointLimits {
    fn default() -> Self {
        Self { positions: MAX_GENERATION_TOKENS, state_bytes: MAX_REPLAY_STATE_BYTES }
    }
}
impl CheckpointLimits {
    fn check(self) -> Result<(), Error> {
        if self.positions > MAX_GENERATION_TOKENS || self.state_bytes > MAX_REPLAY_STATE_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Extra work allowed to reconstruct the saved prefix. This does NOT refill the
/// generation's original decoder/sampler or telemetry budget. Replayed telemetry
/// runs under the original frozen aggregate and per-token caps, not a fresh cap
/// for each position. No wall-clock or physical allocation bound is claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayBudget {
    pub positions: usize,
    pub decoder_products: u64,
    pub vocabulary_scores: u64,
    pub state_bytes: usize,
}
impl Default for ReplayBudget {
    fn default() -> Self {
        Self { positions: MAX_GENERATION_TOKENS, decoder_products: MAX_DECODER_PRODUCTS,
            vocabulary_scores: MAX_GENERATION_SCORES, state_bytes: MAX_REPLAY_STATE_BYTES }
    }
}

struct Recipe {
    model: DecoderModel,
    stream: u64,
    evaluation_origin: u64,
    spec: GenerationSpec,
    policy: LearnedDecoderPolicy,
    budget: GenerationBudget,
    telemetry: GenerationTelemetryBudget,
}
impl Recipe {
    fn start(&self) -> Result<LearnedGeneration, Error> {
        self.model.monitored_generation_with_telemetry(self.stream, self.evaluation_origin,
            self.spec.clone(), self.policy.clone(), self.budget, self.telemetry)
    }
}

/// Owns the ORIGINAL learned generation and its immutable construction inputs.
/// No API adopts an already advanced/held run or returns a mutable inner owner.
/// Checkpoint replay creates a separate numerical computation; it never resets
/// this owner, revives an effect permit, or counts as independent source evidence.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::ReplayableGeneration;
/// fn bypass(run: &mut ReplayableGeneration) { run.generation_mut(); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::ReplayableGeneration;
/// fn clear_hold(run: &mut ReplayableGeneration) { run.reset(); }
/// ```
pub struct ReplayableGeneration {
    recipe: Rc<Recipe>,
    run: LearnedGeneration,
}
impl fmt::Debug for ReplayableGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayableGeneration").field("position", &self.run.position())
            .field("status", &self.run.status()).finish_non_exhaustive()
    }
}
impl DecoderModel {
    /// Uses the original constructor and admission rules. A zero prefix may be
    /// checkpointed, but it does not establish an observed or permitted action.
    pub fn replayable_monitored_generation(&self, stream: u64, evaluation_origin: u64,
        spec: GenerationSpec, policy: LearnedDecoderPolicy, budget: GenerationBudget,
        telemetry: GenerationTelemetryBudget) -> Result<ReplayableGeneration, Error>
    {
        let recipe = Rc::new(Recipe { model: self.clone(), stream, evaluation_origin,
            spec, policy, budget, telemetry });
        let run = recipe.start()?;
        Ok(ReplayableGeneration { recipe, run })
    }
}
impl ReplayableGeneration {
    pub fn generation(&self) -> &LearnedGeneration { &self.run }
    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<GenerationEvent>, Error> {
        self.run.advance(expected_position)
    }
    pub fn run_to_stop(&mut self) -> Result<GenerationStatus, Error> { self.run.run_to_stop() }

    /// Capture only an entirely accepted prefix, retaining its spent lifetime
    /// allowances. A held/failed attempt cannot be laundered into a resumable
    /// checkpoint of the shorter accepted prefix. This operation does not mutate
    /// the run, and old snapshots never change a later owner's terminal status.
    pub fn checkpoint(&self, limits: CheckpointLimits) -> Result<GenerationCheckpoint, Error> {
        let expected = State::capture(&self.run, limits)?;
        Ok(GenerationCheckpoint { recipe: Rc::clone(&self.recipe), expected: Rc::new(expected) })
    }
}

#[derive(Clone)]
struct State {
    status: GenerationStatus,
    work: GenerationWork,
    telemetry: GenerationTelemetryWork,
    tokens: Vec<u32>,
    samples: Vec<SampledToken>,
    sampler: SamplerSnapshot,
    logits: Option<Vec<u32>>,
    cache: Vec<u8>,
    logical_bytes: usize,
}
impl State {
    fn capture(run: &LearnedGeneration, limits: CheckpointLimits) -> Result<Self, Error> {
        limits.check()?;
        if !(run.status().is_active() || matches!(run.status(), GenerationStatus::Finished(_))) {
            return Err(Error::WrongState);
        }
        let count = run.accepted_tokens().len();
        if count > limits.positions { return Err(Error::Limit); }
        let samples = count.saturating_sub(run.spec().prompt().len());
        if run.position() != count as u64 || run.work().admitted_tokens != count as u64
            || run.work().accepted_decoder.tokens != count as u64 || run.samples().len() != samples
            || run.sampler_state().draws() != samples as u64 { return Err(Error::Binding); }
        let image = run.accepted_cache_image()?;
        let cache_bytes = image.descriptor().image_len()?;
        let logits = if count == 0 {
            if run.accepted_logits() != Err(Error::Incomplete) { return Err(Error::Binding); }
            None
        } else { Some(run.accepted_logits()?) };
        // A sampled record has four u64 fields (including probability bits), one
        // u32 ID, and four u64 logical work counts. Count the remaining fixed
        // status/work metadata conservatively, separately from Vec overhead.
        let logical_bytes = cache_bytes.checked_add(256 + SAMPLER_SNAPSHOT_BYTES)
            .and_then(|n| n.checked_add(count.checked_mul(4)?))
            .and_then(|n| n.checked_add(samples.checked_mul(68)?))
            .and_then(|n| n.checked_add(logits.map_or(0, <[f32]>::len).checked_mul(4)?))
            .ok_or(Error::Overflow)?;
        if logical_bytes > limits.state_bytes { return Err(Error::Limit); }
        let cache = image.encode()?;
        if cache.len() != cache_bytes { return Err(Error::Binding); }
        Ok(Self { status: run.status(), work: run.work(), telemetry: run.telemetry_work(),
            tokens: copy_slice(run.accepted_tokens())?, samples: copy_slice(run.samples())?,
            sampler: run.sampler_state(), logits: logits.map(|values| {
                let mut bits = Vec::new();
                bits.try_reserve_exact(values.len()).map_err(|_| Error::Limit)?;
                bits.extend(values.iter().map(|value| value.to_bits()));
                Ok::<_, Error>(bits)
            }).transpose()?, cache, logical_bytes })
    }
    fn matches(&self, actual: &Self) -> bool {
        self.status == actual.status && self.work == actual.work && self.telemetry == actual.telemetry
            && self.tokens == actual.tokens && self.sampler == actual.sampler
            && self.logits == actual.logits && self.cache == actual.cache
            && self.logical_bytes == actual.logical_bytes && self.samples.len() == actual.samples.len()
            && self.samples.iter().zip(&actual.samples).all(|(a, b)| same_sample(a, b))
    }
}
fn copy_slice<T: Clone>(values: &[T]) -> Result<Vec<T>, Error> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len()).map_err(|_| Error::Limit)?;
    copy.extend_from_slice(values);
    Ok(copy)
}
fn same_sample(a: &SampledToken, b: &SampledToken) -> bool {
    a.token == b.token && a.stream == b.stream && a.draw == b.draw
        && a.random_word == b.random_word && a.probability.to_bits() == b.probability.to_bits()
        && a.work == b.work
}

/// Typed checkpoint created only by its owner. Immutable model, codec, monitor
/// coefficients, retention, prompt and budget inputs are retained, not accepted
/// from IDs, Debug text or an untrusted serialized object. Cloning this evidence
/// does not clone a generation owner or grant a right to an external effect.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::tensor::kv::decoder::sampling::replay::GenerationCheckpoint};
/// fn authorize(checkpoint: GenerationCheckpoint) -> Permit { checkpoint }
/// ```
#[derive(Clone)]
pub struct GenerationCheckpoint {
    recipe: Rc<Recipe>,
    expected: Rc<State>,
}
impl fmt::Debug for GenerationCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationCheckpoint").field("positions", &self.positions())
            .field("status", &self.expected.status).field("state_bytes", &self.state_bytes())
            .finish_non_exhaustive()
    }
}
impl GenerationCheckpoint {
    pub fn positions(&self) -> usize { self.expected.tokens.len() }
    pub fn state_bytes(&self) -> usize { self.expected.logical_bytes }
    pub fn status(&self) -> GenerationStatus { self.expected.status }
    pub fn work(&self) -> GenerationWork { self.expected.work }
    pub fn telemetry_work(&self) -> GenerationTelemetryWork { self.expected.telemetry }

    /// Check the whole requested reconstruction before creating its original
    /// generator. Replaying the prefix must also pass all original learned audits;
    /// no saved outcome, sampler, word array or work counter is installed.
    pub fn begin_replay(&self, budget: ReplayBudget) -> Result<GenerationReplay, Error> {
        if budget.positions > MAX_GENERATION_TOKENS || budget.decoder_products > MAX_DECODER_PRODUCTS
            || budget.vocabulary_scores > MAX_GENERATION_SCORES || budget.state_bytes > MAX_REPLAY_STATE_BYTES
            || self.positions() > budget.positions || self.state_bytes() > budget.state_bytes {
            return Err(Error::Limit);
        }
        let products = self.recipe.model.estimate(0, self.positions())?.scalar_products()?;
        let scores = (self.expected.samples.len() as u64)
            .checked_mul(self.recipe.spec.sampling().policy.vocabulary() as u64).ok_or(Error::Overflow)?;
        if products > budget.decoder_products || scores > budget.vocabulary_scores { return Err(Error::Limit); }
        if products != self.expected.work.reserved_decoder_products
            || scores != self.expected.work.reserved_vocabulary_scores { return Err(Error::Binding); }
        Ok(GenerationReplay { checkpoint: self.clone(), candidate: self.recipe.start()?,
            compared: 0, status: ReplayStatus::Pending { compared: 0, remaining: self.positions() },
            limits: CheckpointLimits { positions: budget.positions, state_bytes: budget.state_bytes },
            receipt: None })
    }
    pub fn replay(&self, budget: ReplayBudget) -> Result<(ReplayableGeneration, ReplayReceipt), Error> {
        let mut replay = self.begin_replay(budget)?;
        replay.advance(self.positions())?;
        replay.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayStatus {
    Pending { compared: usize, remaining: usize },
    Verified,
    Failed(Error),
}

/// Newly incurred recomputation, separate from the returned run's historical
/// spend. The identical evaluation origin/stream is preserved for comparison;
/// it is not a new independent observation or a freshness/authentication claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayReceipt {
    pub stream: u64,
    pub evaluation_origin: u64,
    pub positions: usize,
    pub sampled_positions: usize,
    pub cache_bytes_compared: usize,
    pub logits_compared: usize,
    pub recomputation: GenerationWork,
    pub telemetry_recomputation: GenerationTelemetryWork,
    pub restored_status: GenerationStatus,
}

/// Partial reconstruction has no accessor for its candidate owner, logits, cache
/// or sampler. Dropping it abandons extra numerical work, not a production right.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::GenerationReplay;
/// fn incomplete(replay: &mut GenerationReplay) { replay.generation_mut(); }
/// ```
pub struct GenerationReplay {
    checkpoint: GenerationCheckpoint,
    candidate: LearnedGeneration,
    compared: usize,
    status: ReplayStatus,
    limits: CheckpointLimits,
    receipt: Option<ReplayReceipt>,
}
impl fmt::Debug for GenerationReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationReplay").field("status", &self.status).finish_non_exhaustive()
    }
}
impl GenerationReplay {
    pub fn status(&self) -> ReplayStatus { self.status }
    pub fn receipt(&self) -> Option<&ReplayReceipt> { self.receipt.as_ref() }

    /// At most `positions` token computations. Zero makes no progress on a
    /// nonempty prefix. A completely empty checkpoint still needs its final
    /// state comparison; reaching zero remaining positions alone is not success.
    pub fn advance(&mut self, positions: usize) -> Result<ReplayStatus, Error> {
        match self.status {
            ReplayStatus::Verified => return Ok(self.status),
            ReplayStatus::Failed(error) => return Err(error),
            ReplayStatus::Pending { .. } => {}
        }
        // A caught unwind may not retry a half-compared computation.
        self.status = ReplayStatus::Failed(Error::Incomplete);
        match self.advance_inner(positions) {
            Ok(status) => { self.status = status; Ok(status) }
            Err(error) => { self.status = ReplayStatus::Failed(error); Err(error) }
        }
    }
    fn advance_inner(&mut self, positions: usize) -> Result<ReplayStatus, Error> {
        let total = self.checkpoint.positions();
        let end = self.compared + positions.min(total - self.compared);
        while self.compared < end {
            let event = self.candidate.advance(self.compared as u64)?;
            let step = event.accepted().ok_or(Error::Binding)?;
            if !event.audit().complete_quiet() || step.position != self.compared as u64
                || step.token != self.checkpoint.expected.tokens[self.compared] { return Err(Error::Binding); }
            if self.compared >= self.checkpoint.recipe.spec.prompt().len() {
                let index = self.compared - self.checkpoint.recipe.spec.prompt().len();
                let expected = self.checkpoint.expected.samples.get(index).ok_or(Error::Binding)?;
                if !event.sample().is_some_and(|sample| same_sample(expected, sample)) { return Err(Error::Binding); }
            } else if event.sample().is_some() { return Err(Error::Binding); }
            self.compared += 1;
        }
        if self.compared != total {
            return Ok(ReplayStatus::Pending { compared: self.compared, remaining: total - self.compared });
        }
        let actual = State::capture(&self.candidate, self.limits)?;
        if !self.checkpoint.expected.matches(&actual) { return Err(Error::Binding); }
        self.receipt = Some(ReplayReceipt {
            stream: self.checkpoint.recipe.stream, evaluation_origin: self.checkpoint.recipe.evaluation_origin,
            positions: total, sampled_positions: actual.samples.len(), cache_bytes_compared: actual.cache.len(),
            logits_compared: actual.logits.as_ref().map_or(0, Vec::len), recomputation: actual.work,
            telemetry_recomputation: actual.telemetry, restored_status: actual.status,
        });
        Ok(ReplayStatus::Verified)
    }
    pub fn finish(self) -> Result<(ReplayableGeneration, ReplayReceipt), Error> {
        match self.status {
            ReplayStatus::Pending { .. } => Err(Error::Incomplete),
            ReplayStatus::Failed(error) => Err(error),
            ReplayStatus::Verified => {
                let receipt = self.receipt.ok_or(Error::Incomplete)?;
                Ok((ReplayableGeneration { recipe: self.checkpoint.recipe, run: self.candidate }, receipt))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{SamplingPolicy, SamplingStart};
    use crate::action::consequence::activation::monitor::learned::{
        LearnedMonitorBudget, LearnedRefinementMonitor,
        model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
    };
    use crate::action::consequence::activation::probe::LinearProbe;
    use crate::action::consequence::activation::tensor::kv::decoder::{
        DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderProfile, DecoderShape,
        monitoring::LearnedStreamRetention,
    };
    use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
    use crate::action::consequence::activation::tensor::kv::model::learned::{
        FitBudget, LearnedKvCodec, LearnedKvPolicy,
    };
    use super::super::monitored::GenerationStop;
    use std::collections::{BTreeMap, BTreeSet};

    fn checkpoint() -> GenerationCheckpoint {
        let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
            model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
            DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
                query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
        let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
            keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
            feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
        let model = DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0],
            vec![layer.clone(), layer], vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
        let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
        let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
        let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
            &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
        let mut taps = BTreeMap::new();
        for (layer, contract) in model.cache_profile().layers() {
            for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
                let probe = LinearProbe::new(1, 1, tensor.profile(), &vec![0.0; tensor.dimensions()], 0.0, 1.0).unwrap();
                taps.insert(KvTap { layer: *layer, side },
                    LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
            }
        }
        let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
        let policy = LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
            LearnedAuditPreparationBudget::default(), inference).unwrap();
        let spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap();
        let mut run = model.replayable_monitored_generation(21, 201, spec, policy,
            GenerationBudget::default(), GenerationTelemetryBudget::default()).unwrap();
        run.advance(0).unwrap(); run.advance(1).unwrap();
        run.checkpoint(CheckpointLimits::default()).unwrap()
    }

    #[test]
    fn saved_state_is_comparison_material_never_installed_as_success() {
        let original = checkpoint();
        original.replay(ReplayBudget::default()).unwrap();
        for field in 0..10 {
            let mut altered = original.clone();
            let expected = Rc::make_mut(&mut altered.expected);
            match field {
                0 => expected.tokens[1] ^= 1,
                1 => expected.samples[0].random_word ^= 1,
                2 => expected.samples[0].probability = f64::from_bits(expected.samples[0].probability.to_bits() ^ 1),
                3 => {
                    let mut bytes = expected.sampler.encode(); bytes[SAMPLER_SNAPSHOT_BYTES - 1] ^= 1;
                    expected.sampler = SamplerSnapshot::decode(&bytes, expected.sampler.policy()).unwrap();
                }
                4 => expected.logits.as_mut().unwrap()[0] ^= 1,
                5 => *expected.cache.last_mut().unwrap() ^= 1,
                6 => expected.telemetry.monitor_encoded_bytes += 1,
                7 => expected.work.reserved_decoder_products += 1,
                8 => expected.status = GenerationStatus::Finished(GenerationStop::TokenLimit),
                9 => expected.logical_bytes += 1,
                _ => unreachable!(),
            }
            let mut replay = match altered.begin_replay(ReplayBudget::default()) {
                Err(error) => { assert_eq!(error, Error::Binding); continue; }
                Ok(replay) => replay,
            };
            assert_eq!(replay.advance(usize::MAX), Err(Error::Binding), "field {field}");
            assert_eq!(replay.status(), ReplayStatus::Failed(Error::Binding));
            assert!(replay.receipt().is_none());
            assert_eq!(replay.advance(0), Err(Error::Binding));
            assert!(matches!(replay.finish(), Err(Error::Binding)));
        }
        // Mutating the test's expected data never changes the original checkpoint
        // or its immutable recipe. The permitted control still reconstructs.
        original.replay(ReplayBudget::default()).unwrap();
    }
}
