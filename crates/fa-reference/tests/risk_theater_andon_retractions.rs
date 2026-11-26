//! Exact source corrections for original Andon opening provenance.
//! These histories exercise reference observations, not native authority.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use fa_reference::action::consequence::Consequence;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::RiskTheaterFamily;
use fa_reference::governance_assurance::risk_theater_queries::{
    AndonFact, AuditFact, ConsequenceFact, CoverageFact, ExtendedQueryPolicy, HistoryChange,
    HistoryFact, HistoryFrontier, HistoryRecord, IncidentClosureMode, IncidentDisposition,
    MediationLevel, QueryEngine, QueryError, QueryPolicy, QuerySnapshot,
};

const FAMILY: RiskTheaterFamily = RiskTheaterFamily::AndonClosureMode;

fn address(kind: Noun, id: &str) -> Address {
    Address::new("test", kind, id, Some(1)).unwrap()
}

fn owner() -> Address {
    address(Noun::Helper, "independent-evaluator")
}

fn record(kind: Noun, id: &str, occurred_at: u64, fact: HistoryFact) -> HistoryRecord {
    HistoryRecord {
        scope: address(Noun::Run, "run"),
        address: address(kind, id),
        occurred_at,
        fact,
    }
}

fn new_engine() -> QueryEngine {
    QueryEngine::new(
        QueryPolicy::new(address(Noun::Run, "run"), 7, owner(), 8, 2)
            .unwrap()
            .with_extended(ExtendedQueryPolicy::default())
            .unwrap(),
    )
    .unwrap()
}

fn accept(engine: &mut QueryEngine, changes: Vec<HistoryChange>) -> QuerySnapshot {
    let sequence = engine.source_history().len() as u64 + 1;
    let snapshot = engine
        .apply_frontier(&HistoryFrontier {
            sequence,
            closed: true,
            changes,
        })
        .expect("complete exact source boundary");
    assert_eq!(engine.verify_kernel().unwrap(), *snapshot.kernel());
    snapshot
}

fn advance(engine: &mut QueryEngine, records: &[HistoryRecord], through: u64) -> QuerySnapshot {
    for sequence in engine.source_history().len() as u64 + 1..=through {
        accept(
            engine,
            records
                .iter()
                .filter(|record| record.occurred_at == sequence)
                .cloned()
                .map(HistoryChange::Insert)
                .collect(),
        );
    }
    engine.snapshot(through).unwrap()
}

fn reject(engine: &mut QueryEngine, changes: Vec<HistoryChange>, error: QueryError) {
    let sequence = engine.source_history().len() as u64 + 1;
    let previous = engine.snapshot(sequence - 1).unwrap();
    let history = engine.source_history().to_vec();
    assert_eq!(
        engine.apply_frontier(&HistoryFrontier {
            sequence,
            closed: true,
            changes,
        }),
        Err(error)
    );
    assert_eq!(engine.source_history(), history);
    assert_eq!(engine.snapshot(sequence - 1).unwrap(), previous);
    assert_eq!(engine.verify_kernel().unwrap(), *previous.kernel());
    assert!(engine.snapshot(sequence).is_err());
}

fn selected(records: &[HistoryRecord], id: &str) -> HistoryRecord {
    records
        .iter()
        .find(|record| record.address.id == id)
        .unwrap()
        .clone()
}

fn incident(id: &str, at: u64, opened_by: &str) -> HistoryRecord {
    record(
        Noun::Incident,
        id,
        at,
        HistoryFact::Andon(AndonFact::Incident {
            window: address(Noun::Ledger, "window"),
            opened_by: address(Noun::Evidence, opened_by),
        }),
    )
}

fn open_disposition(id: &str, at: u64, original: &str) -> HistoryRecord {
    record(
        Noun::Observation,
        id,
        at,
        HistoryFact::Andon(AndonFact::Disposition {
            incident: address(Noun::Incident, original),
            state: IncidentDisposition::Open,
        }),
    )
}

