//! Authority-bound congress sessions. The action and predecessor are captured
//! from the owning ledger before any commitment; callers cannot attach another
//! action, policy or authority to the completed review.
//!
//! Process-local brands and exact retained data provide reference binding only.
//! The underlying commitment remains the explicitly non-cryptographic round
//! oracle. No authentication, cross-process commitment or helper honesty follows.

use super::ContainmentAuthority;
use crate::Error;
use crate::action::FrozenAction;
use crate::action::consequence::congress::{CongressPolicy, evaluate_round};
use crate::action::consequence::gate::{
    ConsequenceAuthority, ControlReceipt, MAX_DECISIONS, ReviewRequest, TargetCeiling,
};
use crate::action::consequence::{Consequence, Decision};
use crate::reducer::{MAX_VOTES, Reduction};
use crate::round::{Digest, Phase, Round, Verdict, commitment};
use std::rc::Rc;

/// The controller freezes these facts before independent voting. The action,
/// issuer and expected predecessor are deliberately not caller-supplied fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSpec {
    pub attempt: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub policy: CongressPolicy,
    pub exact_disqualifier: bool,
    pub contradiction: bool,
    pub narrowed_targets: TargetCeiling,
}

#[derive(Debug)]
struct Context {
    spec: SessionSpec,
    action: FrozenAction,
    expected_control_sequence: u64,
    authority_issuer: Rc<()>,
}

/// An opaque helper commitment scoped to one immutable session and member.
/// This object is evidence data, not an authority token.
#[derive(Debug)]
pub struct BoundCommitment {
    context: Rc<Context>,
    member: String,
    digest: Digest,
}

/// No mutable round, policy or action accessor and no Clone implementation.
#[derive(Debug)]
pub struct ReviewSession {
    context: Rc<Context>,
    round: Round,
}

impl ReviewSession {
    pub fn begin(gate: &ConsequenceAuthority, spec: SessionSpec) -> Result<Self, Error> {
        if gate.suspended {
            return Err(Error::WrongState);
        }
        if gate.seen_rounds.contains(&spec.round) {
            return Err(Error::Duplicate);
        }
        if gate.seen_rounds.len() >= MAX_DECISIONS || spec.policy.members.len() > MAX_VOTES {
            return Err(Error::Limit);
        }
        let attempt = gate.authority.attempts.get(&spec.attempt).ok_or(Error::Missing)?;
        let mut round = Round::new(spec.round, &spec.evidence_root)?;
        for member in spec.policy.members.keys() {
            round.add_member(member)?;
        }
        // Reuse the existing policy validator on an all-missing projection.
        // This is constructor validation, not evaluation of the actual votes.
        // The real round remains in Commit and receives no derived approval.
        let mut validation = round.clone();
        validation.open_reveals()?;
        evaluate_round(&validation, &spec.policy, spec.exact_disqualifier, spec.contradiction)?;
        let context = Context {
            spec,
            action: attempt.action.clone(),
            expected_control_sequence: gate.sequence,
            authority_issuer: Rc::clone(&gate.authority.issuer),
        };
        Ok(Self {
            context: Rc::new(context),
            round,
        })
    }

    pub fn begin_containment(
        authority: &ContainmentAuthority,
        spec: SessionSpec,
    ) -> Result<Self, Error> {
        Self::begin(&authority.gate, spec)
    }

    pub fn action(&self) -> &FrozenAction {
        &self.context.action
    }

    pub fn policy(&self) -> &CongressPolicy {
        &self.context.spec.policy
    }

    /// Construct a helper's prospective commitment inside the frozen session.
    /// Even identical raw oracle digests cannot cross the session/member brand.
    pub fn commitment(
        &self,
        member: &str,
        verdict: Verdict,
        salt: &[u8],
    ) -> Result<BoundCommitment, Error> {
        if self.round.phase() != Phase::Commit {
            return Err(Error::WrongState);
        }
        self.round.outcome(member)?;
        let digest = commitment(
            self.round.id(), member, self.round.evidence_root(), verdict, salt,
        )?;
        Ok(BoundCommitment {
            context: Rc::clone(&self.context),
            member: member.to_owned(),
            digest,
        })
    }

    pub fn commit(&mut self, member: &str, value: BoundCommitment) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.context, &value.context) || value.member != member {
            return Err(Error::Binding);
        }
        self.round.commit(member, value.digest)
    }

    pub fn open_reveals(&mut self) -> Result<(), Error> {
        self.round.open_reveals()
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        self.round.reveal(member, verdict, salt)
    }

    /// Consumes the round: late evidence cannot mutate an already emitted review.
    /// Missing reveals remain explicit and cannot turn into an implicit quorum.
    pub fn finish(self) -> Result<BoundReview, Error> {
        let spec = &self.context.spec;
        let evaluation = evaluate_round(
            &self.round, &spec.policy, spec.exact_disqualifier, spec.contradiction,
        )?;
        let decision = evaluation.decision();
        let tally = evaluation.tally().clone();
        let missing = evaluation.missing().to_vec();
        let abstained = evaluation.abstained().to_vec();
        let retained = (decision.consequence == Consequence::NarrowAuthority)
            .then(|| spec.narrowed_targets.clone());
        let request = evaluation.into_request(
            spec.attempt,
            self.context.expected_control_sequence,
            self.context.action.clone(),
            retained,
        );
        Ok(BoundReview {
            context: self.context,
            request,
            decision,
            tally,
            missing,
            abstained,
        })
    }
}

/// A completed, non-rebindable review. Only the original authority may apply it.
/// Applying consumes it; it cannot be converted into a mutable raw request.
#[derive(Debug)]
pub struct BoundReview {
    context: Rc<Context>,
    request: ReviewRequest,
    decision: Decision,
    tally: Reduction,
    missing: Vec<String>,
    abstained: Vec<String>,
}

impl BoundReview {
    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    pub fn tally(&self) -> &Reduction {
        &self.tally
    }

    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    pub fn abstained(&self) -> &[String] {
        &self.abstained
    }

    pub fn action(&self) -> &FrozenAction {
        &self.context.action
    }

    pub fn policy(&self) -> &CongressPolicy {
        &self.context.spec.policy
    }

    pub fn apply(self, gate: &mut ConsequenceAuthority) -> Result<ControlReceipt, Error> {
        if !Rc::ptr_eq(&self.context.authority_issuer, &gate.authority.issuer) {
            return Err(Error::Binding);
        }
        gate.apply_review(self.request)
    }

    pub fn apply_to_containment(
        self,
        authority: &mut ContainmentAuthority,
    ) -> Result<ControlReceipt, Error> {
        if !Rc::ptr_eq(&self.context.authority_issuer, &authority.gate.authority.issuer) {
            return Err(Error::Binding);
        }
        authority.apply_review(self.request)
    }
}
