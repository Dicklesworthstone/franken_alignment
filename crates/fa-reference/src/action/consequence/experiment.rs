//! Capability-free, paired policy experiments over verified decision archives.
//!
//! This L7 reference model changes only registered action fields and observed
//! keys. It does not execute a model, simulate the world, copy live rights, or
//! carry old helper approval across changed inputs. Supplied archive provenance
//! and the separately retained anchor remain the replay verifier's assumptions.

mod search;
pub use search::{MAX_SEARCH_CANDIDATES, Minimality, RepairSearch, SearchCase, SufficientRepair};

use super::gate::containment::session::policy::controller::replay::{
    DecisionArchive, ReplayedDecision, ReviewAnchor,
};
use super::gate::containment::session::policy::{Policy, Predicate, Truth};
use crate::action::{
    FrozenAction, MAX_PAYLOAD_BYTES, MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES, Purpose,
    ResolvedTarget,
};
use crate::{Error, ReadWitness};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_INTERVENTIONS: usize = 64;
pub const MAX_INTERVENTION_BYTES: usize = 65_536;

/// Frozen experiment registration. This describes editable data, not live
/// capabilities; the experiment has no gate, adapter, controller or Permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterventionScope {
    id: u64,
    payload: bool,
    target: bool,
    units: bool,
    keys: BTreeSet<u64>,
}

impl InterventionScope {
    pub fn new(
        id: u64,
        payload: bool,
        target: bool,
        units: bool,
        keys: &[u64],
    ) -> Result<Self, Error> {
        if id == 0 {
            return Err(Error::InvalidInput);
        }
        if keys.len() > MAX_INTERVENTIONS {
            return Err(Error::Limit);
        }
        let unique: BTreeSet<_> = keys.iter().copied().collect();
        if unique.len() != keys.len() {
            return Err(Error::Duplicate);
        }
        Ok(Self { id, payload, target, units, keys: unique })
    }
}

