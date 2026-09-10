//! Read-only campaign extraction from the actual owning controller history.
//! No caller chooses which denied proposals or later observations to include.

use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::gate::containment::session::policy::controller::PolicyAuthority;
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayBuilder, ReplayCaseId, ReplayLimits};
use crate::Error;
use std::rc::Rc;

impl PolicyAuthority {
    /// Both collections are append-only. This stamp also changes for newly
    /// admitted exact denials, which do not advance the control sequence.
    pub(crate) fn policy_replay_revision(&self) -> (usize, usize) {
        (self.records.len(), self.reviews.len())
    }

    pub(crate) fn replay_candidate_policy(&self, candidate: Policy, limits: ReplayLimits) -> Result<PolicyReplayReport, Error> {
        let mut builder = ReplayBuilder::new(&self.policy, candidate, limits)?;
        let cases = self.records.values().filter(|record| Rc::ptr_eq(&record.policy, &self.policy)).count()
            + self.reviews.iter().filter(|receipt| Rc::ptr_eq(&receipt.policy, &self.policy)).count();
        if cases > limits.cases { return Err(Error::Limit); }
        for (id, record) in &self.records {
            if Rc::ptr_eq(&record.policy, &self.policy) {
                let action = &self.host.gate.authority.attempts.get(id).ok_or(Error::Missing)?.action;
                builder.push(ReplayCaseId::Proposal(*id), action, &record.evaluation,
                    record.judgment.semantic_epoch, None)?;
            }
        }
        for receipt in &self.reviews {
            if Rc::ptr_eq(&receipt.policy, &self.policy) {
                let control = &receipt.control;
                builder.push(ReplayCaseId::Review { attempt: control.attempt,
                    round: control.binding.round, control_sequence: control.sequence },
                    &control.action, &receipt.evaluation, receipt.snapshot_semantic_epoch,
                    Some(control.decision.consequence))?;
            }
        }
        builder.finish()
    }
}
