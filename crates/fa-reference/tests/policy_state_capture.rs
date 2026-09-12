//! Public stream-to-snapshot controls; the producer remains host-trusted.
use fa_reference::action::{Purpose, Scope};
use fa_reference::action::consequence::oversight::policy_state::*;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn source() -> StateSource {
    StateSource { scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5,
        purpose: Purpose::Effect }, source: 7, generation: 1 }
}
fn image(epoch: u64, pairs: &[(u64, &[u8])]) -> StateEvent {
    StateEvent::Snapshot { semantic_epoch: epoch,
        values: pairs.iter().map(|(key, value)| (*key, value.to_vec())).collect() }
}
fn delta(key: u64, before: Option<&[u8]>, after: Option<&[u8]>) -> StateEvent {
    StateEvent::Delta { semantic_epoch: 1, changes: vec![StateChange {
        key, before: before.map(<[u8]>::to_vec), after: after.map(<[u8]>::to_vec),
    }] }
}
fn seeded(limits: StateLimits) -> (PolicyStateCapture, PolicyStateWriter) {
    let (capture, writer) = PolicyStateCapture::new(source(), limits).unwrap();
    writer.record(1, &image(1, &[(7, b"a")])).unwrap();
    writer.close(1, 1).unwrap();
    (capture, writer)
}

#[test]
fn absence_requires_explicit_closure_and_complete_exact_snapshot_equality() {
    let (capture, writer) = PolicyStateCapture::new(source(), StateLimits::default()).unwrap();
    assert_eq!(capture.capture(), Err(Error::Incomplete));
    assert_eq!(writer.close(1, 1), Err(Error::Incomplete));
    writer.record(1, &image(0, &[])).unwrap();
    assert_eq!(capture.capture(), Err(Error::Incomplete));
    let frontier = writer.close(1, 1).unwrap();
    let cut = capture.capture().unwrap();
    assert_eq!(cut.frontier(), frontier);
    assert_eq!(cut.snapshot(), &Snapshot { semantic_epoch: 0, complete: true, values: BTreeMap::new() });
    capture.validate(cut.snapshot()).unwrap();
    for field in 0..3 {
        let mut wrong = cut.snapshot().clone();
        match field { 0 => wrong.complete = false, 1 => wrong.semantic_epoch = 1,
            _ => { wrong.values.insert(99, b"injected".to_vec()); } }
        assert_eq!(capture.validate(&wrong), Err(Error::Binding));
    }
    assert_eq!(writer.close(1, 1).unwrap(), frontier);
}

#[test]
fn a_missing_middle_and_every_unclosed_tail_invalidate_live_eligibility() {
    let (capture, writer) = seeded(StateLimits::default());
    let old = capture.capture().unwrap();
    writer.record(3, &delta(8, None, Some(b"c"))).unwrap();
    assert_eq!(capture.status().first_missing, Some(2));
    assert_eq!(capture.validate(old.snapshot()), Err(Error::Incomplete));
    assert_eq!(writer.close(1, 2), Err(Error::Incomplete));
    assert_eq!(writer.close(3, 2), Err(Error::Incomplete));
    writer.record(2, &delta(7, Some(b"a"), Some(b"b"))).unwrap();
    assert_eq!(capture.status().applied_through, 3);
    assert_eq!(capture.capture(), Err(Error::Incomplete));
    writer.close(3, 2).unwrap();
    assert_eq!(capture.capture().unwrap().snapshot().values,
        BTreeMap::from([(7, b"b".to_vec()), (8, b"c".to_vec())]));
    assert_eq!(capture.validate(old.snapshot()), Err(Error::Binding));
    assert_eq!(old.snapshot().values[&7], b"a");
}

#[test]
fn exact_retries_do_not_reopen_the_prefix_but_equivocation_poisoning_is_sticky() {
    let (capture, writer) = seeded(StateLimits::default());
    let cut = capture.capture().unwrap();
    let status = capture.status();
    assert!(!writer.record(1, &image(1, &[(7, b"a")])).unwrap());
    assert_eq!(capture.status(), status);
    assert_eq!(writer.record(1, &image(1, &[(7, b"b")])), Err(Error::Binding));
    assert_eq!(capture.validate(cut.snapshot()), Err(Error::Binding));
    assert_eq!(writer.record(1, &image(1, &[(7, b"a")])), Err(Error::WrongState));
    assert_eq!(writer.close(1, 2), Err(Error::WrongState));
}

