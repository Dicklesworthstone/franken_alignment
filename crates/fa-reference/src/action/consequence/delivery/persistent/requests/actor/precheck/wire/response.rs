//! Strict observation-only responses: one envelope and epistemic-state decoder.
use super::{MAX_PRECHECK_RESPONSE_BYTES, PrecheckWireError};
use super::super::{FilePrecheckBasis, FilePrecheckObservation, FilePrecheckOutcome};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecheckResponseError { Malformed, UnsupportedVersion, Capacity, Binding }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecheckWireResponse {
    pub query: Option<u64>,
    pub result: Result<FilePrecheckObservation, PrecheckWireError>,
}

impl PrecheckWireResponse {
    /// Precheck cannot establish a pending effect, outcome, absence or authority.
    pub fn encode(&self) -> Result<Vec<u8>, PrecheckResponseError> {
        encode_envelope("fa.actor-precheck-response/1", "precheck", self.query, &self.result,
            |value| Ok(match value {
                FilePrecheckOutcome::DeniedByPolicy => "\"value\":\"denied_by_policy\"".to_owned(),
                FilePrecheckOutcome::PrecheckPassed => "\"value\":\"precheck_passed\",\"review_required\":true".to_owned(),
            }))
    }
}

/// Representation and correlation, not authenticity or truth. Labels may recur.
/// A no-query parser/capacity refusal cannot become a known observation.
pub fn decode_precheck_response(bytes: &[u8], query: u64, expected_policy_epoch: u64)
    -> Result<PrecheckWireResponse, PrecheckResponseError>
{
    let (query, result) = decode_envelope(bytes, query, expected_policy_epoch,
        "fa.actor-precheck-response/1", "precheck", 48, |value| {
            match text(value, "value")? {
                "denied_by_policy" => {
                    fields(value, &["state", "value", "basis"])?;
                    Ok(FilePrecheckOutcome::DeniedByPolicy)
                }
                "precheck_passed" => {
                    fields(value, &["state", "value", "review_required", "basis"])?;
                    if value.get("review_required").and_then(Json::as_bool) != Some(true) {
                        return Err(PrecheckResponseError::Malformed);
                    }
                    Ok(FilePrecheckOutcome::PrecheckPassed)
                }
                _ => Err(PrecheckResponseError::Malformed),
            }
        })?;
    Ok(PrecheckWireResponse { query, result })
}

pub(super) type Object = BTreeMap<String, Json>;
type Observation<T> = Knowledge<T, FilePrecheckBasis>;
type Decoded<T> = (Option<u64>, Result<Observation<T>, PrecheckWireError>);

// Schema/marker names come only from compiled codecs, never input strings.
// Sharing the envelope keeps discovery from acquiring weaker correlation rules.
pub(super) fn encode_envelope<T>(schema: &str, marker: &str, bound: Option<u64>,
    result: &Result<Observation<T>, PrecheckWireError>,
    known: impl FnOnce(&T) -> Result<String, PrecheckResponseError>,
) -> Result<Vec<u8>, PrecheckResponseError> {
    if bound == Some(0) { return Err(PrecheckResponseError::Binding); }
    let query = bound.map_or_else(|| "null".to_owned(), |query| format!("\"{query}\""));
    let body = match result {
        Ok(value) => {
            let bound = bound.ok_or(PrecheckResponseError::Binding)?;
            format!("\"status\":\"ok\",\"knowledge\":{}", knowledge_json(value, bound, known)?)
        }
        Err(error) => {
            if bound.is_none() && !unbound_error(*error) { return Err(PrecheckResponseError::Binding); }
            format!("\"status\":\"error\",\"reason\":\"{}\"", error.name())
        }
    };
    let bytes = format!("{{\"schema\":\"{schema}\",\"{marker}\":true,\"query\":{query},{body}}}")
        .into_bytes();
    if bytes.len() > MAX_PRECHECK_RESPONSE_BYTES { return Err(PrecheckResponseError::Capacity); }
    Ok(bytes)
}

pub(super) fn decode_envelope<T>(bytes: &[u8], query: u64, expected_policy_epoch: u64,
    schema: &str, marker: &str, max_items: usize,
    known: impl FnOnce(&Object) -> Result<T, PrecheckResponseError>,
) -> Result<Decoded<T>, PrecheckResponseError> {
    if query == 0 { return Err(PrecheckResponseError::Binding); }
    let parsed = strict_json::parse(bytes, Limits {
        max_bytes: MAX_PRECHECK_RESPONSE_BYTES, max_depth: 5, max_items, max_string_bytes: 64,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit =>
            PrecheckResponseError::Capacity,
        _ => PrecheckResponseError::Malformed,
    })?;
    let root = parsed.as_object().ok_or(PrecheckResponseError::Malformed)?;
    if text(root, "schema")? != schema { return Err(PrecheckResponseError::UnsupportedVersion); }
    if root.get(marker).and_then(Json::as_bool) != Some(true) {
        return Err(PrecheckResponseError::Malformed);
    }
    let bound = match root.get("query") {
        Some(Json::Null) => None,
        Some(_) => {
            let actual = decimal(root, "query", false)?;
            if actual != query { return Err(PrecheckResponseError::Binding); }
            Some(actual)
        }
        None => return Err(PrecheckResponseError::Malformed),
    };
    let result = match text(root, "status")? {
        "ok" => {
            fields(root, &["schema", marker, "query", "status", "knowledge"])?;
            let query = bound.ok_or(PrecheckResponseError::Binding)?;
            Ok(knowledge(object(root, "knowledge")?, query, expected_policy_epoch, known)?)
        }
        "error" => {
            fields(root, &["schema", marker, "query", "status", "reason"])?;
            let error = match text(root, "reason")? {
                "malformed_request" => PrecheckWireError::MalformedRequest,
                "unsupported_version" => PrecheckWireError::UnsupportedVersion,
                "capacity" => PrecheckWireError::Capacity,
                "unavailable" => PrecheckWireError::Unavailable,
                "withheld" => PrecheckWireError::Withheld,
                _ => return Err(PrecheckResponseError::Malformed),
            };
            if bound.is_none() && !unbound_error(error) { return Err(PrecheckResponseError::Binding); }
            Err(error)
        }
        _ => return Err(PrecheckResponseError::Malformed),
    };
    Ok((bound, result))
}

