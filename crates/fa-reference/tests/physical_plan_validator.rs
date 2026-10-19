//! Integration tests for the FA-069 physical observation-plan validator
//! (`crates/fa-reference/src/observation_algebra/physical_plan.rs`).

use fa_reference::{
    observation_algebra::{
        AuthorityKind, AuthorityRequirements, Basis, BasisIdentity, CancellationObligations,
        ClaimClass, DecodeLaw, DecodeRequirement, Knowledge, LawDeclaration, MAX_OPERATOR_BYTES,
        MAX_OPERATOR_CANDIDATES, MAX_OPERATOR_DECODED_BYTES, MAX_OPERATOR_FRONTIERS,
        MAX_OPERATOR_INPUTS, MAX_OPERATOR_OUTPUTS, MAX_OPERATOR_WITNESSES, ObjectDeclaration,
        Obligation, ObligationKind, ObservationRequirement, OperatorContext, PrivacyLabel,
        PrivacyRestrictions, ProjectionSpec, ProofStates, RequirementClass, ResourceEnvelope,
        ScopedProof, UncertaintyClass,
        physical_plan::{
            PhysicalOperatorKind, PhysicalPlan, PhysicalPlanNode, PlanRefusalReason,
            validate_physical_plan,
        },
    },
    product_frontier::{
        FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    },
};

fn test_basis() -> Basis {
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
    .expect("valid reference resource envelope")
}

