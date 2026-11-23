//! Public regressions for the bounded risk-theater score/support kernel.
//!
//! These inputs are independently supplied reference history, not authenticated
//! native journal closure or the six production ledger query definitions. Tests
//! establish exact arithmetic, retractions, bounds and differential behavior;
//! they do not establish governance authority, live incidents or actor filtering.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::{
    ControlPlaneDelta, ControlPlaneFrontier, MAX_RISK_THEATER_COMPONENT_BYTES,
    MAX_RISK_THEATER_FRONTIER_DELTAS, MAX_RISK_THEATER_HISTORY_DELTAS,
    MAX_RISK_THEATER_HISTORY_FRONTIERS, MAX_RISK_THEATER_SUPPORTS, RiskTheaterDetector,
    RiskTheaterError, RiskTheaterFamily, RiskTheaterSnapshot, recompute_risk_theater_batch_oracle,
};

fn thresholds() -> BTreeMap<RiskTheaterFamily, i64> {
    RiskTheaterFamily::ALL
        .into_iter()
        .map(|family| (family, 10))
        .collect()
}

fn new_detector() -> RiskTheaterDetector {
    RiskTheaterDetector::new(thresholds()).expect("complete positive thresholds")
}

fn delta(
    seq: u64,
    family: RiskTheaterFamily,
    key: &str,
    multiplicity: i64,
    value: i64,
) -> ControlPlaneDelta {
    ControlPlaneDelta {
        frontier_seq: seq,
        family,
        key: key.to_owned(),
        multiplicity,
        metric_delta: value,
        support_address: Address::new("org", Noun::Ledger, key, Some(1)).expect("address"),
    }
}

fn frontier(seq: u64, deltas: Vec<ControlPlaneDelta>) -> ControlPlaneFrontier {
    ControlPlaneFrontier {
        frontier_seq: seq,
        closed: true,
        deltas,
    }
}

fn one(seq: u64, key: &str, weight: i64, value: i64) -> ControlPlaneFrontier {
    frontier(
        seq,
        vec![delta(
            seq,
            RiskTheaterFamily::ConsequenceDrift,
            key,
            weight,
            value,
        )],
    )
}

fn assert_rejected_atomically(
    detector: &mut RiskTheaterDetector,
    previous: &RiskTheaterSnapshot,
    input: &ControlPlaneFrontier,
    error: RiskTheaterError,
) {
    assert_eq!(detector.apply_frontier(input), Err(error));
    assert_eq!(detector.closed_frontier(), Some(previous.frontier_seq));
    assert_eq!(
        detector.snapshot(previous.frontier_seq),
        Ok(previous.clone())
    );
    if input.frontier_seq != previous.frontier_seq {
        assert!(detector.snapshot(input.frontier_seq).is_err());
    }
}

#[test]
fn every_family_requires_an_explicit_positive_threshold() {
    for family in RiskTheaterFamily::ALL {
        let mut missing = thresholds();
        missing.remove(&family);
        assert!(matches!(
            RiskTheaterDetector::new(missing),
            Err(RiskTheaterError::IncompleteThresholds)
        ));
        for invalid in [0, -1, i64::MIN] {
            let mut invalid_thresholds = thresholds();
            invalid_thresholds.insert(family, invalid);
            assert!(matches!(
                RiskTheaterDetector::new(invalid_thresholds.clone()),
                Err(RiskTheaterError::NonpositiveThreshold)
            ));
            assert_eq!(
                recompute_risk_theater_batch_oracle(&invalid_thresholds, &[frontier(1, vec![])], 1,),
                Err(RiskTheaterError::NonpositiveThreshold)
            );
        }
    }
}

