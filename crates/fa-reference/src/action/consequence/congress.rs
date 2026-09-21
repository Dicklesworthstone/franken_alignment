//! Frozen round -> capped empirical tally -> consequence -> bound gate request.
//!
//! This deliberately small policy profile requires a substantive answer from
//! every frozen member. Missing reveals and abstentions remain separate facts;
//! either prevents Continue. A helper's Deny is an empirical hold recommendation,
//! never an exact disqualifier. Policy, contradiction and disqualifier facts are
//! trusted inputs. The underlying round remains a non-cryptographic oracle.

pub mod credibility;

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
    if round.member_count() > MAX_VOTES {
        return Err(Error::Limit);
    }
    policy.validate()?;
    let evidence_root: [u8; 32] = round.evidence_root().try_into().map_err(|_| Error::Binding)?;
    if round.id() == 0 || evidence_root == [0; 32] {
        return Err(Error::InvalidInput);
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

/// Fixed-point scale used by this explicit reference promotion profile.
pub const CREDIBILITY_PPM: u64 = 1_000_000;

/// Registered requirements, not statistically justified defaults. Every stratum
/// must independently pass; easy examples cannot average away a weak stratum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityRequirements {
    pub minimum_safe_cases: u64,
    pub minimum_violation_cases: u64,
    pub minimum_precision_ppm: u64,
    pub minimum_timely_recall_ppm: u64,
    pub maximum_false_positive_ppm: u64,
    pub base_weight: u64,
    pub lead_bonus_weight: u64,
    pub lead_saturation_sequences: u64,
    pub maximum_evidence_age: u64,
    pub maximum_member_share_ppm: u64,
    pub maximum_cohort_share_ppm: u64,
}

/// Trusted deployment facts, rechecked on every evaluated round. The generation
/// is the NEW reducer generation, never the generation whose weights we replace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityBinding {
    pub scope: credibility::EvaluationScope,
    pub label_owner: String,
    pub helpers: BTreeMap<String, credibility::HelperGeneration>,
    pub strata: BTreeSet<String>,
    pub reducer_generation: u64,
}

/// An immutable, explicitly promoted policy. This cannot retrain helpers, alter
/// its source snapshot or mint permits. Original policy thresholds and absolute
/// caps are preserved; only a new generation's member weights are derived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromotedCongressPolicy {
    policy: CongressPolicy,
    binding: CredibilityBinding,
    snapshot: credibility::CredibilitySnapshot,
    uncapped_weights: BTreeMap<String, u64>,
    valid_from: u64,
    valid_through: u64,
}

/// Carries the exact predecessor at which credibility freshness was checked.
/// Converting to a gate request cannot silently choose a later predecessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityEvaluation {
    evaluation: RoundEvaluation,
    control_sequence: u64,
}

impl CredibilityEvaluation {
    pub fn decision(&self) -> Decision {
        self.evaluation.decision()
    }

    pub fn tally(&self) -> &Reduction {
        self.evaluation.tally()
    }

    pub fn missing(&self) -> &[String] {
        self.evaluation.missing()
    }

    pub fn abstained(&self) -> &[String] {
        self.evaluation.abstained()
    }

    pub fn into_request(
        self,
        attempt: u64,
        action: FrozenAction,
        retained_targets: Option<TargetCeiling>,
    ) -> ReviewRequest {
        self.evaluation.into_request(attempt, self.control_sequence, action, retained_targets)
    }
}

impl CredibilityRequirements {
    fn validate(&self) -> Result<(), Error> {
        if self.minimum_safe_cases == 0 || self.minimum_violation_cases == 0
            || self.minimum_precision_ppm == 0 || self.minimum_precision_ppm > CREDIBILITY_PPM
            || self.minimum_timely_recall_ppm == 0
            || self.minimum_timely_recall_ppm > CREDIBILITY_PPM
            || self.maximum_false_positive_ppm > CREDIBILITY_PPM
            || self.base_weight == 0 || self.lead_bonus_weight > self.base_weight
            || (self.lead_bonus_weight != 0 && self.lead_saturation_sequences == 0)
            || self.maximum_evidence_age == 0
            || self.maximum_member_share_ppm == 0
            || self.maximum_member_share_ppm >= CREDIBILITY_PPM
            || self.maximum_cohort_share_ppm == 0
            || self.maximum_cohort_share_ppm >= CREDIBILITY_PPM
        {
            return Err(Error::InvalidInput);
        }
        self.base_weight.checked_add(self.lead_bonus_weight).ok_or(Error::Overflow)?;
        Ok(())
    }

