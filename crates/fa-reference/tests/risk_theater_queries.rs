//! Semantic regressions for the typed reference control-history queries.
//! These histories are supplied assertions, not authenticated native execution.
//! Expected findings below are independent, hand-specified predicate outcomes.
#![forbid(unsafe_code)]

use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::{
    AutonomyGrade, RiskTheaterFamily, MAX_RISK_THEATER_HISTORY_DELTAS,
};
use fa_reference::governance_assurance::risk_theater_queries::{
    AuditFact, AutonomyFact, HistoryChange, HistoryFact, HistoryFrontier, HistoryRecord,
    HoldLabel, QueryEngine, QueryError, QueryPolicy, QuerySnapshot, ReportReference,
    RosterFact, MAX_QUERY_RECORDS,
};

const EFFECT: &str = "file.write";

fn address(kind: Noun, name: &str, generation: u64) -> Address {
    Address::new("tenant", kind, name, Some(generation)).expect("canonical address")
}
fn scope() -> Address {
    address(Noun::Run, "run", 1)
}
fn id(name: &str) -> Address {
    address(Noun::Observation, name, 1)
}
fn helper() -> Address {
    address(Noun::Helper, "helper", 1)
}
fn evaluator() -> Address {
    address(Noun::Profile, "evaluation-owner", 1)
}
fn principal() -> Address {
    address(Noun::Profile, "actor", 1)
}
fn passport(generation: u64) -> Address {
    address(Noun::Passport, "model", generation)
}
fn policy() -> QueryPolicy {
    QueryPolicy::new(scope(), 7, evaluator(), 100, 3).expect("policy")
}
fn new_engine() -> QueryEngine {
    QueryEngine::new(policy()).expect("engine")
}
fn record(name: &str, at: u64, fact: HistoryFact) -> HistoryRecord {
    HistoryRecord { scope: scope(), address: id(name), occurred_at: at, fact }
}
fn cut(sequence: u64, changes: Vec<HistoryChange>) -> HistoryFrontier {
    HistoryFrontier { sequence, closed: true, changes }
}
fn insert(engine: &mut QueryEngine, records: Vec<HistoryRecord>) -> QuerySnapshot {
    let sequence = engine.source_history().len() as u64 + 1;
    engine.apply_frontier(&cut(
        sequence, records.into_iter().map(HistoryChange::Insert).collect(),
    )).expect("complete admitted boundary")
}
fn advance_to(engine: &mut QueryEngine, next: u64) {
    while engine.source_history().len() as u64 + 1 < next {
        insert(engine, Vec::new());
    }
}
fn absent(at: u64) -> ReportReference {
    ReportReference::Absent { closed_through: at }
}
fn hold(name: &str, at: u64) -> HistoryRecord {
    record(name, at, HistoryFact::Roster(RosterFact::Hold {
        helper: helper(), effect_family: EFFECT.to_owned(),
    }))
}
fn assessment(name: &str, at: u64, target: &str, label: HoldLabel) -> HistoryRecord {
    record(name, at, HistoryFact::Roster(RosterFact::Assessment {
        hold: id(target), evaluator: evaluator(), label,
    }))
}
fn retirement(name: &str, at: u64, report: ReportReference) -> HistoryRecord {
    record(name, at, HistoryFact::Roster(RosterFact::Retirement {
        helper: helper(), effect_family: EFFECT.to_owned(), report,
    }))
}
fn selection_report(name: &str, at: u64, target: &str) -> HistoryRecord {
    record(name, at, HistoryFact::Roster(RosterFact::SelectionReport {
        retirement: id(target), helper: helper(), effect_family: EFFECT.to_owned(),
        evaluator: evaluator(), attack_conditioned: true,
    }))
}
fn enrollment(name: &str, at: u64, model: u64, grade: AutonomyGrade) -> HistoryRecord {
    record(name, at, HistoryFact::Autonomy(AutonomyFact::Enrollment {
        principal: principal(), passport: passport(model),
        effect_family: EFFECT.to_owned(), grade,
    }))
}
fn promotion(
    name: &str, at: u64, previous: &str, model: u64,
    to: AutonomyGrade, report: ReportReference,
) -> HistoryRecord {
    record(name, at, HistoryFact::Autonomy(AutonomyFact::Promotion {
        principal: principal(), passport: passport(model), effect_family: EFFECT.to_owned(),
        previous: id(previous), to, report,
    }))
}
fn promotion_report(
    name: &str, at: u64, target: &str, previous: &str, model: u64,
    from: AutonomyGrade, to: AutonomyGrade,
) -> HistoryRecord {
    record(name, at, HistoryFact::Autonomy(AutonomyFact::PromotionReport {
        promotion: id(target), principal: principal(), passport: passport(model),
        effect_family: EFFECT.to_owned(), previous: id(previous), from, to,
        evaluator: evaluator(), attack_conditioned: true,
    }))
}
fn audit(name: &str, at: u64, due: u64, units: u64) -> HistoryRecord {
    record(name, at, HistoryFact::Audit(AuditFact::Scheduled {
        effect_family: EFFECT.to_owned(), required_units: units, due_at: due,
    }))
}
fn funding(name: &str, at: u64, target: &str, units: u64) -> HistoryRecord {
    record(name, at, HistoryFact::Audit(AuditFact::Funding { audit: id(target), units }))
}
fn rejected(
    engine: &mut QueryEngine, frontier: HistoryFrontier, error: QueryError,
) {
    let previous_seq = engine.source_history().len() as u64;
    let previous = engine.snapshot(previous_seq).expect("preceding cut");
    let history = engine.source_history().to_vec();
    assert_eq!(engine.apply_frontier(&frontier), Err(error));
    assert_eq!(engine.snapshot(previous_seq), Ok(previous));
    assert_eq!(engine.source_history(), history);
    if frontier.sequence != previous_seq {
        assert!(engine.snapshot(frontier.sequence).is_err());
    }
}

