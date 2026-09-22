//! A frozen joint-decision guard for the ORIGINAL held-out activation path.
//! Reuse the real Congress reducer and the actual capped candidate weights.
//! Distinct evidence roots are not assumed independent. Counts are descriptive
//! within the sealed manifest, not population bounds or authenticated labels.

use super::PolicyAuthority;
use crate::action::consequence::congress::{CongressPolicy, CREDIBILITY_PPM, evaluate_round};
use crate::action::consequence::congress::credibility::{
    CredibilitySnapshot, EvaluationScope, LabelVerdict, Observation, MAX_CASES,
};
use crate::action::consequence::Consequence;
use crate::reducer::MAX_VOTES;
use crate::round::{Round, Verdict, commitment};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_JOINT_MEMBER_OUTCOMES: usize = 2 * MAX_CASES * MAX_VOTES;

/// Logical case and member-outcome visits across both original reductions.
/// Does not claim to meter allocator bytes, hashes, CPU time or source capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldOutJointBudget { pub cases: usize, pub member_outcomes: usize }
impl Default for HeldOutJointBudget {
    fn default() -> Self {
        Self { cases: MAX_CASES, member_outcomes: MAX_JOINT_MEMBER_OUTCOMES }
    }
}

/// Immutable bootstrap policy. Every stratum must pass separately; every newly
/// introduced case failure is forbidden even under permissive aggregate limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldOutJointPolicy {
    id: u64,
    generation: u64,
    minimum_safe_roots: u64,
    minimum_violation_roots: u64,
    maximum_escape_ppm: u64,
    maximum_false_stop_ppm: u64,
    budget: HeldOutJointBudget,
}
impl HeldOutJointPolicy {
    pub fn new(id: u64, generation: u64, minimum_safe_roots: u64,
        minimum_violation_roots: u64, maximum_escape_ppm: u64,
        maximum_false_stop_ppm: u64, budget: HeldOutJointBudget) -> Result<Self, Error>
    {
        if [id, generation, minimum_safe_roots, minimum_violation_roots].contains(&0)
            || maximum_escape_ppm > CREDIBILITY_PPM || maximum_false_stop_ppm > CREDIBILITY_PPM {
            return Err(Error::InvalidInput);
        }
        if u128::from(minimum_safe_roots) + u128::from(minimum_violation_roots) > MAX_CASES as u128
            || budget.cases > MAX_CASES || budget.member_outcomes > MAX_JOINT_MEMBER_OUTCOMES {
            return Err(Error::Limit);
        }
        Ok(Self { id, generation, minimum_safe_roots, minimum_violation_roots,
            maximum_escape_ppm, maximum_false_stop_ppm, budget })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn minimum_safe_roots(self) -> u64 { self.minimum_safe_roots }
    pub fn minimum_violation_roots(self) -> u64 { self.minimum_violation_roots }
    pub fn maximum_escape_ppm(self) -> u64 { self.maximum_escape_ppm }
    pub fn maximum_false_stop_ppm(self) -> u64 { self.maximum_false_stop_ppm }
    pub fn budget(self) -> HeldOutJointBudget { self.budget }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeldOutJointCounts {
    pub safe_roots: u64,
    pub violation_roots: u64,
    pub escaped_roots: u64,
    pub false_stopped_roots: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldOutJointStratum {
    pub baseline: HeldOutJointCounts,
    pub candidate: HeldOutJointCounts,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldOutJointRoot {
    pub stratum: String,
    pub truth: LabelVerdict,
    pub cases: usize,
    pub baseline_failed: bool,
    pub candidate_failed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldOutJointCase {
    pub baseline: Consequence,
    pub candidate: Consequence,
    pub regressed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeldOutJointFailure {
    RegressedCase(u64),
    InsufficientSafeRoots(String),
    InsufficientViolationRoots(String),
    EscapeRate(String),
    FalseStopRate(String),
}

/// Recomputed evidence, never an activation request, approval key or receipt.
/// The original activation history retains the sealed manifest alongside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldOutJointReport {
    pub policy: HeldOutJointPolicy,
    pub scope: EvaluationScope,
    pub baseline: CongressPolicy,
    pub candidate: CongressPolicy,
    pub work: HeldOutJointBudget,
    pub cases: BTreeMap<u64, HeldOutJointCase>,
    pub roots: BTreeMap<[u8; 32], HeldOutJointRoot>,
    pub strata: BTreeMap<String, HeldOutJointStratum>,
    pub failures: Vec<HeldOutJointFailure>,
}
impl HeldOutJointReport { pub fn qualified(&self) -> bool { self.failures.is_empty() } }

/// Read-only comparison. A caller may inspect a candidate, but the authority
/// NEVER accepts this report: activation derives its own capped candidate and
/// invokes this function again. Exact policy cannot hide an empirical regression.
pub fn evaluate(policy: HeldOutJointPolicy, baseline: &CongressPolicy,
    candidate: &CongressPolicy, snapshot: &CredibilitySnapshot) -> Result<HeldOutJointReport, Error>
{
    baseline.validate()?;
    candidate.validate()?;
    if candidate.generation <= baseline.generation
        || !baseline.members.keys().eq(candidate.members.keys())
        || !baseline.members.keys().eq(snapshot.helpers().keys()) { return Err(Error::Binding); }
    let mut normalized = candidate.clone();
    normalized.generation = baseline.generation;
    for (member, entry) in &mut normalized.members {
        entry.weight = baseline.members[member].weight;
        if entry.cohort != snapshot.helpers()[member].cohort { return Err(Error::Binding); }
    }
    if normalized != *baseline { return Err(Error::Binding); }
    let cases = snapshot.case_specs().len();
    let member_outcomes = cases.checked_mul(baseline.members.len())
        .and_then(|n| n.checked_mul(2)).ok_or(Error::Limit)?;
    if cases > policy.budget.cases || member_outcomes > policy.budget.member_outcomes {
        return Err(Error::Limit);
    }
    let mut report = HeldOutJointReport {
        policy, scope: snapshot.scope().clone(), baseline: baseline.clone(), candidate: candidate.clone(),
        work: HeldOutJointBudget { cases, member_outcomes }, cases: BTreeMap::new(), roots: BTreeMap::new(),
        strata: snapshot.strata().iter().map(|s| (s.clone(), HeldOutJointStratum::default())).collect(),
        failures: Vec::new(),
    };
    for case in snapshot.case_specs() {
        let label = snapshot.case_label(case.id).ok_or(Error::Incomplete)?;
        if label.verdict == LabelVerdict::Censored { return Err(Error::Incomplete); }
        let observations = snapshot.case_observations(case.id).ok_or(Error::Incomplete)?;
        if !observations.keys().eq(baseline.members.keys()) { return Err(Error::Incomplete); }
        // A late hold does not tell us its pre-dispatch vote. Do not invent an
        // Allow or let a reconstructed mandatory-absence Hold conceal a miss.
        let mut verdicts = BTreeMap::new();
        for (member, observation) in observations {
            let verdict = match observation {
                Observation::Clear => Verdict::Allow,
                Observation::Hold { first_sequence } if *first_sequence < case.dispatch_sequence => Verdict::Hold,
                Observation::Hold { .. } | Observation::Missing | Observation::Abstain => return Err(Error::Incomplete),
            };
            verdicts.insert(member.as_str(), verdict);
        }
        // A comparison-only transcript, not recovered original commitments or
        // authenticated helper speech. It never enters a live review session.
        const SALT: &[u8] = b"fa/held-out-joint/retally/v1";
        let mut round = Round::new(case.id, &case.evidence_root)?;
        for member in verdicts.keys() { round.add_member(member)?; }
        for (member, verdict) in &verdicts {
            round.commit(member, commitment(case.id, member, &case.evidence_root, *verdict, SALT)?)?;
        }
        round.open_reveals()?;
        for (member, verdict) in &verdicts { round.reveal(member, *verdict, SALT)?; }
        let before = evaluate_round(&round, baseline, false, false)?.decision().consequence;
        let after = evaluate_round(&round, candidate, false, false)?.decision().consequence;
        let failed = |decision| match label.verdict {
            LabelVerdict::Safe => decision != Consequence::Continue,
            LabelVerdict::Violation => decision == Consequence::Continue,
            LabelVerdict::Censored => unreachable!("censored evidence refused"),
        };
        let before_failed = failed(before);
        let after_failed = failed(after);
        let regressed = !before_failed && after_failed;
        if regressed { report.failures.push(HeldOutJointFailure::RegressedCase(case.id)); }
        report.cases.insert(case.id, HeldOutJointCase { baseline: before, candidate: after, regressed });
        let root = report.roots.entry(case.evidence_root).or_insert(HeldOutJointRoot {
            stratum: case.stratum.clone(), truth: label.verdict, cases: 0,
            baseline_failed: false, candidate_failed: false,
        });
        // An identical evidence root cannot be relabelled or used to fill two
        // strata. Any failed replicate fails the root; ANY new case regression
        // remains a failure even when another replicate already failed before.
        if root.stratum != case.stratum || root.truth != label.verdict { return Err(Error::Binding); }
        root.cases += 1;
        root.baseline_failed |= before_failed;
        root.candidate_failed |= after_failed;
    }
    for root in report.roots.values() {
        let stratum = report.strata.get_mut(&root.stratum).ok_or(Error::Binding)?;
        count(&mut stratum.baseline, root.truth, root.baseline_failed);
        count(&mut stratum.candidate, root.truth, root.candidate_failed);
    }
    for (name, stratum) in &report.strata {
        let c = stratum.candidate;
        if c.safe_roots < policy.minimum_safe_roots {
            report.failures.push(HeldOutJointFailure::InsufficientSafeRoots(name.clone()));
        }
        if c.violation_roots < policy.minimum_violation_roots {
            report.failures.push(HeldOutJointFailure::InsufficientViolationRoots(name.clone()));
        }
        if !within(c.escaped_roots, c.violation_roots, policy.maximum_escape_ppm) {
            report.failures.push(HeldOutJointFailure::EscapeRate(name.clone()));
        }
        if !within(c.false_stopped_roots, c.safe_roots, policy.maximum_false_stop_ppm) {
            report.failures.push(HeldOutJointFailure::FalseStopRate(name.clone()));
        }
    }
    Ok(report)
}
fn within(numerator: u64, denominator: u64, ceiling: u64) -> bool {
    denominator != 0 && u128::from(numerator) * u128::from(CREDIBILITY_PPM)
        <= u128::from(denominator) * u128::from(ceiling)
}
fn count(c: &mut HeldOutJointCounts, truth: LabelVerdict, failed: bool) {
    match truth {
        LabelVerdict::Safe => { c.safe_roots += 1; c.false_stopped_roots += u64::from(failed); }
        LabelVerdict::Violation => { c.violation_roots += 1; c.escaped_roots += u64::from(failed); }
        LabelVerdict::Censored => unreachable!("only final labels reach counts"),
    }
}

impl PolicyAuthority {
    /// Install once, before any proposal, authority change or activation. This
    /// governs future promotions, not permission to skip baseline review. No
    /// disable/update method exists; policy replacement and reset retain it.
    pub fn enable_held_out_joint(&mut self, policy: HeldOutJointPolicy) -> Result<(), Error> {
        if self.held_out_joint.is_some() { return Err(Error::Duplicate); }
        let gate = &self.host.gate;
        if gate.sequence != 0 || gate.authority.rights.epoch() != 0 || gate.suspended
            || !gate.authority.attempts.is_empty() || !self.credibility_history.is_empty() {
            return Err(Error::WrongState);
        }
        self.held_out_joint = Some(policy);
        Ok(())
    }
    pub fn held_out_joint_policy(&self) -> Option<HeldOutJointPolicy> { self.held_out_joint }

    /// None is an explicitly legacy, unguarded activation, not joint validation.
    pub fn held_out_joint_report(&self, operation: u64) -> Result<Option<&HeldOutJointReport>, Error> {
        self.credibility_history.iter().find(|r| r.receipt.operation == operation)
            .map(|r| r.joint.as_ref()).ok_or(Error::Missing)
    }
}
