//! Correlate a bounded recorded-state diagnosis without reconstructing authority.
//! These checks establish transport shape and internal consistency, not the
//! truth of native history or fresh source, clock, key or endpoint observations.

use super::{Json, ReadArguments, TowerRequest, Verb};
use fa_reference::address_resolver::Noun;

const CONDITIONS: [&str; 10] = [
    "recorded_admission_open",
    "recorded_not_suspended",
    "recorded_policy_epoch_matches",
    "recorded_deadline_open",
    "recorded_stage_authorized",
    "dispatch_not_recorded",
    "current_source_evidence",
    "current_approvals",
    "current_clock",
    "final_publication_eligibility",
];

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    if request.verb != Verb::WhyHeld || request.arguments != ReadArguments::None {
        return false;
    }
    let Some(expected) = request_scope(request) else {
        return false;
    };
    let fields = [
        "schema",
        "operation",
        "scope",
        "request",
        "registration",
        "historical",
        "live_evidence_read",
        "clock_read",
        "effect_authority",
        "payload_disclosed",
        "current_eligibility",
        "blocker_completeness",
    ];
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != fields.len() + 1
        || fields.iter().any(|name| !object.contains_key(*name))
        || value.get("schema").and_then(Json::as_str)
            != Some("fa.publication-hold-diagnosis/1")
        || value.get("operation").and_then(Json::as_str) != Some("why-held")
        || value.get("scope").and_then(Json::as_str) != Some("recorded_publication_state")
        || value.get("request").and_then(decimal) != Some(expected[5])
        || value.get("historical").and_then(Json::as_bool) != Some(true)
        || value.get("current_eligibility").and_then(Json::as_str) != Some("not_observed")
        || value.get("blocker_completeness").and_then(Json::as_str)
            != Some("recorded_conditions_only")
        || ["live_evidence_read", "clock_read", "effect_authority", "payload_disclosed"]
            .iter()
            .any(|name| value.get(name).and_then(Json::as_bool) != Some(false))
    {
        return false;
    }
    let registration_prefix = format!("fa://{}/run/{}@", expected[0], expected[2]);
    let generation = value
        .get("registration")
        .and_then(Json::as_str)
        .and_then(|address| address.strip_prefix(&registration_prefix))
        .and_then(decimal_text);
    if generation.is_none_or(|generation| generation == 0)
        || request.address.generation.is_some_and(|expected| generation != Some(expected))
    {
        return false;
    }
    match (value.get("knowledge"), value.get("error")) {
        (Some(knowledge), None) => match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                exact(knowledge, &["state", "basis", "value"])
                    && knowledge.get("basis").is_some_and(|basis| {
                        bound_basis(basis, &expected)
                            && knowledge.get("value").is_some_and(|value| report(value, basis))
                    })
            }
            Some("unknown") => unknown(knowledge),
            Some("withheld") => {
                exact(knowledge, &["state", "authority_required"])
                    && knowledge.get("authority_required").and_then(Json::as_str)
                        == Some("supervisor_hold_diagnosis")
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

// Tenant, principal, run, branch, authority and ORIGINAL external request.
// A native request number is never interpreted as a ledger attempt number.
fn request_scope(request: &TowerRequest) -> Option<[u64; 6]> {
    if request.address.kind != Noun::Attempt {
        return None;
    }
    let parts: Vec<_> = request.address.id.split('-').collect();
    if parts.len() != 6 || parts[0] != "request" {
        return None;
    }
    let scope = [
        decimal_text(&request.address.tenant)?,
        decimal_text(parts[2])?,
        decimal_text(parts[1])?,
        decimal_text(parts[3])?,
        decimal_text(parts[4])?,
        decimal_text(parts[5])?,
    ];
    (!scope.contains(&0)).then_some(scope)
}

fn bound_basis(basis: &Json, expected: &[u64; 6]) -> bool {
    if !exact(
        basis,
        &[
            "scope",
            "origin",
            "request",
            "request_generation",
            "attempt",
            "journal_revision",
            "control_sequence",
            "recorded_at",
        ],
    ) || basis.get("origin").and_then(Json::as_str) != Some("acknowledged_owner")
        || basis.get("request").and_then(decimal) != Some(expected[5])
        || basis.get("attempt").and_then(decimal).is_none_or(|value| value == 0)
        || ["request_generation", "journal_revision", "control_sequence"]
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
    exact(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])
        && scope.get("purpose").and_then(Json::as_str) == Some("effect")
        && names
            .iter()
            .zip(expected)
            .all(|(name, expected)| scope.get(name).and_then(decimal) == Some(*expected))
}

fn report(value: &Json, basis: &Json) -> bool {
    if !exact(
        value,
        &["stage", "proposal_policy_epoch", "current_policy_epoch", "deadline", "predicates"],
    ) {
        return false;
    }
    let Some(stage) = value.get("stage").and_then(Json::as_str) else {
        return false;
    };
    let dispatch_not_recorded = match stage {
        "proposed" | "prepared" | "reviewing" | "authorized" | "denied" | "cancelled" => true,
        "dispatching" | "confirmed" | "unknown" | "confirmed_not_executed"
        | "irrecoverably_unknown" => false,
        _ => return false,
    };
    let Some(proposal_epoch) = value.get("proposal_policy_epoch").and_then(decimal) else {
        return false;
    };
    let Some(current_epoch) = value.get("current_policy_epoch").and_then(decimal) else {
        return false;
    };
    let Some(deadline) = value.get("deadline").and_then(decimal) else {
        return false;
    };
    let Some(predicates) = value.get("predicates").and_then(Json::as_array) else {
        return false;
    };
    if predicates.len() != CONDITIONS.len() {
        return false;
    }
    let recorded_at = basis.get("recorded_at").and_then(decimal);
    for (index, (predicate, condition)) in predicates.iter().zip(CONDITIONS).enumerate() {
        if !exact(predicate, &["condition", "satisfied"])
            || predicate.get("condition").and_then(Json::as_str) != Some(condition)
        {
            return false;
        }
        let Some(satisfied) = predicate.get("satisfied") else {
            return false;
        };
        if index >= 6 || (index == 3 && recorded_at.is_none()) {
            if !unknown(satisfied) {
                return false;
            }
            continue;
        }
        if !exact(satisfied, &["state", "basis", "value"])
            || satisfied.get("state").and_then(Json::as_str) != Some("known")
            || satisfied.get("basis") != Some(basis)
        {
            return false;
        }
        let Some(observed) = satisfied.get("value").and_then(Json::as_bool) else {
            return false;
        };
        let expected = match index {
            2 => Some(proposal_epoch == current_epoch),
            3 => recorded_at.map(|tick| tick < deadline),
            4 => Some(stage == "authorized"),
            5 => Some(dispatch_not_recorded),
            _ => None,
        };
        if expected.is_some_and(|expected| expected != observed) {
            return false;
        }
    }
    true
}

fn unknown(value: &Json) -> bool {
    exact(value, &["state", "reason"])
        && value.get("state").and_then(Json::as_str) == Some("unknown")
        && value.get("reason").and_then(Json::as_str) == Some("controller_unavailable")
}

fn decimal(value: &Json) -> Option<u64> {
    decimal_text(value.as_str()?)
}

fn decimal_text(spelling: &str) -> Option<u64> {
    let number = spelling.parse::<u64>().ok()?;
    (spelling == number.to_string()).then_some(number)
}

fn exact(value: &Json, names: &[&str]) -> bool {
    value.as_object().is_some_and(|fields| {
        fields.len() == names.len() && names.iter().all(|name| fields.contains_key(*name))
    })
}
