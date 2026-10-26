//! Certified decision-preserving planner rewrites (FA-070, plan §§14.5, 14.8).
//!
//! Certifies a small registered set of decision-preserving observation-plan
//! rewrites against exact byte/decision differentials, authorization-aware
//! equivalence laws, product frontiers (`FA-INV-022`), full-input view bindings
//! (`FA-INV-023`), and complete semantic dependency closures (`FA-INV-021`).
//!
//! No general optimizer theorem is claimed; certificates cover named laws and
//! their deterministic lab differential traces only, and never mint a `Permit`.

use super::{
    Basis, Knowledge,
    physical_plan::{
        PhysicalOperatorKind, PhysicalPlan, PlanRefusalReason, RehearsedPlanSummary,
        validate_physical_plan,
    },
};

/// Plan §14.5 classification of an optimization proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizationClass {
    /// Preserves exact decision, authorized view bytes, frontiers, and authority.
    DecisionPreserving,
    /// Alters sampling, thresholds, or monitor semantics; requires a new generation.
    DecisionAffecting,
    /// Alters capability ceilings or enforcement scope; requires governance.
    AuthorityAffecting,
}

/// Registered decision-preserving rewrite laws from plan §14.8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RewriteLawId {
    /// Push `AuthorizeProjection` ahead of expensive expansion/refinement when it
    /// reveals no denied structure and preserves exact authorized view bytes.
    PushAuthorizationBeforeExpansion,
    /// Share decoding of the same immutable authorized object across observers
    /// when privacy labels and authority scopes match.
    ShareImmutableAuthorizedDecode,
    /// Evaluate an exact denying predicate before scheduling expensive helper
    /// inference (never stopping early after a permissive helper).
    ShortCircuitExactDenyingPredicate,
    /// Reuse a witness refinement when complete declared dependencies and
    /// semantic generations are unchanged.
    ReuseValidWitnessRefinement,
    /// Transmit a verified residual against an exact receiver base instead of a
    /// full object under identical numeric and codec generations.
    TransmitVerifiedResidualAgainstBase,
}

/// Decision outcome observed in a deterministic lab differential run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabDecisionOutcome {
    AllowWithWitness { witness_digest: u64 },
    HoldWithReason { reason_code: u64 },
    DenyExact { reason_code: u64 },
}

/// Deterministic lab execution trace for one side (baseline or transformed) of a
/// rewrite differential check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabObservationTrace {
    pub decision: LabDecisionOutcome,
    /// Exact authorized transformed bytes shown to helpers (`FA-INV-023`).
    pub authorized_view_bytes: Vec<u8>,
    /// Ordered omission identifiers disclosed in the helper view (`FA-INV-023`).
    pub omission_manifest: Vec<u64>,
    /// Any bytes or structural metadata leaked from a denied region during pushdown.
    pub leaked_denied_structure_bytes: usize,
    /// Deterministic order in which cancellation obligations were discharged.
    pub cancellation_discharge_order: Vec<u64>,
    /// Realized cold-cache observation bytes.
    pub cold_bytes: usize,
    /// Realized warm-cache observation bytes.
    pub warm_bytes: usize,
    /// Realized cold-cache work units.
    pub cold_work_units: u64,
    /// Realized warm-cache work units.
    pub warm_work_units: u64,
}

/// Explicit structural flags audited by the rewrite verifier against forbidden
/// seductive rewrites in plan §14.8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewritePreconditions {
    pub pushes_past_unclosed_auth: bool,
    pub cross_tenant_sharing_without_contract: bool,
    pub stops_after_permissive_helper: bool,
    pub moves_absence_before_closure: bool,
    pub equates_embeddings_without_exact_witness: bool,
}

/// Complete rewrite certification request pairing baseline and transformed
/// physical plans with their deterministic lab traces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteProposal {
    pub law: RewriteLawId,
    pub classification: OptimizationClass,
    pub preconditions: RewritePreconditions,
    pub baseline_plan: PhysicalPlan,
    pub transformed_plan: PhysicalPlan,
    pub baseline_trace: LabObservationTrace,
    pub transformed_trace: LabObservationTrace,
}

