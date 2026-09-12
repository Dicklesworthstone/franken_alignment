//! Complete-prefix freshness laws; trusted logical time, not physical capture proof.
use fa_reference::action::{ElapsedTick, Purpose, Scope};
use fa_reference::action::consequence::oversight::policy_state::{
    PolicyStateCapture, PolicyStateWriter, StateChange, StateEvent, StateFreshness, StateLimits, StateSource,
};
use fa_reference::Error;
use std::collections::BTreeMap;

fn source() -> StateSource {
    StateSource { source: 9, generation: 1,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
}
fn image() -> StateEvent {
    StateEvent::Snapshot { semantic_epoch: 1, values: BTreeMap::from([(7, vec![9])]) }
}
fn fresh(age: u64) -> (PolicyStateCapture, PolicyStateWriter) {
    PolicyStateCapture::new_with_freshness(source(), StateLimits::default(), StateFreshness::new(age).unwrap()).unwrap()
}

#[test]
fn unchanged_healthy_observations_renew_but_silence_expires_at_the_exact_tick() {
    let (capture, writer) = fresh(4);
    writer.record(1, &image()).unwrap();
    writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    let first = capture.capture_at(ElapsedTick(1)).unwrap();
    assert_eq!(first.lease().unwrap().expires_at(), ElapsedTick(5));
    assert_eq!(capture.capture_at(ElapsedTick(4)).unwrap(), first);
    assert_eq!(capture.capture_at(ElapsedTick(5)), Err(Error::Stale));
    writer.record(2, &image()).unwrap();
    writer.close_observed(2, 2, ElapsedTick(5)).unwrap();
    let second = capture.validate_at(first.snapshot(), ElapsedTick(5)).unwrap();
    assert_eq!(second.snapshot(), first.snapshot());
    assert_ne!(second.frontier(), first.frontier());
    assert_eq!(second.lease().unwrap().expires_at(), ElapsedTick(9));
    assert_eq!(first.lease().unwrap().validate_at(ElapsedTick(5)), Err(Error::Stale));
}

#[test]
fn old_data_and_a_new_timestamp_or_marker_cannot_renew_a_closed_cut() {
    let (capture, writer) = fresh(4);
    writer.record(1, &image()).unwrap();
    let frontier = writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    assert!(!writer.record(1, &image()).unwrap());
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(1)), Ok(frontier));
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(2)), Err(Error::Binding));
    assert_eq!(writer.close_observed(1, 2, ElapsedTick(2)), Err(Error::Stale));
    assert_eq!(writer.close(1, 2), Err(Error::Incomplete));
    assert_eq!(capture.capture(), Err(Error::Incomplete));
    assert_eq!(capture.validate(&fa_reference::Snapshot::default()), Err(Error::Incomplete));
    assert_eq!(capture.capture_at(ElapsedTick(5)), Err(Error::Stale));
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(1)), Ok(frontier));
    assert_eq!(capture.capture_at(ElapsedTick(5)), Err(Error::Stale));
}

#[test]
fn future_observation_and_consumer_clock_rollback_are_not_freshness() {
    let (capture, writer) = fresh(4);
    writer.record(1, &image()).unwrap();
    writer.close_observed(1, 1, ElapsedTick(10)).unwrap();
    assert_eq!(capture.capture_at(ElapsedTick(9)), Err(Error::Stale));
    assert!(capture.capture_at(ElapsedTick(10)).is_ok());
    assert_eq!(capture.capture_at(ElapsedTick(14)), Err(Error::Stale));
    assert_eq!(capture.capture_at(ElapsedTick(11)), Err(Error::Stale));
    writer.record(2, &image()).unwrap();
    assert_eq!(writer.close_observed(2, 2, ElapsedTick(9)), Err(Error::Stale));
    writer.close_observed(2, 2, ElapsedTick(14)).unwrap();
    assert_eq!(capture.capture_at(ElapsedTick(13)), Err(Error::Stale));
    assert!(capture.capture_at(ElapsedTick(14)).is_ok());
}

