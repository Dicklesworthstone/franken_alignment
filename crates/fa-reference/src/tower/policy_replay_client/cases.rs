//! Validate disclosed case structure without inventing hidden payload/witness data.

use super::syntax::{array, ensure, natural, number, object, optional_number, text};
use super::{Counts, PolicyReplayClientError, Result};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use crate::action::consequence::policy_campaign::{PolicyDelta, ReplayCaseId};
use crate::action::{MAX_PAYLOAD_BYTES, MAX_REQUIRED_WITNESSES, ResolvedTarget};
use crate::strict_json::Json;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Order {
    proposal: Option<u64>,
    review_sequence: Option<u64>,
}

impl Order {
    pub(super) fn accept(&mut self, id: ReplayCaseId) -> Result<()> {
        match id {
            ReplayCaseId::Proposal(attempt) => {
                ensure(
                    self.review_sequence.is_none()
                        && self.proposal.is_none_or(|last| attempt > last),
                )?;
                self.proposal = Some(attempt);
            }
            ReplayCaseId::Review {
                control_sequence, ..
            } => {
                ensure(
                    self.review_sequence
                        .is_none_or(|last| control_sequence > last),
                )?;
                self.review_sequence = Some(control_sequence);
            }
        }
        Ok(())
    }
}

pub(super) struct CheckedCase {
    pub(super) id: ReplayCaseId,
    pub(super) payload_bytes: usize,
    delta: PolicyDelta,
}

impl CheckedCase {
    pub(super) fn blocked(&self) -> bool {
        self.delta == PolicyDelta::NewlyBlocked
    }
    pub(super) fn reviewable(&self) -> bool {
        self.delta == PolicyDelta::NewlyReviewable
    }
    pub(super) fn add(&self, counts: &mut Counts) -> Result<()> {
        let count = match self.delta {
            PolicyDelta::Unchanged => &mut counts.unchanged,
            PolicyDelta::NewlyBlocked => &mut counts.blocked,
            PolicyDelta::NewlyReviewable => &mut counts.reviewable,
            PolicyDelta::RequiresShadow => &mut counts.shadow,
        };
        *count = count
            .checked_add(1)
            .ok_or(PolicyReplayClientError::InvalidResponse)?;
        Ok(())
    }
}

pub(super) fn check(
    value: &Json,
    previous: &Policy,
    candidate: &Policy,
    control_sequence: u64,
    policy_epoch: u64,
) -> Result<CheckedCase> {
    let row = object(
        value,
        &[
            "id",
            "delta",
            "snapshot_semantic_epoch",
            "action",
            "original",
            "candidate",
            "original_consequence",
        ],
    )?;
    let id = case_id(&row["id"])?;
    number(&row["snapshot_semantic_epoch"])?;
    let raw_action = object(
        &row["action"],
        &[
            "target",
            "units",
            "payload_bytes",
            "policy_epoch",
            "deadline",
        ],
    )?;
    let action = Action {
        target: target(&raw_action["target"])?,
        units: number(&raw_action["units"])?,
        payload_bytes: natural(&raw_action["payload_bytes"])?,
    };
    ensure(
        action.target.is_some()
            && action.units != 0
            && number(&raw_action["deadline"])? != 0
            && action.payload_bytes <= MAX_PAYLOAD_BYTES
            && number(&raw_action["policy_epoch"])? <= policy_epoch,
    )?;
    match id {
        ReplayCaseId::Proposal(_) => ensure(row["original_consequence"].is_null())?,
        ReplayCaseId::Review {
            control_sequence: sequence,
            ..
        } => {
            ensure(
                sequence <= control_sequence
                    && matches!(
                        text(&row["original_consequence"])?,
                        "continue"
                            | "hold_effect"
                            | "deny"
                            | "narrow_authority"
                            | "suspend_run"
                            | "reset_to_checkpoint"
                    ),
            )?;
        }
    }
    let original = evaluation(&row["original"], previous, action, false)?;
    let candidate_result = match row["candidate"].get("state").and_then(Json::as_str) {
        Some("known") => evaluation(&row["candidate"], candidate, action, true)?,
        Some("unknown") => {
            let unknown = object(&row["candidate"], &["state", "reason", "missing_nodes"])?;
            ensure(text(&unknown["reason"])? == "missing_retained_observations")?;
            let missing = array(&unknown["missing_nodes"])?;
            ensure(!missing.is_empty() && missing.len() <= candidate.nodes().len())?;
            let mut last = None;
            for value in missing {
                let index = natural(value)?;
                ensure(
                    last.is_none_or(|previous| index > previous)
                        && candidate.nodes().get(index).is_some_and(reads_state),
                )?;
                last = Some(index);
            }
            Truth::Unknown
        }
        _ => return Err(PolicyReplayClientError::InvalidResponse),
    };
    let delta = match candidate_result {
        Truth::Unknown => PolicyDelta::RequiresShadow,
        observed if observed == original => PolicyDelta::Unchanged,
        Truth::Satisfied => PolicyDelta::NewlyReviewable,
        Truth::Violated => PolicyDelta::NewlyBlocked,
    };
    let label = match delta {
        PolicyDelta::Unchanged => "unchanged",
        PolicyDelta::NewlyBlocked => "newly_blocked",
        PolicyDelta::NewlyReviewable => "newly_reviewable",
        PolicyDelta::RequiresShadow => "requires_shadow",
    };
    ensure(text(&row["delta"])? == label)?;
    Ok(CheckedCase {
        id,
        payload_bytes: action.payload_bytes,
        delta,
    })
}

pub(super) fn check_ids(value: &Json, expected: &[ReplayCaseId]) -> Result<()> {
    let values = array(value)?;
    ensure(values.len() == expected.len())?;
    for (value, expected) in values.iter().zip(expected) {
        ensure(case_id(value)? == *expected)?;
    }
    Ok(())
}