/// Typed refusal reasons for planner rewrite certification (FA-070).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewriteRefusalReason {
    InvalidBaselinePlan(PlanRefusalReason),
    InvalidTransformedPlan(PlanRefusalReason),
    PendingFrontierPlan,
    NonDecisionPreservingClass(OptimizationClass),
    UnauthorizedPredicatePushdown,
    CrossTenantSharingOrPrivacyLeak,
    PermissiveEarlyStopForbidden,
    AbsenceMovedBeforeClosure,
    EmbeddingEquivalenceForbidden,
    LoweredAuthorityOrReviewRequirement,
    ChangedBasisOrNumericGeneration,
    ChangedProductFrontier,
    ChangedAuthorizedViewBytes,
    ChangedDecisionDifferential,
    ChangedCancellationOrdering,
    LawStructureMismatch,
    ResourceRegressionOrEnvelopeExceeded,
}

/// Certified decision-preserving planner rewrite (FA-070).
///
/// A rewrite certificate is a rehearsal/planning artifact and can never be
/// converted into an execution permit:
///
/// ```compile_fail,E0599
/// use fa_reference::observation_algebra::rewrites::RewriteCertificate;
/// fn elevate(cert: RewriteCertificate) {
///     let _ = cert.into_permit();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteCertificate {
    law: RewriteLawId,
    classification: OptimizationClass,
    baseline_summary: RehearsedPlanSummary,
    transformed_summary: RehearsedPlanSummary,
    cold_bytes_saved: usize,
    warm_bytes_saved: usize,
    cold_work_saved: u64,
    warm_work_saved: u64,
}

impl RewriteCertificate {
    pub const fn law(&self) -> RewriteLawId {
        self.law
    }

    pub const fn classification(&self) -> OptimizationClass {
        self.classification
    }

    pub const fn baseline_summary(&self) -> &RehearsedPlanSummary {
        &self.baseline_summary
    }

    pub const fn transformed_summary(&self) -> &RehearsedPlanSummary {
        &self.transformed_summary
    }

    pub const fn cold_bytes_saved(&self) -> usize {
        self.cold_bytes_saved
    }

    pub const fn warm_bytes_saved(&self) -> usize {
        self.warm_bytes_saved
    }

    pub const fn cold_work_saved(&self) -> u64 {
        self.cold_work_saved
    }

    pub const fn warm_work_saved(&self) -> u64 {
        self.warm_work_saved
    }

    pub const fn root_basis(&self) -> Basis {
        self.transformed_summary.root_basis()
    }
}

