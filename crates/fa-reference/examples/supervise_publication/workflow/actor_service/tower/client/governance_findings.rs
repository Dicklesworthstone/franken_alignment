//! Check complete native incident observations without mistaking missing
//! standing-query inputs for a clean control plane.

use super::governance_ledger::{ReportedWindows, read_basis, refused_matches};
use super::hold_diagnosis::{decimal, decimal_text, exact};
use super::{Json, ReadArguments, Scope, TowerRequest, Verb};
use fa_reference::action::Purpose;
use fa_reference::action::consequence::oversight::credibility::MAX_EVALUATION_CASES;
use fa_reference::address_resolver::Noun;
use fa_reference::round::review::andon::MAX_WINDOWS;

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    let Some((_, reason)) = validate(value, request) else {
        return false;
    };
    let expected_missing = Json::Array(if reason.is_some() {
        vec![Json::String(request.address.canonical_string())]
    } else {
        Vec::new()
    });
    native_envelope(envelope)
        && envelope.get("missing") == Some(&expected_missing)
        && envelope.get("would_change_this") == Some(&expected_missing)
        && envelope.get("status").and_then(Json::as_str)
            == Some(if reason.is_some() { "error" } else { "ok" })
        && envelope.get("retry_safe").and_then(Json::as_bool) == Some(true)
        && match reason {
            Some(reason) => envelope.get("reason_code").and_then(Json::as_str) == Some(reason),
            None => envelope.get("reason_code").is_some_and(Json::is_null),
        }
}

