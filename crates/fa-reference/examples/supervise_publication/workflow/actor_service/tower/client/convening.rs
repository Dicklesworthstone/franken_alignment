//! Validate one explicitly requested held-review plan or admission disposition.
//! Pending is process-local admission, never RoundOpened or effect authority.
//! No response in this module selects, executes or retries a successor review.

use super::hold_diagnosis::{decimal, decimal_text, exact, request_scope};
use super::{Json, ReadArguments, TowerRequest, Verb};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::action::{ElapsedTick, Purpose, Scope};

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    validate(value, request).is_some() && envelope_matches(value, envelope)
}

pub(super) fn check_profile(
    parsed: &Json,
    request: &TowerRequest,
    scope: Scope,
) -> Result<(), String> {
    if !matches!(
        &request.arguments,
        ReadArguments::ConveneNext { .. } | ReadArguments::Convene { .. }
    ) {
        return Ok(());
    }
    let invalid = || {
        "held-review response changed the configured audience; no later request or retry was sent"
            .to_owned()
    };
    let coordinates = request_scope(request).ok_or_else(invalid)?;
    if coordinates[..5]
        != [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
        ]
        || scope.purpose != Purpose::Effect
    {
        return Err(invalid());
    }
    if let ReadArguments::Convene { expected, .. } = &request.arguments {
        if expected.scope != scope {
            return Err(invalid());
        }
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
        return Err(invalid());
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
        let Some(knowledge) = value.get("knowledge") else {
            return false;
        };
        match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => None,
            Some("unknown") => match knowledge.get("reason").and_then(Json::as_str) {
                Some("controller_unavailable") => Some("LowerLayerUnverified"),
                Some("outcome_unknown") => Some("MissingRequiredObservation"),
                _ => return false,
            },
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
    let (round, expected) = match (request.verb, &request.arguments) {
        (Verb::Next, ReadArguments::ConveneNext { round }) => (*round, None),
        (Verb::Convene, ReadArguments::Convene { round, expected }) => (*round, Some(*expected)),
        _ => return None,
    };
    let coordinates = request_scope(request)?;
    if round == 0
        || request.address.generation == Some(0)
        || request
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key != round.to_string())
        || (expected.is_some()
            && (request.address.generation.is_none()
                || request.idempotency_key.as_deref() != Some(round.to_string().as_str())))
    {
        return None;
    }
    let mut fields = vec![
        "schema",
        "operation",
        "scope",
        "registration",
        "request",
        "round",
        "live_evidence_read",
        "clock_read",
        "effect_authority",
        "payload_disclosed",
        "automatic_retry",
        "idempotency_scope",
    ];
    if expected.is_some() {
        fields.extend(["expected", "queued_work_is_durable"]);
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
                "fa.publication-convene/1"
            } else {
                "fa.publication-convene-next/1"
            }
        || value.get("operation")?.as_str()? != request.verb.as_str()
        || value.get("scope")?.as_str()? != "original_held_review"
        || decimal(value.get("request")?)? != coordinates[5]
        || decimal(value.get("round")?)? != round
        || value.get("idempotency_scope")?.as_str()? != "live_convening_grant"
        || [
            "live_evidence_read",
            "clock_read",
            "effect_authority",
            "payload_disclosed",
            "automatic_retry",
        ]
        .iter()
        .any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return None;
    }
    let prefix = format!("fa://{}/run/{}@", coordinates[0], coordinates[2]);
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
        if !bound(expected, coordinates)
            || expected.held_round == 0
            || basis(value.get("expected")?)? != expected
            || value.get("queued_work_is_durable")?.as_bool()?
        {
            return None;
        }
    } else if value.get("action_completeness")?.as_str()? != "single_connected_held_review" {
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
        "unknown" => (exact(knowledge, &["state", "reason"])
            && matches!(
                knowledge.get("reason")?.as_str()?,
                "controller_unavailable" | "outcome_unknown"
            ))
        .then_some(()),
        "withheld" => (exact(knowledge, &["state", "authority_required"])
            && knowledge.get("authority_required")?.as_str()? == "supervisor_convene")
            .then_some(()),
        "known" => {
            if !exact(knowledge, &["state", "basis", "value"]) {
                return None;
            }
            let basis_json = knowledge.get("basis")?;
            let basis = basis(basis_json)?;
            if !bound(basis, coordinates) {
                return None;
            }
            match expected {
                Some(expected) if basis == expected => receipt(knowledge.get("value")?, expected),
                Some(_) => None,
                None => plan(
                    knowledge.get("value")?,
                    basis_json,
                    basis,
                    round,
                    generation,
                    request,
                ),
            }
        }
        _ => None,
    }
}

