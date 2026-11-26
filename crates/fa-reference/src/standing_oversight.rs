//! Standing finding support algebra, independent batch oversight oracle, and
//! bounded incremental query engine (FA-076, FA-077, FA-078, FA-079, FA-080;
//! plan §§7.9, 14.3, 14.9, 15.8, 18.6, 22.2; invariants FA-INV-021, FA-INV-026, FA-INV-039).
//!
//! Scope and nonclaims:
//! - Models support-backed standing findings with checked Z-set weights, minimal
//!   support antichains, closed-domain negation guards, and explicit generation
//!   boundaries over bounded reference projections.
//! - The batch oracle (`batch_oracle`) is an independent, stateless reference
//!   evaluator used to verify incremental derivatives at closed frontiers.
//! - Neither standing findings nor `ViewReceipt`s grant execution authority or
//!   mutate policy (`Permit` cannot be constructed from any type in this module).

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

use crate::{
    observation_algebra::{Basis, Knowledge, UncertaintyClass},
    product_frontier::{FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey},
};

pub const MAX_SNAPSHOT_FACTS: usize = 512;
pub const MAX_SUPPORT_ATOMS_PER_PATH: usize = 16;
pub const MAX_DEFAULT_ANTICHAIN_WIDTH: usize = 16;
pub const MAX_DELTA_BATCH_LEN: usize = 256;

/// Complete semantic generation vector binding model, codec, monitor, policy,
/// and calibration epochs (FA-076, FA-080, plan §§14.3, 15.8, 22.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticGenerationVector {
    pub model_generation: u64,
    pub codec_generation: u64,
    pub monitor_generation: u64,
    pub policy_generation: u64,
    pub calibration_generation: u64,
}

/// Registered standing oversight query families from plan §15.8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StandingQueryFamily {
    /// Principals that can reach a protected sink along active delegation/dispatch
    /// edges without a qualified gate (`actor -> process -> sink`, guard `gate` absent).
    UngatedSinkReachability,
    /// Artifacts that have gained an unreviewed dependency edge.
    UnreviewedArtifactDependency,
    /// Held proposals invalidated by an observed object or policy change.
    InvalidatedHeldProposal,
    /// Helper cohorts sharing a newly confirmed failure mode.
    SharedHelperFailureCohort,
}

/// Typed base fact in the oversight projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BaseFactKind {
    /// Directed edge `(from, to)` between actor/process/sink nodes.
    AuthorityEdge { from: u64, to: u64 },
    /// Qualified gate protection on `(from, to)`.
    QualifiedGate { from: u64, to: u64 },
    /// Artifact `from` depends on `to`.
    ArtifactDependency { artifact: u64, dependency: u64 },
    /// Dependency `(artifact, dependency)` has a current review witness.
    ReviewedDependency { artifact: u64, dependency: u64 },
    /// Proposal `proposal` depends on observed object/policy key `key`.
    HeldProposalDependency { proposal: u64, key: u64 },
    /// Object or policy key `key` changed at `revision`.
    InvalidatedKey { key: u64, revision: u64 },
    /// Helper `helper` belongs to cohort `cohort` and exhibited failure `failure_id`.
    CohortFailure { cohort: u64, failure_id: u64 },
}

/// Canonical base tuple identified by a stable `fact_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaseFact {
    pub fact_id: u64,
    pub kind: BaseFactKind,
}

/// Canonical signed Z-set tuple (`-1` retraction/deletion, `+1` insertion).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedFactDelta {
    pub fact: BaseFact,
    pub weight: i64,
}

/// Identifier of a derived standing finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FindingKey {
    pub query: StandingQueryFamily,
    pub subject: u64,
    pub target: u64,
}

/// One conjunctive support justification for a standing finding (`FA-076`).
///
/// A support path holds only when all `positive_atoms` are present with positive
/// Z-set multiplicity AND all `negative_guards` are confirmed absent in a closed
/// negative domain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SupportPath {
    pub positive_atoms: BTreeSet<u64>,
    pub negative_guards: BTreeSet<u64>,
}

impl SupportPath {
    pub fn new(
        positive_atoms: BTreeSet<u64>,
        negative_guards: BTreeSet<u64>,
    ) -> Result<Self, StandingOversightError> {
        let path = Self {
            positive_atoms,
            negative_guards,
        };
        path.validate()?;
        Ok(path)
    }

    fn validate(&self) -> Result<(), StandingOversightError> {
        if self.positive_atoms.is_empty()
            || self.positive_atoms.len() > MAX_SUPPORT_ATOMS_PER_PATH
            || self.negative_guards.len() > MAX_SUPPORT_ATOMS_PER_PATH
        {
            return Err(StandingOversightError::InvalidSupportPath);
        }
        Ok(())
    }

    /// Returns `true` if `self` is at least as general as `other` (`self ⊆ other`
    /// on both positive atoms and negative guards), meaning `other` is redundant
    /// in a minimal support antichain.
    #[must_use]
    pub fn dominates(&self, other: &Self) -> bool {
        self.positive_atoms.is_subset(&other.positive_atoms)
            && self.negative_guards.is_subset(&other.negative_guards)
    }
}

/// Configured overflow policy when a finding's distinct minimal support paths
/// exceed the antichain width bound (`FA-079`, plan §14.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntichainOverflowMode {
    /// Coarsen into a conservative summary that retains the union of active
    /// positive atoms and flags the finding as `CoarsenedRequiresBatchCheck`
    /// (never dropping a remaining support or claiming exact universal truth).
    ConservativeCoarsen,
    /// Immediately trigger an exact batch recomputation from the independent
    /// `FA-077` oracle.
    SpillToBatchRecompute,
    /// Fail closed with `AntichainWidthExceeded`.
    FailClosed,
}

/// Exactness state of a support antichain (`FA-076`, `FA-079`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportPrecision {
    /// Minimal antichain is exact and within configured width bounds.
    ExactMinimalAntichain,
    /// Antichain exceeded width bound and was conservatively coarsened; cannot
    /// be promoted as an exact universal claim without batch verification.
    CoarsenedConservative { retained_union_atoms: usize },
}

/// Bounded support antichain for a standing finding (`FA-076`, `FA-079`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportAntichain {
    max_width: usize,
    paths: Vec<SupportPath>,
    support_weights: BTreeMap<SupportPath, i64>,
    precision: SupportPrecision,
}

impl SupportAntichain {
    pub fn new(max_width: usize) -> Result<Self, StandingOversightError> {
        if max_width == 0 || max_width > MAX_SNAPSHOT_FACTS {
            return Err(StandingOversightError::InvalidAntichainLimit);
        }
        Ok(Self {
            max_width,
            paths: Vec::new(),
            support_weights: BTreeMap::new(),
            precision: SupportPrecision::ExactMinimalAntichain,
        })
    }

    pub fn paths(&self) -> &[SupportPath] {
        &self.paths
    }

    pub const fn precision(&self) -> SupportPrecision {
        self.precision
    }

    pub fn is_supported(&self) -> bool {
        !self.paths.is_empty()
    }

    pub fn total_weight(&self) -> Result<i64, StandingOversightError> {
        let mut sum = 0_i64;
        for &w in self.support_weights.values() {
            sum = sum
                .checked_add(w)
                .ok_or(StandingOversightError::WeightOverflow)?;
        }
        Ok(sum)
    }

