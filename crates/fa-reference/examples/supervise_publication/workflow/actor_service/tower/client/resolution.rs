//! Validate an original reconciliation plan/receipt before output or another
//! explicit request. This establishes wire correlation, never execution proof,
//! authority, current eligibility, or a reason to automatically retry.

use super::{Json, ReadArguments, TowerRequest, Verb};
use super::hold_diagnosis::{bound_basis, decimal, decimal_text, exact, request_scope, unknown};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileHoldDiagnosisBasis;

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    let (operation, expected) = match (request.verb, &request.arguments) {
        (Verb::Next, ReadArguments::ResolutionNext { operation }) => (*operation, None),
        (Verb::Resolve, ReadArguments::Resolve { operation, expected }) => {
            (*operation, Some(expected))
        }
        _ => return false,
    };
    if operation == 0
        || request.idempotency_key.as_deref().is_some_and(|key| key != operation.to_string())
        || (expected.is_some()
            && (request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request.address.generation.is_none()))
    {
        return false;
    }
    let Some(scope) = request_scope(request) else {
        return false;
    };
    if expected.is_some_and(|expected| {
        let native = expected.scope;
        [native.tenant, native.principal, native.run, native.branch, native.authority, expected.request]
            != scope
            || native.purpose != fa_reference::action::Purpose::Effect
            || expected.attempt == 0
    }) {
        return false;
    }
    let mut names = vec![
        "schema", "operation", "scope", "request", "registration", "resolution_operation",
        "historical", "live_evidence_read", "clock_read", "effect_authority",
        "payload_disclosed", "idempotency_scope",
    ];
    if expected.is_some() {
        names.extend(["expected", "publication_attempted", "seal_attempted", "may_write_journal"]);
    } else {
        names.push("action_completeness");
    }
    let Some(fields) = value.as_object() else {
        return false;
    };
    if fields.len() != names.len() + 1
        || names.iter().any(|name| !fields.contains_key(*name))
        || value.get("schema").and_then(Json::as_str) != Some(if expected.is_some() {
            "fa.publication-resolution/1"
        } else {
            "fa.publication-resolution-next/1"
        })
        || value.get("operation").and_then(Json::as_str) != Some(request.verb.as_str())
        || value.get("scope").and_then(Json::as_str) != Some("original_endpoint_reconciliation")
        || value.get("request").and_then(decimal) != Some(scope[5])
        || value.get("resolution_operation").and_then(decimal) != Some(operation)
        || value.get("historical").and_then(Json::as_bool) != Some(true)
        || value.get("idempotency_scope").and_then(Json::as_str) != Some("live_resolution_grant")
        || ["live_evidence_read", "clock_read", "effect_authority", "payload_disclosed"]
            .iter().any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return false;
    }
    let prefix = format!("fa://{}/run/{}@", scope[0], scope[2]);
    let Some(generation) = value.get("registration")
        .and_then(Json::as_str)
        .and_then(|address| address.strip_prefix(&prefix))
        .and_then(decimal_text)
        .filter(|generation| *generation != 0)
    else {
        return false;
    };
    if request.address.generation.is_some_and(|selected| selected != generation) {
        return false;
    }
    if let Some(expected) = expected {
        if !value.get("expected").is_some_and(|basis| matches_basis(basis, expected))
            || ["publication_attempted", "seal_attempted"].iter()
                .any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
            || value.get("may_write_journal").and_then(Json::as_bool) != Some(true)
        {
            return false;
        }
    } else if value.get("action_completeness").and_then(Json::as_str)
        != Some("single_connected_reconciliation")
    {
        return false;
    }
    match (value.get("knowledge"), value.get("error")) {
        (Some(knowledge), None) => match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                if !exact(knowledge, &["state", "basis", "value"]) {
                    return false;
                }
                let Some(basis) = knowledge.get("basis").filter(|basis| bound_basis(basis, &scope))
                else {
                    return false;
                };
                let Some(report) = knowledge.get("value") else {
                    return false;
                };
                match expected {
                    None => plan(report, basis, operation, generation, request),
                    Some(expected) => receipt(report, basis, expected),
                }
            }
            Some("unknown") => unknown(knowledge),
            Some("withheld") => exact(knowledge, &["state", "authority_required"])
                && knowledge.get("authority_required").and_then(Json::as_str)
                    == Some("supervisor_resolution"),
            _ => false,
        },
        (None, Some(error)) => exact(error, &["code"])
            && matches!(
                error.get("code").and_then(Json::as_str),
                Some("malformed_request" | "idempotency_conflict" | "stale_predecessor"
                    | "capacity" | "unsupported" | "unavailable")
            ),
        _ => false,
    }
}

fn matches_basis(value: &Json, expected: &FileHoldDiagnosisBasis) -> bool {
    let scope = expected.scope;
    bound_basis(value, &[
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, expected.request,
    ])
        && value.get("request_generation").and_then(decimal) == Some(expected.request_generation)
        && value.get("attempt").and_then(decimal) == Some(expected.attempt)
        && value.get("journal_revision").and_then(decimal) == Some(expected.journal_revision)
        && value.get("control_sequence").and_then(decimal) == Some(expected.control_sequence)
        && match expected.recorded_at {
            Some(tick) => value.get("recorded_at").and_then(decimal) == Some(tick.0),
            None => value.get("recorded_at").is_some_and(Json::is_null),
        }
}

