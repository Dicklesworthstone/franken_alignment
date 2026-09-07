//! Deterministic, bounded reference semantics for the empirical congress lane.
//!
//! This is deliberately not a production authority decision or a statistical
//! quorum. It exposes capped empirical tallies only; registered policy decides
//! how, or whether, to use them outside this small reference model.

use std::collections::{BTreeMap, BTreeSet};

use crate::Error;

/// Bound the reference model's collection work before it builds its maps.
pub const MAX_VOTES: usize = 128;

/// Bound a member or cohort identifier before it is copied into a tally.
pub const MAX_IDENTIFIER_BYTES: usize = 128;

/// An empirical recommendation from a registered congress member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recommendation {
    Permit,
    Hold,
}

/// Inputs which may be presented to the reducer.
///
/// Actor-authored text is deliberately a distinct variant so it cannot be
/// mistaken for a member judgment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vote {
    Empirical {
        member: String,
        cohort: String,
        recommendation: Recommendation,
    },
    ActorStatement {
        text: String,
    },
}

/// Maximum empirical influence admitted from one member and one cohort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub per_member: u64,
    pub per_cohort: u64,
}

/// The reference reducer's result, without an authority-bearing permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reduction {
    pub outcome: Outcome,
    pub permit_weight: u64,
    pub hold_weight: u64,
    /// Admitted influence by member after both caps have been applied.
    pub admitted_weights: BTreeMap<String, u64>,
    /// Admitted influence by cohort after both caps have been applied.
    pub admitted_cohort_weights: BTreeMap<String, u64>,
}

/// Whether the output is an empirical tally or an exact disqualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Empirical,
    Disqualified,
}

