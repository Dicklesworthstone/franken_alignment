//! Correlate an explicit original target-ceiling reduction before output or a
//! later document. A plan or receipt grants no effect authority, restored target,
//! policy change, actor reset, current eligibility, or automatic follow-up.

use super::hold_diagnosis::{decimal, decimal_text, exact, unknown};
use super::{Json, ReadArguments, TowerRequest, Verb};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_TARGETS,
};
use fa_reference::action::{Purpose, ResolvedTarget, Scope, MAX_ATTEMPTS};
use fa_reference::address_resolver::Noun;
use fa_reference::tower::MAX_NARROWING_TARGETS;

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    validate(value, request).is_some() && envelope_matches(value, envelope)
}

/// A Run address carries only tenant/run. The configured connection supplies
/// the remaining original audience coordinates before a plan may be emitted.
pub(super) fn check_profile(parsed: &Json, request: &TowerRequest, scope: Scope) -> Result<(), String> {
    if !matches!(&request.arguments, ReadArguments::NarrowingNext { .. } | ReadArguments::Narrow { .. })
        || parsed.get("result").and_then(|result| result.get("kind")).and_then(Json::as_str) == Some("refused")
    {
        return Ok(());
    }
    let knowledge = parsed.get("result").and_then(|result| result.get("value"))
        .and_then(|value| value.get("knowledge"));
    if knowledge.is_some_and(|knowledge| {
        knowledge.get("state").and_then(Json::as_str) == Some("known")
            && knowledge.get("basis").and_then(basis_scope) != Some(scope)
    }) {
        return Err("native narrowing response changed the configured audience; no later request or retry was sent".into());
    }
    Ok(())
}

fn envelope_matches(value: &Json, envelope: &Json) -> bool {
    let reason = if let Some(error) = value.get("error") {
        match error.get("code").and_then(Json::as_str) {
            Some("malformed_request" | "idempotency_conflict") => Some("IntegrityFailure"),
            Some("stale_predecessor") => Some("StaleGeneration"),
            Some("capacity") => Some("BudgetExhausted"),
            Some("unsupported") => Some("CapabilityUnsupported"),
            Some("unavailable") => Some("LowerLayerUnverified"),
            _ => return false,
        }
    } else {
        match value.get("knowledge").and_then(|knowledge| knowledge.get("state")).and_then(Json::as_str) {
            Some("known") => None,
            Some("unknown") => Some("LowerLayerUnverified"),
            Some("withheld") => Some("DeniedByPolicy"),
            _ => return false,
        }
    };
    match reason {
        Some(reason) => envelope.get("status").and_then(Json::as_str) == Some("error")
            && envelope.get("reason_code").and_then(Json::as_str) == Some(reason),
        None => envelope.get("status").and_then(Json::as_str) == Some("ok")
            && envelope.get("reason_code").is_some_and(Json::is_null),
    }
}

