//! A complete bounded projection of the original owner's evaluation accounting.
//! Counters are exact decimal strings; labels, evidence bytes and policy or
//! committee profiles are deliberately outside this disclosure.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAndonLedgerSnapshot, FileCredibilityLedgerSnapshot, FileGovernanceLedgerBasis,
    FileGovernanceLedgerSnapshot,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::action::consequence::oversight::credibility::{
    Fraction, MAX_EVALUATION_CASES, QualificationFailure,
};
use crate::address_resolver::{Address, Noun};
use crate::round::review::andon::{MAX_FALSE_HOLDS, MAX_WINDOWS};
use crate::round::{MAX_FIELD_LEN, MAX_MEMBERS};
use crate::tower::publication::LinkedGovernanceLedger;

pub(super) fn encode(
    linked: &LinkedGovernanceLedger,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let registration = linked.registration();
    if address != linked.query_address()
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || address.without_generation() != registration.without_generation()
        || registration.kind != Noun::Run
        || registration.generation.is_none_or(|value| value == 0)
        || address
            .generation
            .is_some_and(|value| Some(value) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, value) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", known(value, *basis, registration, encoder)?)
        }
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("unknown")),
                ("reason", quote("controller_unavailable")),
            ]),
        ),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_governance_ledger")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.governance-ledger/1")),
        ("operation", quote("ledger")),
        ("registration", quote(&registration.canonical_string())),
        ("query_address", quote(&address.canonical_string())),
        ("disclosure", quote("recorded_governance_accounting")),
        ("current_eligibility", quote("not_observed")),
        ("label_truth", quote("not_verified")),
        ("population_calibration", quote("not_established")),
        ("effect_authority", "false".to_owned()),
        (field, value),
    ]))
}

fn known(
    snapshot: &FileGovernanceLedgerSnapshot,
    basis: FileGovernanceLedgerBasis,
    registration: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let scope = basis.scope;
    if snapshot.basis() != basis
        || scope.purpose != Purpose::Effect
        || [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
        ]
        .contains(&0)
        || registration.tenant != scope.tenant.to_string()
        || registration.id != scope.run.to_string()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let credibility = match snapshot.credibility() {
        Some(value) => observed_section(credibility(value, encoder)?, basis),
        None => absent_section("original_owner_credibility_ledger", basis),
    };
    let andon = match snapshot.andon() {
        Some(value) => observed_section(andon(value, encoder)?, basis),
        None => absent_section("original_owner_andon_ledger", basis),
    };
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        (
            "value",
            object_json(&[("credibility", credibility), ("andon", andon)]),
        ),
    ]))
}

