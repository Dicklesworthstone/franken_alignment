//! A finite reader of ONE original native situation capture. Wire cursors are
//! ephemeral continuation data, never a native capability or a persisted resume
//! point. Incomplete output stays incomplete; this client cannot recapture it.

use super::{
    Budget, Endpoint, Instant, Json, OperatorSettings, Path, TowerRequest, Verb, Write,
    connect_endpoint, emit_response, parse_response, request_frame,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_INSPECTION_ATTEMPTS;
use fa_reference::action::consequence::oversight::actor::MAX_ACTOR_REQUESTS;
use fa_reference::action::{Purpose, Scope};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::tower::publication::request_address_for;
use fa_reference::tower::{MAX_REQUEST_STRING_BYTES, ReadArguments};
use std::collections::{BTreeMap, BTreeSet};

const MAX_PAGES: usize = 64;
const USAGE: &str = "tower-situation OPERATOR_PROFILE RUN_ADDRESS --max-pages N [--request-links] (canonical run address with nonzero registration generation; N is 1..64; one capture, no reconnect or retry)";
const INVALID: &str =
    "invalid or inconsistent native situation page; no continuation or new capture was requested";

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    // Own the entire grammar and address before opening even the public profile.
    let (address, max_pages, request_links) = selection(args)?;
    let started = Instant::now();
    let settings = OperatorSettings::read(Path::new(&args[1]))?;
    if address.tenant != settings.scope.tenant.to_string()
        || address.id != settings.scope.run.to_string()
    {
        return Err("Tower situation address does not match the operator profile scope; no connection was opened".into());
    }
    enumerate(
        &settings,
        address,
        max_pages,
        request_links,
        output,
        started,
    )
}

fn selection(args: &[String]) -> Result<(Address, usize, bool), String> {
    if !matches!(args.len(), 5 | 6)
        || args[0] != "tower-situation"
        || args[1].is_empty()
        || args[1].starts_with("--")
        || args[2].len() > MAX_REQUEST_STRING_BYTES
        || args[3] != "--max-pages"
        || (args.len() == 6 && args[5] != "--request-links")
    {
        return Err(USAGE.into());
    }
    let address = Address::parse(&args[2]).map_err(|_| USAGE.to_owned())?;
    let max_pages = decimal(&args[4]).ok_or_else(|| USAGE.to_owned())?;
    if address.kind != Noun::Run
        || address.generation.is_none_or(|generation| generation == 0)
        || address.canonical_string() != args[2]
        || decimal(&address.tenant).is_none_or(|tenant| tenant == 0)
        || decimal(&address.id).is_none_or(|run| run == 0)
        || !(1..=MAX_PAGES as u64).contains(&max_pages)
    {
        return Err(USAGE.into());
    }
    Ok((address, max_pages as usize, args.len() == 6))
}

fn enumerate(
    settings: &OperatorSettings,
    address: Address,
    max_pages: usize,
    request_links: bool,
    output: &mut impl Write,
    started: Instant,
) -> Result<(), String> {
    let endpoint = Endpoint::operator(settings);
    let page_limit = (max_pages as u64).min(endpoint.exchanges) as usize;
    let mut budget = Budget::new(&endpoint, started);
    let mut capture = Capture::new(settings.scope, max_pages);
    if request_links {
        capture = capture.with_request_links(address.clone());
    }
    let mut socket = connect_endpoint(endpoint, &budget, || settings.check_directory())?;
    let mut cursor = None;
    for page in 1..=page_limit {
        let mut request = TowerRequest::new(
            format!("tower-situation-page-{page}"),
            address.clone(),
            Verb::Situation,
        );
        request.arguments = if request_links {
            ReadArguments::SituationLinks { cursor }
        } else {
            cursor.map_or(ReadArguments::None, |cursor| ReadArguments::SituationPage { cursor })
        };
        let frame = request_frame(&request)?;
        budget.write_frame(&mut socket, &frame)?;
        let response = budget.read_frame(&mut socket)?;
        let parsed = parse_response(&response, &request)?;
        // Validate the complete page before exposing its bytes or using its
        // cursor. No success is inferred from a partial list or a missing handle.
        let progress = capture.accept(&parsed)?;
        emit_response(&response, output, &budget)?;
        match progress {
            Progress::Complete => return Ok(()),
            Progress::Continue(next) => cursor = Some(next),
            Progress::Stopped(reason) => return Err(reason.into()),
        }
    }
    Err("Tower situation enumeration is incomplete: page or original exchange allowance exhausted; partial pages retained; no reconnect or new capture".into())
}