fn validate(value: &Json, request: &TowerRequest) -> Option<()> {
    let (operation, requested, expected) = match (request.verb, &request.arguments) {
        (Verb::Next, ReadArguments::NarrowingNext { operation, retained_targets }) => {
            (*operation, retained_targets, None)
        }
        (Verb::Narrow, ReadArguments::Narrow { operation, expected, retained_targets }) => {
            (*operation, retained_targets, Some(expected))
        }
        _ => return None,
    };
    if operation == 0
        || request.address.kind != Noun::Run
        || request.address.generation == Some(0)
        || decimal_text(&request.address.tenant)? == 0
        || decimal_text(&request.address.id)? == 0
        || request.idempotency_key.as_deref()
            .is_some_and(|key| key != operation.to_string())
        || (expected.is_some()
            && (request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request.address.generation.is_none()))
        || requested.len() > MAX_NARROWING_TARGETS
    {
        return None;
    }
    let requested: Vec<_> = requested.iter().copied().map(target_key).collect();
    if !canonical(&requested) {
        return None;
    }
    let mut fields = vec![
        "schema", "operation", "scope", "registration", "narrowing_operation",
        "retained_targets", "historical", "live_evidence_read", "clock_read",
        "effect_authority", "payload_disclosed", "idempotency_scope",
    ];
    if expected.is_some() {
        fields.extend(["expected", "may_write_journal", "publication_attempted", "reset_attempted"]);
    } else {
        fields.push("action_completeness");
    }
    fields.push(if value.get("error").is_some() { "error" } else { "knowledge" });
    let schema = if expected.is_some() {
        "fa.publication-narrowing/1"
    } else {
        "fa.publication-narrowing-next/1"
    };
    if !exact(value, &fields)
        || value.get("schema")?.as_str()? != schema
        || value.get("operation")?.as_str()? != request.verb.as_str()
        || value.get("scope")?.as_str()? != "original_run_target_ceiling"
        || decimal(value.get("narrowing_operation")?)? != operation
        || targets(value.get("retained_targets")?, MAX_NARROWING_TARGETS)? != requested
        || !value.get("historical")?.as_bool()?
        || value.get("idempotency_scope")?.as_str()? != "live_narrowing_grant"
        || ["live_evidence_read", "clock_read", "effect_authority", "payload_disclosed"]
            .iter().any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return None;
    }
    let prefix = format!("fa://{}/run/{}@", request.address.tenant, request.address.id);
    let generation = value.get("registration")?.as_str()?.strip_prefix(&prefix)
        .and_then(decimal_text).filter(|generation| *generation != 0)?;
    if request.address.generation.is_some_and(|selected| selected != generation) {
        return None;
    }
    if let Some(expected) = expected {
        if expected.scope.purpose != Purpose::Effect
            || expected.scope.tenant.to_string() != request.address.tenant
            || expected.scope.run.to_string() != request.address.id
            || !matches_basis(value.get("expected")?, expected)
            || !value.get("may_write_journal")?.as_bool()?
            || value.get("publication_attempted")?.as_bool()?
            || value.get("reset_attempted")?.as_bool()?
        {
            return None;
        }
    } else if value.get("action_completeness")?.as_str()? != "single_connected_target_narrowing" {
        return None;
    }
    if let Some(error) = value.get("error") {
        return (exact(error, &["code"])
            && matches!(error.get("code")?.as_str()?,
                "malformed_request" | "idempotency_conflict" | "stale_predecessor"
                | "capacity" | "unsupported" | "unavailable"))
            .then_some(());
    }
    let knowledge = value.get("knowledge")?;
    match knowledge.get("state")?.as_str()? {
        "unknown" => unknown(knowledge).then_some(()),
        "withheld" => (exact(knowledge, &["state", "authority_required"])
            && knowledge.get("authority_required")?.as_str()? == "supervisor_narrow")
            .then_some(()),
        "known" => {
            if !exact(knowledge, &["state", "basis", "value"]) {
                return None;
            }
            let basis = knowledge.get("basis")?;
            let scope = basis_scope(basis)?;
            if scope.tenant.to_string() != request.address.tenant
                || scope.run.to_string() != request.address.id
            {
                return None;
            }
            match expected {
                Some(expected) => receipt(knowledge.get("value")?, basis, expected, &requested),
                None => plan(knowledge.get("value")?, basis, operation, generation, &requested, request),
            }
        }
        _ => None,
    }
}

fn target_key(target: ResolvedTarget) -> [u64; 5] {
    [target.adapter, target.object, target.contract_version, target.expected_version, target.generation]
}

fn canonical(targets: &[[u64; 5]]) -> bool {
    targets.iter().all(|target| !target.contains(&0))
        && targets.windows(2).all(|pair| pair[0] < pair[1])
}

fn targets(value: &Json, limit: usize) -> Option<Vec<[u64; 5]>> {
    let values = value.as_array()?;
    if values.len() > limit {
        return None;
    }
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        if !exact(value, &["adapter", "object", "contract_version", "expected_version", "generation"]) {
            return None;
        }
        result.push([
            decimal(value.get("adapter")?)?, decimal(value.get("object")?)?,
            decimal(value.get("contract_version")?)?, decimal(value.get("expected_version")?)?,
            decimal(value.get("generation")?)?,
        ]);
    }
    canonical(&result).then_some(result)
}

fn basis_scope(value: &Json) -> Option<Scope> {
    if !exact(value, &["scope", "journal_revision", "control_sequence", "authority_epoch", "recorded_at"]) {
        return None;
    }
    for field in ["journal_revision", "control_sequence", "authority_epoch"] {
        decimal(value.get(field)?)?;
    }
    let recorded = value.get("recorded_at")?;
    if !recorded.is_null() {
        decimal(recorded)?;
    }
    let scope = value.get("scope")?;
    if !exact(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])
        || scope.get("purpose")?.as_str()? != "effect"
    {
        return None;
    }
    let scope = Scope {
        tenant: decimal(scope.get("tenant")?)?, principal: decimal(scope.get("principal")?)?,
        run: decimal(scope.get("run")?)?, branch: decimal(scope.get("branch")?)?,
        authority: decimal(scope.get("authority")?)?, purpose: Purpose::Effect,
    };
    (![scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0))
        .then_some(scope)
}

