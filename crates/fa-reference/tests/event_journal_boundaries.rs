//! Admission and wire regressions for the public domain-journal boundary.
//! These use the independent strict JSON syntax parser, not the journal encoder as an oracle.

use fa_reference::action::consequence::Consequence;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::event_journal::{
    DomainEventJournal, JournalError, JournalEvent, JournalEventKind, JournalFold, JournalGapRange,
    MAX_EVENT_ADDRESSES, MAX_SOURCE_FRONTIERS, SourceFrontierEntry, decision_replay_from_journal,
};
use fa_reference::observation_algebra::UnknownReason;
use fa_reference::strict_json::{Json, Limits, parse};
use fa_reference::typestate_cx::RootCx;

fn address(tenant: &str) -> Address {
    Address::new(tenant, Noun::Attempt, "attempt-1", Some(1)).unwrap()
}

fn proposal(control_seq: u64) -> JournalEvent {
    JournalEvent::new(
        control_seq,
        Vec::new(),
        vec![address("org")],
        JournalEventKind::Proposal { attempt_id: 1 },
        [0; 32],
        7,
        42,
    )
    .unwrap()
}

fn rehearsal(control_seq: u64, branch_id: u64) -> JournalEvent {
    JournalEvent::new(
        control_seq,
        Vec::new(),
        vec![address("org")],
        JournalEventKind::RehearsalOutcome {
            branch_id,
            would_consequence: Consequence::HoldEffect,
            rehearsal: true,
        },
        [0; 32],
        7,
        42,
    )
    .unwrap()
}

#[derive(Default)]
struct TraceFold {
    control_seqs: Vec<u64>,
    gaps: Vec<JournalGapRange>,
}

impl JournalFold for TraceFold {
    type Output = (Vec<u64>, Vec<JournalGapRange>, u64);

    fn initial_state() -> Self {
        Self::default()
    }

    fn step(&mut self, event: &JournalEvent) {
        self.control_seqs.push(event.control_seq);
    }

    fn on_gap(&mut self, gap: JournalGapRange) {
        self.gaps.push(gap);
    }

    fn finalize(self, exact_at_control_seq: u64) -> Self::Output {
        (self.control_seqs, self.gaps, exact_at_control_seq)
    }
}

#[test]
fn admitted_text_fields_round_trip_as_data_in_causal_and_wire_json() {
    let mut text = String::from("quote \" slash \\ / non-ASCII é λ 🦀 ");
    text.extend((0u8..=31).map(char::from));
    text.push_str("\",\"realized_cost\":999,\"injected\":true,\"text\":\"");
    let cases = [
        (
            "reason",
            JournalEventKind::Hold {
                attempt_id: 1,
                reason: text.clone(),
            },
        ),
        (
            "outcome_class",
            JournalEventKind::Outcome {
                attempt_id: 1,
                outcome_class: text.clone(),
            },
        ),
        (
            "detector_id",
            JournalEventKind::Finding {
                finding_id: 1,
                detector_id: text.clone(),
                severity: 2,
            },
        ),
    ];
    let mut journal = DomainEventJournal::new_production("org", "domain");

    for (index, (field, kind)) in cases.into_iter().enumerate() {
        let emitted = journal
            .emit_transition(Vec::new(), vec![address("org")], kind, [0; 32], 7, 42)
            .unwrap();
        assert_eq!(emitted.control_seq, (index + 1) as u64);
        assert_eq!(
            journal.fa_tail(index as u64, 1).events,
            vec![emitted.clone()]
        );

        for (encoded, expected_fields) in [
            (emitted.encode_causal_canonical(), 9),
            (emitted.encode_wire_canonical(), 10),
        ] {
            let decoded = parse(encoded.as_bytes(), Limits::default())
                .expect("an admitted event must encode strict JSON");
            assert_eq!(decoded.as_object().unwrap().len(), expected_fields);
            assert_eq!(decoded.get("realized_cost").and_then(Json::as_u64), Some(7));
            assert!(decoded.get("injected").is_none());
            let detail = decoded.get("detail").unwrap();
            assert_eq!(
                detail.get(field).and_then(Json::as_str),
                Some(text.as_str())
            );
            assert!(detail.get("injected").is_none());
        }

        let mut later_clock = emitted.clone();
        later_clock.wall_clock_annotation_ms = u64::MAX;
        assert_eq!(emitted.causal_digest(), later_clock.causal_digest());
        assert_ne!(
            emitted.encode_wire_canonical(),
            later_clock.encode_wire_canonical()
        );
    }
}

