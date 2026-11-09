//! Complete native campaign comparisons with explicit, bounded disclosure.
//! Policy literals and case traces are granted; action payloads and retained
//! source witness values are omitted. No result is a live promotion capability.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplay, FilePolicyReplayBasis,
};
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Policy, Truth};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::action::consequence::policy_campaign::{PolicyDelta, PolicyReplayCase, ReplayCaseId};
use crate::action::{Purpose, ResolvedTarget};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedPolicyReplay, LinkedPolicyReplayPage};

mod pages;
use crate::tower::request::policy;

pub(super) fn encode(
    linked: &LinkedPolicyReplay,
    address: &Address,
    key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if address != linked.registration() || key != Some(linked.campaign().to_string().as_str()) {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = observation(linked.native(), |value, basis| {
        known(value, basis, address, key, encoder)
    })?;
    projection("fa.publication-policy-replay/1", field, result, encoder)
}

pub(super) fn encode_page(
    linked: &LinkedPolicyReplayPage,
    address: &Address,
    key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    pages::encode(linked, address, key, encoder)
}

fn observation<T>(
    result: &Result<Knowledge<T, FilePolicyReplayBasis>, ActorError>,
    encode_known: impl FnOnce(&T, FilePolicyReplayBasis) -> Result<String, TowerEncodingError>,
) -> Result<(&'static str, String), TowerEncodingError> {
    Ok(match result {
        Ok(Knowledge::Known { value, basis }) => ("knowledge", encode_known(value, *basis)?),
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
            authority_required: "supervisor_policy_replay",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_policy_replay")),
            ]),
        ),
        Ok(Knowledge::Stale {
            generation,
            current,
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("stale")),
                ("generation", number(generation)),
                ("current", number(current)),
            ]),
        ),
        Err(error) => (
            "error",
            object_json(&[(
                "code",
                quote(match error {
                    ActorError::MalformedProposal => "malformed",
                    ActorError::IdempotencyConflict => "idempotency_conflict",
                    ActorError::Capacity => "capacity",
                    ActorError::Unavailable => "unavailable",
                    ActorError::Withheld => "withheld",
                }),
            )]),
        ),
        Ok(_) => return Err(TowerEncodingError::InvalidKnowledge),
    })
}

fn projection(
    schema: &str,
    field: &str,
    result: String,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    encoder.bounded(object_json(&[
        ("schema", quote(schema)),
        ("operation", quote("policy-replay")),
        ("scope", quote("retained_current_policy_corpus")),
        ("historical", "true".to_owned()),
        ("rehearsal", "true".to_owned()),
        ("policy_changed", "false".to_owned()),
        ("helpers_rerun", "false".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("promotion_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("witness_values_disclosed", "false".to_owned()),
        ("current_eligibility", quote("not_observed")),
        (field, result),
    ]))
}

fn known(
    value: &FilePolicyReplay,
    basis: FilePolicyReplayBasis,
    address: &Address,
    key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let report = value.report();
    let limits = value.limits();
    if basis != value.basis()
        || basis.query == 0
        || key != Some(basis.query.to_string().as_str())
        || address.kind != Noun::Run
        || address.tenant != basis.scope.tenant.to_string()
        || address.id != basis.scope.run.to_string()
        || address.generation.is_none_or(|generation| generation == 0)
        || basis.scope.purpose != Purpose::Effect
        || basis.policy_generation != report.previous_policy().generation()
        || report.candidate_policy().generation() <= basis.policy_generation
        || report.cases().is_empty()
        || report.cases().len() > limits.cases
        || report.input_bytes() > limits.input_bytes
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let previous = encoder.bounded(policy::encode(report.previous_policy()))?;
    let candidate = encoder.bounded(policy::encode(report.candidate_policy()))?;
    let cases = bounded_array(
        report.cases().iter().map(|case| {
            if case.action().spec().scope != basis.scope {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            case_json(
                case,
                report.previous_policy(),
                report.candidate_policy(),
                encoder,
            )
        }),
        encoder,
    )?;
    let ids = |delta| {
        bounded_array(
            report
                .cases()
                .iter()
                .filter(move |case| case.delta() == delta)
                .map(|case| Ok(case_id(case.id()))),
            encoder,
        )
    };
    let body = encoder.bounded(object_json(&[
        ("previous_policy", previous),
        ("candidate_policy", candidate),
        ("case_count", number(report.cases().len())),
        ("input_bytes", number(report.input_bytes())),
        ("requires_shadow", report.requires_shadow().to_string()),
        ("newly_reviewable", ids(PolicyDelta::NewlyReviewable)?),
        ("newly_blocked", ids(PolicyDelta::NewlyBlocked)?),
        ("cases", cases),
        (
            "limits",
            object_json(&[
                ("cases", number(limits.cases)),
                ("input_bytes", number(limits.input_bytes)),
            ]),
        ),
        (
            "remaining_campaigns_at_capture",
            number(value.remaining_queries()),
        ),
    ]))?;
    let source = basis_json(basis);
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", source),
        ("value", body),
    ]))
}

