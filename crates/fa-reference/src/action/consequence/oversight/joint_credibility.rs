//! Joint congress replay before credibility-weight promotion (FI-A07/FI-A08).
//!
//! Re-tally the ORIGINAL recorded member outcomes with the original congress
//! reducer. Marginal precision/recall are not multiplied into a joint escape
//! estimate. These are finite, independently labelled evaluation cases, not a
//! population bound, detector qualification or an endpoint execution claim.

use super::credibility::{Assessment, EvaluationCase, EvaluationProtocol, Fraction,
    GroundTruth, MAX_EVALUATION_CASES};
use crate::action::consequence::{Consequence, congress::{CongressPolicy, evaluate_round}};
use crate::reducer::MAX_VOTES;
use crate::round::{MemberOutcome, Round, commitment};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_JOINT_MEMBER_OUTCOMES: usize = 2 * MAX_EVALUATION_CASES * MAX_VOTES;

/// Complete per-call admission for two original reductions per retained case.
/// Counts are logical records/visits, not CPU time or allocator measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointReplayBudget { pub cases: usize, pub member_outcomes: usize }
impl Default for JointReplayBudget {
    fn default() -> Self {
        Self { cases: MAX_EVALUATION_CASES, member_outcomes: MAX_JOINT_MEMBER_OUTCOMES }
    }
}

