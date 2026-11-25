//! Typed, bounded reference queries over an explicitly supplied control history.
//!
//! Every sequence in this module belongs to ONE reference control-sequence
//! domain. A frontier closes that domain through its sequence; the first is one
//! and even empty boundaries are supplied. Selecting the base query profile
//! declares complete roster, autonomy and audit source domains at each cut; an
//! empty declared domain is different from an uninstalled native source. The
//! remaining families are incomplete until explicitly configured. This is a
//! caller assertion, not an
//! authenticated native journal, an independently verified label, or a permit.
//!
//! New records cannot be backdated. Exact corrections retract an old observation
//! from the active projection while retaining the complete supplied history and
//! original ID binding. Restoring that exact observation is allowed; replacing
//! its bytes under the old ID is not. Configuration never changes in an engine.
//!
//! Predicates derive their own findings and support tuples. Callers cannot feed
//! family scores. The existing checked detector maintains those tuples with
//! exact retractions. Predicate evaluation is bounded batch reference work, not
//! a claim of optimized incremental query execution. Native Tower families
//! remain Unknown until their actual source and authorization contracts exist.

use std::collections::{BTreeMap, BTreeSet};

use crate::address_resolver::{Address, Noun};

use super::{
    ControlPlaneDelta, ControlPlaneFrontier, NotActorVisible, RiskTheaterDetector,
    RiskTheaterError, RiskTheaterFamily, RiskTheaterSnapshot, RiskTheaterSupport,
    MAX_RISK_THEATER_FRONTIER_DELTAS, MAX_RISK_THEATER_HISTORY_DELTAS,
    MAX_RISK_THEATER_SUPPORTS,
};

mod basic;
pub use basic::{AuditFact, AutonomyFact, HoldLabel, ReportReference, RosterFact};

pub const MAX_QUERY_RECORDS: usize = 128;
pub const MAX_QUERY_FRONTIER_CHANGES: usize = 32;
pub const MAX_QUERY_HISTORY_CHANGES: usize = 1024;
pub const MAX_QUERY_HISTORY_FRONTIERS: usize = 1024;
pub const MAX_QUERY_COMPONENT_BYTES: usize = 64;

/// Immutable reference configuration, fully retained in every snapshot.
/// Matching the configured evaluator is an identity check on supplied records,
/// not authentication of that evaluator or proof that its label is true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryPolicy {
    scope: Address,
    generation: u64,
    evaluation_owner: Address,
    roster_window: u64,
    minimum_dwell: u64,
}

impl QueryPolicy {
    pub fn new(
        scope: Address,
        generation: u64,
        evaluation_owner: Address,
        roster_window: u64,
        minimum_dwell: u64,
    ) -> Result<Self, QueryError> {
        validate_address(&scope, &scope)?;
        validate_address(&evaluation_owner, &scope)?;
        if scope.kind != Noun::Run
            || generation == 0
            || roster_window == 0
            || minimum_dwell == 0
        {
            return Err(QueryError::InvalidPolicy);
        }
        Ok(Self {
            scope,
            generation,
            evaluation_owner,
            roster_window,
            minimum_dwell,
        })
    }

    pub fn scope(&self) -> &Address {
        &self.scope
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn evaluation_owner(&self) -> &Address {
        &self.evaluation_owner
    }
    pub fn roster_window(&self) -> u64 {
        self.roster_window
    }
    pub fn minimum_dwell(&self) -> u64 {
        self.minimum_dwell
    }
}

/// Raw typed source facts, rather than caller-authored finding scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryFact {
    Roster(RosterFact),
    Autonomy(AutonomyFact),
    Audit(AuditFact),
}

/// Full source identity: a record address is permanently bound to this exact
/// run/generation, control occurrence and fact, even after a retraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRecord {
    pub scope: Address,
    pub address: Address,
    pub occurred_at: u64,
    pub fact: HistoryFact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryChange {
    Insert(HistoryRecord),
    Retract(HistoryRecord),
}

