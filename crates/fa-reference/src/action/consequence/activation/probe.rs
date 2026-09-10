//! Exact linear-score intervals over source-checked binary32 observations.
//!
//! Products of two finite binary32 values are integers times 2^-298. Separate
//! positive and negative 576-bit sums retain every product bit, even across
//! catastrophic cancellation. At MAX_VALUES the sums need fewer than 576 bits.
//! This proves only a declared linear threshold decision, not detector accuracy.

use super::{CaptureProfile, FrameIdentity, MAX_VALUES, ProgressiveFrame};
use crate::Error;
use std::cmp::Ordering;

pub const SCORE_WORDS: usize = 9;
pub const SCORE_UNIT_EXPONENT: i32 = -298;

/// Canonical signed magnitude, little-endian words, in units of 2^-298.
/// Zero is never negative. This is an exact margin, not a probability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactScore {
    negative: bool,
    words: [u64; SCORE_WORDS],
}

impl ExactScore {
    pub fn sign(&self) -> Ordering {
        if self.words == [0; SCORE_WORDS] { Ordering::Equal }
        else if self.negative { Ordering::Less } else { Ordering::Greater }
    }
    pub fn magnitude_words(&self) -> &[u64; SCORE_WORDS] { &self.words }
}

impl Ord for ExactScore {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => magnitude_cmp(&self.words, &other.words),
            (true, true) => magnitude_cmp(&other.words, &self.words),
        }
    }
}

impl PartialOrd for ExactScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreInterval {
    pub lower: ExactScore,
    pub upper: ExactScore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    CertifiedAlarm,
    CertifiedQuiet,
    NeedsRefinement,
    /// Strict sign is undecided at equality, even with exact source recovery.
    AtThreshold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeIdentity {
    pub id: u64,
    pub generation: u64,
    pub profile: CaptureProfile,
    pub dimensions: usize,
}

/// Immutable registered coefficients. Registration and host provenance remain
/// trusted inputs; mathematical score fidelity is not empirical qualification.
#[derive(Clone, Debug)]
pub struct LinearProbe {
    identity: ProbeIdentity,
    weights: Vec<u32>,
    bias: u32,
    threshold: u32,
}

/// No permission conversion and no public constructor accepting claimed scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeObservation {
    frame: FrameIdentity,
    probe: ProbeIdentity,
    bits: u8,
    interval: ScoreInterval,
    outcome: ProbeOutcome,
}

impl ProbeObservation {
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn probe(&self) -> ProbeIdentity { self.probe }
    pub fn mantissa_bits(&self) -> u8 { self.bits }
    pub fn interval(&self) -> &ScoreInterval { &self.interval }
    pub fn outcome(&self) -> ProbeOutcome { self.outcome }
}

impl LinearProbe {
    pub fn new(
        id: u64, generation: u64, profile: CaptureProfile,
        weights: &[f32], bias: f32, threshold: f32,
    ) -> Result<Self, Error> {
        if [id, generation, profile.tenant, profile.model, profile.model_generation,
            profile.tap, profile.layout_generation].contains(&0) || weights.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        if weights.len() > MAX_VALUES { return Err(Error::Limit); }
        if weights.iter().any(|v| !v.is_finite()) || !bias.is_finite() || !threshold.is_finite() {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            identity: ProbeIdentity { id, generation, profile, dimensions: weights.len() },
            weights: weights.iter().map(|v| v.to_bits()).collect(),
            bias: bias.to_bits(), threshold: threshold.to_bits(),
        })
    }

    pub fn identity(&self) -> ProbeIdentity { self.identity }

    pub fn evaluate(&self, frame: &ProgressiveFrame) -> Result<ProbeObservation, Error> {
        if frame.identity().profile != self.identity.profile || frame.dimensions() != self.weights.len() {
            return Err(Error::Binding);
        }
        let mut lower = SignedSum::new();
        let mut upper = SignedSum::new();
        for sum in [&mut lower, &mut upper] {
            sum.product(self.bias, 1.0_f32.to_bits())?;
            sum.product(self.threshold ^ 0x8000_0000, 1.0_f32.to_bits())?;
        }
        for (index, weight) in self.weights.iter().copied().enumerate() {
            let [lo, hi] = frame.interval(index)?;
            let (lo, hi) = if weight >> 31 == 0 { (lo, hi) } else { (hi, lo) };
            lower.product(weight, lo.to_bits())?;
            upper.product(weight, hi.to_bits())?;
        }
        let interval = ScoreInterval { lower: lower.finish(), upper: upper.finish() };
        if interval.lower > interval.upper { return Err(Error::Binding); }
        let outcome = if interval.lower.sign() == Ordering::Greater {
            ProbeOutcome::CertifiedAlarm
        } else if interval.upper.sign() == Ordering::Less {
            ProbeOutcome::CertifiedQuiet
        } else if interval.lower.sign() == Ordering::Equal && interval.upper.sign() == Ordering::Equal {
            ProbeOutcome::AtThreshold
        } else {
            ProbeOutcome::NeedsRefinement
        };
        Ok(ProbeObservation { frame: frame.identity(), probe: self.identity,
            bits: frame.mantissa_bits(), interval, outcome })
    }
}