#[test]
fn all_six_families_match_independent_rebuilds_at_every_closed_frontier() {
    let mut detector = new_detector();
    let mut history = Vec::new();
    for season in 0..8 {
        for phase in 0..6 {
            let seq = history.len() as u64 + 1;
            let mut changes = Vec::new();
            for family in RiskTheaterFamily::ALL {
                let first = format!("season-{season}-first");
                let second = format!("season-{season}-second");
                let changes_for_family = match phase {
                    0 => vec![delta(seq, family, &first, 2, 4)],
                    1 => vec![delta(seq, family, &second, 1, 5)],
                    2 => vec![delta(seq, family, &first, -1, 4)],
                    3 => vec![
                        delta(seq, family, &second, -1, 5),
                        delta(seq, family, &second, 1, 12),
                    ],
                    4 => vec![
                        delta(seq, family, &first, -1, 4),
                        delta(seq, family, &second, -1, 12),
                    ],
                    _ => vec![],
                };
                changes.extend(changes_for_family);
            }
            history.push(frontier(seq, changes));
            let snapshot = detector
                .apply_frontier(history.last().expect("current frontier"))
                .expect("valid season");
            let oracle = recompute_risk_theater_batch_oracle(&thresholds(), &history, seq)
                .expect("independent batch evaluation");
            assert_eq!(snapshot, oracle, "season {season}, phase {phase}");
            assert_eq!(
                detector.verify_against_history(&history),
                Ok(snapshot.clone())
            );
            let expected_score = [8, 13, 9, 16, 0, 0][phase];
            let supports_per_family = [1, 2, 2, 2, 0, 0][phase];
            assert_eq!(snapshot.supports.len(), supports_per_family * 6);
            assert_eq!(
                snapshot.findings.len(),
                if expected_score >= 10 { 6 } else { 0 }
            );
            for family in RiskTheaterFamily::ALL {
                assert_eq!(snapshot.scores[&family], expected_score);
            }
            for finding in snapshot.findings {
                assert_eq!(finding.current_score, expected_score);
                assert_eq!(finding.threshold, 10);
                assert_eq!(finding.support_addresses.len(), supports_per_family);
            }
        }
    }
    assert!(
        detector
            .snapshot(48)
            .expect("last empty boundary")
            .supports
            .is_empty()
    );
}

#[test]
fn exact_retraction_preserves_other_supports_and_canonical_replacement() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut detector = new_detector();
    let first = frontier(
        1,
        vec![
            delta(1, family, "first", 3, 4),
            delta(1, family, "second", 1, 10),
        ],
    );
    let snapshot = detector.apply_frontier(&first).expect("two supports");
    assert_eq!(snapshot.findings[0].current_score, 22);
    let second = frontier(2, vec![delta(2, family, "first", -3, 4)]);
    let snapshot = detector.apply_frontier(&second).expect("retract first");
    assert_eq!(snapshot.findings[0].current_score, 10);
    assert_eq!(
        snapshot.findings[0].support_addresses,
        vec![first.deltas[1].support_address.clone()]
    );
    let mut replacement = delta(3, family, "second", 1, 12);
    replacement.support_address.generation = Some(2);
    let third = frontier(
        3,
        vec![delta(3, family, "second", -1, 10), replacement.clone()],
    );
    let snapshot = detector.apply_frontier(&third).expect("exact old then new");
    assert_eq!(snapshot.findings[0].current_score, 12);
    assert_eq!(snapshot.supports[0].support_address.generation, Some(2));
    replacement.frontier_seq = 4;
    replacement.multiplicity = -1;
    let fourth = frontier(4, vec![replacement]);
    let snapshot = detector
        .apply_frontier(&fourth)
        .expect("last support removed");
    assert!(snapshot.findings.is_empty());
    assert!(snapshot.supports.is_empty());
    assert_eq!(
        detector.verify_against_history(&[first, second, third, fourth]),
        Ok(snapshot)
    );
}

