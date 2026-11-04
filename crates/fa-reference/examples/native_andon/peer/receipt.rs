//! Request/receipt correlation on the already kernel-verified socket. This is
//! not a signature, evaluator authority, or a replacement native result engine.
use super::{
    ASSESSMENT_BYTES, EvaluatorPeerProfile, Json, Limits, mutations, strict_json, transport,
};
use crate::config::{debug, hex};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const REQUEST_BYTES: usize = 20 * 1024;

pub(super) struct Request {
    wire: Vec<u8>,
    parsed: Json,
    mutation: mutations::Mutation,
}
impl Request {
    /// Original assessment bytes are losslessly framed, not rewritten or signed.
    pub(super) fn prepare(peer: &EvaluatorPeerProfile, assessment: &[u8]) -> Result<Self, String> {
        mutations::Mutation::decode("assess", assessment)?;
        let hex: String = assessment
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let wire = format!("{{\"schema_version\":\"fa.native-evaluator-request/1\",\"audience\":{},\"assessment_hex\":\"{hex}\"}}",
            audience(peer)).into_bytes();
        Self::accept(peer, &wire)
    }
    /// Reject another deployment/scope BEFORE native recovery or assessment.
    pub(super) fn accept(peer: &EvaluatorPeerProfile, wire: &[u8]) -> Result<Self, String> {
        let parsed = parse(wire, REQUEST_BYTES)?;
        let fields = fields(&parsed, &["schema_version", "audience", "assessment_hex"])?;
        text(fields, "schema_version", "fa.native-evaluator-request/1")?;
        if fields["audience"] != parse(audience(peer).as_bytes(), REQUEST_BYTES)? {
            return Err("evaluator request audience mismatch; no assessment admitted".into());
        }
        let encoded = fields["assessment_hex"]
            .as_str()
            .ok_or("assessment_hex must be text")?;
        if encoded.len() > ASSESSMENT_BYTES * 2 {
            return Err("assessment exceeds byte limit".into());
        }
        let bytes = hex(encoded)?;
        let mutation = mutations::Mutation::decode("assess", &bytes)?;
        Ok(Self {
            wire: wire.to_vec(),
            parsed,
            mutation,
        })
    }
    pub(super) fn wire(&self) -> &[u8] {
        &self.wire
    }
    pub(super) fn mutation(&self) -> &mutations::Mutation {
        &self.mutation
    }
}

/// A missing report remains unconfirmed, including uncertain persistence. A
/// known applied label with failed retention remains applied-but-incomplete.
pub(super) fn capture(
    request: &Request,
    native: &[u8],
    completed: bool,
) -> Result<Vec<u8>, String> {
    let native = if native.is_empty() {
        "null"
    } else {
        validate_native(&parse(native, transport::MAX_MESSAGE_BYTES)?)?;
        std::str::from_utf8(native).map_err(debug)?
    };
    let wire = format!("{{\"schema_version\":\"fa.native-evaluator-receipt/1\",\"request\":{},\"native_completed\":{completed},\"native_result\":{native}}}",
        std::str::from_utf8(request.wire()).map_err(debug)?).into_bytes();
    // Use the SAME check as the client before reporting anything as completed.
    validate(request, &wire)?;
    Ok(wire)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Applied,
    AlreadyApplied,
    Refused,
    Unconfirmed,
    Incomplete,
}
impl Outcome {
    pub(super) fn completed(&self) -> bool {
        matches!(self, Self::Applied | Self::AlreadyApplied)
    }
    pub(super) fn error(&self) -> &'static str {
        match self {
            Self::Refused => "assessment refused; retain the acknowledged native report",
            Self::Unconfirmed => {
                "assessment completion unconfirmed; inspect original history before an explicit retry"
            }
            Self::Incomplete => {
                "assessment recorded but invocation incomplete; preserve original history and anchor custody"
            }
            Self::Applied | Self::AlreadyApplied => "assessment completed",
        }
    }
}

pub(super) fn validate(expected: &Request, wire: &[u8]) -> Result<Outcome, String> {
    let parsed = parse(wire, transport::MAX_MESSAGE_BYTES)?;
    let outer = fields(
        &parsed,
        &[
            "schema_version",
            "request",
            "native_completed",
            "native_result",
        ],
    )?;
    text(outer, "schema_version", "fa.native-evaluator-receipt/1")?;
    if outer["request"] != expected.parsed {
        return Err("evaluator receipt does not match the submitted request".into());
    }
    let completed = outer["native_completed"]
        .as_bool()
        .ok_or("invalid native completion flag")?;
    if outer["native_result"].is_null() {
        return if completed {
            Err("completion requires an acknowledged native result".into())
        } else {
            Ok(Outcome::Unconfirmed)
        };
    }
    let (status, retained) = validate_native(&outer["native_result"])?;
    if completed && (status == "refused" || !retained) {
        return Err("contradictory native completion report".into());
    }
    Ok(match (status, completed) {
        ("applied", true) => Outcome::Applied,
        ("already_applied", true) => Outcome::AlreadyApplied,
        ("refused", _) => Outcome::Refused,
        _ => Outcome::Incomplete,
    })
}

