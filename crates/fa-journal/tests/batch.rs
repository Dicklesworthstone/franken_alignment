//! Grouped accounting must equal ORIGINAL single-record framing and reduction.
use fa_journal::{Error, Event, Identity, Journal, Limits};
use fa_journal::batch::{BatchLimits, MAX_BATCH_ENCODED_BYTES, MAX_BATCH_RECORDS};
use fa_journal::replay::{ReplayBudget, ReplayStatus};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;

fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect(units: u64) -> Effect {
    Effect { principal: "αlice".into(), resolved_target: "ledger://one".into(),
        payload: vec![0, 255, 10, 13, 0], units }
}
fn events() -> Vec<Event> {
    vec![
        Event::Reserve { id: 1, effect: effect(30) },
        Event::Reserve { id: 2, effect: effect(20) },
        Event::Reserve { id: 3, effect: effect(10) },
        Event::Dispatch { id: 1, effect: effect(30) },
        Event::MarkUnknown { id: 1 },
        Event::AbortBeforeDispatch { id: 3 },
        Event::ResetToCheckpoint { keep: BTreeSet::from([1]) },
        Event::RevokeEpoch,
        // A reference observation, not an external receipt authenticated here.
        Event::Reconcile { id: 1, occurred: true },
        Event::Reserve { id: 4, effect: effect(40) },
        Event::Dispatch { id: 4, effect: effect(40) },
        Event::Reconcile { id: 4, occurred: false },
    ]
}
fn same(actual: &Journal, expected: &Journal) {
    assert_eq!(actual.identity(), expected.identity());
    assert_eq!(actual.head(), expected.head());
    assert_eq!(actual.encoded_len(), expected.encoded_len());
    assert_eq!(actual.rights(), expected.rights());
    assert!(actual.rights().conserved());
}

#[test]
fn every_group_partition_preserves_bytes_heads_rights_and_both_replayers() {
    let events = events();
    let mut oracle = Journal::new(identity(), Limits::default()).unwrap();
    let genesis = oracle.head();
    let mut canonical = oracle.header();
    for event in &events { canonical.extend(oracle.append(event).unwrap()); }
    // Every one of 2^(12-1) contiguous partitions of the SAME ordered history.
    for cuts in 0..(1usize << (events.len() - 1)) {
        let mut journal = Journal::new(identity(), Limits::default()).unwrap();
        let mut actual = journal.header();
        let mut start = 0;
        for end in 1..=events.len() {
            if end == events.len() || cuts & (1 << (end - 1)) != 0 {
                actual.extend(journal.append_batch(&events[start..end], BatchLimits::default()).unwrap());
                start = end;
            }
        }
        assert_eq!(actual, canonical, "partition {cuts}");
        same(&journal, &oracle);
    }
    let sync = Journal::replay(canonical.as_slice(), identity(), genesis, Limits::default()).unwrap();
    same(&sync, &oracle);
    let mut pending = Journal::begin_replay(canonical.as_slice(), identity(), genesis, Limits::default()).unwrap();
    while pending.progress().status == ReplayStatus::Reading {
        pending.advance(ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 }).unwrap();
    }
    same(&pending.finish().unwrap(), &oracle);
}

#[test]
fn a_failure_at_any_group_position_exposes_no_prefix_or_charge() {
    let valid = events();
    for count in 0..valid.len() {
        let mut journal = Journal::new(identity(), Limits::default()).unwrap();
        // Seed a retained history to distinguish rollback from resetting it.
        journal.append(&Event::RevokeEpoch).unwrap();
        let original = journal.clone();
        let mut batch = valid[..count].to_vec();
        batch.push(Event::Reserve { id: 99, effect: effect(0) });
        assert!(journal.append_batch(&batch, BatchLimits::default()).is_err());
        same(&journal, &original);
        if count > 0 { journal.append_batch(&valid[..count], BatchLimits::default()).unwrap(); }
    }
}

#[test]
fn order_is_not_sorted_or_weakened_to_admit_revoked_or_changed_dispatch() {
    for mutation in 0..3 {
        let mut journal = Journal::new(identity(), Limits::default()).unwrap();
        let original = journal.clone();
        let mut dispatch = effect(30);
        let mut batch = vec![Event::Reserve { id: 1, effect: effect(30) }];
        match mutation {
            0 => batch.push(Event::RevokeEpoch),
            1 => dispatch.payload.push(42),
            2 => dispatch.resolved_target.push_str("/other"),
            _ => unreachable!(),
        }
        batch.push(Event::Dispatch { id: 1, effect: dispatch });
        assert!(journal.append_batch(&batch, BatchLimits::default()).is_err());
        same(&journal, &original);
    }
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    journal.append_batch(&[
        Event::Reserve { id: 1, effect: effect(30) },
        Event::Dispatch { id: 1, effect: effect(30) }, Event::RevokeEpoch,
    ], BatchLimits::default()).unwrap();
    assert_eq!(journal.rights().state(1), Ok(State::Dispatched));
    assert_eq!(journal.rights().available(), 70);
}