#[test]
fn full_tuple_identity_is_required_for_insertions_and_retractions() {
    let mut detector = new_detector();
    let previous = detector
        .apply_frontier(&one(1, "original", 1, 12))
        .expect("seed");
    for weight in [-1, 1] {
        let mut wrong_metric = one(2, "original", weight, 5);
        assert_rejected_atomically(
            &mut detector,
            &previous,
            &wrong_metric,
            RiskTheaterError::TupleMismatch,
        );
        wrong_metric.deltas[0].metric_delta = 12;
        for variant in 0..4 {
            let mut wrong_address = wrong_metric.clone();
            let address = &mut wrong_address.deltas[0].support_address;
            match variant {
                0 => address.tenant = "other-tenant".to_owned(),
                1 => address.id = "other-evidence".to_owned(),
                2 => address.generation = Some(2),
                _ => address.kind = Noun::Finding,
            }
            assert_rejected_atomically(
                &mut detector,
                &previous,
                &wrong_address,
                RiskTheaterError::TupleMismatch,
            );
        }
    }
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &one(2, "different-key", -1, 12),
        RiskTheaterError::MissingRetractionSupport,
    );
    let corrected = detector
        .apply_frontier(&one(2, "original", -1, 12))
        .expect("retry exact tuple");
    assert!(corrected.findings.is_empty());
    assert_eq!(corrected.scores[&RiskTheaterFamily::ConsequenceDrift], 0);
}

#[test]
fn zero_negative_and_excessive_weights_are_atomic_refusals() {
    let mut detector = new_detector();
    let previous = detector
        .apply_frontier(&one(1, "key", 1, 12))
        .expect("seed");
    for (input, error) in [
        (one(2, "key", 0, 12), RiskTheaterError::ZeroMultiplicity),
        (
            one(2, "key", -2, 12),
            RiskTheaterError::NegativeMultiplicity,
        ),
        (
            one(2, "key", i64::MIN, 12),
            RiskTheaterError::NegativeMultiplicity,
        ),
        (
            one(2, "missing", -1, 12),
            RiskTheaterError::MissingRetractionSupport,
        ),
        (
            one(2, "negative-metric", 1, -1),
            RiskTheaterError::NegativeMetric,
        ),
    ] {
        assert_rejected_atomically(&mut detector, &previous, &input, error);
    }
    let mut overflowing = new_detector();
    let previous = overflowing
        .apply_frontier(&one(1, "zero-metric", i64::MAX, 0))
        .expect("large exact weight");
    assert_rejected_atomically(
        &mut overflowing,
        &previous,
        &one(2, "zero-metric", 1, 0),
        RiskTheaterError::MultiplicityOverflow,
    );
}

#[test]
fn score_overflow_is_checked_for_products_and_family_sums() {
    for bad in [one(2, "product", 2, i64::MAX), one(2, "extra", 1, 1)] {
        let mut detector = new_detector();
        let seed = one(1, "large", 1, i64::MAX);
        let previous = detector.apply_frontier(&seed).expect("maximum exact score");
        assert_rejected_atomically(
            &mut detector,
            &previous,
            &bad,
            RiskTheaterError::ScoreOverflow,
        );
        assert_eq!(
            recompute_risk_theater_batch_oracle(&thresholds(), &[seed, bad], 2),
            Err(RiskTheaterError::ScoreOverflow)
        );
    }
    // A temporary family sum above i64 is harmless when the COMPLETE boundary
    // replaces one supported value with another and its final score fits.
    let mut detector = new_detector();
    let first = one(1, "old", 1, i64::MAX);
    detector.apply_frontier(&first).expect("maximum score");
    let second = frontier(
        2,
        vec![
            delta(2, RiskTheaterFamily::ConsequenceDrift, "new", 1, i64::MAX),
            delta(2, RiskTheaterFamily::ConsequenceDrift, "old", -1, i64::MAX),
        ],
    );
    let snapshot = detector
        .apply_frontier(&second)
        .expect("atomic full-boundary replacement");
    assert_eq!(
        snapshot.scores[&RiskTheaterFamily::ConsequenceDrift],
        i64::MAX
    );
    assert_eq!(snapshot.supports.len(), 1);
    assert_eq!(
        detector.verify_against_history(&[first, second]),
        Ok(snapshot)
    );
}