#[test]
fn closed_empty_reference_domains_are_distinct_from_uninstalled_families() {
    let mut engine = new_engine();
    let snapshot = insert(&mut engine, Vec::new());
    assert_eq!(snapshot.sequence(), 1);
    assert_eq!(snapshot.scope(), &scope());
    assert_eq!(snapshot.policy_generation(), 7);
    assert_eq!(snapshot.policy(), &policy());
    for family in RiskTheaterFamily::ALL {
        let expected = matches!(family,
            RiskTheaterFamily::RosterChurnNearHolds
            | RiskTheaterFamily::AutonomyInflation
            | RiskTheaterFamily::AuditFloorStarvation);
        assert_eq!(snapshot.family(family).complete, expected);
        assert!(snapshot.family(family).findings.is_empty());
    }
    assert_eq!(engine.verify_kernel(), Ok(snapshot.kernel().clone()));
}

#[test]
fn retirement_with_a_prior_bound_attack_report_has_a_causal_missing_report_twin() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    for carried in [true, false] {
        let mut engine = new_engine();
        insert(&mut engine, vec![hold("hold", 1)]);
        insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
        // The report exists in both histories. Only its carriage by the exact
        // retirement changes. An unused report cannot approve that transition.
        insert(&mut engine, vec![selection_report("report", 3, "retire")]);
        let reference = if carried { ReportReference::Present(id("report")) } else { absent(4) };
        let snapshot = insert(&mut engine, vec![retirement("retire", 4, reference)]);
        assert!(snapshot.family(family).complete);
        assert_eq!(snapshot.family(family).findings.len(), usize::from(!carried));
        assert_eq!(snapshot.kernel().scores[&family], i64::from(!carried));
        if !carried {
            let finding = &snapshot.family(family).findings[0];
            assert_eq!(finding.subject, id("retire"));
            assert_eq!(finding.support, vec![id("hold"), id("label"), id("retire")]);
            assert_eq!(finding.reason, "retirement_without_qualified_report");
        }
        assert_eq!(engine.verify_kernel(), Ok(snapshot.kernel().clone()));
    }
}

#[test]
fn hold_truth_requires_the_configured_evaluator_and_preserves_censoring() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    for (label, foreign_owner, complete, findings) in [
        (HoldLabel::ConfirmedViolation, false, true, 1),
        (HoldLabel::Benign, false, true, 0),
        (HoldLabel::Censored, false, false, 0),
        (HoldLabel::ConfirmedViolation, true, false, 0),
    ] {
        let mut engine = new_engine();
        insert(&mut engine, vec![hold("hold", 1)]);
        let mut report = assessment("label", 2, "hold", label);
        if foreign_owner {
            let HistoryFact::Roster(RosterFact::Assessment { evaluator, .. }) = &mut report.fact
                else { unreachable!() };
            *evaluator = address(Noun::Profile, "actor-written-label", 1);
        }
        insert(&mut engine, vec![report]);
        let snapshot = insert(&mut engine, vec![retirement("retire", 3, absent(3))]);
        assert_eq!(snapshot.family(family).complete, complete);
        assert_eq!(snapshot.family(family).findings.len(), findings);
    }
}

