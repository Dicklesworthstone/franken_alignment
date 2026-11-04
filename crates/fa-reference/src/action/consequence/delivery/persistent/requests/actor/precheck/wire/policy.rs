//! Explicit full disclosure of the deterministic policy DEFINITION only.
//! This is a separate read capability, not an upgrade an exact-only actor can
//! request. Source values, empirical review and consequences never enter it.
mod definition;

use super::{FileActorPrecheckPort, PrecheckResponseError, PrecheckWireError};
use super::super::{ActorError, FileActorSupervisor, FileOversight, FilePrecheckBasis,
    FilePrecheckDisclosure, JournalError, Knowledge, Policy};
use super::cache::FilePrecheckCache;
use super::response::{decode_envelope_bounded, encode_envelope_bounded,
    decimal, fields, object, text};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::fmt;

pub const MAX_POLICY_REQUEST_BYTES: usize = 256;
/// Includes the epistemic envelope. Enough for every admitted policy, including
/// the 64 KiB literal ceiling encoded as hex plus all bounded nodes and edges.
pub const MAX_POLICY_RESPONSE_BYTES: usize = 196_608;
pub(super) const REQUEST_SCHEMA: &str = "fa.actor-policy/1";
const RESPONSE_SCHEMA: &str = "fa.actor-policy-response/1";

/// The immutable definition, not a claim that this policy admits any proposal.
/// It contains neither the actual source snapshot nor private dynamic outcomes.
#[derive(Clone, PartialEq, Eq)]
pub struct DisclosedPolicy { definition: Policy }
impl DisclosedPolicy {
    pub fn definition(&self) -> &Policy { &self.definition }
}
impl fmt::Debug for DisclosedPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisclosedPolicy").field("generation", &self.definition.generation())
            .field("nodes", &self.definition.nodes().len()).finish_non_exhaustive()
    }
}
pub type PolicyObservation = Knowledge<DisclosedPolicy, FilePrecheckBasis>;

/// Full-policy access must be granted by the original supervisor. Clones retain
/// the SAME weak owner, revocation and finite query allowance as exact checks.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::{
///     FileActorPrecheckPort, precheck_wire::policy::FileActorPolicyPort};
/// fn no_upgrade(port: FileActorPrecheckPort) -> FileActorPolicyPort { port }
/// ```
#[derive(Clone)]
pub struct FileActorPolicyPort { precheck: FileActorPrecheckPort }
impl fmt::Debug for FileActorPolicyPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileActorPolicyPort { .. }") }
}
impl FileActorSupervisor<FileOversight> {
    /// Explicitly disclose policy structure AND its literal bytes. An operator
    /// must review those literals for secrets before granting this capability.
    /// No existing port is upgraded. Source/revision eligibility, optional cache
    /// owner binding and grant limits are exactly the original precheck checks.
    pub fn disclose_full_precheck(&self, revision: u64, disclosure: u64, queries: usize,
        cache: Option<&FilePrecheckCache>,
    ) -> Result<(FileActorPolicyPort, FilePrecheckDisclosure), JournalError> {
        let (precheck, grant) = match cache {
            Some(cache) => self.disclose_exact_precheck_cached(revision, disclosure, queries, cache)?,
            None => self.disclose_exact_precheck(revision, disclosure, queries)?,
        };
        Ok((FileActorPolicyPort { precheck }, grant))
    }
}
impl FileActorPolicyPort {
    /// Deliberate narrowing: this handle cannot be converted back into full access.
    pub fn prechecks(&self) -> FileActorPrecheckPort { self.precheck.clone() }

    /// Same charged discovery read, describing this stronger capability rather
    /// than the deliberately narrowed exact-only handle returned by prechecks().
    pub fn capabilities(&self, query: u64, expected_policy_epoch: u64)
        -> Result<super::discovery::PrecheckCapabilitiesObservation, ActorError>
    {
        let mut observation = self.precheck.capabilities(query, expected_policy_epoch)?;
        if let Knowledge::Known { value, .. } = &mut observation { value.disclose_definition(); }
        Ok(observation)
    }

