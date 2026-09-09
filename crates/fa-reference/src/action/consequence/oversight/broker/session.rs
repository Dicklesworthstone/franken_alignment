//! Frozen input sessions with explicit logical commit/reveal cutoffs.
//! Clocks are trusted inputs in the same domain as the controller's clock.
//! The existing PolicySession owns voting and replay; these sets only track
//! accepted phase completion and do not independently compute a verdict.

use super::CommitteeInput;
use crate::action::ElapsedTick;
use crate::action::consequence::Decision;
use crate::action::consequence::gate::containment::session::BoundCommitment;
use crate::action::consequence::gate::containment::session::policy::controller::{PolicyReview, PolicySession};
use crate::evidence_view::EvidenceViewManifest;
use crate::round::Verdict;
use crate::Error;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewWindow {
    pub commit_by: ElapsedTick,
    pub reveal_by: ElapsedTick,
}

#[derive(Debug)]
pub struct ObservedSession {
    policy: Option<PolicySession>,
    issuer: Rc<()>,
    attempt: u64,
    revision: u64,
    inputs: Rc<CommitteeInput>,
    window: ReviewWindow,
    started_at: ElapsedTick,
    elapsed: ElapsedTick,
    reveal_phase: bool,
    committed: BTreeSet<String>,
    revealed: BTreeSet<String>,
}

impl ObservedSession {
    pub(super) fn new(
        policy: PolicySession, issuer: Rc<()>, attempt: u64, revision: u64,
        inputs: Rc<CommitteeInput>, window: ReviewWindow, started_at: ElapsedTick,
    ) -> Self {
        Self {
            policy: Some(policy), issuer, attempt, revision, inputs, window, started_at,
            elapsed: started_at, reveal_phase: false, committed: BTreeSet::new(), revealed: BTreeSet::new(),
        }
    }

    pub fn input(&self, member: &str) -> Result<&EvidenceViewManifest, Error> {
        self.inputs.views().get(member).ok_or(Error::Missing)
    }
    pub fn inputs(&self) -> &CommitteeInput { &self.inputs }
    pub fn window(&self) -> ReviewWindow { self.window }
    pub fn elapsed(&self) -> ElapsedTick { self.elapsed }

    pub fn commitment(&self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<BoundCommitment, Error> {
        if self.reveal_phase { return Err(Error::WrongState); }
        if self.elapsed >= self.window.commit_by { return Err(Error::Stale); }
        self.policy.as_ref().ok_or(Error::WrongState)?.commitment(member, verdict, salt)
    }

    pub fn commit(&mut self, member: &str, commitment: BoundCommitment, now: ElapsedTick) -> Result<(), Error> {
        self.observe(now)?;
        if self.reveal_phase { return Err(Error::WrongState); }
        if now >= self.window.commit_by { return Err(Error::Stale); }
        self.policy.as_mut().ok_or(Error::WrongState)?.commit(member, commitment)?;
        self.committed.insert(member.to_owned());
        Ok(())
    }

    pub fn open_reveals(&mut self, now: ElapsedTick) -> Result<(), Error> {
        self.observe(now)?;
        if self.reveal_phase { return Err(Error::WrongState); }
        if self.committed.len() != self.inputs.views().len() && now < self.window.commit_by {
            return Err(Error::Incomplete);
        }
        self.policy.as_mut().ok_or(Error::WrongState)?.open_reveals()?;
        self.reveal_phase = true;
        Ok(())
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8], now: ElapsedTick) -> Result<(), Error> {
        self.observe(now)?;
        if !self.reveal_phase { return Err(Error::WrongState); }
        if now >= self.window.reveal_by { return Err(Error::Stale); }
        self.policy.as_mut().ok_or(Error::WrongState)?.reveal(member, verdict, salt)?;
        self.revealed.insert(member.to_owned());
        Ok(())
    }

    /// A premature finish preserves the session. At reveal expiry, omitted
    /// members remain missing in the original reducer rather than disappearing
    /// from its denominator. No automatic clock advance or deadline extension.
    pub fn finish(&mut self, now: ElapsedTick) -> Result<ObservedReview, Error> {
        self.observe(now)?;
        if !self.reveal_phase {
            if now < self.window.reveal_by { return Err(Error::Incomplete); }
            self.open_reveals(now)?;
        }
        if self.revealed.len() != self.inputs.views().len() && now < self.window.reveal_by {
            return Err(Error::Incomplete);
        }
        let policy = self.policy.take().ok_or(Error::WrongState)?.finish()?;
        Ok(ObservedReview {
            policy, issuer: Rc::clone(&self.issuer), attempt: self.attempt, revision: self.revision,
            inputs: Rc::clone(&self.inputs), window: self.window, started_at: self.started_at,
            completed_at: now,
        })
    }

    fn observe(&mut self, now: ElapsedTick) -> Result<(), Error> {
        if self.policy.is_none() { return Err(Error::WrongState); }
        if now < self.elapsed { return Err(Error::Stale); }
        self.elapsed = now;
        Ok(())
    }
}

/// Non-rebindable evidence, not a Permit. No raw PolicyReview conversion exists.
#[derive(Debug)]
pub struct ObservedReview {
    pub(super) policy: PolicyReview,
    pub(super) issuer: Rc<()>,
    pub(super) attempt: u64,
    pub(super) revision: u64,
    pub(super) inputs: Rc<CommitteeInput>,
    pub(super) window: ReviewWindow,
    pub(super) started_at: ElapsedTick,
    pub(super) completed_at: ElapsedTick,
}

impl ObservedReview {
    pub fn decision(&self) -> &Decision { self.policy.decision() }
    pub fn missing(&self) -> &[String] { self.policy.missing() }
    pub fn abstained(&self) -> &[String] { self.policy.abstained() }
    pub fn inputs(&self) -> &CommitteeInput { &self.inputs }
    pub fn completed_at(&self) -> ElapsedTick { self.completed_at }
}
