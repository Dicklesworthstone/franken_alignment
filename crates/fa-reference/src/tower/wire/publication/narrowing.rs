//! Sealed original-run target restrictions and exact acknowledged intersections.
//! No field certifies current permission, measured cost or a learned ranking.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, FileNarrowBlockReason, FileNarrowError, FileNarrowPlan,
    FileNarrowReceipt, MAX_NARROW_TARGETS,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::action::{MAX_ATTEMPTS, ResolvedTarget};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedNarrowing, LinkedNarrowingNext};
use crate::tower::request::narrowing::{
    encode_basis, encode_targets, ordered_targets, target_key, validate_expected, validate_targets,
};

pub(super) fn encode_next(
    linked: &LinkedNarrowingNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    validate_targets(linked.retained_targets()).map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if linked.operation() == 0 || operation_id != linked.query_operation_id()
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
        ("schema", quote("fa.publication-narrowing-next/1")),
        ("operation", quote("next")),
        ("scope", quote("original_run_target_ceiling")),
        ("registration", quote(&linked.registration().canonical_string())),
        ("narrowing_operation", number(linked.operation())),
        ("retained_targets", encode_targets(linked.retained_targets())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("idempotency_scope", quote("live_narrowing_grant")),
        ("action_completeness", quote("single_connected_target_narrowing")),
        (field, result),
    ]))
}

fn next_known(
    linked: &LinkedNarrowingNext,
    plan: &FileNarrowPlan,
    basis: FileNarrowBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    validate_basis(basis, linked.registration())?;
    if plan.basis() != basis || plan.operation() != linked.operation()
        || plan.retained_targets() != linked.retained_targets()
        || plan.can_narrow() != plan.request().is_some()
        || plan.can_narrow() != plan.block_reason().is_none()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    validate_intersection(plan.current_ceiling(), plan.retained_targets(), plan.effective_targets())?;
    let action = linked.action().map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let action = match action {
        Some(request) => crate::tower::encode_request(&request)
            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
        None => "null".to_owned(),
    };
    let value = encoder.bounded(object_json(&[
        ("current_ceiling", optional_targets(plan.current_ceiling())),
        ("effective_targets", encode_targets(plan.effective_targets())),
        ("can_narrow", plan.can_narrow().to_string()),
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

pub(super) fn encode_narrowing(
    linked: &LinkedNarrowing,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    validate_basis(attempted.expected(), linked.registration())?;
    validate_targets(attempted.retained_targets()).map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if address != linked.registration() || attempted.operation() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge", narrowed_known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-narrowing/1")),
        ("operation", quote("narrow")),
        ("scope", quote("original_run_target_ceiling")),
        ("registration", quote(&linked.registration().canonical_string())),
        ("narrowing_operation", number(attempted.operation())),
        ("retained_targets", encode_targets(attempted.retained_targets())),
        ("expected", encode_basis(attempted.expected())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("reset_attempted", "false".to_owned()),
        ("idempotency_scope", quote("live_narrowing_grant")),
        (field, result),
    ]))
}

fn narrowed_known(
    linked: &LinkedNarrowing,
    receipt: &FileNarrowReceipt,
    basis: FileNarrowBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    validate_basis(after, linked.registration())?;
    if before != linked.attempted().expected() || after != basis
        || receipt.operation() != linked.attempted().operation()
        || receipt.retained_targets() != linked.attempted().retained_targets()
        || before.scope != after.scope || before.authority_epoch != after.authority_epoch
        || before.recorded_at != after.recorded_at
        || before.journal_revision.checked_add(1) != Some(after.journal_revision)
        || before.control_sequence.checked_add(1) != Some(after.control_sequence)
        || receipt.cancelled().len() > MAX_ATTEMPTS || !ordered_ids(receipt.cancelled())
        || (receipt.cancelled().is_empty() && receipt.refunded_units() != 0)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    validate_intersection(receipt.previous_ceiling(), receipt.retained_targets(), receipt.effective_targets())?;
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        ("previous_ceiling", optional_targets(receipt.previous_ceiling())),
        ("effective_targets", encode_targets(receipt.effective_targets())),
        ("cancelled", ids(receipt.cancelled())),
        ("refunded_units", number(receipt.refunded_units())),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")), ("basis", encode_basis(after)), ("value", value),
    ]))
}

fn registration(registration: &Address, query: &Address, outer: &Address) -> Result<(), TowerEncodingError> {
    if query != outer || outer.kind != Noun::Run || registration.kind != Noun::Run
        || outer.without_generation() != registration.without_generation()
        || registration.generation.is_none_or(|generation| generation == 0)
        || outer.generation.is_some_and(|generation| Some(generation) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn validate_basis(basis: FileNarrowBasis, registration: &Address) -> Result<(), TowerEncodingError> {
    if validate_expected(basis).is_err()
        || registration.tenant != basis.scope.tenant.to_string()
        || registration.id != basis.scope.run.to_string()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn validate_intersection(
    previous: Option<&[ResolvedTarget]>,
    requested: &[ResolvedTarget],
    effective: &[ResolvedTarget],
) -> Result<(), TowerEncodingError> {
    if validate_targets(requested).is_err() || !ordered_targets(effective)
        || previous.is_some_and(|targets| targets.len() > MAX_NARROW_TARGETS || !ordered_targets(targets))
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let intersection: Vec<_> = requested.iter().copied().filter(|target| {
        previous.is_none_or(|targets| targets.binary_search_by_key(&target_key(target), target_key).is_ok())
    }).collect();
    if intersection != effective {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn optional_targets(targets: Option<&[ResolvedTarget]>) -> String {
    targets.map_or_else(|| "null".to_owned(), encode_targets)
}

fn ordered_ids(values: &[u64]) -> bool {
    values.first().is_none_or(|value| *value != 0)
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ids(values: &[u64]) -> String {
    format!("[{}]", values.iter().map(|value| number(*value)).collect::<Vec<_>>().join(","))
}

fn unavailable<T>(knowledge: &Knowledge<T, FileNarrowBasis>) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable } => Ok(unknown()),
        Knowledge::Withheld { authority_required: "supervisor_narrow" } => Ok(object_json(&[
            ("state", quote("withheld")), ("authority_required", quote("supervisor_narrow")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn unknown() -> String {
    object_json(&[("state", quote("unknown")), ("reason", quote("controller_unavailable"))])
}

fn error_json(error: FileNarrowError) -> String {
    let code = match error {
        FileNarrowError::MalformedRequest => "malformed_request",
        FileNarrowError::IdempotencyConflict => "idempotency_conflict",
        FileNarrowError::StalePredecessor => "stale_predecessor",
        FileNarrowError::Capacity => "capacity",
        FileNarrowError::Unsupported => "unsupported",
        FileNarrowError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn block_reason(reason: FileNarrowBlockReason) -> &'static str {
    match reason {
        FileNarrowBlockReason::Stopped => "stopped",
        FileNarrowBlockReason::Suspended => "suspended",
        FileNarrowBlockReason::UnsupportedHistory => "unsupported_history",
        FileNarrowBlockReason::ReplayLimit => "replay_limit",
        FileNarrowBlockReason::OperationLimit => "operation_limit",
        FileNarrowBlockReason::OperationAlreadyUsed => "operation_already_used",
    }
}

fn number(value: u64) -> String { quote(&value.to_string()) }
