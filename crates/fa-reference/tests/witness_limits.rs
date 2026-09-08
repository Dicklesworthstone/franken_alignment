//! Public boundary controls for FA-058 witness snapshot and request limits.
//!
//! These assert bounded reference semantics only. They make no allocator,
//! throughput, storage, adapter-authentication, or authority claim.

use fa_reference::{
    Error,
    product_frontier::{ProductFrontiers, ProjectionKey},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES,
        MAX_WITNESSES, QueryRole, Reuse, SnapshotEntry, WitnessJudgment, WitnessRequest,
        WitnessSnapshot,
    },
};

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 23,
        branch: 29,
        projection: 31,
        source_epoch: 37,
    }
}

fn unknown_domain() -> AdapterDomainInput {
    AdapterDomainInput::new(
        DomainProjection::new(41, 43, projection()),
        DomainClosure::Unknown,
    )
}

fn entries(count: usize, value_len: usize) -> Vec<SnapshotEntry> {
    (0..count)
        .map(|key| {
            SnapshotEntry::new(key as u64, 7, vec![(key & 0xff) as u8; value_len])
                .expect("fixed value length is within the requested constructor bound")
        })
        .collect()
}

fn unknown_snapshot(entries: Vec<SnapshotEntry>) -> WitnessSnapshot {
    WitnessSnapshot::new(70, 80, 47, unknown_domain(), entries)
        .expect("fixed unknown-domain snapshot must be well formed")
}

#[test]
fn exact_snapshot_entry_and_value_bounds_accept_and_refuse() {
    let exact_entries = entries(MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES);
    let logical_value_bytes = exact_entries
        .iter()
        .map(|entry| entry.value().len())
        .sum::<usize>();
    assert_eq!(
        logical_value_bytes,
        2 * 1024 * 1024,
        "the exact accepted snapshot control is 256 distinct 8 KiB values"
    );
    let exact_snapshot = unknown_snapshot(exact_entries);
    assert_eq!(
        exact_snapshot
            .entry(0)
            .map(|entry| (entry.value().len(), entry.value()[0])),
        Some((MAX_VALUE_BYTES, 0))
    );
    assert_eq!(
        exact_snapshot
            .entry((MAX_SNAPSHOT_ENTRIES - 1) as u64)
            .map(SnapshotEntry::value)
            .map(|value| value.len()),
        Some(MAX_VALUE_BYTES)
    );

    let over_entries = entries(MAX_SNAPSHOT_ENTRIES + 1, 0);
    assert_eq!(
        WitnessSnapshot::new(70, 80, 47, unknown_domain(), over_entries),
        Err(Error::Limit)
    );

    assert!(SnapshotEntry::new(999, 7, vec![1; MAX_VALUE_BYTES]).is_ok());
    assert_eq!(
        SnapshotEntry::new(999, 7, vec![1; MAX_VALUE_BYTES + 1]),
        Err(Error::Limit)
    );
}

#[test]
fn exact_witness_request_bound_reuses_under_unknown_frontier_and_refuses_over_limit() {
    let snapshot = unknown_snapshot(entries(MAX_WITNESSES, 1));
    let open_frontiers = ProductFrontiers::new(1, 4).expect("fixed bounded frontier configuration");
    let exact_requests = (0..MAX_WITNESSES)
        .map(|key| WitnessRequest::ExactValue {
            key: key as u64,
            role: QueryRole::PredicateInput,
        })
        .collect::<Vec<_>>();
    let judgment = WitnessJudgment::capture(&snapshot, &open_frontiers, exact_requests)
        .expect("exact requests do not need an absent-key closure");
    let Reuse::StillValid { cost } = judgment
        .reuse_at(&snapshot, &open_frontiers)
        .expect("untouched baseline values remain reusable")
    else {
        panic!("untouched exact baseline unexpectedly invalidated");
    };
    assert_eq!(cost.exact_reads(), MAX_WITNESSES as u16);
    assert_eq!(cost.absent_reads(), 0);
    assert_eq!(cost.frontier_checks(), 0);
    for key in 0..MAX_WITNESSES {
        assert_eq!(
            snapshot.entry(key as u64).map(SnapshotEntry::value),
            Some(&[(key & 0xff) as u8][..]),
            "accepted exact request must retain its untouched baseline value"
        );
    }

    let over_snapshot = unknown_snapshot(entries(MAX_WITNESSES + 1, 1));
    let over_requests = (0..=MAX_WITNESSES)
        .map(|key| WitnessRequest::ExactValue {
            key: key as u64,
            role: QueryRole::PredicateInput,
        })
        .collect();
    assert_eq!(
        WitnessJudgment::capture(&over_snapshot, &open_frontiers, over_requests),
        Err(Error::Limit)
    );
}
