//! Original publication envelope and non-Known terminal observations.

use super::syntax::{array, boolean, ensure, object, text};
use super::{PolicyReplayClientError, Result};
use crate::action::consequence::delivery::persistent::requests::actor::{
    MAX_EXPLANATION_EVENTS, MAX_EXPLANATION_QUERIES, MAX_INSPECTION_ATTEMPTS,
    MAX_INSPECTION_QUERIES, MAX_PRECHECK_QUERIES,
};
use crate::action::consequence::oversight::actor::MAX_ACTOR_REQUESTS;
use crate::action::consequence::oversight::actor_wire::MAX_FRAME_BYTES;
use crate::address_resolver::Address;
use crate::strict_json::Json;
use crate::tower::publication::PublicationLimits;
use crate::tower::{ENVELOPE_FIELDS, RESPONSE_SCHEMA, TowerRequest};

pub(super) struct Envelope<'a> {
    ok: bool,
    reason: Option<&'a str>,
    retry_safe: bool,
    unknown_globals: bool,
}

impl Envelope<'_> {
    pub(super) fn known(&self) -> Result<()> {
        ensure(self.ok && self.reason.is_none() && self.retry_safe && self.unknown_globals)
    }

    pub(super) fn error(&self, reason: &str) -> Result<()> {
        ensure(!self.ok && self.reason == Some(reason) && self.retry_safe && self.unknown_globals)
    }
}

pub(super) fn check<'a>(
    value: &'a Json,
    request: &TowerRequest,
    response_bytes: usize,
) -> Result<Envelope<'a>> {
    let envelope = object(value, &ENVELOPE_FIELDS)?;
    ensure(
        text(&envelope["schema_version"])? == RESPONSE_SCHEMA
            && text(&envelope["operation_id"])? == request.operation_id
            && text(&envelope["address"])? == request.address.canonical_string()
            && envelope["idempotency_key"].as_str() == request.idempotency_key.as_deref(),
    )?;
    let ok = match text(&envelope["status"])? {
        "ok" => true,
        "error" => false,
        _ => return Err(PolicyReplayClientError::InvalidResponse),
    };
    let reason = if envelope["reason_code"].is_null() {
        None
    } else {
        Some(text(&envelope["reason_code"])?)
    };
    ensure(ok == reason.is_none())?;
    if let Some(reason) = reason {
        ensure(matches!(
            reason,
            "DeniedByPolicy"
                | "MissingRequiredObservation"
                | "StaleGeneration"
                | "CapabilityUnsupported"
                | "BudgetExhausted"
                | "PermitConsumed"
                | "DeadlineExpired"
                | "OutcomeUnknown"
                | "ReplayClosureUnavailable"
                | "IntegrityFailure"
                | "PurposeMismatch"
                | "LowerLayerUnverified"
        ))?;
    }
    let address = request.address.canonical_string();
    for name in ["missing", "would_change_this"] {
        let values = array(&envelope[name])?;
        if ok {
            ensure(values.is_empty())?;
            continue;
        }
        ensure(values.len() == 1)?;
        let selected = text(&values[0])?;
        if selected == address {
            continue;
        }
        // A stale host registration identifies the original requested mount
        // in `missing` and advises its current generation separately. The
        // advice may stop this capture; it never redirects the next request.
        ensure(name == "would_change_this" && reason == Some("StaleGeneration"))?;
        let current =
            Address::parse(selected).map_err(|_| PolicyReplayClientError::InvalidResponse)?;
        ensure(
            current.canonical_string() == selected
                && current.without_generation() == request.address.without_generation()
                && current.generation.is_some_and(|generation| generation != 0),
        )?;
    }
    let retry_safe = boolean(&envelope["retry_safe"])?;
    let limits = object(
        &envelope["limits"],
        &[
            "max_actor_requests",
            "max_precheck_queries",
            "max_inspection_queries",
            "max_explanation_queries",
            "max_explanation_events",
            "max_inspection_attempts",
            "max_frame_bytes",
            "max_response_bytes",
        ],
    )?;
    for (name, expected) in [
        ("max_actor_requests", MAX_ACTOR_REQUESTS),
        ("max_precheck_queries", MAX_PRECHECK_QUERIES),
        ("max_inspection_queries", MAX_INSPECTION_QUERIES),
        ("max_explanation_queries", MAX_EXPLANATION_QUERIES),
        ("max_explanation_events", MAX_EXPLANATION_EVENTS),
        ("max_inspection_attempts", MAX_INSPECTION_ATTEMPTS),
        ("max_frame_bytes", MAX_FRAME_BYTES),
    ] {
        ensure(limits[name].as_u64() == Some(expected as u64))?;
    }
    let response_limit = limits["max_response_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(PolicyReplayClientError::InvalidResponse)?;
    ensure(
        (PublicationLimits {
            max_response_bytes: response_limit,
        })
        .valid()
            && response_bytes <= response_limit,
    )?;
    let mut unknown_globals = true;
    for (name, kind) in [
        ("generation", "SemanticGeneration"),
        ("control_seq", "ControlSequence"),
        ("source_frontiers", "SourceFrontiers"),
        ("budgets_remaining", "RemainingPublicationBudget"),
        ("realized_cost", "RealizedPublicationCost"),
        ("affordances", "Affordances"),
    ] {
        unknown_globals &= scalar_observation(&envelope[name], kind, false)?;
    }
    Ok(Envelope {
        ok,
        reason,
        retry_safe,
        unknown_globals,
    })
}

pub(super) fn refusal(value: &Json, envelope: &Envelope<'_>) -> Result<()> {
    ensure(!envelope.ok && envelope.reason.is_some())?;
    scalar_observation(value, "PublicationRefusal", true)?;
    Ok(())
}

fn scalar_observation(value: &Json, kind: &str, allow_incomplete: bool) -> Result<bool> {
    match value.get("state").and_then(Json::as_str) {
        Some("Unknown") => {
            let value = object(value, &["type", "state", "reason", "would_change_this"])?;
            ensure(
                text(&value["type"])? == kind
                    && !text(&value["would_change_this"])?.is_empty()
                    && (text(&value["reason"])? == "Unavailable"
                        || (allow_incomplete && text(&value["reason"])? == "Incomplete")),
            )?;
            Ok(true)
        }
        Some("Withheld") => {
            let value = object(
                value,
                &["type", "state", "authority_required", "would_change_this"],
            )?;
            ensure(
                text(&value["type"])? == kind
                    && text(&value["authority_required"])? == "Verify"
                    && !text(&value["would_change_this"])?.is_empty(),
            )?;
            Ok(false)
        }
        _ => Err(PolicyReplayClientError::InvalidResponse),
    }
}
