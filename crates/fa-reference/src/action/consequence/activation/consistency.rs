//! Activation-conditioned binary forecasts and exact sequential evidence.
//!
//! The null is CONDITIONAL calibration of the registered forecast p, given the
//! entire prior history. For event Y, the factor is q/p if Y, otherwise
//! (1-q)/(1-p). Its conditional expectation is one under that null. Neither
//! source checking nor these constructors establish calibration or independence.
//! A threshold crossing requests containment; a quiet process is not permission.

use super::{CaptureProfile, ProgressiveFrame, SourceFrame};
use super::probe::{LinearProbe, ProbeObservation, ProbeOutcome};
use crate::Error;
use std::cmp::Ordering;

pub const PROBABILITY_SCALE: u32 = 65_536;
pub const MAX_SAMPLES: usize = 512;
pub const LIKELIHOOD_WORDS: usize = 129;
pub const MAX_EVENT_PREFIX_BYTES: usize = 4_096;

/// Strictly positive support for BOTH categories in both distributions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinaryForecast { null: u32, alternative: u32 }

impl BinaryForecast {
    pub fn new(null: u32, alternative: u32) -> Result<Self, Error> {
        if null == 0 || alternative == 0 || null >= PROBABILITY_SCALE || alternative >= PROBABILITY_SCALE {
            return Err(Error::InvalidInput);
        }
        Ok(Self { null, alternative })
    }
    pub fn null_numerator(self) -> u32 { self.null }
    pub fn alternative_numerator(self) -> u32 { self.alternative }
    pub fn factor(self, event: bool) -> LikelihoodFactor {
        if event { LikelihoodFactor { numerator: self.alternative, denominator: self.null } }
        else { LikelihoodFactor { numerator: PROBABILITY_SCALE - self.alternative,
            denominator: PROBABILITY_SCALE - self.null } }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LikelihoodFactor { pub numerator: u32, pub denominator: u32 }

/// Lifetime alpha = numerator/denominator, not a posterior probability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorBudget { numerator: u64, denominator: u64 }

impl ErrorBudget {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, Error> {
        if numerator == 0 || numerator >= denominator { return Err(Error::InvalidInput); }
        Ok(Self { numerator, denominator })
    }
    pub fn numerator(self) -> u64 { self.numerator }
    pub fn denominator(self) -> u64 { self.denominator }
}

/// Exact positive integer, little-endian words. At most 512 factors each below
/// 2^16 need 8192 bits, plus 64 for the alpha cross-product: 129 words suffice.
/// No float/log underflow, clipping, renormalization or approximate threshold.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Natural([u64; LIKELIHOOD_WORDS]);

impl Natural {
    fn one() -> Self { let mut words = [0; LIKELIHOOD_WORDS]; words[0] = 1; Self(words) }
    fn times(&self, factor: u64) -> Result<Self, Error> {
        let mut result = [0; LIKELIHOOD_WORDS];
        let mut carry = 0_u128;
        for (out, word) in result.iter_mut().zip(self.0.iter().copied()) {
            let value = u128::from(word) * u128::from(factor) + carry;
            *out = value as u64;
            carry = value >> 64;
        }
        if carry != 0 { return Err(Error::Overflow); }
        Ok(Self(result))
    }
    fn cmp(&self, other: &Self) -> Ordering { self.0.iter().rev().cmp(other.0.iter().rev()) }
}

/// A bounded in-memory e-process oracle. Cloning copies historical evidence,
/// never an error-budget allocation or a controller's authority. The owning
/// broker exposes no replacement/reset path for its instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LikelihoodEvidence {
    alpha: ErrorBudget,
    numerator: Natural,
    denominator: Natural,
    samples: usize,
    first_crossing: Option<usize>,
}

impl LikelihoodEvidence {
    pub fn new(alpha: ErrorBudget) -> Self {
        Self { alpha, numerator: Natural::one(), denominator: Natural::one(), samples: 0, first_crossing: None }
    }
    pub fn alpha(&self) -> ErrorBudget { self.alpha }
    pub fn samples(&self) -> usize { self.samples }
    pub fn first_crossing(&self) -> Option<usize> { self.first_crossing }
    pub fn crossed(&self) -> bool { self.first_crossing.is_some() }
    pub fn numerator_words(&self) -> &[u64; LIKELIHOOD_WORDS] { &self.numerator.0 }
    pub fn denominator_words(&self) -> &[u64; LIKELIHOOD_WORDS] { &self.denominator.0 }

