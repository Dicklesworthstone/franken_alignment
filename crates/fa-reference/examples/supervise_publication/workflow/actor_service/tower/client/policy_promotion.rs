//! Correlate explicit policy promotion documents and original acknowledgments.
//! This checks bounded shape and internal consistency, not remote ledger truth.
//! No report or returned action creates a governor, approval or automatic retry.

use super::hold_diagnosis::{decimal, decimal_text, exact, unknown};
use super::{Json, ReadArguments, TowerRequest, Verb};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePolicyPromotionBasis;
use fa_reference::action::{ElapsedTick, MAX_ATTEMPTS, Purpose, Scope};
use fa_reference::address_resolver::Noun;

fn selected(request: &TowerRequest) -> bool {
    matches!(
        &request.arguments,
        ReadArguments::PolicyPromotionNext { .. } | ReadArguments::Promote { .. }
    )
}

/// Reject a foreign connection audience before any document in the sequence is
/// sent. A Run address alone carries neither principal nor authority identity.
pub(super) fn check_request(request: &TowerRequest, scope: Scope) -> Result<(), String> {
    if !selected(request) {
        return Ok(());
    }
    if scope.purpose != Purpose::Effect
        || request.address.kind != Noun::Run
        || request.address.tenant != scope.tenant.to_string()
        || request.address.id != scope.run.to_string()
        || matches!(&request.arguments, ReadArguments::Promote { expected, .. } if expected.scope != scope)
    {
        return Err(
            "policy promotion request differs from the configured audience; no request was sent"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn check_profile(
    parsed: &Json,
    request: &TowerRequest,
    scope: Scope,
) -> Result<(), String> {
    check_request(request, scope)?;
    if !selected(request) {
        return Ok(());
    }
    let knowledge = parsed
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(|value| value.get("knowledge"));
    if knowledge.is_some_and(|knowledge| {
        knowledge.get("state").and_then(Json::as_str) == Some("known")
            && knowledge
                .get("basis")
                .and_then(basis)
                .map(|basis| basis.scope)
                != Some(scope)
    }) {
        return Err("policy promotion response changed the configured audience; no later request or retry was sent".into());
    }
    Ok(())
}

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    validate(value, request).is_some()
        && envelope_matches(value, envelope)
        && envelope.get("retry_safe").and_then(Json::as_bool) == Some(request.verb == Verb::Next)
}

/// A generic native refusal is still a closed non-success value. Relabelling
/// a promotion receipt as refused must not bypass its structural validation.
pub(super) fn refused_matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    if !selected(request) {
        return true;
    }
    let Some(retry_safe) = envelope.get("retry_safe").and_then(Json::as_bool) else {
        return false;
    };
    if request.verb == Verb::Promote && retry_safe {
        return false;
    }
    if value.get("type").and_then(Json::as_str) != Some("PublicationRefusal")
        || envelope.get("status").and_then(Json::as_str) != Some("error")
        || envelope.get("reason_code").and_then(Json::as_str).is_none()
        || value
            .get("would_change_this")
            .and_then(Json::as_str)
            .is_none()
    {
        return false;
    }
    match value.get("state").and_then(Json::as_str) {
        Some("Unknown") => {
            exact(value, &["type", "state", "reason", "would_change_this"])
                && matches!(
                    value.get("reason").and_then(Json::as_str),
                    Some("Unavailable" | "Incomplete")
                )
        }
        Some("Withheld") => {
            exact(
                value,
                &["type", "state", "authority_required", "would_change_this"],
            ) && value.get("authority_required").and_then(Json::as_str) == Some("Verify")
                && envelope.get("reason_code").and_then(Json::as_str) == Some("DeniedByPolicy")
        }
        _ => false,
    }
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
        match value
            .get("knowledge")
            .and_then(|knowledge| knowledge.get("state"))
            .and_then(Json::as_str)
        {
            Some("known") => None,
            Some("unknown") => Some("LowerLayerUnverified"),
            Some("withheld") => Some("DeniedByPolicy"),
            _ => return false,
        }
    };
    match reason {
        Some(reason) => {
            envelope.get("status").and_then(Json::as_str) == Some("error")
                && envelope.get("reason_code").and_then(Json::as_str) == Some(reason)
        }
        None => {
            envelope.get("status").and_then(Json::as_str) == Some("ok")
                && envelope.get("reason_code").is_some_and(Json::is_null)
        }
    }
}