fn basis_json(basis: FilePolicyReplayBasis) -> String {
    let scope = basis.scope;
    let source = object_json(&[
        (
            "scope",
            object_json(&[
                ("tenant", number(scope.tenant)),
                ("principal", number(scope.principal)),
                ("run", number(scope.run)),
                ("branch", number(scope.branch)),
                ("authority", number(scope.authority)),
                ("purpose", quote("effect")),
            ]),
        ),
        ("campaign", number(basis.query)),
        ("origin", quote("acknowledged_owner")),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("policy_epoch", number(basis.policy_epoch)),
        ("policy_generation", number(basis.policy_generation)),
        (
            "recorded_at",
            basis
                .recorded_at
                .map_or_else(|| "null".to_owned(), |tick| number(tick.0)),
        ),
    ]);
    source
}

fn case_json(
    case: &PolicyReplayCase,
    previous: &Policy,
    candidate: &Policy,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let action = case.action().spec();
    let candidate_result = match case.candidate() {
        Some(evaluated) => evaluation(evaluated, candidate, encoder)?,
        None => object_json(&[
            ("state", quote("unknown")),
            ("reason", quote("missing_retained_observations")),
            (
                "missing_nodes",
                bounded_array(
                    case.missing_nodes().iter().map(|node| Ok(number(node))),
                    encoder,
                )?,
            ),
        ]),
    };
    encoder.bounded(object_json(&[
        ("id", case_id(case.id())),
        (
            "delta",
            quote(match case.delta() {
                PolicyDelta::Unchanged => "unchanged",
                PolicyDelta::NewlyBlocked => "newly_blocked",
                PolicyDelta::NewlyReviewable => "newly_reviewable",
                PolicyDelta::RequiresShadow => "requires_shadow",
            }),
        ),
        (
            "snapshot_semantic_epoch",
            number(case.snapshot_semantic_epoch()),
        ),
        (
            "action",
            object_json(&[
                (
                    "target",
                    action.target.map_or_else(|| "null".to_owned(), target),
                ),
                ("units", number(action.units)),
                ("payload_bytes", number(action.payload.len())),
                ("policy_epoch", number(action.policy_epoch)),
                ("deadline", number(action.deadline.0)),
            ]),
        ),
        ("original", evaluation(case.original(), previous, encoder)?),
        ("candidate", candidate_result),
        (
            "original_consequence",
            case.original_consequence().map_or_else(
                || "null".to_owned(),
                |value| {
                    quote(match value {
                        Consequence::Continue => "continue",
                        Consequence::HoldEffect => "hold_effect",
                        Consequence::Deny => "deny",
                        Consequence::NarrowAuthority => "narrow_authority",
                        Consequence::SuspendRun => "suspend_run",
                        Consequence::ResetToCheckpoint => "reset_to_checkpoint",
                    })
                },
            ),
        ),
    ]))
}

fn evaluation(
    value: &Evaluation,
    policy: &Policy,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if value.generation() != policy.generation() || value.trace().len() != policy.nodes().len() {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let trace = bounded_array(
        value.trace().iter().enumerate().map(|(index, step)| {
            if step.node != index || step.witness.is_some_and(|w| w >= value.witnesses().len()) {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            Ok(object_json(&[
                ("node", number(step.node)),
                ("result", quote(truth(step.result))),
                (
                    "witness",
                    step.witness.map_or_else(|| "null".to_owned(), number),
                ),
            ]))
        }),
        encoder,
    )?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("generation", number(value.generation())),
        ("result", quote(truth(value.result()))),
        ("trace", trace),
        ("witness_count", number(value.witnesses().len())),
    ]))
}

fn bounded_array(
    values: impl IntoIterator<Item = Result<String, TowerEncodingError>>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let mut out = String::from("[");
    for value in values {
        let value = value?;
        let separator = usize::from(out.len() != 1);
        if out
            .len()
            .saturating_add(separator)
            .saturating_add(value.len())
            .saturating_add(1)
            > encoder.max_bytes
        {
            return Err(TowerEncodingError::OutputLimit);
        }
        if separator != 0 {
            out.push(',');
        }
        out.push_str(&value);
    }
    out.push(']');
    Ok(out)
}

fn case_id(id: ReplayCaseId) -> String {
    match id {
        ReplayCaseId::Proposal(attempt) => {
            object_json(&[("kind", quote("proposal")), ("attempt", number(attempt))])
        }
        ReplayCaseId::Review {
            attempt,
            round,
            control_sequence,
        } => object_json(&[
            ("kind", quote("review")),
            ("attempt", number(attempt)),
            ("round", number(round)),
            ("control_sequence", number(control_sequence)),
        ]),
    }
}

fn target(value: ResolvedTarget) -> String {
    object_json(&[
        ("adapter", number(value.adapter)),
        ("object", number(value.object)),
        ("contract_version", number(value.contract_version)),
        ("expected_version", number(value.expected_version)),
        ("generation", number(value.generation)),
    ])
}

fn truth(value: Truth) -> &'static str {
    match value {
        Truth::Satisfied => "satisfied",
        Truth::Violated => "violated",
        Truth::Unknown => "unknown",
    }
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}
