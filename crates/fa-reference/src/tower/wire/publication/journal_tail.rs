//! A closed metadata-only projection of an original acknowledged journal cut.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::observed::replay::situation::{
    FileSituationBasis, FileSituationOrigin,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileJournalTailPage, MAX_JOURNAL_TAIL_CAPTURE_EVENTS, MAX_JOURNAL_TAIL_PAGE_EVENTS,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::LinkedJournalTail;

pub(super) fn encode(
    linked: &LinkedJournalTail,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let registration = linked.registration();
    if address != linked.query_address()
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || address.without_generation() != registration.without_generation()
        || registration.kind != Noun::Run
        || registration.generation.is_none_or(|value| value == 0)
        || address
            .generation
            .is_some_and(|value| Some(value) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, value) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", known(value, *basis, registration, encoder)?)
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
            authority_required: "supervisor_journal_tail",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_journal_tail")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Err(ActorError::IdempotencyConflict) => (
            "error",
            object_json(&[("code", quote("idempotency_conflict"))]),
        ),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-journal-tail/1")),
        ("operation", quote("tail")),
        ("registration", quote(&registration.canonical_string())),
        ("query_address", quote(&address.canonical_string())),
        ("historical", "true".to_owned()),
        ("current_eligibility", quote("not_observed")),
        ("disclosure", quote("redacted_journal_events")),
        (field, value),
    ]))
}

fn known(
    page: &FileJournalTailPage,
    basis: FileSituationBasis,
    registration: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let offset = u64::try_from(page.offset()).map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let count =
        u64::try_from(page.events().len()).map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let start = page
        .after_revision()
        .checked_add(offset)
        .ok_or(TowerEncodingError::InvalidKnowledge)?;
    let through = start
        .checked_add(count)
        .ok_or(TowerEncodingError::InvalidKnowledge)?;
    let retained = basis
        .journal_revision
        .checked_sub(page.after_revision())
        .ok_or(TowerEncodingError::InvalidKnowledge)?;
    if basis != page.basis()
        || basis.origin != FileSituationOrigin::AcknowledgedOwner
        || basis.anchored_through.is_some()
        || basis.scope.purpose != Purpose::Effect
        || registration.tenant != basis.scope.tenant.to_string()
        || registration.id != basis.scope.run.to_string()
        || page.events().len() > MAX_JOURNAL_TAIL_PAGE_EVENTS
        || retained > MAX_JOURNAL_TAIL_CAPTURE_EVENTS as u64
        || through != page.through_revision()
        || through > basis.journal_revision
        || page.enumeration_complete() != (through == basis.journal_revision)
        || (page.enumeration_complete() && page.continuation().is_some())
        || (!page.enumeration_complete() && page.events().is_empty())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut events = Vec::with_capacity(page.events().len());
    for (index, row) in page.events().iter().enumerate() {
        let expected = start
            .checked_add(index as u64)
            .and_then(|value| value.checked_add(1))
            .ok_or(TowerEncodingError::InvalidKnowledge)?;
        if row.revision() != expected
            || [row.attempt(), row.request(), row.round()]
                .into_iter()
                .flatten()
                .any(|value| value == 0)
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        events.push(encoder.bounded(object_json(&[
            ("revision", number(row.revision())),
            ("kind", quote(row.kind().as_str())),
            ("attempt", optional(row.attempt())),
            ("request", optional(row.request())),
            ("round", optional(row.round())),
        ]))?);
    }
    let scope = basis.scope;
    let native_basis = object_json(&[
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
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        (
            "recorded_at",
            optional(basis.recorded_at.map(|tick| tick.0)),
        ),
        ("origin", quote("acknowledged_owner")),
        ("anchored_through", "null".to_owned()),
    ]);
    let value = encoder.bounded(object_json(&[
        ("after_revision", number(page.after_revision())),
        ("offset", number(page.offset())),
        ("through_revision", number(page.through_revision())),
        (
            "enumeration_complete",
            page.enumeration_complete().to_string(),
        ),
        (
            "continuation",
            optional(page.continuation().map(|cursor| cursor.id())),
        ),
        ("events", format!("[{}]", events.join(","))),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", native_basis),
        ("value", value),
    ]))
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), number)
}