/// Complete reference control boundary. Source sequence and control sequence
/// are deliberately the same domain here; no wall clock or native journal
/// coordinate is converted into it. Report-domain absence has its own explicit
/// assertion in ReportReference and is not inferred from this closed bit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryFrontier {
    pub sequence: u64,
    pub closed: bool,
    pub changes: Vec<HistoryChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryError {
    InvalidPolicy,
    InvalidRecord,
    InvalidReference,
    ForeignScope,
    ConflictingRecord,
    DuplicateRecord,
    MissingRetraction,
    ArithmeticOverflow,
    ResourceLimit,
    IncompleteHistory,
    UnclosedFrontier,
    NoClosedFrontier,
    UnexpectedFrontier { expected: u64, actual: u64 },
    BackdatedRecord,
    Kernel(RiskTheaterError),
}

impl From<RiskTheaterError> for QueryError {
    fn from(error: RiskTheaterError) -> Self {
        Self::Kernel(error)
    }
}

/// One predicate violation, with the exact source records that establish it.
/// Absence proofs are limited to the explicit report-domain assertion in a
/// supporting transition and the supplied closed control prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryFinding {
    pub family: RiskTheaterFamily,
    pub subject: Address,
    pub support: Vec<Address>,
    pub reason: &'static str,
}

impl NotActorVisible for QueryFinding {}

/// Complete means the selected reference predicate has all its required source
/// facts at this cut. When false, positive findings remain useful but an empty
/// vector is NOT a clean result. This type is not an actor-facing wire value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyEvaluation {
    pub family: RiskTheaterFamily,
    pub complete: bool,
    pub findings: Vec<QueryFinding>,
}

impl NotActorVisible for FamilyEvaluation {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuerySnapshot {
    sequence: u64,
    scope: Address,
    policy_generation: u64,
    families: Vec<FamilyEvaluation>,
    /// Counts proven positive predicate violations, with all their support.
    /// Zero is NOT evidence of absence for an incomplete family. The family
    /// results and immutable policy are inseparable interpretation requirements.
    kernel: RiskTheaterSnapshot,
    policy: QueryPolicy,
}

impl QuerySnapshot {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn scope(&self) -> &Address {
        &self.scope
    }
    pub fn policy_generation(&self) -> u64 {
        self.policy_generation
    }
    pub fn families(&self) -> &[FamilyEvaluation] {
        &self.families
    }
    pub fn kernel(&self) -> &RiskTheaterSnapshot {
        &self.kernel
    }
    pub fn policy(&self) -> &QueryPolicy {
        &self.policy
    }
    pub fn family(&self, family: RiskTheaterFamily) -> &FamilyEvaluation {
        self.families
            .iter()
            .find(|evaluation| evaluation.family == family)
            .expect("query engine always returns every registered family")
    }
}

impl NotActorVisible for QuerySnapshot {}

type Relation = BTreeMap<(RiskTheaterFamily, String), RiskTheaterSupport>;

/// No effect/policy authority is held or returned by this reference engine.
#[derive(Clone, Debug)]
pub struct QueryEngine {
    policy: QueryPolicy,
    sequence: u64,
    active: BTreeMap<Address, HistoryRecord>,
    identities: BTreeMap<Address, HistoryRecord>,
    history: Vec<HistoryFrontier>,
    history_changes: usize,
    relation: Relation,
    kernel: RiskTheaterDetector,
    kernel_history: Vec<ControlPlaneFrontier>,
    kernel_history_deltas: usize,
    snapshot: Option<QuerySnapshot>,
}

impl QueryEngine {
    pub fn new(policy: QueryPolicy) -> Result<Self, QueryError> {
        let thresholds = RiskTheaterFamily::ALL
            .into_iter()
            .map(|family| (family, 1))
            .collect();
        Ok(Self {
            policy,
            sequence: 0,
            active: BTreeMap::new(),
            identities: BTreeMap::new(),
            history: Vec::new(),
            history_changes: 0,
            relation: BTreeMap::new(),
            kernel: RiskTheaterDetector::new(thresholds)?,
            kernel_history: Vec::new(),
            kernel_history_deltas: 0,
            snapshot: None,
        })
    }

    pub fn policy(&self) -> &QueryPolicy {
        &self.policy
    }

    pub fn source_history(&self) -> &[HistoryFrontier] {
        &self.history
    }

    pub fn snapshot(&self, sequence: u64) -> Result<QuerySnapshot, QueryError> {
        if self.sequence == 0 {
            return Err(QueryError::NoClosedFrontier);
        }
        if sequence != self.sequence {
            return Err(QueryError::UnexpectedFrontier {
                expected: self.sequence,
                actual: sequence,
            });
        }
        self.snapshot.clone().ok_or(QueryError::NoClosedFrontier)
    }