#[derive(Clone, Debug)]
struct SignedSum {
    positive: [u64; SCORE_WORDS],
    negative: [u64; SCORE_WORDS],
}

impl SignedSum {
    fn new() -> Self { Self { positive: [0; SCORE_WORDS], negative: [0; SCORE_WORDS] } }

    fn product(&mut self, a: u32, b: u32) -> Result<(), Error> {
        let (ma, ea) = decompose(a);
        let (mb, eb) = decompose(b);
        let magnitude = ma * mb;
        let words = if (a ^ b) >> 31 == 0 { &mut self.positive } else { &mut self.negative };
        add_shifted(words, magnitude, ea + eb)
    }

    fn finish(self) -> ExactScore {
        let negative = magnitude_cmp(&self.positive, &self.negative) == Ordering::Less;
        let (big, small) = if negative { (self.negative, self.positive) } else { (self.positive, self.negative) };
        let mut words = [0; SCORE_WORDS];
        let mut borrow = false;
        for index in 0..SCORE_WORDS {
            let (value, first) = big[index].overflowing_sub(small[index]);
            let (value, second) = value.overflowing_sub(u64::from(borrow));
            words[index] = value;
            borrow = first || second;
        }
        ExactScore { negative, words }
    }
}

// value = sign * mantissa * 2^(shift - 149); finite inputs only.
fn decompose(bits: u32) -> (u64, u32) {
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x007f_ffff;
    if exponent == 0 { (u64::from(fraction), 0) }
    else { (u64::from(fraction | 0x0080_0000), exponent - 1) }
}

fn add_shifted(words: &mut [u64; SCORE_WORDS], value: u64, shift: u32) -> Result<(), Error> {
    if value == 0 { return Ok(()); }
    let index = (shift / 64) as usize;
    let offset = shift % 64;
    add_word(words, index, value << offset)?;
    if offset != 0 { add_word(words, index + 1, value >> (64 - offset))?; }
    Ok(())
}

fn add_word(words: &mut [u64; SCORE_WORDS], mut index: usize, mut value: u64) -> Result<(), Error> {
    while value != 0 {
        let word = words.get_mut(index).ok_or(Error::Overflow)?;
        let (next, carry) = word.overflowing_add(value);
        *word = next;
        value = u64::from(carry);
        index += 1;
    }
    Ok(())
}

