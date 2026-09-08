//! Public product-frontier boundary tests.
//!
//! A `TrustedClosingMarker` is caller-supplied declarative trust input in this
//! reference model. These tests bind its key, generation, and contiguity
//! semantics; they do not assert marker authentication or adapter provenance.

use fa_reference::Error;
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};

fn key(source: u64, branch: u64, projection: u64, source_epoch: u64) -> ProjectionKey {
    ProjectionKey {
        source,
        branch,
        projection,
        source_epoch,
    }
}

fn marker(key: ProjectionKey, final_sequence: u64, marker_generation: u64) -> TrustedClosingMarker {
    TrustedClosingMarker {
        key,
        final_sequence,
        marker_generation,
    }
}

fn requirement(
    key: ProjectionKey,
    stage: FrontierStage,
    through: u64,
    closure: Option<u64>,
) -> FrontierRequirement {
    FrontierRequirement {
        key,
        stage,
        through,
        closure,
    }
}

fn accept_prefix(
    frontiers: &mut ProductFrontiers,
    key: ProjectionKey,
    stage: FrontierStage,
    through: u64,
) {
    for sequence in 1..=through {
        frontiers.accept(key, stage, sequence).unwrap();
    }
}

#[test]
fn closed_requirement_binds_its_exact_marker_generation_and_stage_prefix() {
    let stream = key(7, 11, 13, 17);
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    accept_prefix(&mut frontiers, stream, FrontierStage::Authenticated, 2);
    accept_prefix(&mut frontiers, stream, FrontierStage::Judged, 2);

    let receipt_requirement = requirement(stream, FrontierStage::Judged, 2, Some(31));

    // Positive prefix progress is not a closed receipt obligation until the
    // explicit marker observation is recorded.
    assert_eq!(frontiers.satisfies(receipt_requirement), Ok(false));
    frontiers.record_close(marker(stream, 2, 31)).unwrap();
    assert_eq!(frontiers.satisfies(receipt_requirement), Ok(true));

    // Causal negatives: the marker generation and terminal extent are fields
    // of the requirement, not ambient state borrowed by another receipt.
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 2, Some(32))),
        Ok(false)
    );
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 3, Some(31))),
        Ok(false)
    );
}

#[test]
fn closure_can_cover_a_vacuous_or_short_prefix_without_covering_the_whole_stage() {
    let stream = key(7, 11, 13, 17);
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    accept_prefix(&mut frontiers, stream, FrontierStage::Authenticated, 2);

    // The caller retains this Copy declaration as its receipt field. A close at
    // two establishes a source endpoint, but no later stage has progressed yet.
    let receipt_marker = marker(stream, 2, 31);
    frontiers.record_close(receipt_marker).unwrap();

    // `through: 0` is a vacuous named prefix with a separately observed close,
    // not an assertion that this nonempty stream was empty.
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 0, Some(31))),
        Ok(true)
    );

    frontiers.accept(stream, FrontierStage::Judged, 1).unwrap();
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 1, Some(31))),
        Ok(true)
    );

    // The whole closed stream remains unavailable at this named stage until
    // that stage itself reaches the terminal sequence.
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 2, Some(31))),
        Ok(false)
    );
    frontiers.accept(stream, FrontierStage::Judged, 2).unwrap();
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 2, Some(31))),
        Ok(true)
    );
}