#[test]
fn late_hold_confirmation_opens_and_exact_correction_retracts_the_original_finding() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    let pending = insert(&mut engine, vec![retirement("retire", 2, absent(2))]);
    assert!(!pending.family(family).complete);
    assert!(pending.family(family).findings.is_empty());
    let label = assessment("label", 3, "hold", HoldLabel::ConfirmedViolation);
    let opened = insert(&mut engine, vec![label.clone()]);
    assert_eq!(opened.family(family).findings.len(), 1);
    let corrected = engine.apply_frontier(&cut(4, vec![HistoryChange::Retract(label.clone())]))
        .expect("exact correction");
    assert!(!corrected.family(family).complete);
    assert_eq!(corrected.kernel().scores[&family], 0);
    assert!(corrected.kernel().supports.is_empty());
    assert_eq!(engine.source_history()[2].changes, vec![HistoryChange::Insert(label.clone())]);
    let restored = engine.apply_frontier(&cut(5, vec![HistoryChange::Insert(label.clone())]))
        .expect("restore exact previously recorded observation");
    assert_eq!(restored.family(family), opened.family(family));
    assert_eq!(engine.verify_kernel(), Ok(restored.kernel().clone()));
    let mut changed = label.clone();
    let HistoryFact::Roster(RosterFact::Assessment { label: value, .. }) = &mut changed.fact
        else { unreachable!() };
    *value = HoldLabel::Benign;
    rejected(&mut engine, cut(6, vec![
        HistoryChange::Retract(label), HistoryChange::Insert(changed),
    ]), QueryError::ConflictingRecord);
}

#[test]
fn censored_holds_resolve_to_independent_violation_or_benign_without_rewriting_history() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    for (label, expected_findings) in [
        (HoldLabel::ConfirmedViolation, 1),
        (HoldLabel::Benign, 0),
    ] {
        let mut engine = new_engine();
        insert(&mut engine, vec![hold("hold", 1)]);
        let censored = assessment("censored", 2, "hold", HoldLabel::Censored);
        insert(&mut engine, vec![censored.clone()]);
        let pending = insert(&mut engine, vec![retirement("retire", 3, absent(3))]);
        assert!(!pending.family(family).complete);
        assert!(pending.family(family).findings.is_empty());

        let resolved = assessment("resolved", 4, "hold", label);
        let snapshot = insert(&mut engine, vec![resolved.clone()]);
        assert!(snapshot.family(family).complete);
        assert_eq!(snapshot.family(family).findings.len(), expected_findings);
        assert_eq!(snapshot.kernel().scores[&family], expected_findings as i64);
        assert_eq!(engine.source_history()[1].changes, vec![HistoryChange::Insert(censored)]);
        assert_eq!(engine.source_history()[3].changes, vec![HistoryChange::Insert(resolved)]);
        if expected_findings != 0 {
            let finding = &snapshot.family(family).findings[0];
            assert_eq!(finding.subject, id("retire"));
            assert_eq!(finding.support, vec![id("censored"), id("hold"), id("resolved"), id("retire")]);
        }
        assert_eq!(engine.verify_kernel(), Ok(snapshot.kernel().clone()));
    }
}

#[test]
fn resolution_and_original_hold_retractions_preserve_censoring_and_restore_exact_support() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut engine = new_engine();
    let original = hold("hold", 1);
    insert(&mut engine, vec![original.clone()]);
    insert(&mut engine, vec![assessment("censored", 2, "hold", HoldLabel::Censored)]);
    insert(&mut engine, vec![retirement("retire", 3, absent(3))]);
    let resolution = assessment("resolved", 4, "hold", HoldLabel::ConfirmedViolation);
    let opened = insert(&mut engine, vec![resolution.clone()]);
    for (sequence, withdrawn) in [(5, resolution), (7, original)] {
        let corrected = engine.apply_frontier(&cut(sequence, vec![
            HistoryChange::Retract(withdrawn.clone()),
        ])).expect("withdraw exact source observation");
        assert!(!corrected.family(family).complete);
        assert!(corrected.family(family).findings.is_empty());
        assert_eq!(corrected.kernel().scores[&family], 0);
        assert!(corrected.kernel().supports.is_empty());
        assert_eq!(engine.verify_kernel(), Ok(corrected.kernel().clone()));
        let restored = engine.apply_frontier(&cut(sequence + 1, vec![
            HistoryChange::Insert(withdrawn),
        ])).expect("restore original source identity");
        assert_eq!(restored.family(family), opened.family(family));
        assert_eq!(engine.verify_kernel(), Ok(restored.kernel().clone()));
    }
}

