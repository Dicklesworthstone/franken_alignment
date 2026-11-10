//! Correlate one native permanent admission-stop acknowledgment before output
//! or a later explicit document. Historical local Stop does not prove endpoint
//! fencing, completed drain, remote actor halt or rollback of dispatched work.

use super::{Json, ReadArguments, TowerRequest, Verb};
use super::hold_diagnosis::{decimal, decimal_text, exact, unknown};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileFenceBasis;
use fa_reference::action::{Purpose, Scope};
use fa_reference::address_resolver::Noun;

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    let ReadArguments::Fence { operation, expected } = &request.arguments else {
        return false;
    };
    let operation = *operation;
    if request.verb != Verb::Fence
        || operation == 0
        || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
        || request.address.kind != Noun::Run
        || request.address.generation.is_none_or(|generation| generation == 0)
        || decimal_text(&request.address.tenant) != Some(expected.scope.tenant)
        || decimal_text(&request.address.id) != Some(expected.scope.run)
        || expected.scope.purpose != Purpose::Effect
        || [
            expected.scope.tenant, expected.scope.principal, expected.scope.run,
            expected.scope.branch, expected.scope.authority,
        ].contains(&0)
    {
        return false;
    }
    let fields = [
        "schema", "operation", "scope", "registration", "fence_operation", "expected",
        "historical", "live_evidence_read", "clock_read", "effect_authority",
        "payload_disclosed", "may_write_journal", "publication_attempted",
        "drain_attempted", "idempotency_scope",
    ];
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != fields.len() + 1
        || fields.iter().any(|name| !object.contains_key(*name))
        || value.get("schema").and_then(Json::as_str) != Some("fa.publication-fence/1")
        || value.get("operation").and_then(Json::as_str) != Some("fence")
        || value.get("scope").and_then(Json::as_str) != Some("original_domain_admission")
        || value.get("registration").and_then(Json::as_str)
            != Some(request.address.canonical_string().as_str())
        || value.get("fence_operation").and_then(decimal) != Some(operation)
        || !value.get("expected").is_some_and(|basis| matches_basis(basis, expected))
        || value.get("historical").and_then(Json::as_bool) != Some(true)
        || value.get("may_write_journal").and_then(Json::as_bool) != Some(true)
        || value.get("idempotency_scope").and_then(Json::as_str) != Some("live_fence_grant")
        || [
            "live_evidence_read", "clock_read", "effect_authority", "payload_disclosed",
            "publication_attempted", "drain_attempted",
        ].iter().any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return false;
    }
    match (value.get("knowledge"), value.get("error")) {
        (Some(knowledge), None) => match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                exact(knowledge, &["state", "basis", "value"])
                    && knowledge.get("basis").is_some_and(|after| {
                        knowledge.get("value").is_some_and(|receipt| {
                            receipt_matches(receipt, after, operation, expected)
                        })
                    })
            }
            Some("unknown") => unknown(knowledge),
            Some("withheld") => exact(knowledge, &["state", "authority_required"])
                && knowledge.get("authority_required").and_then(Json::as_str)
                    == Some("supervisor_fence"),
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

fn bound_basis(value: &Json, expected: Scope) -> bool {
    if !exact(value, &["scope", "journal_revision", "control_sequence", "authority_epoch"])
        || ["journal_revision", "control_sequence", "authority_epoch"].iter()
            .any(|name| value.get(name).and_then(decimal).is_none())
    {
        return false;
    }
    let Some(scope) = value.get("scope") else {
        return false;
    };
    exact(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])
        && scope.get("purpose").and_then(Json::as_str) == Some("effect")
        && ["tenant", "principal", "run", "branch", "authority"].iter()
            .zip([expected.tenant, expected.principal, expected.run, expected.branch, expected.authority])
            .all(|(name, expected)| scope.get(name).and_then(decimal) == Some(expected))
}

