//! Descriptive in-memory costs for bounded FA-004/FA-005 reference profiles.
//!
//! These are not OS/process cold-start, SLO, allocation/RSS, or statistical
//! distribution measurements. Fixtures are compile-time bytes; memory remains
//! unavailable under this in-memory test profile.

use std::{collections::BTreeMap, hint::black_box, time::Instant};

use fa_reference::{
    Error, Snapshot,
    action::{
        ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope,
        TrustedOutcome, VERSION,
    },
    canonical_json::{CanonicalError, decode_canonical, encode, validate_json},
    history::{
        HistoryConfig, HistoryEvent, MAX_HISTORY_EVENTS, ReplayOutcome, SeededHistory, replay,
    },
    strict_json::{Json, Limits, parse},
};

const SAMPLES: usize = 12;
const ACTION: &[u8] = include_bytes!("fixtures/canonical-json-v01/action.json");
const CAPABILITY: &[u8] = include_bytes!("fixtures/canonical-json-v01/capability.json");
const EVIDENCE: &[u8] = include_bytes!("fixtures/canonical-json-v01/evidence.json");

fn scope() -> Scope {
    Scope {
        tenant: 11,
        principal: 12,
        run: 13,
        branch: 14,
        authority: 15,
        purpose: Purpose::Effect,
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 7,
        complete: true,
        values: BTreeMap::new(),
    }
}
fn action(id: u64, units: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(ResolvedTarget {
            adapter: 21,
            object: id,
            contract_version: 23,
            expected_version: 24,
            generation: 25,
        }),
        payload: vec![id as u8],
        required_witnesses: vec![],
        policy_epoch: 0,
        deadline: ElapsedTick(10),
        units,
    })
    .unwrap()
}
fn config(seed: u64, attempts: usize) -> HistoryConfig {
    HistoryConfig {
        scope: scope(),
        total: 10,
        max_attempts: attempts,
        seed,
    }
}
fn successful_lane(attempt: u64, action: FrozenAction) -> Vec<HistoryEvent> {
    let snapshot = snapshot();
    vec![
        HistoryEvent::ObserveTime(ElapsedTick(1)),
        HistoryEvent::Propose {
            attempt,
            action: action.clone(),
        },
        HistoryEvent::Prepare { attempt },
        HistoryEvent::BeginReview { attempt },
        HistoryEvent::Authorize {
            attempt,
            snapshot: snapshot.clone(),
        },
        HistoryEvent::Dispatch {
            attempt,
            action,
            snapshot,
        },
        HistoryEvent::RecordOutcome {
            attempt,
            outcome: TrustedOutcome::Executed,
        },
    ]
}

fn run_case(
    case: &str,
    timing_scope: &str,
    logical_input_bytes: Option<usize>,
    expected_successes: usize,
    expected_refusals: usize,
    mut sample: impl FnMut() -> (usize, usize),
) {
    // An unexpected result panics at its assertion: it must not be converted
    // into a timing record. The raw test failure is retained by the gate.
    let mut timings = Vec::with_capacity(SAMPLES);
    let mut successes = 0;
    let mut refusals = 0;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let (sample_successes, sample_refusals) = sample();
        timings.push(started.elapsed().as_nanos());
        successes += sample_successes;
        refusals += sample_refusals;
    }
    assert_eq!(
        successes,
        expected_successes * SAMPLES,
        "{case} dropped a success"
    );
    assert_eq!(
        refusals,
        expected_refusals * SAMPLES,
        "{case} dropped a refusal"
    );
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "case={case} samples={SAMPLES} successes={successes} refusals={refusals} observed_errors=0 elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} logical_input_bytes={} timing_scope={timing_scope} memory_measurement=unavailable_in_memory_profile",
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
        logical_input_bytes.map_or_else(|| "not_applicable".to_owned(), |bytes| bytes.to_string())
    );
}

fn roundtrip(bytes: &[u8]) {
    let document = decode_canonical(bytes).expect("manual fixture must decode canonically");
    assert_eq!(
        encode(&document),
        bytes,
        "decode/encode must retain manual bytes"
    );
    black_box(document);
}

fn large_unicode_claim() -> Vec<u8> {
    let mut value = parse(EVIDENCE, Limits::default()).expect("manual claim syntax");
    let Json::Object(fields) = &mut value else {
        panic!("claim root must be an object")
    };
    let mut assertion = "😀".repeat(2_047);
    assertion.push('\u{0001}');
    fields.insert("assertion".to_owned(), Json::String(assertion));
    let document = validate_json(&value).expect("large Unicode claim remains schema-valid");
    encode(&document)
}