    /// Applies a signed weight delta (`+w` insertion or `-w` retraction) to a
    /// `SupportPath` under checked Z-set arithmetic and antichain minimization.
    pub fn apply_delta(
        &mut self,
        path: SupportPath,
        delta_weight: i64,
        overflow_mode: AntichainOverflowMode,
    ) -> Result<(), StandingOversightError> {
        // Public path fields are reference inputs, not proof that the
        // constructor's size and nonempty-support contract was checked.
        path.validate()?;
        if delta_weight == 0 {
            return Err(StandingOversightError::ZeroWeightDelta);
        }
        let current = self.support_weights.get(&path).copied().unwrap_or(0);
        let next = current
            .checked_add(delta_weight)
            .ok_or(StandingOversightError::WeightOverflow)?;
        if next < 0 {
            return Err(StandingOversightError::LostRetractionOrNegativeWeight);
        }
        // A rejected width expansion must not leave a hidden support in the
        // weight map which a later retraction could expose.
        let mut candidate = self.clone();
        if next == 0 {
            candidate.support_weights.remove(&path);
        } else {
            candidate.support_weights.insert(path, next);
        }
        candidate.rebuild_minimal_antichain(overflow_mode)?;
        *self = candidate;
        Ok(())
    }

    fn rebuild_minimal_antichain(
        &mut self,
        overflow_mode: AntichainOverflowMode,
    ) -> Result<(), StandingOversightError> {
        let active: Vec<SupportPath> = self.support_weights.keys().cloned().collect();
        let mut minimal: Vec<SupportPath> = Vec::new();

        for candidate in &active {
            let dominated_by_other = active
                .iter()
                .any(|other| other != candidate && other.dominates(candidate));
            if !dominated_by_other {
                minimal.push(candidate.clone());
            }
        }

        if minimal.len() > self.max_width {
            match overflow_mode {
                AntichainOverflowMode::FailClosed
                | AntichainOverflowMode::SpillToBatchRecompute => {
                    return Err(StandingOversightError::AntichainWidthExceeded {
                        width: minimal.len(),
                        max_width: self.max_width,
                    });
                }
                AntichainOverflowMode::ConservativeCoarsen => {
                    // Never drop the sole remaining support; retain up to `max_width`
                    // paths and mark precision as conservatively coarsened.
                    let union_atoms: BTreeSet<u64> = minimal
                        .iter()
                        .flat_map(|p| p.positive_atoms.iter().copied())
                        .collect();
                    minimal.truncate(self.max_width);
                    self.paths = minimal;
                    self.precision = SupportPrecision::CoarsenedConservative {
                        retained_union_atoms: union_atoms.len(),
                    };
                    return Ok(());
                }
            }
        }

        self.paths = minimal;
        self.precision = SupportPrecision::ExactMinimalAntichain;
        Ok(())
    }
}

/// Typed refusal reasons across the support algebra, batch oracle, and
/// incremental oversight engine (FA-076..FA-080).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandingOversightError {
    InvalidSupportPath,
    InvalidAntichainLimit,
    ZeroWeightDelta,
    WeightOverflow,
    LostRetractionOrNegativeWeight,
    FactIdentityMismatch,
    NonPositiveSnapshotWeight,
    GenerationRebuildOverflow,
    UnclosedNegativeDomain,
    IncompleteProductFrontier,
    SnapshotLimitExceeded,
    DeltaBatchLimitExceeded,
    AntichainWidthExceeded {
        width: usize,
        max_width: usize,
    },
    CoarseSupportUniversalClaimForbidden,
    MixedSemanticGeneration {
        expected: SemanticGenerationVector,
        observed: SemanticGenerationVector,
    },
    OracleDivergenceDetected,
}

/// Complete closed snapshot consumed by the independent batch oversight oracle (`FA-077`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OversightSnapshot {
    pub generations: SemanticGenerationVector,
    pub negative_domain_closed: bool,
    pub frontiers: ProductFrontiers,
    pub required_frontier: FrontierRequirement,
    pub facts: BTreeMap<u64, (BaseFact, i64)>,
}

impl OversightSnapshot {
    pub fn new(
        generations: SemanticGenerationVector,
        negative_domain_closed: bool,
        frontiers: ProductFrontiers,
        required_frontier: FrontierRequirement,
    ) -> Self {
        Self {
            generations,
            negative_domain_closed,
            frontiers,
            required_frontier,
            facts: BTreeMap::new(),
        }
    }

    pub fn apply_fact_delta(
        &mut self,
        delta: SignedFactDelta,
    ) -> Result<(), StandingOversightError> {
        if delta.weight == 0 {
            return Err(StandingOversightError::ZeroWeightDelta);
        }
        self.validate_facts()?;
        if self
            .facts
            .get(&delta.fact.fact_id)
            .is_some_and(|(fact, _)| *fact != delta.fact)
        {
            // Replacing a tuple requires retracting its exact old value to
            // zero before inserting the new one. Its ID alone is insufficient.
            return Err(StandingOversightError::FactIdentityMismatch);
        }
        let current = self
            .facts
            .get(&delta.fact.fact_id)
            .map(|(_, w)| *w)
            .unwrap_or(0);
        let next = current
            .checked_add(delta.weight)
            .ok_or(StandingOversightError::WeightOverflow)?;
        if next < 0 {
            return Err(StandingOversightError::LostRetractionOrNegativeWeight);
        }
        if next == 0 {
            self.facts.remove(&delta.fact.fact_id);
        } else {
            if !self.facts.contains_key(&delta.fact.fact_id)
                && self.facts.len() >= MAX_SNAPSHOT_FACTS
            {
                return Err(StandingOversightError::SnapshotLimitExceeded);
            }
            self.facts.insert(delta.fact.fact_id, (delta.fact, next));
        }
        Ok(())
    }

    fn validate_facts(&self) -> Result<(), StandingOversightError> {
        if self.facts.len() > MAX_SNAPSHOT_FACTS {
            return Err(StandingOversightError::SnapshotLimitExceeded);
        }
        for (&fact_id, &(fact, weight)) in &self.facts {
            if fact_id != fact.fact_id {
                return Err(StandingOversightError::FactIdentityMismatch);
            }
            if weight <= 0 {
                return Err(StandingOversightError::NonPositiveSnapshotWeight);
            }
        }
        Ok(())
    }
}

/// Evaluated standing finding with its minimal support paths and Z-set weight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindingEvaluation {
    pub key: FindingKey,
    pub minimal_supports: Vec<SupportPath>,
    pub total_support_weight: i64,
    pub precision: SupportPrecision,
}

/// Output of the independent batch oversight oracle (`FA-077`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchOracleArtifact {
    pub generations: SemanticGenerationVector,
    pub frontier_seq: u64,
    pub findings: BTreeMap<FindingKey, FindingEvaluation>,
    pub tuples_examined: usize,
    pub estimated_memory_bytes: usize,
}

/// Independent stateless batch evaluator for registered standing oversight
/// queries (`FA-077`, plan §15.8).
pub mod batch_oracle {
    use super::*;

