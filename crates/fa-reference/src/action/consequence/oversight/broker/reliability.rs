//! Apply independently labelled credibility to the actual owning oversight path.
//! Started but unapplied rounds remain pending: dropping a poor review cannot
//! quietly remove it from the current-policy promotion denominator.

mod qualification;

use super::{ObservedReview, OversightBroker};
use super::super::credibility::{
    Assessment, CredibilityLedger, CredibilityPromotion, CredibilityReport, EvaluationCase,
    EvaluationProtocol, EvaluationTicket, IndependentEvaluator, PendingCase,
    QualificationFailure, SealedAssessment,
};
use super::super::joint_credibility::{self, JointPromotionPolicy, JointPromotionReport, ReplayBasis};
use crate::action::consequence::{Consequence, congress::CongressPolicy};
use crate::Error;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct EvaluationState {
    ledger: CredibilityLedger,
    unfinished: BTreeMap<u64, u64>,
    joint: Option<JointPromotionPolicy>,
    joint_promotions: Vec<JointPromotionReport>,
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
        if self.delivery.controller().active_credibility().is_some()
            || self.held_out_joint_policy().is_some() { return Err(Error::Binding); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        let (ledger, evaluator) = CredibilityLedger::new(protocol, self.contracts.clone())?;
        self.credibility = Some(EvaluationState { ledger, unfinished: BTreeMap::new(),
            joint: None, joint_promotions: Vec::new() });
        Ok(evaluator)
    }

    /// Irreversible bootstrap opt-in, BEFORE any proposal or review. The same
    /// promotion entry point subsequently requires both individual qualification
    /// and joint replay; there is no separate unguarded weight-update method.
    /// Legacy profiles without this policy retain their original replay semantics.
    pub fn enable_joint_credibility(&mut self, policy: JointPromotionPolicy) -> Result<(), Error> {
        if !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        let state = self.credibility.as_mut().ok_or(Error::Incomplete)?;
        if state.joint.is_some() { return Err(Error::Duplicate); }
        state.joint = Some(policy);
        Ok(())
    }

    pub fn joint_credibility_policy(&self) -> Option<JointPromotionPolicy> {
        self.credibility.as_ref().and_then(|state| state.joint)
    }

    /// Preview the SAME deterministic next weights and complete owned evidence
    /// that promotion would use. This does not accept a caller-supplied candidate,
    /// subset of rounds or positive report. Existing marginal/unfinished-round
    /// failures remain failures rather than being repaired by joint statistics.
    pub fn joint_credibility_report(&self, expected_revision: u64) -> Result<JointPromotionReport, Error> {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        let policy = state.joint.ok_or(Error::Incomplete)?;
        let generation = self.delivery.controller().policy().generation();
        let report = state.report(generation);
        if report.revision != expected_revision { return Err(Error::Stale); }
        if !report.qualified() { return Err(Error::Incomplete); }
        let plan = state.ledger.weights(self.delivery.controller().congress_policy(), generation, expected_revision)?;
        self.replay_joint_weights(policy, &plan.next, &report)
    }

    pub fn joint_credibility_promotions(&self) -> Result<&[JointPromotionReport], Error> {
        Ok(&self.credibility.as_ref().ok_or(Error::Incomplete)?.joint_promotions)
    }

    fn replay_joint_weights(&self, policy: JointPromotionPolicy, candidate: &CongressPolicy,
        report: &CredibilityReport) -> Result<JointPromotionReport, Error>
    {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        let cases = self.started_rounds.iter().filter_map(|round| {
            match state.ledger.ticket(*round) {
                Ok(ticket) => Some(state.ledger.assessments(*round)
                    .map(|history| (ticket.case().clone(), history.last().copied()))),
                // Old unfinished policy strata do not fill the current stratum.
                // Every current-policy unfinished round was refused above.
                Err(Error::Missing) if state.unfinished.get(round)
                    .is_some_and(|generation| *generation != report.policy_generation) => None,
                Err(error) => Some(Err(error)),
            }
        });
        joint_credibility::replay(policy, ReplayBasis { protocol: &report.protocol,
            revision: report.revision, policy_generation: report.policy_generation,
            current: self.delivery.controller().congress_policy(), candidate }, cases)
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
    /// Configured joint replay completes BEFORE any authority or weight mutation.
    pub fn promote_credibility(
        &mut self, expected_sequence: u64, expected_epoch: u64, expected_evaluation_revision: u64,
    ) -> Result<CredibilityPromotion, Error> {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        let policy_generation = self.delivery.controller().policy().generation();
        let report = state.report(policy_generation);
        if report.revision != expected_evaluation_revision { return Err(Error::Stale); }
        if !report.qualified() { return Err(Error::Incomplete); }
        let plan = state.ledger.weights(self.delivery.controller().congress_policy(), policy_generation, expected_evaluation_revision)?;
        let joint = state.joint.map(|policy| self.replay_joint_weights(policy, &plan.next, &report)).transpose()?;
        if joint.as_ref().is_some_and(|report| !report.qualified()) { return Err(Error::Incomplete); }
        if joint.is_some() {
            self.credibility.as_mut().expect("enabled ledger").joint_promotions
                .try_reserve(1).map_err(|_| Error::Limit)?;
        }
        let change = self.delivery.replace_congress_weights(expected_sequence, expected_epoch, plan.next.clone())?;
        // The exclusive borrow keeps the evaluated source unchanged between
        // planning and the atomic control transition. Publication below cannot
        // fail logically or erase the labels used for the transition.
        let state = self.credibility.as_mut().expect("enabled ledger");
        let promotion = state.ledger.promoted(plan, change);
        if let Some(joint) = joint { state.joint_promotions.push(joint); }
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(promotion)
    }
}

#[cfg(test)]
#[path = "reliability/joint_tests.rs"]
mod joint_tests;
