//! Exhaustive bounded public-API checks for FA-059 exact range witnesses.
//!
//! The `BTreeMap` scans in this file are the oracle: they do not reuse a
//! witness to decide whether an interval is unchanged. These are reference
//! snapshots under the caller-supplied adapter-domain assumption, not an
//! authentication, database, cache-authority, or production-effect proof.

use std::collections::BTreeMap;

use fa_reference::{
    Error,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, Invalidation,
        MAX_RANGE_WITNESS_ENTRIES, Reuse, SnapshotEntry, WitnessJudgment, WitnessRequest,
        WitnessSnapshot,
    },
};

type Store = BTreeMap<u64, (u64, Vec<u8>)>;
type ExactMember = (u64, u64, Vec<u8>);

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 41,
        branch: 43,
        projection: 47,
        source_epoch: 53,
    }
}

fn closed_frontiers() -> (ProductFrontiers, TrustedClosingMarker) {
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
        .expect("explicit authenticated closing observation");
    (frontiers, marker)
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
            .expect("bounded independent fixture"),
    )
    .expect("well-formed caller-supplied snapshot")
}

/// Direct exact scan oracle for the declared half-open range API.
fn scan(store: &Store, start: u64, end: u64) -> Vec<ExactMember> {
    store
        .range(start..end)
        .map(|(&key, (version, value))| (key, *version, value.clone()))
        .collect()
}

fn logical_bytes(members: &[ExactMember]) -> usize {
    members.iter().map(|(_, _, value)| value.len()).sum()
}

fn reuse(
    judgment: &WitnessJudgment,
    frontiers: &ProductFrontiers,
    marker: TrustedClosingMarker,
    store: &Store,
) -> Reuse {
    judgment
        .reuse_at(
            &snapshot(11, 21, DomainClosure::Closed(marker), store),
            frontiers,
        )
        .expect("newer closed snapshot")
}

fn assert_empty_result(
    judgment: &WitnessJudgment,
    frontiers: &ProductFrontiers,
    marker: TrustedClosingMarker,
    store: &Store,
    start: u64,
    end: u64,
) {
    let members = scan(store, start, end);
    match (
        members.is_empty(),
        reuse(judgment, frontiers, marker, store),
    ) {
        (true, Reuse::StillValid { cost }) => {
            assert_eq!(cost.range_scans(), 1);
            assert_eq!(cost.range_members(), 0);
            assert_eq!(cost.range_member_bytes(), 0);
        }
        (false, Reuse::Invalidated { reason, cost }) => {
            assert_eq!(reason, Invalidation::EmptyRange);
            assert_eq!(cost.range_scans(), 1);
            assert_eq!(cost.range_members(), 1);
            assert_eq!(cost.range_member_bytes(), members[0].2.len() as u32);
        }
        (expected_empty, actual) => panic!(
            "empty-range result disagreed with direct scan for [{start}, {end}): \
             expected_empty={expected_empty}, actual={actual:?}"
        ),
    }
}

fn assert_members_result(
    judgment: &WitnessJudgment,
    captured: &[ExactMember],
    frontiers: &ProductFrontiers,
    marker: TrustedClosingMarker,
    store: &Store,
    start: u64,
    end: u64,
) {
    let current = scan(store, start, end);
    match (
        current == captured,
        reuse(judgment, frontiers, marker, store),
    ) {
        (true, Reuse::StillValid { cost }) => {
            assert_eq!(cost.range_scans(), 1);
            assert_eq!(cost.range_members(), current.len() as u16);
            assert_eq!(cost.range_member_bytes(), logical_bytes(&current) as u32);
        }
        (false, Reuse::Invalidated { reason, cost }) => {
            assert_eq!(reason, Invalidation::RangeMembers);
            assert_eq!(cost.range_scans(), 1);
        }
        (expected_equal, actual) => panic!(
            "range-members result disagreed with direct scan for [{start}, {end}): \
             expected_equal={expected_equal}, actual={actual:?}"
        ),
    }
}

