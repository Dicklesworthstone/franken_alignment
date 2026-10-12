//! Cooperative preparation versus the ORIGINAL single-record framing oracle.
use fa_journal::{Error, Event, Identity, Journal, Limits, RecoveryReserve};
use fa_journal::batch::{BatchLimits, BatchPreparationError as Failure, BatchPreparationStage as Stage,
    MAX_BATCH_ENCODED_BYTES, MAX_BATCH_RECORDS};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;

fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(),
        payload: vec![0, 255, 10], units: 20 }
}
fn initial() -> Journal {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    journal.append(&Event::Reserve { id: 1, effect: effect() }).unwrap();
    journal
}
fn group() -> Vec<Event> {
    vec![Event::Reserve { id: 2, effect: effect() }, Event::Dispatch { id: 1, effect: effect() },
        Event::MarkUnknown { id: 1 }, Event::AbortBeforeDispatch { id: 2 },
        Event::ResetToCheckpoint { keep: BTreeSet::from([1]) }, Event::RevokeEpoch,
        Event::Reconcile { id: 1, occurred: true }]
}
fn same(actual: &Journal, expected: &Journal) {
    assert_eq!(actual.identity(), expected.identity()); assert_eq!(actual.head(), expected.head());
    assert_eq!(actual.encoded_len(), expected.encoded_len()); assert_eq!(actual.rights(), expected.rights());
    assert_eq!(actual.capacity(), expected.capacity()); assert!(actual.rights().conserved());
}

#[test]
fn every_preparation_partition_preserves_original_bytes_and_accounting() {
    let original = initial(); let events = group();
    let mut expected = original.clone(); let mut wire = Vec::new();
    for event in &events { wire.extend(expected.append(event).unwrap()); }
    let mut sync = original.clone();
    assert_eq!(sync.append_batch(&events, BatchLimits::default()).unwrap(), wire);
    for cuts in 0..1usize << (events.len() - 1) {
        let mut run = original.begin_append_batch(events.clone(), BatchLimits::default()).unwrap();
        let mut start = 0;
        for end in 1..=events.len() {
            if end == events.len() || cuts & (1 << (end - 1)) != 0 {
                let report = run.advance(end - start).unwrap();
                assert_eq!(report.prepared_records, end); assert_eq!(report.attempted_records, end);
                start = end;
            }
        }
        assert_eq!(run.progress().stage, Stage::Ready);
        let (actual, bytes) = run.finish().unwrap(); assert_eq!(bytes, wire); same(&actual, &expected);
    }
    same(&original, &initial()); same(&sync, &expected);
}

#[test]
fn zero_and_terminal_polls_do_no_work_and_cancellation_cannot_change_the_source() {
    let original = initial();
    for stop_at in 0..=group().len() {
        let mut run = original.begin_append_batch(group(), BatchLimits::default()).unwrap();
        let before = run.progress(); assert_eq!(run.advance(0).unwrap(), before);
        let report = run.advance(stop_at).unwrap();
        assert_eq!(report.prepared_records, stop_at);
        if report.stage == Stage::Ready {
            assert_eq!(run.advance(usize::MAX).unwrap(), report);
            assert_eq!(run.advance(0).unwrap(), report);
        }
        assert_eq!(run.cancel(), report); same(&original, &initial());
    }
    let run = original.begin_append_batch(group(), BatchLimits::default()).unwrap();
    assert!(matches!(run.finish(), Err(Failure::NotReady))); same(&original, &initial());
}

#[test]
fn late_reducer_failures_latch_without_exposing_a_valid_prefix() {
    let original = initial(); let valid = group();
    for end in 0..valid.len() {
        let mut events = valid[..end].to_vec();
        events.push(Event::Reserve { id: 1, effect: effect() }); // Original ID remains consumed.
        let mut run = original.begin_append_batch(events, BatchLimits::default()).unwrap();
        run.advance(end).unwrap();
        assert!(matches!(run.advance(1), Err(Failure::Journal(Error::Model(_)))));
        assert_eq!(run.progress().stage, Stage::Failed);
        assert_eq!(run.progress().prepared_records, end);
        assert_eq!(run.progress().attempted_records, end + 1);
        assert!(matches!(run.advance(usize::MAX), Err(Failure::Failed)));
        assert!(matches!(run.finish(), Err(Failure::Failed))); same(&original, &initial());
    }
}

#[test]
fn exact_group_limits_work_and_one_less_never_exports_candidate_bytes() {
    let original = initial(); let events = group(); let mut oracle = original.clone();
    let bytes = oracle.append_batch(&events, BatchLimits::default()).unwrap();
    for enough in [false, true] {
        let limit = BatchLimits { max_records: events.len(), max_encoded_bytes: bytes.len() - usize::from(!enough) };
        let mut run = original.begin_append_batch(events.clone(), limit).unwrap();
        let result = run.advance(usize::MAX);
        if enough { result.unwrap(); let (state, actual) = run.finish().unwrap();
            same(&state, &oracle); assert_eq!(actual, bytes); }
        else { assert!(matches!(result, Err(Failure::Journal(Error::Limit))));
            assert!(matches!(run.finish(), Err(Failure::Failed))); }
        same(&original, &initial());
    }
    assert!(matches!(original.begin_append_batch(events.clone(), BatchLimits {
        max_records: events.len() - 1, ..BatchLimits::default() }), Err(Error::Limit)));
}