#[test]
fn only_one_final_roster_label_is_allowed_and_censoring_resolves_only_once() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let labels = [HoldLabel::Censored, HoldLabel::Benign, HoldLabel::ConfirmedViolation];
    for first in labels {
        for second in labels {
            let mut engine = new_engine();
            insert(&mut engine, vec![hold("hold", 1)]);
            insert(&mut engine, vec![assessment("first", 2, "hold", first)]);
            let next = cut(3, vec![HistoryChange::Insert(assessment("second", 3, "hold", second))]);
            if first == HoldLabel::Censored && second != HoldLabel::Censored {
                engine.apply_frontier(&next).expect("one final independent resolution");
                let snapshot = insert(&mut engine, vec![retirement("retire", 4, absent(4))]);
                assert!(snapshot.family(family).complete);
                assert_eq!(snapshot.family(family).findings.len(), usize::from(second == HoldLabel::ConfirmedViolation));
            } else {
                // This includes a new record repeating the same final label:
                // retrying an observation cannot manufacture another sample.
                rejected(&mut engine, next, QueryError::ConflictingRecord);
            }
        }
    }
}

#[test]
fn same_boundary_censoring_and_resolution_refuse_but_a_later_resolution_is_admitted() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![retirement("retire", 2, absent(2))]);
    let censored = assessment("censored", 3, "hold", HoldLabel::Censored);
    rejected(&mut engine, cut(3, vec![
        HistoryChange::Insert(censored.clone()),
        HistoryChange::Insert(assessment("resolved", 3, "hold", HoldLabel::ConfirmedViolation)),
    ]), QueryError::ConflictingRecord);
    let pending = insert(&mut engine, vec![censored]);
    assert!(!pending.family(family).complete);
    let resolved = insert(&mut engine, vec![assessment("resolved", 4, "hold", HoldLabel::ConfirmedViolation)]);
    assert!(resolved.family(family).complete);
    assert_eq!(resolved.family(family).findings.len(), 1);
}

#[test]
fn a_foreign_resolution_cannot_supply_truth_and_withdrawal_recovers_the_original_label_chain() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    for wrong_owner in [
        address(Noun::Profile, "actor-written-label", 1),
        address(Noun::Profile, "evaluation-owner", 2),
    ] {
        let mut engine = new_engine();
        insert(&mut engine, vec![hold("hold", 1)]);
        insert(&mut engine, vec![assessment("censored", 2, "hold", HoldLabel::Censored)]);
        insert(&mut engine, vec![retirement("retire", 3, absent(3))]);
        let mut foreign = assessment("foreign", 4, "hold", HoldLabel::ConfirmedViolation);
        let HistoryFact::Roster(RosterFact::Assessment { evaluator, .. }) = &mut foreign.fact
            else { unreachable!() };
        *evaluator = wrong_owner;
        let unavailable = insert(&mut engine, vec![foreign.clone()]);
        assert!(!unavailable.family(family).complete);
        assert!(unavailable.family(family).findings.is_empty());
        let still_unavailable = insert(&mut engine, vec![
            assessment("resolved", 5, "hold", HoldLabel::ConfirmedViolation),
        ]);
        assert!(!still_unavailable.family(family).complete);
        assert!(still_unavailable.family(family).findings.is_empty());
        let corrected = engine.apply_frontier(&cut(6, vec![HistoryChange::Retract(foreign)]))
            .expect("withdraw unsupported evaluator observation");
        assert!(corrected.family(family).complete);
        assert_eq!(corrected.family(family).findings[0].support, vec![
            id("censored"), id("hold"), id("resolved"), id("retire"),
        ]);
        assert_eq!(engine.verify_kernel(), Ok(corrected.kernel().clone()));
    }
}

#[test]
fn missing_holds_or_foreign_evaluators_cannot_hide_conflicting_final_labels() {
    for missing_hold in [false, true] {
        for foreign_owner in [false, true] {
            let mut engine = new_engine();
            let original = hold("hold", 1);
            insert(&mut engine, vec![original.clone()]);
            insert(&mut engine, vec![assessment("final", 2, "hold", HoldLabel::ConfirmedViolation)]);
            let mut changes = Vec::new();
            if missing_hold {
                changes.push(HistoryChange::Retract(original));
            }
            if foreign_owner {
                let mut foreign = assessment("foreign", 3, "hold", HoldLabel::Censored);
                let HistoryFact::Roster(RosterFact::Assessment { evaluator, .. }) = &mut foreign.fact
                    else { unreachable!() };
                *evaluator = address(Noun::Profile, "wrong-owner", 1);
                changes.push(HistoryChange::Insert(foreign));
            }
            engine.apply_frontier(&cut(3, changes)).expect("honest partial source view");
            // There is not even a retirement yet. Validate the original label
            // chain now, rather than hiding corruption behind query selection.
            rejected(&mut engine, cut(4, vec![HistoryChange::Insert(
                assessment("conflicting", 4, "hold", HoldLabel::Benign),
            )]), QueryError::ConflictingRecord);
        }
    }
}

