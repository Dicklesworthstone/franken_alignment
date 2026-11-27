//! Causal histories for the bounded typed reference predicates.
//! These are reference fixtures, not native journal or qualification tests.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use fa_reference::action::consequence::Consequence;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::governance_assurance::RiskTheaterFamily;
use fa_reference::governance_assurance::risk_theater_queries::{
    AndonFact, CanaryOutcome, CanarySource, ComparisonWindow, ConsequenceFact, CoverageFact,
    ExtendedQueryPolicy, HistoryChange, HistoryFact, HistoryFrontier, HistoryRecord,
    IncidentClosureMode, IncidentDisposition, MediationLevel, QueryEngine, QueryError,
    QueryPolicy, QuerySnapshot, Rate,
};

fn address(kind: Noun, id: &str) -> Address {
    Address::new("test", kind, id, Some(1)).unwrap()
}

fn scope() -> Address {
    address(Noun::Run, "run")
}

fn owner() -> Address {
    address(Noun::Helper, "independent-evaluator")
}

fn record(kind: Noun, id: &str, occurred_at: u64, fact: HistoryFact) -> HistoryRecord {
    HistoryRecord { scope: scope(), address: address(kind, id), occurred_at, fact }
}

fn engine(policy: ExtendedQueryPolicy) -> QueryEngine {
    let policy = QueryPolicy::new(scope(), 7, owner(), 8, 2).unwrap()
        .with_extended(policy).unwrap();
    QueryEngine::new(policy).unwrap()
}

fn apply(engine: &mut QueryEngine, sequence: u64, records: &[HistoryRecord]) -> QuerySnapshot {
    engine.apply_frontier(&HistoryFrontier {
        sequence,
        closed: true,
        changes: records.iter().cloned().map(HistoryChange::Insert).collect(),
    }).unwrap()
}

fn history(records: &[HistoryRecord], policy: ExtendedQueryPolicy) -> (QueryEngine, QuerySnapshot) {
    let mut engine = engine(policy);
    let last = records.iter().map(|record| record.occurred_at).max().unwrap_or(1);
    for sequence in 1..=last {
        let at_boundary: Vec<_> = records.iter()
            .filter(|record| record.occurred_at == sequence).cloned().collect();
        apply(&mut engine, sequence, &at_boundary);
    }
    let snapshot = engine.snapshot(last).unwrap();
    assert_eq!(engine.verify_kernel().unwrap(), *snapshot.kernel());
    (engine, snapshot)
}

fn assert_refused_atomically(
    engine: &mut QueryEngine,
    sequence: u64,
    records: Vec<HistoryRecord>,
    error: QueryError,
) {
    let before = engine.snapshot(sequence - 1).unwrap();
    let source = engine.source_history().to_vec();
    assert_eq!(engine.apply_frontier(&HistoryFrontier {
        sequence,
        closed: true,
        changes: records.into_iter().map(HistoryChange::Insert).collect(),
    }), Err(error));
    assert_eq!(engine.snapshot(sequence - 1).unwrap(), before);
    assert_eq!(engine.source_history(), source.as_slice());
    assert_eq!(engine.verify_kernel().unwrap(), *before.kernel());
    assert!(engine.snapshot(sequence).is_err());
}

fn strict_drift_policy() -> ExtendedQueryPolicy {
    ExtendedQueryPolicy {
        minimum_continue_increase: Rate { numerator: 1, denominator: 2 },
        minimum_canary_escape: Rate { numerator: 1, denominator: 2 },
        minimum_rounds_per_window: 2,
        minimum_canaries: 2,
        andon_roster_closure_threshold: 1,
    }
}

fn round(id: &str, occurred_at: u64, consequence: Consequence, sample: Option<&str>) -> HistoryRecord {
    record(Noun::Round, id, occurred_at, HistoryFact::Consequence(ConsequenceFact::Round {
        effect_family: "tool.write".to_owned(),
        helper_generation: address(Noun::Helper, "helper"),
        policy_generation: address(Noun::Policy, "policy"),
        consequence,
        canary: sample.map(|id| CanarySource {
            lineage: address(Noun::Profile, "canary-campaign"),
            origin: address(Noun::Observation, id),
        }),
    }))
}

fn label(id: &str, occurred_at: u64, original: &str, outcome: CanaryOutcome) -> HistoryRecord {
    record(Noun::Evidence, id, occurred_at,
        HistoryFact::Consequence(ConsequenceFact::CanaryAssessment {
            round: address(Noun::Round, original), outcome, evaluator: owner(),
        }))
}

fn comparison(id: &str, occurred_at: u64, role: ComparisonWindow, start: u64, end: u64) -> HistoryRecord {
    record(Noun::Frontier, id, occurred_at, HistoryFact::Consequence(ConsequenceFact::Window {
        effect_family: "tool.write".to_owned(),
        helper_generation: address(Noun::Helper, "helper"),
        policy_generation: address(Noun::Policy, "policy"),
        role, start, end,
    }))
}

