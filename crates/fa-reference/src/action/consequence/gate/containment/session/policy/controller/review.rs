//! Immutable policy evidence travels with the frozen congress, not beside it.
//! The retained policy is shared data only; it contains no controller or rights.

pub mod replay;
mod governance;

use crate::action::FrozenAction;
use crate::action::consequence::Decision;
use crate::action::consequence::gate::ControlReceipt;
use crate::action::consequence::gate::containment::session::{BoundCommitment, BoundReview, ReviewSession};
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Policy};
use crate::reducer::Reduction;
use crate::round::Verdict;
use crate::Error;
use std::rc::Rc;

impl super::PolicyAuthority {
    /// Read-only reuse validation for an already consumed permit at a coupled
    /// publication boundary. Reuse the ORIGINAL policy, witnesses and authority
    /// checks; this neither issues another permit nor changes an effect outcome.
    pub(in crate::action::consequence) fn recheck_publication(
        &self, attempt: u64, reviewed_sequence: u64, snapshot: &crate::Snapshot,
    ) -> Result<(), Error> {
        let inspection = self.inspect();
        if inspection.suspended { return Err(Error::WrongState); }
        if inspection.sequence != reviewed_sequence { return Err(Error::Stale); }
        if !matches!(inspection.ledger.stages.get(&attempt),
            Some(crate::action::ActionState::Dispatching | crate::action::ActionState::Unknown))
        { return Err(Error::WrongState); }
        self.recheck(attempt, snapshot)
    }
}

#[derive(Debug)]
pub struct PolicySession {
    session: ReviewSession,
    policy: Rc<Policy>,
    evaluation: Evaluation,
    snapshot_semantic_epoch: u64,
}

impl PolicySession {
    pub(super) fn new(session: ReviewSession, policy: Rc<Policy>, evaluation: Evaluation, snapshot_semantic_epoch: u64) -> Self {
        Self { session, policy, evaluation, snapshot_semantic_epoch }
    }
    pub fn action(&self) -> &FrozenAction { self.session.action() }
    pub fn policy(&self) -> &Policy { &self.policy }
    pub fn evaluation(&self) -> &Evaluation { &self.evaluation }
    pub fn commitment(&self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<BoundCommitment, Error> { self.session.commitment(member, verdict, salt) }
    pub fn commit(&mut self, member: &str, commitment: BoundCommitment) -> Result<(), Error> { self.session.commit(member, commitment) }
    pub fn open_reveals(&mut self) -> Result<(), Error> { self.session.open_reveals() }
    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> { self.session.reveal(member, verdict, salt) }
    pub fn finish(self) -> Result<PolicyReview, Error> {
        let completed = PolicyReview {
            review: self.session.finish()?, policy: self.policy, evaluation: self.evaluation,
            snapshot_semantic_epoch: self.snapshot_semantic_epoch,
        };
        completed.verify_replay()?;
        Ok(completed)
    }

    pub(crate) fn reference_identity(&self) -> (u64, [u8; 32]) {
        self.session.reference_identity()
    }

    pub(crate) fn import_reference_commitment(
        &self, member: &str, digest: crate::round::Digest,
    ) -> Result<BoundCommitment, Error> {
        self.session.import_reference_commitment(member, digest)
    }
}

/// No constructor or conversion back to a mutable review request.
#[derive(Debug)]
pub struct PolicyReview {
    pub(super) review: BoundReview,
    pub(super) policy: Rc<Policy>,
    pub(super) evaluation: Evaluation,
    pub(super) snapshot_semantic_epoch: u64,
}

impl PolicyReview {
    pub fn decision(&self) -> &Decision { self.review.decision() }
    pub fn tally(&self) -> &Reduction { self.review.tally() }
    pub fn missing(&self) -> &[String] { self.review.missing() }
    pub fn abstained(&self) -> &[String] { self.review.abstained() }
    pub fn action(&self) -> &FrozenAction { self.review.action() }
    pub fn policy(&self) -> &Policy { &self.policy }
    pub fn evaluation(&self) -> &Evaluation { &self.evaluation }
}

/// The exact pre-vote policy evidence attached to its actual control transition.
/// This is in-memory evidence, not an authenticated or durable audit capsule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReceipt {
    pub control: ControlReceipt,
    pub policy: Rc<Policy>,
    pub evaluation: Evaluation,
    pub snapshot_semantic_epoch: u64,
}
