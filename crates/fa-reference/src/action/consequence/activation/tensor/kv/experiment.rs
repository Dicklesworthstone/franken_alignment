//! Capability-free sparse interventions over a pinned, immutable KV image.
//!
//! Forks copy edited scalar records, not model-state arrays. Experimental values
//! never escape as SourceFrame, KvImage, a live capture frontier or a Permit.
//! Base authenticity and host-buffer ownership remain the caller's obligations.

use super::image::{KvImage, KvImageDescriptor};
use super::restore::encode_exact;
use super::super::TensorContract;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_KV_BRANCHES: usize = 128;
pub const MAX_KV_BRANCH_DEPTH: usize = 32;
pub const MAX_KV_EDITS_PER_FORK: usize = 1024;
pub const MAX_KV_RETAINED_EDITS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KvSide { Key, Value }

/// Absolute position, stored cache head and channel; not a query-head index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KvCell {
    pub side: KvSide,
    pub position: u64,
    pub head: usize,
    pub channel: usize,
}

/// Binary32 bits of the normalized scalar. The original storage encoding must
/// represent the replacement exactly. Expected bits include the sign of zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvEdit {
    pub cell: KvCell,
    pub expected_bits: u32,
    pub replacement_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvEditScope {
    pub first_position: u64,
    pub token_count: usize,
    pub keys: bool,
    pub values: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvExperimentLimits {
    /// Nonbaseline nodes, including rebases and unchanged control forks.
    pub branches: usize,
    /// Lifetime retained entries, including no-op edits and rebase entries.
    pub retained_edits: usize,
    pub resolution_depth: usize,
}

impl KvExperimentLimits {
    fn validate(self) -> Result<(), Error> {
        if self.branches == 0 || self.retained_edits == 0 || self.resolution_depth == 0 {
            return Err(Error::InvalidInput);
        }
        if self.branches > MAX_KV_BRANCHES || self.retained_edits > MAX_KV_RETAINED_EDITS
            || self.resolution_depth > MAX_KV_BRANCH_DEPTH
        { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KvBranchKind { Baseline, Intervention, Rebase }

/// Provenance of experimental data, not a claim that an actor produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvBranchBasis {
    pub experiment: u64,
    pub branch: u64,
    pub kind: KvBranchKind,
    pub derived_from: Option<u64>,
    pub resolution_depth: usize,
    pub source: KvImageDescriptor,
}

#[derive(Debug)]
struct Base {
    experiment: u64,
    image: KvImage,
    scope: KvEditScope,
    end: u64,
}

#[derive(Debug)]
struct Node {
    id: u64,
    kind: KvBranchKind,
    depth: usize,
    /// Resolution may be shortened by a rebase, without erasing provenance.
    parent: Option<Rc<Node>>,
    origin: Option<Rc<Node>>,
    edits: BTreeMap<KvCell, KvEdit>,
}

/// Cloneable evidence only. A surviving handle pins its original values and
/// all provenance even after the capture and experiment builder are dropped.
/// There is no conversion into captured observations or live authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::experiment::KvBranch;
/// use fa_reference::action::Permit;
/// fn grant(branch: KvBranch) -> Permit { branch }
/// ```
#[derive(Clone, Debug)]
pub struct KvBranch { base: Rc<Base>, node: Rc<Node> }

impl KvBranch {
    pub fn id(&self) -> u64 { self.node.id }
    pub fn basis(&self) -> KvBranchBasis {
        KvBranchBasis {
            experiment: self.base.experiment, branch: self.node.id, kind: self.node.kind,
            derived_from: self.node.origin.as_ref().map(|node| node.id),
            resolution_depth: self.node.depth, source: self.base.image.descriptor().clone(),
        }
    }

    /// Data inspection only. No missing position or coordinate is filled in.
    pub fn bits(&self, cell: KvCell) -> Result<u32, Error> {
        let original = baseline_bits(&self.base.image, cell)?;
        let mut next = Some(self.node.as_ref());
        while let Some(node) = next {
            if let Some(edit) = node.edits.get(&cell) { return Ok(edit.replacement_bits); }
            next = node.parent.as_deref();
        }
        Ok(original)
    }

    /// Chronological derivation, including rebases and no-op controls. A rebase
    /// shortens lookup depth; it does not rewrite the intervention history.
    pub fn lineage(&self) -> Vec<(u64, KvBranchKind)> {
        let mut lineage = Vec::new();
        let mut next = Some(self.node.as_ref());
        while let Some(node) = next {
            lineage.push((node.id, node.kind));
            next = node.origin.as_deref();
        }
        lineage.reverse();
        lineage
    }

    /// Canonical effective differences against the ORIGINAL pinned image.
    /// Ancestor edits later undone remain in lineage, not in this net delta.
    pub fn delta(&self) -> Result<Vec<KvEdit>, Error> {
        Ok(self.effective_edits()?.into_values().collect())
    }

    fn effective_edits(&self) -> Result<BTreeMap<KvCell, KvEdit>, Error> {
        let mut effective = BTreeMap::new();
        let mut next = Some(self.node.as_ref());
        while let Some(node) = next {
            for (cell, edit) in &node.edits {
                effective.entry(*cell).or_insert(*edit);
            }
            next = node.parent.as_deref();
        }
        let mut changes = BTreeMap::new();
        for (cell, edit) in effective {
            let original = baseline_bits(&self.base.image, cell)?;
            if original != edit.replacement_bits {
                changes.insert(cell, KvEdit { expected_bits: original, ..edit });
            }
        }
        Ok(changes)
    }
}

/// Owns bounded experiment history, not a copy of a control or resource ledger.
/// An edit outside its frozen scope cannot be admitted through another branch.
#[derive(Debug)]
pub struct KvExperiment {
    root: KvBranch,
    nodes: BTreeMap<u64, Rc<Node>>,
    limits: KvExperimentLimits,
    retained_edits: usize,
}

impl KvExperiment {
    pub fn new(
        id: u64, image: KvImage, scope: KvEditScope, limits: KvExperimentLimits,
    ) -> Result<Self, Error> {
        limits.validate()?;
        image.descriptor().encoded_len()?;
        if id == 0 || scope.token_count == 0 || (!scope.keys && !scope.values) {
            return Err(Error::InvalidInput);
        }
        let count = u64::try_from(scope.token_count).map_err(|_| Error::Overflow)?;
        let end = scope.first_position.checked_add(count).ok_or(Error::Overflow)?;
        let descriptor = image.descriptor();
        let image_end = descriptor.first_position.checked_add(descriptor.token_count as u64)
            .ok_or(Error::Overflow)?;
        if scope.first_position < descriptor.first_position || end > image_end {
            return Err(Error::Missing);
        }
        let node = Rc::new(Node { id: 0, kind: KvBranchKind::Baseline, depth: 0,
            parent: None, origin: None, edits: BTreeMap::new() });
        let base = Rc::new(Base { experiment: id, image, scope, end });
        let root = KvBranch { base, node: Rc::clone(&node) };
        Ok(Self { root, nodes: BTreeMap::from([(0, node)]), limits, retained_edits: 0 })
    }

    pub fn baseline(&self) -> KvBranch { self.root.clone() }
    pub fn branch_count(&self) -> usize { self.nodes.len() - 1 }
    pub fn retained_edit_count(&self) -> usize { self.retained_edits }

    /// All Result-producing validation precedes publication of the new node.
    /// An empty edit set is an explicit unchanged control, not a free new budget.
    pub fn fork(&mut self, id: u64, parent: &KvBranch, edits: &[KvEdit]) -> Result<KvBranch, Error> {
        self.check_new_node(id, parent)?;
        if edits.len() > MAX_KV_EDITS_PER_FORK { return Err(Error::Limit); }
        let depth = parent.node.depth.checked_add(1).ok_or(Error::Overflow)?;
        if depth > self.limits.resolution_depth { return Err(Error::Limit); }
        let retained = self.checked_retention(edits.len())?;
        let mut staged = BTreeMap::new();
        for edit in edits {
            self.check_edit_scope(edit.cell)?;
            if parent.bits(edit.cell)? != edit.expected_bits { return Err(Error::Stale); }
            let contract = contract(&self.root.base.image, edit.cell.side);
            encode_exact(edit.replacement_bits, contract.encoding(), contract.byte_order())?;
            if staged.insert(edit.cell, *edit).is_some() { return Err(Error::Duplicate); }
        }
        Ok(self.publish(id, parent, Rc::clone(&parent.node), KvBranchKind::Intervention,
            depth, staged, retained))
    }

    /// Rebase exact sparse data onto the same pinned image. The source node is
    /// retained as provenance; no base or historical edit is retired or refunded.
    pub fn rebase(&mut self, id: u64, source: &KvBranch) -> Result<KvBranch, Error> {
        self.check_new_node(id, source)?;
        let staged = source.effective_edits()?;
        let retained = self.checked_retention(staged.len())?;
        // Separate point lookups check the composed map before publication.
        for edit in staged.values() {
            if source.bits(edit.cell)? != edit.replacement_bits { return Err(Error::Binding); }
        }
        Ok(self.publish(id, source, Rc::clone(&self.root.node), KvBranchKind::Rebase,
            1, staged, retained))
    }

    fn check_new_node(&self, id: u64, source: &KvBranch) -> Result<(), Error> {
        self.check_branch(source)?;
        if id == 0 { return Err(Error::InvalidInput); }
        if self.nodes.contains_key(&id) { return Err(Error::Duplicate); }
        if self.branch_count() >= self.limits.branches { return Err(Error::Limit); }
        Ok(())
    }

    fn check_branch(&self, branch: &KvBranch) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.root.base, &branch.base)
            || !self.nodes.get(&branch.node.id).is_some_and(|node| Rc::ptr_eq(node, &branch.node))
        { return Err(Error::Binding); }
        Ok(())
    }

    fn check_edit_scope(&self, cell: KvCell) -> Result<(), Error> {
        let base = &self.root.base;
        let side = match cell.side { KvSide::Key => base.scope.keys, KvSide::Value => base.scope.values };
        if !side || cell.position < base.scope.first_position || cell.position >= base.end {
            return Err(Error::Binding);
        }
        baseline_bits(&base.image, cell)?;
        Ok(())
    }

    fn checked_retention(&self, count: usize) -> Result<usize, Error> {
        let retained = self.retained_edits.checked_add(count).ok_or(Error::Overflow)?;
        if retained > self.limits.retained_edits { return Err(Error::Limit); }
        Ok(retained)
    }

    fn publish(
        &mut self, id: u64, origin: &KvBranch, parent: Rc<Node>, kind: KvBranchKind,
        depth: usize, edits: BTreeMap<KvCell, KvEdit>, retained: usize,
    ) -> KvBranch {
        let node = Rc::new(Node { id, kind, depth, parent: Some(parent),
            origin: Some(Rc::clone(&origin.node)), edits });
        self.nodes.insert(id, Rc::clone(&node));
        self.retained_edits = retained;
        KvBranch { base: Rc::clone(&self.root.base), node }
    }
}

fn contract(image: &KvImage, side: KvSide) -> &TensorContract {
    match side { KvSide::Key => image.descriptor().contract.keys(), KvSide::Value => image.descriptor().contract.values() }
}

fn baseline_bits(image: &KvImage, cell: KvCell) -> Result<u32, Error> {
    let contract = contract(image, cell.side);
    if cell.head >= contract.heads() || cell.channel >= contract.channels() { return Err(Error::InvalidInput); }
    let token = image.token(cell.position)?;
    let source = match cell.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
    source.words.get(cell.head * contract.channels() + cell.channel).copied().ok_or(Error::Missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::CaptureProfile;
    use crate::action::consequence::activation::tensor::{
        BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorLayout,
    };
    use super::super::{KvAppend, KvBudget, KvCapture, KvContract};

    fn image() -> KvImage {
        let profile = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
        let keys = TensorContract::new(profile, ScalarEncoding::Binary32, ByteOrder::Little, 1, 2).unwrap();
        let values = TensorContract::new(CaptureProfile { tap: 6, ..profile }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 2).unwrap();
        let mut capture = KvCapture::new(KvContract::new(keys, values, 2).unwrap(), 7, 0, 10, 20,
            KvBudget { positions: 2, normalized_values: 8 }).unwrap();
        let layout = TensorLayout::new([1, 2, 1, 2], [16, 8, 8, 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
        let kb: Vec<_> = [1_f32, 2.0, 3.0, 4.0].into_iter().flat_map(f32::to_le_bytes).collect();
        let vb: Vec<_> = [5_f32, 6.0, 7.0, 8.0].into_iter().flat_map(f32::to_le_bytes).collect();
        capture.append(0, KvAppend {
            keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &layout, bytes: &kb },
            values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &layout, bytes: &vb },
            first_token: 0, token_count: 2, buffer_first_position: 10, first_sequence: 20,
        }).unwrap();
        capture.snapshot(1).unwrap()
    }

    fn limits() -> KvExperimentLimits {
        KvExperimentLimits { branches: 16, retained_edits: 64, resolution_depth: 2 }
    }
    fn experiment() -> KvExperiment {
        KvExperiment::new(1, image(), KvEditScope { first_position: 10, token_count: 2, keys: true, values: true }, limits()).unwrap()
    }
    fn cell(side: KvSide, position: u64, channel: usize) -> KvCell {
        KvCell { side, position, head: 0, channel }
    }
    fn edit(cell: KvCell, expected: f32, replacement: f32) -> KvEdit {
        KvEdit { cell, expected_bits: expected.to_bits(), replacement_bits: replacement.to_bits() }
    }

    #[test]
    fn sparse_siblings_do_not_modify_the_base_or_each_other() {
        let mut space = experiment();
        let root = space.baseline();
        let before = root.base.image.encode().unwrap();
        let c = cell(KvSide::Key, 10, 0);
        let left = space.fork(1, &root, &[edit(c, 1.0, -1.0)]).unwrap();
        let right = space.fork(2, &root, &[edit(c, 1.0, 9.0)]).unwrap();
        assert_eq!(root.bits(c).unwrap(), 1_f32.to_bits());
        assert_eq!(left.bits(c).unwrap(), (-1_f32).to_bits());
        assert_eq!(right.bits(c).unwrap(), 9_f32.to_bits());
        assert_eq!(root.base.image.encode().unwrap(), before);
        assert!(Rc::ptr_eq(&root.base, &left.base));
        assert!(Rc::ptr_eq(&left.node, &left.clone().node));
        assert_eq!(space.retained_edit_count(), 2);
    }

    #[test]
    fn late_bad_expected_value_and_duplicate_cells_publish_nothing() {
        let mut space = experiment();
        let root = space.baseline();
        let c = cell(KvSide::Key, 10, 0);
        let v = cell(KvSide::Value, 11, 1);
        assert_eq!(space.fork(1, &root, &[edit(c, 1.0, 4.0), edit(v, 0.0, 9.0)]).unwrap_err(), Error::Stale);
        assert_eq!(space.fork(1, &root, &[edit(c, 1.0, 4.0), edit(c, 1.0, 5.0)]).unwrap_err(), Error::Duplicate);
        assert_eq!((space.branch_count(), space.retained_edit_count()), (0, 0));
        assert!(space.fork(1, &root, &[edit(c, 1.0, 4.0)]).is_ok());
    }

    #[test]
    fn expected_bits_are_parent_relative_including_signed_zero() {
        let mut space = experiment();
        let root = space.baseline();
        let c = cell(KvSide::Key, 10, 0);
        let first = space.fork(1, &root, &[edit(c, 1.0, -0.0)]).unwrap();
        assert_eq!(space.fork(2, &first, &[edit(c, 0.0, 3.0)]).unwrap_err(), Error::Stale);
        let child = space.fork(2, &first, &[edit(c, -0.0, 3.0)]).unwrap();
        assert_eq!(child.bits(c).unwrap(), 3_f32.to_bits());
    }

    #[test]
    fn rebase_preserves_every_value_and_original_intervention_lineage() {
        let mut space = experiment();
        let root = space.baseline();
        let c = cell(KvSide::Key, 10, 0);
        let v = cell(KvSide::Value, 11, 1);
        let first = space.fork(1, &root, &[edit(c, 1.0, -1.0), edit(v, 8.0, 9.0)]).unwrap();
        let second = space.fork(2, &first, &[edit(c, -1.0, 1.0)]).unwrap();
        assert_eq!(space.fork(3, &second, &[]).unwrap_err(), Error::Limit);
        let rebased = space.rebase(3, &second).unwrap();
        assert_eq!(rebased.basis().resolution_depth, 1);
        assert_eq!(rebased.delta().unwrap(), vec![edit(v, 8.0, 9.0)]);
        for side in [KvSide::Key, KvSide::Value] {
            for position in 10..12 {
                for channel in 0..2 {
                    let c = cell(side, position, channel);
                    assert_eq!(second.bits(c), rebased.bits(c));
                }
            }
        }
        assert_eq!(rebased.lineage(), vec![(0, KvBranchKind::Baseline),
            (1, KvBranchKind::Intervention), (2, KvBranchKind::Intervention), (3, KvBranchKind::Rebase)]);
        assert!(space.fork(4, &rebased, &[edit(c, 1.0, 4.0)]).is_ok());
        assert_eq!(space.retained_edit_count(), 5);
    }

    #[test]
    fn dropped_capture_builder_and_original_handles_do_not_break_a_branch() {
        let branch = {
            let mut space = experiment();
            let root = space.baseline();
            space.fork(1, &root, &[edit(cell(KvSide::Value, 10, 0), 5.0, 10.0)]).unwrap()
        };
        assert_eq!(branch.bits(cell(KvSide::Value, 10, 0)).unwrap(), 10_f32.to_bits());
        assert_eq!(branch.bits(cell(KvSide::Key, 11, 1)).unwrap(), 4_f32.to_bits());
        assert_eq!(branch.lineage().len(), 2);
    }

    #[test]
    fn foreign_same_number_experiment_cannot_supply_a_parent() {
        let mut left = experiment();
        let right = experiment();
        assert_eq!(left.fork(1, &right.baseline(), &[]).unwrap_err(), Error::Binding);
        assert_eq!(left.rebase(1, &right.baseline()).unwrap_err(), Error::Binding);
        assert_eq!(left.branch_count(), 0);
    }

    #[test]
    fn frozen_scope_coordinates_and_nonfinite_values_are_checked() {
        let mut space = KvExperiment::new(1, image(), KvEditScope {
            first_position: 10, token_count: 1, keys: true, values: false,
        }, limits()).unwrap();
        let root = space.baseline();
        for c in [cell(KvSide::Value, 10, 0), cell(KvSide::Key, 11, 0)] {
            assert_eq!(space.fork(1, &root, &[edit(c, 1.0, 2.0)]).unwrap_err(), Error::Binding);
        }
        assert_eq!(space.fork(1, &root, &[edit(cell(KvSide::Key, 10, 2), 1.0, 2.0)]).unwrap_err(), Error::InvalidInput);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(space.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, value)]).unwrap_err(), Error::InvalidInput);
        }
        assert_eq!(space.branch_count(), 0);
    }

    #[test]
    fn branch_and_retained_edit_limits_do_not_reset_on_controls_or_rebase() {
        let mut space = KvExperiment::new(1, image(), KvEditScope {
            first_position: 10, token_count: 2, keys: true, values: true,
        }, KvExperimentLimits { branches: 3, retained_edits: 1, resolution_depth: 2 }).unwrap();
        let root = space.baseline();
        let first = space.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, 2.0)]).unwrap();
        assert_eq!(space.rebase(2, &first).unwrap_err(), Error::Limit);
        assert_eq!(space.retained_edit_count(), 1);
        space.fork(2, &root, &[]).unwrap();
        space.rebase(3, &root).unwrap();
        assert_eq!(space.fork(4, &root, &[]).unwrap_err(), Error::Limit);
        assert_eq!(space.fork(1, &root, &[]).unwrap_err(), Error::Duplicate);
    }
}
