//! Durable opt-in to the ORIGINAL joint congress replay gate.
//!
//! Both marginal qualification and joint replay must pass before the existing
//! promotion can change authority. No saved report, caller-selected candidate,
//! subset of cases, or product of marginal probabilities is accepted here.
use super::{CredibilityEvent, EvaluationProtocol, FileIndependentEvaluator};
use super::super::{Event, FileOversight, JournalError};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::credibility::Fraction;
use crate::action::consequence::oversight::joint_credibility::{
    JointPromotionPolicy, JointPromotionReport, JointReplayBudget,
};
use crate::Error;
use std::rc::Rc;

impl FileOversight {
    /// Freeze the evaluation protocol AND joint gate in one acknowledged event,
    /// before any request or proposal. Neither can be disabled or replaced, and
    /// an already marginal-only deployment cannot be silently upgraded in place.
    /// The returned role belongs to the independent evaluator, never the actor.
    pub fn enable_joint_credibility(
        &mut self,
        revision: u64,
        protocol: EvaluationProtocol,
        policy: JointPromotionPolicy,
    ) -> Result<FileIndependentEvaluator, JournalError> {
        self.transact(revision, Event::Credibility(CredibilityEvent::EnableJoint(protocol, policy)))?;
        Ok(FileIndependentEvaluator { issuer: Rc::clone(&self.issuer) })
    }

    /// None explicitly means that this owner never selected the joint gate.
    /// It is not a successful joint-validation result.
    pub fn joint_credibility_policy(&self) -> Result<Option<JointPromotionPolicy>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.joint_credibility_policy())
    }

    /// Recompute the native candidate against ALL owned current-policy cases.
    /// Pending/unqualified marginal evidence and exhausted work budgets refuse.
    /// A preview does not mutate the journal, reserve resources or grant rights.
    pub fn joint_credibility_report(
        &self,
        expected_evaluation_revision: u64,
    ) -> Result<JointPromotionReport, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.joint_credibility_report(expected_evaluation_revision).map_err(Into::into)
    }

    /// The native historical report consumed by THIS operation, not a freshly
    /// recomputed report under a later candidate or an authority-bearing token.
    pub fn joint_credibility_promotion(&self, operation: u64) -> Result<&JointPromotionReport, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.machine.broker.joint_credibility_policy().is_none() {
            return Err(Error::Incomplete.into());
        }
        let promotion = self.machine.credibility_promotion(operation)?;
        self.machine.broker.joint_credibility_promotions()?.iter().find(|report| {
            report.evaluation_revision == promotion.report.revision
                && report.policy_generation == promotion.report.policy_generation
                && report.baseline == promotion.change.previous
                && report.candidate == promotion.change.current
        }).ok_or_else(|| Error::Binding.into())
    }
}

// Fixed bounded original POLICY INPUTS only. These functions neither evaluate
// cases nor decode an authority, score, promoted weight or positive report.
pub(super) fn write_policy(w: &mut Writer, policy: JointPromotionPolicy) -> Result<(), Error> {
    let escape = policy.maximum_escape_rate();
    let benign = policy.maximum_benign_stop_rate();
    for value in [
        policy.id(), policy.generation(), escape.numerator, escape.denominator,
        benign.numerator, benign.denominator,
        u64::try_from(policy.budget().cases).map_err(|_| Error::Limit)?,
        u64::try_from(policy.budget().member_outcomes).map_err(|_| Error::Limit)?,
    ] {
        w.u64(value)?;
    }
    Ok(())
}

pub(super) fn read_policy(r: &mut Reader<'_>) -> Result<JointPromotionPolicy, Error> {
    JointPromotionPolicy::new(
        r.u64()?, r.u64()?,
        Fraction { numerator: r.u64()?, denominator: r.u64()? },
        Fraction { numerator: r.u64()?, denominator: r.u64()? },
        JointReplayBudget {
            cases: usize::try_from(r.u64()?).map_err(|_| Error::Limit)?,
            member_outcomes: usize::try_from(r.u64()?).map_err(|_| Error::Limit)?,
        },
    )
}

#[cfg(test)]
mod tests;
