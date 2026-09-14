//! Fixed-budget centered covariance and cyclic symmetric Jacobi fitting.
//! This is the explicitly scoped PCA/linear-autoencoder reference baseline,
//! not a nonlinear sidecar, native BLAS kernel or convergence certificate.
use super::{CodecData, GroupBasis, GroupKey, LearnedKvCodec, LearnedKvPolicy,
    ModelKvDescriptor, ModelKvImage, ModelKvProfile, KvSide, finite, group_words, rounded,
    MAX_LEARNED_CHANNELS, MAX_LEARNED_PARAMETERS, MAX_MODEL_KV_VALUES};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_FIT_WORK: u64 = 1_073_741_824;
pub const MAX_TRAINING_SOURCES: usize = 64;
pub const MAX_TRAINING_ROWS: usize = 16_384;
pub const MAX_LEARNED_GROUPS: usize = 4096;
pub const MAX_FIT_SCRATCH_VALUES: usize = 2 * MAX_LEARNED_CHANNELS * MAX_LEARNED_CHANNELS + 4 * MAX_LEARNED_CHANNELS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FitBudget {
    pub source_values: usize,
    pub parameter_values: usize,
    /// Peak logical numeric/index slots in the per-group fitting workspace;
    /// excludes caller-held inputs, codebooks, source metadata and allocator cost.
    pub scratch_values: usize,
    pub work_units: u64,
}
impl Default for FitBudget {
    fn default() -> Self {
        Self { source_values: MAX_MODEL_KV_VALUES, parameter_values: MAX_LEARNED_PARAMETERS,
            scratch_values: MAX_FIT_SCRATCH_VALUES, work_units: MAX_FIT_WORK }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingSource { pub descriptor: ModelKvDescriptor }
#[derive(Clone, Debug, PartialEq)]
pub struct GroupFitReport {
    pub rows: usize,
    pub channels: usize,
    pub rotations: usize,
    /// Population covariance statistics in binary64. These are descriptive
    /// residuals, not source-checked reconstruction or detector certificates.
    pub covariance_trace: f64,
    pub remaining_off_diagonal_squared: f64,
    pub selected_diagonal: Vec<f64>,
    pub emitted_orthogonality_max_error: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FitReport {
    pub sources: BTreeMap<u64, TrainingSource>,
    pub rows_per_group: usize,
    pub training_values: usize,
    pub source_coordinate_visits: usize,
    pub parameter_values: usize,
    pub scratch_values_reserved: usize,
    pub work_units_reserved: u64,
    pub groups: BTreeMap<GroupKey, GroupFitReport>,
}

struct Admission {
    profile: ModelKvProfile,
    sources: BTreeMap<u64, TrainingSource>,
    groups: Vec<(GroupKey, usize)>,
    rows: usize,
    values: usize,
    parameters: usize,
    scratch: usize,
    work: u64,
}
fn admit(policy: LearnedKvPolicy, training: &BTreeMap<u64, ModelKvImage>, budget: FitBudget) -> Result<Admission, Error> {
    if training.is_empty() || training.contains_key(&0) { return Err(Error::InvalidInput); }
    if training.len() > MAX_TRAINING_SOURCES || budget.source_values > MAX_MODEL_KV_VALUES
        || budget.parameter_values > MAX_LEARNED_PARAMETERS || budget.scratch_values > MAX_FIT_SCRATCH_VALUES
        || budget.work_units > MAX_FIT_WORK { return Err(Error::Limit); }
    let first = training.values().next().expect("nonempty corpus");
    let profile = first.profile();
    let mut rows = 0_usize;
    let mut seen = BTreeSet::new();
    let mut sources = BTreeMap::new();
    for (origin, image) in training {
        if image.profile() != profile { return Err(Error::Binding); }
        if image.is_empty() { return Err(Error::InvalidInput); }
        rows = rows.checked_add(image.len()).ok_or(Error::Overflow)?;
        let descriptor = image.descriptor(); descriptor.image_len()?;
        let cut = descriptor.layers().values().next().ok_or(Error::Incomplete)?;
        // Disjoint windows in the same stream are conservatively one origin,
        // not extra independent training cases. Caller IDs cannot defeat this.
        if !seen.insert((cut.stream, cut.source_batch)) { return Err(Error::Duplicate); }
        sources.insert(*origin, TrainingSource { descriptor });
    }
    if rows < 2 { return Err(Error::Incomplete); }
    if rows > MAX_TRAINING_ROWS { return Err(Error::Limit); }
    let values = rows.checked_mul(profile.values_per_token()).ok_or(Error::Overflow)?;
    if values > budget.source_values { return Err(Error::Limit); }
    let mut group_count = 0_usize;
    let mut parameters = 0_usize;
    let mut scratch = 0_usize;
    let mut work = 0_u64;
    for layer in profile.layers().values() {
        for contract in [layer.keys(), layer.values()] {
            let d = contract.channels();
            if d > MAX_LEARNED_CHANNELS { return Err(Error::Limit); }
            if policy.rank > d { return Err(Error::Binding); }
            let heads = contract.heads();
            group_count = group_count.checked_add(heads).ok_or(Error::Overflow)?;
            parameters = parameters.checked_add(heads.checked_mul(d * (policy.rank + 1)).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            scratch = scratch.max(2 * d * d + 4 * d);
            // Conservative fixed loop-cost model. Skipping an already diagonal
            // pair does not increase the allowance available to another fit.
            let data = (rows as u64).checked_mul((d * d + 4 * d) as u64).ok_or(Error::Overflow)?;
            let rotations = (policy.sweeps as u64).checked_mul((d * (d - 1) / 2) as u64)
                .and_then(|n| n.checked_mul((8 * d + 32) as u64)).ok_or(Error::Overflow)?;
            let export = (2 * policy.rank * policy.rank * d + d * d + 4 * d) as u64;
            let group = data.checked_add(rotations).and_then(|n| n.checked_add(export)).ok_or(Error::Overflow)?;
            work = work.checked_add(group.checked_mul(heads as u64).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        }
    }
    if group_count > MAX_LEARNED_GROUPS || parameters > budget.parameter_values
        || scratch > budget.scratch_values || work > budget.work_units { return Err(Error::Limit); }
    let mut groups = Vec::new(); groups.try_reserve_exact(group_count).map_err(|_| Error::Limit)?;
    for (layer_id, layer) in profile.layers() {
        for (side, contract) in [(KvSide::Key, layer.keys()), (KvSide::Value, layer.values())] {
            for head in 0..contract.heads() { groups.push((GroupKey { layer: *layer_id, side, head }, contract.channels())); }
        }
    }
    Ok(Admission { profile: profile.clone(), sources, groups, rows, values, parameters, scratch, work })
}

pub(super) fn fit(policy: LearnedKvPolicy, training: &BTreeMap<u64, ModelKvImage>, budget: FitBudget)
    -> Result<LearnedKvCodec, Error>
{
    // Every source/profile/count and the entire run's budget are checked before
    // the first scalar is read. A failure cannot return a partial codebook.
    let admission = admit(policy, training, budget)?;
    let mut groups = BTreeMap::new();
    let mut reports = BTreeMap::new();
    for (key, channels) in &admission.groups {
        let (basis, report) = fit_group(*key, *channels, policy, training, &admission.sources, admission.rows)?;
        groups.insert(*key, basis); reports.insert(*key, report);
    }
    let report = FitReport {
        sources: admission.sources, rows_per_group: admission.rows, training_values: admission.values,
        source_coordinate_visits: admission.values * 2, parameter_values: admission.parameters,
        scratch_values_reserved: admission.scratch, work_units_reserved: admission.work, groups: reports,
    };
    Ok(LearnedKvCodec { data: Rc::new(CodecData { policy, profile: admission.profile, groups, report }) })
}
fn zeroes(length: usize) -> Result<Vec<f64>, Error> {
    let mut out = Vec::new(); out.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    out.resize(length, 0.0); Ok(out)
}
fn fit_group(key: GroupKey, d: usize, policy: LearnedKvPolicy, training: &BTreeMap<u64, ModelKvImage>,
    sources: &BTreeMap<u64, TrainingSource>, total_rows: usize) -> Result<(GroupBasis, GroupFitReport), Error>
{
    let mut mean = zeroes(d)?;
    let mut covariance = zeroes(d * d)?;
    let mut vectors = zeroes(d * d)?;
    let mut delta = zeroes(d)?;
    for j in 0..d { vectors[j * d + j] = 1.0; }
    let mut rows = 0_usize;
    for (origin, image) in training {
        let source = &sources[origin].descriptor.layers()[&key.layer];
        for offset in 0..source.token_count {
            rows += 1;
            let words = group_words(image, key, source.first_position + offset as u64, d)?;
            for j in 0..d { mean[j] += (finite(words[j])? - mean[j]) / rows as f64; }
        }
    }
    if rows != total_rows { return Err(Error::Binding); }
    for (origin, image) in training {
        let source = &sources[origin].descriptor.layers()[&key.layer];
        for offset in 0..source.token_count {
            let words = group_words(image, key, source.first_position + offset as u64, d)?;
            for j in 0..d { delta[j] = finite(words[j])? - mean[j]; }
            for j in 0..d {
                for k in j..d { covariance[j * d + k] += delta[j] * delta[k]; }
            }
        }
    }
    for j in 0..d {
        for k in j..d {
            let value = covariance[j * d + k] / rows as f64;
            if !value.is_finite() { return Err(Error::Overflow); }
            covariance[j * d + k] = value; covariance[k * d + j] = value;
        }
    }
    let covariance_trace = (0..d).map(|j| covariance[j * d + j]).sum();
    let mut rotations = 0_usize;
    for _ in 0..policy.sweeps {
        for p in 0..d {
            for q in p + 1..d {
                let off = covariance[p * d + q];
                if off == 0.0 { continue; }
                let a = covariance[p * d + p]; let b = covariance[q * d + q];
                // The unscaled ratio (b-a)/(2*off) can overflow near convergence.
                // This equivalent half-difference/hypot form avoids that ratio.
                let half_difference = (b - a) * 0.5;
                let t = if half_difference == 0.0 { 1.0 } else {
                    off / (half_difference + half_difference.signum() * half_difference.hypot(off))
                };
                let c = 1.0 / (1.0 + t * t).sqrt(); let s = t * c;
                covariance[p * d + p] = a - t * off;
                covariance[q * d + q] = b + t * off;
                covariance[p * d + q] = 0.0; covariance[q * d + p] = 0.0;
                for k in 0..d {
                    if k != p && k != q {
                        let x = covariance[k * d + p]; let y = covariance[k * d + q];
                        let left = c * x - s * y; let right = s * x + c * y;
                        covariance[k * d + p] = left; covariance[p * d + k] = left;
                        covariance[k * d + q] = right; covariance[q * d + k] = right;
                    }
                    let x = vectors[k * d + p]; let y = vectors[k * d + q];
                    vectors[k * d + p] = c * x - s * y;
                    vectors[k * d + q] = s * x + c * y;
                }
                rotations += 1;
            }
        }
    }
    if covariance.iter().chain(&vectors).any(|v| !v.is_finite()) { return Err(Error::Overflow); }
    let mut order = Vec::new(); order.try_reserve_exact(d).map_err(|_| Error::Limit)?;
    order.extend(0..d);
    order.sort_by(|a, b| covariance[*b * d + *b].total_cmp(&covariance[*a * d + *a]).then(a.cmp(b)));
    let mut emitted_mean = Vec::new(); emitted_mean.try_reserve_exact(d).map_err(|_| Error::Limit)?;
    for value in mean { emitted_mean.push(rounded(value)?); }
    let mut axes = Vec::new(); axes.try_reserve_exact(d * policy.rank).map_err(|_| Error::Limit)?;
    let mut selected_diagonal = Vec::new(); selected_diagonal.try_reserve_exact(policy.rank).map_err(|_| Error::Limit)?;
    for column in order.into_iter().take(policy.rank) {
        let mut dominant = 0;
        for j in 1..d {
            if vectors[j * d + column].abs() > vectors[dominant * d + column].abs() { dominant = j; }
        }
        let sign = if vectors[dominant * d + column] < 0.0 { -1.0 } else { 1.0 };
        for j in 0..d { axes.push(rounded(sign * vectors[j * d + column])?); }
        selected_diagonal.push(covariance[column * d + column]);
    }
    let mut orthogonality = 0.0_f64;
    for i in 0..policy.rank {
        for j in 0..policy.rank {
            let dot: f64 = (0..d).map(|k| f64::from(axes[i * d + k]) * f64::from(axes[j * d + k])).sum();
            orthogonality = orthogonality.max((dot - if i == j { 1.0 } else { 0.0 }).abs());
        }
    }
    let mut remaining = 0.0;
    for i in 0..d {
        for j in 0..d { if i != j { remaining += covariance[i * d + j] * covariance[i * d + j]; } }
    }
    Ok((GroupBasis { mean: emitted_mean, axes }, GroupFitReport { rows, channels: d, rotations,
        covariance_trace, remaining_off_diagonal_squared: remaining, selected_diagonal,
        emitted_orthogonality_max_error: orthogonality }))
}
