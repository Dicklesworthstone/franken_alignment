//! Budgeted exact witness validation for the reference model (FA-060).
//!
//! A session borrows its entire basis, snapshot and frontier set immutably. It
//! cannot resume at another cut, rebase a judgment, or turn a partial scan into
//! permission. The existing eager validator remains an independent differential
//! oracle. Neither API authenticates the caller-supplied adapter observations.

pub mod index;

use super::{
    Error, Invalidation, ProductFrontiers, SnapshotEntry, Witness, WitnessJudgment,
    WitnessSnapshot, closed_marker,
};
use std::collections::btree_map::Range;

/// Logical work limits, not elapsed time or an allocator/CPU instruction count.
/// A step is one basis check, witness dispatch, frontier check, point lookup,
/// range advance, or byte-comparison chunk. Each compared byte is charged before
/// comparison; a short-circuiting mismatch may therefore use fewer physical
/// comparisons than charged. Zero budgets are valid and do not grant validity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefinementBudget {
    pub steps: u64,
    pub value_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefinementOutcome {
    StillValid,
    Invalidated {
        reason: Invalidation,
        /// Original zero-based dependency index; None for a basis mismatch.
        witness: Option<usize>,
    },
    NeedsRefinement {
        /// Minimum budget to make the NEXT atomic step, not to finish the scan.
        minimum: RefinementBudget,
    },
    /// Incomplete evidence is a refusal, never a successful absence check.
    Refused(Error),
}

/// Both per-call and lifetime work survive exhaustion AND evidence refusals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefinementReport {
    pub outcome: RefinementOutcome,
    pub spent: RefinementBudget,
    pub total: RefinementBudget,
}

#[derive(Clone, Copy)]
enum Stage<'a> {
    Basis,
    Begin,
    Frontier,
    Exact,
    Absent,
    Range,
    Compare {
        expected: &'a [u8],
        actual: &'a [u8],
        offset: usize,
        in_range: bool,
    },
}

/// Allocation-free cursor over already captured, bounded witness data. Dropping
/// a cursor discards partial work, not any live authority. Terminal results are
/// idempotent historical results for the borrowed snapshot, not fresh permits.
///
/// ```compile_fail,E0506
/// use fa_reference::witness::{WitnessJudgment, WitnessSnapshot};
/// use fa_reference::product_frontier::ProductFrontiers;
/// use fa_reference::witness::refinement::RefinementBudget;
/// fn replace_during_validation(judgment: &WitnessJudgment,
///     snapshot: &mut WitnessSnapshot, replacement: WitnessSnapshot,
///     frontiers: &ProductFrontiers) {
///     let mut session = judgment.begin_refinement(snapshot, frontiers);
///     *snapshot = replacement;
///     session.advance(RefinementBudget { steps: 1, value_bytes: 1 });
/// }
/// ```
#[must_use = "an unfinished refinement is not evidence of validity"]
pub struct WitnessRefinement<'a> {
    judgment: &'a WitnessJudgment,
    snapshot: &'a WitnessSnapshot,
    frontiers: &'a ProductFrontiers,
    witness: usize,
    member: usize,
    range: Option<Range<'a, u64, SnapshotEntry>>,
    stage: Stage<'a>,
    total: RefinementBudget,
    terminal: Option<RefinementOutcome>,
    // Only the complete-tail index may install this fixed-size candidate mask.
    candidates: Option<[bool; super::MAX_WITNESSES]>,
}

impl WitnessJudgment {
    pub fn begin_refinement<'a>(
        &'a self,
        snapshot: &'a WitnessSnapshot,
        frontiers: &'a ProductFrontiers,
    ) -> WitnessRefinement<'a> {
        WitnessRefinement {
            judgment: self,
            snapshot,
            frontiers,
            witness: 0,
            member: 0,
            range: None,
            stage: Stage::Basis,
            total: RefinementBudget::default(),
            terminal: None,
            candidates: None,
        }
    }
}

