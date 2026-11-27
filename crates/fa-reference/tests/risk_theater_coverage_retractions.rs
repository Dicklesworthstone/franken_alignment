//! Exact coverage-source corrections preserve the original active predecessor.
//! These tests exercise the public bounded query engine, not native journal
//! authentication, live policy authority or empirical alignment.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::RiskTheaterFamily;
use fa_reference::governance_assurance::risk_theater_queries::{
    CoverageFact, ExtendedQueryPolicy, HistoryChange, HistoryFact, HistoryFrontier,
    HistoryRecord, MediationLevel, QueryEngine, QueryError, QueryPolicy, QuerySnapshot,
};

fn address(kind: Noun, id: &str) -> Address {
    Address::new("test", kind, id, Some(1)).unwrap()
}

fn scope() -> Address {
    address(Noun::Run, "coverage-run")
}

fn new_engine() -> QueryEngine {
    let policy = QueryPolicy::new(
        scope(),
        1,
        address(Noun::Helper, "independent-evaluator"),
        8,
        2,
    )
    .unwrap()
    .with_extended(ExtendedQueryPolicy::default())
    .unwrap();
    QueryEngine::new(policy).unwrap()
}

fn record(kind: Noun, id: &str, at: u64, fact: CoverageFact) -> HistoryRecord {
    HistoryRecord {
        scope: scope(),
        address: address(kind, id),
        occurred_at: at,
        fact: HistoryFact::Coverage(fact),
    }
}

fn state(id: &str, at: u64, family: &str, mediation: MediationLevel) -> HistoryRecord {
    record(
        Noun::Profile,
        id,
        at,
        CoverageFact::State {
            effect_family: family.to_owned(),
            mediation,
            capture_contract: address(Noun::Profile, "capture-contract"),
            channels: BTreeSet::from(["activation".to_owned(), "output".to_owned()]),
        },
    )
}

fn authorization(
    id: &str,
    at: u64,
    change: &str,
    previous: &str,
    current: &str,
) -> HistoryRecord {
    record(
        Noun::Receipt,
        id,
        at,
        CoverageFact::Authorization {
            change: address(Noun::Consequence, change),
            previous: address(Noun::Profile, previous),
            current: address(Noun::Profile, current),
        },
    )
}

fn change(
    id: &str,
    at: u64,
    family: &str,
    previous: &str,
    current: &str,
    authorization: &str,
) -> HistoryRecord {
    record(
        Noun::Consequence,
        id,
        at,
        CoverageFact::Change {
            effect_family: family.to_owned(),
            previous: address(Noun::Profile, previous),
            current: address(Noun::Profile, current),
            authorization: Some(address(Noun::Receipt, authorization)),
        },
    )
}

fn sources(prefix: &str, family: &str, start: u64) -> Vec<HistoryRecord> {
    let id = |suffix: &str| format!("{prefix}{suffix}");
    vec![
        state(&id("a"), start, family, MediationLevel::Attested),
        state(&id("b"), start + 1, family, MediationLevel::Brokered),
        authorization(&id("allow-ab"), start + 2, &id("ab"), &id("a"), &id("b")),
        change(&id("ab"), start + 3, family, &id("a"), &id("b"), &id("allow-ab")),
        state(&id("c"), start + 4, family, MediationLevel::Brokered),
        authorization(&id("allow-bc"), start + 5, &id("bc"), &id("b"), &id("c")),
        change(&id("bc"), start + 6, family, &id("b"), &id("c"), &id("allow-bc")),
        state(&id("d"), start + 7, family, MediationLevel::Cooperative),
        authorization(&id("allow-cd"), start + 8, &id("cd"), &id("c"), &id("d")),
        change(&id("cd"), start + 9, family, &id("c"), &id("d"), &id("allow-cd")),
    ]
}

fn selected(records: &[HistoryRecord], id: &str) -> HistoryRecord {
    records.iter().find(|record| record.address.id == id).unwrap().clone()
}

fn accept(engine: &mut QueryEngine, changes: Vec<HistoryChange>) -> QuerySnapshot {
    let sequence = engine.source_history().len() as u64 + 1;
    let snapshot = engine.apply_frontier(&HistoryFrontier {
        sequence,
        closed: true,
        changes,
    }).unwrap();
    assert_eq!(engine.verify_kernel().unwrap(), *snapshot.kernel());
    snapshot
}

fn insert(engine: &mut QueryEngine, records: &[HistoryRecord]) -> QuerySnapshot {
    accept(engine, records.iter().cloned().map(HistoryChange::Insert).collect())
}

fn feed(engine: &mut QueryEngine, records: &[HistoryRecord]) -> QuerySnapshot {
    let first = engine.source_history().len() as u64 + 1;
    let last = records.iter().map(|record| record.occurred_at).max().unwrap();
    for sequence in first..=last {
        let at: Vec<_> = records.iter().filter(|record| record.occurred_at == sequence)
            .cloned().collect();
        insert(engine, &at);
    }
    engine.snapshot(last).unwrap()
}