#[test]
fn new_frames_withdraw_leases_and_clock_checks_do_not_fill_sequence_gaps() {
    let (capture, writer) = fresh(20);
    writer.record(1, &image()).unwrap();
    writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    writer.record(3, &image()).unwrap();
    assert_eq!(capture.capture_at(ElapsedTick(2)), Err(Error::Incomplete));
    assert_eq!(writer.close_observed(3, 2, ElapsedTick(2)), Err(Error::Incomplete));
    writer.record(2, &image()).unwrap();
    writer.close_observed(3, 2, ElapsedTick(2)).unwrap();
    assert_eq!(capture.capture_at(ElapsedTick(2)).unwrap().frontier().through, 3);
}

#[test]
fn loss_requires_a_new_full_image_and_faults_cannot_be_renewed_away() {
    let (capture, writer) = fresh(20);
    writer.record(1, &image()).unwrap();
    writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    writer.withdraw();
    writer.record(2, &StateEvent::Delta { semantic_epoch: 1,
        changes: vec![StateChange { key: 7, before: Some(vec![9]), after: Some(vec![9]) }] }).unwrap();
    assert_eq!(writer.close_observed(2, 2, ElapsedTick(2)), Err(Error::Incomplete));
    writer.record(3, &image()).unwrap();
    writer.close_observed(3, 2, ElapsedTick(2)).unwrap();
    assert!(capture.capture_at(ElapsedTick(2)).is_ok());
    assert_eq!(writer.record(3, &StateEvent::Snapshot { semantic_epoch: 1, values: BTreeMap::new() }), Err(Error::Binding));
    assert_eq!(writer.close_observed(3, 3, ElapsedTick(3)), Err(Error::WrongState));
    assert_eq!(capture.capture_at(ElapsedTick(3)), Err(Error::Binding));
}

#[test]
fn writer_loss_keeps_history_but_not_live_eligibility() {
    let (capture, writer) = fresh(20);
    writer.record(1, &image()).unwrap();
    writer.close_observed(1, 1, ElapsedTick(1)).unwrap();
    let historical = capture.capture_at(ElapsedTick(1)).unwrap();
    drop(writer);
    assert_eq!(capture.capture_at(ElapsedTick(2)), Err(Error::Incomplete));
    assert_eq!(historical.snapshot().values[&7], vec![9]);
    assert_eq!(historical.lease().unwrap().expires_at(), ElapsedTick(21));
}

#[test]
fn overflow_never_becomes_an_unbounded_or_wrapped_lease() {
    assert_eq!(StateFreshness::new(0), Err(Error::InvalidInput));
    let (capture, writer) = fresh(4);
    writer.record(1, &image()).unwrap();
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(u64::MAX - 3)), Err(Error::Overflow));
    assert_eq!(capture.capture_at(ElapsedTick(u64::MAX - 4)), Err(Error::Incomplete));
    writer.close_observed(1, 1, ElapsedTick(u64::MAX - 4)).unwrap();
    assert!(capture.capture_at(ElapsedTick(u64::MAX - 1)).is_ok());
    assert_eq!(capture.capture_at(ElapsedTick(u64::MAX)), Err(Error::Stale));
}

#[test]
fn explicitly_untimed_capture_remains_untimed() {
    let (capture, writer) = PolicyStateCapture::new(source(), StateLimits::default()).unwrap();
    writer.record(1, &image()).unwrap();
    assert_eq!(writer.close_observed(1, 1, ElapsedTick(1)), Err(Error::Incomplete));
    writer.close(1, 1).unwrap();
    let cut = capture.capture().unwrap();
    assert_eq!(cut.lease(), None);
    assert_eq!(capture.capture_at(ElapsedTick(u64::MAX)).unwrap(), cut);
}
