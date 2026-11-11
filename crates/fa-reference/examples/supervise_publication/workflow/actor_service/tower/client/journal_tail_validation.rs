//! Correlate bounded native event metadata without reconstructing an event,
//! judgment or authority. Journal revisions count every retained transition;
//! they are not control sequences, timestamps or resumable production rights.

use super::hold_diagnosis::{decimal, decimal_text, exact};
use super::{Json, ReadArguments, Scope, TowerRequest, Verb};
use fa_reference::action::Purpose;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileJournalEventKind, MAX_JOURNAL_TAIL_CAPTURE_EVENTS, MAX_JOURNAL_TAIL_PAGE_EVENTS,
};
use fa_reference::address_resolver::{Address, Noun};
use std::collections::{BTreeMap, BTreeSet};

const INVALID: &str =
    "invalid or inconsistent native tail page; no later request or retry was sent";

pub(super) fn matches(value: &Json, request: &TowerRequest) -> bool {
    validate(value, request).is_some()
}

fn validate(value: &Json, request: &TowerRequest) -> Option<()> {
    if request.verb != Verb::Tail
        || request.address.kind != Noun::Run
        || !exact(
            value,
            &[
                "schema",
                "operation",
                "registration",
                "query_address",
                "historical",
                "disclosure",
                "current_eligibility",
                if value.get("error").is_some() {
                    "error"
                } else {
                    "knowledge"
                },
            ],
        )
        || value.get("schema")?.as_str()? != "fa.publication-journal-tail/1"
        || value.get("operation")?.as_str()? != "tail"
        || !value.get("historical")?.as_bool()?
        || value.get("disclosure")?.as_str()? != "redacted_journal_events"
        || value.get("query_address")?.as_str()? != request.address.canonical_string()
        || value.get("current_eligibility")?.as_str()? != "not_observed"
    {
        return None;
    }
    let text = value.get("registration")?.as_str()?;
    let registration = Address::parse(text).ok()?;
    if registration.canonical_string() != text
        || registration.kind != Noun::Run
        || registration.tenant != request.address.tenant
        || registration.id != request.address.id
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || request
            .address
            .generation
            .is_some_and(|generation| registration.generation != Some(generation))
        || decimal_text(&registration.tenant)? == 0
        || decimal_text(&registration.id)? == 0
    {
        return None;
    }
    let (after, input_cursor) = match request.arguments {
        ReadArguments::JournalTail { after_revision } => (Some(after_revision), None),
        ReadArguments::JournalTailPage { cursor } if cursor != 0 => (None, Some(cursor)),
        _ => return None,
    };
    if let Some(error) = value.get("error") {
        return (exact(error, &["code"])
            && matches!(
                error.get("code")?.as_str()?,
                "capacity" | "unavailable" | "idempotency_conflict"
            ))
        .then_some(());
    }
    let knowledge = value.get("knowledge")?;
    match knowledge.get("state")?.as_str()? {
        "unknown" => {
            return (exact(knowledge, &["state", "reason"])
                && knowledge.get("reason")?.as_str()? == "controller_unavailable")
                .then_some(());
        }
        "withheld" => {
            return (exact(knowledge, &["state", "authority_required"])
                && knowledge.get("authority_required")?.as_str()? == "supervisor_journal_tail")
                .then_some(());
        }
        "known" if exact(knowledge, &["state", "basis", "value"]) => {}
        _ => return None,
    }
    let basis = knowledge.get("basis")?;
    let scope = basis_scope(basis)?;
    if scope.tenant.to_string() != registration.tenant || scope.run.to_string() != registration.id {
        return None;
    }
    let page = knowledge.get("value")?;
    if !exact(
        page,
        &[
            "after_revision",
            "offset",
            "through_revision",
            "enumeration_complete",
            "continuation",
            "events",
        ],
    ) {
        return None;
    }
    let original_after = decimal(page.get("after_revision")?)?;
    let offset = decimal(page.get("offset")?)?;
    let through_revision = decimal(page.get("through_revision")?)?;
    let complete = page.get("enumeration_complete")?.as_bool()?;
    let cursor = optional(page.get("continuation")?)?;
    let events = page.get("events")?.as_array()?;
    let ending = offset.checked_add(events.len() as u64)?;
    let journal_revision = decimal(basis.get("journal_revision")?)?;
    let total = journal_revision.checked_sub(original_after)?;
    if journal_revision > MAX_JOURNAL_TAIL_CAPTURE_EVENTS as u64
        || ending > total
        || events.len() > MAX_JOURNAL_TAIL_PAGE_EVENTS
        || through_revision != original_after.checked_add(ending)?
        || complete != (ending == total)
        || (complete && cursor.is_some())
        || (!complete && events.is_empty())
        || cursor.is_some_and(|cursor| cursor == 0 || Some(cursor) == input_cursor)
        || after.is_some_and(|after| original_after != after || offset != 0)
        || (input_cursor.is_some() && (offset == 0 || events.is_empty()))
    {
        return None;
    }
    for (index, event) in events.iter().enumerate() {
        let revision = original_after
            .checked_add(offset)?
            .checked_add(index as u64)?
            .checked_add(1)?;
        event_matches(event, revision)?;
    }
    Some(())
}