/// Baseline 1/2 Continue; current 2/2; current canary escapes 1/2.
/// Both drift and escape thresholds are met at exact rational equality.
fn drift_records() -> Vec<HistoryRecord> {
    vec![
        round("baseline-hold", 2, Consequence::HoldEffect, None),
        round("baseline-continue", 3, Consequence::Continue, None),
        comparison("baseline", 4, ComparisonWindow::Baseline, 2, 4),
        round("current-first", 5, Consequence::Continue, Some("sample-first")),
        round("current-second", 6, Consequence::Continue, Some("sample-second")),
        label("escaped", 7, "current-first", CanaryOutcome::Escaped),
        label("contained", 8, "current-second", CanaryOutcome::Contained),
        comparison("current", 9, ComparisonWindow::Current, 5, 7),
    ]
}

#[test]
fn exact_rational_drift_preserves_both_denominators_and_canary_lineage() {
    let records = drift_records();
    let (_, snapshot) = history(&records, strict_drift_policy());
    let family = snapshot.family(RiskTheaterFamily::ConsequenceDrift);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    let finding = &family.findings[0];
    assert_eq!(finding.subject, address(Noun::Frontier, "current"));
    let expected: BTreeSet<_> = records.iter().map(|record| record.address.clone()).collect();
    assert_eq!(finding.support.iter().cloned().collect::<BTreeSet<_>>(), expected);
    assert_eq!(snapshot.policy_generation(), 7);
    assert_eq!(snapshot.scope(), &scope());
}

