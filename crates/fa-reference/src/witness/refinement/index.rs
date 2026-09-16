//! Complete-tail candidate indexing for bounded reference snapshots (FA-061).
//!
//! Deltas are derived here from consecutive complete snapshots, never supplied
//! as an unchecked list of supposedly changed keys. Process-local captures bind
//! judgments to this exact history. The index is advisory: the ordinary budgeted
//! validator still checks the basis and every negative closing frontier.

use super::{Error, ProductFrontiers, Witness, WitnessJudgment, WitnessRefinement, WitnessSnapshot, closed_marker};
use crate::witness::{MAX_WITNESSES, WitnessRequest};
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

/// At most 64 retained revisions, each containing at most twice the snapshot
/// entry limit in changed keys. No value payload is retained in a delta.
pub const MAX_CHANGE_DELTAS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeCost {
    /// Ordered-map lookups, not physical comparisons or wall-clock time.
    pub point_lookups: u64,
    /// Logical bytes charged before equal-length/equal-version comparisons.
    pub value_bytes: u64,
    pub changed_keys: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexFallback {
    ForeignHistory,
    /// The explicit target is not the exact immutable snapshot indexed here.
    TargetNotIndexed,
    HistoryEvicted,
    PlanningBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexSelection {
    /// Candidate membership is NOT an invalidation verdict. ABA changes still
    /// need exact final-value validation, and negative frontiers always recheck.
    Candidates { count: usize },
    ExactFallback(IndexFallback),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexReport {
    pub selection: IndexSelection,
    /// One charged check tests one dependency against one retained delta.
    /// Fixed overhead is bounded by MAX_CHANGE_DELTAS and MAX_WITNESSES.
    pub dependency_checks: u64,
    pub covered_from_revision: u64,
    pub covered_through_revision: u64,
}

struct Delta {
    revision: u64,
    keys: BTreeSet<u64>,
}

/// Historical evidence, not a permit. Construction is available only through
/// this index's capture operation, so equal revision numbers cannot forge a
/// relationship to a different snapshot or a different history.
pub struct IndexedJudgment {
    issuer: Rc<()>,
    judgment: WitnessJudgment,
}

impl IndexedJudgment {
    pub fn judgment(&self) -> &WitnessJudgment {
        &self.judgment
    }
}

/// No live adapter authentication or durable history is implemented here.
/// Snapshot assertions have the same trust boundary as WitnessJudgment. The
/// borrowed current snapshot cannot change under a retained index. A fresh
/// history gets a fresh issuer; it cannot inherit omitted deltas as authority.
pub struct WitnessChangeIndex<'a> {
    issuer: Rc<()>,
    current: &'a WitnessSnapshot,
    floor: u64,
    capacity: usize,
    deltas: VecDeque<Delta>,
}

impl<'a> WitnessChangeIndex<'a> {
    pub fn new(
        initial: &'a WitnessSnapshot,
        frontiers: &ProductFrontiers,
        capacity: usize,
    ) -> Result<Self, Error> {
        if capacity == 0 || capacity > MAX_CHANGE_DELTAS {
            return Err(Error::Limit);
        }
        closed_marker(initial, frontiers)?;
        Ok(Self {
            issuer: Rc::new(()),
            current: initial,
            floor: initial.revision,
            capacity,
            deltas: VecDeque::new(),
        })
    }

    pub fn retained_revisions(&self) -> usize {
        self.deltas.len()
    }

    pub fn oldest_covered_revision(&self) -> u64 {
        self.floor
    }

    pub fn current_revision(&self) -> u64 {
        self.current.revision
    }

    /// Freeze the judgment against the actual indexed snapshot, not a caller's
    /// equal-numbered replacement. No API accepts a caller-supplied capture ID.
    pub fn capture(
        &self,
        frontiers: &ProductFrontiers,
        requests: Vec<WitnessRequest>,
    ) -> Result<IndexedJudgment, Error> {
        Ok(IndexedJudgment {
            issuer: Rc::clone(&self.issuer),
            judgment: WitnessJudgment::capture(self.current, frontiers, requests)?,
        })
    }

    /// Appends exactly the next revision in the same semantic/domain profile.
    /// The control cut may stay equal but cannot regress. Failed admission does
    /// not advance or evict history. The final delta is fully built before any
    /// mutation; peak construction retains at most capacity + 1 bounded deltas.
    pub fn observe(
        &mut self,
        next: &'a WitnessSnapshot,
        frontiers: &ProductFrontiers,
    ) -> Result<ChangeCost, Error> {
        let expected = self.current.revision.checked_add(1).ok_or(Error::Overflow)?;
        if next.revision != expected || next.control_cut < self.current.control_cut {
            return Err(Error::Stale);
        }
        if next.semantic_epoch != self.current.semantic_epoch
            || next.domain_input.domain != self.current.domain_input.domain
        {
            return Err(Error::Binding);
        }
        closed_marker(next, frontiers)?;
        let (keys, cost) = changed_keys(self.current, next);
        if self.deltas.len() == self.capacity {
            let evicted = self.deltas.pop_front().expect("nonzero full capacity");
            self.floor = evicted.revision;
        }
        self.deltas.push_back(Delta { revision: next.revision, keys });
        self.current = next;
        Ok(cost)
    }

    /// Select candidates, then use the SAME exact validator. Missing history,
    /// a foreign capture, a different target (even at equal revision numbers),
    /// or planning exhaustion selects a full exact scan, never a partial mask.
    /// The explicit target prevents a failed observe() from silently validating
    /// against the previous snapshot. Planning costs are separate from advance's
    /// step/byte budget; max_checks bounds delta/dependency intersection checks.
    pub fn begin_refinement<'b>(
        &'b self,
        captured: &'b IndexedJudgment,
        target: &'b WitnessSnapshot,
        frontiers: &'b ProductFrontiers,
        max_checks: u64,
    ) -> (IndexReport, WitnessRefinement<'b>) {
        let mut cursor = captured.judgment.begin_refinement(target, frontiers);
        let mut report = IndexReport {
            selection: IndexSelection::Candidates { count: 0 },
            dependency_checks: 0,
            covered_from_revision: self.floor,
            covered_through_revision: self.current.revision,
        };
        let fallback = if !Rc::ptr_eq(&self.issuer, &captured.issuer) {
            Some(IndexFallback::ForeignHistory)
        } else if !std::ptr::eq(self.current, target) {
            Some(IndexFallback::TargetNotIndexed)
        } else if captured.judgment.revision < self.floor {
            Some(IndexFallback::HistoryEvicted)
        } else {
            None
        };
        if let Some(reason) = fallback {
            report.selection = IndexSelection::ExactFallback(reason);
            return (report, cursor);
        }
        let mut candidates = [false; MAX_WITNESSES];
        for (position, dependency) in captured.judgment.witnesses.iter().enumerate() {
            for delta in self.deltas.iter().filter(|delta| delta.revision > captured.judgment.revision) {
                if report.dependency_checks == max_checks {
                    report.selection = IndexSelection::ExactFallback(IndexFallback::PlanningBudget);
                    return (report, cursor);
                }
                report.dependency_checks += 1;
                let overlaps = match dependency {
                    Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. } => delta.keys.contains(key),
                    Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. } => {
                        delta.keys.range(*start..*end).next().is_some()
                    }
                };
                if overlaps {
                    candidates[position] = true;
                    break;
                }
            }
        }
        report.selection = IndexSelection::Candidates {
            count: candidates.iter().filter(|candidate| **candidate).count(),
        };
        cursor.candidates = Some(candidates);
        (report, cursor)
    }
}

fn changed_keys(before: &WitnessSnapshot, after: &WitnessSnapshot) -> (BTreeSet<u64>, ChangeCost) {
    let mut keys = BTreeSet::new();
    let mut cost = ChangeCost::default();
    // Snapshot construction bounds both maps to 256 entries and values to 8 KiB;
    // these counters are bounded by 512 lookups and 2 MiB, respectively.
    for (&key, old) in &before.values {
        cost.point_lookups += 1;
        let changed = match after.values.get(&key) {
            None => true,
            Some(new) if old.version != new.version || old.value.len() != new.value.len() => true,
            Some(new) => {
                cost.value_bytes += old.value.len() as u64;
                old.value != new.value
            }
        };
        if changed {
            keys.insert(key);
        }
    }
    for key in after.values.keys() {
        cost.point_lookups += 1;
        if !before.values.contains_key(key) {
            keys.insert(*key);
        }
    }
    cost.changed_keys = keys.len();
    (keys, cost)
}

#[cfg(test)]
mod tests;