fn basis_scope(basis: &Json) -> Option<Scope> {
    if !exact(
        basis,
        &[
            "scope",
            "journal_revision",
            "control_sequence",
            "recorded_at",
            "origin",
            "anchored_through",
        ],
    ) || basis.get("origin")?.as_str()? != "acknowledged_owner"
        || !basis.get("anchored_through")?.is_null()
    {
        return None;
    }
    decimal(basis.get("journal_revision")?)?;
    decimal(basis.get("control_sequence")?)?;
    optional(basis.get("recorded_at")?)?;
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
    let parsed = Scope {
        tenant: decimal(scope.get("tenant")?)?,
        principal: decimal(scope.get("principal")?)?,
        run: decimal(scope.get("run")?)?,
        branch: decimal(scope.get("branch")?)?,
        authority: decimal(scope.get("authority")?)?,
        purpose: Purpose::Effect,
    };
    (![
        parsed.tenant,
        parsed.principal,
        parsed.run,
        parsed.branch,
        parsed.authority,
    ]
    .contains(&0))
    .then_some(parsed)
}

fn optional(value: &Json) -> Option<Option<u64>> {
    if value.is_null() {
        Some(None)
    } else {
        decimal(value).map(Some)
    }
}

fn event_matches(event: &Json, revision: u64) -> Option<()> {
    if !exact(event, &["revision", "kind", "attempt", "request", "round"])
        || decimal(event.get("revision")?)? != revision
    {
        return None;
    }
    let attempt = optional(event.get("attempt")?)?;
    let round = optional(event.get("round")?)?;
    let request = optional(event.get("request")?)?;
    if [attempt, round, request].contains(&Some(0)) {
        return None;
    }
    kind_links(
        FileJournalEventKind::from_label(event.get("kind")?.as_str()?)?,
        attempt,
        request,
        round,
    )
}

// These are directly recorded identities, not inferred links through other
// records. A review commitment knows its round without knowing its attempt;
// human decision IDs never become external actor request IDs.
fn kind_links(
    kind: FileJournalEventKind,
    attempt: Option<u64>,
    request: Option<u64>,
    round: Option<u64>,
) -> Option<()> {
    use FileJournalEventKind as Kind;
    let valid = match kind {
        Kind::BeginReview | Kind::LearnedSidecarFinish => {
            attempt.is_some() && request.is_none() && round.is_some()
        }
        Kind::Commit | Kind::OpenReveals | Kind::Reveal | Kind::FinishReview => {
            attempt.is_none() && request.is_none() && round.is_some()
        }
        Kind::SubmitRequest | Kind::TextMessage => {
            attempt.is_none() && request.is_some() && round.is_none()
        }
        Kind::Propose
        | Kind::Inputs
        | Kind::InputsUnavailable
        | Kind::Authorize
        | Kind::RequestHuman
        | Kind::Dispatch
        | Kind::Publish
        | Kind::Reconcile
        | Kind::Seal
        | Kind::AbandonUnknown
        | Kind::Cancel
        | Kind::PublishChecked
        | Kind::PublishCredentialed
        | Kind::LearnedSidecarPrepare
        | Kind::LearnedSidecarBegin => attempt.is_some() && request.is_none() && round.is_none(),
        Kind::Time
        | Kind::HumanDecision
        | Kind::RevokeHumans
        | Kind::Fence
        | Kind::Sweep
        | Kind::Stop
        | Kind::StopProgress
        | Kind::ReplacePolicy
        | Kind::NarrowAuthority
        | Kind::ReserveRecovery
        | Kind::ActorState
        | Kind::ActorCheckpoint
        | Kind::ActorReset
        | Kind::PublicationGuard
        | Kind::SourceEnabled
        | Kind::SourceObserved
        | Kind::SourceWithdrawn
        | Kind::SourceReplaced
        | Kind::CredentialGuard
        | Kind::CredentialRotate
        | Kind::CredentialRevoke
        | Kind::PolicyCampaign
        | Kind::StreamBootstrap
        | Kind::GeneratedStreamBootstrap
        | Kind::Identity
        | Kind::DecoderEnabled
        | Kind::Tokenizer
        | Kind::TextIntent
        | Kind::DecoderStep
        | Kind::DecoderResume
        | Kind::DecoderCheckpoint
        | Kind::DecoderStopPolicy
        | Kind::Generation
        | Kind::GenerationBegin
        | Kind::GenerationAdvance
        | Kind::GenerationCancel
        | Kind::LearnedEnabled
        | Kind::LearnedStepBegin
        | Kind::LearnedStep
        | Kind::LearnedResume
        | Kind::LearnedMessageContinuation
        | Kind::LearnedCheckpoint
        | Kind::Credibility
        | Kind::Consistency
        | Kind::Mediation
        | Kind::PublicationWitness => attempt.is_none() && round.is_none() && request.is_none(),
    };
    valid.then_some(())
}

