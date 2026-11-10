//! Correlation and disclosure-shape checks, not an independent congress verifier.

use super::{Json, ReadArguments, TowerRequest, Verb};
use fa_reference::address_resolver::Noun;

pub(super) fn selected_round(request: &TowerRequest) -> Option<u64> {
    match (&request.verb, &request.arguments) {
        (Verb::Explain, ReadArguments::ReviewExplanation { round }) if *round != 0 => Some(*round),
        _ => None,
    }
}

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    let Some(round) = selected_round(request) else {
        return false;
    };
    if !header_matches(
        value,
        round,
        "fa.publication-review-explanation/1",
        "historical_completed_review",
    ) {
        return false;
    }
    match (value.get("knowledge"), value.get("error")) {
        (Some(knowledge), None) => match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                exact(knowledge, &["state", "basis", "value"])
                    && knowledge
                        .get("basis")
                        .is_some_and(|basis| bound_basis(basis, request, round))
                    && knowledge.get("value").is_some_and(|value| {
                        exact(
                            value,
                            &[
                                "action",
                                "policy",
                                "evaluation",
                                "congress",
                                "decision",
                                "application",
                                "effect",
                                "input_revision",
                                "evidence_root",
                                "timing",
                            ],
                        )
                    })
            }
            Some("unknown") => {
                exact(knowledge, &["state", "reason"])
                    && knowledge.get("reason").and_then(Json::as_str)
                        == Some("controller_unavailable")
            }
            Some("withheld") => {
                exact(knowledge, &["state", "authority_required"])
                    && knowledge.get("authority_required").and_then(Json::as_str)
                        == Some("supervisor_review_explanation")
            }
            _ => false,
        },
        (None, Some(error)) => {
            exact(error, &["code"])
                && matches!(
                    error.get("code").and_then(Json::as_str),
                    Some("capacity" | "unavailable")
                )
        }
        _ => false,
    }
}

pub(super) fn header_matches(value: &Json, round: u64, schema: &str, scope: &str) -> bool {
    let fields = [
        "schema",
        "operation",
        "scope",
        "round",
        "historical",
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "payload_disclosed",
        "witness_values_disclosed",
        "helper_inputs_disclosed",
        "commitment_salts_disclosed",
        "current_eligibility",
        "current_hold_reason",
    ];
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != fields.len() + 1
        || fields.iter().any(|name| !object.contains_key(*name))
        || value.get("schema").and_then(Json::as_str) != Some(schema)
        || value.get("operation").and_then(Json::as_str) != Some("explain")
        || value.get("scope").and_then(Json::as_str) != Some(scope)
        || value.get("round").and_then(decimal) != Some(round)
        || value.get("historical").and_then(Json::as_bool) != Some(true)
        || value.get("current_eligibility").and_then(Json::as_str) != Some("not_observed")
        || [
            "helpers_rerun",
            "live_evidence_read",
            "effect_authority",
            "payload_disclosed",
            "witness_values_disclosed",
            "helper_inputs_disclosed",
            "commitment_salts_disclosed",
        ]
        .iter()
        .any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
        || !value.get("current_hold_reason").is_some_and(|hold| {
            exact(hold, &["state", "reason"])
                && hold.get("state").and_then(Json::as_str) == Some("unknown")
                && hold.get("reason").and_then(Json::as_str) == Some("not_observed")
        })
    {
        return false;
    }
    true
}

pub(super) fn bound_basis(basis: &Json, request: &TowerRequest, round: u64) -> bool {
    if !exact(
        basis,
        &[
            "scope",
            "origin",
            "request",
            "attempt",
            "round",
            "begin_revision",
            "begin_control_sequence",
            "started_at",
            "completed_revision",
            "completed_control_sequence",
            "completed_at",
            "journal_revision",
            "control_sequence",
            "recorded_at",
        ],
    ) || basis.get("origin").and_then(Json::as_str) != Some("acknowledged_owner")
        || basis.get("round").and_then(decimal) != Some(round)
    {
        return false;
    }
    let Some(scope) = basis.get("scope") else {
        return false;
    };
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
    ) || scope.get("purpose").and_then(Json::as_str) != Some("effect")
        || ["tenant", "principal", "run", "branch", "authority"]
            .iter()
            .any(|key| {
                scope
                    .get(key)
                    .and_then(decimal)
                    .is_none_or(|value| value == 0)
            })
        || ["request", "attempt", "round"].iter().any(|key| {
            basis
                .get(key)
                .and_then(decimal)
                .is_none_or(|value| value == 0)
        })
        || [
            "begin_revision",
            "begin_control_sequence",
            "completed_revision",
            "completed_control_sequence",
            "journal_revision",
            "control_sequence",
            "started_at",
        ]
        .iter()
        .any(|key| basis.get(key).and_then(decimal).is_none())
        || ["completed_at", "recorded_at"].iter().any(|key| {
            !basis
                .get(key)
                .is_some_and(|value| value.is_null() || decimal(value).is_some())
        })
    {
        return false;
    }
    let expected = format!(
        "request-{}-{}-{}-{}-{}",
        scope.get("run").and_then(Json::as_str).unwrap(),
        scope.get("principal").and_then(Json::as_str).unwrap(),
        scope.get("branch").and_then(Json::as_str).unwrap(),
        scope.get("authority").and_then(Json::as_str).unwrap(),
        basis.get("request").and_then(Json::as_str).unwrap(),
    );
    request.address.kind == Noun::Attempt
        && Some(request.address.tenant.as_str()) == scope.get("tenant").and_then(Json::as_str)
        && request.address.id == expected
        && basis.get("begin_revision").and_then(decimal)
            < basis.get("completed_revision").and_then(decimal)
        && basis.get("completed_revision").and_then(decimal)
            <= basis.get("journal_revision").and_then(decimal)
        && basis.get("begin_control_sequence").and_then(decimal)
            <= basis.get("completed_control_sequence").and_then(decimal)
        && basis.get("completed_control_sequence").and_then(decimal)
            <= basis.get("control_sequence").and_then(decimal)
}

pub(super) fn decimal(value: &Json) -> Option<u64> {
    let spelling = value.as_str()?;
    let number = spelling.parse::<u64>().ok()?;
    (spelling == number.to_string()).then_some(number)
}

pub(super) fn exact(value: &Json, names: &[&str]) -> bool {
    value.as_object().is_some_and(|fields| {
        fields.len() == names.len() && names.iter().all(|name| fields.contains_key(*name))
    })
}