pub(super) fn check_profile(
    parsed: &Json,
    request: &TowerRequest,
    scope: Scope,
) -> Result<(), String> {
    if request.verb != Verb::Findings {
        return Ok(());
    }
    let invalid = || {
        "governance findings changed the configured audience; no later request or retry was sent"
            .to_owned()
    };
    if scope.purpose != Purpose::Effect
        || request.address.kind != Noun::Run
        || request.address.tenant != scope.tenant.to_string()
        || request.address.id != scope.run.to_string()
    {
        return Err(invalid());
    }
    let result = parsed.get("result").ok_or_else(invalid)?;
    let value = result.get("value").ok_or_else(invalid)?;
    if result.get("kind").and_then(Json::as_str) == Some("refused") {
        return if refused_matches(value, request, parsed.get("envelope").ok_or_else(invalid)?) {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let (observed, _) = validate(value, request).ok_or_else(invalid)?;
    if observed.is_some_and(|observed| observed != scope) {
        return Err(invalid());
    }
    Ok(())
}

fn validate(value: &Json, request: &TowerRequest) -> Option<(Option<Scope>, Option<&'static str>)> {
    if request.verb != Verb::Findings
        || request.arguments != ReadArguments::None
        || request.address.kind != Noun::Run
        || request.address.generation == Some(0)
        || decimal_text(&request.address.tenant)? == 0
        || decimal_text(&request.address.id)? == 0
    {
        return None;
    }
    let field = match (value.get("knowledge"), value.get("error")) {
        (Some(_), None) => "knowledge",
        (None, Some(_)) => "error",
        _ => return None,
    };
    if !exact(
        value,
        &[
            "schema",
            "operation",
            "registration",
            "query_address",
            "disclosure",
            "current_eligibility",
            "label_truth",
            "population_calibration",
            "effect_authority",
            field,
        ],
    ) || value.get("schema")?.as_str()? != "fa.governance-findings/1"
        || value.get("operation")?.as_str()? != "findings"
        || value.get("query_address")?.as_str()? != request.address.canonical_string()
        || value.get("disclosure")?.as_str()? != "recorded_governance_accounting"
        || value.get("current_eligibility")?.as_str()? != "not_observed"
        || value.get("label_truth")?.as_str()? != "not_verified"
        || value.get("population_calibration")?.as_str()? != "not_established"
        || value.get("effect_authority")?.as_bool()?
    {
        return None;
    }
    let prefix = format!(
        "fa://{}/run/{}@",
        request.address.tenant, request.address.id
    );
    let generation = decimal_text(value.get("registration")?.as_str()?.strip_prefix(&prefix)?)?;
    if generation == 0
        || request
            .address
            .generation
            .is_some_and(|expected| expected != generation)
    {
        return None;
    }
    if field == "error" {
        let error = value.get("error")?;
        if !exact(error, &["code"]) {
            return None;
        }
        return Some((
            None,
            Some(match error.get("code")?.as_str()? {
                "capacity" => "BudgetExhausted",
                "unavailable" => "LowerLayerUnverified",
                _ => return None,
            }),
        ));
    }
    let knowledge = value.get("knowledge")?;
    match knowledge.get("state")?.as_str()? {
        "known" => {
            if !exact(knowledge, &["state", "basis", "value"]) {
                return None;
            }
            let basis = knowledge.get("basis")?;
            let scope = read_basis(basis, request)?;
            let report = knowledge.get("value")?;
            if !exact(
                report,
                &["credibility_incident", "andon_incidents", "risk_theater"],
            ) || !section(report.get("credibility_incident")?, basis, credibility)
                || !section(report.get("andon_incidents")?, basis, |value| {
                    andon(value, report.get("credibility_incident")?)
                })
                || (report.get("andon_incidents")?.get("state")?.as_str()? == "known"
                    && report.get("credibility_incident")?.get("state")?.as_str()? != "known")
                || !coverage(report.get("risk_theater")?)
            {
                return None;
            }
            Some((Some(scope), None))
        }
        "unknown"
            if exact(knowledge, &["state", "reason"])
                && knowledge.get("reason")?.as_str()? == "controller_unavailable" =>
        {
            Some((None, Some("LowerLayerUnverified")))
        }
        "withheld"
            if exact(knowledge, &["state", "authority_required"])
                && knowledge.get("authority_required")?.as_str()?
                    == "supervisor_governance_ledger" =>
        {
            Some((None, Some("DeniedByPolicy")))
        }
        _ => None,
    }
}

fn section(value: &Json, basis: &Json, check: impl FnOnce(&Json) -> Option<()>) -> bool {
    match value.get("state").and_then(Json::as_str) {
        Some("unknown") => {
            exact(value, &["state", "reason"])
                && value.get("reason").and_then(Json::as_str) == Some("source_not_installed")
        }
        Some("known") => {
            exact(value, &["state", "basis", "value"])
                && value.get("basis") == Some(basis)
                && value
                    .get("value")
                    .is_some_and(|value| check(value).is_some())
        }
        _ => false,
    }
}

const FAMILIES: [&str; 6] = [
    "consequence_drift",
    "andon_closure_mode",
    "roster_churn_near_holds",
    "coverage_erosion",
    "autonomy_inflation",
    "audit_floor_starvation",
];

fn coverage(value: &Json) -> bool {
    exact(value, &FAMILIES)
        && FAMILIES.iter().all(|name| {
            value.get(name).is_some_and(|status| {
                exact(status, &["state", "reason"])
                    && status.get("state").and_then(Json::as_str) == Some("unknown")
                    && status.get("reason").and_then(Json::as_str) == Some("lower_layer_unverified")
            })
        })
}

fn credibility(value: &Json) -> Option<()> {
    if !exact(
        value,
        &[
            "evaluation_scope",
            "ledger_revision",
            "policy_generation",
            "lifetime_false_stops",
            "false_stop_budget",
            "calibration_incident_open",
        ],
    ) {
        return None;
    }
    let scope = value.get("evaluation_scope")?;
    if !exact(scope, &["domain", "stratum", "period"])
        || ["domain", "stratum", "period"].iter().any(|name| {
            scope
                .get(name)
                .and_then(decimal)
                .is_none_or(|value| value == 0)
        })
    {
        return None;
    }
    decimal(value.get("ledger_revision")?)?;
    decimal(value.get("policy_generation")?)?;
    let stops = decimal(value.get("lifetime_false_stops")?)?;
    let budget = decimal(value.get("false_stop_budget")?)?;
    (stops <= MAX_EVALUATION_CASES as u64
        && budget != 0
        && value.get("calibration_incident_open")?.as_bool()? == (stops >= budget))
        .then_some(())
}

fn andon(value: &Json, credibility: &Json) -> Option<()> {
    if !exact(
        value,
        &["deployment", "evaluator", "inspected_windows", "incidents"],
    ) {
        return None;
    }
    let inspected = decimal(value.get("inspected_windows")?)?;
    let incidents = value.get("incidents")?.as_array()?;
    if inspected == 0 || inspected > MAX_WINDOWS as u64 || incidents.len() as u64 > inspected {
        return None;
    }
    let mut observed = ReportedWindows::new(value, credibility)?;
    for incident in incidents {
        if !exact(
            incident,
            &[
                "window",
                "start",
                "end",
                "false_stops_allowed",
                "cost_per_stop",
                "false_stops",
                "total_cost",
                "opened_by_round",
                "support_rounds",
            ],
        ) {
            return None;
        }
        let (budget, count) = observed.window(incident)?;
        let opening = decimal(incident.get("opened_by_round")?)?;
        if count == 0 || count < budget.false_stops_allowed {
            return None;
        }
        let rounds = incident.get("support_rounds")?.as_array()?;
        if rounds.len() as u64 != count {
            return None;
        }
        let mut previous_round = 0;
        let mut opened = false;
        for round in rounds {
            let round = decimal(round)?;
            if round <= previous_round || !observed.round(round) {
                return None;
            }
            previous_round = round;
            opened |= round == opening;
        }
        if !opened {
            return None;
        }
    }
    Some(())
}

fn native_envelope(envelope: &Json) -> bool {
    [
        ("generation", "SemanticGeneration"),
        ("control_seq", "ControlSequence"),
        ("source_frontiers", "SourceFrontiers"),
        ("budgets_remaining", "RemainingPublicationBudget"),
        ("realized_cost", "RealizedPublicationCost"),
        ("affordances", "Affordances"),
    ]
    .iter()
    .all(|(field, kind)| {
        envelope.get(field).is_some_and(|value| {
            exact(value, &["type", "state", "reason", "would_change_this"])
                && value.get("type").and_then(Json::as_str) == Some(*kind)
                && value.get("state").and_then(Json::as_str) == Some("Unknown")
                && value.get("reason").and_then(Json::as_str) == Some("Unavailable")
                && value
                    .get("would_change_this")
                    .and_then(Json::as_str)
                    .is_some()
        })
    })
}
