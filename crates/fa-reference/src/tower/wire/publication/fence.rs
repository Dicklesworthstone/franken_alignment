//! Original admission-stop evidence with no endpoint or fleet claim.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, FileFenceError, FileFenceReceipt,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::action::Purpose;
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::LinkedFence;
use crate::tower::request::fence::encode_basis;

pub(super) fn encode(
    linked: &LinkedFence,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let attempted = linked.attempted();
    let registration = linked.registration();
    if linked.query_address() != address || address != registration
        || address.kind != Noun::Run || address.generation.is_none_or(|g| g == 0)
        || operation_id != linked.query_operation_id() || attempted.operation() == 0
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    validate_basis(attempted.expected(), registration)?;
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge", known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-fence/1")),
        ("operation", quote("fence")),
        ("scope", quote("original_domain_admission")),
        ("registration", quote(&registration.canonical_string())),
        ("fence_operation", number(attempted.operation())),
        ("expected", encode_basis(attempted.expected())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("drain_attempted", "false".to_owned()),
        ("idempotency_scope", quote("live_fence_grant")),
        (field, result),
    ]))
}

fn known(
    linked: &LinkedFence,
    receipt: &FileFenceReceipt,
    basis: FileFenceBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    let stop = receipt.stop();
    let progress = receipt.progress();
    let request = stop.request();
    validate_basis(after, linked.registration())?;
    if before != linked.attempted().expected() || after != basis
        || after.scope != before.scope || receipt.operation() != linked.attempted().operation()
        || request.operation != receipt.operation()
        || request.expected_control_sequence != before.control_sequence
        || request.expected_authority_epoch != before.authority_epoch
        || before.journal_revision.checked_add(1) != Some(after.journal_revision)
        || stop.control_sequence() <= before.control_sequence
        || stop.control_sequence() > after.control_sequence
        || before.authority_epoch.checked_add(1) != Some(after.authority_epoch)
        || stop.revocation_floor() != after.authority_epoch
        || progress.receipt != *stop
        || progress.dispatcher_epoch != stop.dispatcher_epoch()
        || progress.endpoint_fenced || progress.drained() || progress.reserved_units != 0
        || !ordered(stop.cancelled()) || !ordered(&progress.unresolved)
        || !ordered(&progress.irrecoverable)
        || progress.irrecoverable.iter().any(|id| progress.unresolved.binary_search(id).is_err())
        || stop.cancelled().iter().any(|id| progress.unresolved.binary_search(id).is_ok())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let stop_json = encoder.bounded(object_json(&[
        ("operation", number(request.operation)),
        ("expected_control_sequence", number(request.expected_control_sequence)),
        ("expected_authority_epoch", number(request.expected_authority_epoch)),
        ("control_sequence", number(stop.control_sequence())),
        ("revocation_floor", number(stop.revocation_floor())),
        ("dispatcher_epoch", number(stop.dispatcher_epoch())),
        ("cancelled", ids(stop.cancelled())),
        ("refunded_units", number(stop.refunded_units())),
    ]))?;
    let progress_json = encoder.bounded(object_json(&[
        ("dispatcher_epoch", number(progress.dispatcher_epoch)),
        ("endpoint_fenced", progress.endpoint_fenced.to_string()),
        ("unresolved", ids(&progress.unresolved)),
        ("irrecoverable", ids(&progress.irrecoverable)),
        ("reserved_units", number(progress.reserved_units)),
        ("charged_units", number(progress.charged_units)),
        ("drained", progress.drained().to_string()),
    ]))?;
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        ("stop", stop_json),
        ("progress", progress_json),
        ("actor_halted", unknown()),
        ("fleet_fenced", unknown()),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", encode_basis(after)),
        ("value", value),
    ]))
}

fn validate_basis(basis: FileFenceBasis, registration: &Address) -> Result<(), TowerEncodingError> {
    if basis.scope.purpose != Purpose::Effect
        || [basis.scope.tenant, basis.scope.principal, basis.scope.run,
            basis.scope.branch, basis.scope.authority].contains(&0)
        || registration.tenant != basis.scope.tenant.to_string()
        || registration.id != basis.scope.run.to_string()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn ordered(values: &[u64]) -> bool {
    values.first().is_none_or(|value| *value != 0)
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ids(values: &[u64]) -> String {
    format!("[{}]", values.iter().map(|value| number(*value)).collect::<Vec<_>>().join(","))
}

fn unavailable<T>(knowledge: &Knowledge<T, FileFenceBasis>) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable } => Ok(unknown()),
        Knowledge::Withheld { authority_required: "supervisor_fence" } => Ok(object_json(&[
            ("state", quote("withheld")), ("authority_required", quote("supervisor_fence")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn unknown() -> String {
    object_json(&[("state", quote("unknown")), ("reason", quote("controller_unavailable"))])
}

fn error_json(error: FileFenceError) -> String {
    let code = match error {
        FileFenceError::MalformedRequest => "malformed_request",
        FileFenceError::IdempotencyConflict => "idempotency_conflict",
        FileFenceError::StalePredecessor => "stale_predecessor",
        FileFenceError::Capacity => "capacity",
        FileFenceError::Unsupported => "unsupported",
        FileFenceError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