    fn weight(&self, scores: &BTreeMap<String, credibility::CredibilityScore>) -> Result<u64, Error> {
        let mut weakest = u64::MAX;
        for score in scores.values() {
            // This conservative reference profile requires fully resolved,
            // complete held-out data. It does NOT estimate censored outcomes or
            // approve a deployment from a selectively reviewed subset.
            if score.pending != 0 || score.censored != 0 || score.missing != 0 || score.abstained != 0
                || score.safe < self.minimum_safe_cases
                || score.violations < self.minimum_violation_cases
            {
                return Err(Error::Incomplete);
            }
            let precision = ppm(score.precision().ok_or(Error::Incomplete)?)?;
            let timely_recall = ppm(score.timely_recall().ok_or(Error::Incomplete)?)?;
            if precision < self.minimum_precision_ppm || timely_recall < self.minimum_timely_recall_ppm
                || u128::from(score.false_positives) * u128::from(CREDIBILITY_PPM)
                    > u128::from(score.safe) * u128::from(self.maximum_false_positive_ppm)
            {
                return Err(Error::Incomplete);
            }
            let quality = precision.min(timely_recall);
            let base = u128::from(self.base_weight) * u128::from(quality)
                / u128::from(CREDIBILITY_PPM);
            // Divide by ALL violations, not just successes. Missing a violation
            // cannot improve the earliness score by shrinking its denominator.
            let average_lead = score.lead_time_credit / u128::from(score.violations);
            let bonus = if self.lead_bonus_weight == 0 {
                0
            } else {
                u128::from(self.lead_bonus_weight)
                    * average_lead.min(u128::from(self.lead_saturation_sequences))
                    / u128::from(self.lead_saturation_sequences)
            };
            let weight = u64::try_from(base + bonus).map_err(|_| Error::Overflow)?;
            weakest = weakest.min(weight);
        }
        if scores.is_empty() || weakest == 0 {
            return Err(Error::Incomplete);
        }
        Ok(weakest)
    }
}

fn ppm(ratio: credibility::Ratio) -> Result<u64, Error> {
    if ratio.denominator == 0 || ratio.numerator > ratio.denominator {
        return Err(Error::InvalidInput);
    }
    u64::try_from(u128::from(ratio.numerator) * u128::from(CREDIBILITY_PPM)
        / u128::from(ratio.denominator)).map_err(|_| Error::Overflow)
}

impl CongressPolicy {
    pub fn validate(&self) -> Result<(), Error> {
        if self.members.len() > MAX_VOTES {
            return Err(Error::Limit);
        }
        if self.generation == 0 || self.members.is_empty()
            || self.caps.per_member == 0 || self.caps.per_cohort == 0
            || self.continue_minimum == 0 || self.continue_hold_maximum >= self.narrow_at
            || self.narrow_at >= self.suspend_at || self.minimum_members == 0
            || self.minimum_cohorts == 0 || self.minimum_members > self.members.len()
            || self.minimum_cohorts > self.minimum_members
        {
            return Err(Error::InvalidInput);
        }
        for (member, profile) in &self.members {
            if member.is_empty() || profile.cohort.is_empty() || profile.weight == 0 {
                return Err(Error::InvalidInput);
            }
            if member.len() > MAX_IDENTIFIER_BYTES || profile.cohort.len() > MAX_IDENTIFIER_BYTES {
                return Err(Error::Limit);
            }
        }
        Ok(())
    }

