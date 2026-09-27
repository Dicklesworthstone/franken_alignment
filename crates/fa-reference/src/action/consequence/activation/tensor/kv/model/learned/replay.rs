//! Input-derived replay of an actual fitted codebook, never imported coefficients.
//! Checkpoints deliberately retain training inputs and repeat fitting work.
pub mod archive;
use super::{FitBudget, LearnedKvCodec, LearnedKvPolicy, ModelKvImage, ModelKvProfile};
use super::fit::MAX_TRAINING_SOURCES;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// Complete logical source images, including descriptors; not allocator usage.
pub const MAX_FIT_CHECKPOINT_INPUT_BYTES: usize = 96 * 1024 * 1024;
/// Exact comparison material, not a fitted model or evidence imported as truth.
pub const MAX_FIT_CHECKPOINT_WITNESS_BYTES: usize = 16 * 1024 * 1024;

struct FitCheckpointData {
    policy: LearnedKvPolicy,
    budget: FitBudget,
    training: BTreeMap<u64, ModelKvImage>,
    input_bytes: usize,
    witness: Vec<u8>,
}

/// Original fit inputs and byte-exact comparison material. No source is read by
/// inspection, and a checkpoint cannot substitute for a usable fitted codec.
/// Cloning shares immutable inputs; it neither reruns fitting nor creates rights.
/// Training images are retained intentionally, unlike the ordinary fitted codec.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::model::learned::{
///     LearnedKvCodec, replay::LearnedKvFitCheckpoint};
/// fn bypass(saved: LearnedKvFitCheckpoint) -> LearnedKvCodec { saved }
/// ```
#[derive(Clone)]
pub struct LearnedKvFitCheckpoint { data: Rc<FitCheckpointData> }
impl fmt::Debug for LearnedKvFitCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedKvFitCheckpoint").field("policy", &self.policy())
            .field("training_sources", &self.data.training.len())
            .field("input_bytes", &self.input_bytes())
            .field("comparison_bytes", &self.comparison_bytes()).finish_non_exhaustive()
    }
}

impl LearnedKvCodec {
    /// Fit through the ORIGINAL bounded implementation and retain its complete
    /// immutable input corpus for verified reconstruction. No second optimizer,
    /// external parameter import, reorthogonalization or output rounding is used.
    /// Ordinary fit() still discards its raw inputs and retains its old behavior.
    ///
    /// All checkpoint input lengths are admitted before fitting. An allocation
    /// failure while recording the result returns no checkpoint. This offline
    /// operation is not a transaction that refunds physical computation.
    pub fn fit_with_checkpoint(policy: LearnedKvPolicy,
        training: &BTreeMap<u64, ModelKvImage>, budget: FitBudget)
        -> Result<(Self, LearnedKvFitCheckpoint), Error>
    {
        let input_bytes = input_bytes(training)?;
        let codec = Self::fit(policy, training, budget)?;
        let witness = comparison(&codec)?;
        let saved = LearnedKvFitCheckpoint { data: Rc::new(FitCheckpointData {
            policy, budget, training: training.clone(), input_bytes, witness,
        }) };
        Ok((codec, saved))
    }
}

impl LearnedKvFitCheckpoint {
    pub fn policy(&self) -> LearnedKvPolicy { self.data.policy }
    pub fn budget(&self) -> FitBudget { self.data.budget }
    pub fn profile(&self) -> &ModelKvProfile {
        self.data.training.values().next().expect("nonempty original training corpus").profile()
    }
    pub fn input_bytes(&self) -> usize { self.data.input_bytes }
    pub fn comparison_bytes(&self) -> usize { self.data.witness.len() }