#[test]
fn already_valid_wire_bytes_remain_stable() {
    let mut journal = DomainEventJournal::new_production("org", "domain");
    let event = journal
        .emit_transition(
            vec![
                SourceFrontierEntry {
                    source_id: 2,
                    frontier_seq: 9,
                },
                SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: 8,
                },
            ],
            vec![address("org")],
            JournalEventKind::Hold {
                attempt_id: 1,
                reason: "mandatory_absent".to_owned(),
            },
            [0; 32],
            7,
            42,
        )
        .unwrap();
    assert_eq!(
        event.encode_wire_canonical(),
        concat!(
            "{\"schema\":\"fa.event_journal.v1\",\"control_seq\":1,\"layer\":4,",
            "\"kind\":\"hold\",\"detail\":{\"attempt_id\":1,\"reason\":\"mandatory_absent\"},",
            "\"frontiers\":[\"1:8\",\"2:9\"],\"addresses\":[\"fa://org/attempt/attempt-1@1\"],",
            "\"payload_digest\":\"0000000000000000000000000000000000000000000000000000000000000000\",",
            "\"realized_cost\":7,\"wall_clock_annotation_ms\":42}"
        )
    );
}

#[test]
fn append_revalidates_public_fields_before_any_journal_mutation() {
    let mut journal = DomainEventJournal::new_production("org", "domain");
    journal.append(proposal(1)).unwrap();
    let before = journal.fa_tail(0, usize::MAX);
    let mut cases = Vec::new();

    let mut zero_seq = proposal(2);
    zero_seq.control_seq = 0;
    cases.push((zero_seq, JournalError::MissingControlSeq));

    // A struct literal bypasses the constructor entirely.
    cases.push((
        JournalEvent {
            control_seq: 2,
            source_frontiers: Vec::new(),
            addresses: Vec::new(),
            kind: JournalEventKind::Proposal { attempt_id: 2 },
            payload_digest: [0; 32],
            realized_cost: 0,
            wall_clock_annotation_ms: 0,
        },
        JournalError::EmptyAddresses,
    ));

    let mut too_many_addresses = proposal(2);
    too_many_addresses.addresses = vec![address("org"); MAX_EVENT_ADDRESSES + 1];
    cases.push((
        too_many_addresses,
        JournalError::TooManyAddresses {
            count: MAX_EVENT_ADDRESSES + 1,
            max: MAX_EVENT_ADDRESSES,
        },
    ));

    let mut too_many_frontiers = proposal(2);
    too_many_frontiers.source_frontiers = vec![
        SourceFrontierEntry {
            source_id: 1,
            frontier_seq: 1,
        };
        MAX_SOURCE_FRONTIERS + 1
    ];
    cases.push((
        too_many_frontiers,
        JournalError::TooManySourceFrontiers {
            count: MAX_SOURCE_FRONTIERS + 1,
            max: MAX_SOURCE_FRONTIERS,
        },
    ));

    let mut invalid_address = proposal(2);
    invalid_address.addresses[0].id = "quoted\"address".to_owned();
    cases.push((invalid_address, JournalError::InvalidAddress { index: 0 }));

    let mut unflagged = rehearsal(2, 7);
    if let JournalEventKind::RehearsalOutcome { rehearsal, .. } = &mut unflagged.kind {
        *rehearsal = false;
    }
    cases.push((unflagged, JournalError::UnflaggedRehearsal));

    for (candidate, error) in cases {
        assert_eq!(journal.append(candidate), Err(error));
        assert_eq!(journal.last_control_seq(), 1);
        assert_eq!(journal.fa_tail(0, usize::MAX), before);
    }

    // Valid public-field input is normalized just like constructor-built input.
    let mut valid = proposal(2);
    valid.source_frontiers = vec![
        SourceFrontierEntry {
            source_id: 2,
            frontier_seq: 9,
        },
        SourceFrontierEntry {
            source_id: 1,
            frontier_seq: 8,
        },
    ];
    journal.append(valid).unwrap();
    assert_eq!(
        journal.fa_tail(1, 1).events[0].source_frontiers,
        vec![
            SourceFrontierEntry {
                source_id: 1,
                frontier_seq: 8,
            },
            SourceFrontierEntry {
                source_id: 2,
                frontier_seq: 9,
            },
        ]
    );
}

