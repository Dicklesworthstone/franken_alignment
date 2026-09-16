//! Frozen input sessions with explicit logical commit/reveal cutoffs.
//! Clocks are trusted inputs in the same domain as the controller's clock.
//! The existing PolicySession owns voting and replay; these sets only track
//! accepted phase completion and do not independently compute a verdict.

use super::CommitteeInput;
use super::super::replay::{ObservedDecisionArchive, ObservedReviewAnchor, OBSERVED_ARCHIVE_VERSION};
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
    commit_times: Vec<ElapsedTick>,
    reveals_opened_at: Option<ElapsedTick>,
    reveal_times: Vec<ElapsedTick>,
}

impl ObservedSession {
    pub(super) fn new(
        policy: PolicySession, issuer: Rc<()>, attempt: u64, revision: u64,
        inputs: Rc<CommitteeInput>, window: ReviewWindow, started_at: ElapsedTick,
    ) -> Self {
        Self {
            policy: Some(policy), issuer, attempt, revision, inputs, window, started_at,
            elapsed: started_at, reveal_phase: false, committed: BTreeSet::new(), revealed: BTreeSet::new(),
            commit_times: Vec::new(), reveals_opened_at: None, reveal_times: Vec::new(),
        }
    }

    pub fn input(&self, member: &str) -> Result<&EvidenceViewManifest, Error> {
        self.inputs.views().get(member).ok_or(Error::Missing)
    }
    pub fn inputs(&self) -> &CommitteeInput { &self.inputs }
    pub fn window(&self) -> ReviewWindow { self.window }
    pub fn elapsed(&self) -> ElapsedTick { self.elapsed }

    /// Retain this complete immutable basis independently BEFORE voting. It is
    /// data, not a key or an assertion that the source remains current.
    pub fn replay_anchor(&self) -> Result<ObservedReviewAnchor, Error> {
        Ok(ObservedReviewAnchor {
            policy: self.policy.as_ref().ok_or(Error::WrongState)?.replay_anchor(),
            inputs: Rc::clone(&self.inputs), input_revision: self.revision,
            window: self.window, started_at: self.started_at,
        })
    }

    /// Worker admission requires a fresh independent phase. There is no mixed
    /// caller-vote/worker fallback or change of frozen input after admission.
    pub(crate) fn worker_identity(&self) -> Result<(u64, [u8; 32]), Error> {
        if self.reveal_phase || !self.committed.is_empty() || !self.revealed.is_empty() {
            return Err(Error::WrongState);
        }
        if self.elapsed >= self.window.commit_by { return Err(Error::Stale); }
        Ok(self.policy.as_ref().ok_or(Error::WrongState)?.reference_identity())
    }

    /// Only the preassigned worker bridge imports raw reference digests. The
    /// existing commit operation still enforces the original phase and clock.
    pub(crate) fn commit_from_worker(
        &mut self, member: &str, digest: crate::round::Digest, now: ElapsedTick,
    ) -> Result<(), Error> {
        self.observe(now)?;
        if self.reveal_phase { return Err(Error::WrongState); }
        if now >= self.window.commit_by { return Err(Error::Stale); }
        let commitment = self.policy.as_ref().ok_or(Error::WrongState)?
            .import_reference_commitment(member, digest)?;
        self.commit(member, commitment, now)
    }

    pub fn commitment(&self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<BoundCommitment, Error> {
        if self.reveal_phase { return Err(Error::WrongState); }
        if self.elapsed >= self.window.commit_by { return Err(Error::Stale); }
        self.policy.as_ref().ok_or(Error::WrongState)?.commitment(member, verdict, salt)
    }

    pub fn commit(&mut self, member: &str, commitment: BoundCommitment, now: ElapsedTick) -> Result<(), Error> {
        self.observe(now)?;
        if self.reveal_phase { return Err(Error::WrongState); }
        if now >= self.window.commit_by { return Err(Error::Stale); }
        // Reserve before acceptance: recording an accepted vote cannot fail.
        // The original round bounds accepted commits to its fixed roster.
        self.commit_times.try_reserve(1).map_err(|_| Error::Limit)?;
        self.policy.as_mut().ok_or(Error::WrongState)?.commit(member, commitment)?;
        self.commit_times.push(now);
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
        self.reveals_opened_at = Some(now);
        Ok(())
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8], now: ElapsedTick) -> Result<(), Error> {
        self.observe(now)?;
        if !self.reveal_phase { return Err(Error::WrongState); }
        if now >= self.window.reveal_by { return Err(Error::Stale); }
        self.reveal_times.try_reserve(1).map_err(|_| Error::Limit)?;
        self.policy.as_mut().ok_or(Error::WrongState)?.reveal(member, verdict, salt)?;
        self.reveal_times.push(now);
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
            completed_at: now, commit_times: std::mem::take(&mut self.commit_times),
            reveals_opened_at: self.reveals_opened_at.expect("finished reveal phase"),
            reveal_times: std::mem::take(&mut self.reveal_times),
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
    commit_times: Vec<ElapsedTick>,
    reveals_opened_at: ElapsedTick,
    reveal_times: Vec<ElapsedTick>,
}

impl ObservedReview {
    /// Export accepted votes, phase observations and the WHOLE actual input.
    /// This neither consumes the review nor applies it to an authority ledger.
    pub fn replay_archive(&self) -> ObservedDecisionArchive {
        ObservedDecisionArchive {
            version: OBSERVED_ARCHIVE_VERSION, policy: self.policy.replay_archive(),
            inputs: Rc::clone(&self.inputs), input_revision: self.revision,
            window: self.window, started_at: self.started_at,
            commit_times: self.commit_times.clone(), reveals_opened_at: self.reveals_opened_at,
            reveal_times: self.reveal_times.clone(), completed_at: self.completed_at,
        }
    }

    pub fn decision(&self) -> &Decision { self.policy.decision() }
    pub fn missing(&self) -> &[String] { self.policy.missing() }
    pub fn abstained(&self) -> &[String] { self.policy.abstained() }
    pub fn inputs(&self) -> &CommitteeInput { &self.inputs }
    pub fn completed_at(&self) -> ElapsedTick { self.completed_at }
}
