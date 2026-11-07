//! Encode the original exact-policy admission, preserving both historical cuts.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAdmissionExplanation, FileExplanationBasis, FileExplanationResult,
};
use crate::action::consequence::gate::containment::session::policy::{Predicate, Truth};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::action::{ActionState, Purpose, ResolvedTarget};
use crate::address_resolver::{Address, Noun};

pub(super) fn encode(
    result: &FileExplanationResult,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let (field, value) = match result {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", known(value, *basis, address, encoder)?)
        }
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("unknown")),
                ("reason", quote("controller_unavailable")),
            ]),
        ),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_explanation",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_explanation")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-explanation/1")),
        ("operation", quote("explain")),
        ("scope", quote("historical_exact_policy_admission")),
        ("historical", "true".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("current_eligibility", quote("not_observed")),
        (
            "current_hold_reason",
            object_json(&[
                ("state", quote("unknown")),
                ("reason", quote("not_observed")),
            ]),
        ),
        (field, value),
    ]))
}

fn known(
    explanation: &FileAdmissionExplanation,
    basis: FileExplanationBasis,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let scope = basis.scope;
    let expected_subject = format!(
        "request-{}-{}-{}-{}-{}",
        scope.run, scope.principal, scope.branch, scope.authority, basis.request
    );
    let policy = explanation.policy();
    let evaluation = explanation.evaluation();
    let nodes = policy.nodes();
    let trace = evaluation.trace();
    // Both the value's native provenance and the request subject remain bound
    // even when a caller reconstructs the public outer response fields.
    if basis != explanation.basis()
        || address.kind != Noun::Attempt
        || address.tenant != scope.tenant.to_string()
        || address.id != expected_subject
        || scope.purpose != Purpose::Effect
        || nodes.is_empty()
        || trace.len() != nodes.len()
        || evaluation.generation() != policy.generation()
        || !matches!(
            explanation.initial_state(),
            ActionState::Reviewing | ActionState::Denied
        )
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut predicates = Vec::with_capacity(nodes.len());
    for (index, (predicate, step)) in nodes.iter().zip(trace).enumerate() {
        if step.node != index
            || step
                .witness
                .is_some_and(|witness| witness >= evaluation.witnesses().len())
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        predicates.push(
            encoder.bounded(object_json(&[
                ("node", index.to_string()),
                ("predicate", predicate_json(predicate)),
                ("result", quote(truth(step.result))),
                (
                    "witness",
                    step.witness
                        .map_or_else(|| "null".to_owned(), |value| value.to_string()),
                ),
            ]))?,
        );
    }
    let native_scope = object_json(&[
        ("tenant", number(scope.tenant)),
        ("principal", number(scope.principal)),
        ("run", number(scope.run)),
        ("branch", number(scope.branch)),
        ("authority", number(scope.authority)),
        ("purpose", quote("effect")),
    ]);
    let native_basis = object_json(&[
        ("scope", native_scope),
        ("origin", quote("acknowledged_owner")),
        ("request", number(basis.request)),
        ("attempt", number(basis.attempt)),
        ("admission_revision", number(basis.admission_revision)),
        (
            "admission_control_sequence",
            number(basis.admission_control_sequence),
        ),
        (
            "admitted_at",
            optional(basis.admitted_at.map(|tick| tick.0)),
        ),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        (
            "recorded_at",
            optional(basis.recorded_at.map(|tick| tick.0)),
        ),
    ]);
    let value = encoder.bounded(object_json(&[
        (
            "initial_state",
            quote(super::inspection::stage(explanation.initial_state())),
        ),
        (
            "ending_state",
            quote(super::inspection::stage(explanation.ending_state())),
        ),
        (
            "proposal_policy_epoch",
            number(explanation.proposal_policy_epoch()),
        ),
        ("policy_generation", number(policy.generation())),
        ("semantic_epoch", number(explanation.semantic_epoch())),
        (
            "target",
            explanation
                .target()
                .map_or_else(|| "null".to_owned(), target_json),
        ),
        ("units", number(explanation.units())),
        ("payload_bytes", explanation.payload_bytes().to_string()),
        ("result", quote(truth(evaluation.result()))),
        ("witness_count", evaluation.witnesses().len().to_string()),
        // The trace retains witness references. This profile does not serialize
        // observed source values, the action payload or a complete snapshot.
        ("witness_values_disclosed", "false".to_owned()),
        (
            "predicate_tree",
            object_json(&[
                ("root", (nodes.len() - 1).to_string()),
                ("nodes", format!("[{}]", predicates.join(","))),
            ]),
        ),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", native_basis),
        ("value", value),
    ]))
}

fn truth(value: Truth) -> &'static str {
    match value {
        Truth::Satisfied => "satisfied",
        Truth::Violated => "violated",
        Truth::Unknown => "unknown",
    }
}

fn predicate_json(predicate: &Predicate) -> String {
    match predicate {
        Predicate::TargetIs(target) => {
            object_json(&[("op", quote("target_is")), ("target", target_json(*target))])
        }
        Predicate::PayloadIs(bytes) => {
            object_json(&[("op", quote("payload_is")), ("value_hex", hex(bytes))])
        }
        Predicate::PayloadAtMost(maximum) => object_json(&[
            ("op", quote("payload_at_most")),
            ("maximum", maximum.to_string()),
        ]),
        Predicate::UnitsAtMost(maximum) => {
            object_json(&[("op", quote("units_at_most")), ("maximum", number(maximum))])
        }
        Predicate::ExactValue { key, value } => object_json(&[
            ("op", quote("exact_value")),
            ("key", number(key)),
            ("value_hex", hex(value)),
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
                quote(if matches!(predicate, Predicate::All(_)) {
                    "all"
                } else {
                    "any"
                }),
            ),
            (
                "children",
                format!(
                    "[{}]",
                    children
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ),
        ]),
        Predicate::Not(child) => object_json(&[("op", quote("not")), ("child", child.to_string())]),
    }
}

fn target_json(target: ResolvedTarget) -> String {
    object_json(&[
        ("adapter", number(target.adapter)),
        ("object", number(target.object)),
        ("contract_version", number(target.contract_version)),
        ("expected_version", number(target.expected_version)),
        ("generation", number(target.generation)),
    ])
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to a String");
    }
    quote(&value)
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), number)
}