    /// Trusted logical inputs here. The live broker obtains its forecast from
    /// its pre-action predictor and the category from actual proposal bytes.
    /// Refusal is atomic; crossing latches even if subsequent evidence declines.
    pub fn observe(&mut self, forecast: BinaryForecast, event: bool) -> Result<LikelihoodFactor, Error> {
        if self.samples >= MAX_SAMPLES { return Err(Error::Limit); }
        let factor = forecast.factor(event);
        let numerator = self.numerator.times(u64::from(factor.numerator))?;
        let denominator = self.denominator.times(u64::from(factor.denominator))?;
        let crosses = numerator.times(self.alpha.numerator)?.cmp(&denominator.times(self.alpha.denominator)?) != Ordering::Less;
        let samples = self.samples + 1;
        self.numerator = numerator;
        self.denominator = denominator;
        self.samples = samples;
        if crosses && self.first_crossing.is_none() { self.first_crossing = Some(samples); }
        Ok(factor)
    }
}

/// A frozen, declared calibration table over three exact linear-score bands.
/// These probabilities are supplied registration data, not estimated or
/// qualified by this code. The event is an operational byte-prefix category,
/// not a ground-truth label such as maliciousness.
#[derive(Clone, Debug)]
pub struct ForecastRegistration {
    pub domain: u64,
    pub generation: u64,
    pub policy_generation: u64,
    pub event_prefix: Vec<u8>,
    pub negative: BinaryForecast,
    pub at_threshold: BinaryForecast,
    pub positive: BinaryForecast,
}

#[derive(Clone, Debug)]
pub struct ForecastModel { probe: LinearProbe, registration: ForecastRegistration }

/// Exact observed score, selected probability pair and measured encoded length.
/// Contains no live permits, mutable source, or controller reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prediction {
    observation: ProbeObservation,
    forecast: BinaryForecast,
    domain: u64,
    generation: u64,
    policy_generation: u64,
    encoded_bytes: usize,
}

impl Prediction {
    pub fn observation(&self) -> &ProbeObservation { &self.observation }
    pub fn forecast(&self) -> BinaryForecast { self.forecast }
    pub fn domain(&self) -> u64 { self.domain }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn policy_generation(&self) -> u64 { self.policy_generation }
    pub fn encoded_bytes(&self) -> usize { self.encoded_bytes }
}

impl ForecastModel {
    pub fn new(probe: LinearProbe, registration: ForecastRegistration) -> Result<Self, Error> {
        if [registration.domain, registration.generation, registration.policy_generation].contains(&0)
            || registration.event_prefix.is_empty()
        { return Err(Error::InvalidInput); }
        if registration.event_prefix.len() > MAX_EVENT_PREFIX_BYTES { return Err(Error::Limit); }
        Ok(Self { probe, registration })
    }
    pub fn profile(&self) -> CaptureProfile { self.probe.identity().profile }
    pub fn dimensions(&self) -> usize { self.probe.identity().dimensions }
    pub fn policy_generation(&self) -> u64 { self.registration.policy_generation }
    pub fn event(&self, payload: &[u8]) -> bool { payload.starts_with(&self.registration.event_prefix) }