    /// An explicit OFFLINE promotion: no mutation of `self`, no live learning,
    /// no lowering thresholds to make a weak or incomplete campaign pass.
    pub fn promote_credibility(
        &self,
        snapshot: credibility::CredibilitySnapshot,
        requirements: &CredibilityRequirements,
        binding: &CredibilityBinding,
        sequence: u64,
    ) -> Result<PromotedCongressPolicy, Error> {
        self.validate()?;
        requirements.validate()?;
        if binding.reducer_generation <= self.generation {
            return Err(Error::Stale);
        }
        if snapshot.scope() != &binding.scope || snapshot.label_owner() != binding.label_owner
            || snapshot.helpers() != &binding.helpers || snapshot.strata() != &binding.strata
            || !self.members.keys().eq(binding.helpers.keys())
        {
            return Err(Error::Binding);
        }
        for (helper, member) in &self.members {
            if member.cohort != binding.helpers[helper].cohort {
                return Err(Error::Binding);
            }
        }
        // Sealing old samples today does not make them fresh today.
        let valid_through = snapshot.oldest_case_sequence()
            .checked_add(requirements.maximum_evidence_age).ok_or(Error::Overflow)?;
        if sequence < snapshot.sealed_sequence() || sequence > valid_through {
            return Err(Error::Stale);
        }
        let mut policy = self.clone();
        policy.generation = binding.reducer_generation;
        let mut votes = Vec::new();
        let mut uncapped_weights = BTreeMap::new();
        for (helper, member) in &self.members {
            let scores = snapshot.scores().get(helper).ok_or(Error::Incomplete)?;
            let weight = requirements.weight(scores)?;
            uncapped_weights.insert(helper.clone(), weight);
            votes.push(Vote::Empirical {
                member: helper.clone(), cohort: member.cohort.clone(),
                recommendation: Recommendation::Permit,
            });
        }
        let capped = reduce(&votes, &uncapped_weights, self.caps, false)?;
        let total = capped.permit_weight;
        if total == 0 || total < self.continue_minimum
            || capped.admitted_weights.values().any(|weight| *weight == 0)
            || capped.admitted_cohort_weights.values().filter(|weight| **weight > 0).count()
                < self.minimum_cohorts
        {
            return Err(Error::Incomplete);
        }
        // Share caps use ACTUAL admitted influence, not a nominal budget. A
        // singleton or a renamed collection of one cohort cannot appear diverse.
        if capped.admitted_weights.values().any(|weight|
            u128::from(*weight) * u128::from(CREDIBILITY_PPM)
                > u128::from(total) * u128::from(requirements.maximum_member_share_ppm))
            || capped.admitted_cohort_weights.values().any(|weight|
                u128::from(*weight) * u128::from(CREDIBILITY_PPM)
                    > u128::from(total) * u128::from(requirements.maximum_cohort_share_ppm))
        {
            return Err(Error::Limit);
        }
        for (helper, member) in &mut policy.members {
            member.weight = capped.admitted_weights[helper];
        }
        // Store already capped weights so a missing reveal cannot redistribute
        // its cohort allocation to the remaining responders.
        Ok(PromotedCongressPolicy {
            policy, binding: binding.clone(), snapshot, uncapped_weights,
            valid_from: sequence, valid_through,
        })
    }
}

impl PromotedCongressPolicy {
    pub fn snapshot(&self) -> &credibility::CredibilitySnapshot {
        &self.snapshot
    }

    pub fn uncapped_weights(&self) -> &BTreeMap<String, u64> {
        &self.uncapped_weights
    }

    pub fn admitted_members(&self) -> &BTreeMap<String, MemberPolicy> {
        &self.policy.members
    }

    pub fn valid_through(&self) -> u64 {
        self.valid_through
    }

    /// The existing consequence reducer still handles missing reveals,
    /// abstentions, contradictions and exact disqualifiers. No fallback to stale
    /// or hand-supplied weights is performed when credibility checks fail.
    pub fn evaluate_round(
        &self,
        round: &Round,
        binding: &CredibilityBinding,
        stratum: &str,
        sequence: u64,
        exact_disqualifier: bool,
        contradiction: bool,
    ) -> Result<CredibilityEvaluation, Error> {
        if binding != &self.binding || !self.binding.strata.contains(stratum) {
            return Err(Error::Binding);
        }
        if sequence < self.valid_from || sequence > self.valid_through {
            return Err(Error::Stale);
        }
        let evaluation = evaluate_round(round, &self.policy, exact_disqualifier, contradiction)?;
        Ok(CredibilityEvaluation { evaluation, control_sequence: sequence })
    }
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

