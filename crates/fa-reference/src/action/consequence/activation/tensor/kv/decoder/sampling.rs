//! Bounded stochastic selection for numerical replay, never effect authority.
//! V1 uses temperature, then top-k, then nucleus truncation in descending score
//! order (lowest token ID breaks numerical ties). Arithmetic is sequential f64.
//! The PRNG is reproducible, NOT cryptographic and NOT an audit-selection source.

mod session;
pub use session::{SampleBudget, SampledCheckpoint, SampledSession, SampledStep, SamplingStart};

use super::MAX_DECODER_VOCABULARY;
use crate::Error;
use std::fmt;

pub const SAMPLER_SNAPSHOT_BYTES: usize = 96;
const DOMAIN: &[u8; 8] = b"FASAMP\0\x01";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingPolicy {
    id: u64,
    generation: u64,
    vocabulary: usize,
    temperature_bits: u64,
    top_k: usize,
    top_p_bits: u64,
}

impl SamplingPolicy {
    /// top_k=0 retains the whole vocabulary. Temperature zero is not a hidden
    /// greedy mode; use the original decoder's explicit greedy API instead.
    pub fn new(id: u64, generation: u64, vocabulary: usize, temperature: f64,
        top_k: usize, top_p: f64) -> Result<Self, Error>
    {
        if id == 0 || generation == 0 || vocabulary == 0 || !temperature.is_finite()
            || !(0.000001..=1_000_000.0).contains(&temperature)
            || !top_p.is_finite() || top_p <= 0.0 || top_p > 1.0
        { return Err(Error::InvalidInput); }
        if vocabulary > MAX_DECODER_VOCABULARY || top_k > vocabulary { return Err(Error::Limit); }
        Ok(Self { id, generation, vocabulary, temperature_bits: temperature.to_bits(),
            top_k, top_p_bits: top_p.to_bits() })
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn vocabulary(&self) -> usize { self.vocabulary }
    pub fn temperature(&self) -> f64 { f64::from_bits(self.temperature_bits) }
    pub fn top_k(&self) -> usize { self.top_k }
    pub fn top_p(&self) -> f64 { f64::from_bits(self.top_p_bits) }

    /// Check bounds before scanning or sorting, including scores discarded by
    /// top-k. NaN/Inf never become an implicit forbidden-token mask.
    pub fn distribution(&self, logits: &[f32], budget: SamplingBudget) -> Result<SamplingDistribution, Error> {
        if budget.vocabulary > MAX_DECODER_VOCABULARY || logits.len() > budget.vocabulary {
            return Err(Error::Limit);
        }
        if logits.len() != self.vocabulary { return Err(Error::Binding); }
        if logits.iter().any(|score| !score.is_finite()) { return Err(Error::InvalidInput); }
        let mut candidates = Vec::new();
        candidates.try_reserve_exact(logits.len()).map_err(|_| Error::Limit)?;
        candidates.extend(logits.iter().enumerate().map(|(token, score)| Candidate {
            token: token as u32, value: f64::from(*score),
        }));
        // Signed zero is a numerical tie, unlike f64::total_cmp's zero ordering.
        candidates.sort_unstable_by(|a, b| b.value.partial_cmp(&a.value)
            .expect("all input scores finite").then_with(|| a.token.cmp(&b.token)));
        let after_top_k = if self.top_k == 0 { candidates.len() } else { self.top_k };
        candidates.truncate(after_top_k);
        let maximum = candidates[0].value;
        let mut mass = 0.0;
        let mut zero_weights = 0;
        for candidate in &mut candidates {
            candidate.value = ((candidate.value - maximum) / self.temperature()).exp();
            if !candidate.value.is_finite() { return Err(Error::Overflow); }
            mass += candidate.value;
            zero_weights += usize::from(candidate.value == 0.0);
        }
        if !mass.is_finite() || mass <= 0.0 { return Err(Error::Overflow); }
        let mut retained_mass = mass;
        if self.top_p() < 1.0 {
            let cutoff = self.top_p() * mass;
            retained_mass = 0.0;
            let mut count = 0;
            for candidate in &candidates {
                retained_mass += candidate.value;
                count += 1;
                if retained_mass >= cutoff { break; }
            }
            candidates.truncate(count);
        }
        let work = SamplingWork { logits_scanned: logits.len(), exponentials: after_top_k,
            retained_candidates: candidates.len(), zero_weights };
        Ok(SamplingDistribution { candidates, mass: retained_mass, work })
    }
}

/// A per-operation admission ceiling, not conserved production/inference rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplingBudget { pub vocabulary: usize }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplingWork {
    pub logits_scanned: usize,
    pub exponentials: usize,
    pub retained_candidates: usize,
    /// Underflowed exponentials before nucleus truncation; not missing scores.
    pub zero_weights: usize,
}