fn ctx(
    class: RequirementClass,
    uncertainty: UncertaintyClass,
    labels: Vec<PrivacyLabel>,
    obligations: Vec<Obligation>,
) -> OperatorContext {
    OperatorContext::new(
        ObservationRequirement {
            class,
            uncertainty,
            basis: test_basis(),
        },
        PrivacyRestrictions::new(labels).expect("valid privacy labels"),
        CancellationObligations::new(obligations).expect("valid obligations"),
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

fn all_authorities() -> AuthorityRequirements {
    AuthorityRequirements::new(vec![
        AuthorityKind::Observe,
        AuthorityKind::Project,
        AuthorityKind::Verify,
        AuthorityKind::ExactRead,
        AuthorityKind::Discover,
        AuthorityKind::Probe,
        AuthorityKind::Judge,
        AuthorityKind::CloseScope,
    ])
    .expect("valid authority set")
}

fn closed_frontiers() -> (ProductFrontiers, FrontierRequirement) {
    let key = proj_key();
    let marker = TrustedClosingMarker {
        key,
        final_sequence: 5,
        marker_generation: 43,
    };
    let mut frontiers = ProductFrontiers::new(2, 8).expect("bounded frontiers");
    for seq in 1..=5 {
        frontiers
            .accept(key, FrontierStage::Authenticated, seq)
            .expect("authenticated prefix");
    }
    frontiers.record_close(marker).expect("closed marker");
    let req = FrontierRequirement {
        key,
        stage: FrontierStage::Authenticated,
        through: 5,
        closure: Some(43),
    };
    (frontiers, req)
}

#[test]
fn valid_multi_branch_plan_rehearses_exact_lookup_refinement_checkpoint_decode_and_judgment() {
    let (frontiers, closed_req) = closed_frontiers();
    let label = PrivacyLabel {
        purpose_id: 1,
        transform_id: 10,
    };
    let allowed_privacy = PrivacyRestrictions::new(vec![label]).expect("allowed privacy");

    let nodes = vec![
        // 0: Tap
        PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 64,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![label],
                vec![Obligation {
                    id: 1,
                    kind: ObligationKind::RetainInput,
                }],
            ),
        },
        // 1: AuthorizeProjection
        PhysicalPlanNode {
            node_id: 2,
            operator: PhysicalOperatorKind::AuthorizeProjection {
                input: 0,
                projection: ProjectionSpec {
                    key: proj_key(),
                    projection_id: 31,
                },
            },
            context: ctx(
                RequirementClass::Projection,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 2: RequirePrefix (closed authenticated prefix)
        PhysicalPlanNode {
            node_id: 3,
            operator: PhysicalOperatorKind::RequirePrefix {
                input: 1,
                requirement: closed_req,
                claim_closed_prefix: true,
            },
            context: ctx(
                RequirementClass::Prefix,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 3: VerifyObject
        PhysicalPlanNode {
            node_id: 4,
            operator: PhysicalOperatorKind::VerifyObject {
                input: 2,
                declaration: ObjectDeclaration {
                    object_id: 17,
                    generation: 29,
                    profile_id: 3,
                },
                verifier_law: LawDeclaration::new(41, 29, 4, 3).expect("verifier law"),
            },
            context: ctx(
                RequirementClass::ExactObject,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 4: Decode (ExactCheckpoint with RegisteredLossless)
        PhysicalPlanNode {
            node_id: 5,
            operator: PhysicalOperatorKind::Decode {
                input: 3,
                object_id: 17,
                encoded_bytes: 32,
                decoded_bytes: 48,
                requirement: DecodeRequirement::ExactCheckpoint,
                law: DecodeLaw::RegisteredLossless(
                    LawDeclaration::new(51, 7, 4, 3).expect("lossless law"),
                ),
            },
            context: ctx(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 5: RefineWitness on the decoded checkpoint
        PhysicalPlanNode {
            node_id: 6,
            operator: PhysicalOperatorKind::RefineWitness {
                parent: 4,
                refined_bytes: 16,
                law: LawDeclaration::new(61, 4, 4, 3).expect("refinement law"),
            },
            context: ctx(
                RequirementClass::Refine,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 6: AwaitClosedScope + 7: LookupExact with absence proof
        PhysicalPlanNode {
            node_id: 7,
            operator: PhysicalOperatorKind::AwaitClosedScope {
                requirement: closed_req,
                domain_closed: true,
                marker_generation: 43,
                final_sequence: 5,
            },
            context: ctx(
                RequirementClass::ClosedScope,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 8,
            operator: PhysicalOperatorKind::LookupExact {
                key: 99,
                value_bytes: 0,
                closed_scope: Some(6),
                require_absence_proof: true,
            },
            context: ctx(
                RequirementClass::ExactLookup,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 8: JoinEvidence (combining refined witness branch 5 and exact lookup branch 7)
        PhysicalPlanNode {
            node_id: 9,
            operator: PhysicalOperatorKind::JoinEvidence { inputs: vec![5, 7] },
            context: ctx(
                RequirementClass::JoinedEvidence,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        // 9: JudgeIndependent
        PhysicalPlanNode {
            node_id: 10,
            operator: PhysicalOperatorKind::JudgeIndependent {
                submitted_bytes: 24,
                profile_id: 3,
            },
            context: ctx(
                RequirementClass::IndependentJudgment,
                UncertaintyClass::Opaque { calibration_id: 77 },
                vec![],
                vec![],
            ),
        },
        // 10: EmitJudgment
        PhysicalPlanNode {
            node_id: 11,
            operator: PhysicalOperatorKind::EmitJudgment {
                evidence: 8,
                judgment: 9,
            },
            context: ctx(
                RequirementClass::EmittedJudgment,
                UncertaintyClass::Opaque { calibration_id: 77 },
                vec![],
                vec![],
            ),
        },
    ];

    let plan = PhysicalPlan {
        nodes,
        root: 10,
        granted_authority: all_authorities(),
        allowed_privacy,
        frontiers,
        total_envelope: full_envelope(),
    };

    let outcome = validate_physical_plan(&plan).expect("valid plan must rehearse cleanly");
    let Knowledge::Known {
        value: summary,
        basis,
    } = outcome
    else {
        panic!("expected Known rehearsal summary, got {outcome:?}");
    };
    assert!(summary.is_rehearsal_only());
    assert_eq!(basis, test_basis());
    assert_eq!(summary.node_count(), 11);
    assert_eq!(
        summary.root_uncertainty(),
        UncertaintyClass::Opaque { calibration_id: 77 }
    );
    assert_eq!(summary.accumulated_privacy().labels(), &[label]);
    assert_eq!(summary.accumulated_frontiers(), &[closed_req]);
    assert!(summary.total_bytes() > 0);
    assert_eq!(summary.total_decoded_bytes(), 48);
}

#[test]
fn rejects_ann_for_absence_in_lookup_and_discovery_nodes() {
    let (frontiers, closed_req) = closed_frontiers();

    // Case 1: LookupExact requiring absence proof wired to a DiscoverSimilar node
    let nodes = vec![
        PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 16,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 2,
            operator: PhysicalOperatorKind::AuthorizeProjection {
                input: 0,
                projection: ProjectionSpec {
                    key: proj_key(),
                    projection_id: 31,
                },
            },
            context: ctx(
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
                requirement: closed_req,
                claim_closed_prefix: true,
            },
            context: ctx(
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
                    profile_id: 3,
                },
                verifier_law: LawDeclaration::new(41, 29, 4, 3).expect("law"),
            },
            context: ctx(
                RequirementClass::ExactObject,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 5,
            operator: PhysicalOperatorKind::DiscoverSimilar {
                input: 3,
                query_id: 1,
                residual_profile_id: 9,
                candidates: 4,
            },
            context: ctx(
                RequirementClass::ApproximateDiscovery,
                UncertaintyClass::Approximate { profile_id: 9 },
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 6,
            operator: PhysicalOperatorKind::LookupExact {
                key: 100,
                value_bytes: 0,
                closed_scope: Some(4),
                require_absence_proof: true,
            },
            context: ctx(
                RequirementClass::ExactLookup,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
    ];
    let plan = PhysicalPlan {
        nodes,
        root: 5,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&plan),
        Err(PlanRefusalReason::AnnForAbsence { node_index: 5 })
    );

    // Case 2: DiscoverSimilar attempting to claim Universal/Exact absence semantics directly
    let mut plan2 = plan;
    plan2.nodes.pop();
    plan2.root = 4;
    plan2.nodes[4].context = ctx(
        RequirementClass::Universal,
        UncertaintyClass::Exact,
        vec![],
        vec![],
    );
    assert_eq!(
        validate_physical_plan(&plan2),
        Err(PlanRefusalReason::AnnForAbsence { node_index: 4 })
    );
}

#[test]
fn rejects_lossy_decode_for_exact_restart_checkpoint() {
    let (frontiers, closed_req) = closed_frontiers();
    let nodes = vec![
        PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 16,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 2,
            operator: PhysicalOperatorKind::AuthorizeProjection {
                input: 0,
                projection: ProjectionSpec {
                    key: proj_key(),
                    projection_id: 31,
                },
            },
            context: ctx(
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
                requirement: closed_req,
                claim_closed_prefix: true,
            },
            context: ctx(
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
                    profile_id: 3,
                },
                verifier_law: LawDeclaration::new(41, 29, 4, 3).expect("law"),
            },
            context: ctx(
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
                encoded_bytes: 16,
                decoded_bytes: 32,
                requirement: DecodeRequirement::ExactCheckpoint,
                law: DecodeLaw::Lossy {
                    profile_id: 3,
                    error_bound_id: 99,
                },
            },
            context: ctx(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
    ];
    let plan = PhysicalPlan {
        nodes,
        root: 4,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers,
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&plan),
        Err(PlanRefusalReason::LossyForRestart { node_index: 4 })
    );
}

#[test]
fn rejects_premature_prefix_closure_while_open_prefix_returns_pending() {
    let key = proj_key();
    let mut frontiers = ProductFrontiers::new(2, 8).expect("bounded frontiers");
    frontiers
        .accept(key, FrontierStage::Captured, 2)
        .expect("captured prefix");
    let unclosed_req = FrontierRequirement {
        key,
        stage: FrontierStage::Captured,
        through: 5,
        closure: None,
    };

    let base_nodes = vec![
        PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 16,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
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
            context: ctx(
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
                requirement: unclosed_req,
                claim_closed_prefix: true,
            },
            context: ctx(
                RequirementClass::Prefix,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
    ];
    let premature_plan = PhysicalPlan {
        nodes: base_nodes.clone(),
        root: 2,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&premature_plan),
        Err(PlanRefusalReason::PrematurePrefixClosure {
            node_index: 2,
            requirement: unclosed_req,
        })
    );

    // Near-identical positive control: when closure is not claimed prematurely,
    // an unsatisfied prefix yields Knowledge::Pending rather than Known.
    let mut open_plan = premature_plan;
    open_plan.nodes[2].operator = PhysicalOperatorKind::RequirePrefix {
        input: 1,
        requirement: unclosed_req,
        claim_closed_prefix: false,
    };
    assert_eq!(
        validate_physical_plan(&open_plan),
        Ok(Knowledge::Pending {
            frontiers: vec![unclosed_req],
            expected_cost: 1,
        })
    );
}

#[test]
fn rejects_missing_authorization_unclosed_stream_and_unsatisfied_fallback() {
    let (frontiers, closed_req) = closed_frontiers();

    // 1. MissingAuthorization when Observe is omitted from granted_authority
    let tap_only = PhysicalPlan {
        nodes: vec![PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 8,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        }],
        root: 0,
        granted_authority: AuthorityRequirements::new(vec![AuthorityKind::Project]).unwrap(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&tap_only),
        Err(PlanRefusalReason::MissingAuthorization {
            node_index: 0,
            required: AuthorityKind::Observe,
        })
    );

    // 2. UnclosedStream when LookupExact requires absence proof without a closed scope
    let unclosed_lookup = PhysicalPlan {
        nodes: vec![PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::LookupExact {
                key: 7,
                value_bytes: 0,
                closed_scope: None,
                require_absence_proof: true,
            },
            context: ctx(
                RequirementClass::ExactLookup,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        }],
        root: 0,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&unclosed_lookup),
        Err(PlanRefusalReason::UnclosedStream { node_index: 0 })
    );

    // 3. UnsatisfiedFallback when Probe margin lies inside the error interval without an exact fallback
    let probe_nodes = vec![
        PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 16,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 2,
            operator: PhysicalOperatorKind::AuthorizeProjection {
                input: 0,
                projection: ProjectionSpec {
                    key: proj_key(),
                    projection_id: 31,
                },
            },
            context: ctx(
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
                requirement: closed_req,
                claim_closed_prefix: true,
            },
            context: ctx(
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
                    profile_id: 3,
                },
                verifier_law: LawDeclaration::new(41, 29, 4, 3).expect("law"),
            },
            context: ctx(
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
                encoded_bytes: 16,
                decoded_bytes: 16,
                requirement: DecodeRequirement::ApproximateValue { profile_id: 3 },
                law: DecodeLaw::Lossy {
                    profile_id: 3,
                    error_bound_id: 12,
                },
            },
            context: ctx(
                RequirementClass::ApproximateValue,
                UncertaintyClass::Approximate { profile_id: 3 },
                vec![],
                vec![],
            ),
        },
        PhysicalPlanNode {
            node_id: 6,
            operator: PhysicalOperatorKind::Probe {
                input: 4,
                probe_id: 1,
                bound_id: 12,
                signed_margin: 3,
                error_bound: 5,
                fallback: None,
            },
            context: ctx(
                RequirementClass::Probe,
                UncertaintyClass::Bounded { bound_id: 12 },
                vec![],
                vec![],
            ),
        },
    ];
    let mut probe_plan = PhysicalPlan {
        nodes: probe_nodes,
        root: 5,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers,
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&probe_plan),
        Err(PlanRefusalReason::UnsatisfiedFallback { node_index: 5 })
    );

    // Near-identical positive control: supplying the exact VerifiedObject node (3) as fallback succeeds
    probe_plan.nodes[5].operator = PhysicalOperatorKind::Probe {
        input: 4,
        probe_id: 1,
        bound_id: 12,
        signed_margin: 3,
        error_bound: 5,
        fallback: Some(3),
    };
    assert!(matches!(
        validate_physical_plan(&probe_plan),
        Ok(Knowledge::Known { .. })
    ));
}

#[test]
fn rejects_cycles_duplicate_ids_privacy_escape_and_envelope_exhaustion() {
    let (frontiers, _) = closed_frontiers();

    // Forward/self dependency (cycle)
    let cycle_plan = PhysicalPlan {
        nodes: vec![PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::AuthorizeProjection {
                input: 0,
                projection: ProjectionSpec {
                    key: proj_key(),
                    projection_id: 31,
                },
            },
            context: ctx(
                RequirementClass::Projection,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        }],
        root: 0,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&cycle_plan),
        Err(PlanRefusalReason::CycleOrForwardDependency {
            node_index: 0,
            dependency: 0,
        })
    );

    // Privacy boundary exceeded
    let secret = PrivacyLabel {
        purpose_id: 9,
        transform_id: 9,
    };
    let privacy_plan = PhysicalPlan {
        nodes: vec![PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 8,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![secret],
                vec![],
            ),
        }],
        root: 0,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers: frontiers.clone(),
        total_envelope: full_envelope(),
    };
    assert_eq!(
        validate_physical_plan(&privacy_plan),
        Err(PlanRefusalReason::PrivacyBoundaryExceeded {
            node_index: 0,
            label: secret,
        })
    );

    // Total resource envelope exceeded
    let tight_envelope = ResourceEnvelope::new(2, 2, 4, 2, 16, 16, 4).unwrap();
    let over_budget_plan = PhysicalPlan {
        nodes: vec![PhysicalPlanNode {
            node_id: 1,
            operator: PhysicalOperatorKind::Tap {
                source_id: 17,
                source_generation: 29,
                bytes: 32,
            },
            context: ctx(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                vec![],
                vec![],
            ),
        }],
        root: 0,
        granted_authority: all_authorities(),
        allowed_privacy: PrivacyRestrictions::new(vec![]).unwrap(),
        frontiers,
        total_envelope: tight_envelope,
    };
    assert_eq!(
        validate_physical_plan(&over_budget_plan),
        Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: 0 })
    );
}