/// Allowance two, original confirmation order A/B/C, and original opening B.
/// C arrives after the incident and cannot become its historical opening.
fn sources(mode: Option<IncidentClosureMode>) -> Vec<HistoryRecord> {
    let mut records = vec![record(
        Noun::Ledger,
        "window",
        1,
        HistoryFact::Andon(AndonFact::Window {
            start: 2,
            end: 5,
            false_stops_allowed: 2,
            cost_per_stop: u64::MAX,
        }),
    )];
    for (name, occurrence, confirmation) in [("a", 2, 5), ("b", 3, 6), ("c", 4, 7)] {
        let round = format!("round-{name}");
        records.push(record(
            Noun::Round,
            &round,
            occurrence,
            HistoryFact::Consequence(ConsequenceFact::Round {
                effect_family: "tool.write".to_owned(),
                helper_generation: address(Noun::Helper, "helper"),
                policy_generation: address(Noun::Policy, "policy"),
                consequence: Consequence::HoldEffect,
                canary: None,
            }),
        ));
        records.push(record(
            Noun::Evidence,
            name,
            confirmation,
            HistoryFact::Andon(AndonFact::FalseStop {
                window: address(Noun::Ledger, "window"),
                round: address(Noun::Round, &round),
                evaluator: owner(),
            }),
        ));
    }
    records.push(incident("incident", 6, "b"));
    if let Some(mode) = mode {
        records.push(record(
            Noun::Consequence,
            "close-authority",
            8,
            HistoryFact::Andon(AndonFact::Authorization {
                incident: address(Noun::Incident, "incident"),
                mode,
            }),
        ));
        records.push(record(
            Noun::Receipt,
            "closed",
            9,
            HistoryFact::Andon(AndonFact::Closure {
                incident: address(Noun::Incident, "incident"),
                mode,
                authorization: address(Noun::Consequence, "close-authority"),
            }),
        ));
        records.push(record(
            Noun::Observation,
            "disposition",
            10,
            HistoryFact::Andon(AndonFact::Disposition {
                incident: address(Noun::Incident, "incident"),
                state: IncidentDisposition::Closed {
                    closure: address(Noun::Receipt, "closed"),
                },
            }),
        ));
    } else {
        records.push(open_disposition("disposition", 10, "incident"));
    }
    records
}

fn incomplete(snapshot: &QuerySnapshot) {
    assert!(!snapshot.family(FAMILY).complete);
    assert!(snapshot.family(FAMILY).findings.is_empty());
    assert_eq!(snapshot.kernel().scores[&FAMILY], 0);
    assert!(snapshot.kernel().supports.is_empty());
}

#[test]
fn withdrawing_an_earlier_confirmation_retracts_the_finding_without_moving_its_opening() {
    let records = sources(Some(IncidentClosureMode::Roster));
    let mut engine = new_engine();
    let original = advance(&mut engine, &records, 10);
    assert!(original.family(FAMILY).complete);
    assert_eq!(original.family(FAMILY).findings.len(), 1);
    let first = selected(&records, "a");
    incomplete(&accept(
        &mut engine,
        vec![HistoryChange::Retract(first.clone())],
    ));
    incomplete(&accept(&mut engine, Vec::new()));
    let restored = accept(&mut engine, vec![HistoryChange::Insert(first)]);
    assert_eq!(restored.family(FAMILY), original.family(FAMILY));
}

#[test]
fn exact_opening_round_window_and_incident_sources_can_be_withdrawn_and_restored() {
    for id in ["b", "round-a", "window", "incident"] {
        let records = sources(Some(IncidentClosureMode::Roster));
        let mut engine = new_engine();
        let original = advance(&mut engine, &records, 10);
        let source = selected(&records, id);
        incomplete(&accept(
            &mut engine,
            vec![HistoryChange::Retract(source.clone())],
        ));
        incomplete(&accept(&mut engine, Vec::new()));
        let restored = accept(&mut engine, vec![HistoryChange::Insert(source)]);
        assert_eq!(restored.family(FAMILY), original.family(FAMILY));
    }
}

#[test]
fn withdrawing_a_later_confirmation_preserves_the_original_crossing_and_remaining_support() {
    let records = sources(Some(IncidentClosureMode::Roster));
    let mut engine = new_engine();
    let original = advance(&mut engine, &records, 10);
    let later = selected(&records, "c");
    let corrected = accept(&mut engine, vec![HistoryChange::Retract(later.clone())]);
    assert!(corrected.family(FAMILY).complete);
    assert_eq!(corrected.family(FAMILY).findings.len(), 1);
    assert_eq!(
        corrected.family(FAMILY).findings[0].subject,
        address(Noun::Incident, "incident")
    );
    let expected: BTreeSet<_> = original.family(FAMILY).findings[0]
        .support
        .iter()
        .filter(|id| {
            **id != address(Noun::Evidence, "c") && **id != address(Noun::Round, "round-c")
        })
        .cloned()
        .collect();
    assert_eq!(
        corrected.family(FAMILY).findings[0]
            .support
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected
    );
    let restored = accept(&mut engine, vec![HistoryChange::Insert(later)]);
    assert_eq!(restored.family(FAMILY), original.family(FAMILY));
}