#[test]
fn qualified_retirement_still_accepts_delayed_resolution_and_rejects_later_equivocation() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("censored", 2, "hold", HoldLabel::Censored)]);
    insert(&mut engine, vec![selection_report("report", 3, "retire")]);
    let permitted = insert(&mut engine, vec![retirement("retire", 4, ReportReference::Present(id("report")))]);
    assert!(permitted.family(family).complete);
    assert!(permitted.family(family).findings.is_empty());
    let resolved = insert(&mut engine, vec![assessment("resolved", 5, "hold", HoldLabel::ConfirmedViolation)]);
    assert!(resolved.family(family).complete);
    assert!(resolved.family(family).findings.is_empty());
    rejected(&mut engine, cut(6, vec![HistoryChange::Insert(
        assessment("conflicting", 6, "hold", HoldLabel::Benign),
    )]), QueryError::ConflictingRecord);
}

#[test]
fn an_orphan_assessment_cannot_become_a_resolution_of_a_later_hold() {
    for foreign_owner in [false, true] {
        let mut engine = new_engine();
        let mut label = assessment("label", 1, "hold", HoldLabel::Censored);
        if foreign_owner {
            let HistoryFact::Roster(RosterFact::Assessment { evaluator, .. }) = &mut label.fact
                else { unreachable!() };
            *evaluator = address(Noun::Profile, "wrong-owner", 1);
        }
        let pending = insert(&mut engine, vec![label]);
        assert!(!pending.family(RiskTheaterFamily::RosterChurnNearHolds).complete);
        rejected(&mut engine, cut(2, vec![HistoryChange::Insert(hold("hold", 2))]), QueryError::InvalidReference);
    }
}

#[test]
fn a_late_or_unqualified_selection_report_cannot_retroactively_authorize_retirement() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
    let missing = insert(&mut engine, vec![
        retirement("retire", 3, ReportReference::Present(id("report"))),
    ]);
    assert!(!missing.family(family).complete);
    assert!(missing.family(family).findings.is_empty());
    let late = insert(&mut engine, vec![selection_report("report", 4, "retire")]);
    assert!(late.family(family).complete);
    assert_eq!(late.family(family).findings.len(), 1);
    assert!(late.family(family).findings[0].support.contains(&id("report")));

    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
    let mut report = selection_report("report", 3, "retire");
    let HistoryFact::Roster(RosterFact::SelectionReport { attack_conditioned, .. }) =
        &mut report.fact else { unreachable!() };
    *attack_conditioned = false;
    insert(&mut engine, vec![report]);
    let result = insert(&mut engine, vec![
        retirement("retire", 4, ReportReference::Present(id("report"))),
    ]);
    assert_eq!(result.family(family).findings.len(), 1);
}

#[test]
fn roster_generation_family_and_window_are_exact_and_old_findings_survive_rotation() {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let policy = QueryPolicy::new(scope(), 7, evaluator(), 2, 3).expect("window");
    let mut engine = QueryEngine::new(policy).expect("engine");
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
    let opened = insert(&mut engine, vec![retirement("retire-old", 3, absent(3))]);
    assert_eq!(opened.family(family).findings.len(), 1);
    let mut unrelated = retirement("retire-new", 4, absent(4));
    let HistoryFact::Roster(RosterFact::Retirement { helper, .. }) = &mut unrelated.fact
        else { unreachable!() };
    helper.generation = Some(2);
    let after_rotation = insert(&mut engine, vec![unrelated]);
    assert_eq!(after_rotation.family(family).findings, opened.family(family).findings);

    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
    let mut other_family = retirement("retire", 3, absent(3));
    let HistoryFact::Roster(RosterFact::Retirement { effect_family, .. }) =
        &mut other_family.fact else { unreachable!() };
    *effect_family = "network.send".to_owned();
    assert!(insert(&mut engine, vec![other_family]).family(family).findings.is_empty());
}

#[test]
fn a_report_for_another_retirement_is_rejected_without_mutating_the_cut() {
    let mut engine = new_engine();
    insert(&mut engine, vec![hold("hold", 1)]);
    insert(&mut engine, vec![assessment("label", 2, "hold", HoldLabel::ConfirmedViolation)]);
    insert(&mut engine, vec![selection_report("report", 3, "different-retirement")]);
    rejected(&mut engine, cut(4, vec![HistoryChange::Insert(
        retirement("retire", 4, ReportReference::Present(id("report"))),
    )]), QueryError::InvalidReference);
}