fn validate(value: &Json, request: &TowerRequest) -> Option<()> {
    let (campaign, operation, expected) = match (request.verb, &request.arguments) {
        (
            Verb::Next,
            ReadArguments::PolicyPromotionNext {
                campaign,
                operation,
            },
        ) => (*campaign, *operation, None),
        (
            Verb::Promote,
            ReadArguments::Promote {
                operation,
                expected,
            },
        ) => (expected.campaign, *operation, Some(*expected)),
        _ => return None,
    };
    if campaign == 0
        || operation == 0
        || request.address.kind != Noun::Run
        || request.address.generation == Some(0)
        || decimal_text(&request.address.tenant)? == 0
        || decimal_text(&request.address.id)? == 0
        || request
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key != operation.to_string())
        || (expected.is_some()
            && (request.address.generation.is_none()
                || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())))
    {
        return None;
    }
    let mut fields = vec![
        "schema",
        "operation",
        "scope",
        "registration",
        "campaign",
        "promotion_operation",
        "historical",
        "live_evidence_read",
        "clock_read",
        "effect_authority",
        "policy_disclosed",
        "payload_disclosed",
        "idempotency_scope",
    ];
    if expected.is_some() {
        fields.extend([
            "expected",
            "may_write_journal",
            "publication_attempted",
            "automatic_retry",
        ]);
    } else {
        fields.push("action_completeness");
    }
    fields.push(if value.get("error").is_some() {
        "error"
    } else {
        "knowledge"
    });
    if !exact(value, &fields)
        || value.get("schema")?.as_str()?
            != if expected.is_some() {
                "fa.publication-policy-promotion/1"
            } else {
                "fa.publication-policy-promotion-next/1"
            }
        || value.get("operation")?.as_str()? != request.verb.as_str()
        || value.get("scope")?.as_str()? != "original_approved_policy_campaign"
        || decimal(value.get("campaign")?)? != campaign
        || decimal(value.get("promotion_operation")?)? != operation
        || !value.get("historical")?.as_bool()?
        || value.get("idempotency_scope")?.as_str()? != "live_policy_promotion_grant"
        || [
            "live_evidence_read",
            "clock_read",
            "effect_authority",
            "policy_disclosed",
            "payload_disclosed",
        ]
        .iter()
        .any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return None;
    }
    let prefix = format!(
        "fa://{}/run/{}@",
        request.address.tenant, request.address.id
    );
    let generation = value
        .get("registration")?
        .as_str()?
        .strip_prefix(&prefix)
        .and_then(decimal_text)
        .filter(|generation| *generation != 0)?;
    if request
        .address
        .generation
        .is_some_and(|selected| selected != generation)
    {
        return None;
    }
    if let Some(expected) = expected {
        if expected.scope.purpose != Purpose::Effect
            || expected.scope.tenant.to_string() != request.address.tenant
            || expected.scope.run.to_string() != request.address.id
            || basis(value.get("expected")?)? != expected
            || !value.get("may_write_journal")?.as_bool()?
            || value.get("publication_attempted")?.as_bool()?
            || value.get("automatic_retry")?.as_bool()?
        {
            return None;
        }
    } else if value.get("action_completeness")?.as_str()?
        != "single_independently_approved_policy_campaign"
    {
        return None;
    }
    if let Some(error) = value.get("error") {
        return (exact(error, &["code"])
            && matches!(
                error.get("code")?.as_str()?,
                "malformed_request"
                    | "idempotency_conflict"
                    | "stale_predecessor"
                    | "capacity"
                    | "unsupported"
                    | "unavailable"
            ))
        .then_some(());
    }
    let knowledge = value.get("knowledge")?;
    match knowledge.get("state")?.as_str()? {
        "unknown" => unknown(knowledge).then_some(()),
        "withheld" => (exact(knowledge, &["state", "authority_required"])
            && knowledge.get("authority_required")?.as_str()? == "supervisor_policy_promotion")
            .then_some(()),
        "known" => {
            if !exact(knowledge, &["state", "basis", "value"]) {
                return None;
            }
            let basis_json = knowledge.get("basis")?;
            let cut = basis(basis_json)?;
            if cut.campaign != campaign
                || cut.scope.tenant.to_string() != request.address.tenant
                || cut.scope.run.to_string() != request.address.id
            {
                return None;
            }
            match expected {
                Some(expected) => receipt(knowledge.get("value")?, cut, expected),
                None => plan(
                    knowledge.get("value")?,
                    basis_json,
                    cut,
                    operation,
                    generation,
                    request,
                ),
            }
        }
        _ => None,
    }
}