    pub fn evaluate_batch_query(
        snapshot: &OversightSnapshot,
        query: StandingQueryFamily,
        max_antichain_width: usize,
    ) -> Result<BatchOracleArtifact, StandingOversightError> {
        // The snapshot is publicly constructible. Validate its canonical map
        // and resource bound before allocating or evaluating a quadratic join.
        snapshot.validate_facts()?;
        if max_antichain_width == 0 || max_antichain_width > MAX_SNAPSHOT_FACTS {
            return Err(StandingOversightError::InvalidAntichainLimit);
        }
        if !snapshot
            .frontiers
            .satisfies(snapshot.required_frontier)
            .unwrap_or(false)
        {
            return Err(StandingOversightError::IncompleteProductFrontier);
        }

        // Queries with negative/absence guards require a closed negative domain.
        if matches!(
            query,
            StandingQueryFamily::UngatedSinkReachability
                | StandingQueryFamily::UnreviewedArtifactDependency
        ) && !snapshot.negative_domain_closed
        {
            return Err(StandingOversightError::UnclosedNegativeDomain);
        }

        let active_facts: Vec<(BaseFact, i64)> = snapshot
            .facts
            .values()
            .filter(|(_, w)| *w > 0)
            .copied()
            .collect();

        let mut antichains: BTreeMap<FindingKey, SupportAntichain> = BTreeMap::new();
        let mut tuples_examined = 0_usize;

        match query {
            StandingQueryFamily::UngatedSinkReachability => {
                // 2-hop reachability `actor -> middle -> sink` where neither hop has a
                // `QualifiedGate` fact.
                let gates: BTreeMap<(u64, u64), u64> = active_facts
                    .iter()
                    .filter_map(|(f, _)| match f.kind {
                        BaseFactKind::QualifiedGate { from, to } => Some(((from, to), f.fact_id)),
                        _ => None,
                    })
                    .collect();

                for &(f1, w1) in &active_facts {
                    let BaseFactKind::AuthorityEdge {
                        from: actor,
                        to: mid,
                    } = f1.kind
                    else {
                        continue;
                    };
                    for &(f2, w2) in &active_facts {
                        tuples_examined += 1;
                        let BaseFactKind::AuthorityEdge {
                            from: mid2,
                            to: sink,
                        } = f2.kind
                        else {
                            continue;
                        };
                        if mid != mid2 || actor == sink {
                            continue;
                        }
                        if gates.contains_key(&(actor, mid)) || gates.contains_key(&(mid, sink)) {
                            continue;
                        }
                        let mut pos = BTreeSet::new();
                        pos.insert(f1.fact_id);
                        pos.insert(f2.fact_id);
                        let mut neg = BTreeSet::new();
                        // Record negative guard identifiers encoding the absent gates.
                        neg.insert(actor.wrapping_mul(1_000_003).wrapping_add(mid));
                        neg.insert(mid.wrapping_mul(1_000_003).wrapping_add(sink));
                        let path = SupportPath::new(pos, neg)?;
                        let weight = w1
                            .checked_mul(w2)
                            .ok_or(StandingOversightError::WeightOverflow)?;
                        let key = FindingKey {
                            query,
                            subject: actor,
                            target: sink,
                        };
                        let entry = match antichains.entry(key) {
                            std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                            std::collections::btree_map::Entry::Vacant(v) => {
                                v.insert(SupportAntichain::new(max_antichain_width)?)
                            }
                        };
                        entry.apply_delta(path, weight, AntichainOverflowMode::FailClosed)?;
                    }
                }
            }
            StandingQueryFamily::UnreviewedArtifactDependency => {
                let reviewed: BTreeSet<(u64, u64)> = active_facts
                    .iter()
                    .filter_map(|(f, _)| match f.kind {
                        BaseFactKind::ReviewedDependency {
                            artifact,
                            dependency,
                        } => Some((artifact, dependency)),
                        _ => None,
                    })
                    .collect();

                for &(f, w) in &active_facts {
                    tuples_examined += 1;
                    let BaseFactKind::ArtifactDependency {
                        artifact,
                        dependency,
                    } = f.kind
                    else {
                        continue;
                    };
                    if reviewed.contains(&(artifact, dependency)) {
                        continue;
                    }
                    let mut pos = BTreeSet::new();
                    pos.insert(f.fact_id);
                    let mut neg = BTreeSet::new();
                    neg.insert(artifact.wrapping_mul(1_000_003).wrapping_add(dependency));
                    let path = SupportPath::new(pos, neg)?;
                    let key = FindingKey {
                        query,
                        subject: artifact,
                        target: dependency,
                    };
                    let entry = match antichains.entry(key) {
                        std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                        std::collections::btree_map::Entry::Vacant(v) => {
                            v.insert(SupportAntichain::new(max_antichain_width)?)
                        }
                    };
                    entry.apply_delta(path, w, AntichainOverflowMode::FailClosed)?;
                }
            }
            StandingQueryFamily::InvalidatedHeldProposal => {
                for &(f1, w1) in &active_facts {
                    let BaseFactKind::HeldProposalDependency { proposal, key } = f1.kind else {
                        continue;
                    };
                    for &(f2, w2) in &active_facts {
                        tuples_examined += 1;
                        let BaseFactKind::InvalidatedKey { key: k2, .. } = f2.kind else {
                            continue;
                        };
                        if key != k2 {
                            continue;
                        }
                        let mut pos = BTreeSet::new();
                        pos.insert(f1.fact_id);
                        pos.insert(f2.fact_id);
                        let path = SupportPath::new(pos, BTreeSet::new())?;
                        let weight = w1
                            .checked_mul(w2)
                            .ok_or(StandingOversightError::WeightOverflow)?;
                        let finding_key = FindingKey {
                            query,
                            subject: proposal,
                            target: key,
                        };
                        let entry = match antichains.entry(finding_key) {
                            std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                            std::collections::btree_map::Entry::Vacant(v) => {
                                v.insert(SupportAntichain::new(max_antichain_width)?)
                            }
                        };
                        entry.apply_delta(path, weight, AntichainOverflowMode::FailClosed)?;
                    }
                }
            }
            StandingQueryFamily::SharedHelperFailureCohort => {
                for &(f, w) in &active_facts {
                    tuples_examined += 1;
                    let BaseFactKind::CohortFailure { cohort, failure_id } = f.kind else {
                        continue;
                    };
                    let mut pos = BTreeSet::new();
                    pos.insert(f.fact_id);
                    let path = SupportPath::new(pos, BTreeSet::new())?;
                    let key = FindingKey {
                        query,
                        subject: cohort,
                        target: failure_id,
                    };
                    let entry = match antichains.entry(key) {
                        std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                        std::collections::btree_map::Entry::Vacant(v) => {
                            v.insert(SupportAntichain::new(max_antichain_width)?)
                        }
                    };
                    entry.apply_delta(path, w, AntichainOverflowMode::FailClosed)?;
                }
            }
        }

        let mut findings = BTreeMap::new();
        for (key, antichain) in antichains {
            if antichain.is_supported() {
                findings.insert(
                    key,
                    FindingEvaluation {
                        key,
                        minimal_supports: antichain.paths().to_vec(),
                        total_support_weight: antichain.total_weight()?,
                        precision: antichain.precision(),
                    },
                );
            }
        }

        let estimated_memory_bytes =
            active_facts.len() * 32 + findings.len() * 96 + tuples_examined * 8;

        Ok(BatchOracleArtifact {
            generations: snapshot.generations,
            frontier_seq: snapshot.required_frontier.through,
            findings,
            tuples_examined,
            estimated_memory_bytes,
        })
    }
}