impl WitnessRefinement<'_> {
    pub fn total_work(&self) -> RefinementBudget {
        self.total
    }

    /// Never reads another member or compares another byte without first
    /// reserving its work. Even a one-byte budget can progress through a large
    /// value; range scans retain their iterator instead of rescanning a prefix.
    pub fn advance(&mut self, budget: RefinementBudget) -> RefinementReport {
        let mut spent = RefinementBudget::default();
        loop {
            if let Some(outcome) = self.terminal {
                return RefinementReport { outcome, spent, total: self.total };
            }
            let bytes_left = budget.value_bytes - spent.value_bytes;
            let minimum = RefinementBudget {
                steps: 1,
                value_bytes: u64::from(matches!(self.stage, Stage::Compare { .. })),
            };
            if spent.steps == budget.steps || bytes_left < minimum.value_bytes {
                return RefinementReport {
                    outcome: RefinementOutcome::NeedsRefinement { minimum },
                    spent,
                    total: self.total,
                };
            }
            let bytes = match self.stage {
                Stage::Compare { expected, offset, .. } => {
                    bytes_left.min((expected.len() - offset) as u64)
                }
                _ => 0,
            };
            let Some(steps) = self.total.steps.checked_add(1) else {
                self.terminal = Some(RefinementOutcome::Refused(Error::Overflow));
                continue;
            };
            let Some(value_bytes) = self.total.value_bytes.checked_add(bytes) else {
                self.terminal = Some(RefinementOutcome::Refused(Error::Overflow));
                continue;
            };
            self.total = RefinementBudget { steps, value_bytes };
            spent.steps += 1;
            spent.value_bytes += bytes;
            if let Err(error) = self.step(bytes as usize) {
                self.terminal = Some(RefinementOutcome::Refused(error));
            }
        }
    }

    fn invalidate(&mut self, reason: Invalidation, basis: bool) {
        self.terminal = Some(RefinementOutcome::Invalidated {
            reason,
            witness: (!basis).then_some(self.witness),
        });
    }

    fn next_witness(&mut self) {
        self.witness += 1;
        self.range = None;
        self.member = 0;
        self.stage = Stage::Begin;
        if self.witness == self.judgment.witnesses.len() {
            self.terminal = Some(RefinementOutcome::StillValid);
        }
    }

    fn needs_exact(&self) -> bool {
        self.candidates.as_ref().is_none_or(|mask| mask[self.witness])
    }

    fn step(&mut self, bytes: usize) -> Result<(), Error> {
        match self.stage {
            Stage::Basis => {
                let old = self.judgment;
                let new = self.snapshot;
                if new.revision < old.revision || new.control_cut < old.control_cut {
                    return Err(Error::Stale);
                }
                let domain = new.domain_input.domain;
                let reason = if new.semantic_epoch != old.semantic_epoch {
                    Some(Invalidation::SemanticEpoch)
                } else if domain.domain_id != old.domain.domain_id {
                    Some(Invalidation::DomainIdentity)
                } else if domain.domain_epoch != old.domain.domain_epoch {
                    Some(Invalidation::DomainEpoch)
                } else if domain.projection != old.domain.projection {
                    Some(Invalidation::Projection)
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.invalidate(reason, true);
                } else if old.witnesses.is_empty() {
                    self.terminal = Some(RefinementOutcome::StillValid);
                } else {
                    self.stage = Stage::Begin;
                }
            }
            Stage::Begin => {
                if matches!(self.judgment.witnesses[self.witness], Witness::ExactValue { .. })
                    && !self.needs_exact()
                {
                    self.next_witness();
                    return Ok(());
                }
                self.stage = match &self.judgment.witnesses[self.witness] {
                    Witness::ExactValue { .. } => Stage::Exact,
                    _ => Stage::Frontier,
                };
            }
            Stage::Frontier => {
                let dependency = &self.judgment.witnesses[self.witness];
                let marker = match dependency {
                    Witness::AbsentKey { marker, .. }
                    | Witness::EmptyRange { marker, .. }
                    | Witness::RangeMembers { marker, .. } => *marker,
                    Witness::ExactValue { .. } => unreachable!("exact values need no closure"),
                };
                if closed_marker(self.snapshot, self.frontiers)? != marker {
                    self.invalidate(Invalidation::ClosingFrontier, false);
                } else if !self.needs_exact() {
                    // Even disjoint negative dependencies require current closure.
                    self.next_witness();
                } else {
                    match dependency {
                        Witness::AbsentKey { .. } => self.stage = Stage::Absent,
                        Witness::EmptyRange { start, end, .. }
                        | Witness::RangeMembers { start, end, .. } => {
                            self.range = Some(self.snapshot.values.range(*start..*end));
                            self.stage = Stage::Range;
                        }
                        Witness::ExactValue { .. } => unreachable!("exact value frontier"),
                    }
                }
            }
            Stage::Exact => {
                let Witness::ExactValue { key, version, value, .. } =
                    &self.judgment.witnesses[self.witness]
                else { unreachable!("exact value step") };
                let Some(actual) = self.snapshot.entry(*key) else {
                    self.invalidate(Invalidation::ExactValue, false);
                    return Ok(());
                };
                if actual.version != *version || actual.value.len() != value.len() {
                    self.invalidate(Invalidation::ExactValue, false);
                } else if value.is_empty() {
                    self.next_witness();
                } else {
                    self.stage = Stage::Compare {
                        expected: value, actual: &actual.value, offset: 0, in_range: false,
                    };
                }
            }
            Stage::Absent => {
                let Witness::AbsentKey { key, .. } = &self.judgment.witnesses[self.witness]
                else { unreachable!("absent key step") };
                if self.snapshot.entry(*key).is_some() {
                    self.invalidate(Invalidation::AbsentKey, false);
                } else {
                    self.next_witness();
                }
            }
            Stage::Range => {
                let actual = self.range.as_mut().expect("initialized range").next();
                match &self.judgment.witnesses[self.witness] {
                    Witness::EmptyRange { .. } => {
                        if actual.is_some() {
                            self.invalidate(Invalidation::EmptyRange, false);
                        } else {
                            self.next_witness();
                        }
                    }
                    Witness::RangeMembers { members, .. } => {
                        match (members.get(self.member), actual) {
                            (None, None) => self.next_witness(),
                            (Some(expected), Some((_, actual)))
                                if expected.key == actual.key
                                    && expected.version == actual.version
                                    && expected.value.len() == actual.value.len() =>
                            {
                                if expected.value.is_empty() {
                                    self.member += 1;
                                } else {
                                    self.stage = Stage::Compare {
                                        expected: &expected.value, actual: &actual.value,
                                        offset: 0, in_range: true,
                                    };
                                }
                            }
                            _ => self.invalidate(Invalidation::RangeMembers, false),
                        }
                    }
                    _ => unreachable!("range witness step"),
                }
            }
            Stage::Compare { expected, actual, offset, in_range } => {
                let end = offset + bytes;
                if expected[offset..end] != actual[offset..end] {
                    self.invalidate(if in_range {
                        Invalidation::RangeMembers
                    } else {
                        Invalidation::ExactValue
                    }, false);
                } else if end == expected.len() {
                    if in_range {
                        self.member += 1;
                        self.stage = Stage::Range;
                    } else {
                        self.next_witness();
                    }
                } else {
                    self.stage = Stage::Compare { expected, actual, offset: end, in_range };
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