#[test]
fn opening_provenance_is_retained_for_open_evidence_closed_and_policy_closed_incidents() {
    for mode in [
        None,
        Some(IncidentClosureMode::Evidence),
        Some(IncidentClosureMode::Policy),
    ] {
        let records = sources(mode);
        let mut engine = new_engine();
        let original = advance(&mut engine, &records, 10);
        assert!(original.family(FAMILY).complete);
        assert!(original.family(FAMILY).findings.is_empty());
        let first = selected(&records, "a");
        incomplete(&accept(
            &mut engine,
            vec![HistoryChange::Retract(first.clone())],
        ));
        let restored = accept(&mut engine, vec![HistoryChange::Insert(first)]);
        assert_eq!(restored.family(FAMILY), original.family(FAMILY));
    }
}

#[test]
fn fresh_wrong_openings_still_refuse_after_an_unrelated_source_withdrawal() {
    for opening in ["a", "c"] {
        let mut records = sources(None);
        records.retain(|record| {
            !matches!(
                &record.fact,
                HistoryFact::Andon(AndonFact::Incident { .. } | AndonFact::Disposition { .. })
            )
        });
        let unrelated = record(
            Noun::Observation,
            "unrelated-audit",
            1,
            HistoryFact::Audit(AuditFact::Scheduled {
                effect_family: "audit".to_owned(),
                required_units: 1,
                due_at: 30,
            }),
        );
        records.push(unrelated.clone());
        let mut engine = new_engine();
        advance(&mut engine, &records, 7);
        accept(&mut engine, vec![HistoryChange::Retract(unrelated)]);
        reject(
            &mut engine,
            vec![HistoryChange::Insert(incident("fresh", 9, opening))],
            QueryError::InvalidReference,
        );
        accept(
            &mut engine,
            vec![HistoryChange::Insert(incident("fresh", 9, "b"))],
        );
        let valid = accept(
            &mut engine,
            vec![HistoryChange::Insert(open_disposition(
                "fresh-open",
                10,
                "fresh",
            ))],
        );
        assert!(valid.family(FAMILY).complete);
    }
}

fn malformed_coverage(at: u64) -> Vec<HistoryRecord> {
    let mut records = Vec::new();
    for id in ["prior-coverage", "next-coverage"] {
        records.push(record(
            Noun::Profile,
            id,
            at,
            HistoryFact::Coverage(CoverageFact::State {
                effect_family: "tool.write".to_owned(),
                mediation: MediationLevel::Brokered,
                capture_contract: address(Noun::Profile, "capture"),
                channels: BTreeSet::new(),
            }),
        ));
    }
    // Individually valid records, but an unordered same-boundary state pair:
    // coverage evaluation fails AFTER Andon has validated its candidate opening.
    records.push(record(
        Noun::Consequence,
        "coverage-change",
        at,
        HistoryFact::Coverage(CoverageFact::Change {
            effect_family: "tool.write".to_owned(),
            previous: address(Noun::Profile, "prior-coverage"),
            current: address(Noun::Profile, "next-coverage"),
            authorization: None,
        }),
    ));
    records
}

#[test]
fn rejected_later_predicate_cannot_install_an_opening_provenance_exception() {
    let mut records = sources(None);
    records.retain(|record| {
        !matches!(
            &record.fact,
            HistoryFact::Andon(AndonFact::Incident { .. } | AndonFact::Disposition { .. })
        )
    });
    let mut engine = new_engine();
    advance(&mut engine, &records, 7);
    let candidate = incident("candidate", 8, "b");
    let mut changes = vec![HistoryChange::Insert(candidate.clone())];
    changes.extend(malformed_coverage(8).into_iter().map(HistoryChange::Insert));
    reject(&mut engine, changes, QueryError::InvalidReference);
    // If the failed candidate retained provenance, it would incorrectly excuse
    // the absent A and accept this fresh, unvalidated early opening as unknown.
    reject(
        &mut engine,
        vec![
            HistoryChange::Retract(selected(&records, "a")),
            HistoryChange::Insert(candidate.clone()),
        ],
        QueryError::InvalidReference,
    );
    accept(&mut engine, vec![HistoryChange::Insert(candidate)]);
    let valid = accept(
        &mut engine,
        vec![HistoryChange::Insert(open_disposition(
            "candidate-open",
            9,
            "candidate",
        ))],
    );
    assert!(valid.family(FAMILY).complete);
}

#[test]
fn missing_opening_support_cannot_hide_an_observed_reopening_of_a_closed_incident() {
    let records = sources(Some(IncidentClosureMode::Roster));
    let mut engine = new_engine();
    let original = advance(&mut engine, &records, 10);
    let first = selected(&records, "a");
    reject(
        &mut engine,
        vec![
            HistoryChange::Retract(first.clone()),
            HistoryChange::Insert(open_disposition("reopened", 11, "incident")),
        ],
        QueryError::ConflictingRecord,
    );
    incomplete(&accept(
        &mut engine,
        vec![HistoryChange::Retract(first.clone())],
    ));
    let restored = accept(&mut engine, vec![HistoryChange::Insert(first)]);
    assert_eq!(restored.family(FAMILY), original.family(FAMILY));
}

