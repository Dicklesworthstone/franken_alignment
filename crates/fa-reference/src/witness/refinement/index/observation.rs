//! Cooperative construction of the complete change tail. No partial delta is
//! visible to candidate selection: only an explicitly committed, complete scan
//! replaces the indexed snapshot. Dropping a scan leaves the history unchanged.

use super::{
    ChangeCost, Delta, Error, ProductFrontiers, WitnessChangeIndex, WitnessSnapshot, closed_marker,
};
use crate::witness::{SnapshotEntry, refinement::RefinementBudget};
use super::routing::{InvalidationIndex, RoutingBudget, RoutingReport, WitnessChange};
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

/// Result of routing a complete observation before publishing its change tail.
/// `committed` is the publication result; `observation.outcome == Ready` alone
/// still means only that scanning finished. Errors retain both work reports and
/// expose no candidate prefix. This report is neither a permit nor proof that
/// an external consumer has withdrawn the returned observations.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "check publication and apply the complete notification or fail closed"]
pub struct RoutedChangeCommit {
    pub committed: Result<ChangeCost, Error>,
    pub observation: ChangeObservationReport,
    pub routing: RoutingReport,
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

    /// Derive notifications from this ORIGINAL completed snapshot comparison,
    /// route the entire batch, then publish through the ordinary commit path.
    /// No unchecked changed-key list is accepted. A routing failure leaves the
    /// current snapshot, history floor and all retained deltas unchanged.
    ///
    /// A different closing marker needs a domain notification even when no key
    /// changed. An empty delta also routes the domain conservatively: equality
    /// of structured values does not prove an opaque helper input stayed equal.
    /// With a nonempty delta and unchanged closure, only changed keys are routed.
    ///
    /// Scan work is retained separately from the shared routing allowance.
    /// Materializing at most MAX_BATCH_CHANGES value-free notification records,
    /// allocation and commit housekeeping are bounded but not charged as routing
    /// lookup work. No hard elapsed-time or automatic withdrawal claim is made.
    pub fn commit_routed(mut self, routes: &InvalidationIndex, budget: RoutingBudget) -> RoutedChangeCommit {
        let observation = self.advance(RefinementBudget::default());
        let refusal = match observation.outcome {
            ChangeObservationOutcome::Ready => None,
            ChangeObservationOutcome::Refused(error) => Some(error),
            _ => Some(Error::Incomplete),
        };
        if let Some(error) = refusal {
            return RoutedChangeCommit { committed: Err(error), observation,
                routing: RoutingReport { candidates: Err(error), spent: RoutingBudget::default() } };
        }
        let domain = self.next.domain_input.domain;
        let whole_domain = self.keys.is_empty()
            || self.before.domain_input.closure != self.next.domain_input.closure;
        let count = if whole_domain { 1 } else { self.keys.len() };
        let mut changes = Vec::new();
        if count > InvalidationIndex::MAX_BATCH_CHANGES || changes.try_reserve_exact(count).is_err() {
            return RoutedChangeCommit { committed: Err(Error::Limit), observation,
                routing: RoutingReport { candidates: Err(Error::Limit), spent: RoutingBudget::default() } };
        }
        if whole_domain {
            changes.push(WitnessChange::Domain { domain });
        } else {
            changes.extend(self.keys.iter().map(|key| WitnessChange::Key { domain, key: *key }));
        }
        let mut routing = routes.affected_batch(&changes, budget);
        if let Err(error) = routing.candidates.as_ref() {
            return RoutedChangeCommit { committed: Err(*error), observation, routing };
        }
        let committed = self.commit();
        if let Err(error) = committed {
            // Candidates must not be mistaken for a successfully published cut.
            routing.candidates = Err(error);
        }
        RoutedChangeCommit { committed, observation, routing }
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

#[cfg(test)]
mod routed_tests {
    use super::*;
    use crate::product_frontier::{FrontierStage, ProjectionKey, TrustedClosingMarker};
    use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, Invalidation,
        QueryRole, Reuse, WitnessJudgment, WitnessRequest, MAX_SNAPSHOT_ENTRIES};
    use crate::witness::refinement::{RefinementOutcome, index::IndexSelection};
    use crate::witness::refinement::index::routing::{RoutingLimits, RoutingStrategy};

    fn scan_budget() -> RefinementBudget { RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX } }
    fn route_budget() -> RoutingBudget { RoutingBudget { steps: u64::MAX, bytes: u64::MAX } }
    fn basis(revision: u64, generation: u64, entries: Vec<SnapshotEntry>) -> (WitnessSnapshot, ProductFrontiers) {
        let key = ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 };
        let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: generation };
        let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
        frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
        frontiers.record_close(marker).unwrap();
        let domain = DomainProjection::new(5, 6, key);
        (WitnessSnapshot::new(revision, revision, 7,
            AdapterDomainInput::new(domain, DomainClosure::Closed(marker)), entries).unwrap(), frontiers)
    }
    fn value(key: u64, bytes: &[u8]) -> SnapshotEntry {
        SnapshotEntry::new(key, 1, bytes.to_vec()).unwrap()
    }
    fn routes(strategy: RoutingStrategy) -> InvalidationIndex {
        InvalidationIndex::new_with_strategy(RoutingLimits { judgments: 8, dependencies: 8 }, strategy).unwrap()
    }
    fn captured(snapshot: &WitnessSnapshot, frontiers: &ProductFrontiers, request: WitnessRequest) -> WitnessJudgment {
        WitnessJudgment::capture(snapshot, frontiers, vec![request]).unwrap()
    }

    #[test]
    fn routed_commit_uses_the_original_delta_and_preserves_exact_refinement() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (old, old_frontiers) = basis(1, 1, vec![value(2, b"old"), value(8, b"keep")]);
            let (next, next_frontiers) = basis(2, 1, vec![value(2, b"new"), value(3, b"added"), value(8, b"keep")]);
            let mut index = WitnessChangeIndex::new(&old, &old_frontiers, 2).unwrap();
            let changed = index.capture(&old_frontiers,
                vec![WitnessRequest::ExactValue { key: 2, role: QueryRole::PolicyInput }]).unwrap();
            let absent = index.capture(&old_frontiers, vec![WitnessRequest::AbsentKey { key: 3 }]).unwrap();
            let unchanged = index.capture(&old_frontiers,
                vec![WitnessRequest::RangeMembers { start: 8, end: 9 }]).unwrap();
            let mut routing = routes(strategy);
            routing.register(20, Some(changed.judgment()), false).unwrap();
            routing.register(30, Some(absent.judgment()), false).unwrap();
            routing.register(80, Some(unchanged.judgment()), false).unwrap();
            routing.register(7, None, true).unwrap();
            let mut pending = index.begin_observe(&next, &next_frontiers);
            let scanned = pending.advance(scan_budget());
            assert_eq!(scanned.outcome, ChangeObservationOutcome::Ready);
            let report = pending.commit_routed(&routing, route_budget());
            assert_eq!(report.committed, Ok(scanned.cost));
            assert_eq!(report.observation.total, scanned.total);
            assert_eq!(report.observation.spent, RefinementBudget::default());
            assert_eq!(report.routing.candidates, Ok(vec![20, 30, 7]));
            assert_eq!(index.current_revision(), 2);
            let mut ordinary = WitnessChangeIndex::new(&old, &old_frontiers, 2).unwrap();
            assert_eq!(ordinary.observe(&next, &next_frontiers), report.committed);
            let (selection, mut cursor) = index.begin_refinement(&unchanged, &next, &next_frontiers, u64::MAX);
            assert_eq!(selection.selection, IndexSelection::Candidates { count: 0 });
            assert_eq!(cursor.advance(scan_budget()).outcome, RefinementOutcome::StillValid);
            for judgment in [&changed, &absent] {
                let (_, mut cursor) = index.begin_refinement(judgment, &next, &next_frontiers, u64::MAX);
                assert!(matches!(cursor.advance(scan_budget()).outcome, RefinementOutcome::Invalidated { .. }));
            }
        }
    }

    #[test]
    fn routing_exhaustion_does_not_publish_or_evict_and_preserves_scan_work() {
        let (one, f1) = basis(1, 1, vec![value(1, b"old")]);
        let (two, f2) = basis(2, 1, vec![value(1, b"old")]);
        let (three, f3) = basis(3, 1, vec![value(1, b"new")]);
        let mut index = WitnessChangeIndex::new(&one, &f1, 1).unwrap();
        index.observe(&two, &f2).unwrap();
        let mut routing = routes(RoutingStrategy::SubtreeV2);
        routing.register(9, None, true).unwrap();
        let mut pending = index.begin_observe(&three, &f3);
        let scanned = loop {
            let report = pending.advance(RefinementBudget { steps: 1, value_bytes: 1 });
            match report.outcome {
                ChangeObservationOutcome::Ready => break report,
                ChangeObservationOutcome::NeedsWork { .. } => {}
                other => panic!("unexpected scan: {other:?}"),
            }
        };
        let rejected = pending.commit_routed(&routing, RoutingBudget::default());
        assert_eq!(rejected.committed, Err(Error::Incomplete));
        assert_eq!(rejected.routing.candidates, Err(Error::Incomplete));
        assert_eq!(rejected.routing.spent, RoutingBudget::default());
        assert_eq!(rejected.observation.total, scanned.total);
        assert!(rejected.observation.total.steps > 0 && rejected.observation.total.value_bytes > 0);
        assert_eq!(index.current_revision(), 2);
        assert_eq!(index.oldest_covered_revision(), 1);
        assert_eq!(index.retained_revisions(), 1);
        let mut retry = index.begin_observe(&three, &f3);
        assert_eq!(retry.advance(scan_budget()).outcome, ChangeObservationOutcome::Ready);
        let report = retry.commit_routed(&routing, route_budget());
        assert!(report.committed.is_ok());
        assert_eq!(report.routing.candidates, Ok(vec![9]));
        assert_eq!(index.current_revision(), 3);
        assert_eq!(index.oldest_covered_revision(), 2);
    }

    #[test]
    fn unfinished_and_refused_scans_cannot_route_or_advance_history() {
        let (old, frontiers) = basis(1, 1, vec![]);
        let (next, nf) = basis(2, 1, vec![value(3, b"new")]);
        let (skipped, sf) = basis(4, 1, vec![]);
        let mut index = WitnessChangeIndex::new(&old, &frontiers, 1).unwrap();
        let routing = routes(RoutingStrategy::PrefixV1);
        let pending = index.begin_observe(&next, &nf);
        let incomplete = pending.commit_routed(&routing, route_budget());
        assert_eq!(incomplete.committed, Err(Error::Incomplete));
        assert!(matches!(incomplete.observation.outcome, ChangeObservationOutcome::NeedsWork { .. }));
        assert_eq!(incomplete.routing.spent, RoutingBudget::default());
        let mut invalid = index.begin_observe(&skipped, &sf);
        assert_eq!(invalid.advance(scan_budget()).outcome, ChangeObservationOutcome::Refused(Error::Stale));
        let refused = invalid.commit_routed(&routing, route_budget());
        assert_eq!(refused.committed, Err(Error::Stale));
        assert_eq!(refused.routing.candidates, Err(Error::Stale));
        assert_eq!(refused.observation.total.steps, 1);
        assert_eq!(index.current_revision(), 1);
        assert_eq!(index.retained_revisions(), 0);
    }

    #[test]
    fn closure_only_and_empty_deltas_cannot_hide_negative_or_opaque_dependencies() {
        for generation in [1, 2] {
            let (old, frontiers) = basis(1, 1, vec![value(2, b"same")]);
            let (next, nf) = basis(2, generation, vec![value(2, b"same")]);
            let negative = captured(&old, &frontiers, WitnessRequest::AbsentKey { key: 3 });
            let exact = captured(&old, &frontiers, WitnessRequest::ExactValue { key: 2, role: QueryRole::Subject });
            let mut index = WitnessChangeIndex::new(&old, &frontiers, 1).unwrap();
            let mut routing = routes(RoutingStrategy::SubtreeV2);
            routing.register(1, Some(&negative), false).unwrap();
            routing.register(2, Some(&exact), false).unwrap();
            routing.register(3, None, true).unwrap();
            let mut pending = index.begin_observe(&next, &nf);
            assert_eq!(pending.advance(scan_budget()).cost.changed_keys, 0);
            let report = pending.commit_routed(&routing, route_budget());
            assert!(report.committed.is_ok());
            assert_eq!(report.routing.candidates, Ok(vec![1, 2, 3]));
            assert!(matches!(exact.reuse_at(&next, &nf).unwrap(), Reuse::StillValid { .. }));
            if generation == 2 {
                assert!(matches!(negative.reuse_at(&next, &nf).unwrap(),
                    Reuse::Invalidated { reason: Invalidation::ClosingFrontier, .. }));
            } else {
                assert!(matches!(negative.reuse_at(&next, &nf).unwrap(), Reuse::StillValid { .. }));
            }
        }
    }

    #[test]
    fn maximum_disjoint_snapshots_route_every_derived_key_without_truncation() {
        let size = MAX_SNAPSHOT_ENTRIES as u64;
        let (old, f1) = basis(1, 1, (0..size).map(|key| value(key, b"")).collect());
        let (next, f2) = basis(2, 1, (size..size * 2).map(|key| value(key, b"")).collect());
        let old_key = captured(&old, &f1, WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject });
        let new_key = captured(&old, &f1, WitnessRequest::AbsentKey { key: size * 2 - 1 });
        let mut routing = routes(RoutingStrategy::PrefixV1);
        routing.register(1, Some(&old_key), false).unwrap();
        routing.register(2, Some(&new_key), false).unwrap();
        let mut index = WitnessChangeIndex::new(&old, &f1, 1).unwrap();
        let mut pending = index.begin_observe(&next, &f2);
        let scanned = pending.advance(scan_budget());
        assert_eq!(scanned.outcome, ChangeObservationOutcome::Ready);
        assert_eq!(scanned.cost.changed_keys, InvalidationIndex::MAX_BATCH_CHANGES);
        let report = pending.commit_routed(&routing, route_budget());
        assert_eq!(report.committed, Ok(scanned.cost));
        assert_eq!(report.routing.candidates, Ok(vec![1, 2]));
        assert_eq!(index.current_revision(), 2);
    }
}
