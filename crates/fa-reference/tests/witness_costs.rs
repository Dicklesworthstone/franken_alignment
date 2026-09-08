//! Descriptive in-memory costs for bounded FA-058 witness capture and reuse.
//!
//! These fixed cases assert every expected result before printing timing data.
//! They are not OS/process-cold, allocation/RSS, benchmark, SLO, adapter
//! authentication, storage, or authority evidence. Logical byte counts include
//! only requested and successfully returned judgment value bytes: no metadata encoding
//! is invented for keys, versions, closures, or frontier state.

use std::{collections::BTreeMap, hint::black_box, time::Instant};

use fa_reference::{
    Error,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, Invalidation, QueryRole, Reuse,
        SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot,
    },
};

const SAMPLES: usize = 12;
const EXACT_KEY: u64 = 101;
const ABSENT_KEY: u64 = 303;
const UNRELATED_KEY: u64 = 707;
const EXACT_VALUE: &[u8] = b"reviewed";

#[derive(Clone)]
struct Store {
    values: BTreeMap<u64, (u64, Vec<u8>)>,
}

impl Store {
    fn snapshot(&self, revision: u64, control_cut: u64, closure: DomainClosure) -> WitnessSnapshot {
        let entries = self
            .values
            .iter()
            .map(|(&key, (version, value))| {
                SnapshotEntry::new(key, *version, value.clone())
                    .expect("fixed bounded value must remain valid")
            })
            .collect();
        WitnessSnapshot::new(revision, control_cut, 47, domain(closure), entries)
            .expect("fixed snapshot must remain well formed")
    }
}

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 23,
        branch: 29,
        projection: 31,
        source_epoch: 37,
    }
}

fn domain(closure: DomainClosure) -> AdapterDomainInput {
    AdapterDomainInput::new(DomainProjection::new(41, 43, projection()), closure)
}

fn closed_frontier() -> (ProductFrontiers, TrustedClosingMarker) {
    let mut frontiers = ProductFrontiers::new(1, 4).expect("fixed bounded frontier configuration");
    frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("fixed authenticated prefix");
    let marker = TrustedClosingMarker {
        key: projection(),
        final_sequence: 1,
        marker_generation: 5,
    };
    frontiers
        .record_close(marker)
        .expect("fixed complete authenticated closure");
    (frontiers, marker)
}

fn requests() -> Vec<WitnessRequest> {
    vec![
        WitnessRequest::ExactValue {
            key: EXACT_KEY,
            role: QueryRole::PredicateInput,
        },
        WitnessRequest::AbsentKey { key: ABSENT_KEY },
    ]
}

fn baseline_store() -> Store {
    Store {
        values: BTreeMap::from([
            (EXACT_KEY, (7, EXACT_VALUE.to_vec())),
            (UNRELATED_KEY, (1, b"unrelated".to_vec())),
        ]),
    }
}

/// Deliberately independent from WitnessJudgment reuse: direct raw-store
/// recomputation of the two requested facts, without reusing its outcome.
fn always_recompute(store: &Store) -> bool {
    store
        .values
        .get(&EXACT_KEY)
        .is_some_and(|(version, value)| *version == 7 && value == EXACT_VALUE)
        && !store.values.contains_key(&ABSENT_KEY)
}

fn requested_logical_value_bytes() -> usize {
    EXACT_VALUE.len()
}

fn captured_logical_value_bytes() -> usize {
    // The absent-key witness has a typed closing marker but no value bytes.
    EXACT_VALUE.len()
}

fn assert_cost(reuse: Reuse, exact_reads: u16, absent_reads: u16, frontier_checks: u16) {
    let cost = reuse.cost();
    assert_eq!(cost.exact_reads(), exact_reads);
    assert_eq!(cost.absent_reads(), absent_reads);
    assert_eq!(cost.frontier_checks(), frontier_checks);
}

