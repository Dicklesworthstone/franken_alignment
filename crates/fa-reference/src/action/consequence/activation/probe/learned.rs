//! Exact registered linear-probe decisions over a SOURCE-CHECKED learned cache.
//! The codec's MSE report is not accepted as a bound. Coarse intervals enclose
//! original binary32 values; selective XOR refinement recovers exact words.
//! All scoring reuses the original 576-bit integer accumulator, including tiny
//! terms under catastrophic cancellation. A certified sign is not a permission
//! and says nothing about the probe's empirical ability to identify harm.
mod evidence;
pub use evidence::{CheckedKvBudget, CheckedKvReport, CheckedKvResidual, CheckedLearnedKv,
    KvGroup, KvRefinementBudget, KvRefinementReceipt, KvRow, LearnedKvView, ResidualRetention,
    CHECKED_GROUP_BYTES, CHECKED_HEADER_BYTES, MAX_CHECKED_KV_BYTES, MAX_CHECKED_KV_GROUPS,
    MAX_CHECKED_KV_PRODUCTS, RESIDUAL_HEADER_BYTES};

use super::{ExactScore, LinearProbe, ProbeIdentity, ProbeOutcome, ScoreInterval, SignedSum};
use crate::Error;
use crate::action::consequence::activation::FrameIdentity;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LearnedProbeWork {
    /// Full registered coefficient vector, including zero-weight coordinates.
    pub coordinates: usize,
    /// Products in resolving nonzero-weight coarse coordinates. Exact promoted
    /// groups require no learned reconstruction. Integer-score costs are separate.
    pub reconstruction_products: u64,
}
/// This retains the actual source-checked context and the immutable refinement
/// snapshot used by the score, not merely reusable numeric IDs or an MSE claim.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::probe::learned::LearnedProbeObservation};
/// fn allow(observation: LearnedProbeObservation) -> Permit { observation }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedProbeObservation {
    view: LearnedKvView,
    row: KvRow,
    frame: FrameIdentity,
    probe: ProbeIdentity,
    interval: ScoreInterval,
    outcome: ProbeOutcome,
    work: LearnedProbeWork,
}
impl LearnedProbeObservation {
    pub fn view(&self) -> &LearnedKvView { &self.view }
    pub fn row(&self) -> KvRow { self.row }
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn probe(&self) -> ProbeIdentity { self.probe }
    pub fn interval(&self) -> &ScoreInterval { &self.interval }
    pub fn outcome(&self) -> ProbeOutcome { self.outcome }
    pub fn work(&self) -> LearnedProbeWork { self.work }
    pub fn exact_score(&self) -> Option<&ExactScore> {
        (self.interval.lower == self.interval.upper).then_some(&self.interval.lower)
    }
}
impl LinearProbe {
    pub fn learned_work(&self, view: &LearnedKvView, row: KvRow) -> Result<LearnedProbeWork, Error> {
        let (_, channels) = self.check_learned(view, row)?;
        let mut unresolved = 0_u64;
        for (index, weight) in self.weights.iter().copied().enumerate() {
            if weight & 0x7fff_ffff == 0 { continue; }
            if !view.is_refined(KvGroup { row, head: index / channels })? { unresolved += 1; }
        }
        Ok(LearnedProbeWork { coordinates: self.weights.len(), reconstruction_products:
            unresolved.checked_mul(view.source().image().codec().policy().rank() as u64).ok_or(Error::Overflow)? })
    }

    pub fn evaluate_learned(&self, view: &LearnedKvView, row: KvRow) -> Result<LearnedProbeObservation, Error> {
        let (frame, channels) = self.check_learned(view, row)?;
        let mut lower = SignedSum::new(); let mut upper = SignedSum::new();
        for sum in [&mut lower, &mut upper] {
            sum.product(self.bias, 1.0_f32.to_bits())?;
            sum.product(self.threshold ^ 0x8000_0000, 1.0_f32.to_bits())?;
        }
        let mut products = 0_u64;
        for (index, weight) in self.weights.iter().copied().enumerate() {
            if weight & 0x7fff_ffff == 0 { continue; }
            let group = KvGroup { row, head: index / channels };
            let [lo, hi] = view.interval(group, index % channels)?;
            let (lo, hi) = if weight >> 31 == 0 { (lo, hi) } else { (hi, lo) };
            lower.product(weight, lo.to_bits())?; upper.product(weight, hi.to_bits())?;
            if !view.is_refined(group)? {
                products = products.checked_add(view.source().image().codec().policy().rank() as u64).ok_or(Error::Overflow)?;
            }
        }
        let interval = ScoreInterval { lower: lower.finish(), upper: upper.finish() };
        if interval.lower > interval.upper { return Err(Error::Binding); }
        let outcome = if interval.lower.sign() == Ordering::Greater { ProbeOutcome::CertifiedAlarm }
            else if interval.upper.sign() == Ordering::Less { ProbeOutcome::CertifiedQuiet }
            else if interval.lower.sign() == Ordering::Equal && interval.upper.sign() == Ordering::Equal { ProbeOutcome::AtThreshold }
            else { ProbeOutcome::NeedsRefinement };
        Ok(LearnedProbeObservation { view: view.clone(), row, frame, probe: self.identity, interval, outcome,
            work: LearnedProbeWork { coordinates: self.weights.len(), reconstruction_products: products } })
    }

    fn check_learned(&self, view: &LearnedKvView, row: KvRow) -> Result<(FrameIdentity, usize), Error> {
        let (frame, heads, channels) = view.source().row_shape(row)?;
        if frame.profile != self.identity.profile || heads.checked_mul(channels).ok_or(Error::Overflow)? != self.weights.len() {
            return Err(Error::Binding);
        }
        Ok((frame, channels))
    }
}
