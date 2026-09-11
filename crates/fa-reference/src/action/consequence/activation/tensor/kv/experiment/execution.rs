//! Experimental materialization and paired probe calculations.
//!
//! Reuses the existing exact writer and numerical oracle. Synthetic temporary
//! frames stay private; no branch is exported as an observed SourceFrame.

use super::{KvBranch, KvBranchBasis, KvCell, KvSide, contract};
use super::super::restore::{
    KvDestination, KvRestoreReceipt, KvRestoreWindow, PreparedKvRestore, prepare_pair, source_range,
};
use crate::action::consequence::activation::{ProgressiveFrame, SourceFrame};
use crate::action::consequence::activation::probe::{
    LinearProbe, ProbeIdentity, ProbeOutcome, ScoreInterval,
};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

/// These are exact scores of supplied and intervened values, not observed
/// outcomes of resumed model execution or a certificate of causal validity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvProbeComparison {
    pub reference: KvBranchBasis,
    pub candidate: KvBranchBasis,
    pub side: KvSide,
    pub position: u64,
    pub probe: ProbeIdentity,
    pub reference_score: ScoreInterval,
    pub candidate_score: ScoreInterval,
    pub reference_outcome: ProbeOutcome,
    pub candidate_outcome: ProbeOutcome,
    /// New scalar arrays allocated for changed rows, excluding codec copies.
    pub copied_values: usize,
    /// Encoded length of the two exact blocks, not total memory traffic.
    pub encoded_block_bytes: usize,
}

/// Explicit intervention provenance wraps the original writer's byte receipt.
/// It must not be presented as a new capture or native model-restart receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvExperimentRestoreReceipt {
    pub branch: KvBranchBasis,
    pub restoration: KvRestoreReceipt,
    pub copied_values: usize,
}

#[derive(Debug)]
#[must_use = "experimental restoration writes only when commit consumes the plan"]
pub struct PreparedKvExperimentRestore<'a> {
    plan: PreparedKvRestore<'a>,
    branch: KvBranchBasis,
    copied_values: usize,
}

impl PreparedKvExperimentRestore<'_> {
    pub fn staged_bytes(&self) -> usize { self.plan.staged_bytes() }
    pub fn commit(self) -> KvExperimentRestoreReceipt {
        KvExperimentRestoreReceipt { branch: self.branch,
            restoration: self.plan.commit(), copied_values: self.copied_values }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvTwinRestoreReceipt {
    pub reference: KvExperimentRestoreReceipt,
    pub candidate: KvExperimentRestoreReceipt,
}

/// Both destinations are exclusively borrowed and fully staged. No write occurs
/// when preparing the pair or dropping it. Commit has no fallible callback.
/// This is RAM call-level atomicity, not crash/device atomicity or authorization.
#[derive(Debug)]
#[must_use = "neither twin is written until commit consumes the paired plan"]
pub struct PreparedKvTwinRestore<'a> {
    reference: PreparedKvExperimentRestore<'a>,
    candidate: PreparedKvExperimentRestore<'a>,
}

impl PreparedKvTwinRestore<'_> {
    pub fn staged_bytes(&self) -> usize {
        self.reference.staged_bytes() + self.candidate.staged_bytes()
    }
    pub fn commit(self) -> KvTwinRestoreReceipt {
        KvTwinRestoreReceipt { reference: self.reference.commit(), candidate: self.candidate.commit() }
    }
}

impl KvBranch {
    /// Compare the SAME declared probe at the SAME captured position. Both
    /// branches must descend from the identical retained base instance, not just
    /// a matching numeric experiment ID or a same-shaped descriptor.
    pub fn compare_probe(
        &self, reference: &KvBranch, side: KvSide, position: u64, probe: &LinearProbe,
    ) -> Result<KvProbeComparison, Error> {
        self.check_common_base(reference)?;
        let contract = contract(&self.base.image, side);
        if probe.identity().profile != contract.profile()
            || probe.identity().dimensions != contract.dimensions()
        { return Err(Error::Binding); }
        let (left, left_copies) = reference.materialize_row(side, position)?;
        let (right, right_copies) = self.materialize_row(side, position)?;
        let left_bytes = left.encode_initial(23)?;
        let right_bytes = right.encode_initial(23)?;
        let left_frame = ProgressiveFrame::from_initial(&left.verify_block(&left_bytes)?)?;
        let right_frame = ProgressiveFrame::from_initial(&right.verify_block(&right_bytes)?)?;
        let left_score = probe.evaluate(&left_frame)?;
        let right_score = probe.evaluate(&right_frame)?;
        Ok(KvProbeComparison {
            reference: reference.basis(), candidate: self.basis(), side, position, probe: probe.identity(),
            reference_score: left_score.interval().clone(), candidate_score: right_score.interval().clone(),
            reference_outcome: left_score.outcome(), candidate_outcome: right_score.outcome(),
            copied_values: left_copies + right_copies,
            encoded_block_bytes: left_bytes.len() + right_bytes.len(),
        })
    }