#[test]
fn fixed_reference_profile_costs_are_descriptive_and_assert_all_outcomes() {
    run_case(
        "fresh_two_lane_lifecycle",
        "construction_replay_assertions_drop",
        None,
        1,
        0,
        || {
            let history = SeededHistory::new(
                config(0x5eed, 2),
                vec![
                    successful_lane(1, action(101, 3)),
                    successful_lane(2, action(102, 2)),
                ],
            )
            .unwrap();
            let ReplayOutcome::Completed {
                chosen_prefix,
                inspection,
                ..
            } = replay(&history).unwrap()
            else {
                panic!("two valid lanes must complete")
            };
            assert_eq!(chosen_prefix.len(), 14);
            assert_eq!(
                (
                    inspection.available,
                    inspection.reserved,
                    inspection.charged
                ),
                (5, 0, 5)
            );
            assert_eq!(inspection.stages.get(&1), Some(&ActionState::Confirmed));
            assert_eq!(inspection.stages.get(&2), Some(&ActionState::Confirmed));
            black_box(inspection);
            (1, 0)
        },
    );

    run_case(
        "fresh_unknown_cancel_refusal",
        "construction_replay_assertions_drop",
        None,
        0,
        1,
        || {
            let mut lane = successful_lane(1, action(104, 3));
            lane.pop();
            lane.extend([
                HistoryEvent::MarkUnknown { attempt: 1 },
                HistoryEvent::Cancel { attempt: 1 },
            ]);
            let history = SeededHistory::new(config(11, 1), vec![lane]).unwrap();
            let ReplayOutcome::Counterexample {
                chosen_prefix,
                refusal,
                inspection,
                ..
            } = replay(&history).unwrap()
            else {
                panic!("unknown cancellation must refuse")
            };
            assert_eq!(refusal, Error::WrongState);
            assert_eq!(chosen_prefix.len(), 8);
            assert_eq!(
                inspection.charged, 3,
                "unknown effect remains charged liability"
            );
            assert_eq!(inspection.stages.get(&1), Some(&ActionState::Unknown));
            black_box(inspection);
            (0, 1)
        },
    );

    run_case(
        "exact_128_event_observed_clock_history",
        "construction_replay_assertions_drop",
        None,
        1,
        0,
        || {
            let lane = (1..=MAX_HISTORY_EVENTS as u64)
                .map(|tick| HistoryEvent::ObserveTime(ElapsedTick(tick)))
                .collect();
            let history = SeededHistory::new(config(17, 1), vec![lane]).unwrap();
            let ReplayOutcome::Completed {
                chosen_prefix,
                inspection,
                ..
            } = replay(&history).unwrap()
            else {
                panic!("exact bound history must complete")
            };
            assert_eq!(chosen_prefix.len(), MAX_HISTORY_EVENTS);
            assert_eq!(
                inspection.elapsed,
                Some(ElapsedTick(MAX_HISTORY_EVENTS as u64))
            );
            black_box(inspection);
            (1, 0)
        },
    );

    for (case, fixture) in [
        ("canonical_action_roundtrip", ACTION),
        ("canonical_capability_roundtrip", CAPABILITY),
        ("canonical_evidence_roundtrip", EVIDENCE),
    ] {
        run_case(
            case,
            "setup_excluded_decode_encode_assertions_drop",
            Some(fixture.len()),
            1,
            0,
            || {
                roundtrip(fixture);
                (1, 0)
            },
        );
    }
    run_case(
        "noncanonical_whitespace_refusal",
        "construction_control_roundtrip_negative_decode_assertions_drop",
        Some(2 * ACTION.len() + 1),
        1,
        1,
        || {
            roundtrip(ACTION);
            let mut noncanonical = ACTION.to_vec();
            noncanonical.insert(1, b' ');
            assert_eq!(
                decode_canonical(&noncanonical),
                Err(CanonicalError::NonCanonical)
            );
            black_box(noncanonical);
            (1, 1)
        },
    );
    let large_unicode_bytes = large_unicode_claim();
    let large_unicode_len = large_unicode_bytes.len();
    run_case(
        "large_valid_unicode_claim_roundtrip",
        "construction_json_validation_encode_decode_assertions_drop",
        Some(large_unicode_len),
        1,
        0,
        || {
            let bytes = large_unicode_claim();
            assert_eq!(
                bytes.len(),
                large_unicode_len,
                "fixed Unicode construction drifted"
            );
            roundtrip(&bytes);
            black_box(bytes);
            (1, 0)
        },
    );
}