fn validate_native(native: &Json) -> Result<(&'static str, bool), String> {
    let root = fields(
        native,
        &[
            "schema_version",
            "status",
            "recovery_fence_revision",
            "state",
        ],
    )?;
    text(root, "schema_version", "fa.native-andon-mutation-result/1")?;
    let status = match root["status"].as_str() {
        Some("applied") => "applied",
        Some("already_applied") => "already_applied",
        Some("refused") => "refused",
        _ => return Err("unknown native assessment status".into()),
    };
    let fence = decimal64(&root["recovery_fence_revision"])?;
    let state = fields(
        &root["state"],
        &[
            "schema_version",
            "operation",
            "historical",
            "history_anchor_retained",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "evaluation_revision",
            "pending_cases",
            "censored_cases",
            "windows",
        ],
    )?;
    text(state, "schema_version", "fa.native-andon-result/1")?;
    text(state, "operation", "assess")?;
    if state["historical"].as_bool() != Some(false) {
        return Err("historical data is not an assessment receipt".into());
    }
    let retained = state["history_anchor_retained"]
        .as_bool()
        .ok_or("invalid anchor retention flag")?;
    let revision = decimal64(&state["journal_revision"])?;
    if fence == 0
        || revision < fence
        || (status == "applied" && revision == fence)
        || (status == "already_applied" && revision != fence)
    {
        return Err("invalid native acknowledgment revisions".into());
    }
    for key in ["control_sequence", "authority_epoch", "evaluation_revision"] {
        decimal64(&state[key])?;
    }
    for key in ["pending_cases", "censored_cases"] {
        state[key].as_u64().ok_or("invalid native case count")?;
    }
    let windows = state["windows"]
        .as_array()
        .ok_or("invalid native window inventory")?;
    if windows.is_empty() {
        return Err("missing native accounting windows".into());
    }
    let mut seen = BTreeSet::new();
    for window in windows {
        let window = fields(
            window,
            &[
                "window",
                "false_stops",
                "total_cost",
                "incident_opened_by_round",
            ],
        )?;
        let id = decimal64(&window["window"])?;
        if id == 0 || !seen.insert(id) {
            return Err("invalid or duplicate accounting window".into());
        }
        let count = decimal64(&window["false_stops"])?;
        let cost = decimal(&window["total_cost"])?;
        if count == 0 && cost != 0 {
            return Err("invalid zero-stop accounting cost".into());
        }
        if !window["incident_opened_by_round"].is_null()
            && (decimal64(&window["incident_opened_by_round"])? == 0 || count == 0)
        {
            return Err("invalid incident provenance".into());
        }
    }
    Ok((status, retained))
}
fn audience(peer: &EvaluatorPeerProfile) -> String {
    let scope = peer.scope;
    format!(
        "{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{},\"deployment\":{},\"evaluator\":{}}}",
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        peer.deployment,
        peer.evaluator_id
    )
}
fn parse(bytes: &[u8], maximum: usize) -> Result<Json, String> {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: maximum,
            max_depth: 8,
            max_items: 8192,
            max_string_bytes: ASSESSMENT_BYTES * 2,
        },
    )
    .map_err(|_| "invalid evaluator frame JSON".into())
}
fn fields<'a>(value: &'a Json, keys: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let object = value
        .as_object()
        .ok_or("expected evaluator result object")?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err("missing or unknown evaluator result fields".into());
    }
    Ok(object)
}
fn text(object: &BTreeMap<String, Json>, key: &str, expected: &str) -> Result<(), String> {
    if object[key].as_str() == Some(expected) {
        Ok(())
    } else {
        Err("wrong evaluator result schema or operation".into())
    }
}
fn decimal(value: &Json) -> Result<u128, String> {
    let text = value.as_str().ok_or("expected native decimal string")?;
    let number = text.parse::<u128>().map_err(|_| "invalid native decimal")?;
    if number.to_string() != text {
        return Err("noncanonical native decimal".into());
    }
    Ok(number)
}
fn decimal64(value: &Json) -> Result<u64, String> {
    u64::try_from(decimal(value)?).map_err(|_| "native counter exceeds u64".into())
}

#[cfg(test)]
#[path = "receipt/tests.rs"]
mod tests;
