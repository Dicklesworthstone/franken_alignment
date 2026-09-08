//! Public-API boundaries for FA-058's exact-value and absent-key witnesses.
//!
//! These fixtures are caller-supplied reference inputs, not an adapter,
//! authenticated storage implementation, cache, or dispatch authorization.

use fa_reference::{
    Error,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, Invalidation, QueryRole, Reuse,
        SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot,
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

fn domain(closure: DomainClosure) -> AdapterDomainInput {
    AdapterDomainInput::new(DomainProjection::new(41, 43, projection()), closure)
}

fn entry(key: u64, version: u64, value: &[u8]) -> SnapshotEntry {
    SnapshotEntry::new(key, version, value.to_vec()).expect("bounded fixture value")
}

fn snapshot(
    revision: u64,
    control_cut: u64,
    closure: DomainClosure,
    entries: Vec<SnapshotEntry>,
) -> WitnessSnapshot {
    WitnessSnapshot::new(revision, control_cut, 47, domain(closure), entries)
        .expect("well-formed fixture snapshot")
}

fn closed_frontier() -> (ProductFrontiers, TrustedClosingMarker) {
    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .expect("authenticated prefix");
    let marker = TrustedClosingMarker {
        key: projection(),
        final_sequence: 1,
        marker_generation: 5,
    };
    frontiers
        .record_close(marker)
        .expect("complete authenticated closure");
    (frontiers, marker)
}

#[test]
fn exact_value_binds_version_query_role_and_snapshot_without_unrelated_invalidation() {
    let open_frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    let initial = snapshot(
        70,
        80,
        DomainClosure::Unknown,
        vec![entry(101, 7, b"reviewed"), entry(202, 1, b"unrelated")],
    );
    let judgment = WitnessJudgment::capture(
        &initial,
        &open_frontiers,
        vec![WitnessRequest::ExactValue {
            key: 101,
            role: QueryRole::PredicateInput,
        }],
    )
    .expect("exact read does not need a closed domain");

    assert_eq!(
        judgment.exact_version_and_role(101),
        Some((7, QueryRole::PredicateInput))
    );
    assert_eq!(judgment.exact_version_and_role(202), None);

    let unrelated_changed = snapshot(
        71,
        81,
        DomainClosure::Unknown,
        vec![
            entry(101, 7, b"reviewed"),
            entry(202, 2, b"changed elsewhere"),
        ],
    );
    let reuse = judgment
        .reuse_at(&unrelated_changed, &open_frontiers)
        .expect("newer unrelated snapshot is comparable");
    let Reuse::StillValid { cost } = reuse else {
        panic!("unrelated key changed a declared exact dependency");
    };
    assert_eq!(cost.exact_reads(), 1);
    assert_eq!(cost.absent_reads(), 0);
    assert_eq!(cost.frontier_checks(), 0);

    let semantic_changed = WitnessSnapshot::new(
        initial.revision(),
        initial.control_cut(),
        48,
        initial.domain_input(),
        vec![entry(101, 7, b"reviewed"), entry(202, 1, b"unrelated")],
    )
    .expect("only the explicit semantic epoch changed");
    assert!(matches!(
        judgment.reuse_at(&initial, &open_frontiers),
        Ok(Reuse::StillValid { .. })
    ));
    assert!(matches!(
        judgment.reuse_at(&semantic_changed, &open_frontiers),
        Ok(Reuse::Invalidated {
            reason: Invalidation::SemanticEpoch,
            ..
        })
    ));

    let value_changed = snapshot(
        71,
        81,
        DomainClosure::Unknown,
        vec![entry(101, 7, b"different"), entry(202, 1, b"unrelated")],
    );
    assert!(matches!(
        judgment.reuse_at(&value_changed, &open_frontiers),
        Ok(Reuse::Invalidated {
            reason: Invalidation::ExactValue,
            ..
        })
    ));
    assert_eq!(
        judgment.reuse_at(
            &snapshot(
                69,
                80,
                DomainClosure::Unknown,
                vec![entry(101, 7, b"reviewed"), entry(202, 1, b"unrelated")],
            ),
            &open_frontiers,
        ),
        Err(Error::Stale)
    );
    assert_eq!(
        judgment.reuse_at(
            &snapshot(
                70,
                79,
                DomainClosure::Unknown,
                vec![entry(101, 7, b"reviewed"), entry(202, 1, b"unrelated")],
            ),
            &open_frontiers,
        ),
        Err(Error::Stale)
    );
}

#[test]
fn incomplete_domain_allows_exact_reads_but_refuses_absence_until_closed() {
    let open_frontiers = ProductFrontiers::new(1, 4).expect("bounded frontier configuration");
    let incomplete = snapshot(
        10,
        20,
        DomainClosure::Unknown,
        vec![entry(101, 7, b"reviewed")],
    );
    let exact = WitnessJudgment::capture(
        &incomplete,
        &open_frontiers,
        vec![WitnessRequest::ExactValue {
            key: 101,
            role: QueryRole::Subject,
        }],
    )
    .expect("exact positive witness remains available in an incomplete domain");
    assert!(matches!(
        exact.reuse_at(&incomplete, &open_frontiers),
        Ok(Reuse::StillValid { .. })
    ));
    assert_eq!(
        WitnessJudgment::capture(
            &incomplete,
            &open_frontiers,
            vec![WitnessRequest::AbsentKey { key: 303 }],
        ),
        Err(Error::Incomplete)
    );

    let (frontiers, marker) = closed_frontier();
    let closed = snapshot(
        10,
        20,
        DomainClosure::Closed(marker),
        vec![entry(101, 7, b"reviewed")],
    );
    let absence = WitnessJudgment::capture(
        &closed,
        &frontiers,
        vec![WitnessRequest::AbsentKey { key: 303 }],
    )
    .expect("closed authenticated domain can support the absent-key witness");
    assert!(matches!(
        absence.reuse_at(&closed, &frontiers),
        Ok(Reuse::StillValid { .. })
    ));
}

#[test]
fn inserting_a_formerly_absent_key_refuses_reuse_at_a_newer_snapshot() {
    let (frontiers, marker) = closed_frontier();
    let initial = snapshot(
        100,
        110,
        DomainClosure::Closed(marker),
        vec![entry(101, 7, b"reviewed")],
    );
    let absence = WitnessJudgment::capture(
        &initial,
        &frontiers,
        vec![WitnessRequest::AbsentKey { key: 303 }],
    )
    .expect("closed initial absence");
    assert!(matches!(
        absence.reuse_at(&initial, &frontiers),
        Ok(Reuse::StillValid { .. })
    ));
    let inserted = snapshot(
        101,
        111,
        DomainClosure::Closed(marker),
        vec![entry(101, 7, b"reviewed"), entry(303, 1, b"now present")],
    );

    assert!(matches!(
        absence.reuse_at(&inserted, &frontiers),
        Ok(Reuse::Invalidated {
            reason: Invalidation::AbsentKey,
            ..
        })
    ));
}