#[test]
fn exhaustive_empty_intervals_reject_every_interior_insertion_and_keep_half_open_endpoints() {
    let (frontiers, marker) = closed_frontiers();
    let initial_store = Store::from([
        (1, (10, b"one".to_vec())),
        (4, (11, b"four".to_vec())),
        (7, (12, b"seven".to_vec())),
        (12, (13, vec![0; 8 * 1024])),
    ]);

    for start in 0_u64..=8 {
        for end in (start + 1)..=9 {
            let captured = scan(&initial_store, start, end);
            if !captured.is_empty() {
                continue;
            }
            let judgment = WitnessJudgment::capture(
                &snapshot(10, 20, DomainClosure::Closed(marker), &initial_store),
                &frontiers,
                vec![WitnessRequest::EmptyRange { start, end }],
            )
            .expect("directly empty closed interval captures");

            assert_empty_result(&judgment, &frontiers, marker, &initial_store, start, end);

            for inserted_key in start..end {
                let mut inserted = initial_store.clone();
                inserted.insert(inserted_key, (99, vec![inserted_key as u8]));
                assert_empty_result(&judgment, &frontiers, marker, &inserted, start, end);
            }

            // `end` is outside `[start, end)`, even when its changed value is large.
            let mut at_exclusive_end = initial_store.clone();
            at_exclusive_end.insert(end, (100, vec![9; 8 * 1024]));
            assert_empty_result(&judgment, &frontiers, marker, &at_exclusive_end, start, end);
        }
    }
}

#[test]
fn exhaustive_membership_oracle_covers_empty_and_nonempty_sets_and_all_local_mutations() {
    let (frontiers, marker) = closed_frontiers();
    let initial_store = Store::from([
        (0, (10, b"a".to_vec())),
        (2, (11, b"bb".to_vec())),
        (5, (12, b"ccc".to_vec())),
        (8, (13, b"dddd".to_vec())),
        // This key is outside every enumerated range and exercises an unrelated control.
        (12, (14, vec![7; 8 * 1024])),
    ]);

    for start in 0_u64..=8 {
        for end in (start + 1)..=9 {
            let captured = scan(&initial_store, start, end);
            let judgment = WitnessJudgment::capture(
                &snapshot(10, 20, DomainClosure::Closed(marker), &initial_store),
                &frontiers,
                vec![WitnessRequest::RangeMembers { start, end }],
            )
            .expect("closed exact membership, including an exact empty member set");
            assert_eq!(judgment.range_member_count(), captured.len());
            assert_eq!(judgment.range_member_bytes(), logical_bytes(&captured));
            assert_members_result(
                &judgment,
                &captured,
                &frontiers,
                marker,
                &initial_store,
                start,
                end,
            );

            // A changed key outside the range is not a member scan and does not invalidate.
            let mut unrelated = initial_store.clone();
            unrelated.insert(12, (15, vec![8; 8 * 1024]));
            assert_members_result(
                &judgment, &captured, &frontiers, marker, &unrelated, start, end,
            );

            // The exclusive end remains outside the captured membership predicate.
            let mut exclusive_end = initial_store.clone();
            exclusive_end.insert(end, (16, b"end".to_vec()));
            assert_members_result(
                &judgment,
                &captured,
                &frontiers,
                marker,
                &exclusive_end,
                start,
                end,
            );

            for key in start..end {
                if let Some((version, value)) = initial_store.get(&key) {
                    let mut deleted = initial_store.clone();
                    deleted.remove(&key);
                    assert_members_result(
                        &judgment, &captured, &frontiers, marker, &deleted, start, end,
                    );

                    let mut changed_version = initial_store.clone();
                    changed_version.insert(key, (version + 1, value.clone()));
                    assert_members_result(
                        &judgment,
                        &captured,
                        &frontiers,
                        marker,
                        &changed_version,
                        start,
                        end,
                    );

                    let mut changed_value = initial_store.clone();
                    let mut different = value.clone();
                    different.push(0xff);
                    changed_value.insert(key, (*version, different));
                    assert_members_result(
                        &judgment,
                        &captured,
                        &frontiers,
                        marker,
                        &changed_value,
                        start,
                        end,
                    );
                } else {
                    let mut inserted = initial_store.clone();
                    inserted.insert(key, (17, b"late".to_vec()));
                    assert_members_result(
                        &judgment, &captured, &frontiers, marker, &inserted, start, end,
                    );
                }
            }
        }
    }
}

