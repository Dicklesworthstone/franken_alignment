//! Sealed plans and receipts for one original undispatched cancellation.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileCancellationBlockReason, FileCancellationError, FileCancellationPlan,
    FileCancellationReceipt, FileHoldDiagnosisBasis,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::action::{ActionState, Purpose};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedCancellation, LinkedCancellationNext, request_address_for};
use crate::tower::request::resolution::encode_basis;

pub(super) fn encode_next(
    linked: &LinkedCancellationNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    if linked.operation() == 0
        || linked.request() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || idempotency_key.is_some_and(|key| key != linked.operation().to_string())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", next_known(linked, value, *basis, encoder)?)
        }
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-cancellation-next/1")),
        ("operation", quote("next")),
        ("scope", quote("original_undispatched_request")),
        ("request", number(linked.request())),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("cancellation_operation", number(linked.operation())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("idempotency_scope", quote("live_cancellation_grant")),
        (
            "action_completeness",
            quote("single_connected_undispatched_cancellation"),
        ),
        (field, result),
    ]))
}

fn next_known(
    linked: &LinkedCancellationNext,
    plan: &FileCancellationPlan,
    basis: FileHoldDiagnosisBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    validate_basis(
        basis,
        linked.registration(),
        linked.query_address(),
        linked.request(),
    )?;
    if basis != plan.basis()
        || plan.operation() != linked.operation()
        || plan.can_cancel() != plan.request().is_some()
        || plan.can_cancel() != plan.block_reason().is_none()
        || (plan.can_cancel()
            && !matches!(
                plan.stage(),
                ActionState::Proposed
                    | ActionState::Prepared
                    | ActionState::Reviewing
                    | ActionState::Authorized
            ))
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let action = linked
        .action()
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let action = match action {
        Some(request) => crate::tower::encode_request(&request)
            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
        None => "null".to_owned(),
    };
    let value = encoder.bounded(object_json(&[
        ("stage", quote(super::inspection::stage(plan.stage()))),
        ("can_cancel", plan.can_cancel().to_string()),
        (
            "block_reason",
            plan.block_reason()
                .map_or_else(|| "null".to_owned(), |reason| quote(block_reason(reason))),
        ),
        ("action", action),
        ("cost_estimate", unknown()),
        ("expected_information_gain", unknown()),
        ("ranking", unknown()),
        ("current_eligibility", unknown()),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", encode_basis(basis)),
        ("value", value),
    ]))
}

pub(super) fn encode_cancellation(
    linked: &LinkedCancellation,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    validate_basis(
        attempted.expected(),
        linked.registration(),
        address,
        linked.request(),
    )?;
    if attempted.operation() == 0
        || operation_id != linked.query_operation_id()
        || address.generation != linked.registration().generation
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge",
            cancellation_known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-cancellation/1")),
        ("operation", quote("resolve")),
        ("scope", quote("original_undispatched_request")),
        ("request", number(linked.request())),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("cancellation_operation", number(attempted.operation())),
        ("expected", encode_basis(attempted.expected())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("seal_attempted", "false".to_owned()),
        ("idempotency_scope", quote("live_cancellation_grant")),
        (field, result),
    ]))
}

fn cancellation_known(
    linked: &LinkedCancellation,
    receipt: &FileCancellationReceipt,
    basis: FileHoldDiagnosisBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    validate_basis(
        after,
        linked.registration(),
        linked.query_address(),
        linked.request(),
    )?;
    if after != basis
        || before != linked.attempted().expected()
        || receipt.operation() != linked.attempted().operation()
        || after.scope != before.scope
        || after.request != before.request
        || after.attempt != before.attempt
        || after.recorded_at != before.recorded_at
        || before.journal_revision.checked_add(1) != Some(after.journal_revision)
        || after.request_generation < before.request_generation
        || after.control_sequence < before.control_sequence
        || receipt.stage() != ActionState::Cancelled
        || !matches!(
            receipt.before_stage(),
            ActionState::Proposed
                | ActionState::Prepared
                | ActionState::Reviewing
                | ActionState::Authorized
        )
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        (
            "before_stage",
            quote(super::inspection::stage(receipt.before_stage())),
        ),
        ("stage", quote(super::inspection::stage(receipt.stage()))),
        ("refunded_units", number(receipt.refunded_units())),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", encode_basis(after)),
        ("value", value),
    ]))
}

fn registration(
    registration: &Address,
    query: &Address,
    outer: &Address,
) -> Result<(), TowerEncodingError> {
    if query != outer
        || outer.kind != Noun::Attempt
        || outer.tenant != registration.tenant
        || registration.kind != Noun::Run
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || outer
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn validate_basis(
    basis: FileHoldDiagnosisBasis,
    registration: &Address,
    query: &Address,
    request: u64,
) -> Result<(), TowerEncodingError> {
    let expected = request_address_for(basis.scope, registration, basis.request)
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if basis.scope.purpose != Purpose::Effect
        || basis.request == 0
        || basis.attempt == 0
        || basis.request != request
        || expected.without_generation() != query.without_generation()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn unavailable<T>(
    knowledge: &Knowledge<T, FileHoldDiagnosisBasis>,
) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        } => Ok(unknown()),
        Knowledge::Withheld {
            authority_required: "supervisor_cancellation",
        } => Ok(object_json(&[
            ("state", quote("withheld")),
            ("authority_required", quote("supervisor_cancellation")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn unknown() -> String {
    object_json(&[
        ("state", quote("unknown")),
        ("reason", quote("controller_unavailable")),
    ])
}

fn error_json(error: FileCancellationError) -> String {
    let code = match error {
        FileCancellationError::MalformedRequest => "malformed_request",
        FileCancellationError::IdempotencyConflict => "idempotency_conflict",
        FileCancellationError::StalePredecessor => "stale_predecessor",
        FileCancellationError::Capacity => "capacity",
        FileCancellationError::Unsupported => "unsupported",
        FileCancellationError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn block_reason(reason: FileCancellationBlockReason) -> &'static str {
    match reason {
        FileCancellationBlockReason::AlreadyDispatched => "already_dispatched",
        FileCancellationBlockReason::Terminal => "terminal",
        FileCancellationBlockReason::IrrecoverablyUnknown => "irrecoverably_unknown",
        FileCancellationBlockReason::NumericalReplayUnsupported => "numerical_replay_unsupported",
        FileCancellationBlockReason::NumericalReplayLimit => "numerical_replay_limit",
        FileCancellationBlockReason::ReplayLimit => "replay_limit",
        FileCancellationBlockReason::OperationLimit => "operation_limit",
        FileCancellationBlockReason::OperationAlreadyUsed => "operation_already_used",
    }
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