/// Receipt emitted at a closed frontier by the incremental standing oversight
/// engine (`FA-078`, `FA-079`, `FA-080`, plan §15.8).
///
/// A `ViewReceipt` is an evidence/oversight value (`FA-INV-039`), never an
/// execution permit:
///
/// ```compile_fail,E0599
/// use fa_reference::standing_oversight::ViewReceipt;
/// fn elevate(receipt: ViewReceipt) {
///     let _ = receipt.into_permit();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewReceipt {
    pub query: StandingQueryFamily,
    pub generations: SemanticGenerationVector,
    pub uncertainty: UncertaintyClass,
    pub from_frontier_seq: u64,
    pub to_frontier_seq: u64,
    pub findings: BTreeMap<FindingKey, FindingEvaluation>,
    pub coarsened_findings: usize,
    pub spilled_to_batch_recompute: bool,
    pub generation_rebuilds: u64,
    /// Distinct finding keys selected by this batch's old and new exact tuples.
    /// Includes findings removed by the batch; excludes untouched cached keys.
    pub affected_findings: usize,
    /// Input deltas plus indexed neighbor/contribution visits and affected-key
    /// refreshes. This is NOT total update work: atomic state/receipt copies,
    /// map comparisons and antichain minimization are not counted here.
    pub incremental_work_units: usize,
    /// Tuple visits in the mandatory independent batch verification and any
    /// exact spill. Verification still runs over the complete closed snapshot;
    /// indexed maintenance does not make the whole operation sublinear.
    pub batch_work_units: usize,
    /// Estimated fact, index and retained finding payload bytes. Excludes map
    /// allocator overhead, frontier storage and temporary candidate/receipt copies.
    pub retained_state_bytes: usize,
}

impl ViewReceipt {
    /// Checks whether this receipt can back an exact universal oversight claim.
    /// Coarsened support antichains cannot claim universal exactness (`FA-076`, `FA-079`).
    pub fn require_exact_universal_support(&self) -> Result<(), StandingOversightError> {
        if self.coarsened_findings > 0 {
            return Err(StandingOversightError::CoarseSupportUniversalClaimForbidden);
        }
        Ok(())
    }
}

// Exact tuple indexes, not the wrapping display identifiers in negative_guards.
// Each active fact belongs to at most three buckets, and an empty bucket is
// removed immediately. Index space is bounded by the CURRENT snapshot, not by
// the number of historic inserts, retractions or fact-ID replacements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FactIndexKey {
    EdgeFrom(u64),
    EdgeTo(u64),
    Edge(u64, u64),
    Gate(u64, u64),
    Dependency(u64, u64),
    Reviewed(u64, u64),
    HeldKey(u64),
    Held(u64, u64),
    Invalidated(u64),
    Cohort(u64, u64),
}

#[derive(Clone, Debug, Default)]
struct FactIndexes(BTreeMap<FactIndexKey, BTreeSet<u64>>);

impl FactIndexes {
    fn keys(query: StandingQueryFamily, kind: BaseFactKind) -> [Option<FactIndexKey>; 3] {
        use BaseFactKind as Fact;
        use FactIndexKey as Index;
        use StandingQueryFamily as Query;
        match (query, kind) {
            (Query::UngatedSinkReachability, Fact::AuthorityEdge { from, to }) => [
                Some(Index::EdgeFrom(from)),
                Some(Index::EdgeTo(to)),
                Some(Index::Edge(from, to)),
            ],
            (Query::UngatedSinkReachability, Fact::QualifiedGate { from, to }) => {
                [Some(Index::Gate(from, to)), None, None]
            }
            (
                Query::UnreviewedArtifactDependency,
                Fact::ArtifactDependency {
                    artifact,
                    dependency,
                },
            ) => [Some(Index::Dependency(artifact, dependency)), None, None],
            (
                Query::UnreviewedArtifactDependency,
                Fact::ReviewedDependency {
                    artifact,
                    dependency,
                },
            ) => [Some(Index::Reviewed(artifact, dependency)), None, None],
            (Query::InvalidatedHeldProposal, Fact::HeldProposalDependency { proposal, key }) => [
                Some(Index::HeldKey(key)),
                Some(Index::Held(proposal, key)),
                None,
            ],
            (Query::InvalidatedHeldProposal, Fact::InvalidatedKey { key, .. }) => {
                [Some(Index::Invalidated(key)), None, None]
            }
            (Query::SharedHelperFailureCohort, Fact::CohortFailure { cohort, failure_id }) => {
                [Some(Index::Cohort(cohort, failure_id)), None, None]
            }
            _ => [None; 3],
        }
    }

    fn update(&mut self, query: StandingQueryFamily, fact: BaseFact, insert: bool) {
        for key in Self::keys(query, fact.kind).into_iter().flatten() {
            if insert {
                self.0.entry(key).or_default().insert(fact.fact_id);
            } else if let Some(ids) = self.0.get_mut(&key) {
                ids.remove(&fact.fact_id);
                if ids.is_empty() {
                    self.0.remove(&key);
                }
            }
        }
    }

    fn ids(&self, key: FactIndexKey) -> impl Iterator<Item = u64> + '_ {
        self.0.get(&key).into_iter().flatten().copied()
    }

    fn contains(&self, key: FactIndexKey) -> bool {
        self.0.contains_key(&key)
    }

    fn count(&self, key: FactIndexKey) -> usize {
        self.0.get(&key).map_or(0, BTreeSet::len)
    }

    fn payload_bytes(&self) -> usize {
        self.0
            .values()
            .map(|ids| size_of::<FactIndexKey>() + ids.len() * size_of::<u64>())
            .sum()
    }
}

/// Incremental standing oversight engine maintaining support-backed findings
/// across canonical `-old, +new` delta streams with bounded antichains (`FA-079`),
/// semantic-generation rebuilds (`FA-080`), and closed-frontier differential
/// verification against the `FA-077` batch oracle (`FA-078`, `FA-INV-026`).
#[derive(Clone, Debug)]
pub struct IncrementalOversightEngine {
    basis: Basis,
    query: StandingQueryFamily,
    max_antichain_width: usize,
    overflow_mode: AntichainOverflowMode,
    snapshot: OversightSnapshot,
    indexes: FactIndexes,
    maintained_findings: BTreeMap<FindingKey, FindingEvaluation>,
    last_closed_seq: u64,
    generation_rebuilds: u64,
}

impl IncrementalOversightEngine {
    pub fn new(
        basis: Basis,
        query: StandingQueryFamily,
        initial_generations: SemanticGenerationVector,
        projection_key: ProjectionKey,
        max_antichain_width: usize,
        overflow_mode: AntichainOverflowMode,
    ) -> Result<Self, StandingOversightError> {
        if max_antichain_width == 0 || max_antichain_width > MAX_SNAPSHOT_FACTS {
            return Err(StandingOversightError::InvalidAntichainLimit);
        }
        let frontiers = ProductFrontiers::new(4, 16)
            .map_err(|_| StandingOversightError::IncompleteProductFrontier)?;

        let required_frontier = FrontierRequirement {
            key: projection_key,
            stage: FrontierStage::Authenticated,
            through: 0,
            closure: None,
        };
        Ok(Self {
            basis,
            query,
            max_antichain_width,
            overflow_mode,
            snapshot: OversightSnapshot::new(
                initial_generations,
                true,
                frontiers,
                required_frontier,
            ),
            indexes: FactIndexes::default(),
            maintained_findings: BTreeMap::new(),
            last_closed_seq: 0,
            generation_rebuilds: 0,
        })
    }