fn plan(
    value: &Json,
    basis: &Json,
    operation: u64,
    generation: u64,
    request: &TowerRequest,
) -> bool {
    if !exact(value, &[
        "stage", "can_reconcile", "block_reason", "action", "cost_estimate",
        "expected_information_gain", "ranking", "current_eligibility",
    ]) || ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"]
        .iter().any(|name| !value.get(name).is_some_and(unknown))
    {
        return false;
    }
    let Some(stage) = value.get("stage").and_then(Json::as_str).filter(|stage| valid_stage(stage))
    else {
        return false;
    };
    match value.get("can_reconcile").and_then(Json::as_bool) {
        Some(true) => {
            matches!(stage, "dispatching" | "unknown")
                && basis.get("recorded_at").and_then(decimal).is_some()
                && value.get("block_reason").is_some_and(Json::is_null)
                && value.get("action").is_some_and(|action| {
                    executable(action, basis, operation, generation, request)
                })
        }
        Some(false) => {
            value.get("action").is_some_and(Json::is_null)
                && value.get("block_reason").and_then(Json::as_str).is_some_and(|reason| {
                    match reason {
                        "not_dispatched" => matches!(stage, "proposed" | "prepared" | "reviewing" | "authorized"),
                        "terminal" => matches!(stage, "denied" | "cancelled" | "confirmed" | "confirmed_not_executed"),
                        "irrecoverably_unknown" => stage == "irrecoverably_unknown",
                        "clock_unavailable" | "numerical_replay_unsupported" | "replay_limit"
                        | "operation_limit" | "operation_already_used" => true,
                        _ => false,
                    }
                })
        }
        None => false,
    }
}

fn executable(
    action: &Json,
    basis: &Json,
    operation: u64,
    generation: u64,
    request: &TowerRequest,
) -> bool {
    let mut address = request.address.clone();
    address.generation = Some(generation);
    if !exact(action, &[
        "schema_version", "operation_id", "address", "verb", "arguments", "idempotency_key",
    ]) || action.get("schema_version").and_then(Json::as_str) != Some("fa.tower_request/1")
        || action.get("operation_id").and_then(Json::as_str)
            != Some(format!("resolve-{operation}").as_str())
        || action.get("address").and_then(Json::as_str) != Some(address.canonical_string().as_str())
        || action.get("verb").and_then(Json::as_str) != Some("resolve")
        || action.get("idempotency_key").and_then(decimal) != Some(operation)
    {
        return false;
    }
    action.get("arguments").is_some_and(|arguments| {
        exact(arguments, &["operation", "expected"])
            && arguments.get("operation").and_then(decimal) == Some(operation)
            && arguments.get("expected") == Some(basis)
    })
}

fn receipt(value: &Json, after: &Json, expected: &FileHoldDiagnosisBasis) -> bool {
    if !exact(value, &["before", "stage", "outcome"])
        || !value.get("before").is_some_and(|before| matches_basis(before, expected))
        || after.get("attempt").and_then(decimal) != Some(expected.attempt)
        || after.get("journal_revision").and_then(decimal)
            != expected.journal_revision.checked_add(1)
        || after.get("request_generation").and_then(decimal)
            .is_none_or(|generation| generation < expected.request_generation)
        || after.get("control_sequence").and_then(decimal)
            .is_none_or(|sequence| sequence < expected.control_sequence)
        || after.get("recorded_at") != value.get("before").and_then(|before| before.get("recorded_at"))
    {
        return false;
    }
    let Some(outcome) = value.get("outcome") else {
        return false;
    };
    let stage = value.get("stage").and_then(Json::as_str);
    match outcome.get("kind").and_then(Json::as_str) {
        Some("awaiting_resolution" | "retention_expired") => {
            exact(outcome, &["kind"]) && stage == Some("unknown")
        }
        Some("resolved") => {
            if !exact(outcome, &["kind", "receipt"]) {
                return false;
            }
            let Some(receipt) = outcome.get("receipt") else {
                return false;
            };
            match receipt.get("kind").and_then(Json::as_str) {
                Some("executed") => exact(receipt, &["kind", "resulting_version"])
                    && receipt.get("resulting_version").and_then(decimal).is_some()
                    && stage == Some("confirmed"),
                Some("not_executed") => exact(receipt, &["kind", "reason"])
                    && receipt.get("reason").and_then(Json::as_str).is_some_and(valid_nonexecution)
                    && stage == Some("confirmed_not_executed"),
                _ => false,
            }
        }
        _ => false,
    }
}

fn valid_stage(stage: &str) -> bool {
    matches!(stage, "proposed" | "prepared" | "reviewing" | "authorized" | "denied" | "cancelled"
        | "dispatching" | "unknown" | "irrecoverably_unknown" | "confirmed" | "confirmed_not_executed")
}

fn valid_nonexecution(reason: &str) -> bool {
    // Kept closed in lockstep with the original native receipt serializer.
    matches!(reason, "sealed" | "version_conflict" | "deadline_elapsed" | "stream_rejected")
}
