//! Bounded discovery of original applied cases on one acknowledged history cut.
//! A continuation pins both revisions; it never silently resumes a changed ledger.
use super::{RESPONSE_BYTES, Settings, audience, debug, decimal, expected, fields};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::{
        FileOversight,
        credibility::{
            FileEvaluationCaseStatus, FileEvaluationCasesBasis, FileIndependentEvaluator,
            MAX_EVALUATION_CASE_PAGE,
        },
    },
};
use fa_reference::action::consequence::oversight::credibility::MAX_EVALUATION_CASES;
use fa_reference::strict_json::{self, Json, Limits};

// The response echoes the complete submitted frame. This tighter operation
// bound leaves room for sixteen descriptors within the original response cap,
// including a request padded with otherwise legal JSON whitespace.
const CASES_REQUEST_BYTES: usize = 2 * 1024;

pub(crate) struct CasesRequest {
    pub(crate) wire: Vec<u8>,
    pub(crate) parsed: Json,
    pub(crate) limit: usize,
    pub(crate) after_round: Option<u64>,
    pub(crate) expected_basis: Option<FileEvaluationCasesBasis>,
}

impl CasesRequest {
    pub(crate) fn prepare(
        settings: &Settings,
        limit: usize,
        after_round: Option<u64>,
        expected_basis: Option<FileEvaluationCasesBasis>,
    ) -> Result<Self, String> {
        let expected_basis = expected_basis.map_or_else(|| "null".to_owned(), basis_json);
        let after_round = optional_round_json(after_round);
        let wire = format!(
            "{{\"schema_version\":\"fa.native-evaluator-cases-request/1\",\"audience\":{},\"limit\":\"{limit}\",\"after_round\":{after_round},\"expected_basis\":{expected_basis}}}",
            audience(settings)
        );
        Self::accept(settings, wire.as_bytes())
    }

    pub(crate) fn accept(settings: &Settings, wire: &[u8]) -> Result<Self, String> {
        let parsed = parse(wire, CASES_REQUEST_BYTES)?;
        let root = fields(
            &parsed,
            &[
                "schema_version",
                "audience",
                "limit",
                "after_round",
                "expected_basis",
            ],
        )?;
        expected(
            root,
            "schema_version",
            "fa.native-evaluator-cases-request/1",
        )?;
        if root["audience"] != parse(audience(settings).as_bytes(), CASES_REQUEST_BYTES)? {
            return Err(
                "case listing audience does not match this original evaluator owner".into(),
            );
        }
        let limit = decimal(&root["limit"])?;
        if limit == 0 || limit > MAX_EVALUATION_CASE_PAGE as u64 {
            return Err("case listing limit is outside the finite page bound".into());
        }
        let after_round = optional_round(&root["after_round"])?;
        let expected_basis = if root["expected_basis"].is_null() {
            None
        } else {
            Some(basis(&root["expected_basis"])?)
        };
        if after_round.is_some() != expected_basis.is_some() {
            return Err(
                "a case listing continuation requires its round and both original revisions".into(),
            );
        }
        Ok(Self {
            wire: wire.to_vec(),
            parsed,
            limit: limit as usize,
            after_round,
            expected_basis,
        })
    }

    pub(crate) fn observe(
        &self,
        host: &FileOversight,
        evaluator: &FileIndependentEvaluator,
    ) -> Result<Vec<u8>, String> {
        // Native custody and the full acknowledged cut are checked before any
        // case/count is copied. Binding/storage failures are never empty pages.
        match evaluator.evaluation_cases(host, self.after_round, self.expected_basis, self.limit) {
            Ok(page) => {
                let value = Page {
                    total_cases: page.total_cases(),
                    remaining_cases: page.remaining_cases(),
                    unapplied_rounds: page.unapplied_rounds(),
                    cases: page
                        .cases()
                        .iter()
                        .map(|case| Case {
                            round: case.round(),
                            attempt: case.attempt(),
                            policy: case.policy_generation(),
                            congress: case.congress_generation(),
                            root: case.evidence_root(),
                            status: case.status(),
                        })
                        .collect(),
                    next_after: page.next_after(),
                };
                self.encode_page(page.basis(), &value)
            }
            Err(JournalError::Contract(Error::Stale)) => self.encode_unknown("stale_basis"),
            Err(JournalError::Contract(Error::Missing)) => {
                self.encode_unknown("cursor_not_recorded")
            }
            Err(error) => Err(debug(error)),
        }
    }

