//! Exact-value and absent-key judgment witnesses for the bounded reference model.
//!
//! `AdapterDomainInput` is a caller-supplied assertion about an adapter-authenticated
//! domain. This module verifies only its identity, epoch, and declared product
//! frontier; it neither authenticates an adapter nor turns an arbitrary scalar
//! into authenticated evidence. A missing closure is therefore `Unknown`, not
//! evidence that a key is absent.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Error,
    product_frontier::{
        FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    },
};

pub const MAX_SNAPSHOT_ENTRIES: usize = 256;
pub const MAX_VALUE_BYTES: usize = 8 * 1024;
pub const MAX_WITNESSES: usize = 64;

/// The role a deterministic query assigned to an exact value dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueryRole {
    Subject,
    PolicyInput,
    PredicateInput,
}

/// A logical key domain and its interpretation epoch in one exact projection.
///
/// This is an identity binding, not an authentication implementation. Zero is
/// a valid initial ID or epoch in the reference profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainProjection {
    domain_id: u64,
    domain_epoch: u64,
    projection: ProjectionKey,
}

impl DomainProjection {
    #[must_use]
    pub const fn new(domain_id: u64, domain_epoch: u64, projection: ProjectionKey) -> Self {
        Self {
            domain_id,
            domain_epoch,
            projection,
        }
    }

    #[must_use]
    pub const fn domain_id(self) -> u64 {
        self.domain_id
    }

    #[must_use]
    pub const fn domain_epoch(self) -> u64 {
        self.domain_epoch
    }

    #[must_use]
    pub const fn projection(self) -> ProjectionKey {
        self.projection
    }
}

/// A domain closure supplied by an adapter boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainClosure {
    /// The adapter supplied no complete closed-domain observation.
    Unknown,
    /// The complete-domain claim is tied to this exact caller-supplied marker.
    Closed(TrustedClosingMarker),
}

/// Caller input that an outer adapter has authenticated a domain snapshot.
///
/// The name records a precondition for a future adapter integration. This
/// reference model has no credentials, signatures, or origin verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterDomainInput {
    domain: DomainProjection,
    closure: DomainClosure,
}

impl AdapterDomainInput {
    #[must_use]
    pub const fn new(domain: DomainProjection, closure: DomainClosure) -> Self {
        Self { domain, closure }
    }

    #[must_use]
    pub const fn domain(self) -> DomainProjection {
        self.domain
    }

    #[must_use]
    pub const fn closure(self) -> DomainClosure {
        self.closure
    }
}

/// One versioned logical value observed at a control cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotEntry {
    key: u64,
    version: u64,
    value: Vec<u8>,
}

impl SnapshotEntry {
    pub fn new(key: u64, version: u64, value: Vec<u8>) -> Result<Self, Error> {
        if value.len() > MAX_VALUE_BYTES {
            return Err(Error::Limit);
        }
        Ok(Self {
            key,
            version,
            value,
        })
    }

