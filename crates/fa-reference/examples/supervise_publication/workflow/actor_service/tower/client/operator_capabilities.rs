//! Validate bounded discovery of the selected original operator grants.
//! Configuration and remaining work never become current effect eligibility.

use super::hold_diagnosis::{decimal, decimal_text, exact};
use super::{Json, ReadArguments, Scope, TowerRequest, Verb};
use fa_reference::action::Purpose;
use fa_reference::address_resolver::Noun;
use fa_reference::tower::publication::OperatorCapabilityKind;

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    validate(value, request).is_some()
        && envelope.get("status").and_then(Json::as_str) == Some("ok")
        && envelope.get("reason_code").is_some_and(Json::is_null)
        && envelope.get("retry_safe").and_then(Json::as_bool) == Some(true)
}

/// A refusal has its own closed non-success shape. A relabelled discovery
/// must not skip scope, authority and capability validation.
pub(super) fn refused_matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    if request.arguments != ReadArguments::OperatorCapabilities {
        return true;
    }
    if request.verb != Verb::Capabilities
        || value.get("type").and_then(Json::as_str) != Some("PublicationRefusal")
        || envelope.get("status").and_then(Json::as_str) != Some("error")
        || envelope.get("retry_safe").and_then(Json::as_bool).is_none()
        || value
            .get("would_change_this")
            .and_then(Json::as_str)
            .is_none()
        || !matches!(
            envelope.get("reason_code").and_then(Json::as_str),
            Some(
                "DeniedByPolicy"
                    | "MissingRequiredObservation"
                    | "StaleGeneration"
                    | "CapabilityUnsupported"
                    | "BudgetExhausted"
                    | "PermitConsumed"
                    | "DeadlineExpired"
                    | "OutcomeUnknown"
                    | "ReplayClosureUnavailable"
                    | "IntegrityFailure"
                    | "PurposeMismatch"
                    | "LowerLayerUnverified"
            )
        )
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
                && envelope.get("retry_safe").and_then(Json::as_bool) == Some(false)
        }
        _ => false,
    }
}