fn magnitude_cmp(a: &[u64; SCORE_WORDS], b: &[u64; SCORE_WORDS]) -> Ordering {
    a.iter().rev().cmp(b.iter().rev())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::SourceFrame;

    fn identity() -> FrameIdentity {
        FrameIdentity { profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3,
            tap: 4, layout_generation: 5 }, stream: 6, sequence: 7, position: 0 }
    }
    fn frame(source: &SourceFrame, bits: u8) -> ProgressiveFrame {
        ProgressiveFrame::from_initial(&source.verify_block(&source.encode_initial(bits).unwrap()).unwrap()).unwrap()
    }
    fn probe(weights: &[f32], bias: f32, threshold: f32) -> LinearProbe {
        LinearProbe::new(1, 1, identity().profile, weights, bias, threshold).unwrap()
    }

    #[test]
    fn coarse_strict_signs_and_exact_threshold_have_distinct_results() {
        let source = SourceFrame::capture(identity(), &[1.0]).unwrap();
        assert_eq!(probe(&[1.0], 0.0, 0.0).evaluate(&frame(&source, 0)).unwrap().outcome(), ProbeOutcome::CertifiedAlarm);
        assert_eq!(probe(&[-1.0], 0.0, 0.0).evaluate(&frame(&source, 0)).unwrap().outcome(), ProbeOutcome::CertifiedQuiet);
        assert_eq!(probe(&[1.0], 0.0, 1.0).evaluate(&frame(&source, 23)).unwrap().outcome(), ProbeOutcome::AtThreshold);
    }

    #[test]
    fn smallest_product_survives_huge_opposite_products_in_every_order() {
        let tiny = f32::from_bits(1);
        let terms = [(f32::MAX, f32::MAX), (tiny, tiny), (f32::MAX, -f32::MAX)];
        for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            let source = SourceFrame::capture(identity(), &order.map(|i| terms[i].0)).unwrap();
            let result = probe(&order.map(|i| terms[i].1), 0.0, 0.0).evaluate(&frame(&source, 23)).unwrap();
            let mut expected = [0; SCORE_WORDS]; expected[0] = 1;
            assert_eq!(result.interval().lower.magnitude_words(), &expected);
            assert_eq!(result.interval().lower, result.interval().upper);
            assert_eq!(result.outcome(), ProbeOutcome::CertifiedAlarm);
        }
    }

    #[test]
    fn signed_bias_and_threshold_have_exact_manual_dyadic_result() {
        let source = SourceFrame::capture(identity(), &[1.5, -2.0]).unwrap();
        let result = probe(&[2.0, 3.0], 1.0, -1.0).evaluate(&frame(&source, 23)).unwrap();
        let mut expected = [0; SCORE_WORDS]; expected[4] = 1_u64 << 42;
        assert_eq!(result.interval().lower.magnitude_words(), &expected);
        assert_eq!(result.interval().lower.sign(), Ordering::Less);
        assert_eq!(result.interval().upper, result.interval().lower);
    }

    #[test]
    fn omitted_rare_residual_requires_refinement_instead_of_quiet() {
        let source = SourceFrame::capture(identity(), &[f32::from_bits(1)]).unwrap();
        let detector = probe(&[1.0], 0.0, 0.0);
        for bits in 0..23 {
            assert_eq!(detector.evaluate(&frame(&source, bits)).unwrap().outcome(), ProbeOutcome::NeedsRefinement);
        }
        assert_eq!(detector.evaluate(&frame(&source, 23)).unwrap().outcome(), ProbeOutcome::CertifiedAlarm);
    }

    #[test]
    fn all_prefix_intervals_contain_exact_score_with_mixed_sign_weights() {
        let source = SourceFrame::capture(identity(), &[1.2345, -4.5678, f32::MIN_POSITIVE, -0.0]).unwrap();
        let detector = probe(&[-5.0, 3.0, -7.25, 1.0], 8.0, 0.25);
        let exact = detector.evaluate(&frame(&source, 23)).unwrap().interval().lower.clone();
        let mut prior: Option<ScoreInterval> = None;
        for bits in 0..=23 {
            let observation = detector.evaluate(&frame(&source, bits)).unwrap();
            let interval = observation.interval();
            assert!(interval.lower <= exact && exact <= interval.upper);
            if let Some(before) = prior {
                assert!(before.lower <= interval.lower && interval.upper <= before.upper);
            }
            prior = Some(interval.clone());
        }
    }

    #[test]
    fn full_dimension_extremes_fit_and_invalid_coefficients_refuse() {
        let source = SourceFrame::capture(identity(), &vec![f32::MAX; MAX_VALUES]).unwrap();
        let result = probe(&vec![f32::MAX; MAX_VALUES], f32::MAX, -f32::MAX)
            .evaluate(&frame(&source, 23)).unwrap();
        assert_eq!(result.outcome(), ProbeOutcome::CertifiedAlarm);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(LinearProbe::new(1, 1, identity().profile, &[bad], 0.0, 0.0).is_err());
            assert!(LinearProbe::new(1, 1, identity().profile, &[1.0], bad, 0.0).is_err());
        }
        assert_eq!(probe(&[1.0], 0.0, 0.0).evaluate(&frame(&source, 0)), Err(Error::Binding));
        let mut other = identity(); other.profile.model_generation += 1;
        let other = SourceFrame::capture(other, &[1.0]).unwrap();
        assert_eq!(probe(&[1.0], 0.0, 0.0).evaluate(&frame(&other, 0)), Err(Error::Binding));
    }

    #[test]
    fn integer_carries_borrows_and_zero_sign_are_canonical() {
        let mut words = [u64::MAX; SCORE_WORDS];
        assert_eq!(add_word(&mut words, 0, 1), Err(Error::Overflow));
        let mut positive = [0; SCORE_WORDS]; positive[2] = 1;
        let mut negative = [0; SCORE_WORDS]; negative[0] = 1;
        let value = SignedSum { positive, negative }.finish();
        assert_eq!(value.words[0], u64::MAX);
        assert_eq!(value.words[1], u64::MAX);
        assert_eq!(value.words[2], 0);
        let zero = SignedSum { positive, negative: positive }.finish();
        assert_eq!(zero.sign(), Ordering::Equal);
        assert!(!zero.negative);
    }
}
