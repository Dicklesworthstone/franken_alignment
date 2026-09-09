//! Apply independently labelled credibility to the actual owning oversight path.
//! Started but unapplied rounds remain pending: dropping a poor review cannot
//! quietly remove it from the current-policy promotion denominator.

use super::{ObservedReview, OversightBroker};
use super::super::credibility::{
    Assessment, CredibilityLedger, CredibilityPromotion, CredibilityReport, EvaluationCase,
    EvaluationProtocol, EvaluationTicket, IndependentEvaluator, PendingCase,
    QualificationFailure, SealedAssessment,
};
use crate::action::consequence::Consequence;
use crate::Error;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct EvaluationState {
    ledger: CredibilityLedger,
    unfinished: BTreeMap<u64, u64>,
}

impl EvaluationState {
    pub(super) fn started(&mut self, round: u64, policy_generation: u64) { self.unfinished.insert(round, policy_generation); }
    pub(super) fn applied(&mut self, round: u64, prepared: PendingCase) {
        self.ledger.record(prepared);
        self.unfinished.remove(&round);
    }
    fn report(&self, policy_generation: u64) -> CredibilityReport {
        let mut report = self.ledger.report(policy_generation);
        let pending = self.unfinished.values().filter(|generation| **generation == policy_generation).count();
        report.scoped_cases += pending;
        report.retained_cases += self.unfinished.len();
        report.pending_cases += pending;
        if pending != 0 && !report.failures.contains(&QualificationFailure::PendingLabels) { report.failures.push(QualificationFailure::PendingLabels); }
        report
    }
}

impl OversightBroker {
    /// One-time trusted bootstrap. The evaluator receives the only label-minting
    /// handle; the actor/effect interface receives no equivalent constructor.
    pub fn enable_credibility(&mut self, protocol: EvaluationProtocol) -> Result<IndependentEvaluator, Error> {
        if self.credibility.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        let (ledger, evaluator) = CredibilityLedger::new(protocol, self.contracts.clone())?;
        self.credibility = Some(EvaluationState { ledger, unfinished: BTreeMap::new() });
        Ok(evaluator)
    }

    pub fn evaluation_ticket(&self, round: u64) -> Result<EvaluationTicket, Error> {
        self.credibility.as_ref().ok_or(Error::Incomplete)?.ledger.ticket(round)
    }
    pub fn record_evaluation(&mut self, label: SealedAssessment) -> Result<bool, Error> {
        self.credibility.as_mut().ok_or(Error::Incomplete)?.ledger.record_label(label)
    }
    pub fn evaluation_history(&self, round: u64) -> Result<&[Assessment], Error> {
        self.credibility.as_ref().ok_or(Error::Incomplete)?.ledger.assessments(round)
    }
    pub fn credibility_report(&self) -> Result<CredibilityReport, Error> {
        Ok(self.credibility.as_ref().ok_or(Error::Incomplete)?.report(self.delivery.controller().policy().generation()))
    }
    pub fn credibility_promotions(&self) -> Result<&[CredibilityPromotion], Error> {
        Ok(self.credibility.as_ref().ok_or(Error::Incomplete)?.ledger.promotions())
    }

    pub(super) fn prepare_evaluation(&self, review: &ObservedReview) -> Result<Option<(u64, PendingCase)>, Error> {
        let Some(state) = &self.credibility else { return Ok(None); };
        let archive = review.policy.replay_archive();
        if state.unfinished.get(&archive.anchor.round) != Some(&archive.anchor.policy.generation()) { return Err(Error::Binding); }
        let round = archive.transcript.replay()?;
        let case = EvaluationCase {
            round: archive.anchor.round, attempt: review.attempt,
            policy_generation: archive.anchor.policy.generation(), congress_generation: archive.anchor.congress.generation,
            evidence_root: archive.anchor.evidence_root,
            verdicts: round.outcomes().into_iter().map(|(member, verdict)| (member.to_owned(), verdict)).collect(),
            stopped: review.decision().consequence != Consequence::Continue,
        };
        Ok(Some((case.round, state.ledger.prepare(case)?)))
    }

    /// A deterministic, label-qualified weight update, not a caller-selected
    /// roster or threshold change. Every old undispatched attempt is fenced and
    /// cancelled; unknown/committed effects and evaluation history survive.
    pub fn promote_credibility(
        &mut self, expected_sequence: u64, expected_epoch: u64, expected_evaluation_revision: u64,
    ) -> Result<CredibilityPromotion, Error> {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        let policy_generation = self.delivery.controller().policy().generation();
        let report = state.report(policy_generation);
        if report.revision != expected_evaluation_revision { return Err(Error::Stale); }
        if !report.qualified() { return Err(Error::Incomplete); }
        let plan = state.ledger.weights(self.delivery.controller().congress_policy(), policy_generation, expected_evaluation_revision)?;
        let change = self.delivery.replace_congress_weights(expected_sequence, expected_epoch, plan.next.clone())?;
        // The exclusive borrow keeps the evaluated source unchanged between
        // planning and the atomic control transition. Publication below cannot
        // fail logically or erase the labels used for the transition.
        let promotion = self.credibility.as_mut().expect("enabled ledger").ledger.promoted(plan, change);
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(promotion)
    }
}
