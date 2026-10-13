use super::*;

// Fixed logical record widths include the key, owner slot and search metadata.
// Charges occur BEFORE inspecting a record. They are architecture-independent.
const REGISTRATION_BYTES: u64 = 80;
const POINT_BYTES: u64 = 48;
pub(super) const INTERVAL_BYTES: u64 = 64;
pub(super) struct Meter { left: RoutingBudget, spent: RoutingBudget }
impl Meter {
    pub(super) fn new(left: RoutingBudget) -> Self { Self { left, spent: RoutingBudget::default() } }
    pub(super) fn spent(&self) -> RoutingBudget { self.spent }
    pub(super) fn charge(&mut self, bytes: u64) -> Result<(), Error> {
        if self.left.steps == 0 || self.left.bytes < bytes { return Err(Error::Incomplete); }
        self.left.steps -= 1;
        self.left.bytes -= bytes;
        self.spent.steps += 1;
        self.spent.bytes += bytes;
        Ok(())
    }
}
impl InvalidationIndex {
    /// Bound one notification batch before traversing any input records. This
    /// accommodates the full before/after key union of two bounded snapshots.
    pub const MAX_BATCH_CHANGES: usize = 2 * crate::witness::MAX_SNAPSHOT_ENTRIES;

    /// Route one atomic change set with a SINGLE step/byte allowance. The
    /// result is the union in registration order, with each owner emitted once.
    /// No prefix result escapes a malformed change or exhausted budget; callers
    /// must withdraw everything or perform complete exact fallback on error.
    ///
    /// Every input, including duplicates and inputs after `All`, is validated
    /// and charged in supplied order. Output is charged only once. Thus input
    /// order and strategy can affect cost, but never the completed candidate
    /// set. An empty batch has no candidates, including opaque registrations.
    ///
    /// This is notification routing, not proof of validity or an authenticated
    /// transaction. It cannot install the parent's complete-tail skipping mask.
    pub fn affected_batch(&self, changes: &[WitnessChange], budget: RoutingBudget) -> RoutingReport {
        let mut meter = Meter::new(budget);
        let candidates = self.lookup_batch(changes, &mut meter);
        RoutingReport { candidates, spent: meter.spent() }
    }

    fn lookup_batch(&self, changes: &[WitnessChange], meter: &mut Meter) -> Result<Vec<u64>, Error> {
        if changes.len() > Self::MAX_BATCH_CHANGES { return Err(Error::Limit); }
        if changes.is_empty() { return Ok(Vec::new()); }
        let mut affected = [false; MAX_ROUTED_JUDGMENTS];
        for &change in changes {
            self.select(change, &mut affected, meter)?;
        }
        self.collect_ids(&affected, meter)
    }

    pub(super) fn lookup(&self, change: WitnessChange, meter: &mut Meter) -> Result<Vec<u64>, Error> {
        let mut affected = [false; MAX_ROUTED_JUDGMENTS];
        self.select(change, &mut affected, meter)?;
        self.collect_ids(&affected, meter)
    }

    fn select(&self, change: WitnessChange, affected: &mut [bool; MAX_ROUTED_JUDGMENTS],
        meter: &mut Meter) -> Result<(), Error>
    {
        meter.charge(72)?;
        change.validate()?;
        let domain = change.domain();
        for (slot, entry) in self.registrations.iter().enumerate() {
            meter.charge(REGISTRATION_BYTES)?;
            affected[slot] |= entry.opaque || matches!(change, WitnessChange::All)
                || entry.domain.zip(domain).is_some_and(|(old, new)| {
                    DomainKey::from(old) == DomainKey::from(new)
                        && (old != new || matches!(change, WitnessChange::Domain { .. }))
                });
        }
        match change {
            WitnessChange::Key { domain, key } => {
                self.points_in(DomainKey::from(domain), key, None, affected, meter)?;
                self.intervals_in(DomainKey::from(domain), key, None, affected, meter)?;
            }
            WitnessChange::Range { domain, start, end } => {
                self.points_in(DomainKey::from(domain), start, Some(end), affected, meter)?;
                self.intervals_in(DomainKey::from(domain), start, Some(end), affected, meter)?;
            }
            WitnessChange::Domain { .. } | WitnessChange::All => {}
        }
        Ok(())
    }

