//! Complete projection of the capability-free policy experiment. Historical
//! source evidence keeps its original basis independently of the branch cut.

use super::{
    Encoder, KnowledgeKind, TowerEncodingError, address_json, array_json, consequence_tag, hex,
    object_json, quote,
};
use crate::ReadWitness;
use crate::action::consequence::experiment::{
    CounterfactualReport, EmpiricalStatus, Intervention, MAX_INTERVENTION_BYTES, MAX_INTERVENTIONS,
    NextRequirement,
};
use crate::action::consequence::gate::containment::session::policy::controller::ReplayedDecision;
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Truth};
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::event_journal::{ReferenceDigestHasher, rule_tag};
use crate::observation_algebra::Knowledge;
use crate::reducer::{Outcome, Reduction};
use crate::tower::replay::{
    ObservedReplayReport, ReplayPlanMetadata, ReplayProfile, ReplayProperties,
};
use crate::tower::{TowerRehearsalOutcome, TowerReplayPlan};
use crate::typestate_cx::PurposeTag;

const REPLAY_SUMMARY_MAX_BYTES: usize = 1_048_576;
const REPLAY_SUMMARY_DOMAIN: &[u8] =
    b"fa.tower.replay.observed-policy-experiment.reference-checksum.v1\0";

struct SourceControlSequence;
impl KnowledgeKind for SourceControlSequence {
    const NAME: &'static str = "SourceControlSequence";
}

struct ReplayProperty;
impl KnowledgeKind for ReplayProperty {
    const NAME: &'static str = "ReplayProperty";
}

struct ReplayPlanPayload;
impl KnowledgeKind for ReplayPlanPayload {
    const NAME: &'static str = "ReplayPlanMetadata";
}

/// Deterministic reference checksum of the complete structured report, before
/// the request's smaller response limit is applied. This is not a cryptographic
/// authentication primitive or an independent-verification receipt.
pub(crate) fn replay_summary_digest(
    outcome: &TowerRehearsalOutcome,
) -> Result<[u8; 32], TowerEncodingError> {
    let encoder = Encoder {
        max_bytes: REPLAY_SUMMARY_MAX_BYTES,
    };
    let body = encoder.replay_outcome(outcome)?;
    let mut hasher = ReferenceDigestHasher::new();
    hasher.update(REPLAY_SUMMARY_DOMAIN);
    hasher.update(&(body.len() as u64).to_be_bytes());
    hasher.update(body.as_bytes());
    Ok(hasher.finalize())
}