    /// Exact-reconstruction baseline, deliberately not a learned predictor or an
    /// inferred probability from a probe margin. Full bytes and source checking
    /// are paid before choosing the registered negative/equality/positive band.
    pub fn predict(&self, source: &SourceFrame) -> Result<Prediction, Error> {
        if source.identity().profile != self.profile() || source.dimensions() != self.dimensions() {
            return Err(Error::Binding);
        }
        let encoded = source.encode_initial(23)?;
        let checked = source.verify_block(&encoded)?;
        let frame = ProgressiveFrame::from_initial(&checked)?;
        let observation = self.probe.evaluate(&frame)?;
        let forecast = match observation.outcome() {
            ProbeOutcome::CertifiedQuiet => self.registration.negative,
            ProbeOutcome::AtThreshold => self.registration.at_threshold,
            ProbeOutcome::CertifiedAlarm => self.registration.positive,
            ProbeOutcome::NeedsRefinement => return Err(Error::Incomplete),
        };
        Ok(Prediction { observation, forecast, domain: self.registration.domain,
            generation: self.registration.generation, policy_generation: self.registration.policy_generation,
            encoded_bytes: encoded.len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{FrameIdentity, HEADER_BYTES};

    fn pair() -> BinaryForecast { BinaryForecast::new(16_384, 49_152).unwrap() }
    fn evidence() -> LikelihoodEvidence { LikelihoodEvidence::new(ErrorBudget::new(1, 4).unwrap()) }
    fn profile() -> CaptureProfile { CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 } }
    fn model() -> ForecastModel {
        ForecastModel::new(LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 0.0).unwrap(), ForecastRegistration {
            domain: 1, generation: 2, policy_generation: 1, event_prefix: b"publish".to_vec(),
            negative: pair(), at_threshold: BinaryForecast::new(32_768, 32_768).unwrap(),
            positive: BinaryForecast::new(49_152, 16_384).unwrap(),
        }).unwrap()
    }

    #[test]
    fn strict_support_and_alpha_are_required() {
        for (p, q) in [(0, 1), (1, 0), (65_536, 1), (1, 65_536), (u32::MAX, 1)] {
            assert_eq!(BinaryForecast::new(p, q), Err(Error::InvalidInput));
        }
        for (n, d) in [(0, 1), (1, 1), (2, 1), (1, 0)] {
            assert_eq!(ErrorBudget::new(n, d), Err(Error::InvalidInput));
        }
    }

    #[test]
    fn factor_is_a_likelihood_ratio_not_negative_log_probability() {
        assert_eq!(pair().factor(true), LikelihoodFactor { numerator: 49_152, denominator: 16_384 });
        assert_eq!(pair().factor(false), LikelihoodFactor { numerator: 16_384, denominator: 49_152 });
        let neutral = BinaryForecast::new(1, 1).unwrap();
        let mut e = evidence();
        for event in [true, false, true] { e.observe(neutral, event).unwrap(); }
        assert_eq!(e.numerator_words(), e.denominator_words());
        assert!(!e.crossed());
    }

    #[test]
    fn crossing_is_latched_after_likelihood_declines() {
        let mut e = evidence();
        e.observe(pair(), true).unwrap();
        assert!(!e.crossed());
        e.observe(pair(), true).unwrap();
        assert_eq!(e.first_crossing(), Some(2));
        e.observe(pair(), false).unwrap();
        assert_eq!(e.first_crossing(), Some(2));
        assert_eq!(e.numerator_words()[0], 3 * e.denominator_words()[0]);
    }

    #[test]
    fn equality_to_the_threshold_crosses() {
        let mut e = LikelihoodEvidence::new(ErrorBudget::new(1, 2).unwrap());
        e.observe(BinaryForecast::new(16_384, 32_768).unwrap(), true).unwrap();
        assert_eq!(e.first_crossing(), Some(1));
    }

    #[test]
    fn maximum_length_extreme_ratios_and_alpha_fit_exactly() {
        for event in [false, true] {
            let mut e = LikelihoodEvidence::new(ErrorBudget::new(u64::MAX - 1, u64::MAX).unwrap());
            let p = BinaryForecast::new(1, 65_535).unwrap();
            for _ in 0..MAX_SAMPLES { e.observe(p, event).unwrap(); }
            assert_eq!(e.samples(), MAX_SAMPLES);
            assert_eq!(e.crossed(), event);
            let before = e.clone();
            assert_eq!(e.observe(p, event), Err(Error::Limit));
            assert_eq!(e, before);
            assert_eq!(e.numerator_words()[128], 0);
            assert_eq!(e.denominator_words()[128], 0);
        }
    }

    #[test]
    fn short_paths_match_an_independent_u128_oracle() {
        for mask in 0_u8..64 {
            let mut e = evidence(); let mut n = 1_u128; let mut d = 1_u128; let mut crossed = None;
            for index in 0..6 {
                let y = mask & (1 << index) != 0;
                let f = pair().factor(y); n *= u128::from(f.numerator); d *= u128::from(f.denominator);
                e.observe(pair(), y).unwrap();
                if n >= d * 4 && crossed.is_none() { crossed = Some(index + 1); }
                assert_eq!(e.first_crossing(), crossed);
                assert_eq!(e.numerator_words()[0], n as u64);
                assert_eq!(e.numerator_words()[1], (n >> 64) as u64);
                assert_eq!(e.denominator_words()[0], d as u64);
                assert_eq!(e.denominator_words()[1], (d >> 64) as u64);
            }
        }
    }

    #[test]
    fn finite_null_tree_respects_its_declared_crossing_budget() {
        let mut crossed_weight = 0_u64;
        for mask in 0_u16..256 {
            let mut e = evidence(); let mut probability_numerator = 1_u64;
            for index in 0..8 {
                let y = mask & (1 << index) != 0;
                e.observe(pair(), y).unwrap();
                if !y { probability_numerator *= 3; }
            }
            if e.crossed() { crossed_weight += probability_numerator; }
        }
        // Independent enumeration under fixed Bernoulli(1/4), NOT a calibration
        // test of any real predictor or a proof for arbitrary supplied scores.
        assert!(crossed_weight > 0);
        assert!(crossed_weight * 4 <= 4_u64.pow(8));
    }

    #[test]
    fn activation_bands_choose_registered_probabilities_before_any_action() {
        let model = model();
        for (value, expected) in [(-1.0, pair()), (0.0, BinaryForecast::new(32_768, 32_768).unwrap()),
            (1.0, BinaryForecast::new(49_152, 16_384).unwrap())]
        {
            let frame = SourceFrame::capture(FrameIdentity { profile: profile(), stream: 1, sequence: 1, position: 0 }, &[value]).unwrap();
            let prediction = model.predict(&frame).unwrap();
            assert_eq!(prediction.forecast(), expected);
            assert_eq!(prediction.observation().mantissa_bits(), 23);
            assert_eq!(prediction.encoded_bytes(), HEADER_BYTES + 4);
            assert_eq!(prediction.domain(), 1);
            assert_eq!(prediction.generation(), 2);
        }
        assert!(model.event(b"publish\0data"));
        assert!(!model.event(b"Publish"));
        assert!(!model.event(b""));
    }

    #[test]
    fn model_identity_shape_and_event_registration_are_checked() {
        let model = model();
        let mut identity = FrameIdentity { profile: profile(), stream: 1, sequence: 1, position: 0 };
        identity.profile.model += 1;
        let foreign = SourceFrame::capture(identity, &[1.0]).unwrap();
        assert_eq!(model.predict(&foreign), Err(Error::Binding));
        identity.profile = profile();
        let wide = SourceFrame::capture(identity, &[1.0, 2.0]).unwrap();
        assert_eq!(model.predict(&wide), Err(Error::Binding));
        let mut invalid = model.registration.clone(); invalid.event_prefix.clear();
        assert!(matches!(ForecastModel::new(model.probe.clone(), invalid), Err(Error::InvalidInput)));
    }
}