/// Validates and certifies a decision-preserving planner rewrite against
/// structural preconditions and paired deterministic lab differential traces.
pub fn certify_planner_rewrite(
    proposal: &RewriteProposal,
) -> Result<Knowledge<RewriteCertificate>, RewriteRefusalReason> {
    if proposal.classification != OptimizationClass::DecisionPreserving {
        return Err(RewriteRefusalReason::NonDecisionPreservingClass(
            proposal.classification,
        ));
    }

    if proposal.preconditions.pushes_past_unclosed_auth
        || proposal.baseline_trace.leaked_denied_structure_bytes > 0
        || proposal.transformed_trace.leaked_denied_structure_bytes > 0
    {
        return Err(RewriteRefusalReason::UnauthorizedPredicatePushdown);
    }

    if proposal.preconditions.cross_tenant_sharing_without_contract {
        return Err(RewriteRefusalReason::CrossTenantSharingOrPrivacyLeak);
    }

    if proposal.preconditions.stops_after_permissive_helper {
        return Err(RewriteRefusalReason::PermissiveEarlyStopForbidden);
    }

    if proposal.preconditions.moves_absence_before_closure {
        return Err(RewriteRefusalReason::AbsenceMovedBeforeClosure);
    }

    if proposal
        .preconditions
        .equates_embeddings_without_exact_witness
    {
        return Err(RewriteRefusalReason::EmbeddingEquivalenceForbidden);
    }

    let baseline_knowledge = validate_physical_plan(&proposal.baseline_plan)
        .map_err(RewriteRefusalReason::InvalidBaselinePlan)?;
    let transformed_knowledge = validate_physical_plan(&proposal.transformed_plan)
        .map_err(RewriteRefusalReason::InvalidTransformedPlan)?;

    let (
        Knowledge::Known {
            value: baseline_summary,
            ..
        },
        Knowledge::Known {
            value: transformed_summary,
            ..
        },
    ) = (baseline_knowledge, transformed_knowledge)
    else {
        return Err(RewriteRefusalReason::PendingFrontierPlan);
    };

    // Pin semantic identity and uncertainty: no rewrite may change subject, question,
    // profile, semantic generation, policy/model/tokenizer/codec epochs, or proof states.
    if baseline_summary.root_basis() != transformed_summary.root_basis()
        || baseline_summary.root_uncertainty() != transformed_summary.root_uncertainty()
    {
        return Err(RewriteRefusalReason::ChangedBasisOrNumericGeneration);
    }

    // No cheaper rewrite may lower required authority or widen privacy exposure.
    if baseline_summary.required_authority() != transformed_summary.required_authority()
        || proposal.baseline_plan.granted_authority != proposal.transformed_plan.granted_authority
    {
        return Err(RewriteRefusalReason::LoweredAuthorityOrReviewRequirement);
    }

    if baseline_summary.accumulated_privacy() != transformed_summary.accumulated_privacy()
        || proposal.baseline_plan.allowed_privacy != proposal.transformed_plan.allowed_privacy
    {
        return Err(RewriteRefusalReason::CrossTenantSharingOrPrivacyLeak);
    }

    // Product frontiers must remain identical (FA-INV-022).
    if baseline_summary.accumulated_frontiers() != transformed_summary.accumulated_frontiers() {
        return Err(RewriteRefusalReason::ChangedProductFrontier);
    }

    // Cancellation obligations and deterministic discharge order must match.
    if baseline_summary.accumulated_obligations() != transformed_summary.accumulated_obligations()
        || proposal.baseline_trace.cancellation_discharge_order
            != proposal.transformed_trace.cancellation_discharge_order
    {
        return Err(RewriteRefusalReason::ChangedCancellationOrdering);
    }

    // Exact helper view bytes and omission manifest must match (FA-INV-023).
    if proposal.baseline_trace.authorized_view_bytes
        != proposal.transformed_trace.authorized_view_bytes
        || proposal.baseline_trace.omission_manifest != proposal.transformed_trace.omission_manifest
    {
        return Err(RewriteRefusalReason::ChangedAuthorizedViewBytes);
    }

    // Exact decision differential check.
    if proposal.baseline_trace.decision != proposal.transformed_trace.decision {
        return Err(RewriteRefusalReason::ChangedDecisionDifferential);
    }

    // Law-specific structural checks on the physical operator DAGs.
    verify_law_structure(
        proposal.law,
        &proposal.baseline_plan,
        &proposal.transformed_plan,
        &proposal.baseline_trace,
    )?;

    // Performance and resource bounds are checked only after semantic equivalence passes.
    if transformed_summary.total_bytes() > baseline_summary.total_bytes()
        || transformed_summary.total_decoded_bytes() > baseline_summary.total_decoded_bytes()
        || proposal.transformed_trace.cold_bytes > proposal.baseline_trace.cold_bytes
        || proposal.transformed_trace.warm_bytes > proposal.baseline_trace.warm_bytes
        || proposal.transformed_trace.cold_work_units > proposal.baseline_trace.cold_work_units
        || proposal.transformed_trace.warm_work_units > proposal.baseline_trace.warm_work_units
    {
        return Err(RewriteRefusalReason::ResourceRegressionOrEnvelopeExceeded);
    }

    let basis = transformed_summary.root_basis();
    let cert = RewriteCertificate {
        law: proposal.law,
        classification: OptimizationClass::DecisionPreserving,
        cold_bytes_saved: proposal.baseline_trace.cold_bytes
            - proposal.transformed_trace.cold_bytes,
        warm_bytes_saved: proposal.baseline_trace.warm_bytes
            - proposal.transformed_trace.warm_bytes,
        cold_work_saved: proposal.baseline_trace.cold_work_units
            - proposal.transformed_trace.cold_work_units,
        warm_work_saved: proposal.baseline_trace.warm_work_units
            - proposal.transformed_trace.warm_work_units,
        baseline_summary,
        transformed_summary,
    };

    Ok(Knowledge::known(cert, basis))
}