struct Expected {
    registration: String,
    basis: Json,
    after: u64,
    offset: u64,
    // The host fixes this for a grant. An incomplete page exposes its size;
    // final short pages must agree with that size and the captured remainder.
    page_size: usize,
}

/// Correlate pages whose original capture was seen in this finite exchange.
/// An explicit cursor from an earlier connection is still accepted after local
/// shape/scope and configured-page checks; unseen preceding pages cannot be
/// independently verified.
/// The server retains the original cursor capability and enforces its custody.
pub(super) struct Sequence {
    scope: Scope,
    events_per_page: Option<usize>,
    // Native cursor identities are unique across registrations. Retaining the
    // registration as data prevents an unqualified request from rebinding a
    // previously observed cursor to a different host generation.
    continuations: BTreeMap<u64, Expected>,
    consumed: BTreeSet<u64>,
}

impl Sequence {
    pub(super) fn new(scope: Scope, events_per_page: Option<usize>) -> Self {
        Self {
            scope,
            events_per_page,
            continuations: BTreeMap::new(),
            consumed: BTreeSet::new(),
        }
    }

    pub(super) fn accept(&mut self, parsed: &Json, request: &TowerRequest) -> Result<(), String> {
        if request.verb != Verb::Tail
            || parsed
                .get("result")
                .and_then(|result| result.get("kind"))
                .and_then(Json::as_str)
                == Some("refused")
        {
            return Ok(());
        }
        self.accept_tail(parsed, request)
            .ok_or_else(|| INVALID.into())
    }

    fn accept_tail(&mut self, parsed: &Json, request: &TowerRequest) -> Option<()> {
        let wrapper = parsed.get("result")?.get("value")?;
        let envelope = parsed.get("envelope")?;
        if !matches(wrapper, request) {
            return None;
        }
        let knowledge = match wrapper.get("knowledge") {
            Some(knowledge) if knowledge.get("state")?.as_str()? == "known" => knowledge,
            _ => {
                let reason = match wrapper.get("error") {
                    Some(error) => match error.get("code")?.as_str()? {
                        "capacity" => "BudgetExhausted",
                        "unavailable" => "LowerLayerUnverified",
                        "idempotency_conflict" => "IntegrityFailure",
                        _ => return None,
                    },
                    None => match wrapper.get("knowledge")?.get("state")?.as_str()? {
                        "unknown" => "LowerLayerUnverified",
                        "withheld" => "DeniedByPolicy",
                        _ => return None,
                    },
                };
                return (envelope.get("status")?.as_str()? == "error"
                    && envelope.get("reason_code")?.as_str()? == reason)
                    .then_some(());
            }
        };
        if envelope.get("status")?.as_str()? != "ok" || !envelope.get("reason_code")?.is_null() {
            return None;
        }
        let basis = knowledge.get("basis")?;
        if basis_scope(basis)? != self.scope {
            return None;
        }
        let registration = wrapper.get("registration")?.as_str()?.to_owned();
        let page = knowledge.get("value")?;
        let after = decimal(page.get("after_revision")?)?;
        let offset = decimal(page.get("offset")?)?;
        let count = page.get("events")?.as_array()?.len();
        let remaining = decimal(basis.get("journal_revision")?)?
            .checked_sub(after)?
            .checked_sub(offset)?;
        if let Some(page_size) = self.events_per_page {
            if !(1..=MAX_JOURNAL_TAIL_PAGE_EVENTS).contains(&page_size) {
                return None;
            }
            // The public service profile fixes this grant's page size before
            // the first read. Enforce it even when preceding pages were seen
            // on another connection and their opaque cursor is supplied here.
            if !offset.is_multiple_of(page_size as u64)
                || count as u64 != remaining.min(page_size as u64)
            {
                return None;
            }
        }
        let input = match request.arguments {
            ReadArguments::JournalTailPage { cursor } => Some(cursor),
            ReadArguments::JournalTail { .. } => None,
            _ => return None,
        };
        if let Some(key) = &input {
            if self.consumed.contains(key)
                || self.continuations.get(key).is_some_and(|original| {
                    original.registration != registration
                        || original.basis != *basis
                        || original.after != after
                        || original.offset != offset
                        || count as u64 != remaining.min(original.page_size as u64)
                })
                || self.consumed.len() >= super::MAX_DOCUMENTS
            {
                return None;
            }
        }
        let next = optional(page.get("continuation")?)?;
        if next.as_ref().is_some_and(|key| {
            self.continuations.contains_key(key)
                || self.consumed.contains(key)
                || self.continuations.len() >= super::MAX_DOCUMENTS
        }) {
            return None;
        }
        if let Some(key) = input {
            self.consumed.insert(key);
        }
        if let Some(key) = next {
            self.continuations.insert(
                key,
                Expected {
                    registration,
                    basis: basis.clone(),
                    after,
                    offset: offset.checked_add(count as u64)?,
                    page_size: count,
                },
            );
        }
        Some(())
    }
}