#[test]
fn missing_middle_refuses_terminal_marker_and_duplicates_do_not_advance_state() {
    let stream = key(7, 11, 13, 17);
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers
        .accept(stream, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers
        .accept(stream, FrontierStage::Authenticated, 3)
        .unwrap();

    let before_missing_middle = frontiers.clone();
    assert_eq!(
        frontiers.record_close(marker(stream, 3, 31)),
        Err(Error::Incomplete)
    );
    assert_eq!(frontiers, before_missing_middle);

    // The near positive fills precisely the omitted sequence. Replaying that
    // sequence is idempotent and cannot manufacture further frontier progress.
    frontiers
        .accept(stream, FrontierStage::Authenticated, 2)
        .unwrap();
    assert_eq!(
        frontiers.frontier(stream, FrontierStage::Authenticated),
        Ok(3)
    );
    let before_duplicate = frontiers.clone();
    frontiers
        .accept(stream, FrontierStage::Authenticated, 2)
        .unwrap();
    assert_eq!(frontiers, before_duplicate);

    frontiers.record_close(marker(stream, 3, 31)).unwrap();
    assert_eq!(
        frontiers.satisfies(requirement(
            stream,
            FrontierStage::Authenticated,
            3,
            Some(31),
        )),
        Ok(true)
    );
}

#[test]
fn a_close_marker_cannot_cross_source_branch_projection_or_epoch() {
    let primary = key(7, 11, 13, 17);
    let alternatives = [
        key(8, 11, 13, 17),
        key(7, 12, 13, 17),
        key(7, 11, 14, 17),
        key(7, 11, 13, 18),
    ];
    let mut frontiers = ProductFrontiers::new(5, 8).unwrap();

    accept_prefix(&mut frontiers, primary, FrontierStage::Authenticated, 2);
    accept_prefix(&mut frontiers, primary, FrontierStage::Judged, 2);
    frontiers.record_close(marker(primary, 2, 31)).unwrap();
    assert_eq!(
        frontiers.satisfies(requirement(primary, FrontierStage::Judged, 2, Some(31))),
        Ok(true)
    );

    for alternative in alternatives {
        accept_prefix(&mut frontiers, alternative, FrontierStage::Authenticated, 2);
        accept_prefix(&mut frontiers, alternative, FrontierStage::Judged, 2);

        // A complete matching prefix still cannot borrow another product key's
        // close marker.
        assert_eq!(
            frontiers.satisfies(requirement(alternative, FrontierStage::Judged, 2, Some(31),)),
            Ok(false)
        );

        // Near positive: its own exact marker establishes only this key.
        frontiers.record_close(marker(alternative, 2, 31)).unwrap();
        assert_eq!(
            frontiers.satisfies(requirement(alternative, FrontierStage::Judged, 2, Some(31),)),
            Ok(true)
        );
    }
}

#[test]
fn closed_stream_allows_late_lower_stage_completion_but_refuses_stale_and_future_changes() {
    let stream = key(7, 11, 13, 17);
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    accept_prefix(&mut frontiers, stream, FrontierStage::Authenticated, 2);

    // Generation zero is a malformed terminal declaration; it differs from
    // terminal sequence zero, which denotes a valid explicitly empty
    // projection. This is not an authenticity test: a syntactically valid
    // marker remains caller-trusted input at this reference boundary.
    let before_malformed_marker = frontiers.clone();
    assert_eq!(
        frontiers.record_close(marker(stream, 2, 0)),
        Err(Error::InvalidInput)
    );
    assert_eq!(frontiers, before_malformed_marker);

    // A stale terminal position is refused before close and leaves the open
    // product frontier untouched.
    let before_stale_marker = frontiers.clone();
    assert_eq!(
        frontiers.record_close(marker(stream, 1, 31)),
        Err(Error::InvalidInput)
    );
    assert_eq!(frontiers, before_stale_marker);

    frontiers.record_close(marker(stream, 2, 31)).unwrap();
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 2, Some(31))),
        Ok(false)
    );

    // The close freezes source extent, not every independent stage. A late
    // lower-stage sequence may arrive within that extent and become contiguous.
    frontiers.accept(stream, FrontierStage::Judged, 2).unwrap();
    assert_eq!(frontiers.frontier(stream, FrontierStage::Judged), Ok(0));
    frontiers.accept(stream, FrontierStage::Judged, 1).unwrap();
    assert_eq!(frontiers.frontier(stream, FrontierStage::Judged), Ok(2));
    assert_eq!(
        frontiers.satisfies(requirement(stream, FrontierStage::Judged, 2, Some(31))),
        Ok(true)
    );

    let before_future = frontiers.clone();
    assert_eq!(
        frontiers.accept(stream, FrontierStage::Captured, 3),
        Err(Error::WrongState)
    );
    assert_eq!(frontiers, before_future);

    let before_second_marker = frontiers.clone();
    assert_eq!(
        frontiers.record_close(marker(stream, 2, 32)),
        Err(Error::WrongState)
    );
    assert_eq!(frontiers, before_second_marker);
}