fn run_case(
    case: &str,
    timing_scope: &str,
    expected_successes: usize,
    expected_invalidations: usize,
    expected_refusals: usize,
    successful_judgment_value_bytes: usize,
    mut sample: impl FnMut() -> (usize, usize, usize),
) {
    let mut timings = Vec::with_capacity(SAMPLES);
    let mut successes = 0;
    let mut invalidations = 0;
    let mut refusals = 0;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let (sample_successes, sample_invalidations, sample_refusals) = sample();
        let elapsed = started.elapsed().as_nanos();
        successes += sample_successes;
        invalidations += sample_invalidations;
        refusals += sample_refusals;
        timings.push(elapsed);
    }
    assert_eq!(
        successes,
        expected_successes * SAMPLES,
        "{case} dropped a success"
    );
    assert_eq!(
        invalidations,
        expected_invalidations * SAMPLES,
        "{case} dropped an invalidation"
    );
    assert_eq!(
        refusals,
        expected_refusals * SAMPLES,
        "{case} dropped a refusal"
    );
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "case={case} samples={SAMPLES} successes={successes} invalidations={invalidations} refusals={refusals} observed_errors=0 requested_logical_value_bytes={} successful_judgment_logical_value_bytes={} elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} timing_scope={timing_scope} memory_measurement=unavailable_in_memory_profile",
        requested_logical_value_bytes(),
        successful_judgment_value_bytes,
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
    );
}

#[test]
fn fixed_witness_costs_are_descriptive_and_assert_all_outcomes() {
    run_case(
        "fresh_exact_and_absent_capture_reuse",
        "fresh_snapshot_frontier_capture_reuse_assertions_drop",
        1,
        0,
        0,
        captured_logical_value_bytes(),
        || {
            let (frontiers, marker) = closed_frontier();
            let store = baseline_store();
            assert!(always_recompute(&store));
            let snapshot = store.snapshot(70, 80, DomainClosure::Closed(marker));
            let judgment = WitnessJudgment::capture(&snapshot, &frontiers, requests())
                .expect("exact and closed absence must capture");
            let reuse = judgment
                .reuse_at(&snapshot, &frontiers)
                .expect("same snapshot must be reusable");
            assert!(matches!(reuse, Reuse::StillValid { .. }));
            assert_cost(reuse, 1, 1, 1);
            black_box(judgment);
            (1, 0, 0)
        },
    );

    run_case(
        "fresh_unrelated_key_churn_remains_valid",
        "fresh_baseline_capture_unrelated_candidate_reuse_assertions_drop",
        1,
        0,
        0,
        captured_logical_value_bytes(),
        || {
            let (frontiers, marker) = closed_frontier();
            let initial_store = baseline_store();
            let initial = initial_store.snapshot(70, 80, DomainClosure::Closed(marker));
            let judgment = WitnessJudgment::capture(&initial, &frontiers, requests())
                .expect("baseline must capture");
            let mut changed_store = initial_store;
            changed_store
                .values
                .insert(UNRELATED_KEY, (2, b"changed unrelated".to_vec()));
            assert!(always_recompute(&changed_store));
            let changed = changed_store.snapshot(71, 81, DomainClosure::Closed(marker));
            let reuse = judgment
                .reuse_at(&changed, &frontiers)
                .expect("newer unrelated snapshot is comparable");
            assert!(matches!(reuse, Reuse::StillValid { .. }));
            assert_cost(reuse, 1, 1, 1);
            black_box(judgment);
            (1, 0, 0)
        },
    );

    run_case(
        "fresh_changed_exact_value_invalidates",
        "fresh_baseline_capture_control_changed_exact_candidate_reuse_assertions_drop",
        0,
        1,
        0,
        captured_logical_value_bytes(),
        || {
            let (frontiers, marker) = closed_frontier();
            let initial_store = baseline_store();
            let initial = initial_store.snapshot(70, 80, DomainClosure::Closed(marker));
            let judgment = WitnessJudgment::capture(&initial, &frontiers, requests())
                .expect("baseline must capture");
            assert!(always_recompute(&initial_store));
            assert!(matches!(
                judgment.reuse_at(&initial, &frontiers),
                Ok(Reuse::StillValid { .. })
            ));
            let mut changed_store = initial_store;
            changed_store
                .values
                .insert(EXACT_KEY, (7, b"different".to_vec()));
            assert!(!always_recompute(&changed_store));
            let changed = changed_store.snapshot(71, 81, DomainClosure::Closed(marker));
            let reuse = judgment
                .reuse_at(&changed, &frontiers)
                .expect("newer changed snapshot is comparable");
            assert!(matches!(
                reuse,
                Reuse::Invalidated {
                    reason: Invalidation::ExactValue,
                    ..
                }
            ));
            assert_cost(reuse, 1, 0, 0);
            black_box(judgment);
            (0, 1, 0)
        },
    );

    run_case(
        "fresh_absent_insertion_invalidates",
        "fresh_baseline_capture_control_absent_insertion_candidate_reuse_assertions_drop",
        0,
        1,
        0,
        captured_logical_value_bytes(),
        || {
            let (frontiers, marker) = closed_frontier();
            let initial_store = baseline_store();
            let initial = initial_store.snapshot(70, 80, DomainClosure::Closed(marker));
            let judgment = WitnessJudgment::capture(&initial, &frontiers, requests())
                .expect("baseline must capture");
            assert!(always_recompute(&initial_store));
            assert!(matches!(
                judgment.reuse_at(&initial, &frontiers),
                Ok(Reuse::StillValid { .. })
            ));
            let mut changed_store = initial_store;
            changed_store
                .values
                .insert(ABSENT_KEY, (1, b"now present".to_vec()));
            assert!(!always_recompute(&changed_store));
            let changed = changed_store.snapshot(71, 81, DomainClosure::Closed(marker));
            let reuse = judgment
                .reuse_at(&changed, &frontiers)
                .expect("newer inserted snapshot is comparable");
            assert!(matches!(
                reuse,
                Reuse::Invalidated {
                    reason: Invalidation::AbsentKey,
                    ..
                }
            ));
            assert_cost(reuse, 1, 1, 1);
            black_box(judgment);
            (0, 1, 0)
        },
    );

    run_case(
        "fresh_unknown_closure_refusal",
        "fresh_snapshot_capture_refusal_assertions_drop",
        0,
        0,
        1,
        0,
        || {
            let open_frontiers =
                ProductFrontiers::new(1, 4).expect("fixed bounded frontier configuration");
            let store = baseline_store();
            assert!(always_recompute(&store));
            let unknown = store.snapshot(70, 80, DomainClosure::Unknown);
            assert_eq!(
                WitnessJudgment::capture(&unknown, &open_frontiers, requests()),
                Err(Error::Incomplete),
            );
            (0, 0, 1)
        },
    );

    run_case(
        "fresh_stale_snapshot_refusal",
        "fresh_baseline_capture_stale_candidate_reuse_assertions_drop",
        0,
        0,
        1,
        captured_logical_value_bytes(),
        || {
            let (frontiers, marker) = closed_frontier();
            let store = baseline_store();
            assert!(always_recompute(&store));
            let initial = store.snapshot(70, 80, DomainClosure::Closed(marker));
            let judgment = WitnessJudgment::capture(&initial, &frontiers, requests())
                .expect("baseline must capture");
            let stale = store.snapshot(69, 80, DomainClosure::Closed(marker));
            assert_eq!(judgment.reuse_at(&stale, &frontiers), Err(Error::Stale));
            black_box(judgment);
            (0, 0, 1)
        },
    );
}

