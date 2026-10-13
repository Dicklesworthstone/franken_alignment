//! Native evaluation-to-Andon accounting on the original owning broker path.
//!
//! The original independent-evaluator issuer check runs before this module.
//! Only successfully applied, complete, purely empirical stops are eligible.
//! This is process-local role separation, not evaluator authentication. Native
//! windows use the broker's trusted elapsed-clock domain, NOT offline campaign
//! control sequences. Use a dedicated accounting identity for this clock/scope.

use super::{Assessment, CommitteeContract, CredibilityLedger, EvaluationCase, EvaluationProtocol,
    GroundTruth, PendingCase, SealedAssessment};
use crate::action::{ElapsedTick, Scope};
use crate::action::consequence::{Consequence, Decision, Rule};
use crate::round::{MemberOutcome, Verdict};
use crate::round::review::andon::{AndonBudget, AndonLedger, LedgerIdentity, RecordResult, TrustedFalseHold};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Retain this original bootstrap binding independently before collecting
/// native receipts. Equality includes the complete helper input contracts, not
/// just a short ID or the subset of helpers that happened to hold an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAndonBinding {
    identity: LedgerIdentity,
    scope: Scope,
    protocol: EvaluationProtocol,
    contracts: CommitteeContract,
}

impl NativeAndonBinding {
    pub fn identity(&self) -> LedgerIdentity { self.identity }
    pub fn scope(&self) -> Scope { self.scope }
    pub fn protocol(&self) -> &EvaluationProtocol { &self.protocol }
    pub fn contracts(&self) -> &CommitteeContract { &self.contracts }
}

/// An original accepted native assessment paired with its actual applied case.
/// No public constructor accepts caller-authored false-stop claims or a receipt
/// made from an unapplied review. Clones copy evidence only, never authority.
///
/// ```compile_fail,E0451
/// use fa_reference::action::consequence::oversight::credibility::native_andon::NativeEvaluatedFalseHold;
/// fn forge() -> NativeEvaluatedFalseHold { NativeEvaluatedFalseHold {} }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeEvaluatedFalseHold {
    binding: Rc<NativeAndonBinding>,
    case: EvaluationCase,
    assessment: Assessment,
    fact: TrustedFalseHold,
    budget: AndonBudget,
}

impl NativeEvaluatedFalseHold {
    pub fn binding(&self) -> &NativeAndonBinding { &self.binding }
    pub fn case(&self) -> &EvaluationCase { &self.case }
    pub fn assessment(&self) -> Assessment { self.assessment }
    pub fn confirmation(&self) -> &TrustedFalseHold { &self.fact }
    pub fn budget(&self) -> AndonBudget { self.budget }
}

#[derive(Clone, Debug)]
pub(super) struct NativeAndonState {
    binding: Rc<NativeAndonBinding>,
    ledger: AndonLedger,
    receipts: BTreeMap<u64, NativeEvaluatedFalseHold>,
}

#[derive(Debug)]
pub(super) struct NativeReviewBasis {
    occurred_at: ElapsedTick,
    eligible: bool,
}

impl CredibilityLedger {
    pub(crate) fn andon_enabled(&self) -> bool { self.andon.is_some() }

    pub(crate) fn enable_andon(&mut self, scope: Scope, ledger: AndonLedger)
        -> Result<NativeAndonBinding, Error>
    {
        if self.andon.is_some() { return Err(Error::Duplicate); }
        if !self.records.is_empty() || !self.promotions.is_empty() { return Err(Error::WrongState); }
        if ledger.windows().is_empty() { return Err(Error::Incomplete); }
        // This profile names the original helper MODEL epoch as its generation.
        // Full tokenizer/profile/question/projection binding stays in the receipt.
        // Never invent a generation for a legacy contract with epoch zero.
        if self.contracts.members().values().any(|helper| helper.profile_at(0).model_epoch == 0) {
            return Err(Error::Binding);
        }
        let binding = NativeAndonBinding { identity: ledger.identity(), scope,
            protocol: self.protocol.clone(), contracts: self.contracts.clone() };
        self.andon = Some(NativeAndonState { binding: Rc::new(binding.clone()), ledger,
            receipts: BTreeMap::new() });
        Ok(binding)
    }