fn matches_basis(value: &Json, expected: &FileNarrowBasis) -> bool {
    basis_scope(value) == Some(expected.scope)
        && value.get("journal_revision").and_then(decimal) == Some(expected.journal_revision)
        && value.get("control_sequence").and_then(decimal) == Some(expected.control_sequence)
        && value.get("authority_epoch").and_then(decimal) == Some(expected.authority_epoch)
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
    requested: &[[u64; 5]],
    request: &TowerRequest,
) -> Option<()> {
    if !exact(value, &[
        "current_ceiling", "effective_targets", "can_narrow", "block_reason", "action",
        "cost_estimate", "expected_information_gain", "ranking", "current_eligibility",
    ]) || ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"]
        .iter().any(|name| !value.get(name).is_some_and(unknown))
    {
        return None;
    }
    let effective = targets(value.get("effective_targets")?, MAX_NARROWING_TARGETS)?;
    if effective != intersection(value.get("current_ceiling")?, requested)? {
        return None;
    }
    if value.get("can_narrow")?.as_bool()? {
        if !value.get("block_reason")?.is_null()
            || !executable(value.get("action")?, basis, operation, generation, requested, request)
        {
            return None;
        }
    } else if !value.get("action")?.is_null()
        || !matches!(value.get("block_reason")?.as_str()?,
            "stopped" | "suspended" | "unsupported_history" | "replay_limit"
            | "operation_limit" | "operation_already_used")
    {
        return None;
    }
    Some(())
}

fn executable(
    action: &Json,
    basis: &Json,
    operation: u64,
    generation: u64,
    requested: &[[u64; 5]],
    request: &TowerRequest,
) -> bool {
    let mut address = request.address.clone();
    address.generation = Some(generation);
    exact(action, &["schema_version", "operation_id", "address", "verb", "arguments", "idempotency_key"])
        && action.get("schema_version").and_then(Json::as_str) == Some("fa.tower_request/1")
        && action.get("operation_id").and_then(Json::as_str)
            == Some(format!("narrow-authority-{operation}").as_str())
        && action.get("address").and_then(Json::as_str) == Some(address.canonical_string().as_str())
        && action.get("verb").and_then(Json::as_str) == Some("narrow")
        && action.get("idempotency_key").and_then(decimal) == Some(operation)
        && action.get("arguments").is_some_and(|arguments| {
            exact(arguments, &["operation", "expected", "retained_targets"])
                && arguments.get("operation").and_then(decimal) == Some(operation)
                && arguments.get("expected") == Some(basis)
                && arguments.get("retained_targets")
                    .and_then(|value| targets(value, MAX_NARROWING_TARGETS))
                    .is_some_and(|targets| targets == requested)
        })
}

fn receipt(value: &Json, after: &Json, expected: &FileNarrowBasis, requested: &[[u64; 5]]) -> Option<()> {
    if !exact(value, &["before", "previous_ceiling", "effective_targets", "cancelled", "refunded_units"])
        || !matches_basis(value.get("before")?, expected)
        || basis_scope(after)? != expected.scope
        || Some(decimal(after.get("journal_revision")?)?) != expected.journal_revision.checked_add(1)
        || Some(decimal(after.get("control_sequence")?)?) != expected.control_sequence.checked_add(1)
        || decimal(after.get("authority_epoch")?)? != expected.authority_epoch
        || after.get("recorded_at") != value.get("before")?.get("recorded_at")
    {
        return None;
    }
    let effective = targets(value.get("effective_targets")?, MAX_NARROWING_TARGETS)?;
    if effective != intersection(value.get("previous_ceiling")?, requested)? {
        return None;
    }
    let cancelled = value.get("cancelled")?.as_array()?;
    let refund = decimal(value.get("refunded_units")?)?;
    if cancelled.len() > MAX_ATTEMPTS || (cancelled.is_empty() && refund != 0) {
        return None;
    }
    let mut previous = 0;
    for attempt in cancelled {
        let attempt = decimal(attempt)?;
        if attempt <= previous {
            return None;
        }
        previous = attempt;
    }
    Some(())
}

fn intersection(ceiling: &Json, requested: &[[u64; 5]]) -> Option<Vec<[u64; 5]>> {
    if ceiling.is_null() {
        return Some(requested.to_vec());
    }
    let ceiling = targets(ceiling, MAX_NARROW_TARGETS)?;
    Some(requested.iter().copied().filter(|target| ceiling.binary_search(target).is_ok()).collect())
}