fn matches_basis(value: &Json, expected: &FileFenceBasis) -> bool {
    bound_basis(value, expected.scope)
        && value.get("journal_revision").and_then(decimal) == Some(expected.journal_revision)
        && value.get("control_sequence").and_then(decimal) == Some(expected.control_sequence)
        && value.get("authority_epoch").and_then(decimal) == Some(expected.authority_epoch)
}

fn receipt_matches(
    value: &Json,
    after: &Json,
    operation: u64,
    expected: &FileFenceBasis,
) -> bool {
    if !exact(value, &["before", "stop", "progress", "actor_halted", "fleet_fenced"])
        || !value.get("before").is_some_and(|basis| matches_basis(basis, expected))
        || !bound_basis(after, expected.scope)
        || after.get("journal_revision").and_then(decimal)
            != expected.journal_revision.checked_add(1)
        || !value.get("actor_halted").is_some_and(unknown)
        || !value.get("fleet_fenced").is_some_and(unknown)
    {
        return false;
    }
    let Some(stop) = value.get("stop") else {
        return false;
    };
    let Some(progress) = value.get("progress") else {
        return false;
    };
    if !exact(stop, &[
        "operation", "expected_control_sequence", "expected_authority_epoch", "control_sequence",
        "revocation_floor", "dispatcher_epoch", "cancelled", "refunded_units",
    ]) || stop.get("operation").and_then(decimal) != Some(operation)
        || stop.get("expected_control_sequence").and_then(decimal)
            != Some(expected.control_sequence)
        || stop.get("expected_authority_epoch").and_then(decimal)
            != Some(expected.authority_epoch)
        || stop.get("refunded_units").and_then(decimal).is_none()
        || !exact(progress, &[
            "dispatcher_epoch", "endpoint_fenced", "unresolved", "irrecoverable",
            "reserved_units", "charged_units", "drained",
        ])
        || progress.get("endpoint_fenced").and_then(Json::as_bool) != Some(false)
        || progress.get("drained").and_then(Json::as_bool) != Some(false)
        || progress.get("reserved_units").and_then(decimal) != Some(0)
        || progress.get("charged_units").and_then(decimal).is_none()
    {
        return false;
    }
    let Some(sequence) = stop.get("control_sequence").and_then(decimal) else {
        return false;
    };
    let Some(epoch) = stop.get("revocation_floor").and_then(decimal) else {
        return false;
    };
    let Some(dispatcher) = stop.get("dispatcher_epoch").and_then(decimal) else {
        return false;
    };
    if sequence <= expected.control_sequence
        || after.get("control_sequence").and_then(decimal).is_none_or(|after| after < sequence)
        || expected.authority_epoch.checked_add(1) != Some(epoch)
        || after.get("authority_epoch").and_then(decimal) != Some(epoch)
        || dispatcher == 0
        || progress.get("dispatcher_epoch").and_then(decimal) != Some(dispatcher)
    {
        return false;
    }
    let Some(cancelled) = stop.get("cancelled").and_then(attempts) else {
        return false;
    };
    let Some(unresolved) = progress.get("unresolved").and_then(attempts) else {
        return false;
    };
    let Some(irrecoverable) = progress.get("irrecoverable").and_then(attempts) else {
        return false;
    };
    // The original sorted ledger projection cannot both refund and retain one
    // attempt as unresolved. Unknown charges remain explicitly outstanding.
    irrecoverable.iter().all(|attempt| unresolved.binary_search(attempt).is_ok())
        && cancelled.iter().all(|attempt| unresolved.binary_search(attempt).is_err())
}

fn attempts(value: &Json) -> Option<Vec<u64>> {
    let values = value.as_array()?;
    let mut attempts = Vec::with_capacity(values.len());
    let mut previous = 0;
    for value in values {
        let attempt = decimal(value)?;
        if attempt <= previous {
            return None;
        }
        attempts.push(attempt);
        previous = attempt;
    }
    Some(attempts)
}
