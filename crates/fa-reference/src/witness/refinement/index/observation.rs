//! Cooperative construction of the complete change tail. No partial delta is
//! visible to candidate selection: only an explicitly committed, complete scan
//! replaces the indexed snapshot. Dropping a scan leaves the history unchanged.

use super::{
    ChangeCost, Delta, Error, ProductFrontiers, WitnessChangeIndex, WitnessSnapshot, closed_marker,
};
use crate::witness::{SnapshotEntry, refinement::RefinementBudget};
use std::collections::{BTreeSet, btree_map::{Iter, Keys}};

/// Progress of a change observation, never evidence that a judgment is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeObservationOutcome {
    NeedsWork {
        /// Budget for the next atomic step, not the rest of the observation.
        minimum: RefinementBudget,
    },
    /// The full delta is private until `ChangeObservation::commit` consumes it.
    Ready,
    Refused(Error),
    Cancelled,
}

/// Logical work is retained through exhaustion, refusal and explicit cancellation.
/// `cost` counts the inspected prefix, not a published delta or a skip decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeObservationReport {
    pub outcome: ChangeObservationOutcome,
    pub spent: RefinementBudget,
    pub total: RefinementBudget,
    pub cost: ChangeCost,
}

#[derive(Clone, Copy)]
enum Stage<'a> {
    Admission,
    OldKeys,
    Compare {
        key: u64,
        expected: &'a [u8],
        actual: &'a [u8],
        offset: usize,
    },
    NewKeys,
}

/// A bounded, exclusively owned update of the existing complete-tail index.
///
/// Creating a session does no payload work. `advance` reserves each logical
/// step and comparison chunk before executing it. A step is one admission
/// check, one ordered-map lookup (including iterator advance), one exhausted
/// iterator check, or one byte-comparison chunk. Admission includes the existing
/// bounded closing-frontier check. Map operations, allocation and publication
/// housekeeping are not CPU-instruction or wall-clock bounds.
///
/// Only changed keys are retained: at most twice `MAX_SNAPSHOT_ENTRIES`, never
/// copied values. Iterators and slices borrow the original immutable snapshots.
/// The index is exclusively borrowed until commit, cancellation or drop. Thus a
/// partial scan cannot be used to skip validation or evict a complete delta.
/// A terminal result is idempotent; a ready result still does not publish.
///
/// ```compile_fail,E0502
/// use fa_reference::product_frontier::ProductFrontiers;
/// use fa_reference::witness::{WitnessSnapshot, refinement::RefinementBudget};
/// use fa_reference::witness::refinement::index::WitnessChangeIndex;
/// fn no_partial_history<'a>(index: &mut WitnessChangeIndex<'a>,
///     next: &'a WitnessSnapshot, frontiers: &ProductFrontiers) {
///     let mut pending = index.begin_observe(next, frontiers);
///     let _ = index.current_revision();
///     let _ = pending.advance(RefinementBudget::default());
/// }
/// ```
///
/// ```compile_fail,E0382
/// use fa_reference::witness::refinement::index::ChangeObservation;
/// fn no_double_publication(pending: ChangeObservation<'_, '_>) {
///     let _ = pending.commit();
///     let _ = pending.commit();
/// }
/// ```
#[must_use = "an unfinished observation does not update the index"]
pub struct ChangeObservation<'b, 'a> {
    index: &'b mut WitnessChangeIndex<'a>,
    before: &'a WitnessSnapshot,
    next: &'a WitnessSnapshot,
    frontiers: &'b ProductFrontiers,
    old_keys: Iter<'a, u64, SnapshotEntry>,
    new_keys: Keys<'a, u64, SnapshotEntry>,
    keys: BTreeSet<u64>,
    stage: Stage<'a>,
    total: RefinementBudget,
    cost: ChangeCost,
    terminal: Option<ChangeObservationOutcome>,
}

impl<'a> WitnessChangeIndex<'a> {
    /// Prepare a consecutive revision without scanning payloads or modifying
    /// history. Admission errors are returned by the first budgeted advance.
    pub fn begin_observe<'b>(
        &'b mut self,
        next: &'a WitnessSnapshot,
        frontiers: &'b ProductFrontiers,
    ) -> ChangeObservation<'b, 'a> {
        let before = self.current;
        ChangeObservation {
            index: self,
            before,
            next,
            frontiers,
            old_keys: before.values.iter(),
            new_keys: next.values.keys(),
            keys: BTreeSet::new(),
            stage: Stage::Admission,
            total: RefinementBudget::default(),
            cost: ChangeCost::default(),
            terminal: None,
        }
    }
}

impl ChangeObservation<'_, '_> {
    #[must_use]
    pub fn total_work(&self) -> RefinementBudget {
        self.total
    }