    pub(crate) fn register_andon_window(&mut self, budget: AndonBudget) -> Result<bool, Error> {
        self.andon.as_mut().ok_or(Error::Incomplete)?.ledger.register_window(budget)
    }
    pub(crate) fn andon_binding(&self) -> Result<&NativeAndonBinding, Error> {
        Ok(&self.andon.as_ref().ok_or(Error::Incomplete)?.binding)
    }
    pub(crate) fn andon_ledger(&self) -> Result<&AndonLedger, Error> {
        Ok(&self.andon.as_ref().ok_or(Error::Incomplete)?.ledger)
    }
    pub(crate) fn andon_receipt(&self, round: u64) -> Result<&NativeEvaluatedFalseHold, Error> {
        self.andon.as_ref().ok_or(Error::Incomplete)?.receipts.get(&round).ok_or(Error::Missing)
    }

    /// Called by the original broker BEFORE its original apply transition. The
    /// prepared basis becomes visible only when that transition actually succeeds.
    pub(crate) fn prepare_observed(&self, case: EvaluationCase, decision: &Decision,
        observed_at: Option<ElapsedTick>) -> Result<PendingCase, Error>
    {
        let mut prepared = self.prepare(case)?;
        if self.andon.is_some() {
            let occurred_at = observed_at.ok_or(Error::Incomplete)?;
            let outcomes = &prepared.case.verdicts;
            let eligible = prepared.case.stopped
                && matches!(decision.consequence, Consequence::HoldEffect | Consequence::Deny
                    | Consequence::NarrowAuthority | Consequence::SuspendRun)
                && matches!(decision.rules.as_slice(), [Rule::EmpiricalRecommendation])
                && outcomes.values().all(|outcome| matches!(outcome,
                    MemberOutcome::Revealed(Verdict::Allow | Verdict::Hold | Verdict::Deny)))
                && outcomes.values().any(|outcome| matches!(outcome,
                    MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny)));
            prepared.andon_basis = Some(NativeReviewBasis { occurred_at, eligible });
        }
        Ok(prepared)
    }

    /// Original issuer, final-label and origin consistency checks precede this
    /// preflight. All fallible accounting happens on observational candidate
    /// state before the original label/revision and this state are published.
    pub(super) fn prepare_andon_label(&self, label: &SealedAssessment,
        observed_at: Option<ElapsedTick>) -> Result<Option<NativeAndonState>, Error>
    {
        let Some(state) = &self.andon else { return Ok(None); };
        if label.assessment.truth != GroundTruth::Benign { return Ok(None); }
        let record = self.records.get(&label.round).ok_or(Error::Missing)?;
        let basis = record.andon_basis.as_ref().ok_or(Error::Incomplete)?;
        if !basis.eligible { return Ok(None); }
        let confirmed_at = observed_at.ok_or(Error::Incomplete)?;
        if confirmed_at < basis.occurred_at { return Err(Error::Stale); }
        let helper_generations = record.case.verdicts.iter()
            .filter(|(_, outcome)| matches!(outcome, MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny)))
            .map(|(member, _)| (member.clone(),
                state.binding.contracts.members()[member].profile_at(0).model_epoch))
            .collect();
        let fact = TrustedFalseHold {
            identity: state.binding.identity, report: state.binding.protocol.period,
            round: record.case.round, occurred_at: basis.occurred_at.0,
            confirmed_at: confirmed_at.0, evidence_root: record.case.evidence_root.to_vec(),
            helper_generations,
        };
        let mut candidate = state.clone();
        let window = match candidate.ledger.record_trusted(&fact)? {
            RecordResult::Recorded { window, .. } | RecordResult::AlreadyRecorded { window } => window,
        };
        let receipt = NativeEvaluatedFalseHold { binding: Rc::clone(&state.binding),
            case: record.case.clone(), assessment: label.assessment, fact,
            budget: candidate.ledger.windows()[&window].budget };
        candidate.receipts.insert(label.round, receipt);
        Ok(Some(candidate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Fraction, IndependentEvaluator};
    use crate::action::Purpose;
    use crate::action::consequence::oversight::HelperContract;
    use crate::full_input::InputProfileBinding;

    fn setup() -> (CredibilityLedger, IndependentEvaluator) {
        let contracts = CommitteeContract::new(BTreeMap::from([("h".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: vec![], model_epoch: 7,
                tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"question".to_vec()).unwrap())])).unwrap();
        let (mut ledger, evaluator) = CredibilityLedger::new(EvaluationProtocol {
            domain: 1, stratum: 2, period: 3, minimum_violation_origins: 1,
            minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
            recall_floor: Fraction { numerator: 1, denominator: 2 },
            false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10,
        }, contracts).unwrap();
        let mut accounting = AndonLedger::new(LedgerIdentity { deployment: 1, evaluator: 2 }).unwrap();
        accounting.register_window(AndonBudget { window: 1, start: 0, end: 100,
            false_stops_allowed: 1, cost_per_stop: 1 }).unwrap();
        ledger.enable_andon(Scope { tenant: 1, principal: 2, run: 3, branch: 4,
            authority: 5, purpose: Purpose::Effect }, accounting).unwrap();
        (ledger, evaluator)
    }
    fn case() -> EvaluationCase {
        EvaluationCase { round: 1, attempt: 1, policy_generation: 1, congress_generation: 1,
            evidence_root: [1; 32], verdicts: BTreeMap::from([("h".into(), MemberOutcome::Revealed(Verdict::Hold))]), stopped: true }
    }
    fn decision() -> Decision {
        Decision { consequence: Consequence::HoldEffect, rules: vec![Rule::EmpiricalRecommendation] }
    }
    fn label(ledger: &CredibilityLedger, evaluator: &IndependentEvaluator) -> SealedAssessment {
        evaluator.assess(&ledger.ticket(1).unwrap(), Assessment { origin: 1,
            evidence_id: [2; 32], truth: GroundTruth::Benign }).unwrap()
    }

    #[test]
    fn native_classification_excludes_exact_missing_and_contradictory_basis() {
        for extra in [None, Some(Rule::ExactDisqualifier), Some(Rule::MandatoryAbsent), Some(Rule::Contradiction)] {
            let (mut ledger, evaluator) = setup();
            let mut selected = decision();
            if let Some(rule) = extra { selected.rules.push(rule); }
            let prepared = ledger.prepare_observed(case(), &selected, Some(ElapsedTick(10))).unwrap();
            ledger.record(prepared);
            ledger.record_label_at(label(&ledger, &evaluator), Some(ElapsedTick(11))).unwrap();
            assert_eq!(ledger.andon_ledger().unwrap().windows()[&1].false_stops, u64::from(extra.is_none()));
        }
    }

    #[test]
    fn native_clock_refusals_preserve_original_label_and_allow_corrected_retry() {
        let (mut ledger, evaluator) = setup();
        let before = ledger.report(1);
        assert_eq!(ledger.prepare_observed(case(), &decision(), None).unwrap_err(), Error::Incomplete);
        assert_eq!(ledger.report(1), before);
        let prepared = ledger.prepare_observed(case(), &decision(), Some(ElapsedTick(10))).unwrap();
        ledger.record(prepared);
        let truth = label(&ledger, &evaluator);
        let before = ledger.report(1);
        assert_eq!(ledger.record_label_at(truth.clone(), None), Err(Error::Incomplete));
        assert_eq!(ledger.record_label_at(truth.clone(), Some(ElapsedTick(9))), Err(Error::Stale));
        assert_eq!(ledger.report(1), before);
        assert!(ledger.assessments(1).unwrap().is_empty());
        assert_eq!(ledger.record_label_at(truth.clone(), Some(ElapsedTick(10))), Ok(true));
        assert_eq!(ledger.record_label_at(truth, None), Ok(false));
        assert_eq!(ledger.andon_receipt(1).unwrap().confirmation().confirmed_at, 10);
    }

    #[test]
    fn native_missing_or_abstaining_helper_cannot_hide_in_an_empirical_stop() {
        for outcome in [MemberOutcome::Missing, MemberOutcome::Revealed(Verdict::Abstain)] {
            let (mut ledger, evaluator) = setup();
            let mut case = case();
            case.verdicts.insert("h".into(), outcome);
            let prepared = ledger.prepare_observed(case, &decision(), Some(ElapsedTick(10))).unwrap();
            ledger.record(prepared);
            ledger.record_label_at(label(&ledger, &evaluator), Some(ElapsedTick(11))).unwrap();
            assert_eq!(ledger.andon_receipt(1), Err(Error::Missing));
            assert_eq!(ledger.andon_ledger().unwrap().windows()[&1].false_stops, 0);
        }
    }
}
