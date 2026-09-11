//! Exact decimal-string identifiers and hexadecimal payloads preserve all bits.
//! The existing strict parser supplies duplicate-key and allocation bounds.

use super::super::actor::{ActorError, ActorOutcome, ActorProposal, BasisSource, Knowledge, UnknownReason};
use crate::action::{ElapsedTick, MAX_PAYLOAD_BYTES, ResolvedTarget};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::collections::BTreeMap;

/// Excludes the transport newline. Includes room for every u64 at full width.
pub const MAX_FRAME_BYTES: usize = MAX_PAYLOAD_BYTES * 2 + 1_024;
pub const MAX_RESPONSE_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError { MalformedRequest, UnsupportedVersion, IdempotencyConflict, Capacity, Unavailable, Withheld }

impl From<ActorError> for WireError {
    fn from(error: ActorError) -> Self {
        match error {
            ActorError::MalformedProposal => Self::MalformedRequest,
            ActorError::IdempotencyConflict => Self::IdempotencyConflict,
            ActorError::Capacity => Self::Capacity,
            ActorError::Unavailable => Self::Unavailable,
            ActorError::Withheld => Self::Withheld,
        }
    }
}

impl WireError {
    fn name(self) -> &'static str {
        match self {
            Self::MalformedRequest => "malformed_request",
            Self::UnsupportedVersion => "unsupported_version",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::Capacity => "capacity",
            Self::Unavailable => "unavailable",
            Self::Withheld => "withheld",
        }
    }
}

/// These commands carry only the capabilities already exposed by ActorPort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Submit { request: u64, proposal: ActorProposal },
    Poll { request: u64 },
    Cancel { request: u64 },
}