#[test]
fn closed_exact_membership_refuses_unknown_or_unrecorded_tails() {
    let (closed_frontiers, marker) = closed_frontiers();
    let store = Store::new();

    assert_eq!(
        WitnessJudgment::capture(
            &snapshot(10, 20, DomainClosure::Unknown, &store),
            &closed_frontiers,
            vec![WitnessRequest::RangeMembers { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );

    let mut open_frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    open_frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("positive prefix without closing observation");
    assert_eq!(
        WitnessJudgment::capture(
            &snapshot(10, 20, DomainClosure::Closed(marker), &store),
            &open_frontiers,
            vec![WitnessRequest::RangeMembers { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );
}

#[test]
fn range_member_retention_uses_exact_logical_members_and_preflights_aggregate_cap() {
    let (frontiers, marker) = closed_frontiers();
    let store = (0..MAX_RANGE_WITNESS_ENTRIES as u64)
        .map(|key| (key, (1, vec![key as u8])))
        .collect::<Store>();
    let snapshot = snapshot(10, 20, DomainClosure::Closed(marker), &store);

    let at_cap = WitnessJudgment::capture(
        &snapshot,
        &frontiers,
        vec![WitnessRequest::RangeMembers {
            start: 0,
            end: MAX_RANGE_WITNESS_ENTRIES as u64,
        }],
    )
    .expect("exact retained member cap");
    assert_eq!(at_cap.range_member_count(), MAX_RANGE_WITNESS_ENTRIES);
    assert_eq!(at_cap.range_member_bytes(), MAX_RANGE_WITNESS_ENTRIES);

    assert_eq!(
        WitnessJudgment::capture(
            &snapshot,
            &frontiers,
            vec![
                WitnessRequest::RangeMembers { start: 0, end: 128 },
                WitnessRequest::RangeMembers {
                    start: 127,
                    end: 256
                },
            ],
        ),
        Err(Error::Limit),
        "overlapping exact ranges exceed the aggregate cap before becoming a judgment"
    );
}

#[test]
fn half_open_finite_end_handles_u64_extremes_without_claiming_maximum_inclusion() {
    let (frontiers, marker) = closed_frontiers();
    let empty = Store::new();
    let empty_judgment = WitnessJudgment::capture(
        &snapshot(10, 20, DomainClosure::Closed(marker), &empty),
        &frontiers,
        vec![WitnessRequest::EmptyRange {
            start: u64::MAX - 1,
            end: u64::MAX,
        }],
    )
    .expect("highest expressible empty interval");
    let mut inserted_at_top_expressible_key = empty.clone();
    inserted_at_top_expressible_key.insert(u64::MAX - 1, (1, b"member".to_vec()));
    assert_empty_result(
        &empty_judgment,
        &frontiers,
        marker,
        &inserted_at_top_expressible_key,
        u64::MAX - 1,
        u64::MAX,
    );
    let mut maximum_outside_empty = empty;
    maximum_outside_empty.insert(u64::MAX, (1, b"outside".to_vec()));
    assert_empty_result(
        &empty_judgment,
        &frontiers,
        marker,
        &maximum_outside_empty,
        u64::MAX - 1,
        u64::MAX,
    );

    let range = WitnessRequest::RangeMembers {
        start: u64::MAX - 1,
        end: u64::MAX,
    };
    let initial = Store::from([(u64::MAX - 1, (1, b"top".to_vec()))]);
    let captured = scan(&initial, u64::MAX - 1, u64::MAX);
    let judgment = WitnessJudgment::capture(
        &snapshot(10, 20, DomainClosure::Closed(marker), &initial),
        &frontiers,
        vec![range],
    )
    .expect("highest expressible finite range");

    let mut outside_maximum = initial.clone();
    outside_maximum.insert(u64::MAX, (2, b"outside".to_vec()));
    assert_members_result(
        &judgment,
        &captured,
        &frontiers,
        marker,
        &outside_maximum,
        u64::MAX - 1,
        u64::MAX,
    );

    assert_eq!(
        WitnessJudgment::capture(
            &snapshot(10, 20, DomainClosure::Closed(marker), &initial),
            &frontiers,
            vec![WitnessRequest::RangeMembers {
                start: u64::MAX,
                end: u64::MAX,
            }],
        ),
        Err(Error::InvalidInput),
        "the finite half-open API intentionally has no representation containing u64::MAX"
    );
}
