//! Public boundary tests for FA-059 exact range witnesses.
//!
//! These are bounded logical snapshots supplied under the explicit
//! adapter-authenticated-domain assumption. They do not authenticate an
//! adapter, provide an approximate absence proof, integrate a database, or
//! authorize any production effect.

use std::collections::BTreeMap;

use fa_reference::{
    Error,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, Invalidation,
        MAX_RANGE_WITNESS_ENTRIES, MAX_RANGE_WITNESS_VALUE_BYTES, MAX_VALUE_BYTES, Reuse,
        SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot,
    },
};

type Store = BTreeMap<u64, (u64, Vec<u8>)>;

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 13,
        branch: 17,
        projection: 19,
        source_epoch: 23,
    }
}

fn domain(closure: DomainClosure) -> AdapterDomainInput {
    AdapterDomainInput::new(DomainProjection::new(29, 31, projection()), closure)
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
        37,
        domain(closure),
        store
            .iter()
            .map(|(&key, (version, value))| SnapshotEntry::new(key, *version, value.clone()))
            .collect::<Result<Vec<_>, _>>()
            .expect("bounded test store"),
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
        .expect("explicit authenticated terminal marker");
    (frontiers, marker)
}

/// Deliberately independent from witness reuse: direct bounded reference scan.
fn exact_scan(store: &Store, start: u64, end: u64) -> Vec<(u64, u64, Vec<u8>)> {
    store
        .range(start..end)
        .map(|(&key, (version, value))| (key, *version, value.clone()))
        .collect()
}

#[test]
fn every_bounded_empty_interval_rejects_each_interior_insertion_but_not_its_exclusive_end() {
    let (frontiers, marker) = closed_frontier();
    let initial_store = Store::new();

    for start in 0_u64..=3 {
        for end in (start + 1)..=4 {
            assert!(exact_scan(&initial_store, start, end).is_empty());
            let initial = snapshot(10, 20, DomainClosure::Closed(marker), &initial_store);
            let judgment = WitnessJudgment::capture(
                &initial,
                &frontiers,
                vec![WitnessRequest::EmptyRange { start, end }],
            )
            .expect("closed exact empty interval");
            assert!(matches!(
                judgment.reuse_at(&initial, &frontiers),
                Ok(Reuse::StillValid { .. })
            ));

            for inserted_key in start..end {
                let mut inserted = initial_store.clone();
                inserted.insert(inserted_key, (1, vec![inserted_key as u8]));
                assert_eq!(exact_scan(&inserted, start, end).len(), 1);
                assert!(matches!(
                    judgment.reuse_at(
                        &snapshot(11, 21, DomainClosure::Closed(marker), &inserted),
                        &frontiers,
                    ),
                    Ok(Reuse::Invalidated {
                        reason: Invalidation::EmptyRange,
                        ..
                    })
                ));
            }

            let mut exclusive_end = initial_store.clone();
            exclusive_end.insert(end, (1, b"outside".to_vec()));
            assert!(exact_scan(&exclusive_end, start, end).is_empty());
            assert!(matches!(
                judgment.reuse_at(
                    &snapshot(11, 21, DomainClosure::Closed(marker), &exclusive_end),
                    &frontiers,
                ),
                Ok(Reuse::StillValid { .. })
            ));
        }
    }
}

#[test]
fn exact_membership_uses_direct_scan_and_detects_insert_delete_and_value_version_phantoms() {
    let (frontiers, marker) = closed_frontier();
    let store = Store::from([
        (0, (1, b"one".to_vec())),
        (2, (3, b"two".to_vec())),
        (5, (7, b"unrelated".to_vec())),
    ]);
    let initial = snapshot(10, 20, DomainClosure::Closed(marker), &store);
    let judgment = WitnessJudgment::capture(
        &initial,
        &frontiers,
        vec![WitnessRequest::RangeMembers { start: 0, end: 4 }],
    )
    .expect("closed range with exact nonempty membership");

    let oracle = exact_scan(&store, 0, 4);
    let oracle_bytes = oracle
        .iter()
        .map(|(_, _, value)| value.len())
        .sum::<usize>();
    assert_eq!(oracle.len(), 2);
    assert_eq!(judgment.range_member_count(), oracle.len());
    assert_eq!(judgment.range_member_bytes(), oracle_bytes);

    let mut unrelated = store.clone();
    unrelated.insert(5, (8, b"changed outside range".to_vec()));
    let reuse = judgment
        .reuse_at(
            &snapshot(11, 21, DomainClosure::Closed(marker), &unrelated),
            &frontiers,
        )
        .expect("newer snapshot with unchanged exact range");
    let Reuse::StillValid { cost } = reuse else {
        panic!("unrelated key must not invalidate exact membership");
    };
    assert_eq!(cost.frontier_checks(), 1);
    assert_eq!(cost.range_scans(), 1);
    assert_eq!(cost.range_members(), oracle.len() as u16);
    assert_eq!(cost.range_member_bytes(), oracle_bytes as u32);

    let mut inserted = store.clone();
    inserted.insert(1, (1, b"late".to_vec()));
    assert_eq!(exact_scan(&inserted, 0, 4).len(), 3);
    assert_range_members_invalid(&judgment, &frontiers, marker, &inserted);

    let mut deleted = store.clone();
    deleted.remove(&2);
    assert_eq!(exact_scan(&deleted, 0, 4).len(), 1);
    assert_range_members_invalid(&judgment, &frontiers, marker, &deleted);

    let mut version_changed = store.clone();
    version_changed.insert(0, (2, b"one".to_vec()));
    assert_range_members_invalid(&judgment, &frontiers, marker, &version_changed);

    let mut value_changed = store;
    value_changed.insert(0, (1, b"changed".to_vec()));
    assert_range_members_invalid(&judgment, &frontiers, marker, &value_changed);
}

