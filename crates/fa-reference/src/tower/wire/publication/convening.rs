//! Sealed review plans and explicit process-local admission dispositions.
use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileConveneBasis, FileConveneBlockReason, FileConveneError, FileConveneOutcome,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedConvene, LinkedConveneNext, request_address_for};
use crate::tower::request::convening::encode_basis;

pub(super) fn encode_next(
    linked: &LinkedConveneNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    if linked.request() == 0
        || linked.round() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || idempotency_key.is_some_and(|key| key != linked.round().to_string())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            validate_basis(*basis, linked.registration(), address, linked.request())?;
            if value.basis() != *basis
                || value.request() != linked.request()
                || value.round() != linked.round()
                || value.can_convene() != value.block_reason().is_none()
                || value.can_convene() != value.convene_request().is_some()
            {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            let action = linked
                .action()
                .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
            let action = action
                .map(|action| crate::tower::encode_request(&action))
                .transpose()
                .map_err(|_| TowerEncodingError::InvalidKnowledge)?
                .unwrap_or_else(|| "null".into());
            let value = object_json(&[
                ("can_convene", value.can_convene().to_string()),
                (
                    "block_reason",
                    value
                        .block_reason()
                        .map_or_else(|| "null".into(), |r| quote(block_reason(r))),
                ),
                ("action", action),
                ("cost_estimate", unknown()),
                ("expected_information_gain", unknown()),
                ("ranking", unknown()),
                ("current_eligibility", unknown()),
            ]);
            (
                "knowledge",
                object_json(&[
                    ("state", quote("known")),
                    ("basis", encode_basis(*basis)),
                    ("value", value),
                ]),
            )
        }
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-convene-next/1")),
        ("operation", quote("next")),
        ("scope", quote("original_held_review")),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("request", number(linked.request())),
        ("round", number(linked.round())),
        ("live_evidence_read", "false".into()),
        ("clock_read", "false".into()),
        ("effect_authority", "false".into()),
        ("payload_disclosed", "false".into()),
        ("automatic_retry", "false".into()),
        ("idempotency_scope", quote("live_convening_grant")),
        ("action_completeness", quote("single_connected_held_review")),
        (field, result),
    ]))
}

pub(super) fn encode_convene(
    linked: &LinkedConvene,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    let expected = attempted.expected();
    validate_basis(
        expected,
        linked.registration(),
        address,
        attempted.request(),
    )?;
    if attempted.round() == 0
        || expected.held_round == 0
        || operation_id != linked.query_operation_id()
        || address.generation != linked.registration().generation
        || idempotency_key != Some(attempted.round().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            if value.request() != attempted
                || *basis != expected
                || (value.outcome() == FileConveneOutcome::Opened) != value.opened().is_some()
            {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            let opened = if let Some(opened) = value.opened() {
                if opened.journal_revision() <= expected.journal_revision
                    || opened.input_revision() < expected.input_revision
                    || expected
                        .recorded_at
                        .is_some_and(|at| opened.started_at() < at)
                    || opened.started_at() >= expected.deadline
                {
                    return Err(TowerEncodingError::InvalidKnowledge);
                }
                object_json(&[
                    ("journal_revision", number(opened.journal_revision())),
                    ("input_revision", number(opened.input_revision())),
                    ("started_at", number(opened.started_at().0)),
                ])
            } else {
                "null".into()
            };
            let value = object_json(&[
                ("outcome", quote(outcome(value.outcome()))),
                (
                    "round_opened",
                    (value.outcome() == FileConveneOutcome::Opened).to_string(),
                ),
                ("opened", opened),
            ]);
            (
                "knowledge",
                object_json(&[
                    ("state", quote("known")),
                    ("basis", encode_basis(*basis)),
                    ("value", value),
                ]),
            )
        }
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-convene/1")),
        ("operation", quote("convene")),
        ("scope", quote("original_held_review")),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("request", number(attempted.request())),
        ("round", number(attempted.round())),
        ("expected", encode_basis(expected)),
        ("live_evidence_read", "false".into()),
        ("clock_read", "false".into()),
        ("effect_authority", "false".into()),
        ("payload_disclosed", "false".into()),
        ("automatic_retry", "false".into()),
        ("queued_work_is_durable", "false".into()),
        ("idempotency_scope", quote("live_convening_grant")),
        (field, result),
    ]))
}

fn registration(
    registration: &Address,
    original: &Address,
    address: &Address,
) -> Result<(), TowerEncodingError> {
    if original != address
        || registration.kind != Noun::Run
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || address
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
        || address.tenant != registration.tenant
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn validate_basis(
    basis: FileConveneBasis,
    registration: &Address,
    address: &Address,
    request: u64,
) -> Result<(), TowerEncodingError> {
    let expected = request_address_for(basis.scope, registration, request)
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if basis.scope.purpose != Purpose::Effect
        || basis.request != request
        || request == 0
        || expected.without_generation() != address.without_generation()
        || basis.scope.tenant.to_string() != registration.tenant
        || basis.scope.run.to_string() != registration.id
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn unavailable<T>(value: &Knowledge<T, FileConveneBasis>) -> Result<String, TowerEncodingError> {
    match value {
        Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        } => Ok(object_json(&[
            ("state", quote("unknown")),
            ("reason", quote("controller_unavailable")),
        ])),
        Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown,
        } => Ok(object_json(&[
            ("state", quote("unknown")),
            ("reason", quote("outcome_unknown")),
        ])),
        Knowledge::Withheld {
            authority_required: "supervisor_convene",
        } => Ok(object_json(&[
            ("state", quote("withheld")),
            ("authority_required", quote("supervisor_convene")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn error_json(error: FileConveneError) -> String {
    object_json(&[(
        "code",
        quote(match error {
            FileConveneError::MalformedRequest => "malformed_request",
            FileConveneError::IdempotencyConflict => "idempotency_conflict",
            FileConveneError::StalePredecessor => "stale_predecessor",
            FileConveneError::Capacity => "capacity",
            FileConveneError::Unsupported => "unsupported",
            FileConveneError::Unavailable => "unavailable",
        }),
    )])
}

fn block_reason(reason: FileConveneBlockReason) -> &'static str {
    match reason {
        FileConveneBlockReason::NotHeld => "not_held",
        FileConveneBlockReason::Stopped => "stopped",
        FileConveneBlockReason::Suspended => "suspended",
        FileConveneBlockReason::UnsupportedHistory => "unsupported_history",
        FileConveneBlockReason::ActiveReview => "active_review",
        FileConveneBlockReason::StaleReview => "stale_review",
        FileConveneBlockReason::RoundAlreadyUsed => "round_already_used",
        FileConveneBlockReason::RoundLimit => "round_limit",
        FileConveneBlockReason::OperationLimit => "operation_limit",
        FileConveneBlockReason::PendingOperation => "pending_operation",
        FileConveneBlockReason::DeadlineElapsed => "deadline_elapsed",
    }
}
fn outcome(outcome: FileConveneOutcome) -> &'static str {
    match outcome {
        FileConveneOutcome::Pending => "pending",
        FileConveneOutcome::Opened => "opened",
        FileConveneOutcome::Failed => "failed",
        FileConveneOutcome::Abandoned => "abandoned",
    }
}
fn unknown() -> String {
    object_json(&[
        ("state", quote("unknown")),
        ("reason", quote("not_observed")),
    ])
}
fn number(value: u64) -> String {
    quote(&value.to_string())
}
