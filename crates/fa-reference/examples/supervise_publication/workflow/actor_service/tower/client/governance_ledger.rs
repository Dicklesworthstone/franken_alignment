//! Validate an acknowledged governance observation without treating its labels,
//! qualification counters or transport authentication as publication authority.

use super::hold_diagnosis::{decimal, decimal_text, exact};
use super::{Json, ReadArguments, Scope, TowerRequest, Verb};
use fa_reference::action::Purpose;
use fa_reference::action::consequence::oversight::credibility::{Fraction, MAX_EVALUATION_CASES};
use fa_reference::address_resolver::Noun;
use fa_reference::round::review::andon::{
    AndonBudget, AndonLedger, LedgerIdentity, MAX_FALSE_HOLDS, MAX_WINDOWS,
};
use fa_reference::round::{MAX_FIELD_LEN, MAX_MEMBERS};
use std::collections::BTreeSet;

pub(super) fn matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    let Some((_, reason)) = validate(value, request) else {
        return false;
    };
    envelope.get("status").and_then(Json::as_str)
        == Some(if reason.is_some() { "error" } else { "ok" })
        && envelope.get("retry_safe").and_then(Json::as_bool) == Some(true)
        && match reason {
            Some(reason) => envelope.get("reason_code").and_then(Json::as_str) == Some(reason),
            None => envelope.get("reason_code").is_some_and(Json::is_null),
        }
}

