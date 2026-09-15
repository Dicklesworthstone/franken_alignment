//! Decode the existing actor observation format, never an effect credential.
use super::{MAX_RESPONSE_BYTES, WireError, WireResponse};
use super::super::actor::{ActorBasis, ActorOutcome, BasisSource, Knowledge, UnknownReason};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseError { Malformed, UnsupportedVersion, Capacity, Binding }
type Object = BTreeMap<String, Json>;

/// Parse one bounded JSON response, without its transport newline. Unknown
/// fields/variants and cross-request bases refuse. This validates representation
/// and correlation, NOT the server's authenticity or the truth of an outcome.
pub fn decode_response(bytes: &[u8]) -> Result<WireResponse, ResponseError> {
    let parsed = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RESPONSE_BYTES, max_depth: 4, max_items: 48, max_string_bytes: 64,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => ResponseError::Capacity,
        _ => ResponseError::Malformed,
    })?;
    let root = parsed.as_object().ok_or(ResponseError::Malformed)?;
    match root.get("version").and_then(Json::as_u64) {
        Some(1) => {}, Some(_) => return Err(ResponseError::UnsupportedVersion),
        None => return Err(ResponseError::Malformed),
    }
    let request = match root.get("request") {
        Some(Json::Null) => None,
        Some(_) => Some(decimal(root, "request", false)?),
        None => return Err(ResponseError::Malformed),
    };
    let result = match text(root, "status")? {
        "ok" => {
            fields(root, &["version", "request", "status", "knowledge"])?;
            let request = request.ok_or(ResponseError::Binding)?;
            Ok(knowledge(object(root, "knowledge")?, request)?)
        }
        "error" => {
            fields(root, &["version", "request", "status", "reason"])?;
            let error = match text(root, "reason")? {
                "malformed_request" => WireError::MalformedRequest,
                "unsupported_version" => WireError::UnsupportedVersion,
                "idempotency_conflict" => WireError::IdempotencyConflict,
                "capacity" => WireError::Capacity,
                "unavailable" => WireError::Unavailable,
                "withheld" => WireError::Withheld,
                _ => return Err(ResponseError::Malformed),
            };
            if request.is_none() && !matches!(error,
                WireError::MalformedRequest | WireError::UnsupportedVersion | WireError::Capacity)
            { return Err(ResponseError::Binding); }
            Err(error)
        }
        _ => return Err(ResponseError::Malformed),
    };
    Ok(WireResponse { request, result })
}

fn knowledge(value: &Object, request: u64) -> Result<Knowledge<ActorOutcome>, ResponseError> {
    Ok(match text(value, "state")? {
        "known" => {
            fields(value, &["state", "value", "basis"])?;
            let outcome = match text(value, "value")? {
                "not_admitted" => ActorOutcome::NotAdmitted,
                "denied" => ActorOutcome::Denied,
                "cancelled_before_dispatch" => ActorOutcome::CancelledBeforeDispatch,
                "executed" => ActorOutcome::Executed,
                "confirmed_not_executed" => ActorOutcome::ConfirmedNotExecuted,
                _ => return Err(ResponseError::Malformed),
            };
            let basis = object(value, "basis")?;
            fields(basis, &["request", "generation", "source"])?;
            let bound = decimal(basis, "request", false)?;
            if bound != request { return Err(ResponseError::Binding); }
            let generation = decimal(basis, "generation", false)?;
            let source = match text(basis, "source")? {
                "intake" => BasisSource::Intake,
                "control_ledger" => BasisSource::ControlLedger,
                _ => return Err(ResponseError::Malformed),
            };
            if (source == BasisSource::Intake && !matches!(outcome,
                ActorOutcome::NotAdmitted | ActorOutcome::CancelledBeforeDispatch))
                || (source == BasisSource::ControlLedger && outcome == ActorOutcome::NotAdmitted)
            { return Err(ResponseError::Binding); }
            Knowledge::Known { value: outcome, basis: ActorBasis { request, generation, source } }
        }
        "pending" => {
            fields(value, &["state", "request"])?;
            if decimal(value, "request", false)? != request { return Err(ResponseError::Binding); }
            Knowledge::Pending { request }
        }
        "unknown" => {
            fields(value, &["state", "reason"])?;
            Knowledge::Unknown { reason: match text(value, "reason")? {
                "controller_unavailable" => UnknownReason::ControllerUnavailable,
                "outcome_unknown" => UnknownReason::OutcomeUnknown,
                _ => return Err(ResponseError::Malformed),
            } }
        }
        "withheld" => {
            fields(value, &["state", "authority_required"])?;
            if text(value, "authority_required")? != "own_request" { return Err(ResponseError::Malformed); }
            Knowledge::Withheld { authority_required: "own_request" }
        }
        "stale" => {
            fields(value, &["state", "generation", "current"])?;
            Knowledge::Stale { generation: decimal(value, "generation", true)?, current: decimal(value, "current", true)? }
        }
        "absent" => {
            fields(value, &["state", "closed_domain", "frontier"])?;
            Knowledge::Absent { closed_domain: decimal(value, "closed_domain", true)?, frontier: decimal(value, "frontier", true)? }
        }
        _ => return Err(ResponseError::Malformed),
    })
}
fn fields(value: &Object, expected: &[&str]) -> Result<(), ResponseError> {
    if value.len() != expected.len() || expected.iter().any(|key| !value.contains_key(*key)) {
        return Err(ResponseError::Malformed);
    }
    Ok(())
}
fn text<'a>(value: &'a Object, key: &str) -> Result<&'a str, ResponseError> {
    value.get(key).and_then(Json::as_str).ok_or(ResponseError::Malformed)
}
fn object<'a>(value: &'a Object, key: &str) -> Result<&'a Object, ResponseError> {
    value.get(key).and_then(Json::as_object).ok_or(ResponseError::Malformed)
}
fn decimal(value: &Object, key: &str, zero: bool) -> Result<u64, ResponseError> {
    let text = text(value, key)?;
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0')) { return Err(ResponseError::Malformed); }
    let number = text.parse::<u64>().map_err(|_| ResponseError::Malformed)?;
    if !zero && number == 0 { return Err(ResponseError::Malformed); }
    Ok(number)
}