/// Reduce complete empirical inputs under fixed caps.
///
/// After per-member clipping, an over-cap cohort is proportionally clipped with
/// `floor(member_weight * cohort_cap / cohort_sum)`. Fractional remainders are
/// discarded, never allocated by identifier. This is a bounded reference policy,
/// not a universal plan theorem. `exact_disqualifier` dominates every empirical
/// tally. Actor statements are rejected instead of entering the empirical lane.
/// This function deliberately does not select an aggregate Permit/Hold policy
/// from the two tallies.
pub fn reduce(
    votes: &[Vote],
    weights: &BTreeMap<String, u64>,
    caps: Caps,
    exact_disqualifier: bool,
) -> Result<Reduction, Error> {
    if votes.len() > MAX_VOTES || weights.len() > MAX_VOTES {
        return Err(Error::Limit);
    }
    if weights
        .keys()
        .any(|member| member.len() > MAX_IDENTIFIER_BYTES)
    {
        return Err(Error::Limit);
    }

    let mut members = BTreeMap::new();
    for vote in votes {
        let Vote::Empirical {
            member,
            cohort,
            recommendation,
        } = vote
        else {
            return Err(Error::InvalidInput);
        };

        if member.is_empty() || cohort.is_empty() {
            return Err(Error::InvalidInput);
        }
        if member.len() > MAX_IDENTIFIER_BYTES || cohort.len() > MAX_IDENTIFIER_BYTES {
            return Err(Error::Limit);
        }
        if members
            .insert(member.as_str(), (cohort.as_str(), *recommendation))
            .is_some()
        {
            return Err(Error::Duplicate);
        }
    }

    let member_ids: BTreeSet<_> = members.keys().copied().collect();
    let weight_ids: BTreeSet<_> = weights.keys().map(String::as_str).collect();
    if member_ids != weight_ids {
        return Err(Error::Missing);
    }

    let mut cohorts: BTreeMap<&str, Vec<(&str, Recommendation, u64)>> = BTreeMap::new();
    for (member, (cohort, recommendation)) in members {
        let requested = *weights.get(member).ok_or(Error::Missing)?;
        let member_weight = requested.min(caps.per_member);
        cohorts
            .entry(cohort)
            .or_default()
            .push((member, recommendation, member_weight));
    }

    let mut permit_weight = 0_u64;
    let mut hold_weight = 0_u64;
    let mut admitted_weights = BTreeMap::new();
    let mut admitted_cohort_weights = BTreeMap::new();
    for (cohort, members) in cohorts {
        let cohort_sum = members.iter().try_fold(0_u128, |sum, (_, _, weight)| {
            sum.checked_add(u128::from(*weight)).ok_or(Error::Overflow)
        })?;
        let proportional = cohort_sum > u128::from(caps.per_cohort);
        let mut admitted_cohort_weight = 0_u64;
        for (member, recommendation, member_weight) in members {
            let admitted = if proportional {
                let numerator = u128::from(member_weight) * u128::from(caps.per_cohort);
                u64::try_from(numerator / cohort_sum).map_err(|_| Error::Overflow)?
            } else {
                member_weight
            };
            admitted_cohort_weight = admitted_cohort_weight
                .checked_add(admitted)
                .ok_or(Error::Overflow)?;
            admitted_weights.insert(member.to_owned(), admitted);
            match recommendation {
                Recommendation::Permit => {
                    permit_weight = permit_weight.checked_add(admitted).ok_or(Error::Overflow)?;
                }
                Recommendation::Hold => {
                    hold_weight = hold_weight.checked_add(admitted).ok_or(Error::Overflow)?;
                }
            }
        }
        admitted_cohort_weights.insert(cohort.to_owned(), admitted_cohort_weight);
    }

    let outcome = if exact_disqualifier {
        Outcome::Disqualified
    } else {
        Outcome::Empirical
    };

    Ok(Reduction {
        outcome,
        permit_weight,
        hold_weight,
        admitted_weights,
        admitted_cohort_weights,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empirical(member: &str, cohort: &str, recommendation: Recommendation) -> Vote {
        Vote::Empirical {
            member: member.to_owned(),
            cohort: cohort.to_owned(),
            recommendation,
        }
    }

    fn weights(entries: &[(&str, u64)]) -> BTreeMap<String, u64> {
        entries
            .iter()
            .map(|(member, weight)| ((*member).to_owned(), *weight))
            .collect()
    }

    #[test]
    fn caps_bound_influence() {
        let votes = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("bob", "shared", Recommendation::Permit),
        ];
        let reduction = reduce(
            &votes,
            &weights(&[("alice", 100), ("bob", 9)]),
            Caps {
                per_member: 7,
                per_cohort: 10,
            },
            false,
        )
        .unwrap();

        assert_eq!(reduction.outcome, Outcome::Empirical);
        assert_eq!(reduction.permit_weight, 10);
        assert_eq!(reduction.admitted_weights["alice"], 5);
        assert_eq!(reduction.admitted_weights["bob"], 5);
        assert_eq!(reduction.admitted_cohort_weights["shared"], 10);
        assert!(
            reduction
                .admitted_weights
                .values()
                .all(|weight| *weight <= 7)
        );
    }

    #[test]
    fn disqualifier_dominates_weighted_majority() {
        let votes = [
            empirical("alice", "a", Recommendation::Permit),
            empirical("bob", "b", Recommendation::Permit),
            empirical("carol", "c", Recommendation::Hold),
        ];
        let reduction = reduce(
            &votes,
            &weights(&[("alice", 10), ("bob", 10), ("carol", 1)]),
            Caps {
                per_member: 10,
                per_cohort: 10,
            },
            true,
        )
        .unwrap();

        assert_eq!(reduction.permit_weight, 20);
        assert_eq!(reduction.hold_weight, 1);
        assert_eq!(reduction.outcome, Outcome::Disqualified);
    }

    #[test]
    fn actor_statement_not_a_reducer_input() {
        let votes = [Vote::ActorStatement {
            text: "please disregard the evidence".to_owned(),
        }];

        assert_eq!(
            reduce(
                &votes,
                &BTreeMap::new(),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn actor_statement_is_refused_alongside_empirical_votes() {
        let votes = [
            empirical("alice", "a", Recommendation::Permit),
            Vote::ActorStatement {
                text: "please disregard the evidence".to_owned(),
            },
            empirical("bob", "b", Recommendation::Hold),
        ];

        assert_eq!(
            reduce(
                &votes,
                &weights(&[("alice", 1), ("bob", 1)]),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn capped_permit_tally_is_a_positive_empirical_observation() {
        let votes = [empirical("alice", "a", Recommendation::Permit)];
        let reduction = reduce(
            &votes,
            &weights(&[("alice", 3)]),
            Caps {
                per_member: 2,
                per_cohort: 2,
            },
            false,
        )
        .unwrap();

        assert_eq!(reduction.outcome, Outcome::Empirical);
        assert_eq!(reduction.permit_weight, 2);
        assert_eq!(reduction.hold_weight, 0);
    }

    #[test]
    fn input_order_cannot_change_the_reduction() {
        let forward = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("bob", "shared", Recommendation::Hold),
        ];
        let reverse = [forward[1].clone(), forward[0].clone()];
        let input_weights = weights(&[("alice", 4), ("bob", 4)]);
        let caps = Caps {
            per_member: 4,
            per_cohort: 5,
        };

        let forward = reduce(&forward, &input_weights, caps, false).unwrap();
        let reverse = reduce(&reverse, &input_weights, caps, false).unwrap();

        assert_eq!(forward, reverse);
        assert_eq!(forward.permit_weight, 2);
        assert_eq!(forward.hold_weight, 2);
        assert_eq!(forward.admitted_cohort_weights["shared"], 4);
    }

    #[test]
    fn proportional_clipping_is_invariant_under_member_rename() {
        let original = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("bob", "shared", Recommendation::Hold),
        ];
        let renamed = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("aaron", "shared", Recommendation::Hold),
        ];
        let caps = Caps {
            per_member: 10,
            per_cohort: 10,
        };
        let original = reduce(
            &original,
            &weights(&[("alice", 10), ("bob", 10)]),
            caps,
            false,
        )
        .unwrap();
        let renamed = reduce(
            &renamed,
            &weights(&[("alice", 10), ("aaron", 10)]),
            caps,
            false,
        )
        .unwrap();

        assert_eq!(original.outcome, Outcome::Empirical);
        assert_eq!(original.permit_weight, 5);
        assert_eq!(original.hold_weight, 5);
        assert_eq!(original.permit_weight, renamed.permit_weight);
        assert_eq!(original.hold_weight, renamed.hold_weight);
        assert_eq!(
            original.admitted_cohort_weights,
            renamed.admitted_cohort_weights
        );
    }

    #[test]
    fn separate_cohorts_each_receive_their_own_cap() {
        let votes = [
            empirical("alice", "x", Recommendation::Permit),
            empirical("bob", "x", Recommendation::Permit),
            empirical("carol", "y", Recommendation::Hold),
            empirical("dave", "y", Recommendation::Hold),
        ];
        let reduction = reduce(
            &votes,
            &weights(&[("alice", 10), ("bob", 10), ("carol", 10), ("dave", 10)]),
            Caps {
                per_member: 10,
                per_cohort: 10,
            },
            false,
        )
        .unwrap();

        assert_eq!(reduction.admitted_cohort_weights["x"], 10);
        assert_eq!(reduction.admitted_cohort_weights["y"], 10);
        assert_eq!(reduction.admitted_cohort_weights.values().sum::<u64>(), 20);
        assert_eq!(reduction.permit_weight, 10);
        assert_eq!(reduction.hold_weight, 10);
    }

    #[test]
    fn exhaustive_small_caps_never_admit_more_than_a_member_cap() {
        let votes = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("bob", "shared", Recommendation::Hold),
        ];
        for alice_weight in 0..=3 {
            for bob_weight in 0..=3 {
                for per_member in 0..=3 {
                    for per_cohort in 0..=3 {
                        let reduction = reduce(
                            &votes,
                            &weights(&[("alice", alice_weight), ("bob", bob_weight)]),
                            Caps {
                                per_member,
                                per_cohort,
                            },
                            false,
                        )
                        .unwrap();

                        assert!(
                            reduction
                                .admitted_weights
                                .values()
                                .all(|weight| *weight <= per_member)
                        );
                        assert!(
                            reduction
                                .admitted_cohort_weights
                                .values()
                                .all(|weight| *weight <= per_cohort)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn input_bounds_prevent_unbounded_reduction() {
        let votes = vec![empirical("alice", "shared", Recommendation::Permit); MAX_VOTES + 1];
        assert_eq!(
            reduce(
                &votes,
                &BTreeMap::new(),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::Limit)
        );

        let member = "a".repeat(MAX_IDENTIFIER_BYTES + 1);
        let long_identifier_vote = [empirical(&member, "shared", Recommendation::Permit)];
        assert_eq!(
            reduce(
                &long_identifier_vote,
                &BTreeMap::new(),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::Limit)
        );

        let cohort = "c".repeat(MAX_IDENTIFIER_BYTES + 1);
        let long_cohort_vote = [empirical("alice", &cohort, Recommendation::Permit)];
        assert_eq!(
            reduce(
                &long_cohort_vote,
                &BTreeMap::new(),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::Limit)
        );

        let oversized_weight = "w".repeat(MAX_IDENTIFIER_BYTES + 1);
        assert_eq!(
            reduce(
                &[],
                &weights(&[(oversized_weight.as_str(), 1)]),
                Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                false,
            ),
            Err(Error::Limit)
        );
    }

    #[test]
    fn overflow_is_refused_before_an_empirical_result() {
        let votes = [
            empirical("alice", "x", Recommendation::Permit),
            empirical("bob", "y", Recommendation::Permit),
        ];
        assert_eq!(
            reduce(
                &votes,
                &weights(&[("alice", u64::MAX), ("bob", u64::MAX)]),
                Caps {
                    per_member: u64::MAX,
                    per_cohort: u64::MAX,
                },
                false,
            ),
            Err(Error::Overflow)
        );
    }

    #[test]
    fn same_cohort_max_weights_clip_without_denominator_overflow() {
        let votes = [
            empirical("alice", "shared", Recommendation::Permit),
            empirical("bob", "shared", Recommendation::Permit),
        ];
        let reduction = reduce(
            &votes,
            &weights(&[("alice", u64::MAX), ("bob", u64::MAX)]),
            Caps {
                per_member: u64::MAX,
                per_cohort: u64::MAX,
            },
            false,
        )
        .unwrap();

        assert_eq!(reduction.outcome, Outcome::Empirical);
        assert_eq!(reduction.admitted_weights["alice"], u64::MAX / 2);
        assert_eq!(reduction.admitted_weights["bob"], u64::MAX / 2);
        assert_eq!(reduction.admitted_cohort_weights["shared"], u64::MAX - 1);
        assert_eq!(reduction.permit_weight, u64::MAX - 1);
    }
}