#[test]
fn constructor_and_append_reject_malformed_address_values() {
    let mut malformed = address("org");
    malformed.tenant.clear();
    assert_eq!(
        JournalEvent::new(
            1,
            Vec::new(),
            vec![malformed],
            JournalEventKind::Proposal { attempt_id: 1 },
            [0; 32],
            0,
            0,
        ),
        Err(JournalError::InvalidAddress { index: 0 })
    );

    let mut journal = DomainEventJournal::new_production("org", "domain");
    let mut event = proposal(1);
    event.addresses[0].id.clear();
    assert_eq!(
        journal.append(event),
        Err(JournalError::InvalidAddress { index: 0 })
    );
    assert!(journal.is_empty());
    assert_eq!(journal.last_control_seq(), 0);
}

#[test]
fn mixed_tenant_events_are_refused_atomically_in_production_and_branches() {
    let root = RootCx::new_nucleus(1, 1, 1);
    let cx = root.mint_experiment(7).unwrap();
    let journals = [
        DomainEventJournal::new_production("org", "domain"),
        DomainEventJournal::new_branch(&cx, "org", "domain", 7),
    ];
    for mut journal in journals {
        journal.append(proposal(1)).unwrap();
        let before = journal.fa_tail(0, usize::MAX);
        let mut foreign = proposal(2);
        foreign.addresses.push(address("other"));
        assert_eq!(journal.append(foreign), Err(JournalError::TenantMismatch));
        assert_eq!(journal.fa_tail(0, usize::MAX), before);
        assert_eq!(journal.last_control_seq(), 1);
        assert_eq!(
            journal.emit_transition(
                Vec::new(),
                vec![address("other")],
                JournalEventKind::Proposal { attempt_id: 1 },
                [0; 32],
                0,
                0,
            ),
            Err(JournalError::TenantMismatch)
        );
        assert_eq!(journal.fa_tail(0, usize::MAX), before);
    }
}

#[test]
fn rehearsal_branch_and_context_must_match_without_spending_a_sequence() {
    let root = RootCx::new_nucleus(1, 1, 1);
    let cx7 = root.mint_experiment(7).unwrap();
    let cx8 = root.mint_experiment(8).unwrap();
    let mut journal = DomainEventJournal::new_branch(&cx7, "org", "domain", 7);
    let before = journal.fa_tail(0, usize::MAX);

    assert_eq!(
        journal.append(rehearsal(1, 8)),
        Err(JournalError::BranchMismatch {
            expected: 7,
            actual: 8,
        })
    );
    let mut wrong_outcome = proposal(1);
    wrong_outcome.kind = JournalEventKind::BranchOutcome {
        branch_id: 8,
        summary_digest: [0; 32],
    };
    assert_eq!(
        journal.append(wrong_outcome),
        Err(JournalError::BranchMismatch {
            expected: 7,
            actual: 8,
        })
    );
    assert_eq!(
        journal.emit_rehearsal_outcome(
            &cx8,
            7,
            Consequence::HoldEffect,
            vec![address("org")],
            [0; 32],
            7,
            42,
        ),
        Err(JournalError::BranchMismatch {
            expected: 7,
            actual: 8,
        })
    );
    assert_eq!(journal.fa_tail(0, usize::MAX), before);
    assert_eq!(journal.last_control_seq(), 0);

    let accepted = journal
        .emit_rehearsal_outcome(
            &cx7,
            7,
            Consequence::HoldEffect,
            vec![address("org")],
            [0; 32],
            7,
            42,
        )
        .unwrap();
    assert_eq!(accepted.control_seq, 1);
    assert_eq!(journal.fa_tail(0, 1).events, vec![accepted]);

    // An inconsistent constructor binding cannot be used through generic append.
    let mut mismatch = DomainEventJournal::new_branch(&cx7, "org", "domain", 8);
    assert_eq!(
        mismatch.append(proposal(1)),
        Err(JournalError::BranchMismatch {
            expected: 7,
            actual: 8,
        })
    );
    assert!(mismatch.is_empty());
    assert_eq!(mismatch.last_control_seq(), 0);
}

