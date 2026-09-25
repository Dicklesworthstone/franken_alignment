//! Frozen congress policy and transcript-to-reducer integration.
//!
//! A bounded reference profile, not production consensus or authentication.
//! The enclosing round still uses its non-cryptographic comparison oracle.
//! Membership, cohorts, weights and thresholds freeze before any commitment.
//! Missing reveals and abstentions cannot become affirmative observations.

use super::{Digest, MemberOutcome, Phase, Round, Verdict};
use crate::Error;
use crate::reducer::{self, Caps, Recommendation, Reduction, Vote};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: String,
    pub cohort: String,
    pub weight: u64,
}

/// A registered reference policy, not inferred statistical independence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub caps: Caps,
    pub min_permit_weight: u64,
    pub max_hold_weight: u64,
    pub min_permit_cohorts: usize,
}

/// Immutable policy. There is deliberately no post-commit reweighting API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewPolicy {
    members: BTreeMap<String, Member>,
    requirements: Requirements,
}

impl ReviewPolicy {
    pub fn new(members: Vec<Member>, requirements: Requirements) -> Result<Self, Error> {
        if members.is_empty() {
            return Err(Error::Missing);
        }
        if members.len() > reducer::MAX_VOTES {
            return Err(Error::Limit);
        }
        if requirements.caps.per_member == 0
            || requirements.caps.per_cohort == 0
            || requirements.min_permit_weight == 0
            || requirements.min_permit_cohorts == 0
        {
            return Err(Error::InvalidInput);
        }
        let mut registered = BTreeMap::new();
        for member in members {
            if member.id.is_empty() || member.cohort.is_empty() {
                return Err(Error::InvalidInput);
            }
            if member.id.len() > reducer::MAX_IDENTIFIER_BYTES
                || member.cohort.len() > reducer::MAX_IDENTIFIER_BYTES
            {
                return Err(Error::Limit);
            }
            if registered.insert(member.id.clone(), member).is_some() {
                return Err(Error::Duplicate);
            }
        }
        let policy = Self {
            members: registered,
            requirements,
        };
        // Check reachability after BOTH influence caps, not raw member weights.
        // This also refuses policies whose complete tally would overflow u64.
        let votes = policy.votes(|_| Recommendation::Permit);
        let maximum = reducer::reduce(&votes, &policy.weights(), requirements.caps, false)?;
        let active_cohorts = maximum
            .admitted_cohort_weights
            .values()
            .filter(|weight| **weight > 0)
            .count();
        if requirements.min_permit_weight > maximum.permit_weight
            || requirements.min_permit_cohorts > active_cohorts
        {
            return Err(Error::InvalidInput);
        }
        Ok(policy)
    }

    pub fn requirements(&self) -> Requirements {
        self.requirements
    }

    fn weights(&self) -> BTreeMap<String, u64> {
        self.members
            .iter()
            .map(|(id, member)| (id.clone(), member.weight))
            .collect()
    }

    fn votes(&self, mut recommendation: impl FnMut(&str) -> Recommendation) -> Vec<Vote> {
        self.members
            .iter()
            .map(|(id, member)| Vote::Empirical {
                member: id.clone(),
                cohort: member.cohort.clone(),
                recommendation: recommendation(id),
            })
            .collect()
    }
}

/// Caller-supplied exact-validator status, separate from every empirical vote.
/// `Clear` is a declared reference fact, NOT proof that a validator ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactStatus {
    Clear,
    Disqualified,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoldReason {
    ExactEvidenceUnknown,
    IndependentPhaseOpen,
    MissingReveals(Vec<String>),
    Abstentions(Vec<String>),
    InsufficientPermitWeight,
    ExcessHoldWeight,
    InsufficientPermitCohorts,
}

/// A diagnostic decision, never an authority-bearing permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    Ready(Reduction),
    Hold(HoldReason),
    Disqualified,
}

/// Owns the only mutable transcript for a pre-registered policy and evidence view.
/// A clone is merely another reference experiment; it conveys no action rights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CongressReview {
    policy: ReviewPolicy,
    round: Round,
}

impl CongressReview {
    pub fn new(policy: ReviewPolicy, round_id: u64, evidence_root: &[u8]) -> Result<Self, Error> {
        if round_id == 0 {
            return Err(Error::InvalidInput);
        }
        let mut round = Round::new(round_id, evidence_root)?;
        for member in policy.members.keys() {
            round.add_member(member)?;
        }
        Ok(Self { policy, round })
    }

    pub fn transcript(&self) -> &Round {
        &self.round
    }

    pub fn commit(&mut self, member: &str, digest: Digest) -> Result<(), Error> {
        self.round.commit(member, digest)
    }

    pub fn open_reveals(&mut self) -> Result<(), Error> {
        self.round.open_reveals()
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        self.round.reveal(member, verdict, salt)
    }

    pub fn evaluate(&self, exact: ExactStatus) -> Result<ReviewDecision, Error> {
        match exact {
            ExactStatus::Disqualified => return Ok(ReviewDecision::Disqualified),
            ExactStatus::Unknown => {
                return Ok(ReviewDecision::Hold(HoldReason::ExactEvidenceUnknown));
            }
            ExactStatus::Clear => {}
        }
        if self.round.phase() != Phase::Reveal {
            return Ok(ReviewDecision::Hold(HoldReason::IndependentPhaseOpen));
        }
        let mut missing = Vec::new();
        let mut abstaining = Vec::new();
        let mut recommendations = BTreeMap::new();
        for member in self.policy.members.keys() {
            match self.round.outcome(member)? {
                MemberOutcome::Missing => missing.push(member.clone()),
                MemberOutcome::Revealed(Verdict::Abstain) => abstaining.push(member.clone()),
                MemberOutcome::Revealed(Verdict::Allow) => {
                    recommendations.insert(member.as_str(), Recommendation::Permit);
                }
                MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny) => {
                    // An empirical Deny is not an exact disqualifier.
                    recommendations.insert(member.as_str(), Recommendation::Hold);
                }
            }
        }
        if !missing.is_empty() {
            return Ok(ReviewDecision::Hold(HoldReason::MissingReveals(missing)));
        }
        if !abstaining.is_empty() {
            return Ok(ReviewDecision::Hold(HoldReason::Abstentions(abstaining)));
        }
        let votes = self.policy.votes(|member| recommendations[member]);
        let requirements = self.policy.requirements;
        let reduction = reducer::reduce(&votes, &self.policy.weights(), requirements.caps, false)?;
        if reduction.permit_weight < requirements.min_permit_weight {
            return Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitWeight));
        }
        if reduction.hold_weight > requirements.max_hold_weight {
            return Ok(ReviewDecision::Hold(HoldReason::ExcessHoldWeight));
        }
        let permit_cohorts: BTreeSet<_> = self
            .policy
            .members
            .iter()
            .filter(|(id, _)| {
                recommendations[id.as_str()] == Recommendation::Permit
                    && reduction.admitted_weights[*id] > 0
            })
            .map(|(_, member)| member.cohort.as_str())
            .collect();
        if permit_cohorts.len() < requirements.min_permit_cohorts {
            return Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitCohorts));
        }
        Ok(ReviewDecision::Ready(reduction))
    }
}