fn original() -> (QueryEngine, QuerySnapshot, Vec<HistoryRecord>) {
    let records = sources("", "tool.write", 1);
    let mut engine = new_engine();
    let snapshot = feed(&mut engine, &records);
    assert!(snapshot.family(RiskTheaterFamily::CoverageErosion).complete);
    assert_subjects(&snapshot, &["ab", "cd"]);
    (engine, snapshot, records)
}

fn assert_subjects(snapshot: &QuerySnapshot, expected: &[&str]) {
    let actual: BTreeSet<_> = snapshot.family(RiskTheaterFamily::CoverageErosion)
        .findings.iter().map(|finding| finding.subject.id.clone()).collect();
    let expected: BTreeSet<_> = expected.iter().map(|id| (*id).to_owned()).collect();
    assert_eq!(actual, expected);
}

fn assert_incomplete(snapshot: &QuerySnapshot) {
    assert!(!snapshot.family(RiskTheaterFamily::CoverageErosion).complete);
}

fn reject(engine: &mut QueryEngine, changes: Vec<HistoryChange>, error: QueryError) {
    let history = engine.source_history().to_vec();
    let previous = history.len() as u64;
    let snapshot = engine.snapshot(previous).unwrap();
    assert_eq!(engine.apply_frontier(&HistoryFrontier {
        sequence: previous + 1,
        closed: true,
        changes,
    }), Err(error));
    assert_eq!(engine.source_history(), history.as_slice());
    assert_eq!(engine.snapshot(previous).unwrap(), snapshot);
    assert_eq!(engine.verify_kernel().unwrap(), *snapshot.kernel());
    assert!(engine.snapshot(previous + 1).is_err());
}

#[test]
fn a_middle_withdrawal_advances_the_frontier_and_retracts_only_dependent_erosion() {
    let (mut engine, initial, records) = original();
    let middle = selected(&records, "bc");
    let missing = accept(&mut engine, vec![HistoryChange::Retract(middle.clone())]);
    assert_eq!(missing.sequence(), 11);
    assert_incomplete(&missing);
    assert_subjects(&missing, &["ab"]);
    assert_eq!(
        missing.family(RiskTheaterFamily::CoverageErosion).findings[0],
        initial.family(RiskTheaterFamily::CoverageErosion).findings[0],
    );
    let restored = accept(&mut engine, vec![HistoryChange::Insert(middle)]);
    assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
        initial.family(RiskTheaterFamily::CoverageErosion));
}

#[test]
fn a_withdrawn_first_change_cannot_turn_its_successor_into_a_new_baseline() {
    let (mut engine, initial, records) = original();
    let first = selected(&records, "ab");
    let missing = accept(&mut engine, vec![HistoryChange::Retract(first.clone())]);
    assert_incomplete(&missing);
    assert_subjects(&missing, &[]);
    let restored = accept(&mut engine, vec![HistoryChange::Insert(first)]);
    assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
        initial.family(RiskTheaterFamily::CoverageErosion));
}

#[test]
fn a_withdrawn_last_change_is_not_silently_forgotten_by_earlier_complete_history() {
    let (mut engine, initial, records) = original();
    let last = selected(&records, "cd");
    let missing = accept(&mut engine, vec![HistoryChange::Retract(last.clone())]);
    assert_incomplete(&missing);
    assert_subjects(&missing, &["ab"]);
    let restored = accept(&mut engine, vec![HistoryChange::Insert(last)]);
    assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
        initial.family(RiskTheaterFamily::CoverageErosion));
}

#[test]
fn a_missing_chain_does_not_suppress_an_unrelated_familys_supported_findings() {
    let (mut engine, _, records) = original();
    let original_changes: Vec<_> = records.iter().filter(|record| matches!(
        record.fact, HistoryFact::Coverage(CoverageFact::Change { .. })
    )).cloned().collect();
    let missing = accept(&mut engine, original_changes.iter().cloned()
        .map(HistoryChange::Retract).collect());
    assert_incomplete(&missing);
    assert_subjects(&missing, &[]);

    let other = sources("other-", "network.write", 12);
    let independent = feed(&mut engine, &other);
    assert_incomplete(&independent);
    assert_subjects(&independent, &["other-ab", "other-cd"]);
    let absent: BTreeSet<_> = original_changes.iter().map(|record| record.address.clone()).collect();
    for finding in &independent.family(RiskTheaterFamily::CoverageErosion).findings {
        assert!(finding.support.iter().all(|address| !absent.contains(address)));
    }

    let restored = accept(&mut engine, original_changes.into_iter()
        .map(HistoryChange::Insert).collect());
    assert!(restored.family(RiskTheaterFamily::CoverageErosion).complete);
    assert_subjects(&restored, &["ab", "cd", "other-ab", "other-cd"]);
}

