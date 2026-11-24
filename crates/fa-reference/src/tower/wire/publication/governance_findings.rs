//! Original calibration incidents, with unsupported full query families kept
//! explicitly unknown. Encoding never opens an incident or changes policy.

use super::governance_ledger::basis_json;
use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAndonCalibrationFindings, FileCredibilityCalibrationSnapshot,
    FileGovernanceFindingsSnapshot, FileGovernanceLedgerBasis,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::governance_assurance::RiskTheaterFamily;
use crate::observation_algebra::{Knowledge as QueryKnowledge, UnknownReason as QueryUnknown};
use crate::round::review::andon::{MAX_FALSE_HOLDS, MAX_WINDOWS};
use crate::tower::publication::{LinkedGovernanceFindings, PublicationLimits};
use crate::tower::{RESPONSE_SCHEMA, ReasonCode, ResponseStatus, TowerEnvelope};

pub(super) fn validate_envelope(
    linked: &LinkedGovernanceFindings,
    envelope: &TowerEnvelope<PublicationLimits>,
) -> Result<(), TowerEncodingError> {
    fn unobserved<T>(value: &QueryKnowledge<T>) -> bool {
        matches!(
            value,
            QueryKnowledge::Unknown {
                reason: QueryUnknown::Unavailable
            }
        )
    }
    let reason = match linked.native() {
        Ok(Knowledge::Known { .. }) => None,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
        | Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        _ => return Err(TowerEncodingError::InvalidKnowledge),
    };
    let status = if reason.is_some() {
        ResponseStatus::Error
    } else {
        ResponseStatus::Ok
    };
    let expected_missing = if reason.is_some() {
        vec![envelope.address.clone()]
    } else {
        Vec::new()
    };
    if envelope.schema_version != RESPONSE_SCHEMA
        || envelope.status != status
        || envelope.reason_code != reason
        || !envelope.retry_safe
        || envelope.missing != expected_missing
        || envelope.would_change_this != expected_missing
        || !unobserved(&envelope.generation)
        || !unobserved(&envelope.control_seq)
        || !unobserved(&envelope.source_frontiers)
        || !unobserved(&envelope.budgets_remaining)
        || !unobserved(&envelope.realized_cost)
        || !unobserved(&envelope.affordances)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

pub(super) fn encode(
    linked: &LinkedGovernanceFindings,
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
        }) => ("knowledge", unknown("controller_unavailable")),
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
        ("schema", quote("fa.governance-findings/1")),
        ("operation", quote("findings")),
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
    snapshot: &FileGovernanceFindingsSnapshot,
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
        || snapshot.risk_theater().len() != RiskTheaterFamily::ALL.len()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let credibility = match snapshot.credibility() {
        Some(value) => observed(credibility(value)?, basis),
        None => unknown("source_not_installed"),
    };
    let andon = match snapshot.andon() {
        Some(value) => observed(andon(value, encoder)?, basis),
        None => unknown("source_not_installed"),
    };
    let mut coverage = Vec::with_capacity(RiskTheaterFamily::ALL.len());
    for family in RiskTheaterFamily::ALL {
        if !matches!(
            snapshot.risk_theater().get(&family),
            Some(QueryKnowledge::Unknown {
                reason: QueryUnknown::LowerLayerUnverified,
            })
        ) {
            // Even the Andon incident subquery does not establish the full
            // family, which also needs incident closure-mode provenance.
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        coverage.push((family_name(family), unknown("lower_layer_unverified")));
    }
    encoder.bounded(observed(
        object_json(&[
            ("credibility_incident", credibility),
            ("andon_incidents", andon),
            ("risk_theater", object_json(&coverage)),
        ]),
        basis,
    ))
}

fn credibility(value: &FileCredibilityCalibrationSnapshot) -> Result<String, TowerEncodingError> {
    let protocol = value.protocol();
    if [
        protocol.domain,
        protocol.stratum,
        protocol.period,
        protocol.false_stop_budget,
    ]
    .contains(&0)
        || value.calibration_incident_open()
            != (value.lifetime_false_stops() >= protocol.false_stop_budget)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(object_json(&[
        (
            "evaluation_scope",
            object_json(&[
                ("domain", number(protocol.domain)),
                ("stratum", number(protocol.stratum)),
                ("period", number(protocol.period)),
            ]),
        ),
        ("ledger_revision", number(value.revision())),
        ("policy_generation", number(value.policy_generation())),
        ("lifetime_false_stops", number(value.lifetime_false_stops())),
        ("false_stop_budget", number(protocol.false_stop_budget)),
        (
            "calibration_incident_open",
            value.calibration_incident_open().to_string(),
        ),
    ]))
}

fn andon(
    value: &FileAndonCalibrationFindings,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let identity = value.identity();
    if identity.deployment == 0
        || identity.evaluator == 0
        || value.inspected_windows() == 0
        || value.inspected_windows() > MAX_WINDOWS
        || value.incidents().len() > value.inspected_windows()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut incidents = Vec::with_capacity(value.incidents().len());
    for (&window, accounting) in value.incidents() {
        let budget = accounting.budget;
        let incident = accounting
            .incident
            .as_ref()
            .ok_or(TowerEncodingError::InvalidKnowledge)?;
        if window == 0
            || budget.window != window
            || budget.start >= budget.end
            || accounting.false_stops == 0
            || accounting.false_stops > MAX_FALSE_HOLDS as u64
            || accounting.false_stops < budget.false_stops_allowed
            || accounting.total_cost
                != u128::from(accounting.false_stops) * u128::from(budget.cost_per_stop)
            || incident.window != window
            || incident.opened_by_round == 0
            || incident.rounds.len() != accounting.false_stops as usize
            || !incident.rounds.contains(&incident.opened_by_round)
            || incident.rounds.contains(&0)
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        incidents.push(encoder.bounded(object_json(&[
            ("window", number(window)),
            ("start", number(budget.start)),
            ("end", number(budget.end)),
            ("false_stops_allowed", number(budget.false_stops_allowed)),
            ("cost_per_stop", number(budget.cost_per_stop)),
            ("false_stops", number(accounting.false_stops)),
            ("total_cost", number(accounting.total_cost)),
            ("opened_by_round", number(incident.opened_by_round)),
            (
                "support_rounds",
                format!(
                    "[{}]",
                    incident.rounds.iter().map(number).collect::<Vec<_>>().join(",")
                ),
            ),
        ]))?);
    }
    encoder.bounded(object_json(&[
        ("deployment", number(identity.deployment)),
        ("evaluator", number(identity.evaluator)),
        ("inspected_windows", number(value.inspected_windows())),
        ("incidents", format!("[{}]", incidents.join(","))),
    ]))
}

fn observed(value: String, basis: FileGovernanceLedgerBasis) -> String {
    object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", value),
    ])
}

fn unknown(reason: &str) -> String {
    object_json(&[("state", quote("unknown")), ("reason", quote(reason))])
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn family_name(family: RiskTheaterFamily) -> &'static str {
    match family {
        RiskTheaterFamily::ConsequenceDrift => "consequence_drift",
        RiskTheaterFamily::AndonClosureMode => "andon_closure_mode",
        RiskTheaterFamily::RosterChurnNearHolds => "roster_churn_near_holds",
        RiskTheaterFamily::CoverageErosion => "coverage_erosion",
        RiskTheaterFamily::AutonomyInflation => "autonomy_inflation",
        RiskTheaterFamily::AuditFloorStarvation => "audit_floor_starvation",
    }
}