    pub const fn generations(&self) -> SemanticGenerationVector {
        self.snapshot.generations
    }

    pub const fn generation_rebuilds(&self) -> u64 {
        self.generation_rebuilds
    }

    /// Explicitly updates the semantic generation vector (model, codec, monitor,
    /// policy, or calibration epoch) and rebuilds all standing findings from the
    /// base snapshot (`FA-080`).
    pub fn advance_semantic_generations(
        &mut self,
        new_generations: SemanticGenerationVector,
    ) -> Result<(), StandingOversightError> {
        if new_generations.model_generation < self.snapshot.generations.model_generation
            || new_generations.codec_generation < self.snapshot.generations.codec_generation
            || new_generations.monitor_generation < self.snapshot.generations.monitor_generation
            || new_generations.policy_generation < self.snapshot.generations.policy_generation
            || new_generations.calibration_generation
                < self.snapshot.generations.calibration_generation
        {
            return Err(StandingOversightError::MixedSemanticGeneration {
                expected: self.snapshot.generations,
                observed: new_generations,
            });
        }
        if new_generations == self.snapshot.generations {
            return Ok(());
        }
        let mut candidate = self.clone();
        candidate.snapshot.generations = new_generations;
        candidate.maintained_findings.clear();
        candidate.generation_rebuilds = candidate
            .generation_rebuilds
            .checked_add(1)
            .ok_or(StandingOversightError::GenerationRebuildOverflow)?;
        let mut affected = BTreeSet::new();
        let mut work = 0;
        for (fact, _) in candidate.snapshot.facts.values() {
            candidate.collect_affected(fact.kind, &mut affected, &mut work);
        }
        candidate.refresh_findings(&affected, &mut work)?;
        *self = candidate;
        Ok(())
    }

    /// Applies the next canonical delta batch at `next_frontier_seq`, verifies against
    /// the independent `FA-077` batch oracle at the closed frontier (`FA-INV-026`),
    /// and emits a `Knowledge<ViewReceipt>` (`FA-078`, `FA-079`, `FA-080`).
    /// Each batch advances exactly one source position. Replayed, skipped or
    /// unclosed positions are refused; the supplied closure booleans remain
    /// caller-declared reference observations, not origin authentication.
    /// Any refusal leaves the complete previously accepted engine unchanged.
    pub fn apply_delta_and_close_frontier(
        &mut self,
        delta_generations: SemanticGenerationVector,
        deltas: &[SignedFactDelta],
        next_frontier_seq: u64,
        negative_domain_closed: bool,
        frontier_closed: bool,
    ) -> Result<Knowledge<ViewReceipt>, StandingOversightError> {
        if deltas.len() > MAX_DELTA_BATCH_LEN {
            return Err(StandingOversightError::DeltaBatchLimitExceeded);
        }

        // FA-080: Reject delta streams that mix semantic generations without an
        // explicit generation rebuild transaction.
        if delta_generations != self.snapshot.generations {
            return Err(StandingOversightError::MixedSemanticGeneration {
                expected: self.snapshot.generations,
                observed: delta_generations,
            });
        }

        if !frontier_closed || self.last_closed_seq.checked_add(1) != Some(next_frontier_seq) {
            return Err(StandingOversightError::IncompleteProductFrontier);
        }

        if matches!(
            self.query,
            StandingQueryFamily::UngatedSinkReachability
                | StandingQueryFamily::UnreviewedArtifactDependency
        ) && !negative_domain_closed
        {
            return Err(StandingOversightError::UnclosedNegativeDomain);
        }

        let mut candidate = self.clone();
        let receipt =
            candidate.apply_closed_batch(deltas, next_frontier_seq, negative_domain_closed)?;
        *self = candidate;
        Ok(receipt)
    }

    fn apply_closed_batch(
        &mut self,
        deltas: &[SignedFactDelta],
        next_frontier_seq: u64,
        negative_domain_closed: bool,
    ) -> Result<Knowledge<ViewReceipt>, StandingOversightError> {
        self.snapshot.negative_domain_closed = negative_domain_closed;
        let key = self.snapshot.required_frontier.key;
        self.snapshot
            .frontiers
            .accept(key, FrontierStage::Captured, next_frontier_seq)
            .map_err(|_| StandingOversightError::IncompleteProductFrontier)?;
        self.snapshot
            .frontiers
            .accept(key, FrontierStage::Authenticated, next_frontier_seq)
            .map_err(|_| StandingOversightError::IncompleteProductFrontier)?;
        self.snapshot.required_frontier.through = next_frontier_seq;

        let mut affected = BTreeSet::new();
        let mut work_units = deltas.len();
        for &delta in deltas {
            // Route both sides of an exact replacement, including old paths
            // removed by this delta and new joins formed within the same batch.
            // Refresh happens only after ALL final base tuples are admitted.
            self.collect_affected(delta.fact.kind, &mut affected, &mut work_units);
            let was_present = self.snapshot.facts.contains_key(&delta.fact.fact_id);
            self.snapshot.apply_fact_delta(delta)?;
            let is_present = self.snapshot.facts.contains_key(&delta.fact.fact_id);
            if was_present != is_present {
                self.indexes.update(self.query, delta.fact, is_present);
            }
            self.collect_affected(delta.fact.kind, &mut affected, &mut work_units);
        }

        let spill_work = self.refresh_findings(&affected, &mut work_units)?;
        let (coarsened_findings, spilled_to_batch_recompute) = self.precision_state();

        // The complete oracle remains independent of every maintained index and
        // cache. Its cost is explicit, separate from indexed derivative work.
        let oracle_width = if spilled_to_batch_recompute || coarsened_findings > 0 {
            MAX_SNAPSHOT_FACTS
        } else {
            self.max_antichain_width
        };
        let oracle_artifact =
            batch_oracle::evaluate_batch_query(&self.snapshot, self.query, oracle_width)?;
        self.verify_findings(&oracle_artifact.findings)?;

        let from_seq = self.last_closed_seq;
        self.last_closed_seq = next_frontier_seq;
        let uncertainty = if coarsened_findings > 0 {
            UncertaintyClass::Conservative
        } else {
            UncertaintyClass::Exact
        };
        let receipt = ViewReceipt {
            query: self.query,
            generations: self.snapshot.generations,
            uncertainty,
            from_frontier_seq: from_seq,
            to_frontier_seq: next_frontier_seq,
            findings: self.maintained_findings.clone(),
            coarsened_findings,
            spilled_to_batch_recompute,
            generation_rebuilds: self.generation_rebuilds,
            affected_findings: affected.len(),
            incremental_work_units: work_units,
            batch_work_units: spill_work + oracle_artifact.tuples_examined,
            retained_state_bytes: self.retained_payload_bytes(),
        };

        Ok(Knowledge::known(receipt, self.basis))
    }

