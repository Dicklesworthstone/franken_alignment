//! Bounded transport sections from one complete original native comparison.
//! Policy graphs appear in its header once; each subsequent section contains
//! contiguous whole cases. A known page is never a claim of complete traversal.

use super::{
    Encoder, TowerEncodingError, basis_json, bounded_array, case_id, case_json, number,
    object_json, observation, policy, projection, quote,
};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplayBasis, FilePolicyReplayPage, MAX_POLICY_REPLAY_PAGE_CASES,
    MAX_POLICY_REPLAY_PAGE_QUERIES,
};
use crate::action::consequence::policy_campaign::PolicyDelta;
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::LinkedPolicyReplayPage;

pub(super) fn encode(
    linked: &LinkedPolicyReplayPage,
    address: &Address,
    key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if address != linked.registration() || key != Some(linked.campaign().to_string().as_str()) {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = observation(linked.native(), |page, basis| {
        known(page, basis, linked, address, encoder)
    })?;
    projection("fa.publication-policy-replay/2", field, result, encoder)
}

fn known(
    page: &FilePolicyReplayPage,
    basis: FilePolicyReplayBasis,
    linked: &LinkedPolicyReplayPage,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let original = page.original();
    let report = original.report();
    let summary = original.summary();
    let limits = original.limits();
    let counts = summary
        .unchanged
        .checked_add(summary.newly_blocked)
        .and_then(|n| n.checked_add(summary.newly_reviewable))
        .and_then(|n| n.checked_add(summary.requires_shadow_cases));
    if basis != page.basis()
        || basis != original.basis()
        || basis.query != linked.campaign()
        || basis.query == 0
        || page.request_cursor() != linked.request_cursor()
        || address.kind != Noun::Run
        || address.tenant != basis.scope.tenant.to_string()
        || address.id != basis.scope.run.to_string()
        || address.generation.is_none_or(|generation| generation == 0)
        || basis.scope.purpose != Purpose::Effect
        || basis.policy_generation != report.previous_policy().generation()
        || report.candidate_policy().generation() <= basis.policy_generation
        || summary.case_count == 0
        || summary.case_count != report.cases().len()
        || summary.case_count > limits.cases
        || summary.input_bytes != report.input_bytes()
        || summary.input_bytes > limits.input_bytes
        || counts != Some(summary.case_count)
        || summary.requires_shadow != (summary.requires_shadow_cases != 0)
        || page.remaining_pages_at_capture() > MAX_POLICY_REPLAY_PAGE_QUERIES
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let end = page
        .offset()
        .checked_add(page.cases().len())
        .ok_or(TowerEncodingError::InvalidKnowledge)?;
    let expected_complete = !page.is_header() && end == summary.case_count;
    if (page.is_header()
        && (page.request_cursor().is_some() || page.offset() != 0 || !page.cases().is_empty()))
        || (!page.is_header()
            && (page.request_cursor().is_none_or(|id| id == 0)
                || page.offset() >= summary.case_count
                || !page.offset().is_multiple_of(MAX_POLICY_REPLAY_PAGE_CASES)
                || page.cases().len()
                    != (summary.case_count - page.offset()).min(MAX_POLICY_REPLAY_PAGE_CASES)))
        || end > summary.case_count
        || page.enumeration_complete() != expected_complete
        || (page.continuation().is_some()
            != (!expected_complete && page.remaining_pages_at_capture() != 0))
        || page.continuation().is_some_and(|next| {
            next.id() == 0
                || next.campaign() != basis.query
                || Some(next.id()) == page.request_cursor()
        })
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let previous = if page.is_header() {
        encoder.bounded(policy::encode(report.previous_policy()))?
    } else {
        "null".to_owned()
    };
    let candidate = if page.is_header() {
        encoder.bounded(policy::encode(report.candidate_policy()))?
    } else {
        "null".to_owned()
    };
    let cases = bounded_array(
        page.cases().iter().map(|case| {
            if case.action().spec().scope != basis.scope {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            case_json(
                case,
                report.previous_policy(),
                report.candidate_policy(),
                encoder,
            )
        }),
        encoder,
    )?;
    let ids = |delta| {
        bounded_array(
            page.cases()
                .iter()
                .filter(move |case| case.delta() == delta)
                .map(|case| Ok(case_id(case.id()))),
            encoder,
        )
    };
    let body = encoder.bounded(object_json(&[
        (
            "page",
            object_json(&[
                (
                    "kind",
                    quote(if page.is_header() { "header" } else { "cases" }),
                ),
                (
                    "request_cursor",
                    page.request_cursor()
                        .map_or_else(|| "null".to_owned(), number),
                ),
                ("offset", number(page.offset())),
                ("count", number(page.cases().len())),
                ("total", number(summary.case_count)),
                (
                    "enumeration_complete",
                    page.enumeration_complete().to_string(),
                ),
                (
                    "next_cursor",
                    page.continuation()
                        .map_or_else(|| "null".to_owned(), |next| number(next.id())),
                ),
                (
                    "remaining_pages_at_capture",
                    number(page.remaining_pages_at_capture()),
                ),
            ]),
        ),
        (
            "summary",
            object_json(&[
                ("case_count", number(summary.case_count)),
                ("input_bytes", number(summary.input_bytes)),
                ("requires_shadow", summary.requires_shadow.to_string()),
                (
                    "delta_counts",
                    object_json(&[
                        ("unchanged", number(summary.unchanged)),
                        ("newly_blocked", number(summary.newly_blocked)),
                        ("newly_reviewable", number(summary.newly_reviewable)),
                        ("requires_shadow", number(summary.requires_shadow_cases)),
                    ]),
                ),
                (
                    "limits",
                    object_json(&[
                        ("cases", number(limits.cases)),
                        ("input_bytes", number(limits.input_bytes)),
                    ]),
                ),
                (
                    "remaining_campaigns_at_capture",
                    number(original.remaining_queries()),
                ),
            ]),
        ),
        ("previous_policy", previous),
        ("candidate_policy", candidate),
        ("cases", cases),
        ("newly_reviewable", ids(PolicyDelta::NewlyReviewable)?),
        ("newly_blocked", ids(PolicyDelta::NewlyBlocked)?),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", body),
    ]))
}
