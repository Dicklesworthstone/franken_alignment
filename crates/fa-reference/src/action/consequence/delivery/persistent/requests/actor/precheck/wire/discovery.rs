//! Discovery of THIS explicitly granted exact-precheck boundary, not the whole
//! deployment. No policy contents, helper feedback, signatures or effect keys.
use super::{FileActorPrecheckPort, PrecheckResponseError, PrecheckWireError};
use super::super::{ActorError, FilePrecheckBasis, Knowledge, MAX_PRECHECK_QUERIES};
use super::response::{decode_envelope, encode_envelope, decimal, fields, object, text};
use crate::strict_json::{self, ErrorKind, Json, Limits};

pub const MAX_CAPABILITIES_REQUEST_BYTES: usize = 256;
pub(super) const REQUEST_SCHEMA: &str = "fa.actor-precheck-capabilities/1";
const RESPONSE_SCHEMA: &str = "fa.actor-precheck-capabilities-response/1";

/// Informational endpoint data. The remaining count is shared with evaluations
/// and all port clones, measured AFTER this discovery query was charged. It is
/// not a reservation, independent allowance or promise about a later query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecheckCapabilities { remaining_queries: usize, witness_cache: bool }
impl PrecheckCapabilities {
    pub fn remaining_queries(self) -> usize { self.remaining_queries }
    /// Exact memoization was explicitly selected for THIS grant. It is not a
    /// promise of a hit, a source refresh, or a reusable review/permission result.
    pub fn witness_cache_enabled(self) -> bool { self.witness_cache }
}
pub type PrecheckCapabilitiesObservation = Knowledge<PrecheckCapabilities, FilePrecheckBasis>;

