//! Fixed descriptive costs for FA-059 exact range witness capture and reuse.
//!
//! The separately selectable first-capture test below performs its first range
//! validator operation at the timed capture call. Timed intervals cover only
//! `WitnessJudgment::capture`, `WitnessJudgment::reuse_at`, or the independent
//! direct `BTreeMap` recompute; caller-side request, store, snapshot, and
//! frontier construction is deliberately excluded. These samples are
//! descriptive only, not a benchmark, allocation measurement, or latency/SLO
//! claim.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use fa_reference::{
    Error,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, Invalidation,
        MAX_RANGE_WITNESS_ENTRIES, MAX_RANGE_WITNESS_VALUE_BYTES, MAX_VALUE_BYTES, Reuse,
        SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot,
    },
};

const SAMPLES_PER_CASE: usize = 6;
type Store = BTreeMap<u64, (u64, Vec<u8>)>;

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 41,
        branch: 43,
        projection: 47,
        source_epoch: 53,
    }
}

fn domain(closure: DomainClosure) -> AdapterDomainInput {
    AdapterDomainInput::new(DomainProjection::new(59, 61, projection()), closure)
}

fn snapshot(
    revision: u64,
    control_cut: u64,
    closure: DomainClosure,
    store: &Store,
) -> WitnessSnapshot {
    WitnessSnapshot::new(
        revision,
        control_cut,
        67,
        domain(closure),
        store
            .iter()
            .map(|(&key, (version, value))| SnapshotEntry::new(key, *version, value.clone()))
            .collect::<Result<Vec<_>, _>>()
            .expect("bounded fixed fixture"),
    )
    .expect("well-formed caller-supplied snapshot")
}

fn closed_frontier() -> (ProductFrontiers, TrustedClosingMarker) {
    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("authenticated terminal input");
    let marker = TrustedClosingMarker {
        key: projection(),
        final_sequence: 1,
        marker_generation: 1,
    };
    frontiers
        .record_close(marker)
        .expect("recorded terminal marker");
    (frontiers, marker)
}

fn duration_ns(samples: &[Duration]) -> u128 {
    samples.iter().map(Duration::as_nanos).sum()
}

fn range_request() -> Vec<WitnessRequest> {
    vec![WitnessRequest::RangeMembers { start: 0, end: 4 }]
}

fn timed_capture(
    snapshot: &WitnessSnapshot,
    frontiers: &ProductFrontiers,
    request: Vec<WitnessRequest>,
) -> (Result<WitnessJudgment, Error>, Duration) {
    let started = Instant::now();
    let result = WitnessJudgment::capture(snapshot, frontiers, request);
    let elapsed = started.elapsed();
    (result, elapsed)
}

fn timed_reuse(
    judgment: &WitnessJudgment,
    snapshot: &WitnessSnapshot,
    frontiers: &ProductFrontiers,
) -> (Reuse, Duration) {
    let started = Instant::now();
    let result = judgment.reuse_at(snapshot, frontiers);
    let elapsed = started.elapsed();
    (result.expect("comparable fixed snapshot"), elapsed)
}

/// Deliberately independent from witness reuse: exact direct-scan baseline.
fn direct_recompute(store: &Store, start: u64, end: u64) -> Vec<(u64, u64, Vec<u8>)> {
    store
        .range(start..end)
        .map(|(&key, (version, value))| (key, *version, value.clone()))
        .collect()
}

#[test]
fn sole_selectable_first_range_members_capture_emits_one_elapsed_ns() {
    let (frontiers, marker) = closed_frontier();
    let store = Store::from([(0, (1, b"one".to_vec())), (2, (3, b"two".to_vec()))]);
    let initial = snapshot(10, 20, DomainClosure::Closed(marker), &store);
    let request = range_request();

    // When selected by this exact test name, no earlier range operation runs in
    // this target. The request vector was prepared before the timer started.
    let (result, elapsed) = timed_capture(&initial, &frontiers, request);
    let judgment = result.expect("first closed exact range capture");
    assert_eq!(judgment.range_member_count(), 2);
    assert_eq!(judgment.range_member_bytes(), 6);
    eprintln!(
        "FA059 first RangeMembers capture: sample_count=1 elapsed_ns={} retained_logical_value_bytes=6; scope=WitnessJudgment::capture only, excludes request, fixture, snapshot, and frontier setup; descriptive only",
        elapsed.as_nanos(),
    );
}

