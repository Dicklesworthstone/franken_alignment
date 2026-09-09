//! Independent-label credibility accounting for one frozen committee contract.
//!
//! Counts describe the declared evaluation stratum, not population error bounds.
//! The separate evaluator handle models role separation, not real authentication
//! or institutional independence. No consensus-to-ground-truth conversion exists.

use super::CommitteeContract;
use crate::action::consequence::congress::CongressPolicy;
use crate::round::{MemberOutcome, Verdict};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_EVALUATION_CASES: usize = 512;
pub const MAX_WEIGHT_CHANGES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fraction { pub numerator: u64, pub denominator: u64 }

impl Fraction {
    fn valid(self) -> bool { self.denominator != 0 && self.numerator <= self.denominator }
    fn met(self, numerator: u64, denominator: u64) -> bool {
        denominator != 0 && u128::from(numerator) * u128::from(self.denominator)
            >= u128::from(self.numerator) * u128::from(denominator)
    }
    fn not_exceeded(self, numerator: u64, denominator: u64) -> bool {
        denominator != 0 && u128::from(numerator) * u128::from(self.denominator)
            <= u128::from(self.numerator) * u128::from(denominator)
    }
}

/// Frozen before any proposal is admitted through the evaluated facade.
/// Budget units are false-stopping rounds, not currency or inference tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationProtocol {
    pub domain: u64,
    pub stratum: u64,
    pub period: u64,
    pub minimum_violation_origins: u64,
    pub minimum_benign_origins: u64,
    pub precision_floor: Fraction,
    pub recall_floor: Fraction,
    pub false_positive_ceiling: Fraction,
    pub false_stop_budget: u64,
}