#[test]
fn lawful_autonomy_promotion_and_early_promotion_differ_only_in_original_dwell() {
    let family = RiskTheaterFamily::AutonomyInflation;
    for at in [3, 4] {
        let mut engine = new_engine();
        insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
        insert(&mut engine, vec![promotion_report(
            "report", 2, "promote", "initial", 1, AutonomyGrade::Propose, AutonomyGrade::Reviewed,
        )]);
        advance_to(&mut engine, at);
        let snapshot = insert(&mut engine, vec![promotion(
            "promote", at, "initial", 1, AutonomyGrade::Reviewed,
            ReportReference::Present(id("report")),
        )]);
        assert!(snapshot.family(family).complete);
        assert_eq!(snapshot.family(family).findings.len(), usize::from(at == 3));
        if at == 3 {
            assert_eq!(snapshot.family(family).findings[0].reason, "autonomy_dwell_violation");
            assert_eq!(snapshot.family(family).findings[0].support, vec![id("initial"), id("promote")]);
        }
        assert_eq!(engine.verify_kernel(), Ok(snapshot.kernel().clone()));
    }
}

#[test]
fn a_proven_dwell_violation_survives_an_unavailable_report_without_claiming_completeness() {
    let family = RiskTheaterFamily::AutonomyInflation;
    for report in [ReportReference::Unavailable, ReportReference::Present(id("lost-report"))] {
        let mut engine = new_engine();
        insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
        insert(&mut engine, Vec::new());
        let snapshot = insert(&mut engine, vec![
            promotion("promote", 3, "initial", 1, AutonomyGrade::Reviewed, report),
        ]);
        assert!(!snapshot.family(family).complete);
        assert_eq!(snapshot.family(family).findings.len(), 1);
        assert_eq!(snapshot.family(family).findings[0].reason, "autonomy_dwell_violation");
    }
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
    advance_to(&mut engine, 3);
    let snapshot = insert(&mut engine, vec![
        promotion("promote", 3, "initial", 1, AutonomyGrade::Reviewed, absent(3)),
    ]);
    assert!(snapshot.family(family).complete);
    assert_eq!(snapshot.family(family).findings.len(), 2);
}

#[test]
fn immediate_grade_predecessor_prevents_dwell_laundering_and_skipped_grades_are_findings() {
    let family = RiskTheaterFamily::AutonomyInflation;
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
    advance_to(&mut engine, 4);
    insert(&mut engine, vec![
        promotion("first", 4, "initial", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable),
    ]);
    advance_to(&mut engine, 7);
    rejected(&mut engine, cut(7, vec![HistoryChange::Insert(
        promotion("second", 7, "initial", 1, AutonomyGrade::Sampled, ReportReference::Unavailable),
    )]), QueryError::InvalidReference);
    let actual = insert(&mut engine, vec![
        promotion("second", 7, "first", 1, AutonomyGrade::Sampled, ReportReference::Unavailable),
    ]);
    assert!(!actual.family(family).complete);
    assert!(actual.family(family).findings.is_empty());

    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
    advance_to(&mut engine, 4);
    let leap = insert(&mut engine, vec![
        promotion("leap", 4, "initial", 1, AutonomyGrade::Bounded, ReportReference::Unavailable),
    ]);
    assert!(!leap.family(family).complete);
    assert_eq!(leap.family(family).findings[0].reason, "autonomy_grade_skipped");
}

#[test]
fn passport_reset_restarts_dwell_and_old_passport_evidence_cannot_cross_it() {
    let family = RiskTheaterFamily::AutonomyInflation;
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Reviewed)]);
    insert(&mut engine, vec![record("changed", 2, HistoryFact::Autonomy(
        AutonomyFact::PassportChanged {
            principal: principal(), passport: passport(2),
            effect_family: EFFECT.to_owned(), previous: id("initial"),
        },
    ))]);
    insert(&mut engine, vec![promotion_report(
        "report", 3, "promote", "changed", 2, AutonomyGrade::Propose, AutonomyGrade::Reviewed,
    )]);
    advance_to(&mut engine, 5);
    rejected(&mut engine, cut(5, vec![HistoryChange::Insert(
        promotion("promote", 5, "initial", 1, AutonomyGrade::Reviewed, ReportReference::Unavailable),
    )]), QueryError::InvalidReference);
    let fresh = insert(&mut engine, vec![promotion(
        "promote", 5, "changed", 2, AutonomyGrade::Reviewed,
        ReportReference::Present(id("report")),
    )]);
    assert!(fresh.family(family).complete);
    assert!(fresh.family(family).findings.is_empty());
}