    #[must_use]
    pub const fn key(&self) -> u64 {
        self.key
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

/// A bounded adapter-provided view at one monotonic revision and control cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessSnapshot {
    revision: u64,
    control_cut: u64,
    semantic_epoch: u64,
    domain_input: AdapterDomainInput,
    values: BTreeMap<u64, SnapshotEntry>,
}

impl WitnessSnapshot {
    pub fn new(
        revision: u64,
        control_cut: u64,
        semantic_epoch: u64,
        domain_input: AdapterDomainInput,
        entries: Vec<SnapshotEntry>,
    ) -> Result<Self, Error> {
        if entries.len() > MAX_SNAPSHOT_ENTRIES {
            return Err(Error::Limit);
        }
        if let DomainClosure::Closed(marker) = domain_input.closure
            && (marker.key != domain_input.domain.projection || marker.marker_generation == 0)
        {
            return Err(Error::Binding);
        }
        let mut values = BTreeMap::new();
        for entry in entries {
            if values.insert(entry.key, entry).is_some() {
                return Err(Error::Duplicate);
            }
        }
        Ok(Self {
            revision,
            control_cut,
            semantic_epoch,
            domain_input,
            values,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn control_cut(&self) -> u64 {
        self.control_cut
    }

    #[must_use]
    pub const fn semantic_epoch(&self) -> u64 {
        self.semantic_epoch
    }

    #[must_use]
    pub const fn domain_input(&self) -> AdapterDomainInput {
        self.domain_input
    }

    #[must_use]
    pub fn entry(&self, key: u64) -> Option<&SnapshotEntry> {
        self.values.get(&key)
    }
}

/// A requested dependency before capture binds it to an actual snapshot value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessRequest {
    ExactValue { key: u64, role: QueryRole },
    AbsentKey { key: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Witness {
    ExactValue {
        key: u64,
        version: u64,
        value: Vec<u8>,
        role: QueryRole,
    },
    AbsentKey {
        key: u64,
        marker: TrustedClosingMarker,
    },
}

/// A historical result whose declared exact dependencies may be reused only
/// after current validation. It is not a permit and cannot authorize dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessJudgment {
    revision: u64,
    control_cut: u64,
    semantic_epoch: u64,
    domain: DomainProjection,
    witnesses: Vec<Witness>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalidation {
    SemanticEpoch,
    DomainIdentity,
    DomainEpoch,
    Projection,
    ExactValue,
    AbsentKey,
    ClosingFrontier,
}

/// Bounded deterministic work performed by one reuse validation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationCost {
    exact_reads: u16,
    absent_reads: u16,
    frontier_checks: u16,
}

impl ValidationCost {
    #[must_use]
    pub const fn exact_reads(self) -> u16 {
        self.exact_reads
    }

    #[must_use]
    pub const fn absent_reads(self) -> u16 {
        self.absent_reads
    }

    #[must_use]
    pub const fn frontier_checks(self) -> u16 {
        self.frontier_checks
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reuse {
    StillValid {
        cost: ValidationCost,
    },
    Invalidated {
        reason: Invalidation,
        cost: ValidationCost,
    },
}

impl Reuse {
    #[must_use]
    pub const fn cost(self) -> ValidationCost {
        match self {
            Self::StillValid { cost } | Self::Invalidated { cost, .. } => cost,
        }
    }
}

impl WitnessJudgment {
    pub fn capture(
        snapshot: &WitnessSnapshot,
        frontiers: &ProductFrontiers,
        requests: Vec<WitnessRequest>,
    ) -> Result<Self, Error> {
        if requests.len() > MAX_WITNESSES {
            return Err(Error::Limit);
        }
        let mut keys = BTreeSet::new();
        let mut witnesses = Vec::with_capacity(requests.len());
        for request in requests {
            let key = match request {
                WitnessRequest::ExactValue { key, .. } | WitnessRequest::AbsentKey { key } => key,
            };
            if !keys.insert(key) {
                return Err(Error::Duplicate);
            }
            match request {
                WitnessRequest::ExactValue { key, role } => {
                    let entry = snapshot.entry(key).ok_or(Error::Missing)?;
                    witnesses.push(Witness::ExactValue {
                        key,
                        version: entry.version,
                        value: entry.value.clone(),
                        role,
                    });
                }
                WitnessRequest::AbsentKey { key } => {
                    if snapshot.entry(key).is_some() {
                        return Err(Error::Binding);
                    }
                    let marker = closed_marker(snapshot, frontiers)?;
                    witnesses.push(Witness::AbsentKey { key, marker });
                }
            }
        }
        Ok(Self {
            revision: snapshot.revision,
            control_cut: snapshot.control_cut,
            semantic_epoch: snapshot.semantic_epoch,
            domain: snapshot.domain_input.domain,
            witnesses,
        })
    }

    pub fn reuse_at(
        &self,
        snapshot: &WitnessSnapshot,
        frontiers: &ProductFrontiers,
    ) -> Result<Reuse, Error> {
        if snapshot.revision < self.revision || snapshot.control_cut < self.control_cut {
            return Err(Error::Stale);
        }
        let mut cost = ValidationCost::default();
        if snapshot.semantic_epoch != self.semantic_epoch {
            return Ok(invalidated(Invalidation::SemanticEpoch, cost));
        }
        let current_domain = snapshot.domain_input.domain;
        if current_domain.domain_id != self.domain.domain_id {
            return Ok(invalidated(Invalidation::DomainIdentity, cost));
        }
        if current_domain.domain_epoch != self.domain.domain_epoch {
            return Ok(invalidated(Invalidation::DomainEpoch, cost));
        }
        if current_domain.projection != self.domain.projection {
            return Ok(invalidated(Invalidation::Projection, cost));
        }
        for witness in &self.witnesses {
            match witness {
                Witness::ExactValue {
                    key,
                    version,
                    value,
                    role: _,
                } => {
                    cost.exact_reads = cost.exact_reads.checked_add(1).ok_or(Error::Overflow)?;
                    let Some(entry) = snapshot.entry(*key) else {
                        return Ok(invalidated(Invalidation::ExactValue, cost));
                    };
                    if entry.version != *version || entry.value != *value {
                        return Ok(invalidated(Invalidation::ExactValue, cost));
                    }
                }
                Witness::AbsentKey { key, marker } => {
                    cost.frontier_checks =
                        cost.frontier_checks.checked_add(1).ok_or(Error::Overflow)?;
                    let current_marker = closed_marker(snapshot, frontiers)?;
                    if current_marker != *marker {
                        return Ok(invalidated(Invalidation::ClosingFrontier, cost));
                    }
                    cost.absent_reads = cost.absent_reads.checked_add(1).ok_or(Error::Overflow)?;
                    if snapshot.entry(*key).is_some() {
                        return Ok(invalidated(Invalidation::AbsentKey, cost));
                    }
                }
            }
        }
        Ok(Reuse::StillValid { cost })
    }

    /// Returns the captured key version and query role for one exact dependency.
    /// The associated value remains opaque and is compared only at reuse time.
    #[must_use]
    pub fn exact_version_and_role(&self, key: u64) -> Option<(u64, QueryRole)> {
        self.witnesses.iter().find_map(|witness| match witness {
            Witness::ExactValue {
                key: witness_key,
                version,
                role,
                ..
            } if *witness_key == key => Some((*version, *role)),
            Witness::ExactValue { .. } | Witness::AbsentKey { .. } => None,
        })
    }
}

fn invalidated(reason: Invalidation, cost: ValidationCost) -> Reuse {
    Reuse::Invalidated { reason, cost }
}

fn closed_marker(
    snapshot: &WitnessSnapshot,
    frontiers: &ProductFrontiers,
) -> Result<TrustedClosingMarker, Error> {
    let DomainClosure::Closed(marker) = snapshot.domain_input.closure else {
        return Err(Error::Incomplete);
    };
    if marker.key != snapshot.domain_input.domain.projection {
        return Err(Error::Binding);
    }
    let requirement = FrontierRequirement {
        key: marker.key,
        stage: FrontierStage::Authenticated,
        through: marker.final_sequence,
        closure: Some(marker.marker_generation),
    };
    frontiers
        .satisfies(requirement)?
        .then_some(marker)
        .ok_or(Error::Incomplete)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Store {
        values: BTreeMap<u64, (u64, Vec<u8>)>,
    }

    impl Store {
        fn snapshot(
            &self,
            revision: u64,
            control_cut: u64,
            semantic_epoch: u64,
            domain: AdapterDomainInput,
        ) -> WitnessSnapshot {
            let entries = self
                .values
                .iter()
                .map(|(&key, (version, value))| {
                    SnapshotEntry::new(key, *version, value.clone()).unwrap()
                })
                .collect();
            WitnessSnapshot::new(revision, control_cut, semantic_epoch, domain, entries).unwrap()
        }
    }

    fn key() -> ProjectionKey {
        ProjectionKey {
            source: 0,
            branch: 0,
            projection: 0,
            source_epoch: 0,
        }
    }

    fn domain(marker: DomainClosure) -> AdapterDomainInput {
        AdapterDomainInput::new(DomainProjection::new(0, 0, key()), marker)
    }

    fn closed_frontiers() -> (ProductFrontiers, TrustedClosingMarker) {
        let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
        frontiers
            .accept(key(), FrontierStage::Authenticated, 1)
            .unwrap();
        let marker = TrustedClosingMarker {
            key: key(),
            final_sequence: 1,
            marker_generation: 1,
        };
        frontiers.record_close(marker).unwrap();
        (frontiers, marker)
    }

    fn requests() -> Vec<WitnessRequest> {
        vec![
            WitnessRequest::ExactValue {
                key: 0,
                role: QueryRole::PolicyInput,
            },
            WitnessRequest::AbsentKey { key: 7 },
        ]
    }

    // Deliberately independent from `WitnessJudgment::reuse_at`: this is the
    // direct bounded-store recomputation oracle for the acceptance matrix.
    fn always_recompute(store: &Store) -> bool {
        store
            .values
            .get(&0)
            .is_some_and(|(version, value)| *version == 0 && value == b"value")
            && !store.values.contains_key(&7)
    }

    #[test]
    fn exact_value_absence_and_unrelated_change_match_always_recompute() {
        let (frontiers, marker) = closed_frontiers();
        let mut store = Store {
            values: BTreeMap::from([(0, (0, b"value".to_vec())), (9, (1, b"unrelated".to_vec()))]),
        };
        let initial = store.snapshot(0, 0, 0, domain(DomainClosure::Closed(marker)));
        let judgment = WitnessJudgment::capture(&initial, &frontiers, requests()).unwrap();
        assert_eq!(
            judgment.exact_version_and_role(0),
            Some((0, QueryRole::PolicyInput))
        );

        assert!(always_recompute(&store));
        assert_eq!(
            judgment.reuse_at(&initial, &frontiers),
            Ok(Reuse::StillValid {
                cost: ValidationCost {
                    exact_reads: 1,
                    absent_reads: 1,
                    frontier_checks: 1
                }
            })
        );

        store.values.insert(9, (2, b"changed unrelated".to_vec()));
        let unrelated = store.snapshot(1, 1, 0, domain(DomainClosure::Closed(marker)));
        assert!(always_recompute(&store));
        assert_eq!(
            judgment.reuse_at(&unrelated, &frontiers),
            Ok(Reuse::StillValid {
                cost: ValidationCost {
                    exact_reads: 1,
                    absent_reads: 1,
                    frontier_checks: 1
                }
            })
        );
    }

    #[test]
    fn changed_value_version_and_inserted_absence_match_always_recompute() {
        let (frontiers, marker) = closed_frontiers();
        let original = Store {
            values: BTreeMap::from([(0, (0, b"value".to_vec()))]),
        };
        let initial = original.snapshot(3, 4, 0, domain(DomainClosure::Closed(marker)));
        let judgment = WitnessJudgment::capture(&initial, &frontiers, requests()).unwrap();

        let mut value_changed = original.clone();
        value_changed.values.insert(0, (0, b"different".to_vec()));
        assert!(!always_recompute(&value_changed));
        assert!(matches!(
            judgment.reuse_at(
                &value_changed.snapshot(4, 5, 0, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Ok(Reuse::Invalidated {
                reason: Invalidation::ExactValue,
                ..
            })
        ));

        let mut version_changed = original.clone();
        version_changed.values.insert(0, (1, b"value".to_vec()));
        assert!(!always_recompute(&version_changed));
        assert!(matches!(
            judgment.reuse_at(
                &version_changed.snapshot(4, 5, 0, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Ok(Reuse::Invalidated {
                reason: Invalidation::ExactValue,
                ..
            })
        ));

        let mut inserted = original;
        inserted.values.insert(7, (0, b"now present".to_vec()));
        assert!(!always_recompute(&inserted));
        assert!(matches!(
            judgment.reuse_at(
                &inserted.snapshot(4, 5, 0, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Ok(Reuse::Invalidated {
                reason: Invalidation::AbsentKey,
                ..
            })
        ));
    }

    #[test]
    fn stale_semantic_domain_and_closure_controls_refuse_reuse() {
        let (frontiers, marker) = closed_frontiers();
        let store = Store {
            values: BTreeMap::from([(0, (0, b"value".to_vec()))]),
        };
        let initial = store.snapshot(3, 4, 0, domain(DomainClosure::Closed(marker)));
        let judgment = WitnessJudgment::capture(&initial, &frontiers, requests()).unwrap();

        assert_eq!(
            judgment.reuse_at(
                &store.snapshot(2, 4, 0, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Err(Error::Stale)
        );
        assert_eq!(
            judgment.reuse_at(
                &store.snapshot(3, 3, 0, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Err(Error::Stale)
        );
        assert!(matches!(
            judgment.reuse_at(
                &store.snapshot(4, 5, 1, domain(DomainClosure::Closed(marker))),
                &frontiers
            ),
            Ok(Reuse::Invalidated {
                reason: Invalidation::SemanticEpoch,
                ..
            })
        ));

        let changed_domain = AdapterDomainInput::new(
            DomainProjection::new(0, 1, key()),
            DomainClosure::Closed(marker),
        );
        assert!(matches!(
            judgment.reuse_at(&store.snapshot(4, 5, 0, changed_domain), &frontiers),
            Ok(Reuse::Invalidated {
                reason: Invalidation::DomainEpoch,
                ..
            })
        ));
        let other_domain = AdapterDomainInput::new(
            DomainProjection::new(1, 0, key()),
            DomainClosure::Closed(marker),
        );
        assert!(matches!(
            judgment.reuse_at(&store.snapshot(4, 5, 0, other_domain), &frontiers),
            Ok(Reuse::Invalidated {
                reason: Invalidation::DomainIdentity,
                ..
            })
        ));
        let other_projection = ProjectionKey { source: 1, ..key() };
        let projection_marker = TrustedClosingMarker {
            key: other_projection,
            ..marker
        };
        let changed_projection = AdapterDomainInput::new(
            DomainProjection::new(0, 0, other_projection),
            DomainClosure::Closed(projection_marker),
        );
        assert!(matches!(
            judgment.reuse_at(&store.snapshot(4, 5, 0, changed_projection), &frontiers),
            Ok(Reuse::Invalidated {
                reason: Invalidation::Projection,
                ..
            })
        ));
        let unavailable_marker = TrustedClosingMarker {
            marker_generation: 2,
            ..marker
        };
        assert_eq!(
            judgment.reuse_at(
                &store.snapshot(4, 5, 0, domain(DomainClosure::Closed(unavailable_marker))),
                &frontiers
            ),
            Err(Error::Incomplete)
        );
        assert_eq!(
            judgment.reuse_at(
                &store.snapshot(4, 5, 0, domain(DomainClosure::Unknown)),
                &frontiers
            ),
            Err(Error::Incomplete)
        );
    }

    #[test]
    fn absence_needs_closed_authenticated_frontier_but_exact_read_does_not() {
        let store = Store {
            values: BTreeMap::from([(0, (0, b"value".to_vec()))]),
        };
        let open = ProductFrontiers::new(1, 4).unwrap();
        let unknown = store.snapshot(0, 0, 0, domain(DomainClosure::Unknown));
        assert_eq!(
            WitnessJudgment::capture(&unknown, &open, vec![WitnessRequest::AbsentKey { key: 7 }]),
            Err(Error::Incomplete)
        );

        let exact_only = WitnessJudgment::capture(
            &unknown,
            &open,
            vec![WitnessRequest::ExactValue {
                key: 0,
                role: QueryRole::Subject,
            }],
        )
        .unwrap();
        assert!(matches!(
            exact_only.reuse_at(&unknown, &open),
            Ok(Reuse::StillValid { .. })
        ));
    }

    #[test]
    fn bounds_duplicate_keys_and_wrong_marker_projection_refuse() {
        let entry = SnapshotEntry::new(0, 0, vec![]).unwrap();
        assert_eq!(
            SnapshotEntry::new(0, 0, vec![0; MAX_VALUE_BYTES + 1]),
            Err(Error::Limit)
        );
        assert_eq!(
            WitnessSnapshot::new(
                0,
                0,
                0,
                domain(DomainClosure::Unknown),
                vec![entry.clone(), entry]
            ),
            Err(Error::Duplicate)
        );

        let wrong = TrustedClosingMarker {
            key: ProjectionKey { source: 1, ..key() },
            final_sequence: 0,
            marker_generation: 1,
        };
        assert_eq!(
            WitnessSnapshot::new(0, 0, 0, domain(DomainClosure::Closed(wrong)), vec![]),
            Err(Error::Binding)
        );
        let too_many = (0..=MAX_WITNESSES)
            .map(|key| WitnessRequest::AbsentKey { key: key as u64 })
            .collect();
        let (frontiers, marker) = closed_frontiers();
        let snapshot =
            WitnessSnapshot::new(0, 0, 0, domain(DomainClosure::Closed(marker)), vec![]).unwrap();
        assert_eq!(
            WitnessJudgment::capture(&snapshot, &frontiers, too_many),
            Err(Error::Limit)
        );
    }
}
