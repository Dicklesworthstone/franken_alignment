//! Autonomy corrections cannot erase an original demotion, reset or dwell basis.
//! Public reference-query tests: these supplied histories do not authenticate a
//! live native source, qualify a policy, or grant any effect authority.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::{AutonomyGrade, RiskTheaterFamily};
use fa_reference::governance_assurance::risk_theater_queries::{
    AuditFact, AutonomyFact, HistoryChange, HistoryFact, HistoryFrontier, HistoryRecord,
    QueryEngine, QueryError, QueryPolicy, QuerySnapshot, ReportReference,
};

const FAMILY: RiskTheaterFamily = RiskTheaterFamily::AutonomyInflation;
const EFFECT: &str = "publication";

fn address(kind: Noun, id: &str, generation: u64) -> Address {
    Address::new("autonomy-test", kind, id, Some(generation)).unwrap()
}
fn id(name: &str) -> Address {
    address(Noun::Observation, name, 1)
}
fn scope() -> Address {
    address(Noun::Run, "run", 1)
}
fn principal(name: &str) -> Address {
    address(Noun::Profile, name, 1)
}
fn passport(generation: u64) -> Address {
    address(Noun::Passport, "model", generation)
}
fn evaluator() -> Address {
    address(Noun::Profile, "independent-evaluator", 1)
}
fn new_engine() -> QueryEngine {
    QueryEngine::new(QueryPolicy::new(scope(), 1, evaluator(), 8, 3).unwrap()).unwrap()
}
fn record(name: &str, at: u64, fact: AutonomyFact) -> HistoryRecord {
    HistoryRecord {
        scope: scope(),
        address: id(name),
        occurred_at: at,
        fact: HistoryFact::Autonomy(fact),
    }
}
fn enrollment(name: &str, at: u64, actor: &str, effect: &str) -> HistoryRecord {
    record(name, at, AutonomyFact::Enrollment {
        principal: principal(actor), passport: passport(1),
        effect_family: effect.to_owned(), grade: AutonomyGrade::Propose,
    })
}
fn promotion(
    name: &str, at: u64, previous: &str, model: u64,
    to: AutonomyGrade, report: ReportReference,
) -> HistoryRecord {
    record(name, at, AutonomyFact::Promotion {
        principal: principal("actor"), passport: passport(model),
        effect_family: EFFECT.to_owned(), previous: id(previous), to, report,
    })
}
fn demotion(
    name: &str, at: u64, previous: &str, grade: AutonomyGrade,
) -> HistoryRecord {
    record(name, at, AutonomyFact::Demotion {
        principal: principal("actor"), passport: passport(1),
        effect_family: EFFECT.to_owned(), previous: id(previous), grade,
    })
}
fn report(
    name: &str, at: u64, target: &str, previous: &str,
    from: AutonomyGrade, to: AutonomyGrade,
) -> HistoryRecord {
    record(name, at, AutonomyFact::PromotionReport {
        promotion: id(target), principal: principal("actor"), passport: passport(1),
        effect_family: EFFECT.to_owned(), previous: id(previous), from, to,
        evaluator: evaluator(), attack_conditioned: true,
    })
}
fn accept(engine: &mut QueryEngine, changes: Vec<HistoryChange>) -> QuerySnapshot {
    let snapshot = engine.apply_frontier(&HistoryFrontier {
        sequence: engine.source_history().len() as u64 + 1,
        closed: true,
        changes,
    }).unwrap();
    assert_eq!(engine.verify_kernel().unwrap(), *snapshot.kernel());
    snapshot
}
fn insert(engine: &mut QueryEngine, records: Vec<HistoryRecord>) -> QuerySnapshot {
    accept(engine, records.into_iter().map(HistoryChange::Insert).collect())
}
fn reject(engine: &mut QueryEngine, changes: Vec<HistoryChange>, error: QueryError) {
    let history = engine.source_history().to_vec();
    let sequence = history.len() as u64;
    let previous = engine.snapshot(sequence).unwrap();
    assert_eq!(engine.apply_frontier(&HistoryFrontier {
        sequence: sequence + 1, closed: true, changes,
    }), Err(error));
    assert_eq!(engine.source_history(), history.as_slice());
    assert_eq!(engine.snapshot(sequence).unwrap(), previous);
    assert_eq!(engine.verify_kernel().unwrap(), *previous.kernel());
    assert!(engine.snapshot(sequence + 1).is_err());
}
fn reasons(snapshot: &QuerySnapshot, subject: &str) -> BTreeSet<&'static str> {
    snapshot.family(FAMILY).findings.iter()
        .filter(|finding| finding.subject == id(subject))
        .map(|finding| finding.reason).collect()
}
fn without_support(snapshot: &QuerySnapshot, missing: &HistoryRecord) {
    assert!(!snapshot.family(FAMILY).complete);
    assert!(snapshot.family(FAMILY).findings.iter()
        .all(|finding| !finding.support.contains(&missing.address)));
    assert!(snapshot.kernel().supports.iter()
        .all(|support| support.support_address != missing.address));
}
fn sources() -> Vec<HistoryRecord> {
    vec![
        enrollment("a", 1, "actor", EFFECT),
        report("report-b", 2, "b", "a", AutonomyGrade::Propose, AutonomyGrade::Reviewed),
        promotion("b", 4, "a", 1, AutonomyGrade::Reviewed, ReportReference::Present(id("report-b"))),
        demotion("c", 5, "b", AutonomyGrade::Propose),
        report("report-d", 6, "d", "c", AutonomyGrade::Propose, AutonomyGrade::Sampled),
        promotion("d", 7, "c", 1, AutonomyGrade::Sampled, ReportReference::Present(id("report-d"))),
    ]
}
fn selected(records: &[HistoryRecord], name: &str) -> HistoryRecord {
    records.iter().find(|record| record.address == id(name)).unwrap().clone()
}
fn original() -> (QueryEngine, QuerySnapshot, Vec<HistoryRecord>) {
    let records = sources();
    let mut engine = new_engine();
    for sequence in 1..=7 {
        insert(&mut engine, records.iter()
            .filter(|record| record.occurred_at == sequence).cloned().collect());
    }
    let snapshot = engine.snapshot(7).unwrap();
    assert!(snapshot.family(FAMILY).complete);
    assert_eq!(reasons(&snapshot, "d"), BTreeSet::from([
        "autonomy_dwell_violation", "autonomy_grade_skipped",
    ]));
    assert_eq!(snapshot.family(FAMILY).findings.len(), 2);
    (engine, snapshot, records)
}