    fn collect_ids(&self, affected: &[bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter) -> Result<Vec<u64>, Error> {
        let mut ids = Vec::new();
        ids.try_reserve_exact(self.registered()).map_err(|_| Error::Limit)?;
        // Stable registration order, no duplicate IDs despite overlapping ranges.
        for (slot, entry) in self.registrations.iter().enumerate() {
            meter.charge(REGISTRATION_BYTES)?;
            if affected[slot] { meter.charge(8)?; ids.push(entry.id); }
        }
        Ok(ids)
    }

    fn points_in(&self, domain: DomainKey, low: u64, high: Option<u64>,
        affected: &mut [bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter) -> Result<(), Error>
    {
        let (mut lo, mut hi) = (0, self.points.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            meter.charge(POINT_BYTES)?;
            let point = &self.points[mid];
            if (point.domain, point.key) < (domain, low) { lo = mid + 1; } else { hi = mid; }
        }
        for point in &self.points[lo..] {
            meter.charge(POINT_BYTES)?;
            if point.domain != domain || high.map_or(point.key != low, |end| point.key >= end) { break; }
            affected[point.slot] = true;
        }
        Ok(())
    }

    fn intervals_in(&self, domain: DomainKey, low: u64, high: Option<u64>,
        affected: &mut [bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter) -> Result<(), Error>
    {
        if self.strategy == RoutingStrategy::SubtreeV2 {
            return subtree::lookup(self, domain, low, high, affected, meter);
        }
        let (mut lo, mut hi) = (0, self.intervals.len());
        // Upper bound on start: <= key for a point, < end for a half-open range.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            meter.charge(INTERVAL_BYTES)?;
            let range = &self.intervals[mid];
            let before = range.domain < domain || (range.domain == domain
                && high.map_or(range.start <= low, |end| range.start < end));
            if before { lo = mid + 1; } else { hi = mid; }
        }
        while lo > 0 {
            lo -= 1;
            meter.charge(INTERVAL_BYTES)?;
            let range = &self.intervals[lo];
            // All earlier starts are already small enough. The maximum end
            // proves disjointness of this entire remaining per-domain prefix.
            if range.domain != domain || range.prefix_end <= low { break; }
            if range.end > low { affected[range.slot] = true; }
        }
        Ok(())
    }
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use crate::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
    use crate::witness::{AdapterDomainInput, DomainClosure, QueryRole, SnapshotEntry, WitnessRequest, WitnessSnapshot};

    fn domain(source: u64) -> DomainProjection {
        DomainProjection::new(3, 4, ProjectionKey { source, branch: 5, projection: 6, source_epoch: 7 })
    }
    fn unlimited() -> RoutingBudget { RoutingBudget { steps: u64::MAX, bytes: u64::MAX } }
    fn captured(d: DomainProjection, request: Option<WitnessRequest>) -> WitnessJudgment {
        let marker = TrustedClosingMarker { key: d.projection(), final_sequence: 1, marker_generation: 1 };
        let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
        frontiers.accept(marker.key, FrontierStage::Authenticated, 1).unwrap();
        frontiers.record_close(marker).unwrap();
        let entries = match request {
            Some(WitnessRequest::ExactValue { key, .. }) => vec![SnapshotEntry::new(key, 1, b"basis".to_vec()).unwrap()],
            _ => vec![],
        };
        let snapshot = WitnessSnapshot::new(1, 1, 1,
            AdapterDomainInput::new(d, DomainClosure::Closed(marker)), entries).unwrap();
        WitnessJudgment::capture(&snapshot, &frontiers, request.into_iter().collect()).unwrap()
    }
    fn fixture(strategy: RoutingStrategy) -> (InvalidationIndex, Vec<(u64, WitnessJudgment, bool)>) {
        let mut index = InvalidationIndex::new_with_strategy(
            RoutingLimits { judgments: MAX_ROUTED_JUDGMENTS, dependencies: MAX_ROUTED_DEPENDENCIES }, strategy).unwrap();
        let original = vec![
            (50, captured(domain(1), Some(WitnessRequest::ExactValue { key: 3, role: QueryRole::PolicyInput })), false),
            (7, captured(domain(1), Some(WitnessRequest::AbsentKey { key: 10 })), false),
            (88, captured(domain(1), Some(WitnessRequest::EmptyRange { start: 8, end: 13 })), false),
            (2, captured(domain(2), Some(WitnessRequest::AbsentKey { key: 1 })), false),
            (99, captured(domain(2), None), true),
            (5, captured(domain(1), Some(WitnessRequest::RangeMembers { start: 3, end: 8 })), true),
            (3, captured(domain(1), None), false),
            (21, captured(domain(1), Some(WitnessRequest::AbsentKey { key: u64::MAX })), false),
        ];
        for (id, judgment, opaque) in &original { index.register(*id, Some(judgment), *opaque).unwrap(); }
        index.register(44, None, true).unwrap();
        (index, original)
    }

    // Independent scan of the captured witnesses, not postings, subtree
    // summaries, the production selector, or a union of production answers.
    fn scan(judgment: &WitnessJudgment, opaque: bool, change: WitnessChange) -> bool {
        if opaque || matches!(change, WitnessChange::All) { return true; }
        let new = match change {
            WitnessChange::Key { domain, .. } | WitnessChange::Range { domain, .. }
            | WitnessChange::Domain { domain } => domain,
            WitnessChange::All => unreachable!(),
        };
        let old = judgment.domain;
        let a = old.projection(); let b = new.projection();
        if old.domain_id() != new.domain_id() || a.source != b.source
            || a.branch != b.branch || a.projection != b.projection { return false; }
        if old != new || matches!(change, WitnessChange::Domain { .. }) { return true; }
        judgment.witnesses.iter().any(|witness| match (witness, change) {
            (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Key { key: changed, .. }) => *key == changed,
            (Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. }, WitnessChange::Range { start, end, .. }) => start <= *key && *key < end,
            (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Key { key, .. }) => *start <= key && key < *end,
            (Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. }, WitnessChange::Range { start: low, end: high, .. }) => *start < high && low < *end,
            _ => unreachable!(),
        })
    }