#[derive(Clone, Debug)]
struct Candidate { token: u32, value: f64 }

/// Computed distribution, not a certified probability bound or helper judgment.
/// All top-k/p decisions use this profile's declared rounded arithmetic.
#[derive(Clone)]
pub struct SamplingDistribution { candidates: Vec<Candidate>, mass: f64, work: SamplingWork }
impl fmt::Debug for SamplingDistribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SamplingDistribution").field("work", &self.work).finish_non_exhaustive()
    }
}
impl SamplingDistribution {
    pub fn work(&self) -> SamplingWork { self.work }
    pub fn probabilities(&self) -> impl Iterator<Item = (u32, f64)> + '_ {
        self.candidates.iter().map(|candidate| (candidate.token, candidate.value / self.mass))
    }
    fn select(&self, random_word: u64) -> (u32, f64) {
        // Exactly representable 53-bit grid in [0,1), one word per successful
        // sample even for singleton support. No variable rejection-loop draws.
        let unit = (random_word >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
        let threshold = unit * self.mass;
        let mut cumulative = 0.0;
        let mut last_positive = &self.candidates[0];
        for candidate in &self.candidates {
            cumulative += candidate.value;
            if candidate.value > 0.0 {
                last_positive = candidate;
                if threshold < cumulative { return (candidate.token, candidate.value / self.mass); }
            }
        }
        // Multiplication can round unit*mass to mass. Never choose a zero-weight
        // tail entry as the roundoff fallback; use the final positive candidate.
        (last_positive.token, last_positive.value / self.mass)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampledToken {
    pub token: u32,
    pub stream: u64,
    pub draw: u64,
    pub random_word: u64,
    pub probability: f64,
    pub work: SamplingWork,
}

/// Exact portable sampler state. This is replay data, NOT an authenticated
/// history or a source of secure randomness. Encoding exposes the entire PRNG.
#[derive(Clone, PartialEq, Eq)]
pub struct SamplerSnapshot {
    policy: SamplingPolicy,
    stream: u64,
    draws: u64,
    state: [u64; 4],
}
impl fmt::Debug for SamplerSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SamplerSnapshot").field("policy", &self.policy)
            .field("stream", &self.stream).field("draws", &self.draws).finish_non_exhaustive()
    }
}
impl SamplerSnapshot {
    pub fn policy(&self) -> &SamplingPolicy { &self.policy }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn draws(&self) -> u64 { self.draws }
    pub fn encode(&self) -> [u8; SAMPLER_SNAPSHOT_BYTES] {
        let mut bytes = [0_u8; SAMPLER_SNAPSHOT_BYTES];
        bytes[..8].copy_from_slice(DOMAIN);
        bytes[8..16].copy_from_slice(&self.policy.id.to_be_bytes());
        bytes[16..24].copy_from_slice(&self.policy.generation.to_be_bytes());
        bytes[24..28].copy_from_slice(&(self.policy.vocabulary as u32).to_be_bytes());
        bytes[28..32].copy_from_slice(&(self.policy.top_k as u32).to_be_bytes());
        for (index, value) in [self.policy.temperature_bits, self.policy.top_p_bits, self.stream,
            self.draws, self.state[0], self.state[1], self.state[2], self.state[3]].iter().enumerate()
        { bytes[32 + index * 8..40 + index * 8].copy_from_slice(&value.to_be_bytes()); }
        bytes
    }
    pub fn decode(bytes: &[u8], expected: &SamplingPolicy) -> Result<Self, Error> {
        if bytes.len() != SAMPLER_SNAPSHOT_BYTES || bytes.get(..8) != Some(DOMAIN.as_slice()) {
            return Err(Error::InvalidInput);
        }
        let word = |offset| u64::from_be_bytes(bytes[offset..offset + 8].try_into().expect("fixed framing"));
        let count = |offset| u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("fixed framing")) as usize;
        let policy = SamplingPolicy::new(word(8), word(16), count(24), f64::from_bits(word(32)),
            count(28), f64::from_bits(word(40)))?;
        if &policy != expected { return Err(Error::Binding); }
        let snapshot = Self { policy, stream: word(48), draws: word(56),
            state: [word(64), word(72), word(80), word(88)] };
        if snapshot.stream == 0 || snapshot.state == [0; 4] { return Err(Error::InvalidInput); }
        Ok(snapshot)
    }
}