#[test]
fn fixed_descriptive_capture_and_direct_scan_reuse_costs() {
    let (frontiers, marker) = closed_frontier();
    let base_store = Store::from([
        (0, (1, b"one".to_vec())),
        (2, (3, b"two".to_vec())),
        (9, (5, b"outside".to_vec())),
    ]);
    let base = snapshot(10, 20, DomainClosure::Closed(marker), &base_store);

    let mut inserted_store = base_store.clone();
    inserted_store.insert(1, (1, b"late".to_vec()));
    let inserted = snapshot(11, 21, DomainClosure::Closed(marker), &inserted_store);

    let mut deleted_store = base_store.clone();
    deleted_store.remove(&2);
    let deleted = snapshot(11, 21, DomainClosure::Closed(marker), &deleted_store);

    let mut value_changed_store = base_store.clone();
    value_changed_store.insert(0, (1, b"changed".to_vec()));
    let value_changed = snapshot(11, 21, DomainClosure::Closed(marker), &value_changed_store);

    let mut unrelated_store = base_store.clone();
    unrelated_store.insert(9, (6, b"changed-outside".to_vec()));
    let unrelated = snapshot(11, 21, DomainClosure::Closed(marker), &unrelated_store);

    let (first_result, first_capture) = timed_capture(&base, &frontiers, range_request());
    let first_judgment = first_result.expect("closed exact range capture");
    assert_eq!(first_judgment.range_member_count(), 2);
    assert_eq!(first_judgment.range_member_bytes(), 6);

    let mut capture_samples = vec![first_capture];
    let mut stable_reuse_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    let mut inserted_reuse_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    let mut deleted_reuse_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    let mut value_reuse_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    let mut unrelated_reuse_samples = Vec::with_capacity(SAMPLES_PER_CASE);

    for sample in 0..SAMPLES_PER_CASE {
        let (judgment, capture_elapsed) = if sample == 0 {
            (first_judgment.clone(), first_capture)
        } else {
            let request = range_request();
            let (result, elapsed) = timed_capture(&base, &frontiers, request);
            (result.expect("closed exact range capture"), elapsed)
        };
        if sample != 0 {
            capture_samples.push(capture_elapsed);
        }

        let (stable, elapsed) = timed_reuse(&judgment, &base, &frontiers);
        let Reuse::StillValid { cost } = stable else {
            panic!("unchanged exact range must remain valid");
        };
        assert_eq!(cost.frontier_checks(), 1);
        assert_eq!(cost.range_scans(), 1);
        assert_eq!(cost.range_members(), 2);
        assert_eq!(cost.range_member_bytes(), 6);
        stable_reuse_samples.push(elapsed);

        let (reuse, elapsed) = timed_reuse(&judgment, &inserted, &frontiers);
        let Reuse::Invalidated { reason, cost } = reuse else {
            panic!("interior insertion must invalidate exact membership");
        };
        assert_eq!(reason, Invalidation::RangeMembers);
        assert_eq!(cost.range_scans(), 1);
        assert_eq!(cost.range_members(), 2);
        assert_eq!(cost.range_member_bytes(), 7);
        inserted_reuse_samples.push(elapsed);

        let (reuse, elapsed) = timed_reuse(&judgment, &deleted, &frontiers);
        let Reuse::Invalidated { reason, cost } = reuse else {
            panic!("member deletion must invalidate exact membership");
        };
        assert_eq!(reason, Invalidation::RangeMembers);
        assert_eq!(cost.range_scans(), 1);
        assert_eq!(cost.range_members(), 1);
        assert_eq!(cost.range_member_bytes(), 3);
        deleted_reuse_samples.push(elapsed);

        let (reuse, elapsed) = timed_reuse(&judgment, &value_changed, &frontiers);
        let Reuse::Invalidated { reason, cost } = reuse else {
            panic!("member value change must invalidate exact membership");
        };
        assert_eq!(reason, Invalidation::RangeMembers);
        assert_eq!(cost.range_scans(), 1);
        assert_eq!(cost.range_members(), 1);
        assert_eq!(cost.range_member_bytes(), 7);
        value_reuse_samples.push(elapsed);

        let (reuse, elapsed) = timed_reuse(&judgment, &unrelated, &frontiers);
        assert!(matches!(reuse, Reuse::StillValid { .. }));
        unrelated_reuse_samples.push(elapsed);
    }

    assert_eq!(capture_samples.len(), SAMPLES_PER_CASE);
    assert_eq!(stable_reuse_samples.len(), SAMPLES_PER_CASE);
    assert_eq!(inserted_reuse_samples.len(), SAMPLES_PER_CASE);
    assert_eq!(deleted_reuse_samples.len(), SAMPLES_PER_CASE);
    assert_eq!(value_reuse_samples.len(), SAMPLES_PER_CASE);
    assert_eq!(unrelated_reuse_samples.len(), SAMPLES_PER_CASE);

    let mut incomplete_frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier");
    incomplete_frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("positive prefix without terminal marker");
    let incomplete = snapshot(10, 20, DomainClosure::Closed(marker), &base_store);
    let mut incomplete_capture_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    for _ in 0..SAMPLES_PER_CASE {
        let request = range_request();
        let (result, elapsed) = timed_capture(&incomplete, &incomplete_frontiers, request);
        assert_eq!(result, Err(Error::Incomplete));
        incomplete_capture_samples.push(elapsed);
    }
    assert_eq!(incomplete_capture_samples.len(), SAMPLES_PER_CASE);

    let max_bytes_store = (0..MAX_RANGE_WITNESS_ENTRIES as u64)
        .map(|key| (key, (1, vec![0; MAX_VALUE_BYTES])))
        .collect::<Store>();
    let max_bytes = snapshot(10, 20, DomainClosure::Closed(marker), &max_bytes_store);
    let max_bytes_request = || {
        vec![WitnessRequest::RangeMembers {
            start: 0,
            end: MAX_RANGE_WITNESS_ENTRIES as u64,
        }]
    };
    let mut max_bytes_capture_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    for _ in 0..SAMPLES_PER_CASE {
        let request = max_bytes_request();
        let (result, elapsed) = timed_capture(&max_bytes, &frontiers, request);
        let judgment = result.expect("exact 256 by 8192 logical-byte capture");
        assert_eq!(judgment.range_member_count(), MAX_RANGE_WITNESS_ENTRIES);
        assert_eq!(judgment.range_member_bytes(), MAX_RANGE_WITNESS_VALUE_BYTES);
        max_bytes_capture_samples.push(elapsed);
    }
    assert_eq!(max_bytes_capture_samples.len(), SAMPLES_PER_CASE);

    let mut direct_recompute_samples = Vec::with_capacity(SAMPLES_PER_CASE);
    for _ in 0..SAMPLES_PER_CASE {
        let started = Instant::now();
        let direct = direct_recompute(&base_store, 0, 4);
        let elapsed = started.elapsed();
        let direct_bytes = direct
            .iter()
            .map(|(_, _, value)| value.len())
            .sum::<usize>();
        assert_eq!(direct.len(), 2);
        assert_eq!(direct_bytes, 6);
        direct_recompute_samples.push(elapsed);
    }
    assert_eq!(direct_recompute_samples.len(), SAMPLES_PER_CASE);

    let top_empty = snapshot(10, 20, DomainClosure::Closed(marker), &Store::new());
    let top = WitnessJudgment::capture(
        &top_empty,
        &frontiers,
        vec![WitnessRequest::RangeMembers {
            start: u64::MAX - 1,
            end: u64::MAX,
        }],
    )
    .expect("top representable half-open range");
    let top_inserted = snapshot(
        11,
        21,
        DomainClosure::Closed(marker),
        &Store::from([(u64::MAX - 1, (1, b"top".to_vec()))]),
    );
    assert!(matches!(
        top.reuse_at(&top_inserted, &frontiers),
        Ok(Reuse::Invalidated {
            reason: Invalidation::RangeMembers,
            ..
        })
    ));

    eprintln!(
        "FA059 descriptive samples: capture={} stable={} inserted={} deleted={} value={} unrelated={} incomplete_refusal={} max_2MiB_capture={} direct_recompute={}; elapsed_ns: capture={} stable={} inserted={} deleted={} value={} unrelated={} incomplete_refusal={} max_2MiB_capture={} direct_recompute={}; scope=timers cover only capture, reuse, or independent BTreeMap recompute and exclude request, fixture, snapshot, and frontier setup; logical_bytes: stable_scanned=6 direct_recomputed=6 max_retained={}; not a benchmark or SLO",
        capture_samples.len(),
        stable_reuse_samples.len(),
        inserted_reuse_samples.len(),
        deleted_reuse_samples.len(),
        value_reuse_samples.len(),
        unrelated_reuse_samples.len(),
        incomplete_capture_samples.len(),
        max_bytes_capture_samples.len(),
        direct_recompute_samples.len(),
        duration_ns(&capture_samples),
        duration_ns(&stable_reuse_samples),
        duration_ns(&inserted_reuse_samples),
        duration_ns(&deleted_reuse_samples),
        duration_ns(&value_reuse_samples),
        duration_ns(&unrelated_reuse_samples),
        duration_ns(&incomplete_capture_samples),
        duration_ns(&max_bytes_capture_samples),
        duration_ns(&direct_recompute_samples),
        MAX_RANGE_WITNESS_VALUE_BYTES,
    );
}