#[test]
fn empty_and_zero_frontier_reads_are_total() {
    let mut journal = DomainEventJournal::new_production("org", "domain");
    for since in [0, u64::MAX] {
        let tail = journal.fa_tail(since, usize::MAX);
        assert!(tail.events.is_empty());
        assert!(tail.gaps.is_empty());
        assert_eq!(tail.next_cursor_seq, since);
    }
    assert_eq!(journal.run_fold::<TraceFold>(0), (vec![], vec![], 0));
    assert_eq!(journal.run_fold::<TraceFold>(u64::MAX), (vec![], vec![], 0));
    assert_eq!(decision_replay_from_journal(&journal, &[]), Ok(vec![]));

    journal.append(proposal(1)).unwrap();
    assert_eq!(journal.run_fold::<TraceFold>(0), (vec![], vec![], 0));
    let zero_limit = journal.fa_tail(0, 0);
    assert!(zero_limit.events.is_empty());
    assert!(zero_limit.gaps.is_empty());
    assert_eq!(zero_limit.next_cursor_seq, 0);
}

#[test]
fn maximum_sequence_can_be_emitted_tailed_and_folded_but_never_wraps() {
    let mut journal = DomainEventJournal::new_production("org", "domain");
    journal.append(proposal(u64::MAX - 1)).unwrap();
    let last = journal
        .emit_transition(
            Vec::new(),
            vec![address("org")],
            JournalEventKind::Outcome {
                attempt_id: 1,
                outcome_class: "confirmed".to_owned(),
            },
            [0; 32],
            7,
            42,
        )
        .unwrap();
    assert_eq!(last.control_seq, u64::MAX);
    let first_page = journal.fa_tail(u64::MAX - 2, 1);
    assert_eq!(first_page.next_cursor_seq, u64::MAX - 1);
    assert_eq!(first_page.events.len(), 1);
    assert!(first_page.gaps.is_empty());
    let last_page = journal.fa_tail(first_page.next_cursor_seq, 1);
    assert_eq!(last_page.events, vec![last]);
    assert_eq!(last_page.next_cursor_seq, u64::MAX);
    assert!(last_page.gaps.is_empty());
    let end = journal.fa_tail(u64::MAX, usize::MAX);
    assert!(end.events.is_empty());
    assert!(end.gaps.is_empty());
    assert_eq!(end.next_cursor_seq, u64::MAX);

    let gap = JournalGapRange {
        missing_from_seq: 1,
        missing_through_seq: u64::MAX - 2,
    };
    assert_eq!(
        journal.run_fold::<TraceFold>(u64::MAX),
        (vec![u64::MAX - 1, u64::MAX], vec![gap], u64::MAX)
    );
    assert_eq!(
        journal.fa_tail(0, usize::MAX).gaps,
        vec![(gap, UnknownReason::Incomplete)]
    );
    let before = journal.fa_tail(0, usize::MAX);
    assert_eq!(
        journal.emit_transition(
            Vec::new(),
            vec![address("org")],
            JournalEventKind::Proposal { attempt_id: 2 },
            [0; 32],
            0,
            0,
        ),
        Err(JournalError::ControlSequenceExhausted)
    );
    assert_eq!(journal.fa_tail(0, usize::MAX), before);
    assert_eq!(journal.last_control_seq(), u64::MAX);
}

#[test]
fn exhausted_rehearsal_journal_fails_without_appending_or_wrapping() {
    let root = RootCx::new_nucleus(1, 1, 1);
    let cx = root.mint_experiment(7).unwrap();
    let mut journal = DomainEventJournal::new_branch(&cx, "org", "domain", 7);
    journal.append(rehearsal(u64::MAX, 7)).unwrap();
    let before = journal.fa_tail(0, usize::MAX);
    assert_eq!(
        journal.emit_rehearsal_outcome(
            &cx,
            7,
            Consequence::HoldEffect,
            vec![address("org")],
            [0; 32],
            7,
            42,
        ),
        Err(JournalError::ControlSequenceExhausted)
    );
    assert_eq!(journal.fa_tail(0, usize::MAX), before);
    assert_eq!(journal.last_control_seq(), u64::MAX);
}
