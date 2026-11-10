//! Follow one finite original journal capture. Revisions belong to the native
//! journal, not the Tower control ledger. Returned rows are historical metadata;
//! neither their names nor an empty suffix prove an effect outcome or authority.

use super::{
    Budget, Endpoint, Instant, Json, OperatorSettings, Path, ReadArguments, TowerRequest, Verb,
    Write, connect_endpoint, emit_response, parse_response, request_frame,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileJournalEventKind, MAX_JOURNAL_TAIL_CAPTURE_EVENTS, MAX_JOURNAL_TAIL_PAGE_EVENTS,
    MAX_JOURNAL_TAIL_QUERIES,
};
use fa_reference::action::{Purpose, Scope};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::tower::MAX_REQUEST_STRING_BYTES;
use std::collections::{BTreeMap, BTreeSet};

const USAGE: &str = "tower-tail OPERATOR_PROFILE RUN_ADDRESS --after-revision N --max-pages N (canonical original journal revision; pages 1..1024; one capture, no reconnect or retry)";
const INVALID: &str = "invalid or inconsistent native journal tail page; no continuation or new capture was requested";

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let (address, after_revision, max_pages) = selection(args)?;
    let started = Instant::now();
    let settings = OperatorSettings::read(Path::new(&args[1]))?;
    if address.tenant != settings.scope.tenant.to_string()
        || address.id != settings.scope.run.to_string()
    {
        return Err("Tower tail address does not match the operator profile scope; no connection was opened".into());
    }
    let selected = settings.journal_tail.as_ref().ok_or(
        "Tower tail requires explicit journal_tail disclosure and bounds; no connection was opened",
    )?;
    let endpoint = Endpoint::operator(&settings);
    let page_limit = max_pages
        .min(selected.queries)
        .min(endpoint.exchanges as usize);
    let mut capture = Capture::new(settings.scope, after_revision, selected.events_per_page);
    let mut budget = Budget::new(&endpoint, started);
    let mut socket = connect_endpoint(endpoint, &budget, || settings.check_directory())?;
    let mut cursor = None;
    for page in 1..=page_limit {
        let mut request = TowerRequest::new(
            format!("tower-tail-page-{page}"),
            address.clone(),
            Verb::Tail,
        );
        request.arguments = cursor
            .map_or(ReadArguments::JournalTail { after_revision }, |cursor| {
                ReadArguments::JournalTailPage { cursor }
            });
        budget.write_frame(&mut socket, &request_frame(&request)?)?;
        let response = budget.read_frame(&mut socket)?;
        let parsed = parse_response(&response, &request)?;
        let progress = capture.accept(&parsed, &request)?;
        // A complete original response reaches the caller before its cursor
        // can spend another query. Output loss never triggers another request.
        emit_response(&response, output, &budget)?;
        match progress {
            Progress::Complete => return Ok(()),
            Progress::Continue(next) => cursor = Some(next),
            Progress::Stopped(reason) => return Err(reason.into()),
        }
    }
    Err("Tower journal tail is incomplete: page or original exchange allowance exhausted; partial pages retained; no reconnect, retry or new capture".into())
}

fn selection(args: &[String]) -> Result<(Address, u64, usize), String> {
    if args.len() != 7
        || args[0] != "tower-tail"
        || args[1].is_empty()
        || args[1].starts_with("--")
        || args[2].len() > MAX_REQUEST_STRING_BYTES
        || args[3] != "--after-revision"
        || args[5] != "--max-pages"
    {
        return Err(USAGE.into());
    }
    let address = Address::parse(&args[2]).map_err(|_| USAGE.to_owned())?;
    let after_revision = decimal(&args[4]).ok_or_else(|| USAGE.to_owned())?;
    let max_pages = decimal(&args[6]).ok_or_else(|| USAGE.to_owned())?;
    if address.kind != Noun::Run
        || address.generation.is_none_or(|generation| generation == 0)
        || address.canonical_string() != args[2]
        || decimal(&address.tenant).is_none_or(|tenant| tenant == 0)
        || decimal(&address.id).is_none_or(|run| run == 0)
        || !(1..=MAX_JOURNAL_TAIL_QUERIES as u64).contains(&max_pages)
    {
        return Err(USAGE.into());
    }
    Ok((address, after_revision, max_pages as usize))
}

pub(super) fn matches(response: &Json, request: &TowerRequest) -> bool {
    observe(response, request).is_ok()
}

