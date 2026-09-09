//! Frozen round -> capped empirical tally -> consequence -> bound gate request.
//!
//! This deliberately small policy profile requires a substantive answer from
//! every frozen member. Missing reveals and abstentions remain separate facts;
//! either prevents Continue. A helper's Deny is an empirical hold recommendation,
//! never an exact disqualifier. Policy, contradiction and disqualifier facts are
//! trusted inputs. The underlying round remains a non-cryptographic oracle.

use super::gate::{ReviewBinding, ReviewRequest, TargetCeiling};
use super::{Decision, DecisionInputs, Restriction, decide};
use crate::Error;
use crate::action::FrozenAction;
use crate::reducer::{
    Caps, MAX_IDENTIFIER_BYTES, MAX_VOTES, Recommendation, Reduction, Vote, reduce,
};
use crate::round::{MemberOutcome, Phase, Round, Verdict};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberPolicy {
    pub cohort: String,
    pub weight: u64,
}

/// An explicitly supplied policy table, not a universal majority-vote rule.
/// Influence is clipped by the existing reducer before thresholds are tested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CongressPolicy {
    pub generation: u64,
    pub members: BTreeMap<String, MemberPolicy>,
    pub caps: Caps,
    pub continue_minimum: u64,
    pub continue_hold_maximum: u64,
    pub narrow_at: u64,
    pub suspend_at: u64,
    pub minimum_members: usize,
    pub minimum_cohorts: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundEvaluation {
    binding: ReviewBinding,
    inputs: DecisionInputs,
    tally: Reduction,
    missing: Vec<String>,
    abstained: Vec<String>,
}