fn case_id(value: &Json) -> Result<ReplayCaseId> {
    match value.get("kind").and_then(Json::as_str) {
        Some("proposal") => {
            let value = object(value, &["kind", "attempt"])?;
            let attempt = number(&value["attempt"])?;
            ensure(attempt != 0)?;
            Ok(ReplayCaseId::Proposal(attempt))
        }
        Some("review") => {
            let value = object(value, &["kind", "attempt", "round", "control_sequence"])?;
            let attempt = number(&value["attempt"])?;
            let round = number(&value["round"])?;
            let control_sequence = number(&value["control_sequence"])?;
            ensure(attempt != 0 && round != 0 && control_sequence != 0)?;
            Ok(ReplayCaseId::Review {
                attempt,
                round,
                control_sequence,
            })
        }
        _ => Err(PolicyReplayClientError::InvalidResponse),
    }
}

#[derive(Clone, Copy)]
struct Action {
    target: Option<ResolvedTarget>,
    units: u64,
    payload_bytes: usize,
}

fn target(value: &Json) -> Result<Option<ResolvedTarget>> {
    if value.is_null() {
        return Ok(None);
    }
    let value = object(
        value,
        &[
            "adapter",
            "object",
            "contract_version",
            "expected_version",
            "generation",
        ],
    )?;
    let target = ResolvedTarget {
        adapter: number(&value["adapter"])?,
        object: number(&value["object"])?,
        contract_version: number(&value["contract_version"])?,
        expected_version: number(&value["expected_version"])?,
        generation: number(&value["generation"])?,
    };
    ensure(
        ![
            target.adapter,
            target.object,
            target.contract_version,
            target.expected_version,
            target.generation,
        ]
        .contains(&0),
    )?;
    Ok(Some(target))
}

fn reads_state(node: &Predicate) -> bool {
    matches!(
        node,
        Predicate::ExactValue { .. } | Predicate::Absent { .. } | Predicate::EmptyRange { .. }
    )
}

fn evaluation(value: &Json, policy: &Policy, action: Action, allow_unknown: bool) -> Result<Truth> {
    let value = object(
        value,
        &["state", "generation", "result", "trace", "witness_count"],
    )?;
    let raw = array(&value["trace"])?;
    let witness_count = natural(&value["witness_count"])?;
    let result = truth(&value["result"])?;
    ensure(
        text(&value["state"])? == "known"
            && number(&value["generation"])? == policy.generation()
            && raw.len() == policy.nodes().len()
            && witness_count <= MAX_REQUIRED_WITNESSES,
    )?;
    let mut trace = Vec::with_capacity(raw.len());
    let mut witnesses = 0;
    let mut unknown = false;
    for (index, (step, node)) in raw.iter().zip(policy.nodes()).enumerate() {
        let step = object(step, &["node", "result", "witness"])?;
        ensure(natural(&step["node"])? == index)?;
        let observed = truth(&step["result"])?;
        if observed == Truth::Unknown {
            ensure(allow_unknown)?;
            unknown = true;
        }
        let witness = optional_number(&step["witness"])?;
        if reads_state(node) && observed != Truth::Unknown {
            ensure(witness == Some(witnesses as u64))?;
            witnesses += 1;
        } else {
            ensure(witness.is_none())?;
        }
        let expected = match node {
            Predicate::TargetIs(expected) => Some(from_bool(action.target == Some(*expected))),
            Predicate::PayloadAtMost(maximum) => Some(from_bool(action.payload_bytes <= *maximum)),
            Predicate::UnitsAtMost(maximum) => Some(from_bool(action.units <= *maximum)),
            Predicate::PayloadIs(bytes) => {
                ensure(observed != Truth::Unknown)?;
                if bytes.len() != action.payload_bytes {
                    Some(Truth::Violated)
                } else if bytes.is_empty() {
                    Some(Truth::Satisfied)
                } else {
                    None
                }
            }
            Predicate::ExactValue { .. }
            | Predicate::Absent { .. }
            | Predicate::EmptyRange { .. } => None,
            Predicate::All(children) => Some(
                if children
                    .iter()
                    .any(|child| trace[*child] == Truth::Violated)
                {
                    Truth::Violated
                } else if children.iter().any(|child| trace[*child] == Truth::Unknown) {
                    Truth::Unknown
                } else {
                    Truth::Satisfied
                },
            ),
            Predicate::Any(children) => Some(
                if children
                    .iter()
                    .any(|child| trace[*child] == Truth::Satisfied)
                {
                    Truth::Satisfied
                } else if children.iter().any(|child| trace[*child] == Truth::Unknown) {
                    Truth::Unknown
                } else {
                    Truth::Violated
                },
            ),
            Predicate::Not(child) => Some(match trace[*child] {
                Truth::Satisfied => Truth::Violated,
                Truth::Violated => Truth::Satisfied,
                Truth::Unknown => Truth::Unknown,
            }),
        };
        ensure(expected.is_none_or(|expected| expected == observed))?;
        trace.push(observed);
    }
    ensure(
        witnesses == witness_count
            && trace.last() == Some(&result)
            && (!unknown || result == Truth::Unknown),
    )?;
    Ok(result)
}

fn truth(value: &Json) -> Result<Truth> {
    match text(value)? {
        "satisfied" => Ok(Truth::Satisfied),
        "violated" => Ok(Truth::Violated),
        "unknown" => Ok(Truth::Unknown),
        _ => Err(PolicyReplayClientError::InvalidResponse),
    }
}

fn from_bool(value: bool) -> Truth {
    if value {
        Truth::Satisfied
    } else {
        Truth::Violated
    }
}
