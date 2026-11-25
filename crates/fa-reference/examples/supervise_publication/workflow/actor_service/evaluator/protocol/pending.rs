//! Discover bounded pages of original pending assessments without guessing round
//! identities. Continuations pin the applied-case relation's evaluation revision;
//! an empty suffix is complete only for that original owner and selected cut.
use super::{Basis, RESPONSE_BYTES, Settings, audience, debug, decimal, expected, fields, parse};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::{
        FileOversight,
        credibility::{
            FileIndependentEvaluator, MAX_PENDING_EVALUATION_PAGE, PendingEvaluationStatus,
        },
    },
};
use fa_reference::action::consequence::oversight::credibility::MAX_EVALUATION_CASES;
use fa_reference::strict_json::{self, Json, Limits};

// Echoing a maximum-size request must leave room for sixteen full-width case
// identities within the unchanged response budget, even with JSON whitespace.
const PENDING_REQUEST_BYTES: usize = 2 * 1024;

pub(crate) struct PendingRequest {
    pub(crate) wire: Vec<u8>,
    pub(crate) parsed: Json,
    pub(crate) continuation: Option<(u64, u64)>,
}

impl PendingRequest {
    pub(crate) fn prepare(
        settings: &Settings,
        continuation: Option<(u64, u64)>,
    ) -> Result<Self, String> {
        let continuation = continuation.map_or_else(
            || "null".to_owned(),
            |(revision, round)| {
                format!("{{\"evaluation_revision\":\"{revision}\",\"after_round\":\"{round}\"}}")
            },
        );
        let wire = format!(
            "{{\"schema_version\":\"fa.native-evaluator-pending-request/1\",\"audience\":{},\"continuation\":{continuation}}}",
            audience(settings)
        );
        Self::accept(settings, wire.as_bytes())
    }

    pub(crate) fn accept(settings: &Settings, wire: &[u8]) -> Result<Self, String> {
        let parsed = parse(wire, PENDING_REQUEST_BYTES)?;
        let root = fields(&parsed, &["schema_version", "audience", "continuation"])?;
        expected(
            root,
            "schema_version",
            "fa.native-evaluator-pending-request/1",
        )?;
        if root["audience"] != parse(audience(settings).as_bytes(), PENDING_REQUEST_BYTES)? {
            return Err(
                "pending-case audience does not match this original evaluator owner".into(),
            );
        }
        let continuation = if root["continuation"].is_null() {
            None
        } else {
            let cursor = fields(
                &root["continuation"],
                &["evaluation_revision", "after_round"],
            )?;
            Some((
                super::case::positive(&cursor["evaluation_revision"])?,
                super::case::positive(&cursor["after_round"])?,
            ))
        };
        Ok(Self {
            wire: wire.to_vec(),
            parsed,
            continuation,
        })
    }

    pub(crate) fn observe(
        &self,
        host: &FileOversight,
        evaluator: &FileIndependentEvaluator,
    ) -> Result<Vec<u8>, String> {
        // A foreign/recovered-old/faulted owner must not be mistaken for a
        // merely stale cursor. Check custody before disclosing any native cut.
        evaluator.check_owner(host).map_err(debug)?;
        let page = match evaluator.pending_evaluation_cases(host, self.continuation) {
            Ok(page) => page,
            Err(JournalError::Contract(Error::Stale)) if self.continuation.is_some() => {
                return self.unknown("evaluation_revision_changed");
            }
            Err(JournalError::Contract(Error::Binding)) if self.continuation.is_some() => {
                return self.unknown("invalid_continuation");
            }
            Err(error) => return Err(debug(error)),
        };
        let basis = Basis::observe(host)?;
        if basis.evaluation != page.evaluation_revision() {
            return Err("pending cases and native basis differ".into());
        }
        let mut cases = Vec::with_capacity(page.cases().len());
        for case in page.cases() {
            let root: String = case
                .evidence_root()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let status = match case.status() {
                PendingEvaluationStatus::Unassessed => "unassessed",
                PendingEvaluationStatus::Censored => "censored",
            };
            cases.push(format!(
                "{{\"round\":\"{}\",\"attempt\":\"{}\",\"policy_generation\":\"{}\",\"congress_generation\":\"{}\",\"case_evidence_root_hex\":\"{root}\",\"status\":\"{status}\"}}",
                case.round(), case.attempt(), case.policy_generation(), case.congress_generation(),
            ));
        }
        let after = optional_round(page.after_round());
        let next = optional_round(page.next_after_round());
        self.response(&format!(
            "{{\"state\":\"known\",\"basis\":{},\"value\":{{\"after_round\":{after},\"total_pending\":\"{}\",\"unfinished_reviews\":\"{}\",\"cases\":[{}],\"next_after_round\":{next},\"complete\":{}}}}}",
            basis.json(), page.total_pending(), page.unfinished_reviews(), cases.join(","),
            page.next_after_round().is_none(),
        ))
    }

    fn unknown(&self, reason: &str) -> Result<Vec<u8>, String> {
        self.response(&format!(
            "{{\"state\":\"unknown\",\"reason\":\"{reason}\"}}"
        ))
    }

    fn response(&self, knowledge: &str) -> Result<Vec<u8>, String> {
        let wire = format!(
            "{{\"schema_version\":\"fa.live-evaluator-pending/1\",\"request\":{},\"knowledge\":{knowledge},\"effect_authority\":false}}",
            std::str::from_utf8(&self.wire).map_err(debug)?
        )
        .into_bytes();
        validate(self, &wire)?;
        Ok(wire)
    }
}