    fn collect_affected(
        &self,
        kind: BaseFactKind,
        affected: &mut BTreeSet<FindingKey>,
        work: &mut usize,
    ) {
        let mut insert = |subject, target| {
            affected.insert(FindingKey {
                query: self.query,
                subject,
                target,
            });
        };
        match (self.query, kind) {
            (
                StandingQueryFamily::UngatedSinkReachability,
                BaseFactKind::AuthorityEdge { from, to },
            ) => {
                self.edge_neighbors(from, to, &mut insert, work);
            }
            (
                StandingQueryFamily::UngatedSinkReachability,
                BaseFactKind::QualifiedGate { from, to },
            ) => {
                // Blockers refer to exact tuples, and all IDs/multiplicities of
                // the same blocker must disappear before an edge is ungated.
                if self.indexes.contains(FactIndexKey::Edge(from, to)) {
                    self.edge_neighbors(from, to, &mut insert, work);
                }
            }
            (
                StandingQueryFamily::UnreviewedArtifactDependency,
                BaseFactKind::ArtifactDependency {
                    artifact,
                    dependency,
                }
                | BaseFactKind::ReviewedDependency {
                    artifact,
                    dependency,
                },
            ) => {
                *work += 1;
                insert(artifact, dependency);
            }
            (
                StandingQueryFamily::InvalidatedHeldProposal,
                BaseFactKind::HeldProposalDependency { proposal, key },
            ) => {
                *work += 1;
                insert(proposal, key);
            }
            (
                StandingQueryFamily::InvalidatedHeldProposal,
                BaseFactKind::InvalidatedKey { key, .. },
            ) => {
                for id in self.indexes.ids(FactIndexKey::HeldKey(key)) {
                    *work += 1;
                    if let BaseFactKind::HeldProposalDependency { proposal, .. } =
                        self.snapshot.facts[&id].0.kind
                    {
                        insert(proposal, key);
                    }
                }
            }
            (
                StandingQueryFamily::SharedHelperFailureCohort,
                BaseFactKind::CohortFailure { cohort, failure_id },
            ) => {
                *work += 1;
                insert(cohort, failure_id);
            }
            _ => {}
        }
    }

    fn edge_neighbors(
        &self,
        from: u64,
        to: u64,
        insert: &mut impl FnMut(u64, u64),
        work: &mut usize,
    ) {
        for id in self.indexes.ids(FactIndexKey::EdgeFrom(to)) {
            *work += 1;
            if let BaseFactKind::AuthorityEdge { to: sink, .. } = self.snapshot.facts[&id].0.kind {
                if from != sink {
                    insert(from, sink);
                }
            }
        }
        for id in self.indexes.ids(FactIndexKey::EdgeTo(from)) {
            *work += 1;
            if let BaseFactKind::AuthorityEdge { from: actor, .. } = self.snapshot.facts[&id].0.kind
            {
                if actor != to {
                    insert(actor, to);
                }
            }
        }
    }

    /// Reconstruct each selected key once from the FINAL indexed base relation.
    /// This retains all weighted supports under coarsening without applying
    /// transient partial-batch widths or losing a previously hidden alternative.
    /// A spill still uses the original complete independent batch evaluator.
    fn refresh_findings(
        &mut self,
        affected: &BTreeSet<FindingKey>,
        work: &mut usize,
    ) -> Result<usize, StandingOversightError> {
        for &key in affected {
            *work += 1;
            let built = self.build_finding(key, work);
            let chain = match built {
                Ok(chain) => chain,
                Err(StandingOversightError::AntichainWidthExceeded { .. })
                    if self.overflow_mode == AntichainOverflowMode::SpillToBatchRecompute =>
                {
                    let batch = batch_oracle::evaluate_batch_query(
                        &self.snapshot,
                        self.query,
                        MAX_SNAPSHOT_FACTS,
                    )?;
                    self.maintained_findings = batch.findings;
                    return Ok(batch.tuples_examined);
                }
                Err(error) => return Err(error),
            };
            if chain.is_supported() {
                self.maintained_findings.insert(
                    key,
                    FindingEvaluation {
                        key,
                        minimal_supports: chain.paths().to_vec(),
                        total_support_weight: chain.total_weight()?,
                        precision: chain.precision(),
                    },
                );
            } else {
                self.maintained_findings.remove(&key);
            }
        }
        Ok(0)
    }

    fn build_finding(
        &self,
        key: FindingKey,
        work: &mut usize,
    ) -> Result<SupportAntichain, StandingOversightError> {
        use FactIndexKey as Index;
        let mut chain = SupportAntichain::new(self.max_antichain_width)?;
        let subject = key.subject;
        let target = key.target;
        match self.query {
            StandingQueryFamily::UngatedSinkReachability => {
                // Visit the smaller endpoint index, then join exact midpoints.
                // Neither unrelated source pairs nor unrelated fact kinds scan.
                let forward = self.indexes.count(Index::EdgeFrom(subject))
                    <= self.indexes.count(Index::EdgeTo(target));
                let index = if forward {
                    Index::EdgeFrom(subject)
                } else {
                    Index::EdgeTo(target)
                };
                for id in self.indexes.ids(index) {
                    *work += 1;
                    let BaseFactKind::AuthorityEdge { from, to } = self.snapshot.facts[&id].0.kind
                    else {
                        return Err(StandingOversightError::OracleDivergenceDetected);
                    };
                    let middle = if forward { to } else { from };
                    if self.indexes.contains(Index::Gate(subject, middle))
                        || self.indexes.contains(Index::Gate(middle, target))
                    {
                        continue;
                    }
                    let other = if forward {
                        Index::Edge(middle, target)
                    } else {
                        Index::Edge(subject, middle)
                    };
                    for other_id in self.indexes.ids(other) {
                        *work += 1;
                        let (_, first_weight) = self.snapshot.facts[&id];
                        let (_, second_weight) = self.snapshot.facts[&other_id];
                        let weight = first_weight
                            .checked_mul(second_weight)
                            .ok_or(StandingOversightError::WeightOverflow)?;
                        let positive = BTreeSet::from([id, other_id]);
                        let negative = BTreeSet::from([
                            subject.wrapping_mul(1_000_003).wrapping_add(middle),
                            middle.wrapping_mul(1_000_003).wrapping_add(target),
                        ]);
                        chain.apply_delta(
                            SupportPath::new(positive, negative)?,
                            weight,
                            self.overflow_mode,
                        )?;
                    }
                }
            }
            StandingQueryFamily::UnreviewedArtifactDependency => {
                if !self.indexes.contains(Index::Reviewed(subject, target)) {
                    let negative =
                        BTreeSet::from([subject.wrapping_mul(1_000_003).wrapping_add(target)]);
                    for id in self.indexes.ids(Index::Dependency(subject, target)) {
                        *work += 1;
                        chain.apply_delta(
                            SupportPath::new(BTreeSet::from([id]), negative.clone())?,
                            self.snapshot.facts[&id].1,
                            self.overflow_mode,
                        )?;
                    }
                }
            }
            StandingQueryFamily::InvalidatedHeldProposal => {
                for first in self.indexes.ids(Index::Held(subject, target)) {
                    *work += 1;
                    for second in self.indexes.ids(Index::Invalidated(target)) {
                        *work += 1;
                        let weight = self.snapshot.facts[&first]
                            .1
                            .checked_mul(self.snapshot.facts[&second].1)
                            .ok_or(StandingOversightError::WeightOverflow)?;
                        chain.apply_delta(
                            SupportPath::new(BTreeSet::from([first, second]), BTreeSet::new())?,
                            weight,
                            self.overflow_mode,
                        )?;
                    }
                }
            }
            StandingQueryFamily::SharedHelperFailureCohort => {
                for id in self.indexes.ids(Index::Cohort(subject, target)) {
                    *work += 1;
                    chain.apply_delta(
                        SupportPath::new(BTreeSet::from([id]), BTreeSet::new())?,
                        self.snapshot.facts[&id].1,
                        self.overflow_mode,
                    )?;
                }
            }
        }
        Ok(chain)
    }

