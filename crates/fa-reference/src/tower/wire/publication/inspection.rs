//! Canonical projection of original supervisor situation knowledge.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::observed::replay::situation::{
    FileSituationBasis, FileSituationOrigin,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileInspectionPage, FileInspectionResult, MAX_INSPECTION_ATTEMPTS,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::action::{ActionState, Purpose};
use crate::address_resolver::Address;
use crate::tower::publication::{LinkedSituation, request_address_for};

pub(super) fn encode(
    result: &FileInspectionResult,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    encode_selected(result, None, encoder)
}

pub(super) fn encode_linked(
    linked: &LinkedSituation,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if address != linked.query_address() {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    encode_selected(linked.native(), Some(linked.registration()), encoder)
}

fn encode_selected(
    result: &FileInspectionResult,
    registration: Option<&Address>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let (field, value) = match result {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge", known(value, *basis, registration, encoder)?
        ),
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
            authority_required: "supervisor_inspection",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_inspection")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Err(ActorError::IdempotencyConflict) => (
            "error",
            object_json(&[("code", quote("idempotency_conflict"))]),
        ),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote(if registration.is_some() {
            "fa.publication-situation/2"
        } else {
            "fa.publication-situation/1"
        })),
        ("operation", quote("situation")),
        ("historical", "true".to_owned()),
        ("current_eligibility", quote("not_observed")),
        (field, value),
    ]))
}

fn known(
    page: &FileInspectionPage,
    basis: FileSituationBasis,
    registration: Option<&Address>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    // Public response fields cannot replace this sealed capture's origin,
    // scope or clocks. The value contains no policy, payload or helper input.
    let summary = page.summary();
    let end = page
        .offset()
        .checked_add(page.attempts().len())
        .ok_or(TowerEncodingError::InvalidKnowledge)?;
    if basis != page.basis()
        || page.attempts().len() > MAX_INSPECTION_ATTEMPTS
        || end > summary.attempts
        || page.enumeration_complete() != (end == summary.attempts)
        || (page.enumeration_complete() && page.continuation().is_some())
        || (!page.enumeration_complete() && page.attempts().len() != MAX_INSPECTION_ATTEMPTS)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let scope = basis.scope;
    let purpose = match scope.purpose {
        Purpose::Effect => "effect",
        Purpose::Experiment => "experiment",
    };
    let origin = match basis.origin {
        FileSituationOrigin::AcknowledgedOwner => "acknowledged_owner",
        FileSituationOrigin::CanonicalImage => "canonical_image",
    };
    let scope = object_json(&[
        ("tenant", number(scope.tenant)),
        ("principal", number(scope.principal)),
        ("run", number(scope.run)),
        ("branch", number(scope.branch)),
        ("authority", number(scope.authority)),
        ("purpose", quote(purpose)),
    ]);
    let native_basis = object_json(&[
        ("scope", scope),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        (
            "recorded_at",
            optional(basis.recorded_at.map(|tick| tick.0)),
        ),
        ("origin", quote(origin)),
        ("anchored_through", optional(basis.anchored_through)),
    ]);
    let summary = object_json(&[
        ("total_units", number(summary.total_units)),
        ("available_units", number(summary.available_units)),
        ("reserved_units", number(summary.reserved_units)),
        ("charged_units", number(summary.charged_units)),
        ("policy_epoch", number(summary.policy_epoch)),
        ("dispatcher_epoch", number(summary.dispatcher_epoch)),
        ("suspended", summary.suspended.to_string()),
        ("stop_recorded", summary.stop_recorded.to_string()),
        ("executions", number(summary.executions)),
        ("attempts", number(summary.attempts)),
        ("retained_requests", number(summary.retained_requests)),
        ("pending_attempts", number(summary.pending_attempts)),
        ("unresolved_attempts", number(summary.unresolved_attempts)),
        (
            "irrecoverably_unknown_attempts",
            number(summary.irrecoverably_unknown_attempts),
        ),
        ("terminal_attempts", number(summary.terminal_attempts)),
    ]);
    let mut attempts = Vec::with_capacity(page.attempts().len());
    for row in page.attempts() {
        let target = row.target;
        let target = object_json(&[
            ("adapter", number(target.adapter)),
            ("object", number(target.object)),
            ("contract_version", number(target.contract_version)),
            ("expected_version", number(target.expected_version)),
            ("generation", number(target.generation)),
        ]);
        let mut fields = Vec::with_capacity(if registration.is_some() { 10 } else { 8 });
        fields.extend([
            ("address", quote(&row.address.to_string())),
            ("attempt", number(row.attempt)),
            ("stage", quote(stage(row.stage))),
            ("target", target),
            ("units", number(row.units)),
            ("payload_bytes", number(row.payload_bytes)),
            ("proposal_policy_epoch", number(row.proposal_policy_epoch)),
            ("deadline", number(row.deadline.0)),
        ]);
        if let Some(registration) = registration {
            let address = row.external_request.map(|request|
                request_address_for(basis.scope, registration, request)
                    .map(|address| quote(&address.canonical_string()))
                    .map_err(|_| TowerEncodingError::InvalidKnowledge)
            ).transpose()?.unwrap_or_else(|| "null".to_owned());
            fields.push(("external_request", optional(row.external_request)));
            fields.push(("request_address", address));
        }
        attempts.push(encoder.bounded(object_json(&fields))?);
    }
    let value = encoder.bounded(object_json(&[
        ("summary", summary),
        ("offset", number(page.offset())),
        (
            "continuation",
            optional(page.continuation().map(|cursor| cursor.id())),
        ),
        (
            "enumeration_complete",
            page.enumeration_complete().to_string(),
        ),
        ("attempts", format!("[{}]", attempts.join(","))),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", native_basis),
        ("value", value),
    ]))
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), number)
}

pub(super) fn stage(value: ActionState) -> &'static str {
    match value {
        ActionState::Proposed => "proposed",
        ActionState::Prepared => "prepared",
        ActionState::Reviewing => "reviewing",
        ActionState::Authorized => "authorized",
        ActionState::Dispatching => "dispatching",
        ActionState::Confirmed => "confirmed",
        ActionState::Denied => "denied",
        ActionState::Cancelled => "cancelled",
        ActionState::Unknown => "unknown",
        ActionState::ConfirmedNotExecuted => "confirmed_not_executed",
        ActionState::IrrecoverablyUnknown => "irrecoverably_unknown",
    }
}