enum Progress {
    Complete,
    Continue(u64),
    Stopped(&'static str),
}

struct Capture {
    scope: Scope,
    link_registration: Option<Address>,
    basis: Option<Json>,
    summary: Option<Json>,
    next_offset: u64,
    attempts: BTreeSet<u64>,
    requests: BTreeSet<u64>,
    cursors: BTreeSet<u64>,
    max_rows: usize,
}

impl Capture {
    fn new(scope: Scope, max_pages: usize) -> Self {
        Self {
            scope,
            link_registration: None,
            basis: None,
            summary: None,
            next_offset: 0,
            attempts: BTreeSet::new(),
            requests: BTreeSet::new(),
            cursors: BTreeSet::new(),
            max_rows: max_pages * MAX_INSPECTION_ATTEMPTS,
        }
    }

    /// Freeze the original validated run registration for every returned link.
    /// Link data is never followed automatically or exchanged for a ticket.
    fn with_request_links(mut self, registration: Address) -> Self {
        self.link_registration = Some(registration);
        self
    }

    fn accept(&mut self, response: &Json) -> Result<Progress, String> {
        let envelope = response.get("envelope").ok_or(INVALID)?;
        let result = response.get("result").ok_or(INVALID)?;
        if result.get("kind").and_then(Json::as_str) == Some("refused") {
            // The bounded correlated refusal remains an original diagnostic,
            // never an enumeration result or a reason to acquire another grant.
            return Ok(Progress::Stopped(
                "Tower situation was refused; enumeration incomplete; no retry or new capture",
            ));
        }
        let value = result.get("value").ok_or(INVALID)?;
        let has_error = value.get("error").is_some();
        let fields = if has_error {
            [
                "schema",
                "operation",
                "historical",
                "current_eligibility",
                "error",
            ]
        } else {
            [
                "schema",
                "operation",
                "historical",
                "current_eligibility",
                "knowledge",
            ]
        };
        let wrapper = object(value, &fields)?;
        let schema = if self.link_registration.is_some() {
            "fa.publication-situation/2"
        } else {
            "fa.publication-situation/1"
        };
        if wrapper["schema"].as_str() != Some(schema)
            || wrapper["operation"].as_str() != Some("situation")
            || wrapper["historical"].as_bool() != Some(true)
            || wrapper["current_eligibility"].as_str() != Some("not_observed")
        {
            return Err(INVALID.into());
        }
        if has_error {
            let error = object(&wrapper["error"], &["code"])?;
            if envelope.get("status").and_then(Json::as_str) != Some("error")
                || !matches!(
                    error["code"].as_str(),
                    Some("capacity" | "unavailable" | "idempotency_conflict")
                )
            {
                return Err(INVALID.into());
            }
            return Ok(Progress::Stopped(
                "Tower situation returned a native error; enumeration incomplete; no retry or new capture",
            ));
        }
        let knowledge = &wrapper["knowledge"];
        match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                if envelope.get("status").and_then(Json::as_str) != Some("ok")
                    || envelope.get("reason_code") != Some(&Json::Null)
                {
                    return Err(INVALID.into());
                }
                self.known(knowledge)
            }
            Some("unknown") => {
                let knowledge = object(knowledge, &["state", "reason"])?;
                if knowledge["reason"].as_str() != Some("controller_unavailable")
                    || envelope.get("status").and_then(Json::as_str) != Some("error")
                {
                    return Err(INVALID.into());
                }
                Ok(Progress::Stopped(
                    "Tower situation is Unknown; enumeration incomplete; no retry or new capture",
                ))
            }
            Some("withheld") => {
                let knowledge = object(knowledge, &["state", "authority_required"])?;
                if knowledge["authority_required"].as_str() != Some("supervisor_inspection")
                    || envelope.get("status").and_then(Json::as_str) != Some("error")
                {
                    return Err(INVALID.into());
                }
                Ok(Progress::Stopped(
                    "Tower situation is Withheld; enumeration incomplete; no retry or new capture",
                ))
            }
            _ => Err(INVALID.into()),
        }
    }

    fn known(&mut self, knowledge: &Json) -> Result<Progress, String> {
        let knowledge = object(knowledge, &["state", "basis", "value"])?;
        let basis = &knowledge["basis"];
        self.check_basis(basis)?;
        let page = object(
            &knowledge["value"],
            &[
                "summary",
                "offset",
                "continuation",
                "enumeration_complete",
                "attempts",
            ],
        )?;
        let summary = &page["summary"];
        let total = check_summary(summary)?;
        if self
            .basis
            .as_ref()
            .is_some_and(|original| original != basis)
            || self
                .summary
                .as_ref()
                .is_some_and(|original| original != summary)
        {
            return Err(INVALID.into());
        }
        let offset = number(&page["offset"])?;
        let rows = page["attempts"].as_array().ok_or(INVALID)?;
        let end = offset.checked_add(rows.len() as u64).ok_or(INVALID)?;
        let complete = page["enumeration_complete"].as_bool().ok_or(INVALID)?;
        let cursor = optional_number(&page["continuation"])?;
        if offset != self.next_offset
            || rows.len() > MAX_INSPECTION_ATTEMPTS
            || end > total
            || complete != (end == total)
            || (complete && cursor.is_some())
            || (!complete && rows.len() != MAX_INSPECTION_ATTEMPTS)
            || self.attempts.len().saturating_add(rows.len()) > self.max_rows
            || cursor.is_some_and(|cursor| cursor == 0 || self.cursors.contains(&cursor))
            || (cursor.is_some() && self.cursors.len() == MAX_PAGES)
        {
            return Err(INVALID.into());
        }
        let mut new_attempts = BTreeSet::new();
        let mut new_requests = BTreeSet::new();
        for row in rows {
            let (attempt, request) = check_attempt(row, self.scope, self.link_registration.as_ref())?;
            if self.attempts.contains(&attempt) || !new_attempts.insert(attempt) {
                return Err(INVALID.into());
            }
            if let Some(request) = request {
                if self.requests.contains(&request) || !new_requests.insert(request) {
                    return Err(INVALID.into());
                }
            }
        }
        if self.requests.len().saturating_add(new_requests.len()) > MAX_ACTOR_REQUESTS {
            return Err(INVALID.into());
        }
        // Retain only one small basis/summary and bounded IDs, never the whole
        // ledger or accumulated response bytes. Large totals do not enlarge the
        // caller's work allowance and are not rejected merely for being large.
        if self.basis.is_none() {
            self.basis = Some(basis.clone());
            self.summary = Some(summary.clone());
        }
        self.attempts.extend(new_attempts);
        self.requests.extend(new_requests);
        self.next_offset = end;
        if complete {
            Ok(Progress::Complete)
        } else if let Some(cursor) = cursor {
            self.cursors.insert(cursor);
            Ok(Progress::Continue(cursor))
        } else {
            Ok(Progress::Stopped(
                "Tower situation enumeration is incomplete: native continuation unavailable or grant exhausted; partial pages retained; no new capture",
            ))
        }
    }

    fn check_basis(&self, basis: &Json) -> Result<(), String> {
        let basis = object(
            basis,
            &[
                "scope",
                "journal_revision",
                "control_sequence",
                "recorded_at",
                "origin",
                "anchored_through",
            ],
        )?;
        let scope = object(
            &basis["scope"],
            &[
                "tenant",
                "principal",
                "run",
                "branch",
                "authority",
                "purpose",
            ],
        )?;
        for (name, expected) in [
            ("tenant", self.scope.tenant),
            ("principal", self.scope.principal),
            ("run", self.scope.run),
            ("branch", self.scope.branch),
            ("authority", self.scope.authority),
        ] {
            if nonzero(&scope[name])? != expected {
                return Err(INVALID.into());
            }
        }
        if self.scope.purpose != Purpose::Effect
            || scope["purpose"].as_str() != Some("effect")
            || basis["origin"].as_str() != Some("acknowledged_owner")
            || !basis["anchored_through"].is_null()
        {
            return Err(INVALID.into());
        }
        number(&basis["journal_revision"])?;
        number(&basis["control_sequence"])?;
        optional_number(&basis["recorded_at"])?;
        Ok(())
    }
}