    /// Rerun the ORIGINAL fit, then compare every emitted parameter word and
    /// every fit-report field before returning the newly computed codec. Saved
    /// coefficient/report bytes are ONLY equality witnesses, never model state.
    ///
    /// The caller funds fresh fitting work, bounded by the saved original caps;
    /// this cannot enlarge a rank, sweep count, corpus or allowance. A replay
    /// receipt is the returned codec's freshly computed fit_report(). Every call
    /// repeats the reported work; no exactly-once execution or speedup is implied.
    /// Model/source identities and train/evaluation split labels remain trusted
    /// declarations, not authentication or statistical independence evidence.
    pub fn replay(&self, budget: FitBudget) -> Result<LearnedKvCodec, Error> {
        check_budget(budget, self.data.budget)?;
        let codec = LearnedKvCodec::fit(self.data.policy, &self.data.training, budget)?;
        if comparison(&codec)? != self.data.witness { return Err(Error::Binding); }
        Ok(codec)
    }
}

fn check_budget(actual: FitBudget, original: FitBudget) -> Result<(), Error> {
    if actual.source_values > original.source_values
        || actual.parameter_values > original.parameter_values
        || actual.scratch_values > original.scratch_values
        || actual.work_units > original.work_units { return Err(Error::Limit); }
    Ok(())
}

fn input_bytes(training: &BTreeMap<u64, ModelKvImage>) -> Result<usize, Error> {
    if training.is_empty() || training.contains_key(&0) { return Err(Error::InvalidInput); }
    if training.len() > MAX_TRAINING_SOURCES { return Err(Error::Limit); }
    let mut total = 0_usize;
    for image in training.values() {
        total = total.checked_add(image.descriptor().image_len()?).ok_or(Error::Limit)?;
        if total > MAX_FIT_CHECKPOINT_INPUT_BYTES { return Err(Error::Limit); }
    }
    Ok(total)
}

// One closed comparison vocabulary. It includes all FitReport fields as bits,
// not float PartialEq (which would erase signed-zero distinctions). There is NO
// inverse function that constructs GroupBasis or FitReport from these bytes.
fn comparison(codec: &LearnedKvCodec) -> Result<Vec<u8>, Error> {
    let mut out = Witness { bytes: Vec::new() };
    out.raw(b"FAKVCMP\x01")?;
    let policy = codec.policy();
    for value in [policy.id(), policy.generation(), policy.rank() as u64, policy.sweeps() as u64] {
        out.word(value)?;
    }
    let report = codec.fit_report();
    out.count(report.sources.len())?;
    for (origin, source) in &report.sources {
        out.word(*origin)?;
        let descriptor = source.descriptor.encode()?;
        out.count(descriptor.len())?; out.raw(&descriptor)?;
    }
    for value in [report.rows_per_group, report.training_values, report.source_coordinate_visits,
        report.parameter_values, report.scratch_values_reserved] { out.count(value)?; }
    out.word(report.work_units_reserved)?;
    if !codec.groups().keys().eq(report.groups.keys()) { return Err(Error::Binding); }
    out.count(codec.groups().len())?;
    for (key, basis) in codec.groups() {
        out.word(key.layer)?;
        out.raw(&[match key.side { super::KvSide::Key => 0, super::KvSide::Value => 1 }])?;
        out.count(key.head)?;
        out.count(basis.mean().len())?; out.count(basis.axes().len())?;
        for value in basis.mean().iter().chain(basis.axes()) { out.raw(&value.to_bits().to_be_bytes())?; }
        let group = &report.groups[key];
        for value in [group.rows, group.channels, group.rotations] { out.count(value)?; }
        for value in [group.covariance_trace, group.remaining_off_diagonal_squared,
            group.emitted_orthogonality_max_error] { out.word(value.to_bits())?; }
        out.count(group.selected_diagonal.len())?;
        for value in &group.selected_diagonal { out.word(value.to_bits())?; }
    }
    Ok(out.bytes)
}
struct Witness { bytes: Vec<u8> }
impl Witness {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let total = self.bytes.len().checked_add(bytes.len()).ok_or(Error::Limit)?;
        if total > MAX_FIT_CHECKPOINT_WITNESS_BYTES { return Err(Error::Limit); }
        self.bytes.try_reserve(bytes.len()).map_err(|_| Error::Limit)?;
        self.bytes.extend_from_slice(bytes); Ok(())
    }
    fn word(&mut self, value: u64) -> Result<(), Error> { self.raw(&value.to_be_bytes()) }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.word(u64::try_from(value).map_err(|_| Error::Limit)?)
    }
}