#[test]
fn withdrawn_terminal_state_cannot_supply_a_clean_cut_or_an_older_dwell_basis() {
    let (mut engine, before, records) = original();
    let last = selected(&records, "d");
    let missing = accept(&mut engine, vec![HistoryChange::Retract(last.clone())]);
    without_support(&missing, &last);
    assert!(missing.family(FAMILY).findings.is_empty());

    let stale = promotion("e", 9, "c", 1, AutonomyGrade::Bounded, ReportReference::Unavailable);
    reject(&mut engine, vec![HistoryChange::Insert(stale)], QueryError::InvalidReference);
    let lawful = promotion("e", 9, "d", 1, AutonomyGrade::Bounded, ReportReference::Unavailable);
    let continued = insert(&mut engine, vec![lawful]);
    without_support(&continued, &last);
    assert!(continued.family(FAMILY).findings.is_empty());

    let restored = insert(&mut engine, vec![last]);
    assert_eq!(reasons(&restored, "d"), reasons(&before, "d"));
    assert_eq!(reasons(&restored, "e"), BTreeSet::from(["autonomy_dwell_violation"]));
    // The originally unavailable new report remains unavailable.
    assert!(!restored.family(FAMILY).complete);
}

#[test]
fn exact_withdrawal_and_restoration_never_publish_a_missing_state_as_live_support() {
    let records = sources();
    for source in &records {
        let (mut engine, original, _) = original();
        let missing = accept(&mut engine, vec![HistoryChange::Retract(source.clone())]);
        without_support(&missing, source);
        let restored = insert(&mut engine, vec![source.clone()]);
        assert_eq!(restored.family(FAMILY), original.family(FAMILY));
    }
}

#[test]
fn independent_direct_dwell_and_grade_findings_survive_an_earlier_missing_state() {
    for missing_name in ["a", "b"] {
        let (mut engine, original, records) = original();
        let missing = selected(&records, missing_name);
        let snapshot = accept(&mut engine, vec![HistoryChange::Retract(missing.clone())]);
        without_support(&snapshot, &missing);
        assert_eq!(snapshot.family(FAMILY).findings, original.family(FAMILY).findings);
        for finding in &snapshot.family(FAMILY).findings {
            assert_eq!(finding.support, vec![id("c"), id("d")]);
        }
    }
}