fn optional_round(round: Option<u64>) -> String {
    round.map_or_else(|| "null".to_owned(), |round| format!("\"{round}\""))
}

/// A page has at most sixteen minimal identities. Its dedicated item limit
/// does not enlarge the older exact-case or assessment parsers or byte budgets.
pub(crate) fn parse_response(wire: &[u8]) -> Result<Json, String> {
    strict_json::parse(
        wire,
        Limits {
            max_bytes: RESPONSE_BYTES,
            max_depth: 6,
            max_items: 512,
            max_string_bytes: 512,
        },
    )
    .map_err(debug)
}

/// Validate one declared, authenticated page and its explicit continuation.
/// Successful discovery supplies neither a ticket nor a reserved predecessor.
pub(crate) fn validate(request: &PendingRequest, wire: &[u8]) -> Result<bool, String> {
    let parsed = parse_response(wire)?;
    let root = fields(
        &parsed,
        &["schema_version", "request", "knowledge", "effect_authority"],
    )?;
    expected(root, "schema_version", "fa.live-evaluator-pending/1")?;
    if root["request"] != request.parsed {
        return Err("pending-case response does not bind the submitted evaluator request".into());
    }
    if root["effect_authority"].as_bool() != Some(false) {
        return Err("pending-case observations cannot carry effect authority".into());
    }
    let knowledge = root["knowledge"]
        .as_object()
        .ok_or("pending-case response requires explicit knowledge")?;
    match knowledge.get("state").and_then(Json::as_str) {
        Some("known") => {
            let known = fields(&root["knowledge"], &["state", "basis", "value"])?;
            let basis = Basis::decode(&known["basis"])?;
            if basis.journal == 0
                || request
                    .continuation
                    .is_some_and(|(revision, _)| revision != basis.evaluation)
            {
                return Err(
                    "pending-case page does not retain its original acknowledged cut".into(),
                );
            }
            let value = fields(
                &known["value"],
                &[
                    "after_round",
                    "total_pending",
                    "unfinished_reviews",
                    "cases",
                    "next_after_round",
                    "complete",
                ],
            )?;
            let after = decode_optional_round(&value["after_round"])?;
            if after != request.continuation.map(|(_, round)| round) {
                return Err("pending-case page changed its requested lower bound".into());
            }
            let total = decimal(&value["total_pending"])?;
            let unfinished = decimal(&value["unfinished_reviews"])?;
            if total > MAX_EVALUATION_CASES as u64
                || unfinished > MAX_EVALUATION_CASES as u64
                || total + unfinished > MAX_EVALUATION_CASES as u64
            {
                return Err("pending-case counts exceed the original native inventory".into());
            }
            let cases = value["cases"]
                .as_array()
                .ok_or("pending cases must be an ordered bounded array")?;
            if cases.len() > MAX_PENDING_EVALUATION_PAGE
                || cases.len() as u64 + u64::from(after.is_some()) > total
                || (total != 0 && basis.evaluation == 0)
            {
                return Err("pending-case page contradicts its original inventory".into());
            }
            let mut previous = after.unwrap_or(0);
            for case in cases {
                let case = fields(
                    case,
                    &[
                        "round",
                        "attempt",
                        "policy_generation",
                        "congress_generation",
                        "case_evidence_root_hex",
                        "status",
                    ],
                )?;
                let round = super::case::positive(&case["round"])?;
                if round <= previous {
                    return Err(
                        "pending-case identities must be strictly ordered after the cursor".into(),
                    );
                }
                previous = round;
                for field in ["attempt", "policy_generation", "congress_generation"] {
                    super::case::positive(&case[field])?;
                }
                super::case::digest(&case["case_evidence_root_hex"])?;
                if !matches!(case["status"].as_str(), Some("unassessed" | "censored")) {
                    return Err("a pending case cannot claim a final ground-truth label".into());
                }
            }
            let next = decode_optional_round(&value["next_after_round"])?;
            if value["complete"].as_bool() != Some(next.is_none()) {
                return Err("pending-case completeness contradicts its continuation".into());
            }
            if let Some(next) = next {
                if cases.len() != MAX_PENDING_EVALUATION_PAGE
                    || next != previous
                    || total <= cases.len() as u64 + u64::from(after.is_some())
                    || next == u64::MAX
                {
                    return Err("pending-case continuation skips or invents a boundary".into());
                }
            }
            if after.is_none()
                && (cases.len() != (total as usize).min(MAX_PENDING_EVALUATION_PAGE)
                    || next.is_some() != (total as usize > MAX_PENDING_EVALUATION_PAGE))
            {
                return Err(
                    "initial pending-case page silently omits part of its inventory".into(),
                );
            }
            Ok(true)
        }
        Some("unknown") => {
            let unknown = fields(&root["knowledge"], &["state", "reason"])?;
            if request.continuation.is_none()
                || !matches!(
                    unknown["reason"].as_str(),
                    Some("evaluation_revision_changed" | "invalid_continuation")
                )
            {
                return Err("unsupported pending-case cursor observation".into());
            }
            Ok(false)
        }
        _ => Err("unsupported pending-case knowledge state".into()),
    }
}

fn decode_optional_round(value: &Json) -> Result<Option<u64>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        super::case::positive(value).map(Some)
    }
}

#[cfg(test)]
#[path = "../tests/pending_protocol.rs"]
mod tests;
