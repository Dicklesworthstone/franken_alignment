//! Cut arithmetic unit tests; real broker/journal coverage is separate.
use super::*;
use super::super::PublicationSourceStatus;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};

fn cut(through: u64) -> PublicationInputCut { PublicationInputCut { source: 41, through } }
fn feed(through: u64) -> PublicationChangeStatus {
    PublicationChangeStatus { source: 41, through, observed_through: through, unavailable: false }
}
fn source(last: u64, required: u64) -> SourceState {
    SourceState { status: PublicationSourceStatus { source: 91, generation: 8, capture_pending: true, fresh: false },
        last: PublicationInputs { structured: None, opaque: None },
        input_cut: Some(PublicationInputCutStatus { last: cut(last), required_through: required }) }
}
fn slot() -> Slot {
    let action = FrozenAction::freeze(ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0, deadline: ElapsedTick(100), units: 16,
    }).unwrap();
    Slot { action, judgment: None, revision: 0, current: None, floor: None, last: None, source: Some(source(0, 0)) }
}

#[test]
fn exhaustive_prefix_neighbors_require_both_monotonicity_and_covered_changes() {
    for last in 0..6 {
        for required in 0..6 {
            for supplied in 0..6 {
                for through in 0..6 {
                    let state = source(last, required);
                    let expected = if supplied < last || supplied < required { Err(Error::Stale) }
                        else if supplied > through { Err(Error::Incomplete) } else { Ok(()) };
                    assert_eq!(state.check_input_cut(Some(cut(supplied)), 9, Some(feed(through))), expected);
                    assert_eq!(state.input_cut.unwrap().last, cut(last));
                }
            }
        }
    }
}

#[test]
fn incremented_image_generation_cannot_stand_in_for_catching_up_the_snapshot() {
    let state = source(2, 5);
    for generation in [9, 10, u64::MAX] {
        assert_eq!(state.check_input_cut(Some(cut(2)), generation, Some(feed(5))), Err(Error::Stale));
        assert_eq!(state.check_input_cut(Some(cut(5)), generation, Some(feed(5))), Ok(()));
    }
}

#[test]
fn same_generation_binds_the_cut_even_when_payload_bytes_are_equal() {
    let state = source(2, 2);
    assert_eq!(state.check_input_cut(Some(cut(2)), 8, Some(feed(3))), Ok(()));
    assert_eq!(state.check_input_cut(Some(cut(3)), 8, Some(feed(3))), Err(Error::Binding));
    assert_eq!(state.check_input_cut(Some(cut(3)), 9, Some(feed(3))), Ok(()));
}

#[test]
fn missing_coverage_and_foreign_feed_cannot_activate_a_producer_image() {
    let state = source(2, 2);
    assert_eq!(state.check_input_cut(None, 9, Some(feed(2))), Err(Error::Incomplete));
    assert_eq!(state.check_input_cut(Some(cut(2)), 9, None), Err(Error::Incomplete));
    for incomplete in [PublicationChangeStatus { observed_through: 3, ..feed(2) },
        PublicationChangeStatus { unavailable: true, ..feed(2) }] {
        assert_eq!(state.check_input_cut(Some(cut(2)), 9, Some(incomplete)), Err(Error::Incomplete));
    }
    assert_eq!(state.check_input_cut(Some(PublicationInputCut { source: 42, through: 2 }), 9, Some(feed(2))), Err(Error::Binding));
    assert_eq!(state.check_input_cut(Some(cut(2)), 9, Some(PublicationChangeStatus { source: 42, ..feed(2) })), Err(Error::Binding));
    assert_eq!(PublicationInputCut { source: 0, through: 0 }.check(), Err(Error::InvalidInput));
}

#[test]
fn legacy_source_does_not_silently_upgrade_or_accept_a_cut_from_another_profile() {
    let mut state = source(0, 0); state.input_cut = None;
    assert_eq!(state.check_input_cut(None, 9, None), Ok(()));
    assert_eq!(state.check_input_cut(Some(cut(0)), 9, Some(feed(0))), Err(Error::Binding));
}

#[test]
fn withdrawal_keeps_cut_floors_and_a_stale_fresh_bit_cannot_bypass_consumption() {
    let mut slot = slot();
    slot.require_capture_through(7); slot.require_capture_through(3);
    slot.record_inputs(0, None).unwrap();
    assert_eq!(slot.source.as_ref().unwrap().input_cut.unwrap().required_through, 7);
    slot.source.as_mut().unwrap().status.fresh = true;
    assert!(!slot.consume_capture());
    let state = slot.source.as_mut().unwrap();
    assert!(!state.status.fresh); assert!(!state.status.capture_pending);
    state.input_cut.as_mut().unwrap().last = cut(7); state.status.fresh = true;
    assert!(slot.consume_capture()); assert!(!slot.consume_capture());
}

#[test]
fn maximum_sequence_needs_no_increment_or_saturating_fallback() {
    let state = source(u64::MAX - 1, u64::MAX);
    assert_eq!(state.check_input_cut(Some(cut(u64::MAX)), 9, Some(feed(u64::MAX))), Ok(()));
    assert_eq!(state.check_input_cut(Some(cut(u64::MAX - 1)), 9, Some(feed(u64::MAX))), Err(Error::Stale));
    let mut slot = slot(); slot.require_capture_through(u64::MAX); slot.require_capture_through(0);
    assert_eq!(slot.source.unwrap().input_cut.unwrap().required_through, u64::MAX);
}
