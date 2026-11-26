//! Historical plans and acknowledged registration of an original Andon budget.
//! Encodings disclose immutable accounting bounds, never an effect permit.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, FileAndonWindowBlockReason, FileAndonWindowError, FileAndonWindowPlan,
    FileAndonWindowReceipt,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedAndonWindowNext, LinkedAndonWindowRegistration};
use crate::tower::request::andon_window::{
    encode_basis, encode_budget, validate_budget, validate_expected,
};

pub(super) fn encode_next(
    linked: &LinkedAndonWindowNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    if linked.operation() == 0
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
        ("schema", quote("fa.publication-andon-window-next/1")),
        ("operation", quote("next")),
        (
            "scope",
            quote("original_independently_registered_andon_window"),
        ),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("window_operation", number(linked.operation())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("credibility_disclosed", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("idempotency_scope", quote("live_andon_window_grant")),
        (
            "action_completeness",
            quote("single_independently_selected_andon_window"),
        ),
        (field, result),
    ]))
}

fn next_known(
    linked: &LinkedAndonWindowNext,
    plan: &FileAndonWindowPlan,
    basis: FileAndonWindowBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    validate_basis(basis, linked.registration())?;
    if validate_budget(plan.budget()).is_err()
        || plan.basis() != basis
        || plan.operation() != linked.operation()
        || plan.can_register() != plan.request().is_some()
        || plan.can_register() != plan.block_reason().is_none()
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
        ("budget", encode_budget(plan.budget())),
        ("already_registered", plan.already_registered().to_string()),
        ("can_register", plan.can_register().to_string()),
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

pub(super) fn encode_registration(
    linked: &LinkedAndonWindowRegistration,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    validate_basis(attempted.expected(), linked.registration())?;
    if validate_budget(attempted.expected_budget()).is_err()
        || address != linked.registration()
        || attempted.operation() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge",
            registered_known(linked, value, *basis, encoder)?,
        ),
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        (
            "schema",
            quote("fa.publication-andon-window-registration/1"),
        ),
        ("operation", quote("register-andon-window")),
        (
            "scope",
            quote("original_independently_registered_andon_window"),
        ),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("window_operation", number(attempted.operation())),
        ("expected", encode_basis(attempted.expected())),
        (
            "expected_budget",
            encode_budget(attempted.expected_budget()),
        ),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("credibility_disclosed", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("automatic_retry", "false".to_owned()),
        ("idempotency_scope", quote("live_andon_window_grant")),
        (field, result),
    ]))
}

fn registered_known(
    linked: &LinkedAndonWindowRegistration,
    receipt: &FileAndonWindowReceipt,
    basis: FileAndonWindowBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    validate_basis(after, linked.registration())?;
    let expected_revision = if receipt.registered() {
        before.journal_revision.checked_add(1)
    } else {
        Some(before.journal_revision)
    };
    if before != linked.attempted().expected()
        || after != basis
        || receipt.operation() != linked.attempted().operation()
        || receipt.budget() != linked.attempted().expected_budget()
        || validate_budget(receipt.budget()).is_err()
        || before.scope != after.scope
        || before.accounting != after.accounting
        || before.recorded_at != after.recorded_at
        || before.control_sequence != after.control_sequence
        || before.authority_epoch != after.authority_epoch
        || before.evaluation_revision != after.evaluation_revision
        || before.congress_generation != after.congress_generation
        || before.policy_generation != after.policy_generation
        || expected_revision != Some(after.journal_revision)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        ("budget", encode_budget(receipt.budget())),
        ("registered", receipt.registered().to_string()),
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
        || outer.kind != Noun::Run
        || registration.kind != Noun::Run
        || outer.without_generation() != registration.without_generation()
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
    basis: FileAndonWindowBasis,
    registration: &Address,
) -> Result<(), TowerEncodingError> {
    if validate_expected(basis).is_err()
        || registration.tenant != basis.scope.tenant.to_string()
        || registration.id != basis.scope.run.to_string()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn unavailable<T>(
    knowledge: &Knowledge<T, FileAndonWindowBasis>,
) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        } => Ok(unknown()),
        Knowledge::Withheld {
            authority_required: "supervisor_andon_window",
        } => Ok(object_json(&[
            ("state", quote("withheld")),
            ("authority_required", quote("supervisor_andon_window")),
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

fn error_json(error: FileAndonWindowError) -> String {
    let code = match error {
        FileAndonWindowError::MalformedRequest => "malformed_request",
        FileAndonWindowError::IdempotencyConflict => "idempotency_conflict",
        FileAndonWindowError::StalePredecessor => "stale_predecessor",
        FileAndonWindowError::Capacity => "capacity",
        FileAndonWindowError::Unsupported => "unsupported",
        FileAndonWindowError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn block_reason(reason: FileAndonWindowBlockReason) -> &'static str {
    match reason {
        FileAndonWindowBlockReason::WindowConflict => "window_conflict",
        FileAndonWindowBlockReason::WindowOverlap => "window_overlap",
        FileAndonWindowBlockReason::WindowLimit => "window_limit",
        FileAndonWindowBlockReason::NativeAdmissionUnavailable => "native_admission_unavailable",
        FileAndonWindowBlockReason::NumericalReplayUnsupported => "numerical_replay_unsupported",
        FileAndonWindowBlockReason::NumericalReplayLimit => "numerical_replay_limit",
        FileAndonWindowBlockReason::ReplayLimit => "replay_limit",
        FileAndonWindowBlockReason::OperationLimit => "operation_limit",
        FileAndonWindowBlockReason::OperationAlreadyUsed => "operation_already_used",
    }
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
