//! Validate original request/review relationships before disclosure or a later
//! explicit send. These are correlation checks, not an independent journal
//! replay or authority to execute the supplied historical Explain requests.

use super::hold_diagnosis::{decimal, decimal_text, exact, request_scope, unknown};
use super::{Json, ReadArguments, Scope, TowerRequest, Verb, encode_request, strict_json};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_REVIEWS,
};
use std::collections::BTreeSet;

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    if request.verb != Verb::Related || request.arguments != ReadArguments::None {
        return false;
    }
    let Some(expected) = request_scope(request) else {
        return false;
    };
    let fields = [
        "schema",
        "operation",
        "registration",
        "query_address",
        "historical",
        "current_eligibility",
        "disclosure",
    ];
    if value.as_object().is_none_or(|object| {
        object.len() != fields.len() + 1 || fields.iter().any(|name| !object.contains_key(*name))
    }) || value.get("schema").and_then(Json::as_str) != Some("fa.publication-request-history/1")
        || value.get("operation").and_then(Json::as_str) != Some("related")
        || value.get("query_address").and_then(Json::as_str)
            != Some(request.address.canonical_string().as_str())
        || value.get("historical").and_then(Json::as_bool) != Some(true)
        || value.get("current_eligibility").and_then(Json::as_str) != Some("not_observed")
        || value.get("disclosure").and_then(Json::as_str) != Some("recorded_request_relationships")
    {
        return false;
    }
    let prefix = format!("fa://{}/run/{}@", expected[0], expected[2]);
    let Some(generation) = value
        .get("registration")
        .and_then(Json::as_str)
        .and_then(|address| address.strip_prefix(&prefix))
        .and_then(decimal_text)
        .filter(|generation| *generation != 0)
    else {
        return false;
    };
    if request
        .address
        .generation
        .is_some_and(|selected| selected != generation)
    {
        return false;
    }
    match (value.get("knowledge"), value.get("error")) {
        (Some(knowledge), None) => match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                exact(knowledge, &["state", "basis", "value"])
                    && knowledge.get("basis").is_some_and(|basis| {
                        basis_matches(basis, &expected)
                            && knowledge.get("value").is_some_and(|report| {
                                report_matches(report, basis, request, generation)
                            })
                    })
            }
            Some("unknown") => unknown(knowledge),
            Some("withheld") => {
                exact(knowledge, &["state", "authority_required"])
                    && knowledge.get("authority_required").and_then(Json::as_str)
                        == Some("supervisor_request_history")
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

fn basis_matches(basis: &Json, expected: &[u64; 6]) -> bool {
    if !exact(
        basis,
        &[
            "scope",
            "origin",
            "request",
            "request_generation",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "recorded_at",
        ],
    ) || basis.get("origin").and_then(Json::as_str) != Some("acknowledged_owner")
        || basis.get("request").and_then(decimal) != Some(expected[5])
        || [
            "request_generation",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
        ]
        .iter()
        .any(|name| basis.get(name).and_then(decimal).is_none())
        || !basis
            .get("recorded_at")
            .is_some_and(|value| value.is_null() || decimal(value).is_some())
    {
        return false;
    }
    let Some(scope) = basis.get("scope") else {
        return false;
    };
    let names = ["tenant", "principal", "run", "branch", "authority"];
    exact(
        scope,
        &[
            "tenant",
            "principal",
            "run",
            "branch",
            "authority",
            "purpose",
        ],
    ) && scope.get("purpose").and_then(Json::as_str) == Some("effect")
        && names
            .iter()
            .zip(expected)
            .all(|(name, expected)| scope.get(name).and_then(decimal) == Some(*expected))
}

fn report_matches(report: &Json, basis: &Json, request: &TowerRequest, generation: u64) -> bool {
    if !exact(
        report,
        &[
            "admitted",
            "attempt",
            "stage",
            "incomplete_reviews",
            "enumeration_complete",
            "reviews",
        ],
    ) || report.get("enumeration_complete").and_then(Json::as_bool) != Some(true)
    {
        return false;
    }
    let Some(admitted) = report.get("admitted").and_then(Json::as_bool) else {
        return false;
    };
    let Some(reviews) = report.get("reviews").and_then(Json::as_array) else {
        return false;
    };
    let Some(incomplete) = report.get("incomplete_reviews").and_then(decimal) else {
        return false;
    };
    let Some(revision) = basis.get("journal_revision").and_then(decimal) else {
        return false;
    };
    if revision > MAX_REQUEST_HISTORY_EVENTS as u64
        || incomplete > revision
        || reviews.len() > MAX_REQUEST_HISTORY_REVIEWS
    {
        return false;
    }
    if !admitted {
        return report.get("attempt").is_some_and(Json::is_null)
            && report.get("stage").is_some_and(Json::is_null)
            && incomplete == 0
            && reviews.is_empty();
    }
    if report
        .get("attempt")
        .and_then(decimal)
        .is_none_or(|attempt| attempt == 0)
        || !matches!(
            report.get("stage").and_then(Json::as_str),
            Some(
                "proposed"
                    | "prepared"
                    | "reviewing"
                    | "authorized"
                    | "dispatching"
                    | "confirmed"
                    | "denied"
                    | "cancelled"
                    | "unknown"
                    | "confirmed_not_executed"
                    | "irrecoverably_unknown"
            )
        )
    {
        return false;
    }
    let mut rounds = BTreeSet::new();
    let mut last_completion = 0;
    for row in reviews {
        if !exact(
            row,
            &[
                "round",
                "begin_revision",
                "completed_revision",
                "completion_kind",
                "explain_request",
            ],
        ) {
            return false;
        }
        let Some(round) = row
            .get("round")
            .and_then(decimal)
            .filter(|round| *round != 0)
        else {
            return false;
        };
        let Some(begin) = row
            .get("begin_revision")
            .and_then(decimal)
            .filter(|begin| *begin != 0)
        else {
            return false;
        };
        let Some(completed) = row.get("completed_revision").and_then(decimal) else {
            return false;
        };
        if !rounds.insert(round)
            || begin >= completed
            || completed > revision
            || completed <= last_completion
            || !matches!(
                row.get("completion_kind").and_then(Json::as_str),
                Some("application_attempted" | "evidence_refined")
            )
        {
            return false;
        }
        last_completion = completed;
        let mut address = request.address.clone();
        address.generation = Some(generation);
        let mut followup =
            TowerRequest::new(format!("related-explain-{round}"), address, Verb::Explain);
        followup.arguments = ReadArguments::ReviewExplanation { round };
        let Ok(encoded) = encode_request(&followup) else {
            return false;
        };
        let Ok(expected) = strict_json::parse(
            encoded.as_bytes(),
            strict_json::Limits {
                max_bytes: super::MAX_REQUEST_BYTES,
                max_depth: 4,
                max_items: 32,
                max_string_bytes: super::MAX_REQUEST_STRING_BYTES,
            },
        ) else {
            return false;
        };
        if row.get("explain_request") != Some(&expected) {
            return false;
        }
    }
    true
}

/// Bind a successful native disclosure to the independently selected complete
/// endpoint audience and optional local bounds. A profile does not grant the
/// server permission; native Unknown/Withheld/refusal remain truthful output.
pub(super) fn check_profile(
    parsed: &Json,
    request: &TowerRequest,
    scope: Scope,
    selected: Option<(usize, usize)>,
) -> Result<(), String> {
    let Some(result) = parsed.get("result") else {
        return Ok(());
    };
    if result.get("kind").and_then(Json::as_str) != Some("request-history") {
        return Ok(());
    }
    let knowledge = result.get("value").and_then(|value| value.get("knowledge"));
    if knowledge
        .and_then(|value| value.get("state"))
        .and_then(Json::as_str)
        != Some("known")
    {
        return Ok(());
    }
    let expected = [
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
    ];
    let valid = request_scope(request).is_some_and(|actual| actual[..5] == expected)
        && selected.is_none_or(|(events, reviews)| {
            let knowledge = knowledge.unwrap();
            knowledge
                .get("basis")
                .and_then(|value| value.get("journal_revision"))
                .and_then(decimal)
                .is_some_and(|revision| revision <= events as u64)
                && knowledge
                    .get("value")
                    .and_then(|value| value.get("reviews"))
                    .and_then(Json::as_array)
                    .is_some_and(|rows| rows.len() <= reviews)
        });
    if valid {
        Ok(())
    } else {
        Err("invalid or unbound Tower response; no later request or retry was sent".into())
    }
}