    #[test]
    fn batches_match_an_independent_original_witness_scan_for_both_strategies() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (index, original) = fixture(strategy);
            let mut changes = vec![WitnessChange::All, WitnessChange::Domain { domain: domain(1) },
                WitnessChange::Key { domain: domain(1), key: u64::MAX }];
            for source in 1..=3 {
                for key in 0..15 {
                    changes.push(WitnessChange::Key { domain: domain(source), key });
                    changes.push(WitnessChange::Range { domain: domain(source), start: key, end: key + 2 });
                }
            }
            for &a in &changes {
                for &b in &changes {
                    let batch = [a, b, a];
                    let mut expected: Vec<_> = original.iter().filter(|(_, j, opaque)| {
                        batch.iter().any(|change| scan(j, *opaque, *change))
                    }).map(|(id, _, _)| *id).collect();
                    expected.push(44);
                    assert_eq!(index.affected_batch(&batch, unlimited()).candidates.unwrap(), expected,
                        "{strategy:?}: {batch:?}");
                }
            }
        }
    }

    #[test]
    fn shared_budget_charges_output_once_and_never_returns_a_prefix() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (index, _) = fixture(strategy);
            let changes = [WitnessChange::Key { domain: domain(1), key: 10 },
                WitnessChange::Key { domain: domain(2), key: 1 },
                WitnessChange::Key { domain: domain(1), key: 3 }];
            let full = index.affected_batch(&changes, unlimited());
            assert_eq!(full.candidates, Ok(vec![50, 7, 88, 2, 99, 5, 44]));
            assert_eq!(index.affected_batch(&changes, full.spent), full);
            let individual: Vec<_> = changes.iter().map(|change| index.affected(*change, unlimited())).collect();
            assert!(full.spent.steps < individual.iter().map(|r| r.spent.steps).sum::<u64>());
            assert!(full.spent.bytes < individual.iter().map(|r| r.spent.bytes).sum::<u64>());
            for steps in 0..full.spent.steps {
                let budget = RoutingBudget { steps, bytes: full.spent.bytes };
                let refused = index.affected_batch(&changes, budget);
                assert_eq!(refused.candidates, Err(Error::Incomplete));
                assert!(refused.spent.steps <= budget.steps && refused.spent.bytes <= budget.bytes);
            }
            let budget = RoutingBudget { steps: full.spent.steps, bytes: full.spent.bytes - 1 };
            let refused = index.affected_batch(&changes, budget);
            assert_eq!(refused.candidates, Err(Error::Incomplete));
            assert!(refused.spent.bytes <= budget.bytes);
            assert_eq!(index.affected_batch(&changes, unlimited()), full);
            assert_eq!(index.registered(), 9);
        }
    }

    #[test]
    fn malformed_suffix_after_all_is_not_hidden_by_a_complete_prefix() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (index, _) = fixture(strategy);
            let valid = index.affected(WitnessChange::All, unlimited());
            for (start, end) in [(3, 3), (4, 3), (u64::MAX, 0)] {
                let report = index.affected_batch(&[WitnessChange::All,
                    WitnessChange::Range { domain: domain(1), start, end }], unlimited());
                assert_eq!(report.candidates, Err(Error::InvalidInput));
                assert!(report.spent.steps > 1 && report.spent.bytes >= 144);
            }
            assert_eq!(index.affected(WitnessChange::All, unlimited()), valid);
        }
    }

    #[test]
    fn empty_oversized_and_singleton_batches_have_explicit_boundaries() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (index, _) = fixture(strategy);
            assert_eq!(index.affected_batch(&[], RoutingBudget::default()),
                RoutingReport { candidates: Ok(vec![]), spent: RoutingBudget::default() });
            assert_eq!(index.affected_batch(&vec![WitnessChange::All; InvalidationIndex::MAX_BATCH_CHANGES + 1], unlimited()),
                RoutingReport { candidates: Err(Error::Limit), spent: RoutingBudget::default() });
            let all = index.affected(WitnessChange::All, unlimited());
            assert_eq!(index.affected_batch(&[WitnessChange::All], unlimited()), all);
            let maximum = index.affected_batch(&vec![WitnessChange::All; InvalidationIndex::MAX_BATCH_CHANGES], unlimited());
            assert_eq!(maximum.candidates, all.candidates);
            assert!(maximum.spent.steps > all.spent.steps);
        }
    }

    #[test]
    fn epoch_changes_reach_empty_footprints_and_half_open_keys_stay_exact() {
        for strategy in [RoutingStrategy::PrefixV1, RoutingStrategy::SubtreeV2] {
            let (index, _) = fixture(strategy);
            let d = domain(1);
            for changed in [DomainProjection::new(d.domain_id(), 5, d.projection()),
                DomainProjection::new(d.domain_id(), d.domain_epoch(), ProjectionKey { source_epoch: 8, ..d.projection() })] {
                assert_eq!(index.affected_batch(&[WitnessChange::Key { domain: changed, key: 100 }], unlimited()).candidates,
                    Ok(vec![50, 7, 88, 99, 5, 3, 21, 44]));
            }
            assert_eq!(index.affected_batch(&[WitnessChange::Range { domain: d, start: 13, end: u64::MAX }], unlimited()).candidates,
                Ok(vec![99, 5, 44]));
            assert_eq!(index.affected_batch(&[WitnessChange::Key { domain: d, key: u64::MAX }], unlimited()).candidates,
                Ok(vec![99, 5, 21, 44]));
        }
    }
}