#[test]
fn withdrawing_every_state_cannot_reenroll_the_same_principal_and_family() {
    let (mut engine, _, records) = original();
    let states: Vec<_> = records.iter().filter(|record| !matches!(
        record.fact, HistoryFact::Autonomy(AutonomyFact::PromotionReport { .. })
    )).cloned().collect();
    let missing = accept(&mut engine, states.iter().cloned()
        .map(HistoryChange::Retract).collect());
    assert!(!missing.family(FAMILY).complete);
    assert!(missing.family(FAMILY).findings.is_empty());
    reject(&mut engine, vec![HistoryChange::Insert(
        enrollment("replacement", 9, "actor", EFFECT),
    )], QueryError::ConflictingRecord);

    // Distinct principal and effect domains remain usable at the same cut.
    let independent = insert(&mut engine, vec![
        enrollment("other-actor", 9, "other-actor", EFFECT),
        enrollment("other-effect", 9, "actor", "different-effect"),
    ]);
    assert!(!independent.family(FAMILY).complete);
    let restored = insert(&mut engine, states);
    assert!(restored.family(FAMILY).complete);
    assert_eq!(reasons(&restored, "d").len(), 2);
}

#[test]
fn missing_demotion_cannot_hide_an_independent_report_violation_or_erase_its_reset() {
    let records = sources();
    let mut engine = new_engine();
    for sequence in 1..=5 {
        insert(&mut engine, records.iter()
            .filter(|record| record.occurred_at == sequence).cloned().collect());
    }
    let demoted = selected(&records, "c");
    accept(&mut engine, vec![HistoryChange::Retract(demoted.clone())]);
    let absent = ReportReference::Absent { closed_through: 7 };
    reject(&mut engine, vec![HistoryChange::Insert(
        promotion("after", 7, "b", 1, AutonomyGrade::Sampled, absent.clone()),
    )], QueryError::InvalidReference);
    let continued = insert(&mut engine, vec![
        promotion("after", 7, "c", 1, AutonomyGrade::Sampled, absent),
    ]);
    without_support(&continued, &demoted);
    assert_eq!(reasons(&continued, "after"), BTreeSet::from([
        "promotion_without_qualified_report",
    ]));
    let restored = insert(&mut engine, vec![demoted]);
    assert!(restored.family(FAMILY).complete);
    assert_eq!(reasons(&restored, "after"), BTreeSet::from([
        "autonomy_dwell_violation", "autonomy_grade_skipped",
        "promotion_without_qualified_report",
    ]));
}

#[test]
fn a_withdrawn_passport_reset_cannot_restore_old_passport_or_dwell() {
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("a", 1, "actor", EFFECT)]);
    let reset = record("reset", 2, AutonomyFact::PassportChanged {
        principal: principal("actor"), passport: passport(2),
        effect_family: EFFECT.to_owned(), previous: id("a"),
    });
    insert(&mut engine, vec![reset.clone()]);
    accept(&mut engine, vec![HistoryChange::Retract(reset.clone())]);

    reject(&mut engine, vec![HistoryChange::Insert(
        promotion("new", 4, "a", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable),
    )], QueryError::InvalidReference);
    reject(&mut engine, vec![HistoryChange::Insert(
        promotion("new", 4, "reset", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable),
    )], QueryError::InvalidReference);

    let lawful = insert(&mut engine, vec![promotion(
        "new", 4, "reset", 2, AutonomyGrade::Reviewed, ReportReference::Unavailable,
    )]);
    without_support(&lawful, &reset);
    assert!(lawful.family(FAMILY).findings.is_empty());
    let restored = insert(&mut engine, vec![reset]);
    assert_eq!(reasons(&restored, "new"), BTreeSet::from(["autonomy_dwell_violation"]));
}

#[test]
fn same_boundary_withdrawal_cannot_hide_an_ambiguous_trajectory_or_occupy_a_refused_id() {
    let (mut engine, _, _) = original();
    let one = demotion("one", 8, "d", AutonomyGrade::Reviewed);
    let two = demotion("two", 8, "d", AutonomyGrade::Propose);
    reject(&mut engine, vec![
        HistoryChange::Insert(one.clone()), HistoryChange::Insert(two),
        HistoryChange::Retract(one),
    ], QueryError::ConflictingRecord);
    // The refused boundary did not install the first proposed binding for one.
    let accepted = insert(&mut engine, vec![demotion("one", 8, "d", AutonomyGrade::Propose)]);
    assert!(accepted.family(FAMILY).complete);
}