    /// Restore experimental values into explicitly supplied CPU buffers using
    /// the original lossless, prevalidated writer. No observed-image export is
    /// exposed, and the result retains the intervened branch's identity.
    pub fn prepare_restore<'a>(
        &self, destination: KvDestination<'a>, window: KvRestoreWindow,
    ) -> Result<PreparedKvExperimentRestore<'a>, Error> {
        let descriptor = self.base.image.descriptor();
        let range = source_range(descriptor.first_position, descriptor.token_count, window)?;
        let mut keys = Vec::new();
        let mut values = Vec::new();
        keys.try_reserve_exact(range.len()).map_err(|_| Error::Limit)?;
        values.try_reserve_exact(range.len()).map_err(|_| Error::Limit)?;
        let mut copied_values = 0_usize;
        for offset in range {
            let position = descriptor.first_position + offset as u64;
            let (key, count) = self.materialize_row(KvSide::Key, position)?;
            copied_values += count;
            let (value, count) = self.materialize_row(KvSide::Value, position)?;
            copied_values += count;
            keys.push(key);
            values.push(value);
        }
        let key_refs: Vec<_> = keys.iter().collect();
        let value_refs: Vec<_> = values.iter().collect();
        let plan = prepare_pair(&descriptor.contract, &key_refs, &value_refs, destination, window)?;
        Ok(PreparedKvExperimentRestore { plan, branch: self.basis(), copied_values })
    }

    /// Stage the unchanged/control and intervention legs before writing either.
    /// Independent exclusive slices must also name disjoint storage objects.
    pub fn prepare_twin_restore<'a>(
        &self, reference: &KvBranch, reference_destination: KvDestination<'a>,
        candidate_destination: KvDestination<'a>, window: KvRestoreWindow,
    ) -> Result<PreparedKvTwinRestore<'a>, Error> {
        self.check_common_base(reference)?;
        let left = destination_ids(&reference_destination);
        let right = destination_ids(&candidate_destination);
        if left.iter().any(|id| right.contains(id)) { return Err(Error::Binding); }
        let reference = reference.prepare_restore(reference_destination, window)?;
        let candidate = self.prepare_restore(candidate_destination, window)?;
        Ok(PreparedKvTwinRestore { reference, candidate })
    }

    pub(in crate::action::consequence::activation::tensor::kv) fn check_common_base(&self, other: &KvBranch) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.base, &other.base) { return Err(Error::Binding); }
        Ok(())
    }

    /// No array copy for an unchanged row. Changed rows get one private owned
    /// bit array; only their exact sparse overrides are applied. Temporary source
    /// identities describe baseline coordinates, never new observed provenance.
    fn materialize_row(&self, side: KvSide, position: u64) -> Result<(SourceFrame, usize), Error> {
        let token = self.base.image.token(position)?;
        let original = match side { KvSide::Key => token.key(), KvSide::Value => token.value() };
        let contract = contract(&self.base.image, side);
        let first = KvCell { side, position, head: 0, channel: 0 };
        let last = KvCell { side, position, head: usize::MAX, channel: usize::MAX };
        let mut changes = BTreeMap::new();
        let mut next = Some(self.node.as_ref());
        while let Some(node) = next {
            for (cell, edit) in node.edits.range(first..=last) {
                changes.entry(cell.head * contract.channels() + cell.channel)
                    .or_insert(edit.replacement_bits);
            }
            next = node.parent.as_deref();
        }
        if changes.iter().all(|(index, value)| original.words[*index] == *value) {
            return Ok((original.clone(), 0));
        }
        let mut words = Vec::new();
        words.try_reserve_exact(original.dimensions()).map_err(|_| Error::Limit)?;
        words.extend_from_slice(&original.words);
        for (index, value) in changes { words[index] = value; }
        let count = words.len();
        Ok((SourceFrame { identity: original.identity(), words: words.into(), binding: Rc::new(()) }, count))
    }
}

fn destination_ids(destination: &KvDestination<'_>) -> [u64; 2] {
    match destination {
        KvDestination::Separate { keys, values } => [keys.identity.object, values.identity.object],
        KvDestination::Shared { identity, .. } => [identity.object; 2],
    }
}