#[test]
fn accepted_source_history_replay_reconstructs_provenance_through_incomplete_frontiers() {
    let records = sources(Some(IncidentClosureMode::Roster));
    let mut original = new_engine();
    let complete = advance(&mut original, &records, 10);
    let first = selected(&records, "a");
    incomplete(&accept(
        &mut original,
        vec![HistoryChange::Retract(first.clone())],
    ));
    let pending = accept(&mut original, Vec::new());
    let mut replay = new_engine();
    for frontier in original.source_history() {
        replay
            .apply_frontier(frontier)
            .expect("replay original accepted source boundary");
    }
    assert_eq!(replay.snapshot(12).unwrap(), pending);
    let restored = accept(&mut original, vec![HistoryChange::Insert(first.clone())]);
    let replayed = accept(&mut replay, vec![HistoryChange::Insert(first)]);
    assert_eq!(replayed, restored);
    assert_eq!(restored.family(FAMILY), complete.family(FAMILY));
}

#[test]
fn restoring_an_earlier_confirmation_cannot_hide_a_changed_original_crossing() {
    let mut records = sources(None);
    records.retain(|record| {
        !matches!(
            &record.fact,
            HistoryFact::Andon(AndonFact::Incident { .. } | AndonFact::Disposition { .. })
        )
    });
    let mut engine = new_engine();
    advance(&mut engine, &records, 5);
    let first = selected(&records, "a");
    accept(
        &mut engine,
        vec![
            HistoryChange::Retract(first.clone()),
            HistoryChange::Insert(selected(&records, "b")),
        ],
    );
    accept(
        &mut engine,
        vec![
            HistoryChange::Insert(selected(&records, "c")),
            HistoryChange::Insert(incident("remaining-opening", 7, "c")),
        ],
    );
    let known = accept(
        &mut engine,
        vec![HistoryChange::Insert(open_disposition(
            "remaining-open",
            8,
            "remaining-opening",
        ))],
    );
    assert!(known.family(FAMILY).complete);
    reject(
        &mut engine,
        vec![HistoryChange::Insert(first)],
        QueryError::InvalidReference,
    );
    let unchanged = accept(&mut engine, Vec::new());
    assert_eq!(unchanged.family(FAMILY), known.family(FAMILY));
}

#[test]
fn a_missing_pinned_confirmation_does_not_hide_an_observed_earlier_crossing() {
    let mut records = sources(None);
    for source in &mut records {
        match source.address.id.as_str() {
            "a" => source.occurred_at = 6,
            "b" | "incident" => source.occurred_at = 8,
            "c" => source.occurred_at = 9,
            _ => {}
        }
    }
    for (confirmation, round_id, original_round, at) in [
        ("d", "round-d", "round-a", 5),
        ("e", "round-e", "round-b", 7),
    ] {
        let mut round = selected(&records, original_round);
        round.address = address(Noun::Round, round_id);
        records.push(round);
        records.push(record(
            Noun::Evidence,
            confirmation,
            at,
            HistoryFact::Andon(AndonFact::FalseStop {
                window: address(Noun::Ledger, "window"),
                round: address(Noun::Round, round_id),
                evaluator: owner(),
            }),
        ));
    }
    let mut engine = new_engine();
    for sequence in 1..=10 {
        let mut changes: Vec<_> = records
            .iter()
            .filter(|record| record.occurred_at == sequence)
            .cloned()
            .map(HistoryChange::Insert)
            .collect();
        // Each earlier confirmation was genuinely observed then withdrawn
        // before A/B first established the original incident at boundary eight.
        if sequence == 6 {
            changes.push(HistoryChange::Retract(selected(&records, "d")));
        }
        if sequence == 8 {
            changes.push(HistoryChange::Retract(selected(&records, "e")));
        }
        accept(&mut engine, changes);
    }
    let known = engine.snapshot(10).unwrap();
    assert!(known.family(FAMILY).complete);
    reject(
        &mut engine,
        vec![
            HistoryChange::Retract(selected(&records, "a")),
            HistoryChange::Insert(selected(&records, "d")),
            HistoryChange::Insert(selected(&records, "e")),
        ],
        QueryError::InvalidReference,
    );
    // D/E's restored crossing before B is a contradiction, whereas A's exact
    // withdrawal alone only leaves insufficient original support for B.
    incomplete(&accept(
        &mut engine,
        vec![HistoryChange::Retract(selected(&records, "a"))],
    ));
    let restored = accept(
        &mut engine,
        vec![HistoryChange::Insert(selected(&records, "a"))],
    );
    assert_eq!(restored.family(FAMILY), known.family(FAMILY));
}