fn verify_law_structure(
    law: RewriteLawId,
    baseline: &PhysicalPlan,
    transformed: &PhysicalPlan,
    baseline_trace: &LabObservationTrace,
) -> Result<(), RewriteRefusalReason> {
    match law {
        RewriteLawId::PushAuthorizationBeforeExpansion => {
            let has_auth = transformed
                .nodes
                .iter()
                .any(|n| matches!(n.operator, PhysicalOperatorKind::AuthorizeProjection { .. }));
            if !has_auth {
                return Err(RewriteRefusalReason::LawStructureMismatch);
            }
        }
        RewriteLawId::ShareImmutableAuthorizedDecode => {
            let base_decodes = baseline
                .nodes
                .iter()
                .filter(|n| matches!(n.operator, PhysicalOperatorKind::Decode { .. }))
                .count();
            let trans_decodes = transformed
                .nodes
                .iter()
                .filter(|n| matches!(n.operator, PhysicalOperatorKind::Decode { .. }))
                .count();
            if base_decodes == 0 || trans_decodes == 0 || trans_decodes > base_decodes {
                return Err(RewriteRefusalReason::LawStructureMismatch);
            }
        }
        RewriteLawId::ShortCircuitExactDenyingPredicate => {
            // Early stop is legal only for an exact denying predicate, never for permission.
            if !matches!(
                baseline_trace.decision,
                LabDecisionOutcome::DenyExact { .. }
            ) {
                return Err(RewriteRefusalReason::PermissiveEarlyStopForbidden);
            }
        }
        RewriteLawId::ReuseValidWitnessRefinement => {
            let has_refine = transformed
                .nodes
                .iter()
                .any(|n| matches!(n.operator, PhysicalOperatorKind::RefineWitness { .. }));
            if !has_refine {
                return Err(RewriteRefusalReason::LawStructureMismatch);
            }
        }
        RewriteLawId::TransmitVerifiedResidualAgainstBase => {
            let has_decode_or_refine = transformed.nodes.iter().any(|n| {
                matches!(
                    n.operator,
                    PhysicalOperatorKind::Decode { .. }
                        | PhysicalOperatorKind::RefineWitness { .. }
                )
            });
            if !has_decode_or_refine {
                return Err(RewriteRefusalReason::LawStructureMismatch);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observation_algebra::{
            AuthorityKind, AuthorityRequirements, BasisIdentity, CancellationObligations,
            ClaimClass, DecodeLaw, DecodeRequirement, LawDeclaration, MAX_OPERATOR_BYTES,
            MAX_OPERATOR_CANDIDATES, MAX_OPERATOR_DECODED_BYTES, MAX_OPERATOR_FRONTIERS,
            MAX_OPERATOR_INPUTS, MAX_OPERATOR_OUTPUTS, MAX_OPERATOR_WITNESSES, ObjectDeclaration,
            Obligation, ObligationKind, ObservationRequirement, OperatorContext, PrivacyLabel,
            PrivacyRestrictions, ProjectionSpec, ProofStates, RequirementClass, ResourceEnvelope,
            ScopedProof, UncertaintyClass, physical_plan::PhysicalPlanNode,
        },
        product_frontier::{
            FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey,
            TrustedClosingMarker,
        },
    };

    fn sample_basis() -> Basis {
        Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 2,
                profile_id: 3,
                semantic_generation: 4,
                policy_epoch: 5,
                model_epoch: 6,
                tokenizer_epoch: 0,
                codec_epoch: 7,
                control_seq: 8,
            },
            ClaimClass::BoundedModel,
            ProofStates {
                authentic_origin: ScopedProof::Declared {
                    scope_id: 11,
                    generation: 4,
                },
                complete_for_contract: ScopedProof::Declared {
                    scope_id: 13,
                    generation: 4,
                },
                semantically_valid: ScopedProof::Declared {
                    scope_id: 17,
                    generation: 4,
                },
                available_for_replay: ScopedProof::Declared {
                    scope_id: 19,
                    generation: 4,
                },
            },
        )
    }

    fn full_envelope() -> ResourceEnvelope {
        ResourceEnvelope::new(
            MAX_OPERATOR_INPUTS,
            MAX_OPERATOR_OUTPUTS,
            MAX_OPERATOR_WITNESSES,
            MAX_OPERATOR_FRONTIERS,
            MAX_OPERATOR_BYTES,
            MAX_OPERATOR_DECODED_BYTES,
            MAX_OPERATOR_CANDIDATES,
        )
        .unwrap()
    }

    fn make_ctx(
        basis: Basis,
        class: RequirementClass,
        uncertainty: UncertaintyClass,
        labels: Vec<PrivacyLabel>,
        obligations: Vec<Obligation>,
    ) -> OperatorContext {
        OperatorContext::new(
            ObservationRequirement {
                class,
                uncertainty,
                basis,
            },
            PrivacyRestrictions::new(labels).unwrap(),
            CancellationObligations::new(obligations).unwrap(),
            full_envelope(),
        )
    }

    fn proj_key() -> ProjectionKey {
        ProjectionKey {
            source: 17,
            branch: 19,
            projection: 31,
            source_epoch: 29,
        }
    }

    fn sample_plan_pair(
        baseline_decode_bytes: usize,
        transformed_decode_bytes: usize,
        basis: Basis,
    ) -> (PhysicalPlan, PhysicalPlan) {
        let key = proj_key();
        let marker = TrustedClosingMarker {
            key,
            final_sequence: 5,
            marker_generation: 43,
        };
        let mut frontiers = ProductFrontiers::new(2, 8).unwrap();
        for seq in 1..=5 {
            frontiers
                .accept(key, FrontierStage::Authenticated, seq)
                .unwrap();
            frontiers.accept(key, FrontierStage::Captured, seq).unwrap();
        }
        frontiers.record_close(marker).unwrap();
        let req = FrontierRequirement {
            key,
            stage: FrontierStage::Authenticated,
            through: 5,
            closure: Some(43),
        };
        let label = PrivacyLabel {
            purpose_id: 1,
            transform_id: 10,
        };
        let allowed_privacy = PrivacyRestrictions::new(vec![label]).unwrap();
        let granted_authority = AuthorityRequirements::new(vec![
            AuthorityKind::Observe,
            AuthorityKind::Project,
            AuthorityKind::Verify,
        ])
        .unwrap();

        let make_plan = |decode_bytes: usize| PhysicalPlan {
            nodes: vec![
                PhysicalPlanNode {
                    node_id: 1,
                    operator: PhysicalOperatorKind::Tap {
                        source_id: 17,
                        source_generation: 29,
                        bytes: 64,
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::Capture,
                        UncertaintyClass::Exact,
                        vec![label],
                        vec![Obligation {
                            id: 1,
                            kind: ObligationKind::RetainInput,
                        }],
                    ),
                },
                PhysicalPlanNode {
                    node_id: 2,
                    operator: PhysicalOperatorKind::AuthorizeProjection {
                        input: 0,
                        projection: ProjectionSpec {
                            key,
                            projection_id: 31,
                        },
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::Projection,
                        UncertaintyClass::Exact,
                        vec![],
                        vec![],
                    ),
                },
                PhysicalPlanNode {
                    node_id: 3,
                    operator: PhysicalOperatorKind::RequirePrefix {
                        input: 1,
                        requirement: req,
                        claim_closed_prefix: true,
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::Prefix,
                        UncertaintyClass::Exact,
                        vec![],
                        vec![],
                    ),
                },
                PhysicalPlanNode {
                    node_id: 4,
                    operator: PhysicalOperatorKind::VerifyObject {
                        input: 2,
                        declaration: ObjectDeclaration {
                            object_id: 17,
                            generation: 29,
                            profile_id: basis.profile_id(),
                        },
                        verifier_law: LawDeclaration::new(
                            41,
                            29,
                            basis.semantic_generation(),
                            basis.profile_id(),
                        )
                        .unwrap(),
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::ExactObject,
                        UncertaintyClass::Exact,
                        vec![],
                        vec![],
                    ),
                },
                PhysicalPlanNode {
                    node_id: 5,
                    operator: PhysicalOperatorKind::Decode {
                        input: 3,
                        object_id: 17,
                        encoded_bytes: 32,
                        decoded_bytes: decode_bytes,
                        requirement: DecodeRequirement::ExactCheckpoint,
                        law: DecodeLaw::RegisteredLossless(
                            LawDeclaration::new(
                                51,
                                basis.codec_epoch(),
                                basis.semantic_generation(),
                                basis.profile_id(),
                            )
                            .unwrap(),
                        ),
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::ExactCheckpoint,
                        UncertaintyClass::Exact,
                        vec![],
                        vec![],
                    ),
                },
                PhysicalPlanNode {
                    node_id: 6,
                    operator: PhysicalOperatorKind::RefineWitness {
                        parent: 4,
                        refined_bytes: 16,
                        law: LawDeclaration::new(
                            61,
                            basis.semantic_generation(),
                            basis.semantic_generation(),
                            basis.profile_id(),
                        )
                        .unwrap(),
                    },
                    context: make_ctx(
                        basis,
                        RequirementClass::Refine,
                        UncertaintyClass::Exact,
                        vec![],
                        vec![],
                    ),
                },
            ],
            root: 5,
            granted_authority: granted_authority.clone(),
            allowed_privacy: allowed_privacy.clone(),
            frontiers: frontiers.clone(),
            total_envelope: full_envelope(),
        };

        (
            make_plan(baseline_decode_bytes),
            make_plan(transformed_decode_bytes),
        )
    }

    fn sample_traces() -> (LabObservationTrace, LabObservationTrace) {
        let base = LabObservationTrace {
            decision: LabDecisionOutcome::DenyExact { reason_code: 42 },
            authorized_view_bytes: b"authorized-helper-view-v1".to_vec(),
            omission_manifest: vec![1001, 1002],
            leaked_denied_structure_bytes: 0,
            cancellation_discharge_order: vec![1],
            cold_bytes: 1024,
            warm_bytes: 512,
            cold_work_units: 80,
            warm_work_units: 40,
        };
        let trans = LabObservationTrace {
            decision: LabDecisionOutcome::DenyExact { reason_code: 42 },
            authorized_view_bytes: b"authorized-helper-view-v1".to_vec(),
            omission_manifest: vec![1001, 1002],
            leaked_denied_structure_bytes: 0,
            cancellation_discharge_order: vec![1],
            cold_bytes: 512,
            warm_bytes: 256,
            cold_work_units: 45,
            warm_work_units: 20,
        };
        (base, trans)
    }

    #[test]
    fn all_five_registered_laws_certify_with_paired_differential_and_savings() {
        let basis = sample_basis();
        let (baseline_plan, transformed_plan) = sample_plan_pair(64, 32, basis);
        let (baseline_trace, transformed_trace) = sample_traces();

        for law in [
            RewriteLawId::PushAuthorizationBeforeExpansion,
            RewriteLawId::ShareImmutableAuthorizedDecode,
            RewriteLawId::ShortCircuitExactDenyingPredicate,
            RewriteLawId::ReuseValidWitnessRefinement,
            RewriteLawId::TransmitVerifiedResidualAgainstBase,
        ] {
            let proposal = RewriteProposal {
                law,
                classification: OptimizationClass::DecisionPreserving,
                preconditions: RewritePreconditions::default(),
                baseline_plan: baseline_plan.clone(),
                transformed_plan: transformed_plan.clone(),
                baseline_trace: baseline_trace.clone(),
                transformed_trace: transformed_trace.clone(),
            };
            let Knowledge::Known { value: cert, .. } =
                certify_planner_rewrite(&proposal).expect("registered rewrite should certify")
            else {
                panic!("expected known RewriteCertificate");
            };
            assert_eq!(cert.law(), law);
            assert_eq!(cert.classification(), OptimizationClass::DecisionPreserving);
            assert_eq!(cert.cold_bytes_saved(), 512);
            assert_eq!(cert.warm_bytes_saved(), 256);
            assert_eq!(cert.cold_work_saved(), 35);
            assert_eq!(cert.warm_work_saved(), 20);
        }
    }

    #[test]
    fn auth_pushdown_leakage_and_lowered_authority_are_refused() {
        let basis = sample_basis();
        let (baseline_plan, transformed_plan) = sample_plan_pair(64, 32, basis);
        let (baseline_trace, mut leaky_trace) = sample_traces();
        leaky_trace.leaked_denied_structure_bytes = 8;

        let leaky_proposal = RewriteProposal {
            law: RewriteLawId::PushAuthorizationBeforeExpansion,
            classification: OptimizationClass::DecisionPreserving,
            preconditions: RewritePreconditions::default(),
            baseline_plan: baseline_plan.clone(),
            transformed_plan: transformed_plan.clone(),
            baseline_trace: baseline_trace.clone(),
            transformed_trace: leaky_trace,
        };
        assert_eq!(
            certify_planner_rewrite(&leaky_proposal),
            Err(RewriteRefusalReason::UnauthorizedPredicatePushdown)
        );

        let unclosed_auth_proposal = RewriteProposal {
            preconditions: RewritePreconditions {
                pushes_past_unclosed_auth: true,
                ..RewritePreconditions::default()
            },
            transformed_trace: sample_traces().1,
            ..leaky_proposal.clone()
        };
        assert_eq!(
            certify_planner_rewrite(&unclosed_auth_proposal),
            Err(RewriteRefusalReason::UnauthorizedPredicatePushdown)
        );

        // Altering granted authority on the transformed plan is refused.
        let mut weakened_plan = transformed_plan;
        weakened_plan.granted_authority = AuthorityRequirements::new(vec![
            AuthorityKind::Observe,
            AuthorityKind::Project,
            AuthorityKind::Verify,
            AuthorityKind::Judge,
        ])
        .unwrap();
        let weakened_proposal = RewriteProposal {
            transformed_plan: weakened_plan,
            transformed_trace: sample_traces().1,
            ..leaky_proposal
        };
        assert_eq!(
            certify_planner_rewrite(&weakened_proposal),
            Err(RewriteRefusalReason::LoweredAuthorityOrReviewRequirement)
        );
    }

    #[test]
    fn changed_view_bytes_omissions_decision_or_cancellation_order_are_refused() {
        let basis = sample_basis();
        let (baseline_plan, transformed_plan) = sample_plan_pair(64, 32, basis);
        let (baseline_trace, transformed_trace) = sample_traces();

        let base_proposal = RewriteProposal {
            law: RewriteLawId::ReuseValidWitnessRefinement,
            classification: OptimizationClass::DecisionPreserving,
            preconditions: RewritePreconditions::default(),
            baseline_plan,
            transformed_plan,
            baseline_trace,
            transformed_trace,
        };

        let mut view_mutated = base_proposal.clone();
        view_mutated.transformed_trace.authorized_view_bytes = b"mutated-helper-view".to_vec();
        assert_eq!(
            certify_planner_rewrite(&view_mutated),
            Err(RewriteRefusalReason::ChangedAuthorizedViewBytes)
        );

        let mut omission_mutated = base_proposal.clone();
        omission_mutated.transformed_trace.omission_manifest = vec![1001];
        assert_eq!(
            certify_planner_rewrite(&omission_mutated),
            Err(RewriteRefusalReason::ChangedAuthorizedViewBytes)
        );

        let mut decision_mutated = base_proposal.clone();
        decision_mutated.transformed_trace.decision =
            LabDecisionOutcome::AllowWithWitness { witness_digest: 99 };
        assert_eq!(
            certify_planner_rewrite(&decision_mutated),
            Err(RewriteRefusalReason::ChangedDecisionDifferential)
        );

        let mut cancel_mutated = base_proposal;
        cancel_mutated
            .transformed_trace
            .cancellation_discharge_order = vec![2, 1];
        assert_eq!(
            certify_planner_rewrite(&cancel_mutated),
            Err(RewriteRefusalReason::ChangedCancellationOrdering)
        );
    }

    #[test]
    fn changed_frontier_changed_numeric_generation_and_forbidden_rewrites_are_refused() {
        let basis = sample_basis();
        let (baseline_plan, transformed_plan) = sample_plan_pair(64, 32, basis);
        let (baseline_trace, transformed_trace) = sample_traces();

        // Changed codec/model generation is refused.
        let mut drifted_identity = basis.identity();
        drifted_identity.codec_epoch += 1;
        let drifted_basis = Basis::new(drifted_identity, basis.claim_class(), basis.proofs());
        let (_, drifted_plan) = sample_plan_pair(64, 32, drifted_basis);
        let gen_proposal = RewriteProposal {
            law: RewriteLawId::TransmitVerifiedResidualAgainstBase,
            classification: OptimizationClass::DecisionPreserving,
            preconditions: RewritePreconditions::default(),
            baseline_plan: baseline_plan.clone(),
            transformed_plan: drifted_plan,
            baseline_trace: baseline_trace.clone(),
            transformed_trace: transformed_trace.clone(),
        };
        assert_eq!(
            certify_planner_rewrite(&gen_proposal),
            Err(RewriteRefusalReason::ChangedBasisOrNumericGeneration)
        );

        // Changed product frontier is refused (FA-INV-022).
        let mut frontier_drifted_plan = transformed_plan.clone();
        frontier_drifted_plan.nodes[2].operator = PhysicalOperatorKind::RequirePrefix {
            input: 1,
            requirement: FrontierRequirement {
                key: proj_key(),
                stage: FrontierStage::Captured,
                through: 4,
                closure: None,
            },
            claim_closed_prefix: false,
        };
        let frontier_proposal = RewriteProposal {
            transformed_plan: frontier_drifted_plan,
            ..gen_proposal.clone()
        };
        assert_eq!(
            certify_planner_rewrite(&frontier_proposal),
            Err(RewriteRefusalReason::ChangedProductFrontier)
        );

        // Permissive early-stop and non-decision-preserving classes are refused.
        let mut permissive_trace = baseline_trace;
        permissive_trace.decision = LabDecisionOutcome::AllowWithWitness { witness_digest: 7 };
        let mut permissive_trans_trace = transformed_trace;
        permissive_trans_trace.decision =
            LabDecisionOutcome::AllowWithWitness { witness_digest: 7 };
        let permissive_short_circuit = RewriteProposal {
            law: RewriteLawId::ShortCircuitExactDenyingPredicate,
            classification: OptimizationClass::DecisionPreserving,
            preconditions: RewritePreconditions::default(),
            baseline_plan,
            transformed_plan,
            baseline_trace: permissive_trace,
            transformed_trace: permissive_trans_trace,
        };
        assert_eq!(
            certify_planner_rewrite(&permissive_short_circuit),
            Err(RewriteRefusalReason::PermissiveEarlyStopForbidden)
        );

        let non_preserving = RewriteProposal {
            classification: OptimizationClass::DecisionAffecting,
            ..permissive_short_circuit
        };
        assert_eq!(
            certify_planner_rewrite(&non_preserving),
            Err(RewriteRefusalReason::NonDecisionPreservingClass(
                OptimizationClass::DecisionAffecting
            ))
        );
    }
}
