//! Exact closing identity versus prefix coverage, through public witness APIs.
use fa_reference::Error;
use fa_reference::product_frontier::{FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, Invalidation, QueryRole, Reuse, SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot};
use fa_reference::witness::refinement::{RefinementBudget, RefinementOutcome};
use fa_reference::witness::refinement::index::{IndexFallback, IndexSelection, WitnessChangeIndex};

fn projection() -> ProjectionKey {
    ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 }
}

fn closed(final_sequence: u64) -> (ProductFrontiers, TrustedClosingMarker) {
    let key = projection();
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    for sequence in 1..=final_sequence {
        frontiers.accept(key, FrontierStage::Authenticated, sequence).unwrap();
    }
    let marker = TrustedClosingMarker { key, final_sequence, marker_generation: 7 };
    frontiers.record_close(marker).unwrap();
    (frontiers, marker)
}

fn snapshot(marker: TrustedClosingMarker, revision: u64) -> WitnessSnapshot {
    WitnessSnapshot::new(revision, revision, 1,
        AdapterDomainInput::new(DomainProjection::new(1, 1, projection()), DomainClosure::Closed(marker)),
        vec![SnapshotEntry::new(0, 1, b"value".to_vec()).unwrap()],
    ).unwrap()
}

fn negatives() -> [WitnessRequest; 3] {
    [
        WitnessRequest::AbsentKey { key: 7 },
        WitnessRequest::EmptyRange { start: 10, end: 20 },
        WitnessRequest::RangeMembers { start: 0, end: 5 },
    ]
}

fn ample() -> RefinementBudget {
    RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX }
}

#[test]
fn prefix_coverage_does_not_claim_exact_terminal_identity() {
    let (frontiers, marker) = closed(2);
    assert_eq!(frontiers.closing_marker(projection()), Some(marker));
    assert_eq!(frontiers.closing_marker(ProjectionKey { source: 2, ..projection() }), None);
    for through in [0, 1, 2] {
        // This existing general-purpose API intentionally permits a sub-prefix.
        assert_eq!(frontiers.satisfies(FrontierRequirement {
            key: projection(), stage: FrontierStage::Authenticated,
            through, closure: Some(marker.marker_generation),
        }), Ok(true));
    }
    let open = ProductFrontiers::new(1, 8).unwrap();
    assert_eq!(open.closing_marker(projection()), None);
}

#[test]
fn all_negative_capture_kinds_require_the_exact_recorded_terminal() {
    let (frontiers, marker) = closed(2);
    for request in negatives() {
        for declared_final in [0, 1, 3] {
            let truncated = snapshot(TrustedClosingMarker { final_sequence: declared_final, ..marker }, 10);
            assert_eq!(WitnessJudgment::capture(&truncated, &frontiers, vec![request]), Err(Error::Incomplete));
        }
        let exact = snapshot(marker, 10);
        let judgment = WitnessJudgment::capture(&exact, &frontiers, vec![request]).unwrap();
        assert!(matches!(judgment.reuse_at(&exact, &frontiers), Ok(Reuse::StillValid { .. })));
    }
    // Positive exact-value reads do not acquire an unrelated closure requirement.
    let truncated = snapshot(TrustedClosingMarker { final_sequence: 0, ..marker }, 10);
    let exact = WitnessJudgment::capture(&truncated, &frontiers, vec![
        WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
    ]).unwrap();
    assert!(matches!(exact.reuse_at(&truncated, &frontiers), Ok(Reuse::StillValid { .. })));
}

#[test]
fn eager_and_budgeted_reuse_refuse_same_generation_terminal_substitution() {
    let (original_frontiers, original_marker) = closed(1);
    let (extended_frontiers, extended_marker) = closed(2);
    let original = snapshot(original_marker, 10);
    let mismatched = snapshot(original_marker, 11);
    let actual = snapshot(extended_marker, 11);
    for request in negatives() {
        let judgment = WitnessJudgment::capture(&original, &original_frontiers, vec![request]).unwrap();
        assert_eq!(judgment.reuse_at(&mismatched, &extended_frontiers), Err(Error::Incomplete));
        let refused = judgment.begin_refinement(&mismatched, &extended_frontiers).advance(ample());
        assert_eq!(refused.outcome, RefinementOutcome::Refused(Error::Incomplete));
        assert!(refused.total.steps > 0);
        // Properly declaring the new marker is a known invalidation, not unknown evidence.
        assert!(matches!(judgment.reuse_at(&actual, &extended_frontiers), Ok(Reuse::Invalidated {
            reason: Invalidation::ClosingFrontier, ..
        })));
    }
}

#[test]
fn complete_tail_admission_and_fallback_share_exact_closure_checks() {
    let (frontiers, marker) = closed(1);
    let (extended, _) = closed(2);
    let first = snapshot(marker, 10);
    let wrong = snapshot(marker, 11);
    assert!(matches!(WitnessChangeIndex::new(&wrong, &extended, 1), Err(Error::Incomplete)));
    let mut index = WitnessChangeIndex::new(&first, &frontiers, 1).unwrap();
    let captured = index.capture(&frontiers, vec![WitnessRequest::AbsentKey { key: 7 }]).unwrap();
    assert_eq!(index.observe(&wrong, &extended), Err(Error::Incomplete));
    assert_eq!(index.current_revision(), 10);
    let (plan, mut cursor) = index.begin_refinement(&captured, &wrong, &extended, 100);
    assert_eq!(plan.selection, IndexSelection::ExactFallback(IndexFallback::TargetNotIndexed));
    assert_eq!(cursor.advance(ample()).outcome, RefinementOutcome::Refused(Error::Incomplete));
}

#[test]
fn a_genuinely_empty_terminal_still_establishes_absence() {
    let (frontiers, marker) = closed(0);
    let empty = WitnessSnapshot::new(0, 0, 0,
        AdapterDomainInput::new(DomainProjection::new(1, 1, projection()), DomainClosure::Closed(marker)),
        vec![],
    ).unwrap();
    for request in negatives() {
        let judgment = WitnessJudgment::capture(&empty, &frontiers, vec![request]).unwrap();
        assert!(matches!(judgment.reuse_at(&empty, &frontiers), Ok(Reuse::StillValid { .. })));
        assert_eq!(judgment.begin_refinement(&empty, &frontiers).advance(ample()).outcome, RefinementOutcome::StillValid);
    }
}
