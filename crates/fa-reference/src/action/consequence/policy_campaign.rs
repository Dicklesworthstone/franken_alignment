//! Exact-policy replay on retained observations, never a simulated helper vote.
//!
//! A candidate may use only observed keys and closed ranges. The ordinary policy
//! evaluator is reused only after that coverage check; an omitted key must not
//! become an invented absence. Reports neither issue permits nor change policy.

use super::Consequence;
use super::gate::containment::session::policy::{Evaluation, Policy, Predicate, Truth};
use super::gate::containment::session::policy::controller::{DecisionArchive, ReviewAnchor};
use crate::action::{FrozenAction, MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES};
use crate::{Error, Judgment, ReadWitness, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_REPLAY_CASES: usize = 5_120;
pub const MAX_REPLAY_INPUT_BYTES: usize = 32 * 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayLimits {
    pub cases: usize,
    /// Action payload and witness values, counting repeated retained occurrences.
    /// This is not allocator/peak memory or a measured execution-time budget.
    pub input_bytes: usize,
}

impl ReplayLimits {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if self.cases == 0 || self.input_bytes == 0 { return Err(Error::InvalidInput); }
        if self.cases > MAX_REPLAY_CASES || self.input_bytes > MAX_REPLAY_INPUT_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Proposal and later review observations are separate cases, even for one action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplayCaseId {
    Proposal(u64),
    Review { attempt: u64, round: u64, control_sequence: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDelta {
    Unchanged,
    NewlyBlocked,
    /// Exact predicates would now pass. No old helper approval is transferred.
    NewlyReviewable,
    RequiresShadow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReplayCase {
    id: ReplayCaseId,
    action: FrozenAction,
    original: Evaluation,
    candidate: Option<Evaluation>,
    missing_nodes: Vec<usize>,
    original_consequence: Option<Consequence>,
    snapshot_semantic_epoch: u64,
    delta: PolicyDelta,
}

impl PolicyReplayCase {
    pub fn id(&self) -> ReplayCaseId { self.id }
    pub fn action(&self) -> &FrozenAction { &self.action }
    pub fn original(&self) -> &Evaluation { &self.original }
    pub fn candidate(&self) -> Option<&Evaluation> { self.candidate.as_ref() }
    pub fn missing_nodes(&self) -> &[usize] { &self.missing_nodes }
    pub fn original_consequence(&self) -> Option<Consequence> { self.original_consequence }
    pub fn snapshot_semantic_epoch(&self) -> u64 { self.snapshot_semantic_epoch }
    pub fn delta(&self) -> PolicyDelta { self.delta }
}

/// A complete comparison of the supplied corpus, not a population safety claim.
/// Live promotion uses an owning broker's corpus, not caller-selected archives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReplayReport {
    previous: Rc<Policy>,
    candidate: Rc<Policy>,
    cases: Vec<PolicyReplayCase>,
    input_bytes: usize,
}

impl PolicyReplayReport {
    pub fn previous_policy(&self) -> &Policy { &self.previous }
    pub fn candidate_policy(&self) -> &Policy { &self.candidate }
    pub fn cases(&self) -> &[PolicyReplayCase] { &self.cases }
    pub fn input_bytes(&self) -> usize { self.input_bytes }
    pub fn requires_shadow(&self) -> bool {
        self.cases.iter().any(|case| case.delta == PolicyDelta::RequiresShadow)
    }
    pub fn newly_reviewable(&self) -> Vec<ReplayCaseId> {
        self.cases.iter().filter(|case| case.delta == PolicyDelta::NewlyReviewable)
            .map(|case| case.id).collect()
    }
    pub fn newly_blocked(&self) -> Vec<ReplayCaseId> {
        self.cases.iter().filter(|case| case.delta == PolicyDelta::NewlyBlocked)
            .map(|case| case.id).collect()
    }

    /// Offline comparison. Anchors must be retained independently, as required
    /// by DecisionArchive. A fabricated anchor is not authenticated here.
    /// The returned report has no live promotion capability.
    pub fn from_archives(
        previous: &Policy, candidate: Policy,
        archives: &[(&DecisionArchive, &ReviewAnchor)], limits: ReplayLimits,
    ) -> Result<Self, Error> {
        let mut builder = ReplayBuilder::new(previous, candidate, limits)?;
        if archives.len() > limits.cases { return Err(Error::Limit); }
        for (archive, anchor) in archives {
            if &anchor.policy != previous { return Err(Error::Binding); }
            let replayed = archive.verify(anchor)?;
            let sequence = anchor.expected_control_sequence.checked_add(1).ok_or(Error::Overflow)?;
            builder.push(
                ReplayCaseId::Review { attempt: anchor.attempt, round: anchor.round, control_sequence: sequence },
                &anchor.action, replayed.evaluation(), anchor.snapshot_semantic_epoch,
                Some(replayed.decision().consequence),
            )?;
        }
        builder.finish()
    }
}

pub(crate) struct ReplayBuilder {
    report: PolicyReplayReport,
    limits: ReplayLimits,
    ids: BTreeSet<ReplayCaseId>,
}

impl ReplayBuilder {
    pub(crate) fn new(previous: &Policy, candidate: Policy, limits: ReplayLimits) -> Result<Self, Error> {
        limits.validate()?;
        if candidate.generation() <= previous.generation() { return Err(Error::Stale); }
        Ok(Self {
            report: PolicyReplayReport { previous: Rc::new(previous.clone()),
                candidate: Rc::new(candidate), cases: Vec::new(), input_bytes: 0 },
            limits, ids: BTreeSet::new(),
        })
    }

    pub(crate) fn push(
        &mut self, id: ReplayCaseId, action: &FrozenAction, original: &Evaluation,
        semantic_epoch: u64, original_consequence: Option<Consequence>,
    ) -> Result<(), Error> {
        if self.ids.contains(&id) { return Err(Error::Duplicate); }
        if self.report.cases.len() >= self.limits.cases { return Err(Error::Limit); }
        if original.generation() != self.report.previous.generation()
            || original.result() == Truth::Unknown
            || original.trace().iter().any(|step| step.result == Truth::Unknown)
        { return Err(Error::Binding); }
        let before_bytes = self.report.input_bytes;
        let mut bytes = before_bytes.checked_add(action.spec().payload.len()).ok_or(Error::Limit)?;
        bytes = bytes.checked_add(witness_bytes(&action.spec().required_witnesses)?).ok_or(Error::Limit)?;
        bytes = bytes.checked_add(witness_bytes(original.witnesses())?).ok_or(Error::Limit)?;
        if bytes > self.limits.input_bytes { return Err(Error::Limit); }
        let observed = ObservedSlice::new(original.witnesses(), semantic_epoch)?;
        if !observed.missing_nodes(&self.report.previous).is_empty()
            || self.report.previous.evaluate(action, &observed.snapshot)? != *original
        { return Err(Error::Binding); }
        let missing_nodes = observed.missing_nodes(&self.report.candidate);
        let candidate = if missing_nodes.is_empty() {
            Some(self.report.candidate.evaluate(action, &observed.snapshot)?)
        } else { None };
        // The result owns candidate witness occurrences too. Charge before
        // retaining this case; an exhausted campaign never returns a prefix report.
        if let Some(evaluation) = &candidate {
            bytes = bytes.checked_add(witness_bytes(evaluation.witnesses())?).ok_or(Error::Limit)?;
        }
        if bytes > self.limits.input_bytes { return Err(Error::Limit); }
        let delta = match candidate.as_ref().map(Evaluation::result) {
            None | Some(Truth::Unknown) => PolicyDelta::RequiresShadow,
            Some(result) if result == original.result() => PolicyDelta::Unchanged,
            Some(Truth::Satisfied) => PolicyDelta::NewlyReviewable,
            Some(Truth::Violated) => PolicyDelta::NewlyBlocked,
        };
        self.report.cases.push(PolicyReplayCase { id, action: action.clone(), original: original.clone(),
            candidate, missing_nodes, original_consequence, snapshot_semantic_epoch: semantic_epoch, delta });
        self.ids.insert(id);
        self.report.input_bytes = bytes;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<PolicyReplayReport, Error> {
        if self.report.cases.is_empty() { return Err(Error::Incomplete); }
        Ok(self.report)
    }
}

fn witness_bytes(witnesses: &[ReadWitness]) -> Result<usize, Error> {
    if witnesses.len() > MAX_REQUIRED_WITNESSES { return Err(Error::Limit); }
    let mut bytes = 0_usize;
    for witness in witnesses {
        if let ReadWitness::Exact { value: Some(value), .. } = witness {
            bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
        }
    }
    if bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
    Ok(bytes)
}

/// A scoped view, never a full provider database. No candidate evaluator sees
/// this snapshot until every read is supported by retained positive/negative
/// observations. Unsupported reads in *any* Boolean arm require shadow work.
struct ObservedSlice {
    snapshot: Snapshot,
    exact: BTreeSet<u64>,
    closed: Vec<(u64, u64)>,
}

impl ObservedSlice {
    fn new(witnesses: &[ReadWitness], semantic_epoch: u64) -> Result<Self, Error> {
        witness_bytes(witnesses)?;
        let mut exact = BTreeMap::new();
        let mut intervals = Vec::new();
        for witness in witnesses {
            match witness {
                ReadWitness::Exact { key, value } => {
                    if exact.get(key).is_some_and(|previous| previous != value) { return Err(Error::Binding); }
                    exact.insert(*key, value.clone());
                    if let Some(end) = key.checked_add(1) { intervals.push((*key, end)); }
                }
                ReadWitness::EmptyRange { start, end } => {
                    if start >= end { return Err(Error::InvalidInput); }
                    intervals.push((*start, *end));
                }
            }
        }
        let snapshot = Snapshot { semantic_epoch, complete: true,
            values: exact.iter().filter_map(|(key, value)| value.clone().map(|value| (*key, value))).collect() };
        Judgment::capture(&snapshot, witnesses.to_vec())?;
        intervals.sort_unstable();
        let mut closed: Vec<(u64, u64)> = Vec::new();
        for (start, end) in intervals {
            if let Some(last) = closed.last_mut() {
                if start <= last.1 { last.1 = last.1.max(end); continue; }
            }
            closed.push((start, end));
        }
        Ok(Self { snapshot, exact: exact.keys().copied().collect(), closed })
    }

    fn missing_nodes(&self, policy: &Policy) -> Vec<usize> {
        policy.nodes().iter().enumerate().filter_map(|(index, node)| {
            let covered = match node {
                Predicate::ExactValue { key, .. } | Predicate::Absent { key } =>
                    self.exact.contains(key) || self.closed.iter().any(|(start, end)| start <= key && key < end),
                Predicate::EmptyRange { start, end } =>
                    self.snapshot.values.range(*start..*end).next().is_some()
                        || self.closed.iter().any(|(left, right)| left <= start && end <= right),
                Predicate::TargetIs(_) | Predicate::PayloadIs(_) | Predicate::PayloadAtMost(_)
                    | Predicate::UnitsAtMost(_) | Predicate::All(_) | Predicate::Any(_) | Predicate::Not(_) => true,
            };
            (!covered).then_some(index)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};

    fn action() -> FrozenAction {
        FrozenAction::freeze(ActionSpec { version: VERSION,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(100), units: 10 }).unwrap()
    }
    fn policy(generation: u64, nodes: Vec<Predicate>) -> Policy { Policy::new(generation, nodes).unwrap() }
    fn limits() -> ReplayLimits { ReplayLimits { cases: 8, input_bytes: 1_048_576 } }
    fn compare(old: Policy, next: Policy, values: BTreeMap<u64, Vec<u8>>) -> PolicyReplayReport {
        let action = action();
        let observed = old.evaluate(&action, &Snapshot { semantic_epoch: 7, complete: true, values }).unwrap();
        let mut builder = ReplayBuilder::new(&old, next, limits()).unwrap();
        builder.push(ReplayCaseId::Proposal(1), &action, &observed, 7, None).unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn new_absence_is_shadow_not_an_invented_empty_provider() {
        let old = policy(1, vec![Predicate::ExactValue { key: 1, value: vec![1] }]);
        let next = policy(2, vec![Predicate::Absent { key: 2 }]);
        let report = compare(old, next, BTreeMap::from([(1, vec![1]), (2, vec![9])]));
        assert!(report.requires_shadow());
        assert_eq!(report.cases()[0].missing_nodes(), &[0]);
        assert!(report.cases()[0].candidate().is_none());
    }

    #[test]
    fn boolean_success_cannot_hide_an_unobserved_dependency() {
        let report = compare(policy(1, vec![Predicate::PayloadAtMost(100)]), policy(2, vec![
            Predicate::PayloadAtMost(100), Predicate::Absent { key: 99 }, Predicate::Any(vec![0, 1]),
        ]), BTreeMap::new());
        assert_eq!(report.cases()[0].missing_nodes(), &[1]);
        assert!(report.newly_reviewable().is_empty());
    }

    #[test]
    fn closed_subranges_and_adjacent_singletons_support_new_reads() {
        let report = compare(policy(1, vec![Predicate::EmptyRange { start: 10, end: 14 },
            Predicate::Absent { key: 14 }, Predicate::EmptyRange { start: 15, end: 20 }, Predicate::All(vec![0, 1, 2])]),
            policy(2, vec![Predicate::EmptyRange { start: 11, end: 20 }]), BTreeMap::new());
        assert!(!report.requires_shadow());
        assert_eq!(report.cases()[0].candidate().unwrap().result(), Truth::Satisfied);
    }

    #[test]
    fn a_known_positive_member_refutes_a_wider_empty_range_without_closing_it() {
        let report = compare(policy(1, vec![Predicate::ExactValue { key: 15, value: vec![3] }]),
            policy(2, vec![Predicate::EmptyRange { start: 0, end: 100 }]), BTreeMap::from([(15, vec![3])]));
        assert_eq!(report.newly_blocked(), vec![ReplayCaseId::Proposal(1)]);
        assert_eq!(report.cases()[0].candidate().unwrap().witnesses(), &[ReadWitness::Exact { key: 15, value: Some(vec![3]) }]);
    }

    #[test]
    fn denied_examples_remain_in_the_newly_reviewable_set() {
        let report = compare(policy(1, vec![Predicate::PayloadAtMost(2)]),
            policy(2, vec![Predicate::PayloadAtMost(100)]), BTreeMap::new());
        assert_eq!(report.newly_reviewable(), vec![ReplayCaseId::Proposal(1)]);
        assert_eq!(report.cases()[0].original().result(), Truth::Violated);
    }

    #[test]
    fn maximum_key_and_unobserved_range_gap_are_distinct() {
        let report = compare(policy(1, vec![Predicate::Absent { key: u64::MAX }]),
            policy(2, vec![Predicate::ExactValue { key: u64::MAX, value: Vec::new() }]), BTreeMap::new());
        assert_eq!(report.newly_blocked(), vec![ReplayCaseId::Proposal(1)]);
        let report = compare(policy(1, vec![Predicate::EmptyRange { start: 1, end: 4 },
            Predicate::EmptyRange { start: 5, end: 9 }, Predicate::All(vec![0, 1])]),
            policy(2, vec![Predicate::EmptyRange { start: 1, end: 9 }]), BTreeMap::new());
        assert!(report.requires_shadow());
    }

    #[test]
    fn duplicate_and_over_budget_cases_cannot_publish_partial_campaigns() {
        let old = policy(1, vec![Predicate::PayloadAtMost(100)]);
        let observed = old.evaluate(&action(), &Snapshot { complete: true, ..Snapshot::default() }).unwrap();
        let mut builder = ReplayBuilder::new(&old, policy(2, vec![Predicate::PayloadAtMost(100)]),
            ReplayLimits { cases: 1, input_bytes: 7 }).unwrap();
        assert_eq!(builder.push(ReplayCaseId::Proposal(1), &action(), &observed, 0, None), Ok(()));
        assert_eq!(builder.push(ReplayCaseId::Proposal(1), &action(), &observed, 0, None), Err(Error::Duplicate));
        assert_eq!(builder.push(ReplayCaseId::Proposal(2), &action(), &observed, 0, None), Err(Error::Limit));
        let mut builder = ReplayBuilder::new(&old, policy(2, vec![Predicate::PayloadAtMost(100)]),
            ReplayLimits { cases: 1, input_bytes: 6 }).unwrap();
        assert_eq!(builder.push(ReplayCaseId::Proposal(1), &action(), &observed, 0, None), Err(Error::Limit));
        assert!(matches!(builder.finish(), Err(Error::Incomplete)));
    }

    #[test]
    fn baseline_tampering_and_empty_campaigns_refuse() {
        let old = policy(1, vec![Predicate::PayloadAtMost(100)]);
        let observed = old.evaluate(&action(), &Snapshot { complete: true, ..Snapshot::default() }).unwrap();
        let mut spec = action().spec().clone(); spec.payload = vec![0; 101];
        let mut builder = ReplayBuilder::new(&old, policy(2, vec![Predicate::PayloadAtMost(100)]), limits()).unwrap();
        assert_eq!(builder.push(ReplayCaseId::Proposal(1), &FrozenAction::freeze(spec).unwrap(), &observed, 0, None), Err(Error::Binding));
        assert!(matches!(builder.finish(), Err(Error::Incomplete)));
    }
}
