//! Source predicates for roster churn, autonomy promotion and audit funding.
//! Source records and evaluator identities are reference assertions only.

use std::collections::{BTreeMap, BTreeSet};

use crate::address_resolver::{Address, Noun};
use crate::governance_assurance::{AutonomyGrade, RiskTheaterFamily};

use super::{
    FamilyEvaluation, HistoryFact, HistoryRecord, QueryError, QueryFinding, QueryPolicy,
    find_record, resolve_assessment_chain, validate_address, validate_family,
};

/// Absence is stronger than unavailable evidence. An Absent value asserts that
/// the complete report-attachment domain for that exact transition was inspected
/// through its control sequence. A prepared report that was not carried by the
/// transition supplies no approval. It does not authenticate that assertion. A missing Present
/// record is unavailable, never equivalent to Absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportReference {
    Absent { closed_through: u64 },
    Unavailable,
    Present(Address),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldLabel {
    ConfirmedViolation,
    Benign,
    Censored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RosterFact {
    Hold {
        helper: Address,
        effect_family: String,
    },
    /// One original label, followed optionally by a strictly later resolution
    /// of Censored from the same configured evaluation owner. Both records
    /// retain their identities and participate in any resulting finding.
    Assessment {
        hold: Address,
        evaluator: Address,
        label: HoldLabel,
    },
    Retirement {
        helper: Address,
        effect_family: String,
        report: ReportReference,
    },
    /// Attack-conditioned qualification is a recorded property of the supplied
    /// report, not a claim that this module ran or authenticated its campaign.
    SelectionReport {
        retirement: Address,
        helper: Address,
        effect_family: String,
        evaluator: Address,
        attack_conditioned: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutonomyFact {
    /// Exactly one enrollment is allowed per principal generation and family.
    Enrollment {
        principal: Address,
        passport: Address,
        effect_family: String,
        grade: AutonomyGrade,
    },
    Demotion {
        principal: Address,
        passport: Address,
        effect_family: String,
        previous: Address,
        grade: AutonomyGrade,
    },
    /// This scoped reference profile resets a changed passport to Propose.
    /// It observes that declared transition; it does not perform the reset.
    PassportChanged {
        principal: Address,
        passport: Address,
        effect_family: String,
        previous: Address,
    },
    Promotion {
        principal: Address,
        passport: Address,
        effect_family: String,
        previous: Address,
        to: AutonomyGrade,
        report: ReportReference,
    },
    PromotionReport {
        promotion: Address,
        principal: Address,
        passport: Address,
        effect_family: String,
        previous: Address,
        from: AutonomyGrade,
        to: AutonomyGrade,
        evaluator: Address,
        attack_conditioned: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditFact {
    Scheduled {
        effect_family: String,
        required_units: u64,
        due_at: u64,
    },
    Funding {
        audit: Address,
        units: u64,
    },
}

fn validate_report_reference(
    reference: &ReportReference,
    record: &HistoryRecord,
    scope: &Address,
) -> Result<(), QueryError> {
    match reference {
        ReportReference::Absent { closed_through } => {
            if *closed_through != record.occurred_at {
                return Err(QueryError::InvalidRecord);
            }
        }
        ReportReference::Unavailable => {}
        ReportReference::Present(address) => validate_address(address, scope)?,
    }
    Ok(())
}

fn validate_helper(helper: &Address, scope: &Address) -> Result<(), QueryError> {
    validate_address(helper, scope)?;
    if helper.kind != Noun::Helper {
        return Err(QueryError::InvalidRecord);
    }
    Ok(())
}

fn autonomy_subject(fact: &AutonomyFact) -> (&Address, &Address, &str) {
    match fact {
        AutonomyFact::Enrollment { principal, passport, effect_family, .. }
        | AutonomyFact::Demotion { principal, passport, effect_family, .. }
        | AutonomyFact::PassportChanged { principal, passport, effect_family, .. }
        | AutonomyFact::Promotion { principal, passport, effect_family, .. }
        | AutonomyFact::PromotionReport { principal, passport, effect_family, .. } => {
            (principal, passport, effect_family)
        }
    }
}

pub(super) fn validate_fact(record: &HistoryRecord, scope: &Address) -> Result<(), QueryError> {
    match &record.fact {
        HistoryFact::Roster(fact) => match fact {
            RosterFact::Hold { helper, effect_family } => {
                validate_helper(helper, scope)?;
                validate_family(effect_family)?;
            }
            RosterFact::Assessment { hold, evaluator, .. } => {
                validate_address(hold, scope)?;
                validate_address(evaluator, scope)?;
            }
            RosterFact::Retirement { helper, effect_family, report } => {
                validate_helper(helper, scope)?;
                validate_family(effect_family)?;
                validate_report_reference(report, record, scope)?;
            }
            RosterFact::SelectionReport {
                retirement, helper, effect_family, evaluator, ..
            } => {
                validate_address(retirement, scope)?;
                validate_helper(helper, scope)?;
                validate_family(effect_family)?;
                validate_address(evaluator, scope)?;
            }
        },
        HistoryFact::Autonomy(fact) => {
            let (principal, passport, family) = autonomy_subject(fact);
            validate_address(principal, scope)?;
            validate_address(passport, scope)?;
            validate_family(family)?;
            if passport.kind != Noun::Passport {
                return Err(QueryError::InvalidRecord);
            }
            match fact {
                AutonomyFact::Enrollment { .. } => {}
                AutonomyFact::Demotion { previous, .. }
                | AutonomyFact::PassportChanged { previous, .. } => {
                    validate_address(previous, scope)?;
                }
                AutonomyFact::Promotion { previous, report, .. } => {
                    validate_address(previous, scope)?;
                    validate_report_reference(report, record, scope)?;
                }
                AutonomyFact::PromotionReport { promotion, previous, evaluator, .. } => {
                    validate_address(promotion, scope)?;
                    validate_address(previous, scope)?;
                    validate_address(evaluator, scope)?;
                }
            }
        }
        HistoryFact::Audit(fact) => match fact {
            AuditFact::Scheduled { effect_family, required_units, due_at } => {
                validate_family(effect_family)?;
                if *required_units == 0 || *due_at < record.occurred_at {
                    return Err(QueryError::InvalidRecord);
                }
            }
            AuditFact::Funding { audit, units } => {
                validate_address(audit, scope)?;
                if *units == 0 {
                    return Err(QueryError::InvalidRecord);
                }
            }
        },
        HistoryFact::Consequence(_) | HistoryFact::Andon(_) | HistoryFact::Coverage(_) => {
            return Err(QueryError::InvalidRecord);
        }
    }
    Ok(())
}

pub(super) fn evaluate(
    policy: &QueryPolicy,
    records: &[HistoryRecord],
    identities: &BTreeMap<Address, HistoryRecord>,
    frontier: u64,
) -> Result<Vec<FamilyEvaluation>, QueryError> {
    Ok(vec![
        evaluate_roster(policy, records)?,
        evaluate_autonomy(policy, records, identities)?,
        evaluate_audits(records, frontier)?,
    ])
}

struct ReportCheck {
    complete: bool,
    invalid: bool,
    support: Vec<Address>,
}

fn absent_report(reference: &ReportReference) -> Option<ReportCheck> {
    match reference {
        ReportReference::Absent { .. } => Some(ReportCheck {
            complete: true,
            invalid: true,
            support: Vec::new(),
        }),
        ReportReference::Unavailable => Some(ReportCheck {
            complete: false,
            invalid: false,
            support: Vec::new(),
        }),
        ReportReference::Present(_) => None,
    }
}

fn roster_report(
    policy: &QueryPolicy,
    records: &[HistoryRecord],
    retirement: &HistoryRecord,
    helper: &Address,
    family: &str,
    reference: &ReportReference,
) -> Result<ReportCheck, QueryError> {
    if let Some(result) = absent_report(reference) {
        return Ok(result);
    }
    let ReportReference::Present(address) = reference else {
        unreachable!("all absence variants handled above");
    };
    let Some(report) = find_record(records, address) else {
        return Ok(ReportCheck { complete: false, invalid: false, support: Vec::new() });
    };
    let HistoryFact::Roster(RosterFact::SelectionReport {
        retirement: target,
        helper: report_helper,
        effect_family,
        evaluator,
        attack_conditioned,
    }) = &report.fact else {
        return Err(QueryError::InvalidReference);
    };
    if target != &retirement.address || report_helper != helper || effect_family != family {
        return Err(QueryError::InvalidReference);
    }
    Ok(ReportCheck {
        complete: true,
        invalid: report.occurred_at >= retirement.occurred_at
            || evaluator != policy.evaluation_owner()
            || !attack_conditioned,
        support: vec![report.address.clone()],
    })
}

fn evaluate_roster(
    policy: &QueryPolicy,
    records: &[HistoryRecord],
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::RosterChurnNearHolds;
    let mut result = FamilyEvaluation { family, complete: true, findings: Vec::new() };
    let mut assessments = BTreeMap::<Address, Vec<_>>::new();
    for record in records {
        if let HistoryFact::Roster(RosterFact::Assessment { hold, evaluator, label }) = &record.fact {
            match find_record(records, hold) {
                Some(original) => {
                    if !matches!(&original.fact, HistoryFact::Roster(RosterFact::Hold { .. }))
                        || original.occurred_at >= record.occurred_at
                    {
                        return Err(QueryError::InvalidReference);
                    }
                }
                None => result.complete = false,
            }
            assessments.entry(hold.clone()).or_default().push((record, evaluator, *label));
        }
    }
    let mut resolved = BTreeMap::new();
    for (hold, labels) in assessments {
        // Validate every retained chain before selecting a retirement. Missing
        // source or a qualifying retirement report cannot hide label corruption.
        let original_sequence = find_record(records, &hold).map(|record| record.occurred_at);
        resolved.insert(hold, resolve_assessment_chain(
            original_sequence, labels, policy.evaluation_owner(), HoldLabel::Censored,
        )?);
    }

    let mut retired = BTreeSet::new();
    for retirement in records {
        let HistoryFact::Roster(RosterFact::Retirement {
            helper, effect_family, report,
        }) = &retirement.fact else {
            continue;
        };
        if !retired.insert((helper.clone(), effect_family.clone())) {
            return Err(QueryError::ConflictingRecord);
        }
        let report_check = roster_report(
            policy, records, retirement, helper, effect_family, report,
        )?;
        if !report_check.complete {
            result.complete = false;
        }
        if report_check.complete && !report_check.invalid {
            // A qualifying prior report makes this particular predicate false
            // regardless of whether any later label confirms a nearby hold.
            continue;
        }
        let mut confirmed_support = Vec::new();
        for hold in records {
            let HistoryFact::Roster(RosterFact::Hold {
                helper: hold_helper, effect_family: hold_family,
            }) = &hold.fact else {
                continue;
            };
            if hold_helper != helper || hold_family != effect_family
                || hold.occurred_at > retirement.occurred_at
                || retirement.occurred_at - hold.occurred_at > policy.roster_window()
            {
                continue;
            }
            let Some((label, support)) = resolved.get(&hold.address) else {
                result.complete = false;
                continue;
            };
            let Some(label) = label else {
                result.complete = false;
                continue;
            };
            if *label == HoldLabel::ConfirmedViolation {
                confirmed_support.push(hold.address.clone());
                confirmed_support.extend(support.iter().cloned());
            }
        }
        if report_check.invalid && !confirmed_support.is_empty() {
            let mut support = vec![retirement.address.clone()];
            support.extend(report_check.support);
            support.extend(confirmed_support);
            result.findings.push(QueryFinding {
                family,
                subject: retirement.address.clone(),
                support,
                reason: "retirement_without_qualified_report",
            });
        }
    }
    Ok(result)
}

fn state(record: &HistoryRecord) -> Option<(&Address, &Address, &str, AutonomyGrade)> {
    let HistoryFact::Autonomy(fact) = &record.fact else {
        return None;
    };
    let (principal, passport, family) = autonomy_subject(fact);
    let grade = match fact {
        AutonomyFact::Enrollment { grade, .. } | AutonomyFact::Demotion { grade, .. } => *grade,
        AutonomyFact::PassportChanged { .. } => AutonomyGrade::Propose,
        AutonomyFact::Promotion { to, .. } => *to,
        AutonomyFact::PromotionReport { .. } => return None,
    };
    Some((principal, passport, family, grade))
}

/// Validate the immediate predecessor against the complete accepted identity
/// history, including withdrawn observations. This private dependency check is
/// not live evidence: callers must separately require an active predecessor
/// before deriving a dwell or grade finding from its bytes.
fn predecessor<'a>(
    identities: &'a BTreeMap<Address, HistoryRecord>,
    record: &HistoryRecord,
    previous: &Address,
) -> Result<Option<(&'a HistoryRecord, AutonomyGrade)>, QueryError> {
    let (principal, passport, family, _) = state(record).ok_or(QueryError::InvalidRecord)?;
    let latest = identities
        .values()
        .filter(|candidate| {
            candidate.occurred_at < record.occurred_at
                && state(candidate).is_some_and(|(who, _, name, _)| {
                    who == principal && name == family
                })
        })
        .max_by_key(|candidate| candidate.occurred_at);
    if latest.is_some_and(|latest| &latest.address != previous) {
        return Err(QueryError::InvalidReference);
    }
    let Some(original) = identities.get(previous) else {
        return Ok(None);
    };
    let (prior_principal, prior_passport, prior_family, prior_grade) =
        state(original).ok_or(QueryError::InvalidReference)?;
    if principal != prior_principal || family != prior_family
        || original.occurred_at >= record.occurred_at
    {
        return Err(QueryError::InvalidReference);
    }
    match &record.fact {
        HistoryFact::Autonomy(AutonomyFact::PassportChanged { .. }) => {
            if passport == prior_passport {
                return Err(QueryError::InvalidRecord);
            }
        }
        _ => {
            if passport != prior_passport {
                return Err(QueryError::InvalidReference);
            }
        }
    }
    Ok(Some((original, prior_grade)))
}

/// Preserve the original state sequence even when its first, middle or terminal
/// observation is withdrawn. A missing state makes the family incomplete; it
/// cannot create a new enrollment, erase a demotion/passport reset, or lengthen
/// dwell by selecting an older state. The existing lifetime identity bound also
/// bounds this index, and a refused source boundary never commits any of it.
fn validate_autonomy_history(
    records: &[HistoryRecord],
    identities: &BTreeMap<Address, HistoryRecord>,
) -> Result<bool, QueryError> {
    let mut complete = true;
    let mut positions = BTreeSet::new();
    for record in identities.values() {
        let Some((principal, _, family, _)) = state(record) else {
            continue;
        };
        if !positions.insert((principal.clone(), family.to_owned(), record.occurred_at)) {
            // Even a same-boundary insert/retract cannot hide an ambiguous fork.
            return Err(QueryError::ConflictingRecord);
        }
        if find_record(records, &record.address).is_none() {
            complete = false;
        }
        let HistoryFact::Autonomy(fact) = &record.fact else {
            unreachable!("state accepts only autonomy observations");
        };
        let previous = match fact {
            AutonomyFact::Enrollment { .. } => {
                if identities.values().any(|candidate| {
                    candidate.occurred_at < record.occurred_at
                        && state(candidate).is_some_and(|(who, _, name, _)| {
                            who == principal && name == family
                        })
                }) {
                    return Err(QueryError::ConflictingRecord);
                }
                continue;
            }
            AutonomyFact::Demotion { previous, .. }
            | AutonomyFact::PassportChanged { previous, .. }
            | AutonomyFact::Promotion { previous, .. } => previous,
            AutonomyFact::PromotionReport { .. } => unreachable!("not an autonomy state"),
        };
        let Some((_, grade)) = predecessor(identities, record, previous)? else {
            complete = false;
            continue;
        };
        match fact {
            AutonomyFact::Demotion { grade: next, .. } if *next >= grade => {
                return Err(QueryError::InvalidRecord);
            }
            AutonomyFact::Promotion { to, .. } if *to <= grade => {
                return Err(QueryError::InvalidRecord);
            }
            _ => {}
        }
    }
    Ok(complete)
}

fn autonomy_report(
    policy: &QueryPolicy,
    records: &[HistoryRecord],
    promotion: &HistoryRecord,
    previous_grade: Option<AutonomyGrade>,
) -> Result<ReportCheck, QueryError> {
    let HistoryFact::Autonomy(AutonomyFact::Promotion {
        principal, passport, effect_family, previous, to, report: reference,
    }) = &promotion.fact else {
        return Err(QueryError::InvalidRecord);
    };
    if let Some(result) = absent_report(reference) {
        return Ok(result);
    }
    let ReportReference::Present(address) = reference else {
        unreachable!("absence variants handled above");
    };
    let Some(report) = find_record(records, address) else {
        return Ok(ReportCheck { complete: false, invalid: false, support: Vec::new() });
    };
    let HistoryFact::Autonomy(AutonomyFact::PromotionReport {
        promotion: target,
        principal: report_principal,
        passport: report_passport,
        effect_family: report_family,
        previous: report_previous,
        from,
        to: report_to,
        evaluator,
        attack_conditioned,
    }) = &report.fact else {
        return Err(QueryError::InvalidReference);
    };
    if target != &promotion.address || report_principal != principal
        || report_passport != passport || report_family != effect_family
        || report_previous != previous || report_to != to
        || previous_grade.is_some_and(|grade| *from != grade)
    {
        return Err(QueryError::InvalidReference);
    }
    let invalid = report.occurred_at >= promotion.occurred_at
        || evaluator != policy.evaluation_owner()
        || !attack_conditioned;
    Ok(ReportCheck {
        complete: previous_grade.is_some(),
        invalid,
        support: vec![report.address.clone()],
    })
}

fn evaluate_autonomy(
    policy: &QueryPolicy,
    records: &[HistoryRecord],
    identities: &BTreeMap<Address, HistoryRecord>,
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::AutonomyInflation;
    let complete = validate_autonomy_history(records, identities)?;
    let mut result = FamilyEvaluation { family, complete, findings: Vec::new() };
    for record in records {
        let HistoryFact::Autonomy(AutonomyFact::Promotion { previous, to, .. }) = &record.fact else {
            continue;
        };
        let original = predecessor(identities, record, previous)?;
        let prior = original.filter(|(record, _)| find_record(records, &record.address).is_some());
        if prior.is_none() {
            result.complete = false;
        }
        // Original identity can still reject a contradictory report binding.
        // It cannot qualify that report or supply a finding's absent support.
        let mut report = autonomy_report(
            policy, records, record, original.map(|(_, grade)| grade),
        )?;
        report.complete &= prior.is_some();
        if !report.complete {
            result.complete = false;
        }
        if report.invalid {
            let mut support = vec![record.address.clone()];
            support.extend(report.support);
            result.findings.push(QueryFinding {
                family,
                subject: record.address.clone(),
                support,
                reason: "promotion_without_qualified_report",
            });
        }
        if let Some((previous, grade)) = prior {
            let elapsed = record.occurred_at
                .checked_sub(previous.occurred_at)
                .ok_or(QueryError::ArithmeticOverflow)?;
            if elapsed < policy.minimum_dwell() {
                result.findings.push(QueryFinding {
                    family,
                    subject: record.address.clone(),
                    support: vec![record.address.clone(), previous.address.clone()],
                    reason: "autonomy_dwell_violation",
                });
            }
            if (*to as u8) > (grade as u8) + 1 {
                result.findings.push(QueryFinding {
                    family,
                    subject: record.address.clone(),
                    support: vec![record.address.clone(), previous.address.clone()],
                    reason: "autonomy_grade_skipped",
                });
            }
        }
    }
    Ok(result)
}

fn evaluate_audits(
    records: &[HistoryRecord],
    frontier: u64,
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::AuditFloorStarvation;
    let mut result = FamilyEvaluation { family, complete: true, findings: Vec::new() };
    for funding in records {
        if let HistoryFact::Audit(AuditFact::Funding { audit, .. }) = &funding.fact {
            match find_record(records, audit) {
                Some(schedule) => {
                    if !matches!(&schedule.fact, HistoryFact::Audit(AuditFact::Scheduled { .. }))
                        || schedule.occurred_at > funding.occurred_at
                    {
                        return Err(QueryError::InvalidReference);
                    }
                }
                None => result.complete = false,
            }
        }
    }
    for schedule in records {
        let HistoryFact::Audit(AuditFact::Scheduled {
            required_units, due_at, ..
        }) = &schedule.fact else {
            continue;
        };
        let mut units = 0u64;
        let mut support = vec![schedule.address.clone()];
        for funding in records {
            let HistoryFact::Audit(AuditFact::Funding { audit, units: allocation }) =
                &funding.fact else {
                    continue;
                };
            if audit != &schedule.address {
                continue;
            }
            support.push(funding.address.clone());
            // Late money cannot erase starvation at the original deadline.
            if funding.occurred_at <= *due_at {
                units = units.checked_add(*allocation).ok_or(QueryError::ArithmeticOverflow)?;
            }
        }
        if units < *required_units {
            if frontier < *due_at {
                result.complete = false;
            } else {
                result.findings.push(QueryFinding {
                    family,
                    subject: schedule.address.clone(),
                    support,
                    reason: "scheduled_audit_not_funded",
                });
            }
        }
    }
    Ok(result)
}
