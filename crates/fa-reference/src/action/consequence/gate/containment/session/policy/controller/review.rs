//! Immutable policy evidence travels with the frozen congress, not beside it.
//! The retained policy is shared data only; it contains no controller or rights.

pub mod replay;

use crate::action::FrozenAction;
use crate::action::consequence::Decision;
use crate::action::consequence::gate::ControlReceipt;
use crate::action::consequence::gate::containment::session::{
    BoundCommitment, BoundReview, ReviewSession,
};
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Policy};
use crate::reducer::Reduction;
use crate::round::Verdict;
use crate::Error;
use std::rc::Rc;

#[derive(Debug)]
pub struct PolicySession {
    session: ReviewSession,
    policy: Rc<Policy>,
    evaluation: Evaluation,
    snapshot_semantic_epoch: u64,
}

impl PolicySession {
    pub(super) fn new(
        session: ReviewSession,
        policy: Rc<Policy>,
        evaluation: Evaluation,
        snapshot_semantic_epoch: u64,
    ) -> Self {
        Self { session, policy, evaluation, snapshot_semantic_epoch }
    }

    pub fn action(&self) -> &FrozenAction {
        self.session.action()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }

    pub fn commitment(&self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<BoundCommitment, Error> {
        self.session.commitment(member, verdict, salt)
    }

    pub fn commit(&mut self, member: &str, commitment: BoundCommitment) -> Result<(), Error> {
        self.session.commit(member, commitment)
    }

    pub fn open_reveals(&mut self) -> Result<(), Error> {
        self.session.open_reveals()
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        self.session.reveal(member, verdict, salt)
    }

    pub fn finish(self) -> Result<PolicyReview, Error> {
        let completed = PolicyReview {
            review: self.session.finish()?,
            policy: self.policy,
            evaluation: self.evaluation,
            snapshot_semantic_epoch: self.snapshot_semantic_epoch,
        };
        // The ordinary control path consumes only a replay-consistent review.
        // This is independent recomputation, not authentication of source facts.
        completed.verify_replay()?;
        Ok(completed)
    }
}

/// No constructor or conversion back to a mutable review request. The
/// originating controller alone checks and consumes this completed review.
#[derive(Debug)]
pub struct PolicyReview {
    pub(super) review: BoundReview,
    pub(super) policy: Rc<Policy>,
    pub(super) evaluation: Evaluation,
    pub(super) snapshot_semantic_epoch: u64,
}

impl PolicyReview {
    pub fn decision(&self) -> &Decision {
        self.review.decision()
    }

    pub fn tally(&self) -> &Reduction {
        self.review.tally()
    }

    pub fn missing(&self) -> &[String] {
        self.review.missing()
    }

    pub fn abstained(&self) -> &[String] {
        self.review.abstained()
    }

    pub fn action(&self) -> &FrozenAction {
        self.review.action()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }
}

/// The exact policy, witnessed predicate results and snapshot semantic epoch
/// that preceded the vote, attached to its actual control transition. This is
/// an in-memory evidence record, not an authenticated or durable audit capsule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReceipt {
    pub control: ControlReceipt,
    pub policy: Rc<Policy>,
    pub evaluation: Evaluation,
    pub snapshot_semantic_epoch: u64,
}