fn basis(value: &Json) -> Option<FilePolicyPromotionBasis> {
    if !exact(
        value,
        &[
            "scope",
            "campaign",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "policy_generation",
            "recorded_at",
        ],
    ) {
        return None;
    }
    let scope = value.get("scope")?;
    if !exact(
        scope,
        &[
            "tenant",
            "principal",
            "run",
            "branch",
            "authority",
            "purpose",
        ],
    ) || scope.get("purpose")?.as_str()? != "effect"
    {
        return None;
    }
    let cut = FilePolicyPromotionBasis {
        scope: Scope {
            tenant: decimal(scope.get("tenant")?)?,
            principal: decimal(scope.get("principal")?)?,
            run: decimal(scope.get("run")?)?,
            branch: decimal(scope.get("branch")?)?,
            authority: decimal(scope.get("authority")?)?,
            purpose: Purpose::Effect,
        },
        campaign: decimal(value.get("campaign")?)?,
        journal_revision: decimal(value.get("journal_revision")?)?,
        control_sequence: decimal(value.get("control_sequence")?)?,
        authority_epoch: decimal(value.get("authority_epoch")?)?,
        policy_generation: decimal(value.get("policy_generation")?)?,
        recorded_at: match value.get("recorded_at")? {
            Json::Null => None,
            tick => Some(ElapsedTick(decimal(tick)?)),
        },
    };
    (![
        cut.scope.tenant,
        cut.scope.principal,
        cut.scope.run,
        cut.scope.branch,
        cut.scope.authority,
        cut.campaign,
        cut.policy_generation,
    ]
    .contains(&0))
    .then_some(cut)
}

fn plan(
    value: &Json,
    basis_json: &Json,
    cut: FilePolicyPromotionBasis,
    operation: u64,
    generation: u64,
    request: &TowerRequest,
) -> Option<()> {
    if !exact(
        value,
        &[
            "candidate_policy_generation",
            "can_promote",
            "block_reason",
            "action",
            "cost_estimate",
            "expected_information_gain",
            "ranking",
            "current_eligibility",
        ],
    ) || [
        "cost_estimate",
        "expected_information_gain",
        "ranking",
        "current_eligibility",
    ]
    .iter()
    .any(|name| !value.get(name).is_some_and(unknown))
    {
        return None;
    }
    let candidate = decimal(value.get("candidate_policy_generation")?)?;
    if candidate == 0 {
        return None;
    }
    if value.get("can_promote")?.as_bool()? {
        if candidate <= cut.policy_generation
            || !value.get("block_reason")?.is_null()
            || !executable(
                value.get("action")?,
                basis_json,
                operation,
                generation,
                request,
            )
        {
            return None;
        }
    } else if !value.get("action")?.is_null()
        || !matches!(
            value.get("block_reason")?.as_str()?,
            "stopped"
                | "campaign_not_approved"
                | "source_unavailable"
                | "stale_campaign"
                | "numerical_replay_unsupported"
                | "replay_limit"
                | "operation_limit"
                | "operation_already_used"
        )
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
    request: &TowerRequest,
) -> bool {
    let mut address = request.address.clone();
    address.generation = Some(generation);
    exact(
        action,
        &[
            "schema_version",
            "operation_id",
            "address",
            "verb",
            "arguments",
            "idempotency_key",
        ],
    ) && action.get("schema_version").and_then(Json::as_str) == Some("fa.tower_request/1")
        && action.get("operation_id").and_then(Json::as_str)
            == Some(format!("promote-policy-{operation}").as_str())
        && action.get("address").and_then(Json::as_str) == Some(address.canonical_string().as_str())
        && action.get("verb").and_then(Json::as_str) == Some("promote")
        && action.get("idempotency_key").and_then(decimal) == Some(operation)
        && action.get("arguments").is_some_and(|arguments| {
            exact(arguments, &["operation", "expected"])
                && arguments.get("operation").and_then(decimal) == Some(operation)
                && arguments.get("expected") == Some(basis)
        })
}

fn receipt(
    value: &Json,
    after: FilePolicyPromotionBasis,
    expected: FilePolicyPromotionBasis,
) -> Option<()> {
    if !exact(
        value,
        &[
            "before",
            "candidate_policy_generation",
            "cancelled",
            "refunded_units",
        ],
    ) || basis(value.get("before")?)? != expected
        || after.scope != expected.scope
        || after.campaign != expected.campaign
        || after.recorded_at != expected.recorded_at
        || Some(after.journal_revision) != expected.journal_revision.checked_add(1)
        || Some(after.control_sequence) != expected.control_sequence.checked_add(1)
        || Some(after.authority_epoch) != expected.authority_epoch.checked_add(1)
        || after.policy_generation <= expected.policy_generation
        || decimal(value.get("candidate_policy_generation")?)? != after.policy_generation
    {
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