impl FileActorPrecheckPort {
    /// Uses the same revocation, original-owner eligibility and finite work as
    /// exact evaluation. Discovery never creates a grant, reads source or time,
    /// evaluates policy, acquires an admission snapshot or appends a journal event.
    /// Repeating a label spends another query; failed entered reads also count.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
    /// fn no_disclosure(port: FileActorPort) { port.capabilities(1, 0); }
    /// ```
    pub fn capabilities(&self, query: u64, expected_policy_epoch: u64)
        -> Result<PrecheckCapabilitiesObservation, ActorError>
    {
        self.with_disclosure(|grant| {
            if query == 0 { return Err(ActorError::MalformedProposal); }
            let projection = &grant.projection;
            if expected_policy_epoch != projection.epoch {
                return Ok(Knowledge::Stale { generation: expected_policy_epoch, current: projection.epoch });
            }
            Ok(Knowledge::Known {
                value: PrecheckCapabilities { remaining_queries: grant.remaining.get(), witness_cache: grant.cache.is_some() },
                basis: FilePrecheckBasis { disclosure: grant.id, query,
                    policy_epoch: projection.epoch, observed_at: projection.now },
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecheckCapabilitiesRequest {
    pub query: u64,
    pub expected_policy_epoch: u64,
}
impl PrecheckCapabilitiesRequest {
    pub fn encode(self) -> Result<Vec<u8>, PrecheckWireError> {
        if self.query == 0 { return Err(PrecheckWireError::MalformedRequest); }
        Ok(format!(concat!("{{\"schema\":\"fa.actor-precheck-capabilities/1\",",
            "\"operation\":\"capabilities\",\"query\":\"{}\",\"expected_policy_epoch\":\"{}\"}}"),
            self.query, self.expected_policy_epoch).into_bytes())
    }
}

pub fn decode_capabilities_request(bytes: &[u8]) -> Result<PrecheckCapabilitiesRequest, PrecheckWireError> {
    let parsed = strict_json::parse(bytes, Limits {
        max_bytes: MAX_CAPABILITIES_REQUEST_BYTES, max_depth: 2, max_items: 16, max_string_bytes: 64,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit =>
            PrecheckWireError::Capacity,
        _ => PrecheckWireError::MalformedRequest,
    })?;
    decode_request_value(&parsed)
}

pub(super) fn decode_request_value(parsed: &Json) -> Result<PrecheckCapabilitiesRequest, PrecheckWireError> {
    let root = parsed.as_object().ok_or(PrecheckWireError::MalformedRequest)?;
    if text(root, "schema").map_err(request_error)? != REQUEST_SCHEMA {
        return Err(PrecheckWireError::UnsupportedVersion);
    }
    fields(root, &["schema", "operation", "query", "expected_policy_epoch"]).map_err(request_error)?;
    if text(root, "operation").map_err(request_error)? != "capabilities" {
        return Err(PrecheckWireError::MalformedRequest);
    }
    Ok(PrecheckCapabilitiesRequest {
        query: decimal(root, "query", false).map_err(request_error)?,
        expected_policy_epoch: decimal(root, "expected_policy_epoch", true).map_err(request_error)?,
    })
}
fn request_error(error: PrecheckResponseError) -> PrecheckWireError {
    match error {
        PrecheckResponseError::Capacity => PrecheckWireError::Capacity,
        _ => PrecheckWireError::MalformedRequest,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecheckCapabilitiesResponse {
    pub query: Option<u64>,
    pub result: Result<PrecheckCapabilitiesObservation, PrecheckWireError>,
}

// Closed, implemented endpoint behavior, NOT caller-supplied profile assertions.
// In particular labels and SO_PEERCRED are not signatures or instance nonces.
const TEXT_FIELDS: [(&str, &str); 10] = [
    ("policy_disclosure", "exact_predicates"), ("precheck_scope", "exact_policy"),
    ("policy_contents", "withheld"), ("helper_feedback", "withheld"),
    ("source_values", "withheld"), ("witness_cache", "unavailable"),
    ("source_refresh", "unavailable"), ("source_semantics", "captured_cut"),
    ("correlation", "labels_only"), ("timing", "observable"),
];
const BOOL_FIELDS: [(&str, bool); 3] = [
    ("signed_profile", false), ("effect_authority", false), ("review_required", true),
];
impl PrecheckCapabilitiesResponse {
    pub fn encode(&self) -> Result<Vec<u8>, PrecheckResponseError> {
        encode_envelope(RESPONSE_SCHEMA, "informational", self.query, &self.result, |value| {
            if value.remaining_queries >= MAX_PRECHECK_QUERIES { return Err(PrecheckResponseError::Capacity); }
            let mut members = Vec::with_capacity(TEXT_FIELDS.len() + BOOL_FIELDS.len() + 1);
            for (key, text) in TEXT_FIELDS {
                let text = if key == "witness_cache" && value.witness_cache { "exact_witnesses" } else { text };
                members.push(format!("\"{key}\":\"{text}\""));
            }
            for (key, value) in BOOL_FIELDS { members.push(format!("\"{key}\":{value}")); }
            members.push(format!("\"remaining_queries\":\"{}\"", value.remaining_queries));
            Ok(format!("\"value\":{{{}}}", members.join(",")))
        })
    }
}

pub fn decode_capabilities_response(bytes: &[u8], query: u64, expected_policy_epoch: u64)
    -> Result<PrecheckCapabilitiesResponse, PrecheckResponseError>
{
    let (query, result) = decode_envelope(bytes, query, expected_policy_epoch,
        RESPONSE_SCHEMA, "informational", 64, |known| {
            fields(known, &["state", "value", "basis"])?;
            let value = object(known, "value")?;
            if value.len() != TEXT_FIELDS.len() + BOOL_FIELDS.len() + 1 {
                return Err(PrecheckResponseError::Malformed);
            }
            let witness_cache = match text(value, "witness_cache")? {
                "unavailable" => false,
                "exact_witnesses" => true,
                _ => return Err(PrecheckResponseError::Malformed),
            };
            for (key, expected) in TEXT_FIELDS {
                if key != "witness_cache" && text(value, key)? != expected { return Err(PrecheckResponseError::Malformed); }
            }
            for (key, expected) in BOOL_FIELDS {
                if value.get(key).and_then(Json::as_bool) != Some(expected) {
                    return Err(PrecheckResponseError::Malformed);
                }
            }
            let remaining_queries = usize::try_from(decimal(value, "remaining_queries", true)?)
                .map_err(|_| PrecheckResponseError::Capacity)?;
            if remaining_queries >= MAX_PRECHECK_QUERIES { return Err(PrecheckResponseError::Capacity); }
            Ok(PrecheckCapabilities { remaining_queries, witness_cache })
        })?;
    Ok(PrecheckCapabilitiesResponse { query, result })
}

#[cfg(test)]
mod tests;