#[test]
fn invalid_whole_group_bounds_refuse_before_any_preparation() {
    let original = initial();
    assert!(matches!(original.begin_append_batch(Vec::new(), BatchLimits::default()), Err(Error::InvalidInput)));
    for limits in [BatchLimits { max_records: 0, ..BatchLimits::default() },
        BatchLimits { max_records: MAX_BATCH_RECORDS + 1, ..BatchLimits::default() },
        BatchLimits { max_encoded_bytes: 0, ..BatchLimits::default() },
        BatchLimits { max_encoded_bytes: MAX_BATCH_ENCODED_BYTES + 1, ..BatchLimits::default() },
        BatchLimits { max_encoded_bytes: 20, ..BatchLimits::default() }] {
        assert!(matches!(original.begin_append_batch(vec![Event::RevokeEpoch], limits), Err(Error::Limit)));
    }
    let lifetime = Journal::new(identity(), Limits { max_records: 1, ..Limits::default() }).unwrap();
    assert!(matches!(lifetime.begin_append_batch(vec![Event::RevokeEpoch; 2], BatchLimits::default()), Err(Error::Limit)));
    same(&original, &initial());
}

#[test]
fn reserved_tail_survives_yields_without_refilling_on_nonexecution() {
    let reserve = RecoveryReserve { records: 3, bytes: 80 }; // Unknown + revoke + reconcile.
    let limits = Limits { max_records: 5, ..Limits::default() };
    let mut original = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    original.append(&Event::Reserve { id: 1, effect: effect() }).unwrap();
    original.append(&Event::Dispatch { id: 1, effect: effect() }).unwrap();
    assert_eq!(original.capacity().ordinary.records, 0);
    let events = vec![Event::MarkUnknown { id: 1 }, Event::RevokeEpoch,
        Event::Reconcile { id: 1, occurred: false }];
    let mut run = original.begin_append_batch(events.clone(), BatchLimits::default()).unwrap();
    for _ in &events { run.advance(1).unwrap(); }
    let (finished, bytes) = run.finish().unwrap(); let mut expected = original.clone();
    assert_eq!(bytes, expected.append_batch(&events, BatchLimits::default()).unwrap());
    same(&finished, &expected); assert_eq!(finished.rights().available(), 100);
    assert_eq!(finished.capacity().ordinary.records, 0); assert_eq!(finished.capacity().recovery.records, 0);
    assert_eq!(original.rights().state(1), Ok(State::Dispatched));
    let mut forbidden = original.begin_append_batch(vec![Event::Reserve { id: 2, effect: effect() }],
        BatchLimits::default()).unwrap();
    assert!(matches!(forbidden.advance(1), Err(Failure::Journal(Error::Limit))));
}

#[test]
fn original_order_and_exact_effect_binding_remain_mandatory() {
    for mutation in 0..3 {
        let original = initial(); let mut effect = effect(); let mut events = Vec::new();
        match mutation { 0 => events.push(Event::RevokeEpoch),
            1 => effect.payload.push(42), _ => effect.resolved_target.push_str("/other") }
        events.push(Event::Dispatch { id: 1, effect });
        let mut run = original.begin_append_batch(events, BatchLimits::default()).unwrap();
        assert!(run.advance(usize::MAX).is_err()); same(&original, &initial());
    }
    let mut run = initial().begin_append_batch(vec![Event::Dispatch { id: 1, effect: effect() },
        Event::RevokeEpoch], BatchLimits::default()).unwrap();
    run.advance(usize::MAX).unwrap();
    assert_eq!(run.finish().unwrap().0.rights().state(1), Ok(State::Dispatched));
}

#[test]
fn encoding_failure_counts_once_and_cannot_be_resumed_with_a_larger_poll() {
    let original = initial(); let mut oversized = effect(); oversized.payload = vec![0; 1024];
    let mut run = original.begin_append_batch(vec![Event::RevokeEpoch,
        Event::Reserve { id: 2, effect: oversized }], BatchLimits { max_encoded_bytes: 128,
            ..BatchLimits::default() }).unwrap();
    assert!(matches!(run.advance(2), Err(Failure::Journal(Error::Limit))));
    let failure = run.progress(); assert_eq!(failure.prepared_records, 1); assert_eq!(failure.attempted_records, 2);
    assert!(matches!(run.advance(usize::MAX), Err(Failure::Failed)));
    assert_eq!(run.progress(), failure); same(&original, &initial());
}
