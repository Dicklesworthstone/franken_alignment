//! The existing bounded exact-policy graph in the Tower's lossless wire syntax.
//! Every integer is a canonical decimal string. Parsing creates immutable policy
//! data only; native corpus selection and governance custody stay in the owner.

use super::{
    DecodeResult, Json, MAX_REQUEST_ITEMS, TowerDecodingError, array_json, charged_hex, decimal,
    decimal_text, decode_target, encode_target, field, fields, hex, object, object_json, quote,
    text,
};
use crate::action::consequence::gate::containment::session::policy::{
    MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES, MAX_POLICY_NODES, Policy, Predicate,
};

pub(in crate::tower) fn decode(value: &Json) -> DecodeResult<Policy> {
    let value = object(value)?;
    fields(value, &["generation", "nodes"])?;
    let generation = decimal(value, "generation")?;
    let raw = field(value, "nodes")?
        .as_array()
        .ok_or(TowerDecodingError::MalformedRequest)?;
    if raw.len() > MAX_POLICY_NODES {
        return Err(TowerDecodingError::Capacity);
    }
    let mut literals = MAX_POLICY_LITERAL_BYTES;
    let mut edges = MAX_POLICY_EDGES;
    let mut nodes = Vec::with_capacity(raw.len());
    for node in raw {
        let node = object(node)?;
        let predicate = match text(node, "op")? {
            "target_is" => {
                fields(node, &["op", "target"])?;
                Predicate::TargetIs(decode_target(field(node, "target")?)?)
            }
            "payload_is" => {
                fields(node, &["op", "value_hex"])?;
                Predicate::PayloadIs(charged_hex(text(node, "value_hex")?, &mut literals)?)
            }
            "payload_at_most" => {
                fields(node, &["op", "maximum"])?;
                Predicate::PayloadAtMost(index(decimal(node, "maximum")?)?)
            }
            "units_at_most" => {
                fields(node, &["op", "maximum"])?;
                Predicate::UnitsAtMost(decimal(node, "maximum")?)
            }
            "exact_value" => {
                fields(node, &["op", "key", "value_hex"])?;
                Predicate::ExactValue {
                    key: decimal(node, "key")?,
                    value: charged_hex(text(node, "value_hex")?, &mut literals)?,
                }
            }
            "absent" => {
                fields(node, &["op", "key"])?;
                Predicate::Absent {
                    key: decimal(node, "key")?,
                }
            }
            "empty_range" => {
                fields(node, &["op", "start", "end"])?;
                Predicate::EmptyRange {
                    start: decimal(node, "start")?,
                    end: decimal(node, "end")?,
                }
            }
            "all" | "any" => {
                fields(node, &["op", "children"])?;
                let children = field(node, "children")?
                    .as_array()
                    .ok_or(TowerDecodingError::MalformedRequest)?;
                edges = edges
                    .checked_sub(children.len())
                    .ok_or(TowerDecodingError::Capacity)?;
                let children = children
                    .iter()
                    .map(|value| {
                        let value = value.as_str().ok_or(TowerDecodingError::MalformedRequest)?;
                        index(decimal_text(value)?)
                    })
                    .collect::<DecodeResult<Vec<_>>>()?;
                if text(node, "op")? == "all" {
                    Predicate::All(children)
                } else {
                    Predicate::Any(children)
                }
            }
            "not" => {
                fields(node, &["op", "child"])?;
                edges = edges.checked_sub(1).ok_or(TowerDecodingError::Capacity)?;
                Predicate::Not(index(decimal(node, "child")?)?)
            }
            _ => return Err(TowerDecodingError::MalformedRequest),
        };
        nodes.push(predicate);
    }
    // The original constructor validates topological ordering, reachability,
    // positive target identities and every native policy bound.
    Policy::new(generation, nodes).map_err(|error| match error {
        crate::Error::Limit | crate::Error::Overflow => TowerDecodingError::Capacity,
        _ => TowerDecodingError::MalformedRequest,
    })
}

fn index(value: u64) -> DecodeResult<usize> {
    usize::try_from(value).map_err(|_| TowerDecodingError::Capacity)
}

/// A valid native graph can still exceed the existing request's JSON item cap.
/// Check the exact fixed encoding before retaining or encoding a typed request;
/// never enlarge the common transport limits for a candidate policy.
pub(in crate::tower) fn validate_wire(policy: &Policy) -> DecodeResult<()> {
    validate_items(policy, 23)
}

pub(in crate::tower) fn validate_paged_wire(policy: &Policy) -> DecodeResult<()> {
    // The explicit paged selector adds its key and boolean value.
    validate_items(policy, 25)
}

fn validate_items(policy: &Policy, fixed: usize) -> DecodeResult<()> {
    let node_items: usize = policy
        .nodes()
        .iter()
        .map(|node| match node {
            Predicate::TargetIs(_) => 15,
            Predicate::ExactValue { .. } | Predicate::EmptyRange { .. } => 7,
            Predicate::All(children) | Predicate::Any(children) => 5 + children.len(),
            _ => 5,
        })
        .sum();
    if fixed + node_items > MAX_REQUEST_ITEMS {
        Err(TowerDecodingError::Capacity)
    } else {
        Ok(())
    }
}

pub(in crate::tower) fn encode(policy: &Policy) -> String {
    object_json(&[
        ("generation", number(policy.generation())),
        ("nodes", array_json(policy.nodes().iter().map(predicate))),
    ])
}

fn predicate(node: &Predicate) -> String {
    match node {
        Predicate::TargetIs(target) => object_json(&[
            ("op", quote("target_is")),
            ("target", encode_target(target)),
        ]),
        Predicate::PayloadIs(bytes) => object_json(&[
            ("op", quote("payload_is")),
            ("value_hex", quote(&hex(bytes))),
        ]),
        Predicate::PayloadAtMost(maximum) => object_json(&[
            ("op", quote("payload_at_most")),
            ("maximum", number(maximum)),
        ]),
        Predicate::UnitsAtMost(maximum) => {
            object_json(&[("op", quote("units_at_most")), ("maximum", number(maximum))])
        }
        Predicate::ExactValue { key, value } => object_json(&[
            ("op", quote("exact_value")),
            ("key", number(key)),
            ("value_hex", quote(&hex(value))),
        ]),
        Predicate::Absent { key } => object_json(&[("op", quote("absent")), ("key", number(key))]),
        Predicate::EmptyRange { start, end } => object_json(&[
            ("op", quote("empty_range")),
            ("start", number(start)),
            ("end", number(end)),
        ]),
        Predicate::All(children) | Predicate::Any(children) => object_json(&[
            (
                "op",
                quote(if matches!(node, Predicate::All(_)) {
                    "all"
                } else {
                    "any"
                }),
            ),
            ("children", array_json(children.iter().map(number))),
        ]),
        Predicate::Not(child) => object_json(&[("op", quote("not")), ("child", number(child))]),
    }
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}