impl RoundEvaluation {
    pub fn decision(&self) -> Decision {
        decide(self.inputs)
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

    /// The caller supplies the exact action and expected predecessor; the gate
    /// rechecks both. This produces data, not a Permit. Reusing an already-applied
    /// round is rejected by the gate even if later reveals have arrived.
    pub fn into_request(
        self,
        attempt: u64,
        expected_control_sequence: u64,
        action: FrozenAction,
        retained_targets: Option<TargetCeiling>,
    ) -> ReviewRequest {
        ReviewRequest {
            attempt,
            expected_control_sequence,
            action,
            binding: self.binding,
            inputs: self.inputs,
            retained_targets,
        }
    }
}

pub fn evaluate_round(
    round: &Round,
    policy: &CongressPolicy,
    exact_disqualifier: bool,
    contradiction: bool,
) -> Result<RoundEvaluation, Error> {
    if round.phase() != Phase::Reveal {
        return Err(Error::WrongState);
    }
    if policy.members.len() > MAX_VOTES || round.member_count() > MAX_VOTES {
        return Err(Error::Limit);
    }
    if policy.generation == 0
        || round.id() == 0
        || policy.members.is_empty()
        || policy.caps.per_member == 0
        || policy.caps.per_cohort == 0
        || policy.continue_minimum == 0
        || policy.continue_hold_maximum >= policy.narrow_at
        || policy.narrow_at >= policy.suspend_at
        || policy.minimum_members == 0
        || policy.minimum_cohorts == 0
        || policy.minimum_members > policy.members.len()
        || policy.minimum_cohorts > policy.minimum_members
    {
        return Err(Error::InvalidInput);
    }
    let evidence_root: [u8; 32] = round.evidence_root().try_into().map_err(|_| Error::Binding)?;
    if evidence_root == [0; 32] {
        return Err(Error::InvalidInput);
    }
    for (member, profile) in &policy.members {
        if member.is_empty() || profile.cohort.is_empty() || profile.weight == 0 {
            return Err(Error::InvalidInput);
        }
        if member.len() > MAX_IDENTIFIER_BYTES || profile.cohort.len() > MAX_IDENTIFIER_BYTES {
            return Err(Error::Limit);
        }
    }
    let outcomes = round.outcomes();
    if outcomes.keys().copied().collect::<BTreeSet<_>>()
        != policy.members.keys().map(String::as_str).collect::<BTreeSet<_>>()
    {
        return Err(Error::Binding);
    }
    let mut votes = Vec::new();
    let mut weights = BTreeMap::new();
    let mut missing = Vec::new();
    let mut abstained = Vec::new();
    for (member, outcome) in outcomes {
        let recommendation = match outcome {
            MemberOutcome::Missing => {
                missing.push(member.to_owned());
                continue;
            }
            MemberOutcome::Revealed(Verdict::Abstain) => {
                abstained.push(member.to_owned());
                continue;
            }
            MemberOutcome::Revealed(Verdict::Allow) => Recommendation::Permit,
            MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny) => Recommendation::Hold,
        };
        let profile = &policy.members[member];
        votes.push(Vote::Empirical {
            member: member.to_owned(),
            cohort: profile.cohort.clone(),
            recommendation,
        });
        weights.insert(member.to_owned(), profile.weight);
    }
    let tally = reduce(&votes, &weights, policy.caps, exact_disqualifier)?;
    let positive_members = tally.admitted_weights.values().filter(|w| **w > 0).count();
    let positive_cohorts = tally
        .admitted_cohort_weights
        .values()
        .filter(|w| **w > 0)
        .count();
    let empirical = if tally.hold_weight >= policy.suspend_at {
        Restriction::SuspendRun
    } else if tally.hold_weight >= policy.narrow_at {
        Restriction::NarrowAuthority
    } else if tally.permit_weight >= policy.continue_minimum
        && tally.hold_weight <= policy.continue_hold_maximum
        && positive_members >= policy.minimum_members
        && positive_cohorts >= policy.minimum_cohorts
    {
        Restriction::Continue
    } else {
        Restriction::HoldEffect
    };
    Ok(RoundEvaluation {
        binding: ReviewBinding {
            round: round.id(),
            evidence_root,
            reducer_generation: policy.generation,
        },
        inputs: DecisionInputs {
            empirical,
            exact_disqualifier,
            mandatory_absent: !missing.is_empty() || !abstained.is_empty(),
            contradiction,
        },
        tally,
        missing,
        abstained,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::Consequence;
    use crate::round::commitment;

    fn policy() -> CongressPolicy {
        CongressPolicy {
            generation: 1,
            members: BTreeMap::from([
                ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 5 }),
                ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 5 }),
            ]),
            caps: Caps { per_member: 5, per_cohort: 5 },
            continue_minimum: 8,
            continue_hold_maximum: 0,
            narrow_at: 8,
            suspend_at: 10,
            minimum_members: 2,
            minimum_cohorts: 2,
        }
    }

    fn panel(alice: Option<Verdict>, bob: Option<Verdict>) -> Round {
        let mut round = Round::new(7, &[9; 32]).unwrap();
        round.add_member("alice").unwrap();
        round.add_member("bob").unwrap();
        for (member, verdict) in [("alice", alice), ("bob", bob)] {
            let digest = commitment(
                round.id(), member, round.evidence_root(),
                verdict.unwrap_or(Verdict::Allow), b"reference-salt",
            ).unwrap();
            round.commit(member, digest).unwrap();
        }
        round.open_reveals().unwrap();
        for (member, verdict) in [("alice", alice), ("bob", bob)] {
            if let Some(verdict) = verdict {
                round.reveal(member, verdict, b"reference-salt").unwrap();
            }
        }
        round
    }

    #[test]
    fn complete_round_uses_capped_policy_and_disqualifier_dominates() {
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        let evaluation = evaluate_round(&round, &policy(), false, false).unwrap();
        assert_eq!(evaluation.decision().consequence, Consequence::Continue);
        assert_eq!(evaluation.tally().permit_weight, 10);
        assert!(evaluation.missing().is_empty());
        assert_eq!(
            evaluate_round(&round, &policy(), true, false).unwrap().decision().consequence,
            Consequence::Deny
        );
        assert_eq!(
            evaluate_round(&round, &policy(), false, true).unwrap().decision().consequence,
            Consequence::HoldEffect
        );
    }

    #[test]
    fn missing_and_abstaining_members_are_distinct_and_never_silently_dropped() {
        let mut policy = policy();
        policy.continue_minimum = 1;
        policy.minimum_members = 1;
        policy.minimum_cohorts = 1;
        let missing = evaluate_round(
            &panel(Some(Verdict::Allow), None), &policy, false, false,
        ).unwrap();
        let abstained = evaluate_round(
            &panel(Some(Verdict::Allow), Some(Verdict::Abstain)), &policy, false, false,
        ).unwrap();
        assert_eq!(missing.missing(), &["bob".to_owned()]);
        assert!(missing.abstained().is_empty());
        assert_eq!(abstained.abstained(), &["bob".to_owned()]);
        assert!(abstained.missing().is_empty());
        assert_eq!(missing.decision().consequence, Consequence::HoldEffect);
        assert_eq!(abstained.decision().consequence, Consequence::HoldEffect);
    }

    #[test]
    fn helper_deny_is_not_an_exact_disqualifier() {
        let round = panel(Some(Verdict::Allow), Some(Verdict::Deny));
        let evaluation = evaluate_round(&round, &policy(), false, false).unwrap();
        assert_eq!(evaluation.decision().consequence, Consequence::HoldEffect);
        assert_eq!(evaluation.tally().hold_weight, 5);
        assert!(!evaluation.inputs.exact_disqualifier);
    }

    #[test]
    fn registered_score_bands_select_narrowing_and_suspension() {
        let round = panel(Some(Verdict::Hold), Some(Verdict::Hold));
        let mut policy = policy();
        assert_eq!(
            evaluate_round(&round, &policy, false, false).unwrap().decision().consequence,
            Consequence::SuspendRun
        );
        policy.suspend_at = 11;
        assert_eq!(
            evaluate_round(&round, &policy, false, false).unwrap().decision().consequence,
            Consequence::NarrowAuthority
        );
    }

    #[test]
    fn zero_effective_influence_cannot_satisfy_a_quorum() {
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        let mut policy = policy();
        policy.members.get_mut("bob").unwrap().cohort = "a".to_owned();
        policy.caps.per_cohort = 1;
        policy.minimum_cohorts = 1;
        policy.continue_minimum = 1;
        let evaluation = evaluate_round(&round, &policy, false, false).unwrap();
        assert_eq!(evaluation.tally().permit_weight, 0);
        assert_eq!(evaluation.decision().consequence, Consequence::HoldEffect);
    }

    #[test]
    fn membership_phase_and_policy_errors_fail_closed() {
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        let mut invalid = policy();
        invalid.members.remove("bob");
        invalid.minimum_members = 1;
        invalid.minimum_cohorts = 1;
        assert_eq!(evaluate_round(&round, &invalid, false, false).unwrap_err(), Error::Binding);
        let mut invalid = policy();
        invalid.continue_minimum = 0;
        assert_eq!(evaluate_round(&round, &invalid, false, false).unwrap_err(), Error::InvalidInput);
        let mut invalid = policy();
        invalid.suspend_at = invalid.narrow_at;
        assert_eq!(evaluate_round(&round, &invalid, false, false).unwrap_err(), Error::InvalidInput);
        let unopened = Round::new(7, &[9; 32]).unwrap();
        assert_eq!(evaluate_round(&unopened, &policy(), false, false).unwrap_err(), Error::WrongState);
    }
}