#[test]
fn later_failure_does_not_partially_retract_an_earlier_tuple_in_the_batch() {
    let mut detector = new_detector();
    let previous = detector
        .apply_frontier(&one(1, "kept", 1, 12))
        .expect("seed");
    let bad = frontier(
        2,
        vec![
            delta(2, RiskTheaterFamily::ConsequenceDrift, "kept", -1, 12),
            delta(2, RiskTheaterFamily::ConsequenceDrift, "missing", -1, 12),
        ],
    );
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &bad,
        RiskTheaterError::MissingRetractionSupport,
    );
    assert_eq!(
        detector
            .evaluate_findings(1)
            .expect("previous complete view")
            .len(),
        1
    );
}

#[test]
fn incomplete_gapped_replayed_and_mislabeled_frontiers_never_yield_a_clean_current_view() {
    let mut detector = new_detector();
    assert_eq!(detector.closed_frontier(), None);
    assert_eq!(
        detector.evaluate_findings(0),
        Err(RiskTheaterError::NoClosedFrontier)
    );
    let mut unclosed = frontier(1, vec![]);
    unclosed.closed = false;
    assert_eq!(
        detector.apply_frontier(&unclosed),
        Err(RiskTheaterError::UnclosedFrontier)
    );
    assert_eq!(
        detector.evaluate_findings(1),
        Err(RiskTheaterError::NoClosedFrontier)
    );
    let previous = detector
        .apply_frontier(&frontier(1, vec![]))
        .expect("explicit empty closure");
    assert!(previous.findings.is_empty());
    for seq in [0, 1, 3, u64::MAX] {
        assert_rejected_atomically(
            &mut detector,
            &previous,
            &frontier(seq, vec![]),
            RiskTheaterError::UnexpectedFrontier {
                expected: 2,
                actual: seq,
            },
        );
    }
    let bad_label = frontier(2, one(3, "future", 1, 12).deltas);
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &bad_label,
        RiskTheaterError::DeltaFrontierMismatch,
    );
    unclosed.frontier_seq = 2;
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &unclosed,
        RiskTheaterError::UnclosedFrontier,
    );
    detector
        .apply_frontier(&one(2, "accepted", 1, 12))
        .expect("correct next frontier");
    assert_eq!(detector.evaluate_findings(2).expect("complete").len(), 1);
}

#[test]
fn canonical_address_and_component_bounds_are_enforced_before_retention() {
    let mut detector = new_detector();
    let previous = detector
        .apply_frontier(&frontier(1, vec![]))
        .expect("empty closure");
    for key in [
        String::new(),
        "x".repeat(MAX_RISK_THEATER_COMPONENT_BYTES + 1),
    ] {
        let mut bad = one(2, "valid", 1, 12);
        bad.deltas[0].key = key;
        assert_rejected_atomically(&mut detector, &previous, &bad, RiskTheaterError::InvalidKey);
    }
    for value in [
        String::new(),
        "bad/segment".to_owned(),
        "x".repeat(MAX_RISK_THEATER_COMPONENT_BYTES + 1),
    ] {
        for tenant in [false, true] {
            let mut bad = one(2, "valid", 1, 12);
            if tenant {
                bad.deltas[0].support_address.tenant = value.clone();
            } else {
                bad.deltas[0].support_address.id = value.clone();
            }
            assert_rejected_atomically(
                &mut detector,
                &previous,
                &bad,
                RiskTheaterError::InvalidSupportAddress,
            );
        }
    }
    let boundary = "x".repeat(MAX_RISK_THEATER_COMPONENT_BYTES);
    let accepted = detector
        .apply_frontier(&one(2, &boundary, 1, 12))
        .expect("exact component bound");
    assert_eq!(
        accepted.supports[0].key.len(),
        MAX_RISK_THEATER_COMPONENT_BYTES
    );
}