/// Key edits name their expected baseline value. None is an observed absence,
/// never a wildcard. Deleting an observed value does not close unseen ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intervention {
    Payload(Vec<u8>),
    Target(ResolvedTarget),
    Units(u64),
    Key {
        key: u64,
        expected: Option<Vec<u8>>,
        replacement: Option<Vec<u8>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextRequirement {
    FreshIndependentReview,
    ExactPolicyViolation,
    MoreEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmpiricalStatus {
    /// The unedited archive can describe history, never renew live permission.
    HistoricalOnly,
    /// The old congress judged different inputs; no new votes were simulated.
    InvalidatedByIntervention,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeChange {
    pub node: usize,
    pub baseline: Truth,
    pub counterfactual: Truth,
}

/// A report cannot enter the permit path.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::experiment::CounterfactualReport;
/// use fa_reference::action::Permit;
/// fn cannot_authorize(report: CounterfactualReport) -> Permit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterfactualReport {
    scope: u64,
    attempt: u64,
    round: u64,
    baseline: Truth,
    counterfactual: Truth,
    trace: Vec<Truth>,
    changes: Vec<NodeChange>,
    interventions: Vec<Intervention>,
    next: NextRequirement,
    empirical: EmpiricalStatus,
}

impl CounterfactualReport {
    pub fn scope(&self) -> u64 { self.scope }
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn round(&self) -> u64 { self.round }
    pub fn baseline(&self) -> Truth { self.baseline }
    pub fn counterfactual(&self) -> Truth { self.counterfactual }
    pub fn trace(&self) -> &[Truth] { &self.trace }
    pub fn changes(&self) -> &[NodeChange] { &self.changes }
    pub fn interventions(&self) -> &[Intervention] { &self.interventions }
    pub fn next_requirement(&self) -> NextRequirement { self.next }
    pub fn empirical_status(&self) -> EmpiricalStatus { self.empirical }
}

/// Stores data only. There is no constructor from a controller, checkpoint
/// authority, Permit, or caller-authored "successful" evaluation.
#[derive(Clone, Debug)]
pub struct PolicyExperiment {
    anchor: ReviewAnchor,
    baseline: ReplayedDecision,
    evidence: EvidenceSlice,
    scope: InterventionScope,
}

impl PolicyExperiment {
    pub fn from_archive(
        archive: &DecisionArchive,
        expected: &ReviewAnchor,
        scope: InterventionScope,
    ) -> Result<Self, Error> {
        let baseline = archive.verify(expected)?;
        let evidence = EvidenceSlice::new(&expected.observations)?;
        for key in &scope.keys {
            if evidence.lookup(*key).is_none() {
                return Err(Error::Incomplete);
            }
        }
        // Cross-check the partial-domain evaluator against the original replay
        // before admitting interventions. No omitted provider rows are invented.
        let trace = evidence.evaluate(&expected.policy, &expected.action)?;
        if trace.len() != baseline.evaluation().trace().len()
            || trace.iter().zip(baseline.evaluation().trace())
                .any(|(result, step)| *result != step.result)
        {
            return Err(Error::Binding);
        }
        Ok(Self { anchor: expected.clone(), baseline, evidence, scope })
    }

    pub fn baseline(&self) -> &ReplayedDecision { &self.baseline }

    /// Each branch starts from the immutable baseline, not the previous branch.
    /// Errors leave it intact. Even an empty/no-op branch is historical data,
    /// and an exact-policy repair still requires a new independent congress.
    pub fn run(&self, edits: &[Intervention]) -> Result<CounterfactualReport, Error> {
        validate_edit_bounds(edits)?;
        let mut spec = self.anchor.action.spec().clone();
        spec.scope.purpose = Purpose::Experiment;
        spec.required_witnesses.clear();
        let mut evidence = self.evidence.clone();
        let mut selected = BTreeSet::new();
        let mut changed = false;
        for edit in edits {
            let slot = match edit {
                Intervention::Payload(_) => (0_u8, 0),
                Intervention::Target(_) => (1, 0),
                Intervention::Units(_) => (2, 0),
                Intervention::Key { key, .. } => (3, *key),
            };
            if !selected.insert(slot) {
                return Err(Error::Duplicate);
            }
            match edit {
                Intervention::Payload(payload) => {
                    if !self.scope.payload { return Err(Error::Binding); }
                    changed |= spec.payload != *payload;
                    spec.payload = payload.clone();
                }
                Intervention::Target(target) => {
                    if !self.scope.target { return Err(Error::Binding); }
                    changed |= spec.target != Some(*target);
                    spec.target = Some(*target);
                }
                Intervention::Units(units) => {
                    if !self.scope.units { return Err(Error::Binding); }
                    changed |= spec.units != *units;
                    spec.units = *units;
                }
                Intervention::Key { key, expected, replacement } => {
                    if !self.scope.keys.contains(key) { return Err(Error::Binding); }
                    let actual = self.evidence.lookup(*key).ok_or(Error::Incomplete)?;
                    if actual != expected.as_deref() { return Err(Error::Stale); }
                    changed |= expected != replacement;
                    evidence.values.insert(*key, replacement.clone());
                }
            }
        }
        evidence.check_bytes()?;
        let action = FrozenAction::freeze(spec)?;
        let trace = evidence.evaluate(&self.anchor.policy, &action)?;
        let result = *trace.last().ok_or(Error::InvalidInput)?;
        let changes = trace.iter().zip(self.baseline.evaluation().trace())
            .enumerate().filter_map(|(node, (after, before))| {
                (*after != before.result).then_some(NodeChange {
                    node, baseline: before.result, counterfactual: *after,
                })
            }).collect();
        let next = match result {
            Truth::Violated => NextRequirement::ExactPolicyViolation,
            Truth::Satisfied if !trace.contains(&Truth::Unknown) => {
                NextRequirement::FreshIndependentReview
            }
            _ => NextRequirement::MoreEvidence,
        };
        Ok(CounterfactualReport {
            scope: self.scope.id,
            attempt: self.anchor.attempt,
            round: self.anchor.round,
            baseline: self.baseline.evaluation().result(),
            counterfactual: result,
            trace,
            changes,
            interventions: edits.to_vec(),
            next,
            empirical: if changed {
                EmpiricalStatus::InvalidatedByIntervention
            } else {
                EmpiricalStatus::HistoricalOnly
            },
        })
    }
}

fn validate_edit_bounds(edits: &[Intervention]) -> Result<(), Error> {
    if edits.len() > MAX_INTERVENTIONS { return Err(Error::Limit); }
    let mut bytes = 0_usize;
    for edit in edits {
        let size = match edit {
            Intervention::Payload(value) => {
                if value.len() > MAX_PAYLOAD_BYTES { return Err(Error::Limit); }
                value.len()
            }
            Intervention::Key { expected, replacement, .. } => {
                expected.as_ref().map_or(0, Vec::len)
                    .checked_add(replacement.as_ref().map_or(0, Vec::len))
                    .ok_or(Error::Limit)?
            }
            _ => 0,
        };
        bytes = bytes.checked_add(size).ok_or(Error::Limit)?;
        if bytes > MAX_INTERVENTION_BYTES { return Err(Error::Limit); }
    }
    Ok(())
}

/// Exact facts plus closed observed domains. Closure survives a value edit but
/// does not expand. A positive range witness closes only its own singleton.
#[derive(Clone, Debug)]
struct EvidenceSlice {
    values: BTreeMap<u64, Option<Vec<u8>>>,
    closed: Vec<(u64, u64)>,
}

impl EvidenceSlice {
    fn new(observations: &[ReadWitness]) -> Result<Self, Error> {
        if observations.len() > MAX_REQUIRED_WITNESSES { return Err(Error::Limit); }
        let mut values = BTreeMap::new();
        let mut closed = Vec::new();
        let mut empty = Vec::new();
        let mut bytes = 0_usize;
        for observation in observations {
            match observation {
                ReadWitness::Exact { key, value } => {
                    bytes = bytes.checked_add(value.as_ref().map_or(0, Vec::len))
                        .ok_or(Error::Limit)?;
                    if bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
                    if values.get(key).is_some_and(|previous| previous != value) {
                        return Err(Error::Binding);
                    }
                    values.insert(*key, value.clone());
                    if let Some(end) = key.checked_add(1) {
                        closed.push((*key, end));
                    }
                }
                ReadWitness::EmptyRange { start, end } => {
                    if start >= end { return Err(Error::InvalidInput); }
                    closed.push((*start, *end));
                    empty.push((*start, *end));
                }
            }
        }
        if values.iter().any(|(key, value)| {
            value.is_some() && empty.iter().any(|(start, end)| start <= key && key < end)
        }) {
            return Err(Error::Binding);
        }
        closed.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::new();
        for (start, end) in closed {
            if let Some(last) = merged.last_mut() {
                if start <= last.1 {
                    last.1 = last.1.max(end);
                    continue;
                }
            }
            merged.push((start, end));
        }
        Ok(Self { values, closed: merged })
    }

    /// Outer None is unobserved; Some(None) is a known absence.
    fn lookup(&self, key: u64) -> Option<Option<&[u8]>> {
        if let Some(value) = self.values.get(&key) {
            return Some(value.as_deref());
        }
        self.closed.iter().any(|(start, end)| *start <= key && key < *end)
            .then_some(None)
    }

    fn check_bytes(&self) -> Result<(), Error> {
        let mut bytes = 0_usize;
        for value in self.values.values().flatten() {
            bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
            if bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
        }
        Ok(())
    }

    fn range_truth(&self, start: u64, end: u64) -> Truth {
        if self.values.range(start..end).any(|(_, value)| value.is_some()) {
            return Truth::Violated;
        }
        if self.closed.iter().any(|(left, right)| *left <= start && end <= *right) {
            Truth::Satisfied
        } else {
            Truth::Unknown
        }
    }

    fn evaluate(&self, policy: &Policy, action: &FrozenAction) -> Result<Vec<Truth>, Error> {
        let mut trace = Vec::with_capacity(policy.nodes().len());
        let mut bytes = 0_usize;
        for node in policy.nodes() {
            // Charge repeated read occurrences, not merely unique stored bytes.
            let observed = match node {
                Predicate::ExactValue { key, .. } | Predicate::Absent { key } => {
                    self.lookup(*key).flatten()
                }
                Predicate::EmptyRange { start, end } => {
                    self.values.range(*start..*end).find_map(|(_, value)| value.as_deref())
                }
                _ => None,
            };
            bytes = bytes.checked_add(observed.map_or(0, <[u8]>::len)).ok_or(Error::Limit)?;
            if bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            let value = match node {
                Predicate::TargetIs(target) => truth(action.spec().target == Some(*target)),
                Predicate::PayloadIs(payload) => truth(action.spec().payload == *payload),
                Predicate::PayloadAtMost(limit) => truth(action.spec().payload.len() <= *limit),
                Predicate::UnitsAtMost(limit) => truth(action.spec().units <= *limit),
                Predicate::ExactValue { key, value } => match self.lookup(*key) {
                    Some(actual) => truth(actual == Some(value.as_slice())),
                    None => Truth::Unknown,
                },
                Predicate::Absent { key } => match self.lookup(*key) {
                    Some(actual) => truth(actual.is_none()),
                    None => Truth::Unknown,
                },
                Predicate::EmptyRange { start, end } => self.range_truth(*start, *end),
                Predicate::All(children) => {
                    if children.iter().any(|child| trace[*child] == Truth::Violated) {
                        Truth::Violated
                    } else if children.iter().any(|child| trace[*child] == Truth::Unknown) {
                        Truth::Unknown
                    } else {
                        Truth::Satisfied
                    }
                }
                Predicate::Any(children) => {
                    if children.iter().any(|child| trace[*child] == Truth::Satisfied) {
                        Truth::Satisfied
                    } else if children.iter().any(|child| trace[*child] == Truth::Unknown) {
                        Truth::Unknown
                    } else {
                        Truth::Violated
                    }
                }
                Predicate::Not(child) => match trace[*child] {
                    Truth::Satisfied => Truth::Violated,
                    Truth::Violated => Truth::Satisfied,
                    Truth::Unknown => Truth::Unknown,
                },
            };
            trace.push(value);
        }
        Ok(trace)
    }
}

fn truth(value: bool) -> Truth {
    if value { Truth::Satisfied } else { Truth::Violated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_of_the_only_seen_member_does_not_close_a_range() {
        let mut evidence = EvidenceSlice::new(&[
            ReadWitness::Exact { key: 15, value: Some(vec![1]) },
        ]).unwrap();
        assert_eq!(evidence.range_truth(10, 20), Truth::Violated);
        evidence.values.insert(15, None);
        assert_eq!(evidence.range_truth(10, 20), Truth::Unknown);
        assert_eq!(evidence.range_truth(15, 16), Truth::Satisfied);
        assert_eq!(evidence.lookup(14), None);
        assert_eq!(evidence.lookup(15), Some(None));
    }

    #[test]
    fn adjacent_closed_domains_merge_without_filling_a_gap() {
        let mut evidence = EvidenceSlice::new(&[
            ReadWitness::EmptyRange { start: 10, end: 14 },
            ReadWitness::Exact { key: 14, value: None },
            ReadWitness::EmptyRange { start: 15, end: 20 },
            ReadWitness::EmptyRange { start: 21, end: 25 },
        ]).unwrap();
        assert_eq!(evidence.range_truth(10, 20), Truth::Satisfied);
        assert_eq!(evidence.range_truth(10, 25), Truth::Unknown);
        evidence.values.insert(12, Some(vec![9]));
        assert_eq!(evidence.range_truth(10, 20), Truth::Violated);
        evidence.values.insert(12, None);
        assert_eq!(evidence.range_truth(10, 20), Truth::Satisfied);
    }

    #[test]
    fn contradictory_observations_are_not_repaired_by_last_write_wins() {
        assert!(matches!(EvidenceSlice::new(&[
            ReadWitness::Exact { key: 1, value: None },
            ReadWitness::Exact { key: 1, value: Some(vec![1]) },
        ]), Err(Error::Binding)));
        assert!(matches!(EvidenceSlice::new(&[
            ReadWitness::Exact { key: 1, value: Some(vec![1]) },
            ReadWitness::EmptyRange { start: 0, end: 2 },
        ]), Err(Error::Binding)));
    }

    #[test]
    fn maximum_key_is_exact_without_an_overflowing_successor() {
        let evidence = EvidenceSlice::new(&[
            ReadWitness::Exact { key: u64::MAX, value: Some(vec![1]) },
            ReadWitness::EmptyRange { start: u64::MAX - 2, end: u64::MAX },
        ]).unwrap();
        assert_eq!(evidence.lookup(u64::MAX), Some(Some(&[1][..])));
        assert_eq!(evidence.lookup(u64::MAX - 1), Some(None));
        assert_eq!(evidence.range_truth(u64::MAX - 2, u64::MAX), Truth::Satisfied);
    }

    #[test]
    fn evidence_bounds_count_duplicate_retained_occurrences() {
        let value = vec![1; MAX_WITNESS_BYTES / 2];
        let observation = ReadWitness::Exact { key: 1, value: Some(value) };
        assert!(EvidenceSlice::new(&[observation.clone(), observation.clone()]).is_ok());
        assert!(matches!(EvidenceSlice::new(&[
            observation.clone(), observation.clone(), observation,
        ]), Err(Error::Limit)));
        let empty = ReadWitness::Exact { key: 0, value: None };
        assert!(EvidenceSlice::new(&vec![empty.clone(); MAX_REQUIRED_WITNESSES]).is_ok());
        assert!(matches!(EvidenceSlice::new(&vec![
            empty; MAX_REQUIRED_WITNESSES + 1
        ]), Err(Error::Limit)));
    }

    #[test]
    fn intervention_limits_are_checked_before_copying_input_bytes() {
        assert!(validate_edit_bounds(&[
            Intervention::Payload(vec![0; MAX_INTERVENTION_BYTES]),
        ]).is_ok());
        assert_eq!(validate_edit_bounds(&[
            Intervention::Payload(vec![0; MAX_INTERVENTION_BYTES]),
            Intervention::Key { key: 0, expected: None, replacement: Some(vec![1]) },
        ]), Err(Error::Limit));
        assert_eq!(validate_edit_bounds(&vec![
            Intervention::Units(1); MAX_INTERVENTIONS + 1
        ]), Err(Error::Limit));
        assert_eq!(InterventionScope::new(1, false, false, false, &[1, 1]), Err(Error::Duplicate));
    }
}