#[test]
fn stale_forks_refuse_across_withdrawn_first_middle_and_last_predecessors() {
    for missing_id in ["ab", "bc", "cd"] {
        let (mut engine, _, records) = original();
        let missing = selected(&records, missing_id);
        accept(&mut engine, vec![HistoryChange::Retract(missing.clone())]);
        insert(&mut engine, &[state("e", 12, "tool.write", MediationLevel::ObserveOnly)]);
        insert(&mut engine, &[authorization("allow-de", 13, "de", "d", "e")]);

        let stale = change("de", 14, "tool.write", "c", "e", "allow-de");
        reject(&mut engine, vec![HistoryChange::Insert(stale)], QueryError::ConflictingRecord);

        let lawful = change("de", 14, "tool.write", "d", "e", "allow-de");
        let continued = insert(&mut engine, &[lawful]);
        assert_incomplete(&continued);
        // The new erosion depends on the missing original prefix. Its private
        // predecessor identity cannot be promoted to live finding support.
        assert!(!continued.family(RiskTheaterFamily::CoverageErosion).findings.iter()
            .any(|finding| finding.subject.id == "de"));
        let restored = accept(&mut engine, vec![HistoryChange::Insert(missing)]);
        assert!(restored.family(RiskTheaterFamily::CoverageErosion).complete);
        assert_subjects(&restored, &["ab", "cd", "de"]);
    }
}

#[test]
fn withdrawn_source_identity_cannot_be_rebound_to_erase_coverage_history() {
    let (mut engine, initial, records) = original();
    let original_change = selected(&records, "ab");
    accept(&mut engine, vec![HistoryChange::Retract(original_change.clone())]);
    let mut changed = original_change.clone();
    let HistoryFact::Coverage(CoverageFact::Change { current, .. }) = &mut changed.fact else {
        unreachable!("selected change");
    };
    *current = address(Noun::Profile, "d");
    reject(&mut engine, vec![HistoryChange::Insert(changed)], QueryError::ConflictingRecord);
    let restored = accept(&mut engine, vec![HistoryChange::Insert(original_change)]);
    assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
        initial.family(RiskTheaterFamily::CoverageErosion));
}

#[test]
fn rejected_same_boundary_forks_cannot_install_a_predecessor_or_occupy_an_identity() {
    let (mut engine, _, _) = original();
    insert(&mut engine, &[state("e", 11, "tool.write", MediationLevel::ObserveOnly)]);
    insert(&mut engine, &[authorization("allow-de", 12, "de", "d", "e")]);
    let valid = change("de", 13, "tool.write", "d", "e", "allow-de");
    let ambiguous = change("other-de", 13, "tool.write", "d", "e", "allow-de");
    reject(&mut engine, vec![
        HistoryChange::Insert(valid.clone()),
        HistoryChange::Insert(ambiguous),
    ], QueryError::ConflictingRecord);
    let accepted = insert(&mut engine, &[valid]);
    assert!(accepted.family(RiskTheaterFamily::CoverageErosion).complete);
    assert_subjects(&accepted, &["ab", "cd", "de"]);
}

#[test]
fn every_original_source_can_be_withdrawn_and_restored_without_phantom_support() {
    let records = sources("", "tool.write", 1);
    for source in &records {
        let (mut engine, initial, _) = original();
        let missing = accept(&mut engine, vec![HistoryChange::Retract(source.clone())]);
        assert_incomplete(&missing);
        for finding in &missing.family(RiskTheaterFamily::CoverageErosion).findings {
            assert!(!finding.support.contains(&source.address));
        }
        let restored = accept(&mut engine, vec![HistoryChange::Insert(source.clone())]);
        assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
            initial.family(RiskTheaterFamily::CoverageErosion));
    }
}

#[test]
fn accepted_history_replay_reconstructs_predecessors_through_missing_boundaries() {
    let (mut engine, initial, records) = original();
    let first = selected(&records, "ab");
    let middle = selected(&records, "bc");
    let last = selected(&records, "cd");
    accept(&mut engine, vec![HistoryChange::Retract(middle.clone())]);
    accept(&mut engine, vec![HistoryChange::Retract(first.clone())]);
    accept(&mut engine, vec![HistoryChange::Insert(middle)]);
    accept(&mut engine, vec![HistoryChange::Retract(last.clone())]);
    accept(&mut engine, vec![HistoryChange::Insert(first)]);
    let restored = accept(&mut engine, vec![HistoryChange::Insert(last)]);
    assert_eq!(restored.family(RiskTheaterFamily::CoverageErosion),
        initial.family(RiskTheaterFamily::CoverageErosion));

    let mut replay = new_engine();
    for frontier in engine.source_history() {
        let snapshot = replay.apply_frontier(frontier).unwrap();
        assert_eq!(replay.verify_kernel().unwrap(), *snapshot.kernel());
    }
    assert_eq!(replay.source_history(), engine.source_history());
    assert_eq!(replay.snapshot(restored.sequence()).unwrap(), restored);
}