fn check_summary(summary: &Json) -> Result<u64, String> {
    let summary = object(
        summary,
        &[
            "total_units",
            "available_units",
            "reserved_units",
            "charged_units",
            "policy_epoch",
            "dispatcher_epoch",
            "suspended",
            "stop_recorded",
            "executions",
            "attempts",
            "retained_requests",
            "pending_attempts",
            "unresolved_attempts",
            "irrecoverably_unknown_attempts",
            "terminal_attempts",
        ],
    )?;
    for (name, value) in summary {
        if matches!(name.as_str(), "suspended" | "stop_recorded") {
            value.as_bool().ok_or(INVALID)?;
        } else {
            number(value)?;
        }
    }
    let sum = |names: &[&str]| -> Result<u64, String> {
        names.iter().try_fold(0_u64, |total, name| {
            total
                .checked_add(number(&summary[*name])?)
                .ok_or_else(|| INVALID.into())
        })
    };
    let attempts = number(&summary["attempts"])?;
    if sum(&["available_units", "reserved_units", "charged_units"])?
        != number(&summary["total_units"])?
        || sum(&[
            "pending_attempts",
            "unresolved_attempts",
            "terminal_attempts",
        ])? != attempts
        || number(&summary["irrecoverably_unknown_attempts"])?
            > number(&summary["unresolved_attempts"])?
    {
        return Err(INVALID.into());
    }
    Ok(attempts)
}