    fn requirements() -> CredibilityRequirements {
        CredibilityRequirements {
            minimum_safe_cases: 1,
            minimum_violation_cases: 1,
            minimum_precision_ppm: 900_000,
            minimum_timely_recall_ppm: 900_000,
            maximum_false_positive_ppm: 0,
            base_weight: 10,
            lead_bonus_weight: 10,
            lead_saturation_sequences: 10,
            maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000,
            maximum_cohort_share_ppm: 500_000,
        }
    }

    fn calibration(fault: &str, seal_at: u64) -> credibility::CredibilitySnapshot {
        use credibility::*;
        let mut campaign = Campaign {
            scope: EvaluationScope {
                campaign: 1, model_generation: 2, evaluator_generation: 3,
                held_out_manifest: [4; 32],
            },
            label_owner: "independent-evaluation".into(),
            helpers: BTreeMap::from([
                ("alice".into(), HelperGeneration { generation: 1, cohort: "a".into() }),
                ("bob".into(), HelperGeneration { generation: 1, cohort: "b".into() }),
            ]),
            strata: BTreeSet::from(["tool".into()]),
            cases: (1..=2).map(|id| CaseSpec {
                id, stratum: "tool".into(), evidence_root: [id as u8; 32], dispatch_sequence: 100,
            }).collect(),
        };
        if fault == "same-cohort" {
            campaign.helpers.get_mut("bob").unwrap().cohort = "a".into();
        }
        if fault == "weak-stratum" {
            campaign.strata.insert("publication".into());
            for id in 3..=4 {
                campaign.cases.push(CaseSpec {
                    id, stratum: "publication".into(), evidence_root: [id as u8; 32],
                    dispatch_sequence: 100,
                });
            }
        }
        let cases = campaign.cases.clone();
        let mut ledger = CredibilityLedger::new(campaign).unwrap();
        for case in cases {
            let violation = case.id % 2 == 1;
            let mut alice = if violation { Observation::Hold { first_sequence: 10 } }
                else { Observation::Clear };
            let bob = if violation { Observation::Hold { first_sequence: 20 } }
                else { Observation::Clear };
            match (fault, case.id) {
                ("missing", 1) => alice = Observation::Missing,
                ("abstained", 1) => alice = Observation::Abstain,
                ("late", 1) => alice = Observation::Hold { first_sequence: 100 },
                ("false-alarm", 2) => alice = Observation::Hold { first_sequence: 1 },
                ("weak-stratum", 3) => alice = Observation::Clear,
                _ => {}
            }
            ledger.record_observations(case.id,
                BTreeMap::from([("alice".into(), alice), ("bob".into(), bob)])).unwrap();
            if fault == "pending" && case.id == 2 {
                continue;
            }
            let verdict = if fault == "censored" && case.id == 2 { LabelVerdict::Censored }
                else if violation { LabelVerdict::Violation } else { LabelVerdict::Safe };
            ledger.record_label(case.id, EvaluationLabel {
                owner: "independent-evaluation".into(), evaluator_generation: 3,
                source: LabelSource::IndependentEvaluation,
                evidence_root: case.evidence_root, recorded_sequence: 120, verdict,
            }).unwrap();
        }
        ledger.seal(seal_at).unwrap()
    }

    fn deployment(snapshot: &credibility::CredibilitySnapshot) -> CredibilityBinding {
        CredibilityBinding {
            scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(),
            reducer_generation: 2,
        }
    }