#[test]
fn a_late_promotion_report_does_not_erase_an_original_authority_gap() {
    let family = RiskTheaterFamily::AutonomyInflation;
    let mut engine = new_engine();
    insert(&mut engine, vec![enrollment("initial", 1, 1, AutonomyGrade::Propose)]);
    advance_to(&mut engine, 4);
    let pending = insert(&mut engine, vec![promotion(
        "promote", 4, "initial", 1, AutonomyGrade::Reviewed,
        ReportReference::Present(id("late")),
    )]);
    assert!(!pending.family(family).complete);
    let result = insert(&mut engine, vec![promotion_report(
        "late", 5, "promote", "initial", 1, AutonomyGrade::Propose, AutonomyGrade::Reviewed,
    )]);
    assert!(result.family(family).complete);
    assert_eq!(result.family(family).findings.len(), 1);
    assert_eq!(result.family(family).findings[0].reason, "promotion_without_qualified_report");
}

#[test]
fn funding_is_bound_to_its_exact_audit_and_late_money_does_not_rewrite_the_deadline() {
    let family = RiskTheaterFamily::AuditFloorStarvation;
    for correct_target in [true, false] {
        let mut engine = new_engine();
        let pending = insert(&mut engine, vec![audit("audit-a", 1, 3, 10)]);
        assert!(!pending.family(family).complete);
        insert(&mut engine, vec![funding("first", 2, "audit-a", 5), audit("audit-b", 2, 3, 5)]);
        let target = if correct_target { "audit-a" } else { "audit-b" };
        let snapshot = insert(&mut engine, vec![
            funding("second", 3, target, 5), funding("other", 3, "audit-b", 5),
        ]);
        assert!(snapshot.family(family).complete);
        assert_eq!(snapshot.family(family).findings.len(), usize::from(!correct_target));
        if !correct_target {
            let finding = &snapshot.family(family).findings[0];
            assert_eq!(finding.subject, id("audit-a"));
            assert_eq!(finding.support, vec![id("audit-a"), id("first")]);
            let late = insert(&mut engine, vec![funding("late", 4, "audit-a", 5)]);
            assert_eq!(late.family(family).findings.len(), 1);
            assert!(late.family(family).findings[0].support.contains(&id("late")));
            assert_eq!(engine.verify_kernel(), Ok(late.kernel().clone()));
        }
    }
}

#[test]
fn funding_retractions_restore_starvation_and_overflow_refuses_atomically() {
    let family = RiskTheaterFamily::AuditFloorStarvation;
    let mut engine = new_engine();
    insert(&mut engine, vec![audit("audit", 1, 2, 10)]);
    let paid = funding("paid", 2, "audit", 10);
    let funded = insert(&mut engine, vec![paid.clone()]);
    assert!(funded.family(family).findings.is_empty());
    let corrected = engine.apply_frontier(&cut(3, vec![HistoryChange::Retract(paid.clone())]))
        .expect("correct source payment");
    assert_eq!(corrected.family(family).findings.len(), 1);
    let restored = engine.apply_frontier(&cut(4, vec![HistoryChange::Insert(paid)]))
        .expect("restore exact old payment observation");
    assert!(restored.family(family).findings.is_empty());
    assert_eq!(engine.verify_kernel(), Ok(restored.kernel().clone()));

    let mut engine = new_engine();
    insert(&mut engine, vec![audit("audit", 1, 3, u64::MAX)]);
    insert(&mut engine, vec![funding("full", 2, "audit", u64::MAX)]);
    rejected(&mut engine, cut(3, vec![HistoryChange::Insert(
        funding("overflow", 3, "audit", 1),
    )]), QueryError::ArithmeticOverflow);
}

#[test]
fn orphan_funding_is_incomplete_and_cannot_later_claim_an_earlier_schedule() {
    let family = RiskTheaterFamily::AuditFloorStarvation;
    let mut engine = new_engine();
    let missing = insert(&mut engine, vec![funding("money", 1, "missing", 5)]);
    assert!(!missing.family(family).complete);
    assert!(missing.family(family).findings.is_empty());
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(
        audit("missing", 2, 2, 5),
    )]), QueryError::InvalidReference);
}