    /// Independent verification of the generated weighted relation only.
    /// This does not independently verify the semantic predicates or authenticate
    /// the source history; their tests need separate expected outcomes.
    pub fn verify_kernel(&self) -> Result<RiskTheaterSnapshot, QueryError> {
        Ok(self.kernel.verify_against_history(&self.kernel_history)?)
    }

    /// Validate before retained cloning; stage every change, predicate result and
    /// kernel update before committing any state. Refusal preserves the previous
    /// cut and cannot be read as an empty successful result at the refused cut.
    pub fn apply_frontier(
        &mut self,
        frontier: &HistoryFrontier,
    ) -> Result<QuerySnapshot, QueryError> {
        if !frontier.closed {
            return Err(QueryError::UnclosedFrontier);
        }
        let expected = self
            .sequence
            .checked_add(1)
            .ok_or(QueryError::ArithmeticOverflow)?;
        if frontier.sequence != expected {
            return Err(QueryError::UnexpectedFrontier {
                expected,
                actual: frontier.sequence,
            });
        }
        let history_changes = self
            .history_changes
            .checked_add(frontier.changes.len())
            .ok_or(QueryError::ResourceLimit)?;
        if frontier.changes.len() > MAX_QUERY_FRONTIER_CHANGES
            || self.history.len() >= MAX_QUERY_HISTORY_FRONTIERS
            || history_changes > MAX_QUERY_HISTORY_CHANGES
        {
            return Err(QueryError::ResourceLimit);
        }
        for change in &frontier.changes {
            let record = match change {
                HistoryChange::Insert(record) | HistoryChange::Retract(record) => record,
            };
            validate_record(record, &self.policy.scope, frontier.sequence)?;
            if let HistoryChange::Insert(record) = change
                && !self.identities.contains_key(&record.address)
                && record.occurred_at != frontier.sequence
            {
                return Err(QueryError::BackdatedRecord);
            }
        }

        let mut active = self.active.clone();
        let mut identities = self.identities.clone();
        for change in &frontier.changes {
            match change {
                HistoryChange::Insert(record) => {
                    if let Some(original) = identities.get(&record.address)
                        && original != record
                    {
                        return Err(QueryError::ConflictingRecord);
                    }
                    if active.contains_key(&record.address) {
                        return Err(QueryError::DuplicateRecord);
                    }
                    identities.insert(record.address.clone(), record.clone());
                    active.insert(record.address.clone(), record.clone());
                }
                HistoryChange::Retract(record) => {
                    let Some(original) = active.get(&record.address) else {
                        return Err(QueryError::MissingRetraction);
                    };
                    if original != record {
                        return Err(QueryError::ConflictingRecord);
                    }
                    active.remove(&record.address);
                }
            }
        }
        // This is a lifetime identity bound, not only the current live count.
        // Retractions cannot buy fresh IDs or refund accepted-history work.
        if identities.len() > MAX_QUERY_RECORDS {
            return Err(QueryError::ResourceLimit);
        }
        let records: Vec<_> = active.values().cloned().collect();
        let mut families = basic::evaluate(&self.policy, &records, frontier.sequence)?;
        for family in RiskTheaterFamily::ALL {
            if !families.iter().any(|evaluation| evaluation.family == family) {
                families.push(FamilyEvaluation {
                    family,
                    complete: false,
                    findings: Vec::new(),
                });
            }
        }
        families.sort_by_key(|evaluation| evaluation.family);
        let relation = derive_relation(&mut families, &active, &self.policy.scope)?;
        let mut deltas = Vec::new();
        for (key, old) in &self.relation {
            if relation.get(key) != Some(old) {
                deltas.push(delta_for(old, frontier.sequence, -1));
            }
        }
        for (key, new) in &relation {
            if self.relation.get(key) != Some(new) {
                deltas.push(delta_for(new, frontier.sequence, 1));
            }
        }
        let kernel_history_deltas = self
            .kernel_history_deltas
            .checked_add(deltas.len())
            .ok_or(QueryError::ResourceLimit)?;
        if deltas.len() > MAX_RISK_THEATER_FRONTIER_DELTAS
            || kernel_history_deltas > MAX_RISK_THEATER_HISTORY_DELTAS
        {
            return Err(QueryError::ResourceLimit);
        }
        let kernel_frontier = ControlPlaneFrontier {
            frontier_seq: frontier.sequence,
            closed: true,
            deltas,
        };
        let mut kernel = self.kernel.clone();
        let kernel_snapshot = kernel.apply_frontier(&kernel_frontier)?;
        let snapshot = QuerySnapshot {
            sequence: frontier.sequence,
            scope: self.policy.scope.clone(),
            policy_generation: self.policy.generation,
            families,
            kernel: kernel_snapshot,
            policy: self.policy.clone(),
        };
        self.active = active;
        self.identities = identities;
        self.history.push(frontier.clone());
        self.history_changes = history_changes;
        self.relation = relation;
        self.kernel = kernel;
        self.kernel_history.push(kernel_frontier);
        self.kernel_history_deltas = kernel_history_deltas;
        self.sequence = frontier.sequence;
        self.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }
}

fn delta_for(support: &RiskTheaterSupport, sequence: u64, multiplicity: i64) -> ControlPlaneDelta {
    ControlPlaneDelta {
        frontier_seq: sequence,
        family: support.family,
        key: support.key.clone(),
        multiplicity,
        metric_delta: support.metric_value,
        support_address: support.support_address.clone(),
    }
}

fn derive_relation(
    families: &mut [FamilyEvaluation],
    records: &BTreeMap<Address, HistoryRecord>,
    scope: &Address,
) -> Result<Relation, QueryError> {
    let mut seen_families = BTreeSet::new();
    let mut relation = BTreeMap::new();
    for evaluation in families {
        if !seen_families.insert(evaluation.family) {
            return Err(QueryError::ConflictingRecord);
        }
        let mut seen_findings = BTreeSet::new();
        for finding in &mut evaluation.findings {
            validate_address(&finding.subject, scope)?;
            validate_family(finding.reason)?;
            if finding.family != evaluation.family
                || !records.contains_key(&finding.subject)
                || finding.support.is_empty()
                || !finding.support.contains(&finding.subject)
                || !seen_findings.insert((finding.subject.clone(), finding.reason))
            {
                return Err(QueryError::InvalidReference);
            }
            finding.support.sort();
            finding.support.dedup();
            for (index, address) in finding.support.iter().enumerate() {
                validate_address(address, scope)?;
                if !records.contains_key(address) {
                    return Err(QueryError::InvalidReference);
                }
                let key = format!(
                    "{}:{}:{}:{index}",
                    finding.family as u8, finding.subject, finding.reason
                );
                if key.len() > super::MAX_RISK_THEATER_COMPONENT_BYTES {
                    return Err(QueryError::ResourceLimit);
                }
                relation.insert(
                    (finding.family, key.clone()),
                    RiskTheaterSupport {
                        family: finding.family,
                        key,
                        multiplicity: 1,
                        metric_value: i64::from(index == 0),
                        support_address: address.clone(),
                    },
                );
                if relation.len() > MAX_RISK_THEATER_SUPPORTS {
                    return Err(QueryError::ResourceLimit);
                }
            }
        }
        evaluation
            .findings
            .sort_by(|left, right| (&left.subject, left.reason).cmp(&(&right.subject, right.reason)));
    }
    Ok(relation)
}

fn validate_record(record: &HistoryRecord, scope: &Address, sequence: u64) -> Result<(), QueryError> {
    if &record.scope != scope {
        return Err(QueryError::ForeignScope);
    }
    validate_address(&record.address, scope)?;
    if record.occurred_at == 0 || record.occurred_at > sequence {
        return Err(QueryError::InvalidRecord);
    }
    basic::validate_fact(record, scope)
}

pub(super) fn validate_address(address: &Address, scope: &Address) -> Result<(), QueryError> {
    if address.tenant != scope.tenant {
        return Err(QueryError::ForeignScope);
    }
    validate_family(&address.tenant)?;
    validate_family(&address.id)?;
    if address.generation.is_none_or(|generation| generation == 0) {
        return Err(QueryError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn validate_family(value: &str) -> Result<(), QueryError> {
    if value.is_empty()
        || value.len() > MAX_QUERY_COMPONENT_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(QueryError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn find_record<'a>(
    records: &'a [HistoryRecord],
    address: &Address,
) -> Option<&'a HistoryRecord> {
    records.iter().find(|record| &record.address == address)
}