    #[test]
    fn offline_promotion_preserves_policy_caps_and_does_not_update_live_weights() {
        let base = policy();
        let before = base.clone();
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let promoted = base.promote_credibility(snapshot, &requirements(), &binding, 120).unwrap();
        assert_eq!(base, before);
        assert_eq!(promoted.uncapped_weights()["alice"], 20);
        assert_eq!(promoted.uncapped_weights()["bob"], 10);
        assert_eq!(promoted.admitted_members()["alice"].weight, 5);
        assert_eq!(promoted.admitted_members()["bob"].weight, 5);
        assert_eq!(promoted.policy.continue_minimum, before.continue_minimum);
        assert_eq!(promoted.policy.caps, before.caps);
        assert_eq!(promoted.valid_through(), 200);
        assert_eq!(promoted.snapshot().case_label(1).unwrap().verdict,
            credibility::LabelVerdict::Violation);
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        let result = promoted.evaluate_round(&round, &binding, "tool", 120, false, false).unwrap();
        assert_eq!(result.decision().consequence, Consequence::Continue);
        assert_eq!(result.tally().permit_weight, 10);
        assert_eq!(result.evaluation.binding.reducer_generation, 2);
        assert_eq!(result.control_sequence, 120);
    }

    #[test]
    fn incomplete_weak_and_late_campaigns_cannot_be_promoted() {
        for fault in ["pending", "censored", "missing", "abstained", "late", "false-alarm", "weak-stratum"] {
            let snapshot = calibration(fault, 120);
            let binding = deployment(&snapshot);
            assert_eq!(policy().promote_credibility(snapshot, &requirements(), &binding, 120)
                .unwrap_err(), Error::Incomplete, "{fault}");
        }
    }

    #[test]
    fn fresh_sealing_cannot_launder_old_evidence_and_expiry_is_rechecked() {
        let snapshot = calibration("", 500);
        let binding = deployment(&snapshot);
        assert_eq!(policy().promote_credibility(snapshot, &requirements(), &binding, 500)
            .unwrap_err(), Error::Stale);
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let promoted = policy().promote_credibility(snapshot, &requirements(), &binding, 120).unwrap();
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        for sequence in [119, 201] {
            assert_eq!(promoted.evaluate_round(&round, &binding, "tool", sequence, false, false)
                .unwrap_err(), Error::Stale);
        }
        assert!(promoted.evaluate_round(&round, &binding, "tool", 200, false, false).is_ok());
    }

    #[test]
    fn model_helper_cohort_evaluator_manifest_and_reducer_drift_fail_closed() {
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let promoted = policy().promote_credibility(snapshot, &requirements(), &binding, 120).unwrap();
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        for mutation in 0..8 {
            let mut changed = binding.clone();
            match mutation {
                0 => changed.scope.model_generation += 1,
                1 => changed.helpers.get_mut("alice").unwrap().generation += 1,
                2 => changed.helpers.get_mut("alice").unwrap().cohort = "new-cohort".into(),
                3 => changed.scope.evaluator_generation += 1,
                4 => changed.scope.held_out_manifest = [99; 32],
                5 => changed.reducer_generation += 1,
                6 => changed.label_owner = "alice".into(),
                _ => { changed.strata.remove("tool"); }
            }
            assert_eq!(promoted.evaluate_round(&round, &changed, "tool", 120, false, false)
                .unwrap_err(), Error::Binding);
        }
        assert_eq!(promoted.evaluate_round(&round, &binding, "unqualified", 120, false, false)
            .unwrap_err(), Error::Binding);
    }

    #[test]
    fn promotion_cannot_reuse_generation_drop_members_or_reassign_cohorts() {
        let snapshot = calibration("", 120);
        let mut binding = deployment(&snapshot);
        binding.reducer_generation = 1;
        assert_eq!(policy().promote_credibility(snapshot.clone(), &requirements(), &binding, 120)
            .unwrap_err(), Error::Stale);
        binding.reducer_generation = 2;
        let mut base = policy();
        base.members.remove("bob");
        base.minimum_members = 1;
        base.minimum_cohorts = 1;
        assert_eq!(base.promote_credibility(snapshot.clone(), &requirements(), &binding, 120)
            .unwrap_err(), Error::Binding);
        let mut base = policy();
        base.members.get_mut("bob").unwrap().cohort = "a".into();
        assert_eq!(base.promote_credibility(snapshot, &requirements(), &binding, 120)
            .unwrap_err(), Error::Binding);
    }