impl Command {
    pub fn request(&self) -> u64 {
        match self { Self::Submit { request, .. } | Self::Poll { request } | Self::Cancel { request } => *request }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireResponse {
    pub request: Option<u64>,
    pub result: Result<Knowledge<ActorOutcome>, WireError>,
}

impl WireResponse {
    /// Fixed fields and enum spellings only: no user strings or internal errors
    /// are interpolated into JSON. Encoding cannot acquire or exercise authority.
    pub fn encode(&self) -> Vec<u8> {
        let request = self.request.map_or_else(|| "null".to_owned(), |id| format!("\"{id}\""));
        let body = match &self.result {
            Ok(knowledge) => format!("\"status\":\"ok\",\"knowledge\":{}", knowledge_json(knowledge)),
            Err(error) => format!("\"status\":\"error\",\"reason\":\"{}\"", error.name()),
        };
        format!("{{\"version\":1,\"request\":{request},{body}}}").into_bytes()
    }
}

pub fn decode_command(bytes: &[u8]) -> Result<Command, WireError> {
    let parsed = strict_json::parse(bytes, Limits {
        max_bytes: MAX_FRAME_BYTES, max_depth: 4, max_items: 64, max_string_bytes: MAX_PAYLOAD_BYTES * 2,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => WireError::Capacity,
        _ => WireError::MalformedRequest,
    })?;
    let root = parsed.as_object().ok_or(WireError::MalformedRequest)?;
    match root.get("version").and_then(Json::as_u64) {
        Some(1) => {}, Some(_) => return Err(WireError::UnsupportedVersion),
        None => return Err(WireError::MalformedRequest),
    }
    let request = decimal(root, "request", false)?;
    let operation = root.get("operation").and_then(Json::as_str).ok_or(WireError::MalformedRequest)?;
    let command = match operation {
        "submit" => {
            exact_fields(root, &["version", "operation", "request", "target", "payload_hex", "units", "deadline", "expected_policy_epoch"])?;
            let target = root.get("target").and_then(Json::as_object).ok_or(WireError::MalformedRequest)?;
            exact_fields(target, &["adapter", "object", "contract_version", "expected_version", "generation"])?;
            let target = ResolvedTarget {
                adapter: decimal(target, "adapter", false)?, object: decimal(target, "object", false)?,
                contract_version: decimal(target, "contract_version", false)?,
                expected_version: decimal(target, "expected_version", false)?, generation: decimal(target, "generation", false)?,
            };
            let text = root.get("payload_hex").and_then(Json::as_str).ok_or(WireError::MalformedRequest)?;
            let proposal = ActorProposal {
                target, payload: unhex(text)?, units: decimal(root, "units", false)?,
                deadline: ElapsedTick(decimal(root, "deadline", false)?),
                expected_policy_epoch: decimal(root, "expected_policy_epoch", true)?,
            };
            validate_proposal(&proposal)?;
            Command::Submit { request, proposal }
        }
        "poll" | "cancel" => {
            exact_fields(root, &["version", "operation", "request"])?;
            if operation == "poll" { Command::Poll { request } } else { Command::Cancel { request } }
        }
        _ => return Err(WireError::MalformedRequest),
    };
    Ok(command)
}

pub fn encode_command(command: &Command) -> Result<Vec<u8>, WireError> {
    let request = command.request();
    if request == 0 { return Err(WireError::MalformedRequest); }
    let json = match command {
        Command::Submit { proposal, .. } => {
            validate_proposal(proposal)?;
            let t = proposal.target;
            format!(concat!("{{\"version\":1,\"operation\":\"submit\",\"request\":\"{}\",",
                "\"target\":{{\"adapter\":\"{}\",\"object\":\"{}\",\"contract_version\":\"{}\",",
                "\"expected_version\":\"{}\",\"generation\":\"{}\"}},",
                "\"payload_hex\":\"{}\",\"units\":\"{}\",\"deadline\":\"{}\",\"expected_policy_epoch\":\"{}\"}}"),
                request, t.adapter, t.object, t.contract_version, t.expected_version, t.generation,
                hex(&proposal.payload), proposal.units, proposal.deadline.0, proposal.expected_policy_epoch)
        }
        Command::Poll { .. } => format!("{{\"version\":1,\"operation\":\"poll\",\"request\":\"{request}\"}}"),
        Command::Cancel { .. } => format!("{{\"version\":1,\"operation\":\"cancel\",\"request\":\"{request}\"}}"),
    };
    Ok(json.into_bytes())
}

fn validate_proposal(p: &ActorProposal) -> Result<(), WireError> {
    if [p.target.adapter, p.target.object, p.target.contract_version, p.target.expected_version,
        p.target.generation, p.units, p.deadline.0].contains(&0) { return Err(WireError::MalformedRequest); }
    if p.payload.len() > MAX_PAYLOAD_BYTES || p.payload.len() as u64 > p.units { return Err(WireError::Capacity); }
    Ok(())
}

fn exact_fields(object: &BTreeMap<String, Json>, names: &[&str]) -> Result<(), WireError> {
    if object.len() != names.len() || names.iter().any(|name| !object.contains_key(*name)) {
        return Err(WireError::MalformedRequest);
    }
    Ok(())
}

fn decimal(object: &BTreeMap<String, Json>, name: &str, zero: bool) -> Result<u64, WireError> {
    let text = object.get(name).and_then(Json::as_str).ok_or(WireError::MalformedRequest)?;
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0')) { return Err(WireError::MalformedRequest); }
    let value = text.parse::<u64>().map_err(|_| WireError::MalformedRequest)?;
    if !zero && value == 0 { return Err(WireError::MalformedRequest); }
    Ok(value)
}

fn hex(bytes: &[u8]) -> String {
    let digits = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes { text.push(char::from(digits[usize::from(byte >> 4)])); text.push(char::from(digits[usize::from(byte & 15)])); }
    text
}

fn unhex(text: &str) -> Result<Vec<u8>, WireError> {
    if text.len() > MAX_PAYLOAD_BYTES * 2 { return Err(WireError::Capacity); }
    if !text.len().is_multiple_of(2) { return Err(WireError::MalformedRequest); }
    let digit = |byte| match byte { b'0'..=b'9' => Ok(byte - b'0'), b'a'..=b'f' => Ok(byte - b'a' + 10), _ => Err(WireError::MalformedRequest) };
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) { bytes.push((digit(pair[0])? << 4) | digit(pair[1])?); }
    Ok(bytes)
}

fn knowledge_json(knowledge: &Knowledge<ActorOutcome>) -> String {
    match knowledge {
        Knowledge::Known { value, basis } => {
            let outcome = match value {
                ActorOutcome::NotAdmitted => "not_admitted", ActorOutcome::Denied => "denied",
                ActorOutcome::CancelledBeforeDispatch => "cancelled_before_dispatch", ActorOutcome::Executed => "executed",
                ActorOutcome::ConfirmedNotExecuted => "confirmed_not_executed",
            };
            let source = match basis.source { BasisSource::Intake => "intake", BasisSource::ControlLedger => "control_ledger" };
            format!("{{\"state\":\"known\",\"value\":\"{outcome}\",\"basis\":{{\"request\":\"{}\",\"generation\":\"{}\",\"source\":\"{source}\"}}}}", basis.request, basis.generation)
        }
        Knowledge::Pending { request } => format!("{{\"state\":\"pending\",\"request\":\"{request}\"}}"),
        Knowledge::Unknown { reason } => {
            let reason = match reason { UnknownReason::ControllerUnavailable => "controller_unavailable", UnknownReason::OutcomeUnknown => "outcome_unknown" };
            format!("{{\"state\":\"unknown\",\"reason\":\"{reason}\"}}")
        }
        Knowledge::Withheld { .. } => "{\"state\":\"withheld\",\"authority_required\":\"own_request\"}".to_owned(),
        Knowledge::Stale { generation, current } => format!("{{\"state\":\"stale\",\"generation\":\"{generation}\",\"current\":\"{current}\"}}"),
        Knowledge::Absent { closed_domain, frontier } => format!("{{\"state\":\"absent\",\"closed_domain\":\"{closed_domain}\",\"frontier\":\"{frontier}\"}}"),
    }
}