    fn encode_page(&self, basis: FileEvaluationCasesBasis, page: &Page) -> Result<Vec<u8>, String> {
        let cases = page.cases.iter().map(|case| {
            let root: String = case.root.iter().map(|byte| format!("{byte:02x}")).collect();
            let status = match case.status {
                FileEvaluationCaseStatus::Pending => "pending",
                FileEvaluationCaseStatus::Censored => "censored",
                FileEvaluationCaseStatus::Resolved => "resolved",
            };
            format!(
                "{{\"round\":\"{}\",\"attempt\":\"{}\",\"policy_generation\":\"{}\",\"congress_generation\":\"{}\",\"case_evidence_root_hex\":\"{root}\",\"status\":\"{status}\"}}",
                case.round, case.attempt, case.policy, case.congress,
            )
        }).collect::<Vec<_>>().join(",");
        let knowledge = format!(
            "{{\"state\":\"known\",\"basis\":{},\"value\":{{\"total_cases\":\"{}\",\"remaining_cases\":\"{}\",\"unapplied_rounds\":\"{}\",\"cases\":[{cases}],\"next_after\":{}}}}}",
            basis_json(basis),
            page.total_cases,
            page.remaining_cases,
            page.unapplied_rounds,
            optional_round_json(page.next_after),
        );
        self.encode(&knowledge)
    }

    fn encode_unknown(&self, reason: &str) -> Result<Vec<u8>, String> {
        self.encode(&format!(
            "{{\"state\":\"unknown\",\"reason\":\"{reason}\"}}"
        ))
    }

    fn encode(&self, knowledge: &str) -> Result<Vec<u8>, String> {
        let wire = format!(
            "{{\"schema_version\":\"fa.live-evaluator-cases/1\",\"request\":{},\"knowledge\":{knowledge},\"effect_authority\":false}}",
            std::str::from_utf8(&self.wire).map_err(debug)?
        ).into_bytes();
        validate(self, &wire)?;
        Ok(wire)
    }
}

struct Case {
    round: u64,
    attempt: u64,
    policy: u64,
    congress: u64,
    root: [u8; 32],
    status: FileEvaluationCaseStatus,
}

struct Page {
    total_cases: usize,
    remaining_cases: usize,
    unapplied_rounds: usize,
    cases: Vec<Case>,
    next_after: Option<u64>,
}

/// An authenticated, request-bound page is an observation. Its revisions can
/// become stale immediately and its descriptors cannot authorize assessments.
pub(crate) fn validate(request: &CasesRequest, wire: &[u8]) -> Result<bool, String> {
    page_metadata(request, wire).map(|page| page.is_some())
}

/// Validated inventory coordinates, kept with their exact request and cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageMetadata {
    pub(crate) basis: FileEvaluationCasesBasis,
    pub(crate) total_cases: usize,
    pub(crate) remaining_cases: usize,
    pub(crate) unapplied_rounds: usize,
    pub(crate) count: usize,
    pub(crate) next_after: Option<u64>,
}