fn unbound_error(error: PrecheckWireError) -> bool {
    matches!(error, PrecheckWireError::MalformedRequest | PrecheckWireError::UnsupportedVersion | PrecheckWireError::Capacity)
}
fn knowledge<T>(value: &Object, query: u64, expected_policy_epoch: u64,
    known: impl FnOnce(&Object) -> Result<T, PrecheckResponseError>,
) -> Result<Observation<T>, PrecheckResponseError> {
    Ok(match text(value, "state")? {
        "known" => {
            let outcome = known(value)?;
            let basis = object(value, "basis")?;
            fields(basis, &["disclosure", "query", "policy_epoch", "observed_at"])?;
            if decimal(basis, "query", false)? != query
                || decimal(basis, "policy_epoch", true)? != expected_policy_epoch {
                return Err(PrecheckResponseError::Binding);
            }
            Knowledge::Known { value: outcome, basis: FilePrecheckBasis {
                disclosure: decimal(basis, "disclosure", false)?, query,
                policy_epoch: expected_policy_epoch, observed_at: ElapsedTick(decimal(basis, "observed_at", true)?),
            } }
        }
        "unknown" => {
            fields(value, &["state", "reason"])?;
            if text(value, "reason")? != "controller_unavailable" { return Err(PrecheckResponseError::Malformed); }
            Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
        }
        "withheld" => {
            fields(value, &["state", "authority_required"])?;
            if text(value, "authority_required")? != "exact_policy_disclosure" { return Err(PrecheckResponseError::Malformed); }
            Knowledge::Withheld { authority_required: "exact_policy_disclosure" }
        }
        "stale" => {
            fields(value, &["state", "generation", "current"])?;
            let generation = decimal(value, "generation", true)?;
            let current = decimal(value, "current", true)?;
            if generation != expected_policy_epoch || generation == current { return Err(PrecheckResponseError::Binding); }
            Knowledge::Stale { generation, current }
        }
        _ => return Err(PrecheckResponseError::Malformed),
    })
}
fn knowledge_json<T>(value: &Observation<T>, query: u64,
    known: impl FnOnce(&T) -> Result<String, PrecheckResponseError>,
) -> Result<String, PrecheckResponseError> {
    Ok(match value {
        Knowledge::Known { value, basis } => {
            if basis.query != query || basis.disclosure == 0 { return Err(PrecheckResponseError::Binding); }
            format!(concat!("{{\"state\":\"known\",{},\"basis\":{{\"disclosure\":\"{}\",\"query\":\"{}\",",
                "\"policy_epoch\":\"{}\",\"observed_at\":\"{}\"}}}}"),
                known(value)?, basis.disclosure, query, basis.policy_epoch, basis.observed_at.0)
        }
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable } =>
            "{\"state\":\"unknown\",\"reason\":\"controller_unavailable\"}".to_owned(),
        Knowledge::Withheld { authority_required: "exact_policy_disclosure" } =>
            "{\"state\":\"withheld\",\"authority_required\":\"exact_policy_disclosure\"}".to_owned(),
        Knowledge::Stale { generation, current } if generation != current =>
            format!("{{\"state\":\"stale\",\"generation\":\"{generation}\",\"current\":\"{current}\"}}"),
        _ => return Err(PrecheckResponseError::Malformed),
    })
}
pub(super) fn fields(value: &Object, expected: &[&str]) -> Result<(), PrecheckResponseError> {
    if value.len() != expected.len() || expected.iter().any(|key| !value.contains_key(*key)) {
        return Err(PrecheckResponseError::Malformed);
    }
    Ok(())
}
pub(super) fn text<'a>(value: &'a Object, key: &str) -> Result<&'a str, PrecheckResponseError> {
    value.get(key).and_then(Json::as_str).ok_or(PrecheckResponseError::Malformed)
}
pub(super) fn object<'a>(value: &'a Object, key: &str) -> Result<&'a Object, PrecheckResponseError> {
    value.get(key).and_then(Json::as_object).ok_or(PrecheckResponseError::Malformed)
}
pub(super) fn decimal(value: &Object, key: &str, zero: bool) -> Result<u64, PrecheckResponseError> {
    let text = text(value, key)?;
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0')) { return Err(PrecheckResponseError::Malformed); }
    let number = text.parse::<u64>().map_err(|_| PrecheckResponseError::Malformed)?;
    if !zero && number == 0 { return Err(PrecheckResponseError::Malformed); }
    Ok(number)
}