    #[test]
    fn actual_member_and_cohort_shares_are_capped_not_nominal_budget_shares() {
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let mut limits = requirements();
        limits.maximum_member_share_ppm = 499_999;
        assert_eq!(policy().promote_credibility(snapshot, &limits, &binding, 120)
            .unwrap_err(), Error::Limit);
        let snapshot = calibration("same-cohort", 120);
        let binding = deployment(&snapshot);
        let mut base = policy();
        base.members.get_mut("bob").unwrap().cohort = "a".into();
        base.minimum_cohorts = 1;
        base.caps.per_cohort = 10;
        assert_eq!(base.promote_credibility(snapshot.clone(), &requirements(), &binding, 120)
            .unwrap_err(), Error::Limit);
        base.caps.per_cohort = 1;
        assert_eq!(base.promote_credibility(snapshot, &requirements(), &binding, 120)
            .unwrap_err(), Error::Incomplete);
    }

    #[test]
    fn promoted_round_keeps_missing_abstention_contradiction_and_exact_veto_semantics() {
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let promoted = policy().promote_credibility(snapshot, &requirements(), &binding, 120).unwrap();
        for verdict in [None, Some(Verdict::Abstain)] {
            let result = promoted.evaluate_round(&panel(Some(Verdict::Allow), verdict),
                &binding, "tool", 120, false, false).unwrap();
            assert_eq!(result.decision().consequence, Consequence::HoldEffect);
            assert_eq!(result.tally().permit_weight, 5);
            assert_eq!(result.missing().len() + result.abstained().len(), 1);
        }
        let round = panel(Some(Verdict::Allow), Some(Verdict::Allow));
        assert_eq!(promoted.evaluate_round(&round, &binding, "tool", 120, true, false)
            .unwrap().decision().consequence, Consequence::Deny);
        assert_eq!(promoted.evaluate_round(&round, &binding, "tool", 120, false, true)
            .unwrap().decision().consequence, Consequence::HoldEffect);
    }

    #[test]
    fn unsatisfiable_thresholds_and_overflow_never_trigger_automatic_relaxation() {
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let mut base = policy();
        base.continue_minimum = 11;
        assert_eq!(base.promote_credibility(snapshot.clone(), &requirements(), &binding, 120)
            .unwrap_err(), Error::Incomplete);
        assert_eq!(base.continue_minimum, 11);
        let mut limits = requirements();
        limits.base_weight = u64::MAX;
        limits.lead_bonus_weight = 1;
        assert_eq!(policy().promote_credibility(snapshot.clone(), &limits, &binding, 120)
            .unwrap_err(), Error::Overflow);
        let mut limits = requirements();
        limits.maximum_evidence_age = u64::MAX;
        assert_eq!(policy().promote_credibility(snapshot, &limits, &binding, 120)
            .unwrap_err(), Error::Overflow);
    }

    #[test]
    fn credibility_request_pins_generation_and_checked_control_predecessor() {
        use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
        let snapshot = calibration("", 120);
        let binding = deployment(&snapshot);
        let promoted = policy().promote_credibility(snapshot, &requirements(), &binding, 120).unwrap();
        let result = promoted.evaluate_round(&panel(Some(Verdict::Allow), Some(Verdict::Allow)),
            &binding, "tool", 125, false, false).unwrap();
        let action = FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: Scope { tenant: 1, principal: 1, run: 1, branch: 1, authority: 1, purpose: Purpose::Effect },
            target: Some(ResolvedTarget {
                adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1,
            }),
            payload: b"reviewed-effect".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: 1, deadline: ElapsedTick(1000), units: 1,
        }).unwrap();
        let request = result.into_request(7, action.clone(), None);
        assert_eq!(request.expected_control_sequence, 125);
        assert_eq!(request.binding.reducer_generation, 2);
        assert_eq!(request.binding.round, 7);
        assert_eq!(request.action, action);
    }
}
