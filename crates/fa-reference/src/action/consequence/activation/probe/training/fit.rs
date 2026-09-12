//! Deterministic full-batch class-balanced logistic fitting on the training split.
//! Optimization uses rounded binary64; emitted binary32 coefficients are evaluated
//! by the original exact linear-probe implementation. No convergence is assumed.

use super::{CaseLabel, ClassCounts, DataSplit, SealedCorpus};
use super::super::LinearProbe;
use crate::Error;
use std::fmt;
use std::rc::Rc;

pub const MAX_TRAINING_VISITS: u64 = 4_294_967_296;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitPolicy {
    id: u64,
    generation: u64,
    epochs: usize,
    rate_bits: u64,
    l2_bits: u64,
    floor_bits: u64,
}
impl FitPolicy {
    pub fn new(id: u64, generation: u64, epochs: usize, learning_rate: f64,
        l2: f64, scale_floor: f64) -> Result<Self, Error>
    {
        if id == 0 || generation == 0 || epochs == 0 || !learning_rate.is_finite()
            || learning_rate <= 0.0 || learning_rate > 1.0 || !l2.is_finite() || !(0.0..=1000.0).contains(&l2)
            || !scale_floor.is_finite() || !(1e-12..=1e12).contains(&scale_floor)
        { return Err(Error::InvalidInput); }
        if epochs > 4096 { return Err(Error::Limit); }
        Ok(Self { id, generation, epochs, rate_bits: learning_rate.to_bits(),
            l2_bits: l2.to_bits(), floor_bits: scale_floor.to_bits() })
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn epochs(&self) -> usize { self.epochs }
    pub fn learning_rate(&self) -> f64 { f64::from_bits(self.rate_bits) }
    pub fn l2(&self) -> f64 { f64::from_bits(self.l2_bits) }
    pub fn scale_floor(&self) -> f64 { f64::from_bits(self.floor_bits) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainingBudget {
    /// Complete source-coordinate passes, including fitting and normalization.
    /// Does not count allocator overhead, sigmoid calls or parameter-only loops.
    pub source_coordinate_visits: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainingWork {
    pub classes: ClassCounts,
    pub epochs: usize,
    pub source_coordinate_visits: u64,
    pub parameter_updates: u64,
    pub sigmoid_evaluations: u64,
}

struct FittedData {
    corpus: SealedCorpus,
    policy: FitPolicy,
    weights: Vec<f32>,
    bias: f32,
    mean: Vec<f64>,
    scale: Vec<f64>,
    work: TrainingWork,
}

/// Learned coefficients, not a calibrated probability, deployed monitor or
/// safety certificate. All retained sources are immutable and origin-partitioned.
#[derive(Clone)]
pub struct FittedProbe { data: Rc<FittedData> }
impl fmt::Debug for FittedProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FittedProbe").field("policy", self.policy()).field("work", &self.work())
            .field("corpus", self.corpus()).finish_non_exhaustive()
    }
}
impl FittedProbe {
    pub fn corpus(&self) -> &SealedCorpus { &self.data.corpus }
    pub fn policy(&self) -> &FitPolicy { &self.data.policy }
    pub fn weights(&self) -> &[f32] { &self.data.weights }
    pub fn bias(&self) -> f32 { self.data.bias }
    pub fn training_mean(&self) -> &[f64] { &self.data.mean }
    pub fn training_scale(&self) -> &[f64] { &self.data.scale }
    pub fn work(&self) -> TrainingWork { self.data.work }

    /// An explicit candidate with an operator-selected score threshold. This is
    /// not empirical qualification: the original monitor treats coefficients as
    /// registered data and still requires its existing profile and evidence gates.
    pub fn probe(&self, threshold: f32) -> Result<LinearProbe, Error> {
        LinearProbe::new(self.policy().id(), self.policy().generation(), self.corpus().profile(),
            self.weights(), self.bias(), threshold)
    }
}

impl SealedCorpus {
    pub fn estimate_fit(&self, policy: &FitPolicy) -> Result<TrainingWork, Error> {
        let classes = self.counts(DataSplit::Training);
        let n = classes.total() as u64;
        let d = self.dimensions() as u64;
        let epochs = policy.epochs() as u64;
        Ok(TrainingWork { classes, epochs: policy.epochs(),
            source_coordinate_visits: n.checked_mul(d).and_then(|v| v.checked_mul(1 + 2 * epochs)).ok_or(Error::Overflow)?,
            parameter_updates: d.checked_mul(epochs).ok_or(Error::Overflow)?,
            sigmoid_evaluations: n.checked_mul(epochs).ok_or(Error::Overflow)?,
        })
    }

    /// Fixed epochs, zero initialization, deterministic origin/coordinate order.
    /// Each class contributes one half of the data objective. Only weights are
    /// L2-regularized. No calibration or evaluation value is inspected by fitting.
    pub fn fit(&self, policy: FitPolicy, budget: TrainingBudget) -> Result<FittedProbe, Error> {
        let work = self.estimate_fit(&policy)?;
        if budget.source_coordinate_visits > MAX_TRAINING_VISITS
            || work.source_coordinate_visits > budget.source_coordinate_visits { return Err(Error::Limit); }
        let d = self.dimensions();
        let mut mean = zeroes(d)?;
        let mut squared = zeroes(d)?;
        for (index, (_, row)) in self.rows(DataSplit::Training).enumerate() {
            let count = (index + 1) as f64;
            for (j, word) in row.source.words.iter().copied().enumerate() {
                let x = f64::from(f32::from_bits(word));
                let delta = x - mean[j];
                mean[j] = finite(mean[j] + delta / count)?;
                squared[j] = finite(squared[j] + delta * (x - mean[j]))?;
            }
        }
        let scale = squared.into_iter().map(|sum| {
            finite((sum.max(0.0) / work.classes.total() as f64).sqrt().max(policy.scale_floor()))
        }).collect::<Result<Vec<_>, _>>()?;
        let mut weights = zeroes(d)?;
        let mut gradient = zeroes(d)?;
        let mut bias = 0.0;
        for _ in 0..policy.epochs() {
            gradient.fill(0.0);
            let mut bias_gradient = 0.0;
            for (_, row) in self.rows(DataSplit::Training) {
                let mut score = bias;
                for (j, word) in row.source.words.iter().copied().enumerate() {
                    let z = (f64::from(f32::from_bits(word)) - mean[j]) / scale[j];
                    score = finite(score + weights[j] * z)?;
                }
                let probability = if score >= 0.0 { 1.0 / (1.0 + (-score).exp()) }
                    else { let exp = score.exp(); exp / (1.0 + exp) };
                let (target, cases) = match row.label {
                    CaseLabel::Benign => (0.0, work.classes.benign),
                    CaseLabel::Violation => (1.0, work.classes.violation),
                };
                let error = finite((probability - target) * (0.5 / cases as f64))?;
                bias_gradient = finite(bias_gradient + error)?;
                for (j, word) in row.source.words.iter().copied().enumerate() {
                    let z = (f64::from(f32::from_bits(word)) - mean[j]) / scale[j];
                    gradient[j] = finite(gradient[j] + error * z)?;
                }
            }
            for j in 0..d {
                weights[j] = finite(weights[j] - policy.learning_rate() * (gradient[j] + policy.l2() * weights[j]))?;
            }
            bias = finite(bias - policy.learning_rate() * bias_gradient)?;
        }
        // Freeze raw-space binary32 coefficients. Recenter the bias using those
        // rounded coefficients; later calibration evaluates THIS exact emitted
        // probe, not the unrounded optimizer or its standardized coordinates.
        let weights = weights.iter().zip(&scale).map(|(w, s)| narrow(w / s)).collect::<Result<Vec<_>, _>>()?;
        for (weight, center) in weights.iter().zip(&mean) { bias = finite(bias - f64::from(*weight) * center)?; }
        let bias = narrow(bias)?;
        let fitted = FittedProbe { data: Rc::new(FittedData { corpus: self.clone(), policy,
            weights, bias, mean, scale, work }) };
        fitted.probe(0.0)?;
        Ok(fitted)
    }
}
fn finite(value: f64) -> Result<f64, Error> { if value.is_finite() { Ok(value) } else { Err(Error::Overflow) } }
fn narrow(value: f64) -> Result<f32, Error> {
    let value = finite(value)? as f32;
    if value.is_finite() { Ok(value) } else { Err(Error::Overflow) }
}
fn zeroes(count: usize) -> Result<Vec<f64>, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    values.resize(count, 0.0);
    Ok(values)
}