#[test]
fn transactions_check_all_preimages_and_apply_final_capacity_atomically() {
    let (capture, writer) = PolicyStateCapture::new(source(), StateLimits::default()).unwrap();
    let values: BTreeMap<_, _> = (0..MAX_STATE_ENTRIES as u64).map(|key| (key, vec![1])).collect();
    writer.record(1, &StateEvent::Snapshot { semantic_epoch: 1, values }).unwrap();
    writer.close(1, 1).unwrap();
    let transaction = StateEvent::Delta { semantic_epoch: 1, changes: vec![
        StateChange { key: 1_000, before: None, after: Some(vec![2]) },
        StateChange { key: 0, before: Some(vec![1]), after: None },
    ] };
    writer.record(2, &transaction).unwrap();
    writer.close(2, 2).unwrap();
    let cut = capture.capture().unwrap();
    assert_eq!(cut.snapshot().values.len(), MAX_STATE_ENTRIES);
    assert_eq!(cut.snapshot().values.get(&0), None);
    assert_eq!(cut.snapshot().values[&1_000], vec![2]);
    let invalid = StateEvent::Delta { semantic_epoch: 1, changes: vec![
        StateChange { key: 1, before: Some(vec![1]), after: Some(vec![3]) },
        StateChange { key: 2, before: Some(vec![9]), after: None },
    ] };
    assert_eq!(writer.record(3, &invalid), Err(Error::Binding));
    assert_eq!(capture.status().applied_through, 2);
    assert_eq!(cut.snapshot().values[&1], vec![1]);
    assert_eq!(capture.capture(), Err(Error::Binding));
}

#[test]
fn observation_loss_requires_a_new_full_image_not_a_new_label_on_old_deltas() {
    let (capture, writer) = seeded(StateLimits::default());
    writer.withdraw();
    assert_eq!(writer.close(1, 2), Err(Error::Incomplete));
    writer.record(2, &delta(7, Some(b"a"), Some(b"b"))).unwrap();
    assert_eq!(writer.close(2, 2), Err(Error::Incomplete));
    writer.record(3, &image(1, &[])).unwrap();
    assert_eq!(writer.close(3, 1), Err(Error::Stale));
    writer.close(3, 2).unwrap();
    assert!(capture.capture().unwrap().snapshot().values.is_empty());
    assert!(!capture.status().requires_full_snapshot);
}

#[test]
fn semantic_epoch_changes_require_full_capture_and_can_never_move_backwards() {
    let (capture, writer) = seeded(StateLimits::default());
    let old = capture.capture().unwrap();
    writer.record(2, &image(u64::MAX, &[(7, b"a")])).unwrap();
    writer.close(2, 2).unwrap();
    assert_eq!(capture.validate(old.snapshot()), Err(Error::Binding));
    assert_eq!(capture.capture().unwrap().snapshot().semantic_epoch, u64::MAX);
    assert_eq!(writer.record(3, &delta(7, Some(b"a"), None)), Err(Error::Stale));
    assert_eq!(capture.capture(), Err(Error::Stale));
    let (capture, writer) = seeded(StateLimits::default());
    assert_eq!(writer.record(2, &image(0, &[])), Err(Error::Stale));
    assert_eq!(capture.status().applied_through, 1);
}

#[test]
fn source_bounds_never_leave_the_previous_snapshot_eligible_after_dropped_input() {
    let (capture, writer) = seeded(StateLimits { events: 2, retained_bytes: 3 });
    writer.record(2, &delta(7, Some(b"a"), Some(b"b"))).unwrap();
    writer.close(2, 2).unwrap();
    assert_eq!(capture.status().retained_bytes, 3);
    assert_eq!(writer.record(3, &image(1, &[])), Err(Error::Limit));
    assert_eq!(capture.capture(), Err(Error::Limit));
    let (capture, writer) = seeded(StateLimits { events: 3, retained_bytes: 2 });
    assert_eq!(writer.record(2, &delta(7, Some(b"a"), Some(b"b"))), Err(Error::Limit));
    assert_eq!(capture.status().retained_bytes, 1);
    assert_eq!(capture.capture(), Err(Error::Limit));
    for sequence in [0, u64::MAX, MAX_STATE_PENDING + 2] {
        let (capture, writer) = seeded(StateLimits::default());
        assert!(writer.record(sequence, &image(1, &[])).is_err());
        assert!(capture.capture().is_err());
    }
}

#[test]
fn every_four_event_arrival_permutation_reconstructs_the_same_independent_state() {
    let events = [delta(7, Some(b"a"), Some(b"b")), delta(8, None, Some(b"c")),
        delta(7, Some(b"b"), None), delta(9, None, Some(b"d"))];
    let mut cases = 0;
    for a in 0..4 { for b in 0..4 { for c in 0..4 { for d in 0..4 {
        let order = [a, b, c, d];
        if order.iter().copied().collect::<std::collections::BTreeSet<_>>().len() != 4 { continue; }
        let (capture, writer) = seeded(StateLimits::default());
        for index in order {
            writer.record(index as u64 + 2, &events[index]).unwrap();
            assert_eq!(capture.capture(), Err(Error::Incomplete));
        }
        writer.close(5, 2).unwrap();
        assert_eq!(capture.capture().unwrap().snapshot().values,
            BTreeMap::from([(8, b"c".to_vec()), (9, b"d".to_vec())]));
        for (i, event) in events.iter().enumerate() { assert_eq!(capture.event(i as u64 + 2).as_ref(), Some(event)); }
        cases += 1;
    } } } }
    assert_eq!(cases, 24);
}

#[test]
fn dropping_the_only_writer_does_not_turn_a_historical_capture_into_live_state() {
    let (capture, writer) = seeded(StateLimits::default());
    let historical = capture.capture().unwrap();
    drop(writer);
    assert_eq!(capture.validate(historical.snapshot()), Err(Error::Incomplete));
    assert!(!capture.status().writer_live);
    assert_eq!(historical.snapshot().values[&7], b"a");
}