/// Mutable numerical state only. No clone API: explicit snapshots make the
/// intended replay/fork visible. A sampled decoder commits this only with a token.
#[derive(Debug)]
pub struct Sampler { snapshot: SamplerSnapshot }
struct PreparedSample { next: SamplerSnapshot, sample: SampledToken }

impl Sampler {
    pub fn seeded(policy: SamplingPolicy, stream: u64, seed: u64) -> Result<Self, Error> {
        if stream == 0 { return Err(Error::InvalidInput); }
        let mut seed = seed;
        let state = std::array::from_fn(|_| splitmix(&mut seed));
        Ok(Self { snapshot: SamplerSnapshot { policy, stream, draws: 0, state } })
    }
    pub fn from_snapshot(snapshot: &SamplerSnapshot) -> Self { Self { snapshot: snapshot.clone() } }
    pub fn snapshot(&self) -> SamplerSnapshot { self.snapshot.clone() }
    pub fn sample(&mut self, logits: &[f32], budget: SamplingBudget) -> Result<SampledToken, Error> {
        let prepared = self.prepare(logits, budget)?;
        self.snapshot = prepared.next;
        Ok(prepared.sample)
    }
    fn prepare(&self, logits: &[f32], budget: SamplingBudget) -> Result<PreparedSample, Error> {
        let draw = self.snapshot.draws.checked_add(1).ok_or(Error::Overflow)?;
        let distribution = self.snapshot.policy.distribution(logits, budget)?;
        let mut next = self.snapshot.clone();
        let random_word = xoshiro(&mut next.state);
        next.draws = draw;
        let (token, probability) = distribution.select(random_word);
        let sample = SampledToken { token, stream: next.stream, draw, random_word,
            probability, work: distribution.work() };
        Ok(PreparedSample { next, sample })
    }
}

// Public-domain algorithms by Blackman/Vigna, pinned here as exact V1 integer
// transitions. References: https://prng.di.unimi.it/xoshiro256starstar.c and
// https://prng.di.unimi.it/splitmix64.c . Not cryptographic, including seeded use.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn xoshiro(s: &mut [u64; 4]) -> u64 {
    let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
    let t = s[1] << 17;
    s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
    s[2] ^= t; s[3] = s[3].rotate_left(45);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_and_last_grid_point_never_select_underflowed_mass() {
        let p = SamplingPolicy::new(1, 1, 3, 0.000001, 0, 1.0).unwrap();
        let d = p.distribution(&[0.0, -f32::MAX, -f32::MAX], SamplingBudget { vocabulary: 3 }).unwrap();
        assert_eq!(d.work().zero_weights, 2);
        assert_eq!(d.select(0), (0, 1.0));
        assert_eq!(d.select(u64::MAX), (0, 1.0));
        let uniform = SamplingPolicy::new(1, 1, 4, 1.0, 0, 1.0).unwrap()
            .distribution(&[0.0; 4], SamplingBudget { vocabulary: 4 }).unwrap();
        assert_eq!(uniform.select(0).0, 0);
        assert_eq!(uniform.select(1_u64 << 62).0, 1);
        assert_eq!(uniform.select(u64::MAX).0, 3);
    }
}
