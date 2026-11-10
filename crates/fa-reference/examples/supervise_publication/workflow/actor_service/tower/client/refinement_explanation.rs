//! Correlation and disclosure checks for an original completed refinement.
//! An observed residual update is never an application or an effect grant.

use super::review_explanation::{bound_basis, decimal, exact, header_matches, selected_round};
use super::{Json, TowerRequest};

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    let Some(round) = selected_round(request) else {
        return false;
    };
    if !header_matches(
        value,
        round,
        "fa.publication-refinement-explanation/1",
        "historical_completed_refinement",
    ) {
        return false;
    }
    let Some(knowledge) = value.get("knowledge") else {
        return false;
    };
    if !exact(knowledge, &["state", "basis", "value"])
        || knowledge.get("state").and_then(Json::as_str) != Some("known")
        || !knowledge
            .get("basis")
            .is_some_and(|basis| bound_basis(basis, request, round))
    {
        return false;
    }
    let Some(body) = knowledge.get("value") else {
        return false;
    };
    if !exact(
        body,
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
            "refinement",
        ],
    ) || !body.get("application").is_some_and(|application| {
        exact(application, &["state"])
            && application.get("state").and_then(Json::as_str) == Some("not_attempted")
    }) {
        return false;
    }
    let Some(refinement) = body.get("refinement") else {
        return false;
    };
    if !exact(
        refinement,
        &[
            "kind",
            "group",
            "reviewed_input_revision",
            "refined_input_revision",
            "requires_fresh_review",
        ],
    ) || refinement.get("kind").and_then(Json::as_str) != Some("kv_exact_residual")
        || refinement
            .get("requires_fresh_review")
            .and_then(Json::as_bool)
            != Some(true)
    {
        return false;
    }
    let Some(group) = refinement.get("group") else {
        return false;
    };
    if !exact(group, &["layer", "side", "position", "head"])
        || !matches!(
            group.get("side").and_then(Json::as_str),
            Some("key" | "value")
        )
        || ["layer", "position", "head"]
            .iter()
            .any(|name| group.get(name).and_then(decimal).is_none())
    {
        return false;
    }
    let (Some(reviewed), Some(refined)) = (
        refinement.get("reviewed_input_revision").and_then(decimal),
        refinement.get("refined_input_revision").and_then(decimal),
    ) else {
        return false;
    };
    body.get("input_revision").and_then(decimal) == Some(reviewed)
        && refined > reviewed
        && body
            .get("effect")
            .is_some_and(|effect| independent_effect(effect, round, refined))
}

fn independent_effect(effect: &Json, round: u64, refined: u64) -> bool {
    if !exact(
        effect,
        &[
            "scope",
            "stage_at_review",
            "stage_at_end",
            "dispatch",
            "outcome",
            "reconciliation",
        ],
    ) || effect.get("scope").and_then(Json::as_str)
        != Some("original_reference_publication_sink")
    {
        return false;
    }
    let Some(dispatch) = effect.get("dispatch") else {
        return false;
    };
    match dispatch.get("state").and_then(Json::as_str) {
        Some("unknown") => {
            exact(dispatch, &["state", "reason"])
                && dispatch.get("reason").and_then(Json::as_str) == Some("not_observed")
        }
        Some("known") => {
            if !exact(dispatch, &["state", "value"]) {
                return false;
            }
            let Some(observed) = dispatch.get("value") else {
                return false;
            };
            if !exact(
                observed,
                &["journal_revision", "selected_review", "automatic", "human"],
            ) || observed.get("journal_revision").and_then(decimal).is_none()
                || observed.get("selected_review").and_then(Json::as_bool) != Some(false)
            {
                return false;
            }
            let Some(automatic) = observed.get("automatic") else {
                return false;
            };
            if !exact(
                automatic,
                &[
                    "round",
                    "review_revision",
                    "authorization_revision",
                    "input_revision",
                ],
            ) || automatic
                .get("round")
                .and_then(decimal)
                .is_none_or(|value| value == 0 || value == round)
                || ["review_revision", "authorization_revision"]
                    .iter()
                    .any(|name| automatic.get(name).and_then(decimal).is_none())
                || automatic
                    .get("input_revision")
                    .and_then(decimal)
                    .is_none_or(|value| value < refined)
            {
                return false;
            }
            // Observe the independently consumed human approval. No native key
            // or approval is reconstructed from this public report.
            observed.get("human").is_some_and(|human| {
                exact(
                    human,
                    &[
                        "request",
                        "reviewer",
                        "disposition",
                        "created_at",
                        "expires_at",
                        "issued_at",
                        "finished_at",
                    ],
                ) && human.get("disposition").and_then(Json::as_str) == Some("consumed")
                    && ["request", "reviewer", "created_at", "expires_at"]
                        .iter()
                        .all(|name| human.get(name).and_then(decimal).is_some())
                    && ["issued_at", "finished_at"].iter().all(|name| {
                        human
                            .get(name)
                            .is_some_and(|value| value.is_null() || decimal(value).is_some())
                    })
            })
        }
        _ => false,
    }
}