#[test]
fn delta_and_live_support_limits_refuse_without_truncating_findings() {
    let mut detector = new_detector();
    let mut history = Vec::new();
    for block in 0..(MAX_RISK_THEATER_SUPPORTS / MAX_RISK_THEATER_FRONTIER_DELTAS) {
        let seq = block as u64 + 1;
        let changes = (0..MAX_RISK_THEATER_FRONTIER_DELTAS)
            .map(|index| {
                delta(
                    seq,
                    RiskTheaterFamily::ConsequenceDrift,
                    &format!(
                        "support-{}",
                        block * MAX_RISK_THEATER_FRONTIER_DELTAS + index
                    ),
                    1,
                    1,
                )
            })
            .collect();
        history.push(frontier(seq, changes));
        detector
            .apply_frontier(history.last().expect("boundary"))
            .expect("at limit");
    }
    let previous = detector
        .snapshot(history.len() as u64)
        .expect("full relation");
    assert_eq!(previous.supports.len(), MAX_RISK_THEATER_SUPPORTS);
    assert_eq!(
        previous.findings[0].support_addresses.len(),
        MAX_RISK_THEATER_SUPPORTS
    );
    let next = previous.frontier_seq + 1;
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &one(next, "one-too-many", 1, 1),
        RiskTheaterError::SupportLimit,
    );
    let oversized = frontier(
        next,
        vec![
            delta(next, RiskTheaterFamily::ConsequenceDrift, "repeated", 1, 1);
            MAX_RISK_THEATER_FRONTIER_DELTAS + 1
        ],
    );
    assert_rejected_atomically(
        &mut detector,
        &previous,
        &oversized,
        RiskTheaterError::FrontierDeltaLimit,
    );
    // Capacity applies to the closed relation, not tuple order inside an atomic
    // boundary. Adding before deleting still returns the complete set.
    let replacement = frontier(
        next,
        vec![
            delta(
                next,
                RiskTheaterFamily::ConsequenceDrift,
                "replacement",
                1,
                1,
            ),
            delta(
                next,
                RiskTheaterFamily::ConsequenceDrift,
                "support-0",
                -1,
                1,
            ),
        ],
    );
    let snapshot = detector
        .apply_frontier(&replacement)
        .expect("same-size replacement");
    history.push(replacement);
    assert_eq!(snapshot.supports.len(), MAX_RISK_THEATER_SUPPORTS);
    assert_eq!(detector.verify_against_history(&history), Ok(snapshot));
}

#[test]
fn finding_addresses_are_a_set_but_exact_weighted_supports_are_not_discarded() {
    let mut detector = new_detector();
    let mut same_address = delta(1, RiskTheaterFamily::ConsequenceDrift, "other-key", 1, 6);
    same_address.support_address = one(1, "shared", 1, 6).deltas[0].support_address.clone();
    let input = frontier(
        1,
        vec![
            delta(1, RiskTheaterFamily::ConsequenceDrift, "shared", 1, 6),
            same_address,
        ],
    );
    let snapshot = detector
        .apply_frontier(&input)
        .expect("two tuples, one address");
    assert_eq!(snapshot.findings[0].current_score, 12);
    assert_eq!(snapshot.findings[0].support_addresses.len(), 1);
    assert_eq!(snapshot.supports.len(), 2);
    assert_eq!(detector.verify_against_history(&[input]), Ok(snapshot));
}

#[test]
fn lost_retraction_is_detected_even_when_no_finding_crosses_a_threshold() {
    for metric in [0, 12] {
        let first = one(1, "retired", 1, metric);
        let retraction = one(2, "retired", -1, metric);
        let mut faulty_projection = new_detector();
        faulty_projection.apply_frontier(&first).expect("insert");
        let stale = faulty_projection
            .apply_frontier(&frontier(2, vec![]))
            .expect("simulated lost retraction");
        let history = [first, retraction];
        let oracle = recompute_risk_theater_batch_oracle(&thresholds(), &history, 2)
            .expect("independent history");
        assert!(oracle.findings.is_empty());
        assert!(oracle.supports.is_empty());
        assert_eq!(stale.supports.len(), 1);
        if metric == 0 {
            assert!(stale.findings.is_empty());
        }
        assert_eq!(
            faulty_projection.verify_against_history(&history),
            Err(RiskTheaterError::OracleMismatch)
        );
        assert_eq!(faulty_projection.snapshot(2), Ok(stale));
    }
}

