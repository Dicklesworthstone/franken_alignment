//! Closed structural codec for the ORIGINAL Policy, not another evaluator.
use super::super::response::{Object, decimal, fields, object, text};
use super::{Policy, PrecheckResponseError};
use crate::action::ResolvedTarget;
use crate::action::consequence::gate::containment::session::policy::{
    Predicate, MAX_POLICY_NODES, MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES,
};
use crate::{Error, strict_json::Json};

pub(super) fn encode(policy: &Policy) -> Result<String, PrecheckResponseError> {
    let mut nodes = Vec::with_capacity(policy.nodes().len());
    for node in policy.nodes() {
        nodes.push(match node {
            Predicate::TargetIs(target) => format!(concat!(
                "{{\"op\":\"target_is\",\"target\":{{\"adapter\":\"{}\",\"object\":\"{}\",",
                "\"contract_version\":\"{}\",\"expected_version\":\"{}\",\"generation\":\"{}\"}}}}"),
                target.adapter, target.object, target.contract_version, target.expected_version, target.generation),
            Predicate::PayloadIs(bytes) => format!("{{\"op\":\"payload_is\",\"value_hex\":\"{}\"}}", hex(bytes)),
            Predicate::PayloadAtMost(limit) => format!("{{\"op\":\"payload_at_most\",\"maximum\":\"{limit}\"}}"),
            Predicate::UnitsAtMost(limit) => format!("{{\"op\":\"units_at_most\",\"maximum\":\"{limit}\"}}"),
            Predicate::ExactValue { key, value } => format!("{{\"op\":\"exact_value\",\"key\":\"{key}\",\"value_hex\":\"{}\"}}", hex(value)),
            Predicate::Absent { key } => format!("{{\"op\":\"absent\",\"key\":\"{key}\"}}"),
            Predicate::EmptyRange { start, end } => format!("{{\"op\":\"empty_range\",\"start\":\"{start}\",\"end\":\"{end}\"}}"),
            Predicate::All(children) | Predicate::Any(children) => {
                let op = if matches!(node, Predicate::All(_)) { "all" } else { "any" };
                let children = children.iter().map(|index| format!("\"{index}\"")).collect::<Vec<_>>().join(",");
                format!("{{\"op\":\"{op}\",\"children\":[{children}]}}")
            }
            Predicate::Not(child) => format!("{{\"op\":\"not\",\"child\":\"{child}\"}}"),
        });
    }
    Ok(format!("{{\"generation\":\"{}\",\"nodes\":[{}]}}", policy.generation(), nodes.join(",")))
}

pub(super) fn decode(root: &Object) -> Result<Policy, PrecheckResponseError> {
    fields(root, &["generation", "nodes"])?;
    let generation = decimal(root, "generation", false)?;
    let input = root.get("nodes").and_then(Json::as_array).ok_or(PrecheckResponseError::Malformed)?;
    if input.len() > MAX_POLICY_NODES { return Err(PrecheckResponseError::Capacity); }
    let mut nodes = Vec::with_capacity(input.len());
    let mut literal_bytes = 0;
    let mut edges = 0;
    for value in input {
        let node = value.as_object().ok_or(PrecheckResponseError::Malformed)?;
        let op = text(node, "op")?;
        nodes.push(match op {
            "target_is" => {
                fields(node, &["op", "target"])?;
                let target = object(node, "target")?;
                fields(target, &["adapter", "object", "contract_version", "expected_version", "generation"])?;
                Predicate::TargetIs(ResolvedTarget {
                    adapter: decimal(target, "adapter", false)?, object: decimal(target, "object", false)?,
                    contract_version: decimal(target, "contract_version", false)?,
                    expected_version: decimal(target, "expected_version", false)?,
                    generation: decimal(target, "generation", false)?,
                })
            }
            "payload_is" => {
                fields(node, &["op", "value_hex"])?;
                Predicate::PayloadIs(unhex(text(node, "value_hex")?, &mut literal_bytes)?)
            }
            "payload_at_most" => {
                fields(node, &["op", "maximum"])?;
                Predicate::PayloadAtMost(index(decimal(node, "maximum", true)?)?)
            }
            "units_at_most" => {
                fields(node, &["op", "maximum"])?;
                Predicate::UnitsAtMost(decimal(node, "maximum", true)?)
            }
            "exact_value" => {
                fields(node, &["op", "key", "value_hex"])?;
                Predicate::ExactValue { key: decimal(node, "key", true)?,
                    value: unhex(text(node, "value_hex")?, &mut literal_bytes)? }
            }
            "absent" => {
                fields(node, &["op", "key"])?;
                Predicate::Absent { key: decimal(node, "key", true)? }
            }
            "empty_range" => {
                fields(node, &["op", "start", "end"])?;
                Predicate::EmptyRange { start: decimal(node, "start", true)?, end: decimal(node, "end", true)? }
            }
            "all" | "any" => {
                fields(node, &["op", "children"])?;
                let input = node.get("children").and_then(Json::as_array).ok_or(PrecheckResponseError::Malformed)?;
                charge(&mut edges, input.len(), MAX_POLICY_EDGES)?;
                let children = input.iter().map(|value| {
                    let raw = value.as_str().ok_or(PrecheckResponseError::Malformed)?;
                    if raw.is_empty() || raw.len() > 20 || !raw.bytes().all(|byte| byte.is_ascii_digit())
                        || (raw.len() > 1 && raw.starts_with('0')) { return Err(PrecheckResponseError::Malformed); }
                    index(raw.parse::<u64>().map_err(|_| PrecheckResponseError::Malformed)?)
                }).collect::<Result<Vec<_>, _>>()?;
                if op == "all" { Predicate::All(children) } else { Predicate::Any(children) }
            }
            "not" => {
                fields(node, &["op", "child"])?;
                charge(&mut edges, 1, MAX_POLICY_EDGES)?;
                Predicate::Not(index(decimal(node, "child", true)?)?)
            }
            _ => return Err(PrecheckResponseError::Malformed),
        });
    }
    // Original graph validator rejects cycles, forward edges, unreachable nodes,
    // zero generation, invalid ranges and too many state-read occurrences.
    Policy::new(generation, nodes).map_err(|error| match error {
        Error::Limit | Error::Overflow => PrecheckResponseError::Capacity,
        _ => PrecheckResponseError::Malformed,
    })
}
fn index(value: u64) -> Result<usize, PrecheckResponseError> {
    usize::try_from(value).map_err(|_| PrecheckResponseError::Capacity)
}
fn charge(total: &mut usize, count: usize, maximum: usize) -> Result<(), PrecheckResponseError> {
    let next = total.checked_add(count).ok_or(PrecheckResponseError::Capacity)?;
    if next > maximum { return Err(PrecheckResponseError::Capacity); }
    *total = next; Ok(())
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
fn unhex(text: &str, total: &mut usize) -> Result<Vec<u8>, PrecheckResponseError> {
    if !text.len().is_multiple_of(2) { return Err(PrecheckResponseError::Malformed); }
    charge(total, text.len() / 2, MAX_POLICY_LITERAL_BYTES)?;
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'), b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(PrecheckResponseError::Malformed),
    };
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) { bytes.push((digit(pair[0])? << 4) | digit(pair[1])?); }
    Ok(bytes)
}