/// Install once before the first proposal or review. Per-origin regressions are
/// ALWAYS forbidden: looser rate ceilings cannot waive a newly introduced miss
/// or false stop on a previously successful recorded case. No replacement API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointPromotionPolicy {
    id: u64,
    generation: u64,
    maximum_escape_rate: Fraction,
    maximum_benign_stop_rate: Fraction,
    budget: JointReplayBudget,
}
impl JointPromotionPolicy {
    pub fn new(id: u64, generation: u64, maximum_escape_rate: Fraction,
        maximum_benign_stop_rate: Fraction, budget: JointReplayBudget) -> Result<Self, Error>
    {
        if id == 0 || generation == 0
            || maximum_escape_rate.denominator == 0
            || maximum_escape_rate.numerator > maximum_escape_rate.denominator
            || maximum_benign_stop_rate.denominator == 0
            || maximum_benign_stop_rate.numerator > maximum_benign_stop_rate.denominator
        { return Err(Error::InvalidInput); }
        if budget.cases > MAX_EVALUATION_CASES || budget.member_outcomes > MAX_JOINT_MEMBER_OUTCOMES {
            return Err(Error::Limit);
        }
        Ok(Self { id, generation, maximum_escape_rate, maximum_benign_stop_rate, budget })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn maximum_escape_rate(self) -> Fraction { self.maximum_escape_rate }
    pub fn maximum_benign_stop_rate(self) -> Fraction { self.maximum_benign_stop_rate }
    pub fn budget(self) -> JointReplayBudget { self.budget }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JointCounts {
    pub violation_origins: usize,
    pub benign_origins: usize,
    /// At least one replicate of this violation received empirical Continue.
    pub escaped_violation_origins: usize,
    /// At least one replicate of this benign case received a restriction.
    pub stopped_benign_origins: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointOrigin {
    pub truth: GroundTruth,
    pub rounds: usize,
    pub baseline_failed: bool,
    pub candidate_failed: bool,
    /// ANY individual replicate newly failed; grouping cannot hide this behind
    /// a different replicate that already failed under the baseline.
    pub regressed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JointFailure {
    PendingLabels,
    CensoredLabels,
    InsufficientViolationOrigins,
    InsufficientBenignOrigins,
    EscapeRate,
    BenignStopRate,
    RegressedOrigin(u64),
}

/// Historical supervisor evidence. It never authorizes a promotion or effect by
/// itself; the owning broker reconstructs it from its own retained cases.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::oversight::joint_credibility::JointPromotionReport;
/// fn grant(report: JointPromotionReport) -> Permit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointPromotionReport {
    pub policy: JointPromotionPolicy,
    pub evaluation_revision: u64,
    pub policy_generation: u64,
    pub baseline: CongressPolicy,
    pub candidate: CongressPolicy,
    pub work: JointReplayBudget,
    pub pending_cases: usize,
    pub censored_cases: usize,
    pub baseline_counts: JointCounts,
    pub candidate_counts: JointCounts,
    pub origins: BTreeMap<u64, JointOrigin>,
    pub failures: Vec<JointFailure>,
}
impl JointPromotionReport {
    pub fn qualified(&self) -> bool { self.failures.is_empty() }
}

pub(crate) struct ReplayBasis<'a> {
    pub protocol: &'a EvaluationProtocol,
    pub revision: u64,
    pub policy_generation: u64,
    pub current: &'a CongressPolicy,
    pub candidate: &'a CongressPolicy,
}

/// Crate-private source: the broker supplies its own original ticket/history
/// rows, never a caller-selected list. Old-policy rows cannot fill new-policy
/// denominators. Whole admission precedes the first re-tally.
pub(crate) fn replay(policy: JointPromotionPolicy, basis: ReplayBasis<'_>,
    cases: impl IntoIterator<Item = Result<(EvaluationCase, Option<Assessment>), Error>>)
    -> Result<JointPromotionReport, Error>
{
    weights_only(basis.current, basis.candidate)?;
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    let mut work = JointReplayBudget { cases: 0, member_outcomes: 0 };
    for (index, row) in cases.into_iter().enumerate() {
        if index >= MAX_EVALUATION_CASES { return Err(Error::Limit); }
        let (case, label) = row?;
        if !seen.insert(case.round) { return Err(Error::Duplicate); }
        if case.policy_generation != basis.policy_generation { continue; }
        if !case.verdicts.keys().eq(basis.current.members.keys()) { return Err(Error::Binding); }
        work.cases = work.cases.checked_add(1).ok_or(Error::Overflow)?;
        work.member_outcomes = work.member_outcomes.checked_add(
            case.verdicts.len().checked_mul(2).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        if work.cases > policy.budget.cases || work.member_outcomes > policy.budget.member_outcomes {
            return Err(Error::Limit);
        }
        selected.try_reserve(1).map_err(|_| Error::Limit)?;
        selected.push((case, label));
    }
    let mut report = JointPromotionReport {
        policy, evaluation_revision: basis.revision, policy_generation: basis.policy_generation,
        baseline: basis.current.clone(), candidate: basis.candidate.clone(), work,
        pending_cases: 0, censored_cases: 0, baseline_counts: JointCounts::default(),
        candidate_counts: JointCounts::default(), origins: BTreeMap::new(), failures: Vec::new(),
    };
    for (case, label) in selected {
        let round = reconstitute(&case)?;
        // Exact disqualifiers/contradictions are NOT inferred from a stopped flag.
        // This comparison isolates the empirical congress affected by reweighting.
        let before = evaluate_round(&round, basis.current, false, false)?.decision().consequence;
        let after = evaluate_round(&round, basis.candidate, false, false)?.decision().consequence;
        let Some(label) = label else { report.pending_cases += 1; continue; };
        if label.origin == 0 || label.evidence_id == [0; 32] { return Err(Error::InvalidInput); }
        if label.truth == GroundTruth::Censored { report.censored_cases += 1; continue; }
        let failure = |decision| match label.truth {
            GroundTruth::Violation => decision == Consequence::Continue,
            GroundTruth::Benign => decision != Consequence::Continue,
            GroundTruth::Censored => unreachable!("censored labels remain separate"),
        };
        let before = failure(before);
        let after = failure(after);
        let origin = report.origins.entry(label.origin).or_insert(JointOrigin {
            truth: label.truth, rounds: 0, baseline_failed: false,
            candidate_failed: false, regressed: false,
        });
        if origin.truth != label.truth { return Err(Error::Binding); }
        origin.rounds += 1;
        origin.baseline_failed |= before;
        origin.candidate_failed |= after;
        origin.regressed |= !before && after;
    }
    for (id, origin) in &report.origins {
        count(&mut report.baseline_counts, origin.truth, origin.baseline_failed);
        count(&mut report.candidate_counts, origin.truth, origin.candidate_failed);
        if origin.regressed { report.failures.push(JointFailure::RegressedOrigin(*id)); }
    }
    if report.pending_cases != 0 { report.failures.push(JointFailure::PendingLabels); }
    if report.censored_cases != 0 { report.failures.push(JointFailure::CensoredLabels); }
    let counts = report.candidate_counts;
    if (counts.violation_origins as u64) < basis.protocol.minimum_violation_origins {
        report.failures.push(JointFailure::InsufficientViolationOrigins);
    }
    if (counts.benign_origins as u64) < basis.protocol.minimum_benign_origins {
        report.failures.push(JointFailure::InsufficientBenignOrigins);
    }
    if !within(counts.escaped_violation_origins, counts.violation_origins, policy.maximum_escape_rate) {
        report.failures.push(JointFailure::EscapeRate);
    }
    if !within(counts.stopped_benign_origins, counts.benign_origins, policy.maximum_benign_stop_rate) {
        report.failures.push(JointFailure::BenignStopRate);
    }
    Ok(report)
}

fn weights_only(current: &CongressPolicy, next: &CongressPolicy) -> Result<(), Error> {
    if next.generation != current.generation.checked_add(1).ok_or(Error::Overflow)?
        || !current.members.keys().eq(next.members.keys()) { return Err(Error::Binding); }
    let mut normalized = next.clone();
    normalized.generation = current.generation;
    for (name, member) in &mut normalized.members { member.weight = current.members[name].weight; }
    if normalized != *current { return Err(Error::Binding); }
    Ok(())
}
fn within(numerator: usize, denominator: usize, ceiling: Fraction) -> bool {
    denominator != 0 && numerator as u128 * u128::from(ceiling.denominator)
        <= denominator as u128 * u128::from(ceiling.numerator)
}
fn count(counts: &mut JointCounts, truth: GroundTruth, failed: bool) {
    match truth {
        GroundTruth::Violation => {
            counts.violation_origins += 1;
            counts.escaped_violation_origins += usize::from(failed);
        }
        GroundTruth::Benign => {
            counts.benign_origins += 1;
            counts.stopped_benign_origins += usize::from(failed);
        }
        GroundTruth::Censored => unreachable!("only final labels enter the origin table"),
    }
}
fn reconstitute(case: &EvaluationCase) -> Result<Round, Error> {
    let mut round = Round::new(case.round, &case.evidence_root)?;
    for name in case.verdicts.keys() { round.add_member(name)?; }
    // Comparison-only transcript. These fixed salts are NOT recovered original
    // commitments or helper authentication. Missing stays missing; abstentions
    // remain real abstentions. No reconstructed round enters the live gate.
    const SALT: &[u8] = b"fa/joint-credibility/retally/v1";
    for (name, outcome) in &case.verdicts {
        if let MemberOutcome::Revealed(verdict) = outcome {
            round.commit(name, commitment(case.round, name, &case.evidence_root, *verdict, SALT)?)?;
        }
    }
    round.open_reveals()?;
    for (name, outcome) in &case.verdicts {
        if let MemberOutcome::Revealed(verdict) = outcome { round.reveal(name, *verdict, SALT)?; }
    }
    Ok(round)
}

#[cfg(test)]
mod tests;