enum Progress {
    Complete,
    Continue(u64),
    Stopped(&'static str),
}

struct Capture {
    scope: Scope,
    after_revision: u64,
    events_per_page: usize,
    basis: Option<Json>,
    next_offset: u64,
    cursors: BTreeSet<u64>,
}

impl Capture {
    fn new(scope: Scope, after_revision: u64, events_per_page: usize) -> Self {
        Self {
            scope,
            after_revision,
            events_per_page,
            basis: None,
            next_offset: 0,
            cursors: BTreeSet::new(),
        }
    }

    fn accept(&mut self, response: &Json, request: &TowerRequest) -> Result<Progress, String> {
        let page = match observe(response, request)? {
            Observation::Page(page) => page,
            Observation::Stopped(reason) => return Ok(Progress::Stopped(reason)),
        };
        if page.scope != self.scope
            || page.after_revision != self.after_revision
            || page.offset != self.next_offset
            || page.events > self.events_per_page
            || (!page.complete && page.events != self.events_per_page)
            || self.basis.as_ref().is_some_and(|basis| basis != page.basis)
            || page
                .cursor
                .is_some_and(|cursor| self.cursors.contains(&cursor))
            || (page.cursor.is_some() && self.cursors.len() == MAX_JOURNAL_TAIL_QUERIES)
        {
            return Err(INVALID.into());
        }
        if self.basis.is_none() {
            self.basis = Some(page.basis.clone());
        }
        self.next_offset = page.offset + page.events as u64;
        if page.complete {
            Ok(Progress::Complete)
        } else if let Some(cursor) = page.cursor {
            self.cursors.insert(cursor);
            Ok(Progress::Continue(cursor))
        } else {
            Ok(Progress::Stopped(
                "Tower journal tail is incomplete: native continuation unavailable or grant exhausted; partial pages retained; no new capture",
            ))
        }
    }
}

struct Page<'a> {
    scope: Scope,
    basis: &'a Json,
    after_revision: u64,
    offset: u64,
    events: usize,
    complete: bool,
    cursor: Option<u64>,
}

enum Observation<'a> {
    Page(Page<'a>),
    Stopped(&'static str),
}

fn observe<'a>(response: &'a Json, request: &TowerRequest) -> Result<Observation<'a>, String> {
    if request.verb != Verb::Tail
        || !matches!(
            request.arguments,
            ReadArguments::JournalTail { .. } | ReadArguments::JournalTailPage { .. }
        )
    {
        return Err(INVALID.into());
    }
    let envelope = response.get("envelope").ok_or(INVALID)?;
    let result = response.get("result").ok_or(INVALID)?;
    if result.get("kind").and_then(Json::as_str) == Some("refused") {
        if envelope.get("status").and_then(Json::as_str) != Some("error") {
            return Err(INVALID.into());
        }
        return Ok(Observation::Stopped(
            "Tower journal tail was refused; enumeration incomplete; no retry or new capture",
        ));
    }
    if result.get("kind").and_then(Json::as_str) != Some("journal-tail") {
        return Err(INVALID.into());
    }
    let value = result.get("value").ok_or(INVALID)?;
    let has_error = value.get("error").is_some();
    let mut fields = vec![
        "schema",
        "operation",
        "registration",
        "query_address",
        "historical",
        "current_eligibility",
        "disclosure",
    ];
    fields.push(if has_error { "error" } else { "knowledge" });
    let wrapper = object(value, &fields)?;
    let address = request.address.canonical_string();
    let encoded_registration = wrapper["registration"].as_str().ok_or(INVALID)?;
    let registration = Address::parse(encoded_registration).map_err(|_| INVALID.to_owned())?;
    if request.address.kind != Noun::Run
        || registration.canonical_string() != encoded_registration
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || registration.without_generation() != request.address.without_generation()
        || request
            .address
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
        || wrapper["schema"].as_str() != Some("fa.publication-journal-tail/1")
        || wrapper["operation"].as_str() != Some("tail")
        || wrapper["query_address"].as_str() != Some(address.as_str())
        || wrapper["historical"].as_bool() != Some(true)
        || wrapper["current_eligibility"].as_str() != Some("not_observed")
        || wrapper["disclosure"].as_str() != Some("redacted_journal_events")
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
        return Ok(Observation::Stopped(
            "Tower journal tail returned a native error; enumeration incomplete; no retry or new capture",
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
            known(knowledge, request).map(Observation::Page)
        }
        Some("unknown") => {
            let knowledge = object(knowledge, &["state", "reason"])?;
            if knowledge["reason"].as_str() != Some("controller_unavailable")
                || envelope.get("status").and_then(Json::as_str) != Some("error")
            {
                return Err(INVALID.into());
            }
            Ok(Observation::Stopped(
                "Tower journal tail is Unknown; enumeration incomplete; no retry or new capture",
            ))
        }
        Some("withheld") => {
            let knowledge = object(knowledge, &["state", "authority_required"])?;
            if knowledge["authority_required"].as_str() != Some("supervisor_journal_tail")
                || envelope.get("status").and_then(Json::as_str) != Some("error")
            {
                return Err(INVALID.into());
            }
            Ok(Observation::Stopped(
                "Tower journal tail is Withheld; enumeration incomplete; no retry or new capture",
            ))
        }
        _ => Err(INVALID.into()),
    }
}