#[test]
fn scope_clock_identity_and_report_closure_failures_preserve_the_last_complete_cut() {
    let mut engine = new_engine();
    insert(&mut engine, Vec::new());
    let ordinary = hold("hold", 2);
    let mut foreign_run = ordinary.clone();
    foreign_run.scope.id = "another-run".to_owned();
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(foreign_run)]), QueryError::ForeignScope);
    let mut foreign_tenant = ordinary.clone();
    foreign_tenant.address.tenant = "foreign".to_owned();
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(foreign_tenant)]), QueryError::ForeignScope);
    let mut wrong_generation = ordinary.clone();
    wrong_generation.scope.generation = Some(2);
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(wrong_generation)]), QueryError::ForeignScope);
    let mut missing_generation = ordinary.clone();
    missing_generation.address.generation = None;
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(missing_generation)]), QueryError::InvalidRecord);
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(hold("old-unseen", 1))]), QueryError::BackdatedRecord);
    rejected(&mut engine, cut(2, vec![HistoryChange::Insert(
        retirement("bad-absence", 2, absent(1)),
    )]), QueryError::InvalidRecord);
    let mut open = cut(2, Vec::new());
    open.closed = false;
    rejected(&mut engine, open, QueryError::UnclosedFrontier);
    rejected(&mut engine, cut(3, Vec::new()), QueryError::UnexpectedFrontier { expected: 2, actual: 3 });
    let valid = insert(&mut engine, vec![ordinary.clone()]);
    assert_eq!(valid.sequence(), 2);
    rejected(&mut engine, cut(3, vec![HistoryChange::Insert(ordinary)]), QueryError::DuplicateRecord);
}

#[test]
fn retraction_does_not_refund_the_lifetime_record_identity_budget() {
    let mut engine = new_engine();
    let mut all = Vec::new();
    for group in 0..(MAX_QUERY_RECORDS / 32) {
        let at = group as u64 + 1;
        let records: Vec<_> = (0..32).map(|offset| {
            let index = group * 32 + offset;
            selection_report(&format!("report-{index}"), at, &format!("future-{index}"))
        }).collect();
        all.extend(records.iter().cloned());
        insert(&mut engine, records);
    }
    let at = engine.source_history().len() as u64 + 1;
    engine.apply_frontier(&cut(at, vec![HistoryChange::Retract(all[0].clone())]))
        .expect("projection may retract at identity capacity");
    rejected(&mut engine, cut(at + 1, vec![HistoryChange::Insert(
        selection_report("new-id", at + 1, "new-future"),
    )]), QueryError::ResourceLimit);
    let restored = engine.apply_frontier(&cut(at + 1, vec![HistoryChange::Insert(all[0].clone())]))
        .expect("restoring original identity needs no new slot");
    assert_eq!(engine.verify_kernel(), Ok(restored.kernel().clone()));
}

#[test]
fn generated_support_churn_is_bounded_by_the_independent_oracle_lifetime_capacity() {
    let mut engine = new_engine();
    let holds: Vec<_> = (0..63).map(|index| {
        hold(&format!("h{index:02}"), if index < 32 { 1 } else { 2 })
    }).collect();
    insert(&mut engine, holds[..32].to_vec());
    insert(&mut engine, holds[32..].to_vec());
    let labels: Vec<_> = (0..63).map(|index| {
        assessment(
            &format!("a{index:02}"), if index < 32 { 3 } else { 4 },
            &format!("h{index:02}"), HoldLabel::ConfirmedViolation,
        )
    }).collect();
    insert(&mut engine, labels[..32].to_vec());
    insert(&mut engine, labels[32..].to_vec());
    let opened = insert(&mut engine, vec![retirement("retire", 5, absent(5))]);
    assert_eq!(opened.kernel().supports.len(), 127);
    let mut accepted_changes = 0;
    let mut refused = false;
    for turn in 0..64 {
        let at = engine.source_history().len() as u64 + 1;
        let previous = engine.snapshot(at - 1).expect("previous cut");
        let history = engine.source_history().to_vec();
        let change = if turn % 2 == 0 {
            HistoryChange::Retract(labels[0].clone())
        } else {
            HistoryChange::Insert(labels[0].clone())
        };
        match engine.apply_frontier(&cut(at, vec![change])) {
            Ok(_) => accepted_changes += 1,
            Err(QueryError::ResourceLimit) => {
                assert_eq!(engine.snapshot(at - 1), Ok(previous.clone()));
                assert_eq!(engine.source_history(), history);
                assert_eq!(engine.verify_kernel(), Ok(previous.kernel().clone()));
                assert!(engine.snapshot(at).is_err());
                // The refusal consumed neither the source boundary nor work.
                let same_cut_without_expansion = insert(&mut engine, Vec::new());
                assert_eq!(same_cut_without_expansion.sequence(), at);
                assert_eq!(engine.verify_kernel(), Ok(same_cut_without_expansion.kernel().clone()));
                refused = true;
                break;
            }
            Err(error) => panic!("unexpected refusal: {error:?}"),
        }
    }
    assert!(refused, "generated deltas must not exceed {MAX_RISK_THEATER_HISTORY_DELTAS}");
    assert!(accepted_changes > 1, "permit meaningful bounded support corrections before exhaustion");
}