pub(crate) fn page_metadata(
    request: &CasesRequest,
    wire: &[u8],
) -> Result<Option<PageMetadata>, String> {
    let parsed = parse(wire, RESPONSE_BYTES)?;
    let root = fields(
        &parsed,
        &["schema_version", "request", "knowledge", "effect_authority"],
    )?;
    expected(root, "schema_version", "fa.live-evaluator-cases/1")?;
    if root["request"] != request.parsed {
        return Err("case listing response does not bind the submitted evaluator request".into());
    }
    if root["effect_authority"].as_bool() != Some(false) {
        return Err("case listings cannot carry effect authority".into());
    }
    let knowledge = root["knowledge"]
        .as_object()
        .ok_or("case listing requires explicit knowledge")?;
    match knowledge.get("state").and_then(Json::as_str) {
        Some("known") => {
            let known = fields(&root["knowledge"], &["state", "basis", "value"])?;
            let observed_basis = basis(&known["basis"])?;
            if request
                .expected_basis
                .is_some_and(|expected| expected != observed_basis)
            {
                return Err("case listing response changed the pinned original revisions".into());
            }
            let value = fields(
                &known["value"],
                &[
                    "total_cases",
                    "remaining_cases",
                    "unapplied_rounds",
                    "cases",
                    "next_after",
                ],
            )?;
            let total = decimal(&value["total_cases"])?;
            let remaining = decimal(&value["remaining_cases"])?;
            let unapplied = decimal(&value["unapplied_rounds"])?;
            if total
                .checked_add(unapplied)
                .is_none_or(|count| count > MAX_EVALUATION_CASES as u64)
            {
                return Err("case listing exceeds the original retained history bound".into());
            }
            let cases = value["cases"]
                .as_array()
                .ok_or("case listing requires a bounded array")?;
            if remaining > total || cases.len() as u64 != remaining.min(request.limit as u64) {
                return Err(
                    "case listing contradicts its requested limit or remaining total".into(),
                );
            }
            if total > observed_basis.evaluation_revision
                || observed_basis.evaluation_revision > observed_basis.journal_revision
            {
                return Err("case listing count contradicts its acknowledged revisions".into());
            }
            let mut previous = request.after_round.unwrap_or(0);
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
                        "case listing rounds must increase strictly after the cursor".into(),
                    );
                }
                previous = round;
                for field in ["attempt", "policy_generation", "congress_generation"] {
                    super::case::positive(&case[field])?;
                }
                super::case::digest(&case["case_evidence_root_hex"])?;
                if !matches!(
                    case["status"].as_str(),
                    Some("pending" | "censored" | "resolved")
                ) {
                    return Err("unsupported case listing status".into());
                }
            }
            let next = optional_round(&value["next_after"])?;
            if next.is_some() != (remaining > cases.len() as u64)
                || next.is_some_and(|next| cases.is_empty() || next != previous)
            {
                return Err(
                    "case listing continuation must name a complete nonterminal page".into(),
                );
            }
            if request.after_round.is_none() {
                if remaining != total {
                    return Err("first case listing page contradicts its retained total".into());
                }
            } else if remaining >= total {
                // The retained cursor is itself an additional original case.
                return Err("case listing total omits its retained continuation boundary".into());
            }
            Ok(Some(PageMetadata {
                basis: observed_basis,
                total_cases: total as usize,
                remaining_cases: remaining as usize,
                unapplied_rounds: unapplied as usize,
                count: cases.len(),
                next_after: next,
            }))
        }
        Some("unknown") => {
            let unknown = fields(&root["knowledge"], &["state", "reason"])?;
            if request.after_round.is_none()
                || !matches!(
                    unknown["reason"].as_str(),
                    Some("stale_basis" | "cursor_not_recorded")
                )
            {
                return Err("unsupported case listing uncertainty".into());
            }
            Ok(None)
        }
        _ => Err("unsupported case listing knowledge state".into()),
    }
}

fn basis(value: &Json) -> Result<FileEvaluationCasesBasis, String> {
    let fields = fields(value, &["journal_revision", "evaluation_revision"])?;
    Ok(FileEvaluationCasesBasis {
        journal_revision: super::case::positive(&fields["journal_revision"])?,
        evaluation_revision: decimal(&fields["evaluation_revision"])?,
    })
}

fn basis_json(basis: FileEvaluationCasesBasis) -> String {
    format!(
        "{{\"journal_revision\":\"{}\",\"evaluation_revision\":\"{}\"}}",
        basis.journal_revision, basis.evaluation_revision,
    )
}

fn optional_round(value: &Json) -> Result<Option<u64>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        super::case::positive(value).map(Some)
    }
}

fn optional_round_json(round: Option<u64>) -> String {
    round.map_or_else(|| "null".to_owned(), |round| format!("\"{round}\""))
}

// Sixteen six-field descriptors require more nodes than the scalar protocol.
// Keep its old parser untouched and bound this one separately, including keys.
fn parse(bytes: &[u8], maximum: usize) -> Result<Json, String> {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: maximum,
            max_depth: 6,
            max_items: 512,
            max_string_bytes: 128,
        },
    )
    .map_err(debug)
}

#[cfg(test)]
#[path = "../tests/cases_protocol.rs"]
mod tests;