fn credibility(
    ledger: &FileCredibilityLedgerSnapshot,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let protocol = ledger.protocol();
    if ledger.members().len() > MAX_MEMBERS
        || ledger.retained_cases() > MAX_EVALUATION_CASES
        || ledger.scoped_cases() > ledger.retained_cases()
        || ledger.pending_cases() > ledger.scoped_cases()
        || ledger.censored_cases() > ledger.scoped_cases()
        || ledger.failures().len() > 3 * MAX_MEMBERS + 5
        || [
            protocol.domain,
            protocol.stratum,
            protocol.period,
            protocol.minimum_violation_origins,
            protocol.minimum_benign_origins,
            protocol.false_stop_budget,
            protocol.precision_floor.numerator,
            protocol.recall_floor.numerator,
        ]
        .contains(&0)
        || u128::from(protocol.minimum_violation_origins)
            + u128::from(protocol.minimum_benign_origins)
            > MAX_EVALUATION_CASES as u128
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut members = Vec::with_capacity(ledger.members().len());
    for (member, metrics) in ledger.members() {
        if member.is_empty() || member.len() > MAX_FIELD_LEN {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        members.push(encoder.bounded(object_json(&[
            ("member", quote(member)),
            ("true_positives", number(metrics.true_positives)),
            ("false_negatives", number(metrics.false_negatives)),
            ("false_positives", number(metrics.false_positives)),
            ("true_negatives", number(metrics.true_negatives)),
            ("missing_origins", number(metrics.missing_origins)),
            ("abstaining_origins", number(metrics.abstaining_origins)),
        ]))?);
    }
    let mut failures = Vec::with_capacity(ledger.failures().len());
    for failure in ledger.failures() {
        let (reason, member) = match failure {
            QualificationFailure::PendingLabels => ("pending_labels", None),
            QualificationFailure::CensoredLabels => ("censored_labels", None),
            QualificationFailure::InsufficientViolationOrigins => {
                ("insufficient_violation_origins", None)
            }
            QualificationFailure::InsufficientBenignOrigins => {
                ("insufficient_benign_origins", None)
            }
            QualificationFailure::Precision(member) => ("precision", Some(member)),
            QualificationFailure::Recall(member) => ("recall", Some(member)),
            QualificationFailure::FalsePositiveRate(member) => {
                ("false_positive_rate", Some(member))
            }
            QualificationFailure::FalseStopBudgetExhausted => ("false_stop_budget_exhausted", None),
        };
        if member.is_some_and(|value| value.is_empty() || value.len() > MAX_FIELD_LEN) {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        failures.push(object_json(&[
            ("reason", quote(reason)),
            (
                "member",
                member.map_or_else(|| "null".to_owned(), |value| quote(value)),
            ),
        ]));
    }
    encoder.bounded(object_json(&[
        (
            "evaluation_scope",
            object_json(&[
                ("domain", number(protocol.domain)),
                ("stratum", number(protocol.stratum)),
                ("period", number(protocol.period)),
            ]),
        ),
        (
            "minimum_violation_origins",
            number(protocol.minimum_violation_origins),
        ),
        (
            "minimum_benign_origins",
            number(protocol.minimum_benign_origins),
        ),
        ("precision_floor", fraction(protocol.precision_floor)?),
        ("recall_floor", fraction(protocol.recall_floor)?),
        (
            "false_positive_ceiling",
            fraction(protocol.false_positive_ceiling)?,
        ),
        ("ledger_revision", number(ledger.revision())),
        ("policy_generation", number(ledger.policy_generation())),
        ("scoped_cases", number(ledger.scoped_cases())),
        ("retained_cases", number(ledger.retained_cases())),
        ("pending_cases", number(ledger.pending_cases())),
        ("censored_cases", number(ledger.censored_cases())),
        ("violation_origins", number(ledger.violation_origins())),
        ("benign_origins", number(ledger.benign_origins())),
        (
            "lifetime_false_stops",
            number(ledger.lifetime_false_stops()),
        ),
        ("false_stop_budget", number(protocol.false_stop_budget)),
        (
            "calibration_incident_open",
            ledger.calibration_incident_open().to_string(),
        ),
        ("members", format!("[{}]", members.join(","))),
        (
            "qualification_failures",
            format!("[{}]", failures.join(",")),
        ),
    ]))
}

fn andon(
    ledger: &FileAndonLedgerSnapshot,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let identity = ledger.identity();
    if identity.deployment == 0 || identity.evaluator == 0 || ledger.windows().len() > MAX_WINDOWS {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut windows = Vec::with_capacity(ledger.windows().len());
    for (&window, accounting) in ledger.windows() {
        let budget = accounting.budget;
        if window == 0
            || budget.window != window
            || budget.start >= budget.end
            || accounting.false_stops > MAX_FALSE_HOLDS as u64
            || accounting.total_cost
                != u128::from(accounting.false_stops) * u128::from(budget.cost_per_stop)
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        let incident = match &accounting.incident {
            Some(incident) => {
                if incident.window != window
                    || incident.opened_by_round == 0
                    || incident.rounds.is_empty()
                    || incident.rounds.len() > MAX_FALSE_HOLDS
                    || !incident.rounds.contains(&incident.opened_by_round)
                {
                    return Err(TowerEncodingError::InvalidKnowledge);
                }
                object_json(&[
                    ("window", number(incident.window)),
                    ("opened_by_round", number(incident.opened_by_round)),
                    ("contributing_rounds", number(incident.rounds.len())),
                ])
            }
            None => "null".to_owned(),
        };
        windows.push(encoder.bounded(object_json(&[
            ("window", number(window)),
            ("start", number(budget.start)),
            ("end", number(budget.end)),
            ("false_stops_allowed", number(budget.false_stops_allowed)),
            ("cost_per_stop", number(budget.cost_per_stop)),
            ("false_stops", number(accounting.false_stops)),
            ("total_cost", number(accounting.total_cost)),
            ("calibration_incident", incident),
        ]))?);
    }
    encoder.bounded(object_json(&[
        ("deployment", number(identity.deployment)),
        ("evaluator", number(identity.evaluator)),
        ("windows", format!("[{}]", windows.join(","))),
    ]))
}

fn observed_section(value: String, basis: FileGovernanceLedgerBasis) -> String {
    object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", value),
    ])
}

fn absent_section(domain: &str, basis: FileGovernanceLedgerBasis) -> String {
    object_json(&[
        ("state", quote("absent")),
        ("closed_domain", quote(domain)),
        ("basis", basis_json(basis)),
    ])
}

pub(super) fn basis_json(basis: FileGovernanceLedgerBasis) -> String {
    let scope = basis.scope;
    object_json(&[
        (
            "scope",
            object_json(&[
                ("tenant", number(scope.tenant)),
                ("principal", number(scope.principal)),
                ("run", number(scope.run)),
                ("branch", number(scope.branch)),
                ("authority", number(scope.authority)),
                ("purpose", quote("effect")),
            ]),
        ),
        ("origin", quote("acknowledged_owner")),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
        (
            "recorded_at",
            basis
                .recorded_at
                .map_or_else(|| "null".to_owned(), |tick| number(tick.0)),
        ),
    ])
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn fraction(value: Fraction) -> Result<String, TowerEncodingError> {
    if value.denominator == 0 || value.numerator > value.denominator {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(object_json(&[
        ("numerator", number(value.numerator)),
        ("denominator", number(value.denominator)),
    ]))
}