    fn precision_state(&self) -> (usize, bool) {
        let coarsened = self
            .maintained_findings
            .values()
            .filter(|finding| {
                matches!(
                    finding.precision,
                    SupportPrecision::CoarsenedConservative { .. }
                )
            })
            .count();
        // A retained exact spill survives irrelevant/empty batches. Its larger
        // verification width is withdrawn only after every wide finding shrinks.
        let spilled = self.overflow_mode == AntichainOverflowMode::SpillToBatchRecompute
            && self
                .maintained_findings
                .values()
                .any(|finding| finding.minimal_supports.len() > self.max_antichain_width);
        (coarsened, spilled)
    }

    fn verify_findings(
        &self,
        oracle: &BTreeMap<FindingKey, FindingEvaluation>,
    ) -> Result<(), StandingOversightError> {
        if self.maintained_findings.len() != oracle.len() {
            return Err(StandingOversightError::OracleDivergenceDetected);
        }
        for (key, actual) in &self.maintained_findings {
            let expected = oracle
                .get(key)
                .ok_or(StandingOversightError::OracleDivergenceDetected)?;
            let agrees = match actual.precision {
                SupportPrecision::ExactMinimalAntichain => actual == expected,
                SupportPrecision::CoarsenedConservative {
                    retained_union_atoms,
                } => {
                    let union: BTreeSet<_> = expected
                        .minimal_supports
                        .iter()
                        .flat_map(|path| path.positive_atoms.iter().copied())
                        .collect();
                    expected.minimal_supports.len() > self.max_antichain_width
                        && actual.key == expected.key
                        && actual.total_support_weight == expected.total_support_weight
                        && retained_union_atoms == union.len()
                        && actual.minimal_supports
                            == expected.minimal_supports[..self.max_antichain_width]
                }
            };
            if !agrees {
                return Err(StandingOversightError::OracleDivergenceDetected);
            }
        }
        Ok(())
    }