#[test]
fn unchanged_continue_share_and_lower_escape_are_nearby_benign_controls() {
    let mut unchanged_share = drift_records();
    if let HistoryFact::Consequence(ConsequenceFact::Round { consequence, .. }) = &mut unchanged_share[4].fact {
        *consequence = Consequence::HoldEffect;
    } else { unreachable!(); }
    let (_, unchanged) = history(&unchanged_share, strict_drift_policy());
    assert!(unchanged.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(unchanged.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());

    let mut lower_escape = drift_records();
    if let HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { outcome, .. }) = &mut lower_escape[5].fact {
        *outcome = CanaryOutcome::Contained;
    } else { unreachable!(); }
    let (_, lower) = history(&lower_escape, strict_drift_policy());
    assert!(lower.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(lower.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
}

#[test]
fn absent_or_censored_label_is_unknown_until_original_sample_is_resolved() {
    let mut pending = drift_records();
    if let HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { outcome, .. }) = &mut pending[5].fact {
        *outcome = CanaryOutcome::Censored;
    } else { unreachable!(); }
    let (mut engine, before) = history(&pending, strict_drift_policy());
    assert!(!before.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(before.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
    let resolved = label("resolved-later", 10, "current-first", CanaryOutcome::Escaped);
    let after = apply(&mut engine, 10, &[resolved.clone()]);
    let family = after.family(RiskTheaterFamily::ConsequenceDrift);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    assert!(family.findings[0].support.contains(&pending[5].address));
    assert!(family.findings[0].support.contains(&resolved.address));
    assert!(family.findings[0].support.contains(&pending[3].address));

    pending.remove(5);
    let (_, absent) = history(&pending, strict_drift_policy());
    assert!(!absent.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(absent.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
}

#[test]
fn zero_baseline_current_or_canary_denominator_never_certifies_clean() {
    for remove in [0u8, 1, 2] {
        let mut records = drift_records();
        records.retain(|record| match &record.fact {
            HistoryFact::Consequence(ConsequenceFact::Round { .. })
                if remove == 0 && record.occurred_at < 4 => false,
            HistoryFact::Consequence(ConsequenceFact::Round { .. })
                if remove == 1 && record.occurred_at > 4 => false,
            HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { .. })
                if remove == 1 || remove == 2 => false,
            _ => true,
        });
        if remove == 2 {
            for record in &mut records {
                if let HistoryFact::Consequence(ConsequenceFact::Round { canary, .. }) = &mut record.fact {
                    *canary = None;
                }
            }
        }
        let (_, snapshot) = history(&records, strict_drift_policy());
        let family = snapshot.family(RiskTheaterFamily::ConsequenceDrift);
        assert!(!family.complete);
        assert!(family.findings.is_empty());
    }
}

#[test]
fn wrong_evaluator_generation_cannot_supply_canary_ground_truth() {
    let mut records = drift_records();
    if let HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { evaluator, .. }) = &mut records[5].fact {
        evaluator.generation = Some(2);
    } else { unreachable!(); }
    let (_, snapshot) = history(&records, strict_drift_policy());
    let family = snapshot.family(RiskTheaterFamily::ConsequenceDrift);
    assert!(!family.complete);
    assert!(family.findings.is_empty());
}

#[test]
fn helper_and_policy_generations_are_never_pooled() {
    for change_helper in [true, false] {
        let mut records = drift_records();
        for record in &mut records {
            if record.occurred_at < 5 { continue; }
            match &mut record.fact {
                HistoryFact::Consequence(ConsequenceFact::Round { helper_generation, policy_generation, .. })
                | HistoryFact::Consequence(ConsequenceFact::Window { helper_generation, policy_generation, .. }) => {
                    if change_helper {
                        helper_generation.generation = Some(2);
                    } else {
                        policy_generation.generation = Some(2);
                    }
                }
                _ => {}
            }
        }
        let (_, snapshot) = history(&records, strict_drift_policy());
        assert!(!snapshot.family(RiskTheaterFamily::ConsequenceDrift).complete);
        assert!(snapshot.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
    }
}

#[test]
fn one_original_canary_cannot_be_relabelled_as_a_new_campaign_sample() {
    let records = drift_records();
    let (mut engine, _) = history(&records, strict_drift_policy());
    let mut duplicate = round("invented-round", 10, Consequence::Continue, Some("sample-first"));
    if let HistoryFact::Consequence(ConsequenceFact::Round { canary: Some(source), .. }) = &mut duplicate.fact {
        source.lineage = address(Noun::Profile, "different-campaign");
    } else { unreachable!(); }
    assert_refused_atomically(&mut engine, 10, vec![duplicate], QueryError::ConflictingRecord);
}

#[test]
fn final_label_conflict_is_atomic_even_outside_selected_comparison_windows() {
    let mut records = drift_records();
    if let HistoryFact::Consequence(ConsequenceFact::Round { canary, .. }) = &mut records[0].fact {
        *canary = Some(CanarySource {
            lineage: address(Noun::Profile, "old-campaign"),
            origin: address(Noun::Observation, "baseline-sample"),
        });
    } else { unreachable!(); }
    records.push(label("baseline-final", 4, "baseline-hold", CanaryOutcome::Contained));
    let (mut engine, _) = history(&records, strict_drift_policy());
    let contradictory = label("baseline-conflict", 10, "baseline-hold", CanaryOutcome::Escaped);
    assert_refused_atomically(&mut engine, 10, vec![contradictory], QueryError::ConflictingRecord);
}

#[test]
fn withdrawing_a_canary_round_cannot_hide_conflicting_independent_outcomes() {
    let records = drift_records();
    let (mut engine, original) = history(&records, strict_drift_policy());
    let withdrawn = &records[3];
    let family = RiskTheaterFamily::ConsequenceDrift;
    let missing = engine.apply_frontier(&HistoryFrontier {
        sequence: 10,
        closed: true,
        changes: vec![HistoryChange::Retract(withdrawn.clone())],
    }).unwrap();
    assert!(!missing.family(family).complete);
    assert!(missing.family(family).findings.is_empty());

    let mut foreign = label("foreign", 11, "current-first", CanaryOutcome::Contained);
    let HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { evaluator, .. }) =
        &mut foreign.fact else { unreachable!() };
    evaluator.generation = Some(2);
    assert_refused_atomically(&mut engine, 11, vec![
        foreign,
        label("second-final", 11, "current-first", CanaryOutcome::Contained),
    ], QueryError::ConflictingRecord);

    let restored = apply(&mut engine, 11, std::slice::from_ref(withdrawn));
    assert_eq!(restored.family(family), original.family(family));
    assert_eq!(engine.verify_kernel().unwrap(), *restored.kernel());
}

#[test]
fn absent_canary_round_allows_ordered_censored_resolution_but_not_ambiguous_labels() {
    let family = RiskTheaterFamily::ConsequenceDrift;
    let (mut engine, _) = history(&[], strict_drift_policy());
    let censored = label("censored", 2, "missing", CanaryOutcome::Censored);
    assert_refused_atomically(&mut engine, 2, vec![
        censored.clone(), label("same-cut", 2, "missing", CanaryOutcome::Escaped),
    ], QueryError::ConflictingRecord);
    let pending = apply(&mut engine, 2, &[censored]);
    assert!(!pending.family(family).complete);
    assert_refused_atomically(&mut engine, 3, vec![
        label("repeat-censor", 3, "missing", CanaryOutcome::Censored),
    ], QueryError::ConflictingRecord);
    let resolved = apply(&mut engine, 3, &[label("later", 3, "missing", CanaryOutcome::Escaped)]);
    assert!(!resolved.family(family).complete);
    assert!(resolved.family(family).findings.is_empty());
    assert_refused_atomically(&mut engine, 4, vec![
        label("second-final", 4, "missing", CanaryOutcome::Escaped),
    ], QueryError::ConflictingRecord);
}

#[test]
fn correcting_a_denominator_retracts_and_restores_the_derived_finding_exactly() {
    let records = drift_records();
    let (mut engine, before) = history(&records, strict_drift_policy());
    let missing = engine.apply_frontier(&HistoryFrontier {
        sequence: 10, closed: true, changes: vec![HistoryChange::Retract(records[0].clone())],
    }).unwrap();
    assert!(!missing.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(missing.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
    let restored = engine.apply_frontier(&HistoryFrontier {
        sequence: 11, closed: true, changes: vec![HistoryChange::Insert(records[0].clone())],
    }).unwrap();
    assert_eq!(restored.family(RiskTheaterFamily::ConsequenceDrift),
        before.family(RiskTheaterFamily::ConsequenceDrift));
    assert_eq!(engine.verify_kernel().unwrap(), *restored.kernel());
}

fn andon_records(mode: IncidentClosureMode) -> Vec<HistoryRecord> {
    vec![
        record(Noun::Ledger, "budget", 1, HistoryFact::Andon(AndonFact::Window {
            start: 2, end: 4, false_stops_allowed: 2, cost_per_stop: u64::MAX,
        })),
        round("hold-first", 2, Consequence::HoldEffect, None),
        round("hold-second", 3, Consequence::HoldEffect, None),
        record(Noun::Evidence, "false-first", 4, HistoryFact::Andon(AndonFact::FalseStop {
            window: address(Noun::Ledger, "budget"),
            round: address(Noun::Round, "hold-first"), evaluator: owner(),
        })),
        record(Noun::Evidence, "false-second", 5, HistoryFact::Andon(AndonFact::FalseStop {
            window: address(Noun::Ledger, "budget"),
            round: address(Noun::Round, "hold-second"), evaluator: owner(),
        })),
        record(Noun::Incident, "exhausted", 5, HistoryFact::Andon(AndonFact::Incident {
            window: address(Noun::Ledger, "budget"),
            opened_by: address(Noun::Evidence, "false-second"),
        })),
        record(Noun::Consequence, "close-authority", 6, HistoryFact::Andon(AndonFact::Authorization {
            incident: address(Noun::Incident, "exhausted"), mode,
        })),
        record(Noun::Receipt, "closed", 7, HistoryFact::Andon(AndonFact::Closure {
            incident: address(Noun::Incident, "exhausted"), mode,
            authorization: address(Noun::Consequence, "close-authority"),
        })),
        record(Noun::Observation, "disposition", 8, HistoryFact::Andon(AndonFact::Disposition {
            incident: address(Noun::Incident, "exhausted"),
            state: IncidentDisposition::Closed { closure: address(Noun::Receipt, "closed") },
        })),
    ]
}

#[test]
fn exhausted_andon_roster_closure_preserves_original_cost_and_occurrence_support() {
    let records = andon_records(IncidentClosureMode::Roster);
    let (_, snapshot) = history(&records, ExtendedQueryPolicy::default());
    let family = snapshot.family(RiskTheaterFamily::AndonClosureMode);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    assert_eq!(family.findings[0].subject, address(Noun::Incident, "exhausted"));
    let expected: BTreeSet<_> = records.iter().map(|record| record.address.clone()).collect();
    assert_eq!(family.findings[0].support.iter().cloned().collect::<BTreeSet<_>>(), expected);
}

#[test]
fn identical_exhaustion_closed_by_evidence_or_policy_is_a_benign_control() {
    for mode in [IncidentClosureMode::Evidence, IncidentClosureMode::Policy] {
        let (_, snapshot) = history(&andon_records(mode), ExtendedQueryPolicy::default());
        let family = snapshot.family(RiskTheaterFamily::AndonClosureMode);
        assert!(family.complete);
        assert!(family.findings.is_empty());
    }
}

#[test]
fn andon_missing_disposition_and_known_open_state_are_distinct() {
    let mut records = andon_records(IncidentClosureMode::Roster);
    records.retain(|record| record.occurred_at <= 5);
    let (mut engine, missing) = history(&records, ExtendedQueryPolicy::default());
    assert!(!missing.family(RiskTheaterFamily::AndonClosureMode).complete);
    let open = record(Noun::Observation, "known-open", 6, HistoryFact::Andon(AndonFact::Disposition {
        incident: address(Noun::Incident, "exhausted"), state: IncidentDisposition::Open,
    }));
    let known = apply(&mut engine, 6, &[open]);
    assert!(known.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(known.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
}

#[test]
fn zero_allowance_requires_a_real_first_stop_and_positive_allowance_opens_at_equality() {
    let mut records = andon_records(IncidentClosureMode::Roster);
    if let HistoryFact::Andon(AndonFact::Window { false_stops_allowed, .. }) = &mut records[0].fact {
        *false_stops_allowed = 0;
    } else { unreachable!(); }
    let (_, empty) = history(&records[..1], ExtendedQueryPolicy::default());
    assert!(empty.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(empty.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
    records.retain(|record| record.address != address(Noun::Round, "hold-second")
        && record.address != address(Noun::Evidence, "false-second"));
    for record in &mut records {
        if let HistoryFact::Andon(AndonFact::Incident { opened_by, .. }) = &mut record.fact {
            *opened_by = address(Noun::Evidence, "false-first");
        }
    }
    let (_, first_stop) = history(&records, ExtendedQueryPolicy::default());
    assert!(first_stop.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert_eq!(first_stop.family(RiskTheaterFamily::AndonClosureMode).findings.len(), 1);

    let mut below = andon_records(IncidentClosureMode::Roster);
    below.retain(|record| !matches!(&record.fact,
        HistoryFact::Andon(AndonFact::Incident { .. }
            | AndonFact::Disposition { .. }
            | AndonFact::Authorization { .. }
            | AndonFact::Closure { .. })));
    if let HistoryFact::Andon(AndonFact::Window { false_stops_allowed, .. }) = &mut below[0].fact {
        *false_stops_allowed = 3;
    } else { unreachable!(); }
    let (_, before_equality) = history(&below, ExtendedQueryPolicy::default());
    assert!(before_equality.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(before_equality.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
    if let HistoryFact::Andon(AndonFact::Window { false_stops_allowed, .. }) = &mut below[0].fact {
        *false_stops_allowed = 2;
    }
    let (_, missing_incident) = history(&below, ExtendedQueryPolicy::default());
    assert!(!missing_incident.family(RiskTheaterFamily::AndonClosureMode).complete);
}

#[test]
fn actor_confirmation_cannot_exhaust_the_independent_andon_budget() {
    let mut records = andon_records(IncidentClosureMode::Roster);
    if let HistoryFact::Andon(AndonFact::FalseStop { evaluator, .. }) = &mut records[3].fact {
        *evaluator = address(Noun::Helper, "actor");
    } else { unreachable!(); }
    let (_, snapshot) = history(&records, ExtendedQueryPolicy::default());
    assert!(!snapshot.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(snapshot.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
}

#[test]
fn a_second_confirmation_address_cannot_charge_the_same_original_round_twice() {
    let (mut engine, _) = history(&andon_records(IncidentClosureMode::Roster), ExtendedQueryPolicy::default());
    let repeated = record(Noun::Evidence, "repeated-first", 9, HistoryFact::Andon(AndonFact::FalseStop {
        window: address(Noun::Ledger, "budget"),
        round: address(Noun::Round, "hold-first"), evaluator: owner(),
    }));
    assert_refused_atomically(&mut engine, 9, vec![repeated], QueryError::ConflictingRecord);
}

#[test]
fn retrospective_andon_budget_cannot_replace_original_predeclared_limits() {
    let original = round("already-held", 2, Consequence::HoldEffect, None);
    let (mut engine, _) = history(&[original], ExtendedQueryPolicy::default());
    let retrospective = record(Noun::Ledger, "retrospective", 3, HistoryFact::Andon(AndonFact::Window {
        start: 2, end: 4, false_stops_allowed: 100, cost_per_stop: 1,
    }));
    assert_refused_atomically(&mut engine, 3, vec![retrospective], QueryError::InvalidRecord);

    let exact_start = record(Noun::Ledger, "at-start", 1, HistoryFact::Andon(AndonFact::Window {
        start: 1, end: 2, false_stops_allowed: 1, cost_per_stop: 1,
    }));
    let (_, accepted) = history(&[exact_start], ExtendedQueryPolicy::default());
    assert!(accepted.family(RiskTheaterFamily::AndonClosureMode).complete);
}

#[test]
fn closure_authorization_must_precede_and_bind_exact_original_incident_and_mode() {
    let mut records = andon_records(IncidentClosureMode::Roster);
    records[6].occurred_at = 7;
    let prefix: Vec<_> = records.iter().filter(|record| record.occurred_at <= 6).cloned().collect();
    let (mut engine, _) = history(&prefix, ExtendedQueryPolicy::default());
    apply(&mut engine, 6, &[]);
    let at_seven: Vec<_> = records.iter().filter(|record| record.occurred_at == 7).cloned().collect();
    assert_refused_atomically(&mut engine, 7, at_seven, QueryError::InvalidReference);
}

fn channels(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn coverage_records(next: MediationLevel, captured: &[&str], contract: &str) -> Vec<HistoryRecord> {
    vec![
        record(Noun::Profile, "coverage-before", 1, HistoryFact::Coverage(CoverageFact::State {
            effect_family: "tool.write".to_owned(),
            mediation: MediationLevel::Brokered,
            capture_contract: address(Noun::Profile, "capture-contract"),
            channels: channels(&["activation", "output"]),
        })),
        record(Noun::Profile, "coverage-after", 2, HistoryFact::Coverage(CoverageFact::State {
            effect_family: "tool.write".to_owned(),
            mediation: next,
            capture_contract: address(Noun::Profile, contract),
            channels: channels(captured),
        })),
        record(Noun::Receipt, "coverage-authority", 3, HistoryFact::Coverage(CoverageFact::Authorization {
            change: address(Noun::Consequence, "coverage-change"),
            previous: address(Noun::Profile, "coverage-before"),
            current: address(Noun::Profile, "coverage-after"),
        })),
        record(Noun::Consequence, "coverage-change", 4, HistoryFact::Coverage(CoverageFact::Change {
            effect_family: "tool.write".to_owned(),
            previous: address(Noun::Profile, "coverage-before"),
            current: address(Noun::Profile, "coverage-after"),
            authorization: Some(address(Noun::Receipt, "coverage-authority")),
        })),
    ]
}

#[test]
fn lost_capture_channel_is_erosion_even_when_mediation_improves() {
    let records = coverage_records(MediationLevel::Attested, &["output"], "capture-contract");
    let (_, snapshot) = history(&records, ExtendedQueryPolicy::default());
    let family = snapshot.family(RiskTheaterFamily::CoverageErosion);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    assert_eq!(family.findings[0].subject, address(Noun::Consequence, "coverage-change"));
    let expected: BTreeSet<_> = records.iter().map(|record| record.address.clone()).collect();
    assert_eq!(family.findings[0].support.iter().cloned().collect::<BTreeSet<_>>(), expected);
}

#[test]
fn unchanged_or_broader_coverage_is_the_permitted_twin() {
    for (level, capture) in [
        (MediationLevel::Brokered, vec!["activation", "output"]),
        (MediationLevel::Attested, vec!["activation", "output", "tool-intent"]),
    ] {
        let (_, snapshot) = history(&coverage_records(level, &capture, "capture-contract"),
            ExtendedQueryPolicy::default());
        assert!(snapshot.family(RiskTheaterFamily::CoverageErosion).complete);
        assert!(snapshot.family(RiskTheaterFamily::CoverageErosion).findings.is_empty());
    }
}

#[test]
fn improved_capture_does_not_hide_lower_mediation_and_authority_does_not_erase_erosion() {
    let records = coverage_records(MediationLevel::Cooperative,
        &["activation", "output", "tool-intent"], "capture-contract");
    let (_, snapshot) = history(&records, ExtendedQueryPolicy::default());
    let family = snapshot.family(RiskTheaterFamily::CoverageErosion);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    assert!(family.findings[0].support.contains(&address(Noun::Receipt, "coverage-authority")));
}

#[test]
fn incomparable_capture_contract_is_unknown_but_independent_mediation_loss_survives() {
    for level in [MediationLevel::Brokered, MediationLevel::Cooperative] {
        let (_, snapshot) = history(
            &coverage_records(level, &["activation", "output"], "new-contract"),
            ExtendedQueryPolicy::default(),
        );
        let family = snapshot.family(RiskTheaterFamily::CoverageErosion);
        assert!(!family.complete);
        assert_eq!(family.findings.len(), usize::from(level == MediationLevel::Cooperative));
    }
}

#[test]
fn missing_authorization_keeps_known_erosion_visible_without_clean_completeness() {
    let mut records = coverage_records(MediationLevel::Cooperative, &["output"], "capture-contract");
    records.remove(2);
    let (_, snapshot) = history(&records, ExtendedQueryPolicy::default());
    let family = snapshot.family(RiskTheaterFamily::CoverageErosion);
    assert!(!family.complete);
    assert_eq!(family.findings.len(), 1);
    assert!(!family.findings[0].support.contains(&address(Noun::Receipt, "coverage-authority")));
}

#[test]
fn matching_authorization_from_same_boundary_cannot_retroactively_authorize_a_change() {
    let mut records = coverage_records(MediationLevel::Cooperative, &["output"], "capture-contract");
    records[2].occurred_at = 4;
    let (mut engine, _) = history(&records[..2], ExtendedQueryPolicy::default());
    apply(&mut engine, 3, &[]);
    assert_refused_atomically(&mut engine, 4, records[2..].to_vec(), QueryError::InvalidReference);
}

#[test]
fn active_coverage_chain_rejects_stale_predecessor_that_would_hide_erosion() {
    let first = coverage_records(MediationLevel::Attested, &["activation", "output"], "capture-contract");
    let (mut stale_engine, _) = history(&first, ExtendedQueryPolicy::default());
    let current = record(Noun::Profile, "coverage-third", 5, HistoryFact::Coverage(CoverageFact::State {
        effect_family: "tool.write".to_owned(),
        mediation: MediationLevel::Brokered,
        capture_contract: address(Noun::Profile, "capture-contract"),
        channels: channels(&["activation", "output"]),
    }));
    apply(&mut stale_engine, 5, &[current.clone()]);
    let stale_authority = record(Noun::Receipt, "third-authority", 6,
        HistoryFact::Coverage(CoverageFact::Authorization {
            change: address(Noun::Consequence, "third-change"),
            previous: address(Noun::Profile, "coverage-before"),
            current: current.address.clone(),
        }));
    apply(&mut stale_engine, 6, &[stale_authority.clone()]);
    let stale_change = record(Noun::Consequence, "third-change", 7,
        HistoryFact::Coverage(CoverageFact::Change {
            effect_family: "tool.write".to_owned(),
            previous: address(Noun::Profile, "coverage-before"),
            current: current.address.clone(),
            authorization: Some(stale_authority.address.clone()),
        }));
    assert_refused_atomically(&mut stale_engine, 7, vec![stale_change.clone()], QueryError::ConflictingRecord);

    let mut actual = first;
    actual.push(current);
    let mut actual_authority = stale_authority;
    let mut actual_change = stale_change;
    if let HistoryFact::Coverage(CoverageFact::Authorization { previous, .. }) = &mut actual_authority.fact {
        *previous = address(Noun::Profile, "coverage-after");
    }
    if let HistoryFact::Coverage(CoverageFact::Change { previous, .. }) = &mut actual_change.fact {
        *previous = address(Noun::Profile, "coverage-after");
    }
    actual.extend([actual_authority, actual_change]);
    let (_, correct) = history(&actual, ExtendedQueryPolicy::default());
    let family = correct.family(RiskTheaterFamily::CoverageErosion);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 1);
    assert_eq!(family.findings[0].subject, address(Noun::Consequence, "third-change"));
    assert!(family.findings[0].support.contains(&address(Noun::Consequence, "coverage-change")));
    assert!(family.findings[0].support.contains(&address(Noun::Receipt, "coverage-authority")));
}

#[test]
fn missing_original_opening_support_retracts_andon_finding_without_fabricating_a_new_crossing() {
    let mut records = andon_records(IncidentClosureMode::Roster);
    if let HistoryFact::Andon(AndonFact::Window { false_stops_allowed, .. }) = &mut records[0].fact {
        *false_stops_allowed = 0;
    }
    if let HistoryFact::Andon(AndonFact::Incident { opened_by, .. }) = &mut records[5].fact {
        *opened_by = address(Noun::Evidence, "false-first");
    }
    let (mut engine, original) = history(&records, ExtendedQueryPolicy::default());
    assert_eq!(original.family(RiskTheaterFamily::AndonClosureMode).findings.len(), 1);
    let missing = engine.apply_frontier(&HistoryFrontier {
        sequence: 9, closed: true, changes: vec![HistoryChange::Retract(records[3].clone())],
    }).unwrap();
    assert!(!missing.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(missing.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
    let restored = engine.apply_frontier(&HistoryFrontier {
        sequence: 10, closed: true, changes: vec![HistoryChange::Insert(records[3].clone())],
    }).unwrap();
    assert_eq!(restored.family(RiskTheaterFamily::AndonClosureMode),
        original.family(RiskTheaterFamily::AndonClosureMode));
    assert_eq!(engine.verify_kernel().unwrap(), *restored.kernel());
}

fn second_andon_window() -> Vec<HistoryRecord> {
    fn shift(address: &mut Address) {
        address.id.push_str("-second-window");
    }
    let mut records = andon_records(IncidentClosureMode::Roster);
    for record in &mut records {
        record.occurred_at += 8;
        shift(&mut record.address);
        match &mut record.fact {
            HistoryFact::Andon(AndonFact::Window { start, end, .. }) => {
                *start += 8;
                *end += 8;
            }
            HistoryFact::Andon(AndonFact::FalseStop { window, round, .. }) => {
                shift(window);
                shift(round);
            }
            HistoryFact::Andon(AndonFact::Incident { window, opened_by }) => {
                shift(window);
                shift(opened_by);
            }
            HistoryFact::Andon(AndonFact::Authorization { incident, .. }) => shift(incident),
            HistoryFact::Andon(AndonFact::Closure { incident, authorization, .. }) => {
                shift(incident);
                shift(authorization);
            }
            HistoryFact::Andon(AndonFact::Disposition { incident, state }) => {
                shift(incident);
                if let IncidentDisposition::Closed { closure } = state { shift(closure); }
            }
            _ => {}
        }
    }
    records
}

#[test]
fn repeated_roster_closure_threshold_has_every_contributing_incident_as_support() {
    let policy = ExtendedQueryPolicy {
        andon_roster_closure_threshold: 2,
        ..ExtendedQueryPolicy::default()
    };
    let mut records = andon_records(IncidentClosureMode::Roster);
    let (_, one) = history(&records, policy.clone());
    assert!(one.family(RiskTheaterFamily::AndonClosureMode).complete);
    assert!(one.family(RiskTheaterFamily::AndonClosureMode).findings.is_empty());
    records.extend(second_andon_window());
    let (_, two) = history(&records, policy);
    let family = two.family(RiskTheaterFamily::AndonClosureMode);
    assert!(family.complete);
    assert_eq!(family.findings.len(), 2);
    let expected: BTreeSet<_> = records.iter().map(|record| record.address.clone()).collect();
    for finding in &family.findings {
        assert_eq!(finding.support.iter().cloned().collect::<BTreeSet<_>>(), expected);
    }
}

#[test]
fn comparative_sources_require_explicit_selection_and_refuse_atomically_when_disabled() {
    let base = QueryPolicy::new(scope(), 7, owner(), 8, 2).unwrap();
    assert!(base.extended().is_none());
    let mut disabled = QueryEngine::new(base.clone()).unwrap();
    apply(&mut disabled, 1, &[]);
    let original = round("selected-round", 2, Consequence::HoldEffect, None);
    assert_refused_atomically(
        &mut disabled, 2, vec![original.clone()], QueryError::InvalidPolicy,
    );
    assert_eq!(disabled.policy(), &base);
    assert!(disabled.policy().extended().is_none());

    // The same raw record enters only a newly constructed, explicitly selected
    // profile. Selection does not invent the missing windows or canary labels.
    let mut selected = engine(ExtendedQueryPolicy::default());
    apply(&mut selected, 1, &[]);
    let admitted = apply(&mut selected, 2, &[original.clone()]);
    assert_eq!(selected.source_history()[1].changes, vec![HistoryChange::Insert(original)]);
    assert!(!admitted.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(admitted.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
    assert_eq!(selected.verify_kernel().unwrap(), *admitted.kernel());
}

#[test]
fn comparative_policy_refuses_undefined_rates_and_zero_minima_but_accepts_exact_wide_ratios() {
    let base = QueryPolicy::new(scope(), 7, owner(), 8, 2).unwrap();
    for invalid_rate in [
        Rate { numerator: 0, denominator: 1 },
        Rate { numerator: 1, denominator: 0 },
        Rate { numerator: 2, denominator: 1 },
    ] {
        for continue_rate in [true, false] {
            let mut candidate = ExtendedQueryPolicy::default();
            if continue_rate {
                candidate.minimum_continue_increase = invalid_rate;
            } else {
                candidate.minimum_canary_escape = invalid_rate;
            }
            assert_eq!(
                base.clone().with_extended(candidate),
                Err(QueryError::InvalidPolicy),
            );
        }
    }
    for minimum in 0..3 {
        let mut candidate = ExtendedQueryPolicy::default();
        match minimum {
            0 => candidate.minimum_rounds_per_window = 0,
            1 => candidate.minimum_canaries = 0,
            _ => candidate.andon_roster_closure_threshold = 0,
        }
        assert_eq!(
            base.clone().with_extended(candidate),
            Err(QueryError::InvalidPolicy),
        );
    }
    let wide = ExtendedQueryPolicy {
        minimum_continue_increase: Rate { numerator: u64::MAX, denominator: u64::MAX },
        minimum_canary_escape: Rate { numerator: 1, denominator: u64::MAX },
        ..ExtendedQueryPolicy::default()
    };
    let admitted = base.clone().with_extended(wide.clone()).unwrap();
    assert_eq!(admitted.extended(), Some(&wide));
    assert!(base.extended().is_none());
}

#[test]
fn snapshots_retain_the_full_original_query_configuration_across_distinct_reference_profiles() {
    let records = drift_records();
    let mut external_configuration = strict_drift_policy();
    let original_configuration = external_configuration.clone();
    let (original_engine, original) = history(&records, external_configuration.clone());
    assert_eq!(original.family(RiskTheaterFamily::ConsequenceDrift).findings.len(), 1);

    // Mutating a caller-owned configuration can construct another reference
    // profile; it cannot change the original engine or its historical snapshot.
    external_configuration.minimum_continue_increase = Rate { numerator: 3, denominator: 4 };
    external_configuration.andon_roster_closure_threshold = 2;
    let (_, alternative) = history(&records, external_configuration.clone());
    assert!(alternative.family(RiskTheaterFamily::ConsequenceDrift).complete);
    assert!(alternative.family(RiskTheaterFamily::ConsequenceDrift).findings.is_empty());
    assert_eq!(original.sequence(), alternative.sequence());
    assert_eq!(original.scope(), alternative.scope());
    assert_eq!(original.policy_generation(), alternative.policy_generation());
    assert_ne!(original.policy(), alternative.policy());
    assert_eq!(original.policy().extended(), Some(&original_configuration));
    assert_eq!(original_engine.policy().extended(), Some(&original_configuration));
    assert_eq!(alternative.policy().extended(), Some(&external_configuration));
    assert_eq!(original.policy().evaluation_owner(), &owner());
    assert_eq!(original_engine.snapshot(original.sequence()).unwrap(), original);
}