fn bound(basis: FileConveneBasis, coordinates: [u64; 6]) -> bool {
    basis.scope.purpose == Purpose::Effect
        && [
            basis.scope.tenant,
            basis.scope.principal,
            basis.scope.run,
            basis.scope.branch,
            basis.scope.authority,
            basis.request,
        ] == coordinates
}

fn basis(value: &Json) -> Option<FileConveneBasis> {
    if !exact(
        value,
        &[
            "scope",
            "request",
            "request_generation",
            "attempt",
            "held_round",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "input_revision",
            "recorded_at",
            "deadline",
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
    let scope = Scope {
        tenant: decimal(scope.get("tenant")?)?,
        principal: decimal(scope.get("principal")?)?,
        run: decimal(scope.get("run")?)?,
        branch: decimal(scope.get("branch")?)?,
        authority: decimal(scope.get("authority")?)?,
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
    let recorded_at = match value.get("recorded_at")? {
        Json::Null => None,
        value => Some(ElapsedTick(decimal(value)?)),
    };
    let basis = FileConveneBasis {
        scope,
        request: decimal(value.get("request")?)?,
        request_generation: decimal(value.get("request_generation")?)?,
        attempt: decimal(value.get("attempt")?)?,
        held_round: decimal(value.get("held_round")?)?,
        journal_revision: decimal(value.get("journal_revision")?)?,
        control_sequence: decimal(value.get("control_sequence")?)?,
        authority_epoch: decimal(value.get("authority_epoch")?)?,
        input_revision: decimal(value.get("input_revision")?)?,
        recorded_at,
        deadline: ElapsedTick(decimal(value.get("deadline")?)?),
    };
    (basis.request != 0 && basis.attempt != 0).then_some(basis)
}

fn plan(
    value: &Json,
    basis_json: &Json,
    basis: FileConveneBasis,
    round: u64,
    generation: u64,
    request: &TowerRequest,
) -> Option<()> {
    if !exact(
        value,
        &[
            "can_convene",
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
    .any(|name| !value.get(name).is_some_and(not_observed))
    {
        return None;
    }
    if value.get("can_convene")?.as_bool()? {
        if basis.held_round == 0
            || basis.held_round == round
            || basis.journal_revision == 0
            || basis.control_sequence == 0
            || basis.recorded_at.is_none_or(|now| now >= basis.deadline)
            || !value.get("block_reason")?.is_null()
            || !executable(value.get("action")?, basis_json, round, generation, request)
        {
            return None;
        }
    } else if !value.get("action")?.is_null()
        || !matches!(
            value.get("block_reason")?.as_str()?,
            "not_held"
                | "stopped"
                | "suspended"
                | "unsupported_history"
                | "active_review"
                | "stale_review"
                | "round_already_used"
                | "round_limit"
                | "operation_limit"
                | "pending_operation"
                | "deadline_elapsed"
        )
    {
        return None;
    }
    Some(())
}

fn executable(
    action: &Json,
    basis: &Json,
    round: u64,
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
            == Some(format!("convene-{round}").as_str())
        && action.get("address").and_then(Json::as_str) == Some(address.canonical_string().as_str())
        && action.get("verb").and_then(Json::as_str) == Some("convene")
        && action.get("idempotency_key").and_then(decimal) == Some(round)
        && action.get("arguments").is_some_and(|arguments| {
            exact(arguments, &["round", "expected"])
                && arguments.get("round").and_then(decimal) == Some(round)
                && arguments.get("expected") == Some(basis)
        })
}

fn receipt(value: &Json, expected: FileConveneBasis) -> Option<()> {
    if !exact(value, &["outcome", "round_opened", "opened"]) {
        return None;
    }
    let opened = value.get("opened")?;
    match value.get("outcome")?.as_str()? {
        "pending" | "failed" | "abandoned" => {
            (!value.get("round_opened")?.as_bool()? && opened.is_null()).then_some(())
        }
        "opened" => {
            if !value.get("round_opened")?.as_bool()?
                || !exact(
                    opened,
                    &["journal_revision", "input_revision", "started_at"],
                )
                || decimal(opened.get("journal_revision")?)? <= expected.journal_revision
                || decimal(opened.get("input_revision")?)? < expected.input_revision
            {
                return None;
            }
            let started = ElapsedTick(decimal(opened.get("started_at")?)?);
            (started < expected.deadline
                && expected
                    .recorded_at
                    .is_none_or(|recorded| started >= recorded))
            .then_some(())
        }
        _ => None,
    }
}

fn not_observed(value: &Json) -> bool {
    exact(value, &["state", "reason"])
        && value.get("state").and_then(Json::as_str) == Some("unknown")
        && value.get("reason").and_then(Json::as_str) == Some("not_observed")
}
