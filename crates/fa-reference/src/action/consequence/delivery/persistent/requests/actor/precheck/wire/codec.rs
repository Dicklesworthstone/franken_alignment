//! Exact decimal identifiers and lower-case hexadecimal payloads, no new parser.
use super::{MAX_PRECHECK_FRAME_BYTES, PrecheckWireError};
use super::discovery::{self, PrecheckCapabilitiesRequest, MAX_CAPABILITIES_REQUEST_BYTES};
use super::policy::{self, PolicyRequest, MAX_POLICY_REQUEST_BYTES};
use crate::action::{ElapsedTick, MAX_PAYLOAD_BYTES, ResolvedTarget};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecheckRequest {
    /// Read correlation only: this label reserves no actor request ID.
    pub query: u64,
    pub proposal: ActorProposal,
}

pub(super) enum WireRequest {
    Precheck(PrecheckRequest),
    Capabilities(PrecheckCapabilitiesRequest),
    Policy(PolicyRequest),
}

// The mixed endpoint parses once under its ORIGINAL finite frame allowance.
// Individual grammars remain closed; capability documents also retain their
// smaller byte ceiling. No parse error can echo a partially decoded query.
pub(super) fn decode_wire_request(bytes: &[u8]) -> Result<WireRequest, PrecheckWireError> {
    let parsed = parse_request(bytes)?;
    let root = parsed.as_object().ok_or(PrecheckWireError::MalformedRequest)?;
    if text(root, "schema")? == discovery::REQUEST_SCHEMA {
        if bytes.len() > MAX_CAPABILITIES_REQUEST_BYTES { return Err(PrecheckWireError::Capacity); }
        discovery::decode_request_value(&parsed).map(WireRequest::Capabilities)
    } else if text(root, "schema")? == policy::REQUEST_SCHEMA {
        if bytes.len() > MAX_POLICY_REQUEST_BYTES { return Err(PrecheckWireError::Capacity); }
        policy::decode_request_value(&parsed).map(WireRequest::Policy)
    } else {
        decode_precheck_value(&parsed).map(WireRequest::Precheck)
    }
}

pub fn decode_precheck_request(bytes: &[u8]) -> Result<PrecheckRequest, PrecheckWireError> {
    decode_precheck_value(&parse_request(bytes)?)
}

fn parse_request(bytes: &[u8]) -> Result<Json, PrecheckWireError> {
    strict_json::parse(bytes, Limits {
        max_bytes: MAX_PRECHECK_FRAME_BYTES, max_depth: 4, max_items: 64,
        max_string_bytes: MAX_PAYLOAD_BYTES * 2,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit =>
            PrecheckWireError::Capacity,
        _ => PrecheckWireError::MalformedRequest,
    })
}

fn decode_precheck_value(parsed: &Json) -> Result<PrecheckRequest, PrecheckWireError> {
    let root = parsed.as_object().ok_or(PrecheckWireError::MalformedRequest)?;
    let schema = text(root, "schema")?;
    if schema != "fa.actor-precheck/1" { return Err(PrecheckWireError::UnsupportedVersion); }
    fields(root, &["schema", "operation", "query", "target", "payload_hex", "units",
        "deadline", "expected_policy_epoch"])?;
    if text(root, "operation")? != "precheck" { return Err(PrecheckWireError::MalformedRequest); }
    let query = decimal(root, "query", false)?;
    let target = root.get("target").and_then(Json::as_object).ok_or(PrecheckWireError::MalformedRequest)?;
    fields(target, &["adapter", "object", "contract_version", "expected_version", "generation"])?;
    let proposal = ActorProposal {
        target: ResolvedTarget {
            adapter: decimal(target, "adapter", false)?, object: decimal(target, "object", false)?,
            contract_version: decimal(target, "contract_version", false)?,
            expected_version: decimal(target, "expected_version", false)?,
            generation: decimal(target, "generation", false)?,
        },
        payload: unhex(text(root, "payload_hex")?)?,
        units: decimal(root, "units", false)?,
        deadline: ElapsedTick(decimal(root, "deadline", false)?),
        expected_policy_epoch: decimal(root, "expected_policy_epoch", true)?,
    };
    proposal.validate().map_err(PrecheckWireError::from)?;
    Ok(PrecheckRequest { query, proposal })
}

pub fn encode_precheck_request(request: &PrecheckRequest) -> Result<Vec<u8>, PrecheckWireError> {
    if request.query == 0 { return Err(PrecheckWireError::MalformedRequest); }
    request.proposal.validate().map_err(PrecheckWireError::from)?;
    let proposal = &request.proposal;
    let target = proposal.target;
    let json = format!(concat!(
        "{{\"schema\":\"fa.actor-precheck/1\",\"operation\":\"precheck\",\"query\":\"{}\",",
        "\"target\":{{\"adapter\":\"{}\",\"object\":\"{}\",\"contract_version\":\"{}\",",
        "\"expected_version\":\"{}\",\"generation\":\"{}\"}},\"payload_hex\":\"{}\",",
        "\"units\":\"{}\",\"deadline\":\"{}\",\"expected_policy_epoch\":\"{}\"}}"),
        request.query, target.adapter, target.object, target.contract_version,
        target.expected_version, target.generation, hex(&proposal.payload), proposal.units,
        proposal.deadline.0, proposal.expected_policy_epoch);
    Ok(json.into_bytes())
}

fn fields(object: &BTreeMap<String, Json>, names: &[&str]) -> Result<(), PrecheckWireError> {
    if object.len() != names.len() || names.iter().any(|name| !object.contains_key(*name)) {
        return Err(PrecheckWireError::MalformedRequest);
    }
    Ok(())
}
fn text<'a>(object: &'a BTreeMap<String, Json>, name: &str) -> Result<&'a str, PrecheckWireError> {
    object.get(name).and_then(Json::as_str).ok_or(PrecheckWireError::MalformedRequest)
}
fn decimal(object: &BTreeMap<String, Json>, name: &str, zero: bool) -> Result<u64, PrecheckWireError> {
    let value = text(object, name)?;
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0')) { return Err(PrecheckWireError::MalformedRequest); }
    let number = value.parse::<u64>().map_err(|_| PrecheckWireError::MalformedRequest)?;
    if !zero && number == 0 { return Err(PrecheckWireError::MalformedRequest); }
    Ok(number)
}
fn hex(bytes: &[u8]) -> String {
    let digits = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(digits[usize::from(byte >> 4)]));
        text.push(char::from(digits[usize::from(byte & 15)]));
    }
    text
}
fn unhex(text: &str) -> Result<Vec<u8>, PrecheckWireError> {
    if text.len() > MAX_PAYLOAD_BYTES * 2 { return Err(PrecheckWireError::Capacity); }
    if !text.len().is_multiple_of(2) { return Err(PrecheckWireError::MalformedRequest); }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(PrecheckWireError::MalformedRequest),
    };
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) { bytes.push((digit(pair[0])? << 4) | digit(pair[1])?); }
    Ok(bytes)
}