impl EvaluationProtocol {
    fn validate(&self) -> Result<(), Error> {
        if [self.domain, self.stratum, self.period, self.minimum_violation_origins,
            self.minimum_benign_origins, self.false_stop_budget].contains(&0)
            || !self.precision_floor.valid() || !self.recall_floor.valid()
            || !self.false_positive_ceiling.valid()
            || self.precision_floor.numerator == 0 || self.recall_floor.numerator == 0
        {
            return Err(Error::InvalidInput);
        }
        if u128::from(self.minimum_violation_origins) + u128::from(self.minimum_benign_origins)
            > MAX_EVALUATION_CASES as u128
        {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundTruth { Benign, Violation, Censored }

/// `origin` groups repeats from the same original evaluation scenario. The
/// evaluator, not a majority vote or the actor, owns both origin and label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assessment {
    pub origin: u64,
    pub evidence_id: [u8; 32],
    pub truth: GroundTruth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationCase {
    pub round: u64,
    pub attempt: u64,
    pub policy_generation: u64,
    pub congress_generation: u64,
    pub evidence_root: [u8; 32],
    pub verdicts: BTreeMap<String, MemberOutcome>,
    pub stopped: bool,
}

#[derive(Clone, Debug)]
pub struct EvaluationTicket { issuer: Rc<()>, case: EvaluationCase }
impl EvaluationTicket { pub fn case(&self) -> &EvaluationCase { &self.case } }

/// Created once by trusted bootstrap and returned to the independent evaluator,
/// not retained as a minting handle by the effect controller.
#[derive(Debug)]
pub struct IndependentEvaluator { issuer: Rc<()> }

#[derive(Clone, Debug)]
pub struct SealedAssessment { issuer: Rc<()>, round: u64, assessment: Assessment }

impl IndependentEvaluator {
    pub fn assess(&self, ticket: &EvaluationTicket, assessment: Assessment) -> Result<SealedAssessment, Error> {
        if !Rc::ptr_eq(&self.issuer, &ticket.issuer) { return Err(Error::Binding); }
        if assessment.origin == 0 || assessment.evidence_id == [0; 32] { return Err(Error::InvalidInput); }
        Ok(SealedAssessment { issuer: Rc::clone(&self.issuer), round: ticket.case.round, assessment })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberMetrics {
    pub true_positives: u64,
    pub false_negatives: u64,
    pub false_positives: u64,
    pub true_negatives: u64,
    pub missing_origins: u64,
    pub abstaining_origins: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualificationFailure {
    PendingLabels,
    CensoredLabels,
    InsufficientViolationOrigins,
    InsufficientBenignOrigins,
    Precision(String),
    Recall(String),
    FalsePositiveRate(String),
    FalseStopBudgetExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityReport {
    pub protocol: EvaluationProtocol,
    pub contracts: CommitteeContract,
    pub revision: u64,
    pub policy_generation: u64,
    pub scoped_cases: usize,
    pub retained_cases: usize,
    pub pending_cases: usize,
    pub censored_cases: usize,
    pub violation_origins: u64,
    pub benign_origins: u64,
    pub lifetime_false_stops: u64,
    pub calibration_incident_open: bool,
    pub members: BTreeMap<String, MemberMetrics>,
    pub failures: Vec<QualificationFailure>,
}
impl CredibilityReport { pub fn qualified(&self) -> bool { self.failures.is_empty() } }

/// Actual fenced control transition, paired with the label basis used to choose
/// its weights. Does not certify endpoint outcomes or evaluator truth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CongressChange {
    pub previous: CongressPolicy,
    pub current: CongressPolicy,
    pub sequence: u64,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityPromotion { pub report: CredibilityReport, pub change: CongressChange }

#[derive(Debug)]
struct Record { case: EvaluationCase, assessments: Vec<Assessment> }

#[derive(Debug)]
pub(crate) struct PendingCase { case: EvaluationCase, revision: u64 }

#[derive(Debug)]
pub(crate) struct WeightPlan {
    pub next: CongressPolicy,
    pub report: CredibilityReport,
    revision: u64,
}

#[derive(Debug)]
pub(crate) struct CredibilityLedger {
    issuer: Rc<()>,
    protocol: EvaluationProtocol,
    contracts: CommitteeContract,
    revision: u64,
    records: BTreeMap<u64, Record>,
    promotions: Vec<CredibilityPromotion>,
}

impl CredibilityLedger {
    pub(crate) fn new(protocol: EvaluationProtocol, contracts: CommitteeContract) -> Result<(Self, IndependentEvaluator), Error> {
        protocol.validate()?;
        let issuer = Rc::new(());
        let evaluator = IndependentEvaluator { issuer: Rc::clone(&issuer) };
        Ok((Self { issuer, protocol, contracts, revision: 0, records: BTreeMap::new(), promotions: Vec::new() }, evaluator))
    }

    pub(crate) fn prepare(&self, case: EvaluationCase) -> Result<PendingCase, Error> {
        if [case.round, case.attempt, case.policy_generation, case.congress_generation].contains(&0)
            || case.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        if !case.verdicts.keys().eq(self.contracts.members().keys()) { return Err(Error::Binding); }
        if self.records.contains_key(&case.round) { return Err(Error::Duplicate); }
        if self.records.len() >= MAX_EVALUATION_CASES { return Err(Error::Limit); }
        Ok(PendingCase { case, revision: self.revision.checked_add(1).ok_or(Error::Overflow)? })
    }

    /// The owning facade calls this immediately after the corresponding review
    /// transition, under the same exclusive borrow and after prepare succeeded.
    pub(crate) fn record(&mut self, pending: PendingCase) {
        self.revision = pending.revision;
        self.records.insert(pending.case.round, Record { case: pending.case, assessments: Vec::new() });
    }

    pub(crate) fn ticket(&self, round: u64) -> Result<EvaluationTicket, Error> {
        let record = self.records.get(&round).ok_or(Error::Missing)?;
        Ok(EvaluationTicket { issuer: Rc::clone(&self.issuer), case: record.case.clone() })
    }

    pub(crate) fn assessments(&self, round: u64) -> Result<&[Assessment], Error> {
        Ok(&self.records.get(&round).ok_or(Error::Missing)?.assessments)
    }

    pub(crate) fn record_label(&mut self, label: SealedAssessment) -> Result<bool, Error> {
        if !Rc::ptr_eq(&self.issuer, &label.issuer) { return Err(Error::Binding); }
        let record = self.records.get(&label.round).ok_or(Error::Missing)?;
        if let Some(previous) = record.assessments.last() {
            if previous == &label.assessment { return Ok(false); }
            // A censored label may be resolved once, retaining the original.
            // Final truth and origin cannot be rewritten or silently corrected.
            if previous.truth != GroundTruth::Censored || label.assessment.truth == GroundTruth::Censored
                || previous.origin != label.assessment.origin { return Err(Error::Binding); }
        }
        if label.assessment.truth != GroundTruth::Censored {
            for other in self.records.values().filter(|r| r.case.policy_generation == record.case.policy_generation) {
                if let Some(prior) = other.assessments.last()
                    && prior.origin == label.assessment.origin && prior.truth != GroundTruth::Censored
                    && prior.truth != label.assessment.truth
                {
                    return Err(Error::Binding);
                }
            }
        }
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.records.get_mut(&label.round).expect("checked case").assessments.push(label.assessment);
        self.revision = revision;
        Ok(true)
    }

    pub(crate) fn report(&self, policy_generation: u64) -> CredibilityReport {
        let mut report = CredibilityReport {
            protocol: self.protocol.clone(), contracts: self.contracts.clone(), revision: self.revision, policy_generation,
            scoped_cases: 0, retained_cases: self.records.len(), pending_cases: 0, censored_cases: 0,
            violation_origins: 0, benign_origins: 0, lifetime_false_stops: 0, calibration_incident_open: false,
            members: self.contracts.members().keys().map(|m| (m.clone(), MemberMetrics::default())).collect(), failures: Vec::new(),
        };
        let mut origins: BTreeMap<u64, (GroundTruth, BTreeMap<String, Flags>)> = BTreeMap::new();
        for record in self.records.values() {
            let assessment = record.assessments.last();
            if record.case.stopped && assessment.is_some_and(|a| a.truth == GroundTruth::Benign) {
                report.lifetime_false_stops += 1;
            }
            if record.case.policy_generation != policy_generation { continue; }
            report.scoped_cases += 1;
            let Some(assessment) = assessment else { report.pending_cases += 1; continue; };
            if assessment.truth == GroundTruth::Censored { report.censored_cases += 1; continue; }
            let (_, flags) = origins.entry(assessment.origin).or_insert_with(|| (
                assessment.truth, self.contracts.members().keys().map(|m| (m.clone(), Flags::default())).collect(),
            ));
            for (member, outcome) in &record.case.verdicts {
                let f = flags.get_mut(member).expect("frozen roster");
                f.all_alert &= matches!(outcome, MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny));
                f.all_allow &= *outcome == MemberOutcome::Revealed(Verdict::Allow);
                f.missing |= *outcome == MemberOutcome::Missing;
                f.abstaining |= *outcome == MemberOutcome::Revealed(Verdict::Abstain);
            }
        }
        for (truth, flags) in origins.values() {
            match truth { GroundTruth::Violation => report.violation_origins += 1, GroundTruth::Benign => report.benign_origins += 1, GroundTruth::Censored => unreachable!("filtered censored") }
            for (member, flags) in flags {
                let metric = report.members.get_mut(member).expect("frozen roster");
                if *truth == GroundTruth::Violation {
                    if flags.all_alert { metric.true_positives += 1; } else { metric.false_negatives += 1; }
                } else if flags.all_allow { metric.true_negatives += 1; } else { metric.false_positives += 1; }
                metric.missing_origins += u64::from(flags.missing);
                metric.abstaining_origins += u64::from(flags.abstaining);
            }
        }
        // All counts are <= MAX_EVALUATION_CASES. Duplicates are pessimistically
        // grouped: they never increase sample size or hide a bad replicated vote.
        if report.pending_cases != 0 { report.failures.push(QualificationFailure::PendingLabels); }
        if report.censored_cases != 0 { report.failures.push(QualificationFailure::CensoredLabels); }
        if report.violation_origins < self.protocol.minimum_violation_origins { report.failures.push(QualificationFailure::InsufficientViolationOrigins); }
        if report.benign_origins < self.protocol.minimum_benign_origins { report.failures.push(QualificationFailure::InsufficientBenignOrigins); }
        for (member, m) in &report.members {
            if !self.protocol.precision_floor.met(m.true_positives, m.true_positives + m.false_positives) { report.failures.push(QualificationFailure::Precision(member.clone())); }
            if !self.protocol.recall_floor.met(m.true_positives, m.true_positives + m.false_negatives) { report.failures.push(QualificationFailure::Recall(member.clone())); }
            if !self.protocol.false_positive_ceiling.not_exceeded(m.false_positives, m.false_positives + m.true_negatives) { report.failures.push(QualificationFailure::FalsePositiveRate(member.clone())); }
        }
        report.calibration_incident_open = report.lifetime_false_stops >= self.protocol.false_stop_budget;
        if report.calibration_incident_open { report.failures.push(QualificationFailure::FalseStopBudgetExhausted); }
        report
    }

    pub(crate) fn weights(&self, current: &CongressPolicy, policy_generation: u64, expected_revision: u64) -> Result<WeightPlan, Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.promotions.len() >= MAX_WEIGHT_CHANGES { return Err(Error::Limit); }
        if !current.members.keys().eq(self.contracts.members().keys()) || current.caps.per_member == 0 { return Err(Error::Binding); }
        let report = self.report(policy_generation);
        if !report.qualified() { return Err(Error::Incomplete); }
        let mut next = current.clone();
        next.generation = current.generation.checked_add(1).ok_or(Error::Overflow)?;
        let mut changed = false;
        for (member, entry) in &mut next.members {
            let m = &report.members[member];
            let tp = u128::from(m.true_positives);
            let denominator = u128::from(m.true_positives + m.false_positives) * u128::from(m.true_positives + m.false_negatives);
            // Registered deterministic rule: 1 + floor((cap-1)*precision*recall).
            // cap <= u64::MAX and every count <= 512, so this fits in u128.
            let scaled = u128::from(current.caps.per_member - 1) * tp * tp / denominator;
            let weight = 1 + u64::try_from(scaled).map_err(|_| Error::Overflow)?;
            changed |= entry.weight != weight;
            entry.weight = weight;
        }
        if !changed { return Err(Error::WrongState); }
        Ok(WeightPlan { next, report, revision: self.revision.checked_add(1).ok_or(Error::Overflow)? })
    }

    pub(crate) fn promoted(&mut self, plan: WeightPlan, change: CongressChange) -> CredibilityPromotion {
        let promotion = CredibilityPromotion { report: plan.report, change };
        self.revision = plan.revision;
        self.promotions.push(promotion.clone());
        promotion
    }
    pub(crate) fn promotions(&self) -> &[CredibilityPromotion] { &self.promotions }
}

#[derive(Debug)]
struct Flags { all_alert: bool, all_allow: bool, missing: bool, abstaining: bool }
impl Default for Flags {
    fn default() -> Self { Self { all_alert: true, all_allow: true, missing: false, abstaining: false } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::congress::MemberPolicy;
    use crate::full_input::InputProfileBinding;
    use crate::reducer::Caps;
    use crate::action::consequence::oversight::HelperContract;

    fn protocol() -> EvaluationProtocol {
        EvaluationProtocol { domain: 1, stratum: 2, period: 3, minimum_violation_origins: 1, minimum_benign_origins: 1,
            precision_floor: Fraction { numerator: 1, denominator: 2 }, recall_floor: Fraction { numerator: 1, denominator: 2 },
            false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
    }
    fn ledger() -> (CredibilityLedger, IndependentEvaluator) {
        let profile = InputProfileBinding { profile_id: 1, profile_bytes: vec![], model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 };
        let contracts = CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(profile, 1, b"Q".to_vec()).unwrap())])).unwrap();
        CredibilityLedger::new(protocol(), contracts).unwrap()
    }
    fn congress() -> CongressPolicy {
        CongressPolicy { generation: 1, members: BTreeMap::from([("helper".into(), MemberPolicy { cohort: "a".into(), weight: 1 })]),
            caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 11, suspend_at: 12,
            minimum_members: 1, minimum_cohorts: 1 }
    }
    fn add(l: &mut CredibilityLedger, round: u64, outcome: MemberOutcome, policy_generation: u64) {
        let pending = l.prepare(EvaluationCase { round, attempt: round, policy_generation, congress_generation: 1,
            evidence_root: [1; 32], verdicts: BTreeMap::from([("helper".into(), outcome)]),
            stopped: outcome != MemberOutcome::Revealed(Verdict::Allow) }).unwrap();
        l.record(pending);
    }
    fn label(l: &mut CredibilityLedger, e: &IndependentEvaluator, round: u64, origin: u64, truth: GroundTruth) -> Result<bool, Error> {
        let ticket = l.ticket(round)?;
        l.record_label(e.assess(&ticket, Assessment { origin, evidence_id: [2; 32], truth })?)
    }
    #[test]
    fn independent_labels_support_both_denominators_and_bounded_weights() {
        let (mut l, e) = ledger();
        add(&mut l, 1, MemberOutcome::Revealed(Verdict::Hold), 1);
        add(&mut l, 2, MemberOutcome::Revealed(Verdict::Allow), 1);
        assert!(!l.report(1).qualified());
        label(&mut l, &e, 1, 1, GroundTruth::Violation).unwrap();
        label(&mut l, &e, 2, 2, GroundTruth::Benign).unwrap();
        let report = l.report(1); assert!(report.qualified());
        assert_eq!(report.members["helper"].true_positives, 1);
        assert_eq!(report.members["helper"].true_negatives, 1);
        let plan = l.weights(&congress(), 1, report.revision).unwrap();
        assert_eq!(plan.next.members["helper"].weight, 10);
        assert_eq!(plan.next.caps, congress().caps);
    }
    #[test]
    fn quiet_helper_cannot_gain_influence_from_false_alarm_perfection() {
        let (mut l, e) = ledger();
        for round in 1..=2 { add(&mut l, round, MemberOutcome::Revealed(Verdict::Allow), 1); }
        label(&mut l, &e, 1, 1, GroundTruth::Violation).unwrap(); label(&mut l, &e, 2, 2, GroundTruth::Benign).unwrap();
        let report = l.report(1); assert_eq!(report.members["helper"].false_positives, 0);
        assert!(report.failures.contains(&QualificationFailure::Recall("helper".into())));
        assert_eq!(l.weights(&congress(), 1, report.revision).unwrap_err(), Error::Incomplete);
    }
    #[test]
    fn same_origin_replication_does_not_manufacture_sample_size_or_hide_misses() {
        let (mut l, e) = ledger();
        add(&mut l, 1, MemberOutcome::Revealed(Verdict::Hold), 1);
        add(&mut l, 2, MemberOutcome::Revealed(Verdict::Allow), 1);
        label(&mut l, &e, 1, 7, GroundTruth::Violation).unwrap(); label(&mut l, &e, 2, 7, GroundTruth::Violation).unwrap();
        let report = l.report(1); assert_eq!(report.violation_origins, 1);
        assert_eq!(report.members["helper"].true_positives, 0); assert_eq!(report.members["helper"].false_negatives, 1);
    }
    #[test]
    fn missing_and_abstaining_are_not_discarded_from_either_denominator() {
        let (mut l, e) = ledger();
        add(&mut l, 1, MemberOutcome::Missing, 1); add(&mut l, 2, MemberOutcome::Revealed(Verdict::Abstain), 1);
        label(&mut l, &e, 1, 1, GroundTruth::Violation).unwrap(); label(&mut l, &e, 2, 2, GroundTruth::Benign).unwrap();
        let m = l.report(1).members.remove("helper").unwrap();
        assert_eq!((m.false_negatives, m.false_positives, m.missing_origins, m.abstaining_origins), (1, 1, 1, 1));
    }
    #[test]
    fn censored_labels_can_resolve_but_final_truth_cannot_be_rewritten() {
        let (mut l, e) = ledger(); add(&mut l, 1, MemberOutcome::Revealed(Verdict::Hold), 1);
        label(&mut l, &e, 1, 1, GroundTruth::Censored).unwrap(); assert_eq!(l.report(1).censored_cases, 1);
        label(&mut l, &e, 1, 1, GroundTruth::Violation).unwrap(); let revision = l.revision;
        assert_eq!(label(&mut l, &e, 1, 1, GroundTruth::Violation), Ok(false));
        assert_eq!(label(&mut l, &e, 1, 1, GroundTruth::Benign), Err(Error::Binding));
        assert_eq!(l.revision, revision); assert_eq!(l.assessments(1).unwrap().len(), 2);
    }
    #[test]
    fn evaluator_and_label_instances_cannot_cross_ledger_boundaries() {
        let (mut a, ea) = ledger(); let (mut b, eb) = ledger();
        add(&mut a, 1, MemberOutcome::Missing, 1); add(&mut b, 1, MemberOutcome::Missing, 1);
        let ticket = a.ticket(1).unwrap(); let assessment = Assessment { origin: 1, evidence_id: [1; 32], truth: GroundTruth::Violation };
        assert_eq!(eb.assess(&ticket, assessment).unwrap_err(), Error::Binding);
        let label = ea.assess(&ticket, assessment).unwrap(); assert_eq!(b.record_label(label), Err(Error::Binding));
    }
    #[test]
    fn false_stop_budget_persists_across_policy_generations() {
        let (mut l, e) = ledger(); l.protocol.false_stop_budget = 1;
        add(&mut l, 1, MemberOutcome::Revealed(Verdict::Hold), 1); label(&mut l, &e, 1, 1, GroundTruth::Benign).unwrap();
        let report = l.report(2); assert_eq!(report.scoped_cases, 0); assert_eq!(report.retained_cases, 1);
        assert!(report.calibration_incident_open); assert_eq!(report.lifetime_false_stops, 1);
        assert!(report.failures.contains(&QualificationFailure::FalseStopBudgetExhausted));
    }
    #[test]
    fn stale_metrics_and_conflicting_origin_labels_fail_without_mutation() {
        let (mut l, e) = ledger(); add(&mut l, 1, MemberOutcome::Revealed(Verdict::Hold), 1);
        let old = l.revision; label(&mut l, &e, 1, 1, GroundTruth::Violation).unwrap();
        assert_eq!(l.weights(&congress(), 1, old).unwrap_err(), Error::Stale);
        add(&mut l, 2, MemberOutcome::Revealed(Verdict::Allow), 1); let before = l.report(1);
        assert_eq!(label(&mut l, &e, 2, 1, GroundTruth::Benign), Err(Error::Binding)); assert_eq!(l.report(1), before);
    }
    #[test]
    fn rate_comparisons_do_not_overflow_or_treat_zero_denominators_as_success() {
        let fraction = Fraction { numerator: u64::MAX - 1, denominator: u64::MAX };
        assert!(fraction.met(u64::MAX - 1, u64::MAX)); assert!(!fraction.met(0, 0));
        assert!(!fraction.met(u64::MAX - 2, u64::MAX)); assert!(fraction.not_exceeded(0, 1));
    }
}