    fn retained_payload_bytes(&self) -> usize {
        self.snapshot.facts.len() * size_of::<(u64, (BaseFact, i64))>()
            + self.indexes.payload_bytes()
            + self
                .maintained_findings
                .values()
                .map(|finding| {
                    size_of::<FindingKey>()
                        + size_of::<FindingEvaluation>()
                        + finding
                            .minimal_supports
                            .iter()
                            .map(|path| {
                                size_of::<SupportPath>()
                                    + size_of::<u64>()
                                        * (path.positive_atoms.len() + path.negative_guards.len())
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation_algebra::{BasisIdentity, ClaimClass, ProofStates, ScopedProof};

    fn sample_basis() -> Basis {
        Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 2,
                profile_id: 3,
                semantic_generation: 1,
                policy_epoch: 1,
                model_epoch: 1,
                tokenizer_epoch: 1,
                codec_epoch: 1,
                control_seq: 1,
            },
            ClaimClass::Invariant,
            ProofStates {
                authentic_origin: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                complete_for_contract: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                semantically_valid: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                available_for_replay: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
            },
        )
    }

    fn sample_generations() -> SemanticGenerationVector {
        SemanticGenerationVector {
            model_generation: 1,
            codec_generation: 1,
            monitor_generation: 1,
            policy_generation: 1,
            calibration_generation: 1,
        }
    }

    #[test]
    fn support_algebra_preserves_independent_supports_and_rejects_lost_retractions_or_overflow() {
        let mut antichain = SupportAntichain::new(4).unwrap();
        let path_a = SupportPath::new(BTreeSet::from([1, 2]), BTreeSet::from([100])).unwrap();
        let path_b = SupportPath::new(BTreeSet::from([1, 3]), BTreeSet::from([100])).unwrap();
        let redundant_superset =
            SupportPath::new(BTreeSet::from([1, 2, 9]), BTreeSet::from([100])).unwrap();

        antichain
            .apply_delta(path_a.clone(), 1, AntichainOverflowMode::FailClosed)
            .unwrap();
        antichain
            .apply_delta(path_b.clone(), 1, AntichainOverflowMode::FailClosed)
            .unwrap();
        antichain
            .apply_delta(
                redundant_superset.clone(),
                1,
                AntichainOverflowMode::FailClosed,
            )
            .unwrap();

        // `redundant_superset` is dominated by `path_a`, so minimal antichain has only `path_a` and `path_b`.
        assert_eq!(antichain.paths(), &[path_a.clone(), path_b.clone()]);
        assert_eq!(antichain.total_weight().unwrap(), 3);

        // Retracting `path_a` exposes `redundant_superset` alongside `path_b`; retracting
        // `redundant_superset` then leaves `path_b` supporting the finding.
        antichain
            .apply_delta(path_a.clone(), -1, AntichainOverflowMode::FailClosed)
            .unwrap();
        assert!(antichain.is_supported());
        assert_eq!(
            antichain.paths(),
            &[redundant_superset.clone(), path_b.clone()]
        );
        antichain
            .apply_delta(redundant_superset, -1, AntichainOverflowMode::FailClosed)
            .unwrap();
        assert_eq!(antichain.paths(), &[path_b]);

        // Retracting `path_a` a second time is refused as a lost/unmatched retraction.
        assert_eq!(
            antichain.apply_delta(path_a.clone(), -1, AntichainOverflowMode::FailClosed),
            Err(StandingOversightError::LostRetractionOrNegativeWeight)
        );

        // Weight overflow is caught by checked arithmetic.
        antichain
            .apply_delta(path_a.clone(), i64::MAX, AntichainOverflowMode::FailClosed)
            .unwrap();
        assert_eq!(
            antichain.apply_delta(path_a, 1, AntichainOverflowMode::FailClosed),
            Err(StandingOversightError::WeightOverflow)
        );
    }

    #[test]
    fn incremental_ungated_reachability_matches_batch_oracle_across_inserts_deletes_and_gates() {
        let basis = sample_basis();
        let gens = sample_generations();
        let proj = ProjectionKey {
            source: 1,
            branch: 1,
            projection: 1,
            source_epoch: 1,
        };
        let mut engine = IncrementalOversightEngine::new(
            basis,
            StandingQueryFamily::UngatedSinkReachability,
            gens,
            proj,
            8,
            AntichainOverflowMode::FailClosed,
        )
        .unwrap();

        // Frontier 1: two independent paths from actor 10 to sink 90 (via mid 20 and mid 30).
        let f1 = BaseFact {
            fact_id: 1,
            kind: BaseFactKind::AuthorityEdge { from: 10, to: 20 },
        };
        let f2 = BaseFact {
            fact_id: 2,
            kind: BaseFactKind::AuthorityEdge { from: 20, to: 90 },
        };
        let f3 = BaseFact {
            fact_id: 3,
            kind: BaseFactKind::AuthorityEdge { from: 10, to: 30 },
        };
        let f4 = BaseFact {
            fact_id: 4,
            kind: BaseFactKind::AuthorityEdge { from: 30, to: 90 },
        };

        let Knowledge::Known { value: r1, .. } = engine
            .apply_delta_and_close_frontier(
                gens,
                &[
                    SignedFactDelta {
                        fact: f1,
                        weight: 1,
                    },
                    SignedFactDelta {
                        fact: f2,
                        weight: 1,
                    },
                    SignedFactDelta {
                        fact: f3,
                        weight: 1,
                    },
                    SignedFactDelta {
                        fact: f4,
                        weight: 1,
                    },
                ],
                1,
                true,
                true,
            )
            .unwrap()
        else {
            panic!("expected known receipt");
        };

        let key = FindingKey {
            query: StandingQueryFamily::UngatedSinkReachability,
            subject: 10,
            target: 90,
        };
        assert_eq!(r1.findings[&key].minimal_supports.len(), 2);
        assert_eq!(r1.findings[&key].total_support_weight, 2);

        // Frontier 2: install a QualifiedGate on `(20, 90)`. Path via 20 is blocked,
        // but independent support via 30 keeps the finding alive!
        let g1 = BaseFact {
            fact_id: 5,
            kind: BaseFactKind::QualifiedGate { from: 20, to: 90 },
        };
        let Knowledge::Known { value: r2, .. } = engine
            .apply_delta_and_close_frontier(
                gens,
                &[SignedFactDelta {
                    fact: g1,
                    weight: 1,
                }],
                2,
                true,
                true,
            )
            .unwrap()
        else {
            panic!("expected known receipt");
        };
        assert_eq!(r2.findings[&key].minimal_supports.len(), 1);
        assert_eq!(r2.findings[&key].total_support_weight, 1);

        // Frontier 3: retract edge `f4` (`30 -> 90`). Now zero ungated paths remain.
        let Knowledge::Known { value: r3, .. } = engine
            .apply_delta_and_close_frontier(
                gens,
                &[SignedFactDelta {
                    fact: f4,
                    weight: -1,
                }],
                3,
                true,
                true,
            )
            .unwrap()
        else {
            panic!("expected known receipt");
        };
        assert!(r3.findings.is_empty());
    }

    #[test]
    fn exponential_support_fanout_coarsens_or_spills_without_false_negatives() {
        let basis = sample_basis();
        let gens = sample_generations();
        let proj = ProjectionKey {
            source: 1,
            branch: 1,
            projection: 2,
            source_epoch: 1,
        };

        // Build 6 parallel 2-hop paths between actor 1 and sink 99 while max_antichain_width = 2.
        let mut deltas = Vec::new();
        let mut next_id = 1_u64;
        for mid in 10..16 {
            deltas.push(SignedFactDelta {
                fact: BaseFact {
                    fact_id: next_id,
                    kind: BaseFactKind::AuthorityEdge { from: 1, to: mid },
                },
                weight: 1,
            });
            next_id += 1;
            deltas.push(SignedFactDelta {
                fact: BaseFact {
                    fact_id: next_id,
                    kind: BaseFactKind::AuthorityEdge { from: mid, to: 99 },
                },
                weight: 1,
            });
            next_id += 1;
        }

        // 1. ConservativeCoarsen mode preserves the finding and marks it coarsened.
        let mut coarsen_engine = IncrementalOversightEngine::new(
            basis,
            StandingQueryFamily::UngatedSinkReachability,
            gens,
            proj,
            2,
            AntichainOverflowMode::ConservativeCoarsen,
        )
        .unwrap();
        let Knowledge::Known {
            value: coarsened_receipt,
            ..
        } = coarsen_engine
            .apply_delta_and_close_frontier(gens, &deltas, 1, true, true)
            .unwrap()
        else {
            panic!("expected known receipt");
        };
        assert_eq!(
            coarsened_receipt.uncertainty,
            UncertaintyClass::Conservative
        );
        assert_eq!(coarsened_receipt.coarsened_findings, 1);
        assert_eq!(
            coarsened_receipt.require_exact_universal_support(),
            Err(StandingOversightError::CoarseSupportUniversalClaimForbidden)
        );

        // 2. SpillToBatchRecompute mode spills to exact batch evaluation with zero false negatives.
        let mut spill_engine = IncrementalOversightEngine::new(
            basis,
            StandingQueryFamily::UngatedSinkReachability,
            gens,
            proj,
            2,
            AntichainOverflowMode::SpillToBatchRecompute,
        )
        .unwrap();
        let Knowledge::Known {
            value: spill_receipt,
            ..
        } = spill_engine
            .apply_delta_and_close_frontier(gens, &deltas, 1, true, true)
            .unwrap()
        else {
            panic!("expected known receipt");
        };
        assert_eq!(spill_receipt.uncertainty, UncertaintyClass::Exact);
        assert!(spill_receipt.spilled_to_batch_recompute);
        let key = FindingKey {
            query: StandingQueryFamily::UngatedSinkReachability,
            subject: 1,
            target: 99,
        };
        assert_eq!(spill_receipt.findings[&key].minimal_supports.len(), 6);
    }

    #[test]
    fn mixed_semantic_generations_unclosed_negation_and_incomplete_frontier_are_refused() {
        let basis = sample_basis();
        let gens = sample_generations();
        let proj = ProjectionKey {
            source: 1,
            branch: 1,
            projection: 3,
            source_epoch: 1,
        };
        let mut engine = IncrementalOversightEngine::new(
            basis,
            StandingQueryFamily::UnreviewedArtifactDependency,
            gens,
            proj,
            4,
            AntichainOverflowMode::FailClosed,
        )
        .unwrap();

        let dep_fact = SignedFactDelta {
            fact: BaseFact {
                fact_id: 1,
                kind: BaseFactKind::ArtifactDependency {
                    artifact: 10,
                    dependency: 20,
                },
            },
            weight: 1,
        };

        // Unclosed negative domain refuses negation-guarded queries.
        assert_eq!(
            engine.apply_delta_and_close_frontier(gens, &[dep_fact], 1, false, true),
            Err(StandingOversightError::UnclosedNegativeDomain)
        );

        // Incomplete frontier refuses receipt emission.
        assert_eq!(
            engine.apply_delta_and_close_frontier(gens, &[dep_fact], 1, true, false),
            Err(StandingOversightError::IncompleteProductFrontier)
        );

        // Same-shape changed model or policy generation in delta stream is refused
        // until an explicit generation rebuild occurs (FA-080).
        let mut drifted_gens = gens;
        drifted_gens.model_generation += 1;
        assert_eq!(
            engine.apply_delta_and_close_frontier(drifted_gens, &[dep_fact], 1, true, true),
            Err(StandingOversightError::MixedSemanticGeneration {
                expected: gens,
                observed: drifted_gens,
            })
        );

        // Explicit generation advance rebuilds cleanly and allows subsequent deltas at the new generation.
        engine.advance_semantic_generations(drifted_gens).unwrap();
        assert_eq!(engine.generation_rebuilds(), 1);
        let Knowledge::Known { value: receipt, .. } = engine
            .apply_delta_and_close_frontier(drifted_gens, &[dep_fact], 1, true, true)
            .unwrap()
        else {
            panic!("expected known receipt");
        };
        assert_eq!(receipt.generations, drifted_gens);
        assert_eq!(receipt.generation_rebuilds, 1);
        assert_eq!(receipt.findings.len(), 1);
    }
}
