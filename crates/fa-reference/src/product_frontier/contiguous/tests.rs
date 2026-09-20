use super::*;
use crate::product_frontier::{FrontierRequirement, TrustedClosingMarker};

fn key() -> ProjectionKey { ProjectionKey { source: 1, branch: 2, projection: 3, source_epoch: 4 } }
fn close(end: u64) -> TrustedClosingMarker {
    TrustedClosingMarker { key: key(), final_sequence: end, marker_generation: 7 }
}

#[test]
fn every_small_pending_pattern_matches_independent_single_position_ingestion() {
    for mask in 0_u64..128 {
        let mut initial = ProductFrontiers::new(1, 8).unwrap();
        for sequence in 1..=7 {
            if mask & (1 << (sequence - 1)) != 0 {
                initial.accept(key(), FrontierStage::Authenticated, sequence).unwrap();
            }
        }
        let prefix = initial.frontier(key(), FrontierStage::Authenticated).unwrap_or(0);
        for first in 1..=8 {
            for last in first..=8 {
                let mut batch = initial.clone();
                if first > prefix + 1 {
                    assert_eq!(batch.accept_contiguous(key(), FrontierStage::Authenticated, first, last), Err(Error::Incomplete));
                    assert_eq!(batch, initial);
                } else {
                    let mut oracle = initial.clone();
                    for position in first..=last { oracle.accept(key(), FrontierStage::Authenticated, position).unwrap(); }
                    batch.accept_contiguous(key(), FrontierStage::Authenticated, first, last).unwrap();
                    assert_eq!(batch, oracle, "mask={mask}, interval={first}..={last}");
                }
            }
        }
    }
}

#[test]
fn huge_prefix_and_maximum_sequence_need_neither_per_position_work_nor_overflow() {
    for end in [4_097, 1_u64 << 40, u64::MAX - 1, u64::MAX] {
        let mut state = ProductFrontiers::new(1, 1).unwrap();
        state.accept_contiguous(key(), FrontierStage::Authenticated, 1, end).unwrap();
        assert_eq!(state.frontier(key(), FrontierStage::Authenticated), Ok(end));
        assert_eq!(state.closing_marker(key()), None);
        state.record_close(close(end)).unwrap();
        assert_eq!(state.closing_marker(key()), Some(close(end)));
        let retained = state.clone();
        state.accept_contiguous(key(), FrontierStage::Authenticated, end, end).unwrap();
        assert_eq!(state, retained);
    }
}

#[test]
fn contiguous_batches_cannot_fill_a_missing_predecessor_or_invent_closure() {
    let mut state = ProductFrontiers::new(1, 8).unwrap();
    state.accept(key(), FrontierStage::Authenticated, 4).unwrap();
    let before = state.clone();
    assert_eq!(state.accept_contiguous(key(), FrontierStage::Authenticated, 2, 4), Err(Error::Incomplete));
    assert_eq!(state, before);
    state.accept_contiguous(key(), FrontierStage::Authenticated, 1, 2).unwrap();
    assert_eq!(state.frontier(key(), FrontierStage::Authenticated), Ok(2));
    assert_eq!(state.record_close(close(4)), Err(Error::Incomplete));
    state.accept_contiguous(key(), FrontierStage::Authenticated, 3, 3).unwrap();
    assert_eq!(state.frontier(key(), FrontierStage::Authenticated), Ok(4));
    let required = FrontierRequirement { key: key(), stage: FrontierStage::Authenticated, through: 4, closure: Some(7) };
    assert_eq!(state.satisfies(required), Ok(false));
    state.record_close(close(4)).unwrap();
    assert_eq!(state.satisfies(required), Ok(true));
}

#[test]
fn stage_projection_and_terminal_limits_remain_independent() {
    let mut state = ProductFrontiers::new(2, 1).unwrap();
    state.accept_contiguous(key(), FrontierStage::Captured, 1, 100).unwrap();
    assert_eq!(state.record_close(close(100)), Err(Error::Incomplete));
    state.accept_contiguous(key(), FrontierStage::Authenticated, 1, 100).unwrap();
    state.record_close(close(100)).unwrap();
    assert_eq!(state.frontier(key(), FrontierStage::Judged), Ok(0));
    let before = state.clone();
    assert_eq!(state.accept_contiguous(key(), FrontierStage::Presented, 1, 101), Err(Error::WrongState));
    assert_eq!(state, before);
    let other = ProjectionKey { source_epoch: 5, ..key() };
    state.accept_contiguous(other, FrontierStage::Judged, 1, 100).unwrap();
    assert_eq!(state.closing_marker(other), None);
    assert_eq!(state.frontier(other, FrontierStage::Authenticated), Ok(0));
}

#[test]
fn empty_close_invalid_intervals_and_full_capacity_refuse_without_mutation() {
    let mut state = ProductFrontiers::new(1, 1).unwrap();
    for (first, last) in [(0, 0), (0, 1), (2, 1)] {
        let before = state.clone();
        assert_eq!(state.accept_contiguous(key(), FrontierStage::Authenticated, first, last), Err(Error::InvalidInput));
        assert_eq!(state, before);
    }
    state.record_close(close(0)).unwrap();
    let before = state.clone();
    assert_eq!(state.accept_contiguous(key(), FrontierStage::Authenticated, 1, 1), Err(Error::WrongState));
    assert_eq!(state.accept_contiguous(ProjectionKey { branch: 9, ..key() }, FrontierStage::Authenticated, 1, 1), Err(Error::Limit));
    assert_eq!(state, before);
}

#[test]
fn a_batch_does_not_relax_the_original_out_of_order_gap_window() {
    let mut state = ProductFrontiers::new(1, 2).unwrap();
    state.accept_contiguous(key(), FrontierStage::Authenticated, 1, 1_000_000).unwrap();
    let before = state.clone();
    assert_eq!(state.accept(key(), FrontierStage::Authenticated, 1_000_003), Err(Error::Limit));
    assert_eq!(state, before);
    state.accept(key(), FrontierStage::Authenticated, 1_000_002).unwrap();
    state.accept_contiguous(key(), FrontierStage::Authenticated, 1_000_001, 1_000_001).unwrap();
    assert_eq!(state.frontier(key(), FrontierStage::Authenticated), Ok(1_000_002));
}