pub(super) fn refused_matches(value: &Json, request: &TowerRequest, envelope: &Json) -> bool {
    if !matches!(request.verb, Verb::Ledger | Verb::Findings) {
        return true;
    }
    if value.get("type").and_then(Json::as_str) != Some("PublicationRefusal")
        || envelope.get("status").and_then(Json::as_str) != Some("error")
        || envelope.get("retry_safe").and_then(Json::as_bool).is_none()
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
                && matches!(
                    envelope.get("reason_code").and_then(Json::as_str),
                    Some(
                        "DeniedByPolicy"
                            | "MissingRequiredObservation"
                            | "StaleGeneration"
                            | "CapabilityUnsupported"
                            | "BudgetExhausted"
                            | "IntegrityFailure"
                            | "PurposeMismatch"
                            | "LowerLayerUnverified"
                    )
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
    if request.verb != Verb::Ledger {
        return Ok(());
    }
    let invalid = || {
        "governance ledger changed the configured audience; no later request or retry was sent"
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
    if request.verb != Verb::Ledger
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
    ) || value.get("schema")?.as_str()? != "fa.governance-ledger/1"
        || value.get("operation")?.as_str()? != "ledger"
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
            if !exact(report, &["credibility", "andon"])
                || !section(
                    report.get("credibility")?,
                    basis,
                    "original_owner_credibility_ledger",
                    credibility,
                )
                || !section(
                    report.get("andon")?,
                    basis,
                    "original_owner_andon_ledger",
                    |value| andon(value, report.get("credibility")?),
                )
                || (report.get("andon")?.get("state")?.as_str()? == "known"
                    && report.get("credibility")?.get("state")?.as_str()? != "known")
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

pub(super) fn read_basis(basis: &Json, request: &TowerRequest) -> Option<Scope> {
    if !exact(
        basis,
        &[
            "scope",
            "origin",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "recorded_at",
        ],
    ) || basis.get("origin")?.as_str()? != "acknowledged_owner"
        || ["journal_revision", "control_sequence", "authority_epoch"]
            .iter()
            .any(|name| basis.get(name).and_then(decimal).is_none())
        || !basis
            .get("recorded_at")
            .is_some_and(|value| value.is_null() || decimal(value).is_some())
    {
        return None;
    }
    let scope = basis.get("scope")?;
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
        || request.address.tenant != scope.tenant.to_string()
        || request.address.id != scope.run.to_string()
    {
        return None;
    }
    Some(scope)
}

fn section(
    value: &Json,
    basis: &Json,
    domain: &str,
    check: impl FnOnce(&Json) -> Option<()>,
) -> bool {
    if value.get("basis") != Some(basis) {
        return false;
    }
    match value.get("state").and_then(Json::as_str) {
        Some("absent") => {
            exact(value, &["state", "closed_domain", "basis"])
                && value.get("closed_domain").and_then(Json::as_str) == Some(domain)
        }
        Some("known") => {
            exact(value, &["state", "basis", "value"])
                && value
                    .get("value")
                    .is_some_and(|value| check(value).is_some())
        }
        _ => false,
    }
}

fn credibility(value: &Json) -> Option<()> {
    if !exact(
        value,
        &[
            "evaluation_scope",
            "ledger_revision",
            "policy_generation",
            "scoped_cases",
            "retained_cases",
            "pending_cases",
            "censored_cases",
            "violation_origins",
            "benign_origins",
            "lifetime_false_stops",
            "false_stop_budget",
            "minimum_violation_origins",
            "minimum_benign_origins",
            "precision_floor",
            "recall_floor",
            "false_positive_ceiling",
            "calibration_incident_open",
            "members",
            "qualification_failures",
        ],
    ) {
        return None;
    }
    let scope = value.get("evaluation_scope")?;
    if !exact(scope, &["domain", "stratum", "period"])
        || ["domain", "stratum", "period"]
            .iter()
            .any(|name| scope.get(name).and_then(decimal).is_none_or(|n| n == 0))
    {
        return None;
    }
    for name in ["ledger_revision", "policy_generation"] {
        decimal(value.get(name)?)?;
    }
    let minimum_violations = decimal(value.get("minimum_violation_origins")?)?;
    let minimum_benign = decimal(value.get("minimum_benign_origins")?)?;
    let lifetime_false_stops = decimal(value.get("lifetime_false_stops")?)?;
    let false_stop_budget = decimal(value.get("false_stop_budget")?)?;
    let precision_floor = fraction(value.get("precision_floor")?, true)?;
    let recall_floor = fraction(value.get("recall_floor")?, true)?;
    let false_positive_ceiling = fraction(value.get("false_positive_ceiling")?, false)?;
    if minimum_violations == 0
        || minimum_benign == 0
        || false_stop_budget == 0
        || minimum_violations.checked_add(minimum_benign)? > MAX_EVALUATION_CASES as u64
        || value.get("calibration_incident_open")?.as_bool()?
            != (lifetime_false_stops >= false_stop_budget)
    {
        return None;
    }
    let scoped = decimal(value.get("scoped_cases")?)?;
    let retained = decimal(value.get("retained_cases")?)?;
    let pending = decimal(value.get("pending_cases")?)?;
    let censored = decimal(value.get("censored_cases")?)?;
    let violations = decimal(value.get("violation_origins")?)?;
    let benign = decimal(value.get("benign_origins")?)?;
    if retained > MAX_EVALUATION_CASES as u64
        || scoped > retained
        || lifetime_false_stops > retained
        || pending
            .checked_add(censored)?
            .checked_add(violations)?
            .checked_add(benign)?
            > scoped
    {
        return None;
    }
    let members = value.get("members")?.as_array()?;
    if members.is_empty() || members.len() > MAX_MEMBERS {
        return None;
    }
    // Recompute only the original report's internal consistency. This does not
    // verify its independently assessed truth or the configured thresholds.
    // Native reports append global failures first, then the BTreeMap member
    // failures in name order, then the lifetime false-stop failure.
    let mut expected = Vec::with_capacity(3 * members.len() + 5);
    if pending != 0 {
        expected.push(("pending_labels", None));
    }
    if censored != 0 {
        expected.push(("censored_labels", None));
    }
    if violations < minimum_violations {
        expected.push(("insufficient_violation_origins", None));
    }
    if benign < minimum_benign {
        expected.push(("insufficient_benign_origins", None));
    }
    let mut previous: Option<&str> = None;
    for member in members {
        if !exact(
            member,
            &[
                "member",
                "true_positives",
                "false_negatives",
                "false_positives",
                "true_negatives",
                "missing_origins",
                "abstaining_origins",
            ],
        ) {
            return None;
        }
        let name = member.get("member")?.as_str()?;
        if name.is_empty()
            || name.len() > MAX_FIELD_LEN
            || previous.is_some_and(|previous| name <= previous)
        {
            return None;
        }
        previous = Some(name);
        let true_positives = decimal(member.get("true_positives")?)?;
        let false_negatives = decimal(member.get("false_negatives")?)?;
        let true_negatives = decimal(member.get("true_negatives")?)?;
        let false_positives = decimal(member.get("false_positives")?)?;
        if true_positives.checked_add(false_negatives)? != violations
            || true_negatives.checked_add(false_positives)? != benign
            // Replicated cases from one origin can include both a missing and
            // an abstaining verdict; these are overlapping coverage counters.
            || decimal(member.get("missing_origins")?)? > violations.checked_add(benign)?
            || decimal(member.get("abstaining_origins")?)? > violations.checked_add(benign)?
        {
            return None;
        }
        if !meets_floor(
            precision_floor,
            true_positives,
            true_positives.checked_add(false_positives)?,
        ) {
            expected.push(("precision", Some(name)));
        }
        if !meets_floor(recall_floor, true_positives, violations) {
            expected.push(("recall", Some(name)));
        }
        if !meets_ceiling(false_positive_ceiling, false_positives, benign) {
            expected.push(("false_positive_rate", Some(name)));
        }
    }
    if lifetime_false_stops >= false_stop_budget {
        expected.push(("false_stop_budget_exhausted", None));
    }
    let failures = value.get("qualification_failures")?.as_array()?;
    if failures.len() != expected.len() {
        return None;
    }
    for (failure, (reason, member)) in failures.iter().zip(expected) {
        if !exact(failure, &["reason", "member"]) || failure.get("reason")?.as_str()? != reason {
            return None;
        }
        match (member, failure.get("member")?) {
            (None, Json::Null) => {}
            (Some(expected), Json::String(actual)) if expected == actual.as_str() => {}
            _ => return None,
        }
    }
    Some(())
}

/// Constraints visible across a single file-backed Andon observation. Reuse
/// the native registration law without fabricating undisclosed confirmations.
pub(super) struct ReportedWindows {
    ledger: AndonLedger,
    remaining_stops: u64,
    rounds: BTreeSet<u64>,
}

impl ReportedWindows {
    pub(super) fn new(value: &Json, credibility: &Json) -> Option<Self> {
        let identity = LedgerIdentity {
            deployment: decimal(value.get("deployment")?)?,
            evaluator: decimal(value.get("evaluator")?)?,
        };
        // This file owner starts with no confirmations. Only a final benign
        // assessment of an eligible stopped case can charge its window.
        let lifetime = decimal(credibility.get("value")?.get("lifetime_false_stops")?)?;
        Some(Self {
            ledger: AndonLedger::new(identity).ok()?,
            remaining_stops: lifetime.min(MAX_FALSE_HOLDS as u64),
            rounds: BTreeSet::new(),
        })
    }

    pub(super) fn window(&mut self, value: &Json) -> Option<(AndonBudget, u64)> {
        let budget = AndonBudget {
            window: decimal(value.get("window")?)?,
            start: decimal(value.get("start")?)?,
            end: decimal(value.get("end")?)?,
            false_stops_allowed: decimal(value.get("false_stops_allowed")?)?,
            cost_per_stop: decimal(value.get("cost_per_stop")?)?,
        };
        let count = decimal(value.get("false_stops")?)?;
        if self
            .ledger
            .windows()
            .last_key_value()
            .is_some_and(|(id, _)| *id >= budget.window)
            || !self.ledger.register_window(budget).ok()?
            || decimal128(value.get("total_cost")?)?
                != u128::from(count) * u128::from(budget.cost_per_stop)
        {
            return None;
        }
        self.remaining_stops = self.remaining_stops.checked_sub(count)?;
        Some((budget, count))
    }

    pub(super) fn round(&mut self, round: u64) -> bool {
        round != 0 && self.rounds.insert(round)
    }
}

fn andon(value: &Json, credibility: &Json) -> Option<()> {
    if !exact(value, &["deployment", "evaluator", "windows"]) {
        return None;
    }
    let mut observed = ReportedWindows::new(value, credibility)?;
    let windows = value.get("windows")?.as_array()?;
    if windows.is_empty() || windows.len() > MAX_WINDOWS {
        return None;
    }
    for window in windows {
        if !exact(
            window,
            &[
                "window",
                "start",
                "end",
                "false_stops_allowed",
                "cost_per_stop",
                "false_stops",
                "total_cost",
                "calibration_incident",
            ],
        ) {
            return None;
        }
        let (budget, count) = observed.window(window)?;
        let incident = window.get("calibration_incident")?;
        if incident.is_null() {
            if count > 0 && count >= budget.false_stops_allowed {
                return None;
            }
        } else if !exact(
            incident,
            &["window", "opened_by_round", "contributing_rounds"],
        ) || decimal(incident.get("window")?)? != budget.window
            || !observed.round(decimal(incident.get("opened_by_round")?)?)
            || decimal(incident.get("contributing_rounds")?)? == 0
            || decimal(incident.get("contributing_rounds")?)? != count
            || count < budget.false_stops_allowed
        {
            return None;
        }
    }
    Some(())
}

pub(super) fn decimal128(value: &Json) -> Option<u128> {
    let text = value.as_str()?;
    let number = text.parse::<u128>().ok()?;
    (number.to_string() == text).then_some(number)
}

fn fraction(value: &Json, positive: bool) -> Option<Fraction> {
    if !exact(value, &["numerator", "denominator"]) {
        return None;
    }
    let numerator = decimal(value.get("numerator")?)?;
    let denominator = decimal(value.get("denominator")?)?;
    (denominator != 0 && numerator <= denominator && (!positive || numerator != 0)).then_some(
        Fraction {
            numerator,
            denominator,
        },
    )
}

// Match the native Fraction methods, including a failure when no observations
// supply the denominator. Even a zero false-positive ceiling needs a sample.
fn meets_floor(fraction: Fraction, numerator: u64, denominator: u64) -> bool {
    denominator != 0
        && u128::from(numerator) * u128::from(fraction.denominator)
            >= u128::from(fraction.numerator) * u128::from(denominator)
}

fn meets_ceiling(fraction: Fraction, numerator: u64, denominator: u64) -> bool {
    denominator != 0
        && u128::from(numerator) * u128::from(fraction.denominator)
            <= u128::from(fraction.numerator) * u128::from(denominator)
}