/// Select this exact test alone in a fresh release process for a one-sample
/// first-capture observation. There is no capture before the timed call; a
/// normal suite invocation is not OS/page-cache/CPU-cold evidence.
#[test]
fn first_witness_capture_is_selectable_for_fresh_release_measurement() {
    let (frontiers, marker) = closed_frontier();
    let store = baseline_store();
    let snapshot = store.snapshot(70, 80, DomainClosure::Closed(marker));
    let requested = requests();
    let started = Instant::now();
    let captured = WitnessJudgment::capture(&snapshot, &frontiers, requested);
    let elapsed = started.elapsed().as_nanos();
    let judgment = captured.expect("fixed exact-and-absent capture must succeed");
    assert_eq!(
        judgment.exact_version_and_role(EXACT_KEY),
        Some((7, QueryRole::PredicateInput))
    );
    println!(
        "case=witness_first_capture samples=1 successes=1 invalidations=0 refusals=0 observed_errors=0 requested_logical_value_bytes={} successful_judgment_logical_value_bytes={} elapsed_ns={elapsed} timing_scope=fresh_process_exact_capture_setup_excluded_only_when_sole_selected_test memory_measurement=unavailable_in_memory_profile",
        requested_logical_value_bytes(),
        captured_logical_value_bytes(),
    );
}