fn known<'a>(knowledge: &'a Json, request: &TowerRequest) -> Result<Page<'a>, String> {
    let knowledge = object(knowledge, &["state", "basis", "value"])?;
    let original_basis = &knowledge["basis"];
    let basis = object(
        original_basis,
        &[
            "scope",
            "journal_revision",
            "control_sequence",
            "recorded_at",
            "origin",
            "anchored_through",
        ],
    )?;
    let encoded_scope = object(
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
    let scope = Scope {
        tenant: nonzero(&encoded_scope["tenant"])?,
        principal: nonzero(&encoded_scope["principal"])?,
        run: nonzero(&encoded_scope["run"])?,
        branch: nonzero(&encoded_scope["branch"])?,
        authority: nonzero(&encoded_scope["authority"])?,
        purpose: Purpose::Effect,
    };
    if encoded_scope["purpose"].as_str() != Some("effect")
        || request.address.tenant != scope.tenant.to_string()
        || request.address.id != scope.run.to_string()
        || basis["origin"].as_str() != Some("acknowledged_owner")
        || !basis["anchored_through"].is_null()
    {
        return Err(INVALID.into());
    }
    let end = number(&basis["journal_revision"])?;
    number(&basis["control_sequence"])?;
    optional_number(&basis["recorded_at"])?;
    let page = object(
        &knowledge["value"],
        &[
            "after_revision",
            "offset",
            "through_revision",
            "enumeration_complete",
            "continuation",
            "events",
        ],
    )?;
    let after_revision = number(&page["after_revision"])?;
    let offset = number(&page["offset"])?;
    let through_revision = number(&page["through_revision"])?;
    let complete = page["enumeration_complete"].as_bool().ok_or(INVALID)?;
    let cursor = optional_number(&page["continuation"])?;
    let rows = page["events"].as_array().ok_or(INVALID)?;
    let first = after_revision.checked_add(offset).ok_or(INVALID)?;
    if rows.len() > MAX_JOURNAL_TAIL_PAGE_EVENTS
        || offset
            .checked_add(rows.len() as u64)
            .is_none_or(|total| total > MAX_JOURNAL_TAIL_CAPTURE_EVENTS as u64)
        || end
            .checked_sub(after_revision)
            .is_none_or(|total| total > MAX_JOURNAL_TAIL_CAPTURE_EVENTS as u64)
        || first.checked_add(rows.len() as u64) != Some(through_revision)
        || after_revision > end
        || through_revision > end
        || complete != (through_revision == end)
        || (rows.is_empty() && (!complete || offset != 0))
        || (complete && cursor.is_some())
        || cursor == Some(0)
    {
        return Err(INVALID.into());
    }
    match request.arguments {
        ReadArguments::JournalTail {
            after_revision: selected,
        } if selected == after_revision && offset == 0 => {}
        ReadArguments::JournalTailPage { cursor: selected }
            if selected != 0 && offset != 0 && cursor != Some(selected) => {}
        _ => return Err(INVALID.into()),
    }
    for (index, row) in rows.iter().enumerate() {
        let row = object(row, &["revision", "kind", "attempt", "request", "round"])?;
        if number(&row["revision"])? != first + index as u64 + 1
            || row["kind"]
                .as_str()
                .and_then(FileJournalEventKind::from_label)
                .is_none()
        {
            return Err(INVALID.into());
        }
        for name in ["attempt", "request", "round"] {
            if optional_number(&row[name])? == Some(0) {
                return Err(INVALID.into());
            }
        }
    }
    Ok(Page {
        scope,
        basis: original_basis,
        after_revision,
        offset,
        events: rows.len(),
        complete,
        cursor,
    })
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let value = value.as_object().ok_or(INVALID)?;
    if value.len() != fields.len() || fields.iter().any(|field| !value.contains_key(*field)) {
        return Err(INVALID.into());
    }
    Ok(value)
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
    let value = number(value)?;
    if value == 0 {
        Err(INVALID.into())
    } else {
        Ok(value)
    }
}

fn optional_number(value: &Json) -> Result<Option<u64>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        number(value).map(Some)
    }
}

#[cfg(test)]
#[path = "journal_tail/tests.rs"]
mod tests;