    /// Continue the same frozen comparison without restarting a value prefix.
    /// Zero budgets perform no work. Comparison bytes are charged before the
    /// comparison, including the entire chunk containing the first mismatch.
    /// Consequently chunked mismatch costs may be lower than a whole-value
    /// synchronous comparison, while the final changed-key set is identical.
    pub fn advance(&mut self, budget: RefinementBudget) -> ChangeObservationReport {
        let mut spent = RefinementBudget::default();
        loop {
            if let Some(outcome) = self.terminal {
                return self.report(outcome, spent);
            }
            let minimum = RefinementBudget {
                steps: 1,
                value_bytes: u64::from(matches!(self.stage, Stage::Compare { .. })),
            };
            let bytes_left = budget.value_bytes - spent.value_bytes;
            if spent.steps == budget.steps || bytes_left < minimum.value_bytes {
                return self.report(ChangeObservationOutcome::NeedsWork { minimum }, spent);
            }
            let bytes = match self.stage {
                Stage::Compare { expected, offset, .. } => {
                    bytes_left.min((expected.len() - offset) as u64)
                }
                _ => 0,
            };
            let Some(steps) = self.total.steps.checked_add(1) else {
                self.terminal = Some(ChangeObservationOutcome::Refused(Error::Overflow));
                continue;
            };
            let Some(value_bytes) = self.total.value_bytes.checked_add(bytes) else {
                self.terminal = Some(ChangeObservationOutcome::Refused(Error::Overflow));
                continue;
            };
            self.total = RefinementBudget { steps, value_bytes };
            spent.steps += 1;
            spent.value_bytes += bytes;
            self.cost.value_bytes = value_bytes;
            if let Err(error) = self.step(bytes as usize) {
                self.terminal = Some(ChangeObservationOutcome::Refused(error));
            }
            self.cost.changed_keys = self.keys.len();
        }
    }

    /// Cancel without refunding the reported work or changing complete history.
    /// A prior refusal remains the terminal reason, rather than being masked.
    #[must_use]
    pub fn cancel(self) -> ChangeObservationReport {
        let outcome = match self.terminal {
            Some(ChangeObservationOutcome::Refused(error)) => {
                ChangeObservationOutcome::Refused(error)
            }
            _ => ChangeObservationOutcome::Cancelled,
        };
        self.report(outcome, RefinementBudget::default())
    }

    /// Publish only a fully scanned delta. This consumes the update, so it
    /// cannot commit twice. No payload reads or comparisons occur here; bounded
    /// delta eviction and queue housekeeping are separate from scan work.
    /// An unfinished or refused update cannot mutate the index.
    pub fn commit(self) -> Result<ChangeCost, Error> {
        match self.terminal {
            Some(ChangeObservationOutcome::Ready) => {}
            Some(ChangeObservationOutcome::Refused(error)) => return Err(error),
            _ => return Err(Error::Incomplete),
        }
        if self.index.deltas.len() == self.index.capacity {
            let evicted = self.index.deltas.pop_front().ok_or(Error::WrongState)?;
            self.index.floor = evicted.revision;
        }
        self.index.deltas.push_back(Delta {
            revision: self.next.revision,
            keys: self.keys,
        });
        self.index.current = self.next;
        Ok(self.cost)
    }

    fn report(
        &self,
        outcome: ChangeObservationOutcome,
        spent: RefinementBudget,
    ) -> ChangeObservationReport {
        ChangeObservationReport { outcome, spent, total: self.total, cost: self.cost }
    }

    fn step(&mut self, bytes: usize) -> Result<(), Error> {
        match self.stage {
            Stage::Admission => {
                let expected = self.before.revision.checked_add(1).ok_or(Error::Overflow)?;
                if self.next.revision != expected || self.next.control_cut < self.before.control_cut {
                    return Err(Error::Stale);
                }
                if self.next.semantic_epoch != self.before.semantic_epoch
                    || self.next.domain_input.domain != self.before.domain_input.domain
                {
                    return Err(Error::Binding);
                }
                closed_marker(self.next, self.frontiers)?;
                self.stage = Stage::OldKeys;
            }
            Stage::OldKeys => {
                let Some((&key, old)) = self.old_keys.next() else {
                    self.stage = Stage::NewKeys;
                    return Ok(());
                };
                self.cost.point_lookups += 1;
                match self.next.values.get(&key) {
                    None => { self.keys.insert(key); }
                    Some(new) if old.version != new.version || old.value.len() != new.value.len() => {
                        self.keys.insert(key);
                    }
                    Some(new) if !old.value.is_empty() => {
                        self.stage = Stage::Compare {
                            key, expected: &old.value, actual: &new.value, offset: 0,
                        };
                    }
                    Some(_) => {}
                }
            }
            Stage::Compare { key, expected, actual, offset } => {
                // Admission to this stage establishes equal lengths and a
                // nonempty remainder; advance reserves at most that remainder.
                let end = offset + bytes;
                if expected[offset..end] != actual[offset..end] {
                    self.keys.insert(key);
                    self.stage = Stage::OldKeys;
                } else if end == expected.len() {
                    self.stage = Stage::OldKeys;
                } else {
                    self.stage = Stage::Compare { key, expected, actual, offset: end };
                }
            }
            Stage::NewKeys => {
                if let Some(&key) = self.new_keys.next() {
                    self.cost.point_lookups += 1;
                    if !self.before.values.contains_key(&key) {
                        self.keys.insert(key);
                    }
                } else {
                    self.terminal = Some(ChangeObservationOutcome::Ready);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