fn check_attempt(
    row: &Json,
    scope: Scope,
    registration: Option<&Address>,
) -> Result<(u64, Option<u64>), String> {
    const FIELDS: [&str; 10] = [
        "address",
        "attempt",
        "stage",
        "target",
        "units",
        "payload_bytes",
        "proposal_policy_epoch",
        "deadline",
        "external_request",
        "request_address",
    ];
    let row = object(row, &FIELDS[..if registration.is_some() { 10 } else { 8 }])?;
    let attempt = nonzero(&row["attempt"])?;
    if row["address"].as_str() != Some(format!("fa://{}/attempt/{attempt}", scope.tenant).as_str())
        || !matches!(
            row["stage"].as_str(),
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
        return Err(INVALID.into());
    }
    let target = object(
        &row["target"],
        &[
            "adapter",
            "object",
            "contract_version",
            "expected_version",
            "generation",
        ],
    )?;
    for value in target.values() {
        nonzero(value)?;
    }
    nonzero(&row["units"])?;
    number(&row["payload_bytes"])?;
    number(&row["proposal_policy_epoch"])?;
    nonzero(&row["deadline"])?;
    let request = match registration {
        None => None,
        Some(registration) => match (
            row["external_request"].is_null(),
            row["request_address"].is_null(),
        ) {
            (true, true) => None,
            (false, false) => {
                let request = nonzero(&row["external_request"])?;
                let address = request_address_for(scope, registration, request)
                    .map_err(|_| INVALID.to_owned())?;
                if row["request_address"].as_str() != Some(address.canonical_string().as_str()) {
                    return Err(INVALID.into());
                }
                Some(request)
            }
            _ => return Err(INVALID.into()),
        },
    };
    Ok((attempt, request))
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let object = value.as_object().ok_or(INVALID)?;
    if object.len() != fields.len() || fields.iter().any(|name| !object.contains_key(*name)) {
        return Err(INVALID.into());
    }
    Ok(object)
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

fn number(value: &Json) -> Result<u64, String> {
    value
        .as_str()
        .and_then(decimal)
        .ok_or_else(|| INVALID.into())
}

fn nonzero(value: &Json) -> Result<u64, String> {
    let number = number(value)?;
    if number == 0 {
        return Err(INVALID.into());
    }
    Ok(number)
}

fn optional_number(value: &Json) -> Result<Option<u64>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        number(value).map(Some)
    }
}

#[cfg(test)]
#[path = "situation/tests.rs"]
mod tests;