    /// One entered read is charged before validation/cloning. No evaluator, clock,
    /// source read, helper, admission snapshot, journal or effect is touched.
    pub fn policy(&self, query: u64, expected_policy_epoch: u64)
        -> Result<PolicyObservation, ActorError>
    {
        self.precheck.with_disclosure(|grant| {
            if query == 0 { return Err(ActorError::MalformedProposal); }
            if expected_policy_epoch != grant.projection.epoch {
                return Ok(Knowledge::Stale {
                    generation: expected_policy_epoch, current: grant.projection.epoch,
                });
            }
            Ok(Knowledge::Known {
                value: DisclosedPolicy { definition: grant.projection.policy.clone() },
                basis: FilePrecheckBasis { disclosure: grant.id, query,
                    policy_epoch: grant.projection.epoch, observed_at: grant.projection.now },
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyRequest { pub query: u64, pub expected_policy_epoch: u64 }
impl PolicyRequest {
    pub fn encode(self) -> Result<Vec<u8>, PrecheckWireError> {
        if self.query == 0 { return Err(PrecheckWireError::MalformedRequest); }
        Ok(format!(concat!("{{\"schema\":\"fa.actor-policy/1\",\"operation\":\"get\",",
            "\"query\":\"{}\",\"expected_policy_epoch\":\"{}\"}}"),
            self.query, self.expected_policy_epoch).into_bytes())
    }
}
pub fn decode_policy_request(bytes: &[u8]) -> Result<PolicyRequest, PrecheckWireError> {
    let parsed = strict_json::parse(bytes, Limits { max_bytes: MAX_POLICY_REQUEST_BYTES,
        max_depth: 2, max_items: 16, max_string_bytes: 64 }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit =>
                PrecheckWireError::Capacity,
            _ => PrecheckWireError::MalformedRequest,
        })?;
    decode_request_value(&parsed)
}
pub(super) fn decode_request_value(parsed: &Json) -> Result<PolicyRequest, PrecheckWireError> {
    let decode = || {
        let root = parsed.as_object().ok_or(PrecheckResponseError::Malformed)?;
        if text(root, "schema")? != REQUEST_SCHEMA { return Err(PrecheckResponseError::UnsupportedVersion); }
        fields(root, &["schema", "operation", "query", "expected_policy_epoch"])?;
        if text(root, "operation")? != "get" { return Err(PrecheckResponseError::Malformed); }
        Ok(PolicyRequest { query: decimal(root, "query", false)?,
            expected_policy_epoch: decimal(root, "expected_policy_epoch", true)? })
    };
    decode().map_err(|error| match error {
        PrecheckResponseError::UnsupportedVersion => PrecheckWireError::UnsupportedVersion,
        PrecheckResponseError::Capacity => PrecheckWireError::Capacity,
        _ => PrecheckWireError::MalformedRequest,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyResponse {
    pub query: Option<u64>,
    pub result: Result<PolicyObservation, PrecheckWireError>,
}
impl PolicyResponse {
    pub fn encode(&self) -> Result<Vec<u8>, PrecheckResponseError> {
        encode_envelope_bounded(RESPONSE_SCHEMA, "informational", self.query, &self.result,
            MAX_POLICY_RESPONSE_BYTES, |value| Ok(format!(concat!(
                "\"value\":{{\"policy_disclosure\":\"full\",\"scope\":\"exact_policy_definition\",",
                "\"source_values\":\"withheld\",\"helper_feedback\":\"withheld\",",
                "\"consequences\":\"withheld\",\"effect_authority\":false,",
                "\"review_required\":true,\"definition\":{}}}"),
                definition::encode(value.definition())?)))
    }
}
pub fn decode_policy_response(bytes: &[u8], query: u64, expected_policy_epoch: u64)
    -> Result<PolicyResponse, PrecheckResponseError>
{
    let (query, result) = decode_envelope_bounded(bytes, query, expected_policy_epoch,
        RESPONSE_SCHEMA, "informational", Limits { max_bytes: MAX_POLICY_RESPONSE_BYTES,
            max_depth: 8, max_items: 4_096, max_string_bytes: 131_072 }, |known| {
            fields(known, &["state", "value", "basis"])?;
            let value = object(known, "value")?;
            fields(value, &["policy_disclosure", "scope", "source_values", "helper_feedback",
                "consequences", "effect_authority", "review_required", "definition"])?;
            for (key, expected) in [("policy_disclosure", "full"), ("scope", "exact_policy_definition"),
                ("source_values", "withheld"), ("helper_feedback", "withheld"), ("consequences", "withheld")] {
                if text(value, key)? != expected { return Err(PrecheckResponseError::Malformed); }
            }
            if value.get("effect_authority").and_then(Json::as_bool) != Some(false)
                || value.get("review_required").and_then(Json::as_bool) != Some(true) {
                return Err(PrecheckResponseError::Malformed);
            }
            Ok(DisclosedPolicy { definition: definition::decode(object(value, "definition")?)? })
        })?;
    Ok(PolicyResponse { query, result })
}

#[cfg(test)]
mod tests;