fn assert_range_members_invalid(
    judgment: &WitnessJudgment,
    frontiers: &ProductFrontiers,
    marker: TrustedClosingMarker,
    store: &Store,
) {
    assert!(matches!(
        judgment.reuse_at(
            &snapshot(11, 21, DomainClosure::Closed(marker), store),
            frontiers,
        ),
        Ok(Reuse::Invalidated {
            reason: Invalidation::RangeMembers,
            ..
        })
    ));
}

#[test]
fn only_exact_recorded_closure_supports_empty_or_membership_range_witnesses() {
    let (recorded_frontiers, recorded_marker) = closed_frontier();
    let recorded_empty = snapshot(
        10,
        20,
        DomainClosure::Closed(recorded_marker),
        &Store::new(),
    );
    let empty_judgment = WitnessJudgment::capture(
        &recorded_empty,
        &recorded_frontiers,
        vec![WitnessRequest::EmptyRange { start: 1, end: 2 }],
    )
    .expect("same exact snapshot with recorded exact marker");
    let recorded_members_store = Store::from([(1, (1, b"member".to_vec()))]);
    let recorded_members = snapshot(
        10,
        20,
        DomainClosure::Closed(recorded_marker),
        &recorded_members_store,
    );
    let members_judgment = WitnessJudgment::capture(
        &recorded_members,
        &recorded_frontiers,
        vec![WitnessRequest::RangeMembers { start: 1, end: 2 }],
    )
    .expect("same exact membership snapshot with recorded exact marker");
    assert!(matches!(
        empty_judgment.reuse_at(&recorded_empty, &recorded_frontiers),
        Ok(Reuse::StillValid { .. })
    ));
    assert!(matches!(
        members_judgment.reuse_at(&recorded_members, &recorded_frontiers),
        Ok(Reuse::StillValid { .. })
    ));

    let conservative_empty = snapshot(10, 20, DomainClosure::ConservativeSummary, &Store::new());
    let conservative_members = snapshot(
        10,
        20,
        DomainClosure::ConservativeSummary,
        &recorded_members_store,
    );
    assert_eq!(
        WitnessJudgment::capture(
            &conservative_empty,
            &recorded_frontiers,
            vec![WitnessRequest::EmptyRange { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );
    assert_eq!(
        WitnessJudgment::capture(
            &conservative_members,
            &recorded_frontiers,
            vec![WitnessRequest::RangeMembers { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );
    assert_eq!(
        empty_judgment.reuse_at(&conservative_empty, &recorded_frontiers),
        Err(Error::Incomplete)
    );
    assert_eq!(
        members_judgment.reuse_at(&conservative_members, &recorded_frontiers),
        Err(Error::Incomplete)
    );

    let unknown_empty = snapshot(10, 20, DomainClosure::Unknown, &Store::new());
    let unknown_members = snapshot(10, 20, DomainClosure::Unknown, &recorded_members_store);
    assert_eq!(
        empty_judgment.reuse_at(&unknown_empty, &recorded_frontiers),
        Err(Error::Incomplete)
    );
    assert_eq!(
        members_judgment.reuse_at(&unknown_members, &recorded_frontiers),
        Err(Error::Incomplete)
    );

    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("only a positive prefix");
    let unrecorded_terminal = TrustedClosingMarker {
        key: projection(),
        final_sequence: 1,
        marker_generation: 1,
    };

    let empty = snapshot(
        10,
        20,
        DomainClosure::Closed(unrecorded_terminal),
        &Store::new(),
    );
    assert_eq!(
        WitnessJudgment::capture(
            &empty,
            &frontiers,
            vec![WitnessRequest::EmptyRange { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );
    assert_eq!(
        empty_judgment.reuse_at(&empty, &frontiers),
        Err(Error::Incomplete)
    );

    let members_store = Store::from([(1, (1, b"member".to_vec()))]);
    let members = snapshot(
        10,
        20,
        DomainClosure::Closed(unrecorded_terminal),
        &members_store,
    );
    assert_eq!(
        WitnessJudgment::capture(
            &members,
            &frontiers,
            vec![WitnessRequest::RangeMembers { start: 1, end: 2 }],
        ),
        Err(Error::Incomplete)
    );
    assert_eq!(
        members_judgment.reuse_at(&members, &frontiers),
        Err(Error::Incomplete)
    );
}

#[test]
fn range_retention_is_exact_and_aggregate_bounded() {
    let (frontiers, marker) = closed_frontier();
    let store = (0..MAX_RANGE_WITNESS_ENTRIES as u64)
        .map(|key| (key, (1, vec![key as u8])))
        .collect::<Store>();
    let initial = snapshot(10, 20, DomainClosure::Closed(marker), &store);

    let at_entry_cap = WitnessJudgment::capture(
        &initial,
        &frontiers,
        vec![WitnessRequest::RangeMembers {
            start: 0,
            end: MAX_RANGE_WITNESS_ENTRIES as u64,
        }],
    )
    .expect("exact aggregate entry cap");
    assert_eq!(at_entry_cap.range_member_count(), MAX_RANGE_WITNESS_ENTRIES);
    assert_eq!(at_entry_cap.range_member_bytes(), MAX_RANGE_WITNESS_ENTRIES);
    assert!(at_entry_cap.range_member_bytes() <= MAX_RANGE_WITNESS_VALUE_BYTES);

    let max_bytes_store = (0..MAX_RANGE_WITNESS_ENTRIES as u64)
        .map(|key| (key, (1, vec![0; MAX_VALUE_BYTES])))
        .collect::<Store>();
    let max_bytes_snapshot = snapshot(10, 20, DomainClosure::Closed(marker), &max_bytes_store);
    let at_value_byte_cap = WitnessJudgment::capture(
        &max_bytes_snapshot,
        &frontiers,
        vec![WitnessRequest::RangeMembers {
            start: 0,
            end: MAX_RANGE_WITNESS_ENTRIES as u64,
        }],
    )
    .expect("exact aggregate logical-value-byte cap");
    assert_eq!(
        at_value_byte_cap.range_member_count(),
        MAX_RANGE_WITNESS_ENTRIES
    );
    assert_eq!(
        at_value_byte_cap.range_member_bytes(),
        MAX_RANGE_WITNESS_VALUE_BYTES
    );

    assert_eq!(
        WitnessJudgment::capture(
            &initial,
            &frontiers,
            vec![
                WitnessRequest::RangeMembers {
                    start: 0,
                    end: MAX_RANGE_WITNESS_ENTRIES as u64,
                },
                WitnessRequest::RangeMembers {
                    start: 0,
                    end: (MAX_RANGE_WITNESS_ENTRIES - 1) as u64,
                },
            ],
        ),
        Err(Error::Limit)
    );

    let empty_members = WitnessJudgment::capture(
        &snapshot(10, 20, DomainClosure::Closed(marker), &Store::new()),
        &frontiers,
        vec![WitnessRequest::RangeMembers { start: 0, end: 1 }],
    )
    .expect("authenticated empty membership is a phantom-sensitive witness");
    assert_eq!(empty_members.range_member_count(), 0);
    assert_eq!(empty_members.range_member_bytes(), 0);
    assert!(matches!(
        empty_members.reuse_at(
            &snapshot(10, 20, DomainClosure::Closed(marker), &Store::new()),
            &frontiers,
        ),
        Ok(Reuse::StillValid { .. })
    ));
    let inserted = Store::from([(0, (1, b"late-member".to_vec()))]);
    assert_range_members_invalid(&empty_members, &frontiers, marker, &inserted);
}

#[test]
fn half_open_profile_handles_the_top_expressible_interval_without_end_plus_one() {
    let (frontiers, marker) = closed_frontier();
    let empty = snapshot(10, 20, DomainClosure::Closed(marker), &Store::new());
    let judgment = WitnessJudgment::capture(
        &empty,
        &frontiers,
        vec![WitnessRequest::EmptyRange {
            start: u64::MAX - 1,
            end: u64::MAX,
        }],
    )
    .expect("top representable half-open interval");
    assert!(matches!(
        judgment.reuse_at(&empty, &frontiers),
        Ok(Reuse::StillValid { .. })
    ));

    let inserted = Store::from([(u64::MAX - 1, (1, b"top".to_vec()))]);
    assert!(matches!(
        judgment.reuse_at(
            &snapshot(11, 21, DomainClosure::Closed(marker), &inserted),
            &frontiers,
        ),
        Ok(Reuse::Invalidated {
            reason: Invalidation::EmptyRange,
            ..
        })
    ));
    assert_eq!(
        WitnessJudgment::capture(
            &empty,
            &frontiers,
            vec![WitnessRequest::EmptyRange {
                start: u64::MAX,
                end: u64::MAX,
            }],
        ),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        WitnessJudgment::capture(
            &empty,
            &frontiers,
            vec![WitnessRequest::RangeMembers {
                start: u64::MAX,
                end: u64::MAX,
            }],
        ),
        Err(Error::InvalidInput)
    );
}