impl Encoder {
    pub(super) fn replay_plan(&self, plan: &TowerReplayPlan) -> Result<String, TowerEncodingError> {
        if plan.branch_id == 0 {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        let source_basis = self.knowledge::<_, SourceControlSequence>(
            &Knowledge::known(plan.source_basis.control_seq(), plan.source_basis),
            |sequence| Ok(sequence.to_string()),
        )?;
        // The shared encoder invokes this closure only for Known metadata.
        // Withheld plans disclose no native identity or registration payload.
        let metadata = self.knowledge::<_, ReplayPlanPayload>(&plan.metadata, |metadata| {
            self.replay_plan_metadata(metadata)
        })?;
        self.bounded(object_json(&[
            ("branch_id", plan.branch_id.to_string()),
            ("branch_policy_epoch", plan.branch_policy_epoch.to_string()),
            ("source_address", address_json(&plan.source_address)),
            ("source_basis", source_basis),
            ("metadata", metadata),
            ("supported_semantics", quote("decision_replay")),
            ("retention", quote("in_memory")),
            (
                "requirements",
                object_json(&[
                    ("operator", "true".to_owned()),
                    ("full_domain_access", "true".to_owned()),
                    ("purpose", quote(PurposeTag::Experiment.as_str())),
                    ("owned_experiment_context", "true".to_owned()),
                    ("matching_native_tenant_and_run", "true".to_owned()),
                    ("current_branch_epoch", "true".to_owned()),
                    ("branch_distinct_from_source", "true".to_owned()),
                    ("idempotency_key", quote("exact_request")),
                ]),
            ),
            (
                "bounds",
                object_json(&[
                    ("max_interventions", MAX_INTERVENTIONS.to_string()),
                    ("max_intervention_bytes", MAX_INTERVENTION_BYTES.to_string()),
                ]),
            ),
            ("logical_replay_cost", "1".to_owned()),
        ]))
    }

    fn replay_plan_metadata(
        &self,
        metadata: &ReplayPlanMetadata,
    ) -> Result<String, TowerEncodingError> {
        if metadata.registration.keys.len() > MAX_INTERVENTIONS {
            return Err(TowerEncodingError::OutputLimit);
        }
        let properties = self.replay_properties(&metadata.properties)?;
        self.bounded(object_json(&[
            (
                "profile",
                quote(match metadata.profile {
                    ReplayProfile::ObservedPolicyExperiment => "observed_policy_experiment",
                }),
            ),
            ("native_scope", native_scope_json(metadata.native_scope)),
            ("native_attempt", metadata.native_attempt.to_string()),
            ("native_round", metadata.native_round.to_string()),
            (
                "native_expected_control_sequence",
                metadata.native_expected_control_sequence.to_string(),
            ),
            (
                "native_receipt_control_sequence",
                metadata
                    .native_receipt_control_sequence
                    .map_or_else(|| "null".to_owned(), |sequence| sequence.to_string()),
            ),
            ("has_receipt", metadata.has_receipt.to_string()),
            ("retained_bytes", metadata.retained_bytes.to_string()),
            (
                "registration",
                object_json(&[
                    ("id", metadata.registration.id.to_string()),
                    (
                        "allow_payload",
                        metadata.registration.allow_payload.to_string(),
                    ),
                    (
                        "allow_target",
                        metadata.registration.allow_target.to_string(),
                    ),
                    ("allow_units", metadata.registration.allow_units.to_string()),
                    (
                        "keys",
                        array_json(metadata.registration.keys.iter().map(u64::to_string)),
                    ),
                ]),
            ),
            ("properties", properties),
        ]))
    }

    pub(super) fn replay_outcome(
        &self,
        outcome: &TowerRehearsalOutcome,
    ) -> Result<String, TowerEncodingError> {
        if !outcome.rehearsal || outcome.branch_id == 0 {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        // The outer result describes a branch observation. Its nested source
        // cut must retain the original identities, epochs and proof states.
        let source_basis = self.knowledge::<_, SourceControlSequence>(
            &Knowledge::known(outcome.source_basis.control_seq(), outcome.source_basis),
            |sequence| Ok(sequence.to_string()),
        )?;
        let source = self.observed_replay_report(&outcome.source)?;
        self.bounded(object_json(&[
            ("rehearsal", outcome.rehearsal.to_string()),
            ("branch_id", outcome.branch_id.to_string()),
            ("source_address", address_json(&outcome.source_address)),
            ("source_basis", source_basis),
            ("source", source),
        ]))
    }

    fn observed_replay_report(
        &self,
        report: &ObservedReplayReport,
    ) -> Result<String, TowerEncodingError> {
        let replayed = self.replayed_decision(&report.replayed)?;
        let counterfactual = self.counterfactual_report(&report.counterfactual)?;
        let properties = self.replay_properties(&report.properties)?;
        self.bounded(object_json(&[
            (
                "profile",
                quote(match report.profile {
                    ReplayProfile::ObservedPolicyExperiment => "observed_policy_experiment",
                }),
            ),
            ("native_scope", native_scope_json(report.native_scope)),
            ("native_attempt", report.native_attempt.to_string()),
            ("native_round", report.native_round.to_string()),
            (
                "native_expected_control_sequence",
                report.native_expected_control_sequence.to_string(),
            ),
            (
                "native_receipt_control_sequence",
                report
                    .native_receipt_control_sequence
                    .map_or_else(|| "null".to_owned(), |sequence| sequence.to_string()),
            ),
            ("replayed", replayed),
            ("counterfactual", counterfactual),
            ("properties", properties),
        ]))
    }

    fn replay_properties(
        &self,
        properties: &ReplayProperties,
    ) -> Result<String, TowerEncodingError> {
        let encode = |property: &Knowledge<bool>| {
            self.knowledge::<_, ReplayProperty>(property, |value| Ok(value.to_string()))
        };
        self.bounded(object_json(&[
            (
                "replay_consistency",
                encode(&properties.replay_consistency)?,
            ),
            (
                "retained_anchor_binding",
                encode(&properties.retained_anchor_binding)?,
            ),
            ("receipt_binding", encode(&properties.receipt_binding)?),
            (
                "cryptographic_authentication",
                encode(&properties.cryptographic_authentication)?,
            ),
            (
                "independent_verifier_isolation",
                encode(&properties.independent_verifier_isolation)?,
            ),
            ("provider_truth", encode(&properties.provider_truth)?),
            (
                "external_execution",
                encode(&properties.external_execution)?,
            ),
            (
                "current_eligibility",
                encode(&properties.current_eligibility)?,
            ),
            (
                "full_ledger_accounting",
                encode(&properties.full_ledger_accounting)?,
            ),
        ]))
    }

    fn replayed_decision(&self, replayed: &ReplayedDecision) -> Result<String, TowerEncodingError> {
        let evaluation = self.policy_evaluation(replayed.evaluation())?;
        let tally = self.bounded(reduction_json(replayed.tally()))?;
        let decision = replayed.decision();
        self.bounded(object_json(&[
            (
                "decision",
                object_json(&[
                    ("consequence", quote(consequence_tag(decision.consequence))),
                    (
                        "rules",
                        array_json(decision.rules.iter().map(|rule| quote(rule_tag(*rule)))),
                    ),
                ]),
            ),
            ("evaluation", evaluation),
            ("tally", tally),
            (
                "missing",
                array_json(replayed.missing().iter().map(|member| quote(member))),
            ),
            (
                "abstained",
                array_json(replayed.abstained().iter().map(|member| quote(member))),
            ),
        ]))
    }

    fn policy_evaluation(&self, evaluation: &Evaluation) -> Result<String, TowerEncodingError> {
        let witnesses = self.bounded(array_json(
            evaluation.witnesses().iter().map(read_witness_json),
        ))?;
        self.bounded(object_json(&[
            ("generation", evaluation.generation().to_string()),
            ("result", quote(truth_tag(evaluation.result()))),
            (
                "trace",
                array_json(evaluation.trace().iter().map(|step| {
                    object_json(&[
                        ("node", step.node.to_string()),
                        ("result", quote(truth_tag(step.result))),
                        (
                            "witness",
                            step.witness
                                .map_or_else(|| "null".to_owned(), |index| index.to_string()),
                        ),
                    ])
                })),
            ),
            ("witnesses", witnesses),
        ]))
    }

    fn counterfactual_report(
        &self,
        report: &CounterfactualReport,
    ) -> Result<String, TowerEncodingError> {
        let interventions = self.bounded(array_json(
            report.interventions().iter().map(intervention_json),
        ))?;
        self.bounded(object_json(&[
            ("scope", report.scope().to_string()),
            ("attempt", report.attempt().to_string()),
            ("round", report.round().to_string()),
            ("baseline", quote(truth_tag(report.baseline()))),
            ("counterfactual", quote(truth_tag(report.counterfactual()))),
            (
                "trace",
                array_json(report.trace().iter().map(|truth| quote(truth_tag(*truth)))),
            ),
            (
                "changes",
                array_json(report.changes().iter().map(|change| {
                    object_json(&[
                        ("node", change.node.to_string()),
                        ("baseline", quote(truth_tag(change.baseline))),
                        ("counterfactual", quote(truth_tag(change.counterfactual))),
                    ])
                })),
            ),
            ("interventions", interventions),
            (
                "next_requirement",
                quote(match report.next_requirement() {
                    NextRequirement::FreshIndependentReview => "fresh_independent_review",
                    NextRequirement::ExactPolicyViolation => "exact_policy_violation",
                    NextRequirement::MoreEvidence => "more_evidence",
                }),
            ),
            (
                "empirical_status",
                quote(match report.empirical_status() {
                    EmpiricalStatus::HistoricalOnly => "historical_only",
                    EmpiricalStatus::InvalidatedByIntervention => "invalidated_by_intervention",
                }),
            ),
        ]))
    }
}

fn native_scope_json(scope: Scope) -> String {
    object_json(&[
        ("tenant", scope.tenant.to_string()),
        ("principal", scope.principal.to_string()),
        ("run", scope.run.to_string()),
        ("branch", scope.branch.to_string()),
        ("authority", scope.authority.to_string()),
        (
            "purpose",
            quote(match scope.purpose {
                Purpose::Effect => "effect",
                Purpose::Experiment => "experiment",
            }),
        ),
    ])
}

fn reduction_json(reduction: &Reduction) -> String {
    let weights = |values: &std::collections::BTreeMap<String, u64>| {
        object_json(
            &values
                .iter()
                .map(|(identifier, weight)| (identifier.as_str(), weight.to_string()))
                .collect::<Vec<_>>(),
        )
    };
    object_json(&[
        (
            "outcome",
            quote(match reduction.outcome {
                Outcome::Empirical => "empirical",
                Outcome::Disqualified => "disqualified",
            }),
        ),
        ("permit_weight", reduction.permit_weight.to_string()),
        ("hold_weight", reduction.hold_weight.to_string()),
        ("admitted_weights", weights(&reduction.admitted_weights)),
        (
            "admitted_cohort_weights",
            weights(&reduction.admitted_cohort_weights),
        ),
    ])
}

fn read_witness_json(witness: &ReadWitness) -> String {
    match witness {
        ReadWitness::Exact { key, value } => object_json(&[
            ("kind", quote("exact")),
            ("key", key.to_string()),
            ("value_hex", optional_bytes_hex(value.as_deref())),
        ]),
        ReadWitness::EmptyRange { start, end } => object_json(&[
            ("kind", quote("empty_range")),
            ("start", start.to_string()),
            ("end", end.to_string()),
        ]),
    }
}

fn intervention_json(intervention: &Intervention) -> String {
    match intervention {
        Intervention::Payload(bytes) => object_json(&[
            ("kind", quote("payload")),
            ("payload_hex", quote(&hex(bytes))),
        ]),
        Intervention::Target(target) => object_json(&[
            ("kind", quote("target")),
            ("target", resolved_target_json(target)),
        ]),
        Intervention::Units(units) => {
            object_json(&[("kind", quote("units")), ("units", units.to_string())])
        }
        Intervention::Key {
            key,
            expected,
            replacement,
        } => object_json(&[
            ("kind", quote("key")),
            ("key", key.to_string()),
            ("expected_hex", optional_bytes_hex(expected.as_deref())),
            (
                "replacement_hex",
                optional_bytes_hex(replacement.as_deref()),
            ),
        ]),
    }
}

fn resolved_target_json(target: &ResolvedTarget) -> String {
    object_json(&[
        ("adapter", target.adapter.to_string()),
        ("object", target.object.to_string()),
        ("contract_version", target.contract_version.to_string()),
        ("expected_version", target.expected_version.to_string()),
        ("generation", target.generation.to_string()),
    ])
}

fn optional_bytes_hex(bytes: Option<&[u8]>) -> String {
    bytes.map_or_else(|| "null".to_owned(), |value| quote(&hex(value)))
}

fn truth_tag(truth: Truth) -> &'static str {
    match truth {
        Truth::Satisfied => "satisfied",
        Truth::Violated => "violated",
        Truth::Unknown => "unknown",
    }
}