pub(super) fn check_profile(
    parsed: &Json,
    request: &TowerRequest,
    scope: Scope,
) -> Result<(), String> {
    if request.arguments != ReadArguments::OperatorCapabilities {
        return Ok(());
    }
    let invalid = || {
        "operator capabilities changed the configured audience; no later request or retry was sent"
            .to_owned()
    };
    if scope.purpose != Purpose::Effect
        || request.address.kind != Noun::Run
        || request.address.tenant != scope.tenant.to_string()
        || request.address.id != scope.run.to_string()
    {
        return Err(invalid());
    }
    // An ordinary refusal carries no native discovery basis. It cannot be
    // reinterpreted as a configured or available capability by this client.
    let Some(result) = parsed.get("result") else {
        return Err(invalid());
    };
    if result.get("kind").and_then(Json::as_str) == Some("refused") {
        return if refused_matches(
            result.get("value").ok_or_else(invalid)?,
            request,
            parsed.get("envelope").ok_or_else(invalid)?,
        ) {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let value = result.get("value").ok_or_else(invalid)?;
    let entries = validate(value, request).ok_or_else(invalid)?;
    for entry in entries {
        let grant = entry.get("grant").ok_or_else(invalid)?;
        if grant.get("state").and_then(Json::as_str) == Some("known")
            && grant
                .get("basis")
                .and_then(|basis| basis.get("scope"))
                .and_then(read_scope)
                != Some(scope)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn validate<'a>(value: &'a Json, request: &TowerRequest) -> Option<&'a [Json]> {
    if request.verb != Verb::Capabilities
        || request.arguments != ReadArguments::OperatorCapabilities
        || request.address.kind != Noun::Run
        || request.address.generation == Some(0)
        || decimal_text(&request.address.tenant)? == 0
        || decimal_text(&request.address.id)? == 0
        || !exact(
            value,
            &[
                "schema",
                "operation",
                "registration",
                "query_address",
                "disclosure",
                "current_eligibility",
                "unlisted_limits",
                "signed_profile",
                "effect_authority",
                "configuration",
            ],
        )
        || value.get("schema")?.as_str()? != "fa.operator-capabilities/1"
        || value.get("operation")?.as_str()? != "capabilities"
        || value.get("query_address")?.as_str()? != request.address.canonical_string()
        || value.get("disclosure")?.as_str()? != "mounted_operator_grants"
        || value.get("current_eligibility")?.as_str()? != "not_observed"
        || value.get("unlisted_limits")?.as_str()? != "not_observed"
        || value.get("signed_profile")?.as_bool()?
        || value.get("effect_authority")?.as_bool()?
    {
        return None;
    }
    let registration = value.get("registration")?.as_str()?;
    let prefix = format!(
        "fa://{}/run/{}@",
        request.address.tenant, request.address.id
    );
    let generation = decimal_text(registration.strip_prefix(&prefix)?)?;
    if generation == 0
        || request
            .address
            .generation
            .is_some_and(|expected| expected != generation)
    {
        return None;
    }
    let configuration = value.get("configuration")?;
    let basis = configuration.get("basis")?;
    if !exact(configuration, &["state", "basis", "value"])
        || configuration.get("state")?.as_str()? != "known"
        || !exact(basis, &["origin", "registration"])
        || basis.get("origin")?.as_str()? != "host_registration"
        || basis.get("registration")?.as_str()? != registration
    {
        return None;
    }
    let entries = configuration.get("value")?.as_array()?;
    if entries.len() != OperatorCapabilityKind::ALL.len() {
        return None;
    }
    for (entry, kind) in entries.iter().zip(OperatorCapabilityKind::ALL) {
        if !exact(
            entry,
            &[
                "capability",
                "verbs",
                "required_purpose",
                "route_connected",
                "purpose_allowed",
                "installed",
                "grant",
            ],
        ) || entry.get("capability")?.as_str()? != kind.as_str()
        {
            return None;
        }
        let verbs = entry.get("verbs")?.as_array()?;
        if verbs.len() != kind.verbs().len()
            || !verbs
                .iter()
                .zip(kind.verbs())
                .all(|(actual, expected)| actual.as_str() == Some(expected.as_str()))
        {
            return None;
        }
        let purpose = entry.get("required_purpose")?;
        match kind.required_purpose() {
            Some(expected) if purpose.as_str()? != expected.as_str() => return None,
            None if !purpose.is_null() => return None,
            _ => {}
        }
        entry.get("route_connected")?.as_bool()?;
        entry.get("purpose_allowed")?.as_bool()?;
        let installed = entry.get("installed")?.as_bool()?;
        let grant = entry.get("grant")?;
        match grant.get("state")?.as_str()? {
            "known" => {
                if !installed || !exact(grant, &["state", "basis", "value"]) {
                    return None;
                }
                let basis = grant.get("basis")?;
                if !exact(basis, &["scope", "journal_revision", "origin"])
                    || basis.get("origin")?.as_str()? != "original_live_grant"
                {
                    return None;
                }
                decimal(basis.get("journal_revision")?)?;
                let scope = read_scope(basis.get("scope")?)?;
                if scope.tenant.to_string() != request.address.tenant
                    || scope.run.to_string() != request.address.id
                {
                    return None;
                }
                limits(grant.get("value")?, kind)?;
            }
            "withheld"
                if exact(grant, &["state", "authority_required"])
                    && grant.get("authority_required")?.as_str()? == kind.authority_required() => {}
            "unknown"
                if installed
                    && exact(grant, &["state", "reason"])
                    && grant.get("reason")?.as_str()? == "controller_unavailable" => {}
            _ => return None,
        }
    }
    Some(entries)
}

fn read_scope(value: &Json) -> Option<Scope> {
    if !exact(
        value,
        &[
            "tenant",
            "principal",
            "run",
            "branch",
            "authority",
            "purpose",
        ],
    ) || value.get("purpose")?.as_str()? != "effect"
    {
        return None;
    }
    let scope = Scope {
        tenant: decimal(value.get("tenant")?)?,
        principal: decimal(value.get("principal")?)?,
        run: decimal(value.get("run")?)?,
        branch: decimal(value.get("branch")?)?,
        authority: decimal(value.get("authority")?)?,
        purpose: Purpose::Effect,
    };
    if [
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
    ]
    .contains(&0)
    {
        return None;
    }
    Some(scope)
}

fn limits(value: &Json, kind: OperatorCapabilityKind) -> Option<()> {
    use OperatorCapabilityKind::*;
    let (required, optional): (&[&str], Option<&str>) = match kind {
        Situation | HoldDiagnosis | GovernanceLedger => (&["remaining_queries"], None),
        JournalTail => (&["remaining_queries", "events_per_page"], None),
        RequestHistory => (
            &["remaining_queries", "history_event_limit", "review_limit"],
            None,
        ),
        AdmissionExplanation | ReviewExplanation => (
            &["remaining_queries", "replay_event_limit"],
            Some("remaining_numerical_replay_events"),
        ),
        Resolution | Cancellation | Abandonment | Narrowing | PolicyPromotion => (
            &[
                "remaining_queries",
                "remaining_operations",
                "replay_event_limit",
            ],
            Some("remaining_numerical_replay_events"),
        ),
        Convening => (&["remaining_queries", "remaining_operations"], None),
        Fence => (
            &["remaining_operations", "replay_event_limit"],
            Some("remaining_numerical_replay_events"),
        ),
        PolicyReplay => (
            &["remaining_queries", "case_limit", "input_byte_limit"],
            Some("remaining_page_queries"),
        ),
    };
    let fields = value.as_object()?;
    if required.iter().any(|key| !fields.contains_key(*key))
        || fields
            .keys()
            .any(|key| !required.contains(&key.as_str()) && optional != Some(key.as_str()))
    {
        return None;
    }
    for value in fields.values() {
        decimal(value)?;
    }
    Some(())
}