#[test]
fn a_later_predicate_refusal_cannot_install_an_autonomy_predecessor() {
    let (mut engine, before, _) = original();
    let raw = |name: &str, fact| HistoryRecord {
        scope: scope(), address: id(name), occurred_at: 8, fact: HistoryFact::Audit(fact),
    };
    reject(&mut engine, vec![
        HistoryChange::Insert(demotion("next", 8, "d", AutonomyGrade::Reviewed)),
        HistoryChange::Insert(raw("audit", AuditFact::Scheduled {
            effect_family: EFFECT.to_owned(), required_units: 1, due_at: 8,
        })),
        HistoryChange::Insert(raw("full", AuditFact::Funding { audit: id("audit"), units: u64::MAX })),
        HistoryChange::Insert(raw("overflow", AuditFact::Funding { audit: id("audit"), units: 1 })),
    ], QueryError::ArithmeticOverflow);
    let accepted = insert(&mut engine, vec![demotion("next", 8, "d", AutonomyGrade::Propose)]);
    assert_eq!(accepted.family(FAMILY).findings, before.family(FAMILY).findings);
}

#[test]
fn withdrawn_predecessors_still_reject_contradictory_grades_and_report_bindings() {
    let (mut engine, _, records) = original();
    let last = selected(&records, "d");
    accept(&mut engine, vec![HistoryChange::Retract(last.clone())]);
    reject(&mut engine, vec![HistoryChange::Insert(
        demotion("bad", 9, "d", AutonomyGrade::Sampled),
    )], QueryError::InvalidRecord);
    insert(&mut engine, vec![promotion(
        "new", 9, "d", 1, AutonomyGrade::Bounded, ReportReference::Present(id("late")),
    )]);
    reject(&mut engine, vec![HistoryChange::Insert(report(
        "late", 10, "new", "d", AutonomyGrade::Reviewed, AutonomyGrade::Bounded,
    ))], QueryError::InvalidReference);
    let labeled = insert(&mut engine, vec![report(
        "late", 10, "new", "d", AutonomyGrade::Sampled, AutonomyGrade::Bounded,
    )]);
    without_support(&labeled, &last);
    assert_eq!(reasons(&labeled, "new"), BTreeSet::from([
        "promotion_without_qualified_report",
    ]));
    let restored = insert(&mut engine, vec![last]);
    assert!(restored.family(FAMILY).complete);
    assert_eq!(reasons(&restored, "new"), BTreeSet::from([
        "autonomy_dwell_violation", "promotion_without_qualified_report",
    ]));
}

#[test]
fn a_missing_named_predecessor_cannot_displace_an_observed_immediate_state() {
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("a", 1, "actor", EFFECT)]);
    reject(&mut engine, vec![HistoryChange::Insert(promotion(
        "new", 2, "invented", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable,
    ))], QueryError::InvalidReference);
    let observed = insert(&mut engine, vec![promotion(
        "new", 2, "a", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable,
    )]);
    assert_eq!(reasons(&observed, "new"), BTreeSet::from(["autonomy_dwell_violation"]));
}

#[test]
fn accepted_history_replay_preserves_original_trajectory_and_exact_corrections() {
    let (mut engine, original, records) = original();
    let first = selected(&records, "a");
    let middle = selected(&records, "c");
    let last = selected(&records, "d");
    accept(&mut engine, vec![HistoryChange::Retract(middle.clone())]);
    accept(&mut engine, vec![HistoryChange::Retract(first.clone())]);
    accept(&mut engine, vec![HistoryChange::Retract(last.clone())]);
    insert(&mut engine, vec![middle]);
    insert(&mut engine, vec![last]);
    let restored = insert(&mut engine, vec![first]);
    assert_eq!(restored.family(FAMILY), original.family(FAMILY));

    let mut replay = new_engine();
    for frontier in engine.source_history() {
        let snapshot = replay.apply_frontier(frontier).unwrap();
        assert_eq!(replay.verify_kernel().unwrap(), *snapshot.kernel());
    }
    assert_eq!(replay.source_history(), engine.source_history());
    assert_eq!(replay.snapshot(restored.sequence()).unwrap(), restored);
}
