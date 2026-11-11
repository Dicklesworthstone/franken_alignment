//! Closed metadata and explicit read links from one original acknowledged cut.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileRequestHistory, FileRequestHistoryBasis, FileRequestReviewCompletion,
    MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_REVIEWS,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::encode_request;
use crate::tower::publication::{LinkedRequestHistory, request_address_for};
use std::collections::BTreeSet;

pub(super) fn encode(
    linked: &LinkedRequestHistory,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let registration = linked.registration();
    if address != linked.query_address()
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || address.kind != Noun::Attempt
        || address.tenant != registration.tenant
        || address
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
        || registration.kind != Noun::Run
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || linked.request() == 0
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, value) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", known(value, *basis, linked, encoder)?)
        }
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("unknown")),
                ("reason", quote("controller_unavailable")),
            ]),
        ),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_request_history",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_request_history")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-request-history/1")),
        ("operation", quote("related")),
        ("registration", quote(&registration.canonical_string())),
        ("query_address", quote(&address.canonical_string())),
        ("historical", "true".to_owned()),
        ("current_eligibility", quote("not_observed")),
        ("disclosure", quote("recorded_request_relationships")),
        (field, value),
    ]))
}

fn known(
    history: &FileRequestHistory,
    basis: FileRequestHistoryBasis,
    linked: &LinkedRequestHistory,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let expected = request_address_for(basis.scope, linked.registration(), basis.request)
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if history.basis() != basis
        || basis.request != linked.request()
        || expected.without_generation() != linked.query_address().without_generation()
        || history.admitted() != history.attempt().is_some()
        || history.admitted() != history.stage().is_some()
        || history.attempt() == Some(0)
        || history.reviews().len() > MAX_REQUEST_HISTORY_REVIEWS
        || history.incomplete_reviews() > MAX_REQUEST_HISTORY_EVENTS
        || (!history.admitted()
            && (!history.reviews().is_empty() || history.incomplete_reviews() != 0))
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }

    let requests = linked.explain_requests()?;
    let mut reviews = Vec::with_capacity(history.reviews().len());
    let mut rounds = BTreeSet::new();
    let mut previous_completion = 0;
    for (review, request) in history.reviews().iter().zip(requests) {
        if review.round() == 0
            || !rounds.insert(review.round())
            || review.begin_revision() == 0
            || review.begin_revision() >= review.completed_revision()
            || review.completed_revision() <= previous_completion
            || review.completed_revision() > basis.journal_revision
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        previous_completion = review.completed_revision();
        reviews.push(encoder.bounded(object_json(&[
            ("round", number(review.round())),
            ("begin_revision", number(review.begin_revision())),
            ("completed_revision", number(review.completed_revision())),
            (
                "completion_kind",
                quote(match review.completion_kind() {
                    FileRequestReviewCompletion::ApplicationAttempted => "application_attempted",
                    FileRequestReviewCompletion::EvidenceRefined => "evidence_refined",
                }),
            ),
            (
                "explain_request",
                encode_request(&request).map_err(|_| TowerEncodingError::InvalidKnowledge)?,
            ),
        ]))?);
    }
    let value = encoder.bounded(object_json(&[
        ("admitted", history.admitted().to_string()),
        ("attempt", optional(history.attempt())),
        (
            "stage",
            history.stage().map_or_else(
                || "null".to_owned(),
                |value| quote(super::inspection::stage(value)),
            ),
        ),
        ("incomplete_reviews", number(history.incomplete_reviews())),
        ("enumeration_complete", "true".to_owned()),
        ("reviews", format!("[{}]", reviews.join(","))),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", value),
    ]))
}

fn basis_json(basis: FileRequestHistoryBasis) -> String {
    let scope = basis.scope;
    object_json(&[
        (
            "scope",
            object_json(&[
                ("tenant", number(scope.tenant)),
                ("principal", number(scope.principal)),
                ("run", number(scope.run)),
                ("branch", number(scope.branch)),
                ("authority", number(scope.authority)),
                ("purpose", quote("effect")),
            ]),
        ),
        ("origin", quote("acknowledged_owner")),
        ("request", number(basis.request)),
        ("request_generation", number(basis.request_generation)),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
        (
            "recorded_at",
            optional(basis.recorded_at.map(|tick| tick.0)),
        ),
    ])
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), number)
}