#[test]
fn oracle_requires_the_complete_prefix_and_rejects_bad_earlier_boundaries() {
    let valid = one(1, "kept", 1, 12);
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &[], 1),
        Err(RiskTheaterError::MissingHistoryFrontier)
    );
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &[], 0),
        Err(RiskTheaterError::NoClosedFrontier)
    );
    let mut unclosed = frontier(2, vec![]);
    unclosed.closed = false;
    assert!(
        recompute_risk_theater_batch_oracle(&thresholds(), &[valid.clone(), unclosed.clone()], 1)
            .is_ok()
    );
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &[valid.clone(), unclosed], 2),
        Err(RiskTheaterError::UnclosedFrontier)
    );
    assert_eq!(
        recompute_risk_theater_batch_oracle(
            &thresholds(),
            &[valid.clone(), frontier(3, vec![])],
            2
        ),
        Err(RiskTheaterError::UnexpectedFrontier {
            expected: 2,
            actual: 3
        })
    );
    assert_eq!(
        recompute_risk_theater_batch_oracle(
            &thresholds(),
            &[valid.clone(), one(2, "kept", -1, 5)],
            2
        ),
        Err(RiskTheaterError::TupleMismatch)
    );
    // A subsequent deletion cannot conceal an overflowing earlier closure.
    let excessive = frontier(
        1,
        vec![
            delta(1, RiskTheaterFamily::ConsequenceDrift, "first", 1, i64::MAX),
            delta(1, RiskTheaterFamily::ConsequenceDrift, "second", 1, 1),
        ],
    );
    let later = one(2, "second", -1, 1);
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &[excessive, later], 2),
        Err(RiskTheaterError::ScoreOverflow)
    );
}

#[test]
fn oracle_reconstruction_has_explicit_history_work_limits() {
    let mut history = Vec::new();
    for seq in 1..=(MAX_RISK_THEATER_HISTORY_DELTAS / MAX_RISK_THEATER_FRONTIER_DELTAS) {
        history.push(frontier(
            seq as u64,
            vec![
                delta(
                    seq as u64,
                    RiskTheaterFamily::ConsequenceDrift,
                    "zero",
                    1,
                    0
                );
                MAX_RISK_THEATER_FRONTIER_DELTAS
            ],
        ));
    }
    let at_limit =
        recompute_risk_theater_batch_oracle(&thresholds(), &history, history.len() as u64)
            .expect("exact input work limit");
    assert_eq!(
        at_limit.supports[0].multiplicity,
        MAX_RISK_THEATER_HISTORY_DELTAS as i64
    );
    let seq = history.len() as u64 + 1;
    history.push(one(seq, "zero", 1, 0));
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &history, seq),
        Err(RiskTheaterError::HistoryLimit)
    );
    let mut empty_history = (1..=MAX_RISK_THEATER_HISTORY_FRONTIERS)
        .map(|seq| frontier(seq as u64, vec![]))
        .collect::<Vec<_>>();
    assert!(
        recompute_risk_theater_batch_oracle(
            &thresholds(),
            &empty_history,
            MAX_RISK_THEATER_HISTORY_FRONTIERS as u64
        )
        .is_ok()
    );
    let seq = MAX_RISK_THEATER_HISTORY_FRONTIERS as u64 + 1;
    empty_history.push(frontier(seq, vec![]));
    assert_eq!(
        recompute_risk_theater_batch_oracle(&thresholds(), &empty_history, seq),
        Err(RiskTheaterError::HistoryLimit)
    );
}