#[test]
fn unknown_outcomes_survive_grouped_reset_and_only_reconciliation_refunds() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    journal.append_batch(&events()[..5], BatchLimits::default()).unwrap();
    let original = journal.clone();
    assert!(journal.append_batch(&[Event::ResetToCheckpoint { keep: BTreeSet::new() },
        Event::AbortBeforeDispatch { id: 1 }], BatchLimits::default()).is_err());
    same(&journal, &original);
    journal.append_batch(&[Event::ResetToCheckpoint { keep: BTreeSet::new() }, Event::RevokeEpoch],
        BatchLimits::default()).unwrap();
    assert_eq!(journal.rights().state(1), Ok(State::Unknown));
    assert_eq!(journal.rights().available(), 70);
    journal.append_batch(&[Event::Reconcile { id: 1, occurred: false }], BatchLimits::default()).unwrap();
    assert_eq!(journal.rights().available(), 100);
}

#[test]
fn per_call_record_and_wire_limits_have_exact_boundaries() {
    let events = events();
    let mut oracle = Journal::new(identity(), Limits::default()).unwrap();
    let bytes: Vec<u8> = events.iter().flat_map(|event| oracle.append(event).unwrap()).collect();
    for budget in [BatchLimits { max_records: events.len() - 1, max_encoded_bytes: bytes.len() },
        BatchLimits { max_records: events.len(), max_encoded_bytes: bytes.len() - 1 }] {
        let mut journal = Journal::new(identity(), Limits::default()).unwrap();
        let original = journal.clone();
        assert!(matches!(journal.append_batch(&events, budget), Err(Error::Limit)));
        same(&journal, &original);
    }
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    assert_eq!(journal.append_batch(&events, BatchLimits {
        max_records: events.len(), max_encoded_bytes: bytes.len(),
    }).unwrap(), bytes);
    same(&journal, &oracle);
}

#[test]
fn original_lifetime_and_frame_limits_are_not_refilled_by_group_boundaries() {
    let limits = Limits { max_frame_bytes: 13, max_records: 3, max_total_bytes: 84 + 3 * 21 };
    let mut journal = Journal::new(identity(), limits).unwrap();
    journal.append_batch(&[Event::RevokeEpoch, Event::RevokeEpoch], BatchLimits::default()).unwrap();
    let original = journal.clone();
    assert!(matches!(journal.append_batch(&[Event::RevokeEpoch, Event::RevokeEpoch],
        BatchLimits::default()), Err(Error::Limit)));
    same(&journal, &original);
    journal.append_batch(&[Event::RevokeEpoch], BatchLimits::default()).unwrap();
    assert_eq!(journal.encoded_len(), limits.max_total_bytes);
    assert_eq!(journal.head().sequence, 3);
    let mut journal = Journal::new(identity(), Limits { max_frame_bytes: 13, ..Limits::default() }).unwrap();
    let original = journal.clone();
    assert!(matches!(journal.append_batch(&[Event::RevokeEpoch, Event::Reserve { id: 1, effect: effect(1) }],
        BatchLimits::default()), Err(Error::Limit)));
    same(&journal, &original);
    journal.append_batch(&[Event::RevokeEpoch], BatchLimits::default()).unwrap();
    let mut oracle = Journal::new(identity(), Limits::default()).unwrap();
    let count = events().len();
    let bytes = events().iter().map(|event| oracle.append(event).unwrap().len() as u64).sum::<u64>();
    for limit in [bytes - 1, bytes] {
        let mut journal = Journal::new(identity(), Limits { max_total_bytes: 84 + limit,
            max_records: count as u64, ..Limits::default() }).unwrap();
        let original = journal.clone();
        let result = journal.append_batch(&events(), BatchLimits::default());
        if limit < bytes { assert!(matches!(result, Err(Error::Limit))); same(&journal, &original); }
        else { result.unwrap(); same(&journal, &oracle); }
    }
}

#[test]
fn empty_and_invalid_or_excessive_groups_do_not_mutate() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let original = journal.clone();
    assert!(matches!(journal.append_batch(&[], BatchLimits::default()), Err(Error::InvalidInput)));
    for budget in [BatchLimits { max_records: 0, ..BatchLimits::default() },
        BatchLimits { max_encoded_bytes: 0, ..BatchLimits::default() },
        BatchLimits { max_records: MAX_BATCH_RECORDS + 1, ..BatchLimits::default() },
        BatchLimits { max_encoded_bytes: MAX_BATCH_ENCODED_BYTES + 1, ..BatchLimits::default() }] {
        assert!(matches!(journal.append_batch(&[Event::RevokeEpoch], budget), Err(Error::Limit)));
        same(&journal, &original);
    }
    assert!(matches!(journal.append_batch(&vec![Event::RevokeEpoch; MAX_BATCH_RECORDS + 1],
        BatchLimits::default()), Err(Error::Limit)));
    same(&journal, &original);
    let bytes = journal.append_batch(&vec![Event::RevokeEpoch; MAX_BATCH_RECORDS],
        BatchLimits { max_records: MAX_BATCH_RECORDS, max_encoded_bytes: MAX_BATCH_RECORDS * 21 }).unwrap();
    assert_eq!(bytes.len(), MAX_BATCH_RECORDS * 21);
    assert_eq!(journal.head().sequence, MAX_BATCH_RECORDS as u64);
}

#[test]
fn oversized_last_payload_refuses_instead_of_publishing_prior_valid_records() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let original = journal.clone();
    let mut huge = effect(1); huge.payload = vec![0; Limits::default().max_frame_bytes as usize];
    assert!(matches!(journal.append_batch(&[Event::RevokeEpoch, Event::Reserve { id: 1, effect: huge }],
        BatchLimits::default()), Err(Error::Limit)));
    same(&journal, &original);
}
