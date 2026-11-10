//! Sealed native reconciliation plans and original endpoint observations.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::Reconciliation;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, FileResolutionBlockReason, FileResolutionError,
    FileResolutionPlan, FileResolutionReceipt,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::action::{ActionState, Purpose};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedResolution, LinkedResolutionNext, request_address_for};
use crate::tower::request::resolution::encode_basis;

pub(super) fn encode_next(
    linked: &LinkedResolutionNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    if linked.operation() == 0 || linked.request() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || idempotency_key.is_some_and(|key| key != linked.operation().to_string())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge", next_known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-resolution-next/1")),
        ("operation", quote("next")),
        ("scope", quote("original_endpoint_reconciliation")),
        ("request", number(linked.request())),
        ("registration", quote(&linked.registration().canonical_string())),
        ("resolution_operation", number(linked.operation())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("idempotency_scope", quote("live_resolution_grant")),
        ("action_completeness", quote("single_connected_reconciliation")),
        (field, result),
    ]))
}

fn next_known(
    linked: &LinkedResolutionNext,
    plan: &FileResolutionPlan,
    basis: FileHoldDiagnosisBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    validate_basis(basis, linked.registration(), linked.query_address(), linked.request())?;
    if basis != plan.basis() || plan.operation() != linked.operation()
        || plan.can_reconcile() != plan.request().is_some()
        || plan.can_reconcile() != plan.block_reason().is_none()
        || (plan.can_reconcile() && !matches!(plan.stage(), ActionState::Dispatching | ActionState::Unknown))
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let action = linked.action().map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let action = match action {
        Some(request) => crate::tower::encode_request(&request)
            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
        None => "null".to_owned(),
    };
    let value = encoder.bounded(object_json(&[
        ("stage", quote(super::inspection::stage(plan.stage()))),
        ("can_reconcile", plan.can_reconcile().to_string()),
        ("block_reason", plan.block_reason().map_or_else(
            || "null".to_owned(), |reason| quote(block_reason(reason)),
        )),
        ("action", action),
        ("cost_estimate", unknown()),
        ("expected_information_gain", unknown()),
        ("ranking", unknown()),
        ("current_eligibility", unknown()),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")), ("basis", encode_basis(basis)), ("value", value),
    ]))
}

pub(super) fn encode_resolution(
    linked: &LinkedResolution,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    validate_basis(attempted.expected(), linked.registration(), address, linked.request())?;
    if attempted.operation() == 0 || operation_id != linked.query_operation_id()
        || address.generation != linked.registration().generation
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge", resolution_known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-resolution/1")),
        ("operation", quote("resolve")),
        ("scope", quote("original_endpoint_reconciliation")),
        ("request", number(linked.request())),
        ("registration", quote(&linked.registration().canonical_string())),
        ("resolution_operation", number(attempted.operation())),
        ("expected", encode_basis(attempted.expected())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("seal_attempted", "false".to_owned()),
        ("idempotency_scope", quote("live_resolution_grant")),
        (field, result),
    ]))
}

fn resolution_known(
    linked: &LinkedResolution,
    receipt: &FileResolutionReceipt,
    basis: FileHoldDiagnosisBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    validate_basis(after, linked.registration(), linked.query_address(), linked.request())?;
    if after != basis || before != linked.attempted().expected()
        || receipt.operation() != linked.attempted().operation()
        || after.scope != before.scope || after.request != before.request
        || after.attempt != before.attempt || after.recorded_at != before.recorded_at
        || before.journal_revision.checked_add(1) != Some(after.journal_revision)
        || after.request_generation < before.request_generation
        || after.control_sequence < before.control_sequence
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let correct_stage = match receipt.outcome() {
        Reconciliation::Resolved(EndpointOutcome::Executed { .. }) => ActionState::Confirmed,
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }) => ActionState::ConfirmedNotExecuted,
        Reconciliation::AwaitingResolution | Reconciliation::RetentionExpired => ActionState::Unknown,
    };
    if receipt.stage() != correct_stage {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        ("stage", quote(super::inspection::stage(receipt.stage()))),
        ("outcome", outcome_json(receipt.outcome())),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")), ("basis", encode_basis(after)), ("value", value),
    ]))
}

fn registration(
    registration: &Address,
    query: &Address,
    outer: &Address,
) -> Result<(), TowerEncodingError> {
    if query != outer || outer.kind != Noun::Attempt
        || outer.tenant != registration.tenant || registration.kind != Noun::Run
        || registration.generation.is_none_or(|generation| generation == 0)
        || outer.generation.is_some_and(|generation| Some(generation) != registration.generation)
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
    if basis.scope.purpose != Purpose::Effect || basis.request == 0 || basis.attempt == 0
        || basis.request != request || expected.without_generation() != query.without_generation()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn unavailable<T>(knowledge: &Knowledge<T, FileHoldDiagnosisBasis>) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable } => Ok(unknown()),
        Knowledge::Withheld { authority_required: "supervisor_resolution" } => Ok(object_json(&[
            ("state", quote("withheld")),
            ("authority_required", quote("supervisor_resolution")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn unknown() -> String {
    object_json(&[
        ("state", quote("unknown")), ("reason", quote("controller_unavailable")),
    ])
}

fn error_json(error: FileResolutionError) -> String {
    let code = match error {
        FileResolutionError::MalformedRequest => "malformed_request",
        FileResolutionError::IdempotencyConflict => "idempotency_conflict",
        FileResolutionError::StalePredecessor => "stale_predecessor",
        FileResolutionError::Capacity => "capacity",
        FileResolutionError::Unsupported => "unsupported",
        FileResolutionError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn block_reason(reason: FileResolutionBlockReason) -> &'static str {
    match reason {
        FileResolutionBlockReason::NotDispatched => "not_dispatched",
        FileResolutionBlockReason::Terminal => "terminal",
        FileResolutionBlockReason::IrrecoverablyUnknown => "irrecoverably_unknown",
        FileResolutionBlockReason::ClockUnavailable => "clock_unavailable",
        FileResolutionBlockReason::NumericalReplayUnsupported => "numerical_replay_unsupported",
        FileResolutionBlockReason::NumericalReplayLimit => "numerical_replay_limit",
        FileResolutionBlockReason::ReplayLimit => "replay_limit",
        FileResolutionBlockReason::OperationLimit => "operation_limit",
        FileResolutionBlockReason::OperationAlreadyUsed => "operation_already_used",
    }
}

fn outcome_json(outcome: Reconciliation) -> String {
    match outcome {
        Reconciliation::AwaitingResolution => object_json(&[("kind", quote("awaiting_resolution"))]),
        Reconciliation::RetentionExpired => object_json(&[("kind", quote("retention_expired"))]),
        Reconciliation::Resolved(outcome) => {
            let receipt = match outcome {
                EndpointOutcome::Executed { resulting_version } => object_json(&[
                    ("kind", quote("executed")), ("resulting_version", number(resulting_version)),
                ]),
                EndpointOutcome::NotExecuted { reason } => object_json(&[
                    ("kind", quote("not_executed")),
                    ("reason", quote(match reason {
                        NonExecutionReason::Sealed => "sealed",
                        NonExecutionReason::VersionConflict => "version_conflict",
                        NonExecutionReason::DeadlineElapsed => "deadline_elapsed",
                        NonExecutionReason::StreamRejected => "stream_rejected",
                    })),
                ]),
            };
            object_json(&[("kind", quote("resolved")), ("receipt", receipt)])
        }
    }
}

fn number(value: u64) -> String { quote(&value.to_string()) }
