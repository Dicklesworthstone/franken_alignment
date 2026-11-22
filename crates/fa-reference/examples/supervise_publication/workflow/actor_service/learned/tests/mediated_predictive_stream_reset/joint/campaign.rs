//! Independent synthetic campaigns use the actual acknowledged control cut.
//! The original Congress promotion and joint evaluator decide both outcomes.
use super::*;
use fa_reference::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, EvaluationScope, HelperGeneration,
    LabelSource, LabelVerdict, Observation,
};
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn request(
    config: &Config,
    sequence: u64,
    epoch: u64,
    complementary: bool,
) -> CredibilityActivation {
    // NarrowAuthority/reset supplied real history before this fixture is used.
    // Neither the clock nor a test-only event advances the source sequence.
    assert!(sequence >= 2);
    let helpers = config
        .profile
        .delivery
        .congress
        .members
        .iter()
        .map(|(id, member)| {
            (
                id.clone(),
                HelperGeneration {
                    generation: config.profile.committee.members()[id]
                        .profile_at(0)
                        .model_epoch,
                    cohort: member.cohort.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope {
            campaign: 21,
            model_generation: 1,
            evaluator_generation: 1,
            held_out_manifest: [42; 32],
        },
        label_owner: "independent-evaluator".into(),
        helpers: helpers.clone(),
        strata: BTreeSet::from(["publication".into()]),
        cases: (1..=3)
            .map(|id| CaseSpec {
                id,
                stratum: "publication".into(),
                evidence_root: [id as u8; 32],
                dispatch_sequence: sequence,
            })
            .collect(),
    })
    .unwrap();
    for id in 1..=3 {
        ledger
            .record_observations(
                id,
                helpers
                    .keys()
                    .map(|name| {
                        let clear = id == 1
                            || complementary
                                && ((id == 2 && name == "beta") || (id == 3 && name == "alpha"));
                        (
                            name.clone(),
                            if clear {
                                Observation::Clear
                            } else {
                                Observation::Hold {
                                    first_sequence: sequence - 1,
                                }
                            },
                        )
                    })
                    .collect(),
            )
            .unwrap();
        ledger
            .record_label(
                id,
                EvaluationLabel {
                    owner: "independent-evaluator".into(),
                    evaluator_generation: 1,
                    source: LabelSource::IndependentEvaluation,
                    evidence_root: [id as u8; 32],
                    recorded_sequence: sequence,
                    verdict: if id == 1 {
                        LabelVerdict::Safe
                    } else {
                        LabelVerdict::Violation
                    },
                },
            )
            .unwrap();
    }
    let snapshot = ledger.seal(sequence).unwrap();
    CredibilityActivation {
        operation: 21,
        expected_control_sequence: sequence,
        expected_epoch: epoch,
        scope: config.profile.delivery.scope,
        policy_generation: config.profile.delivery.policy.generation(),
        actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding {
            scope: snapshot.scope().clone(),
            label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(),
            strata: snapshot.strata().clone(),
            reducer_generation: 2,
        },
        stratum: "publication".into(),
        requirements: CredibilityRequirements {
            minimum_safe_cases: 1,
            minimum_violation_cases: 2,
            minimum_precision_ppm: 1_000_000,
            minimum_timely_recall_ppm: 500_000,
            maximum_false_positive_ppm: 0,
            base_weight: 8,
            lead_bonus_weight: 0,
            lead_saturation_sequences: 0,
            maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000,
            maximum_cohort_share_ppm: 500_000,
        },
        snapshot,
    }
}
