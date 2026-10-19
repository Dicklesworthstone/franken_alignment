//! Physical observation-plan validator (FA-069, plan §§14.1, 14.7, 14.9).
//!
//! Validates a bounded physical observation DAG against registered operator and
//! requirement laws, authority and privacy boundaries, product frontiers, and
//! cumulative resource envelopes before execution.
//!
//! Validation is strictly a dry rehearsal (`FA-INV-040`): it performs no host
//! observations, mints no permits, and produces a `Knowledge<RehearsedPlanSummary>`
//! value (`FA-INV-039`) or a typed `PlanRefusalReason`.

use std::collections::BTreeSet;

use crate::{
    Error,
    product_frontier::{FrontierRequirement, FrontierStage, ProductFrontiers},
};

use super::{
    AuthorityKind, AuthorityRequirements, Basis, CancellationObligations, DecodeLaw,
    DecodeRequirement, Knowledge, LawDeclaration, MAX_OPERATOR_FRONTIERS, ObjectDeclaration,
    OperatorContext, PrivacyLabel, PrivacyRestrictions, ProjectionSpec, RequirementClass,
    ResourceEnvelope, UncertaintyClass, union_frontiers,
};

pub const MAX_PLAN_NODES: usize = 32;
pub const MAX_PLAN_EDGES: usize = 64;

/// Physical operator variants in a bounded observation DAG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhysicalOperatorKind {
    Tap {
        source_id: u64,
        source_generation: u64,
        bytes: usize,
    },
    AuthorizeProjection {
        input: usize,
        projection: ProjectionSpec,
    },
    RequirePrefix {
        input: usize,
        requirement: FrontierRequirement,
        claim_closed_prefix: bool,
    },
    VerifyObject {
        input: usize,
        declaration: ObjectDeclaration,
        verifier_law: LawDeclaration,
    },
    LookupExact {
        key: u64,
        value_bytes: usize,
        closed_scope: Option<usize>,
        require_absence_proof: bool,
    },
    DiscoverSimilar {
        input: usize,
        query_id: u64,
        residual_profile_id: u64,
        candidates: usize,
    },
    RefineWitness {
        parent: usize,
        refined_bytes: usize,
        law: LawDeclaration,
    },
    Decode {
        input: usize,
        object_id: u64,
        encoded_bytes: usize,
        decoded_bytes: usize,
        requirement: DecodeRequirement,
        law: DecodeLaw,
    },
    Probe {
        input: usize,
        probe_id: u64,
        bound_id: u64,
        signed_margin: i64,
        error_bound: i64,
        fallback: Option<usize>,
    },
    JudgeIndependent {
        submitted_bytes: usize,
        profile_id: u64,
    },
    AwaitClosedScope {
        requirement: FrontierRequirement,
        domain_closed: bool,
        marker_generation: u64,
        final_sequence: u64,
    },
    JoinEvidence {
        inputs: Vec<usize>,
    },
    EmitJudgment {
        evidence: usize,
        judgment: usize,
    },
}

/// One node in a physical observation DAG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalPlanNode {
    pub node_id: u64,
    pub operator: PhysicalOperatorKind,
    pub context: OperatorContext,
}

/// Bounded physical observation plan submitted for pre-execution validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalPlan {
    pub nodes: Vec<PhysicalPlanNode>,
    pub root: usize,
    pub granted_authority: AuthorityRequirements,
    pub allowed_privacy: PrivacyRestrictions,
    pub frontiers: ProductFrontiers,
    pub total_envelope: ResourceEnvelope,
}

/// Typed refusal reasons for invalid physical observation plans (FA-069).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRefusalReason {
    EmptyOrOversizedPlan,
    CycleOrForwardDependency {
        node_index: usize,
        dependency: usize,
    },
    DuplicateNodeId {
        node_id: u64,
    },
    AnnForAbsence {
        node_index: usize,
    },
    LossyForRestart {
        node_index: usize,
    },
    PrematurePrefixClosure {
        node_index: usize,
        requirement: FrontierRequirement,
    },
    MissingAuthorization {
        node_index: usize,
        required: AuthorityKind,
    },
    UnclosedStream {
        node_index: usize,
    },
    UnsatisfiedFallback {
        node_index: usize,
    },
    LawOrBasisMismatch {
        node_index: usize,
    },
    PrivacyBoundaryExceeded {
        node_index: usize,
        label: PrivacyLabel,
    },
    ResourceEnvelopeExceeded {
        node_index: usize,
    },
}

/// Summary of a validated physical observation plan.
///
/// Validation is strictly rehearsal (`FA-INV-040`) and cannot be converted into
/// an execution permit:
///
/// ```compile_fail,E0599
/// use fa_reference::observation_algebra::physical_plan::RehearsedPlanSummary;
/// fn forge_permit(summary: RehearsedPlanSummary) {
///     let _ = summary.into_permit();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RehearsedPlanSummary {
    rehearsal_only: bool,
    node_count: usize,
    edge_count: usize,
    root_basis: Basis,
    root_uncertainty: UncertaintyClass,
    required_authority: AuthorityRequirements,
    accumulated_privacy: PrivacyRestrictions,
    accumulated_frontiers: Vec<FrontierRequirement>,
    accumulated_obligations: CancellationObligations,
    total_bytes: usize,
    total_decoded_bytes: usize,
    total_witnesses: usize,
}

impl RehearsedPlanSummary {
    pub const fn is_rehearsal_only(&self) -> bool {
        self.rehearsal_only
    }

    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn root_basis(&self) -> Basis {
        self.root_basis
    }

    pub const fn root_uncertainty(&self) -> UncertaintyClass {
        self.root_uncertainty
    }

    pub fn required_authority(&self) -> &AuthorityRequirements {
        &self.required_authority
    }

    pub fn accumulated_privacy(&self) -> &PrivacyRestrictions {
        &self.accumulated_privacy
    }

    pub fn accumulated_frontiers(&self) -> &[FrontierRequirement] {
        &self.accumulated_frontiers
    }

    pub fn accumulated_obligations(&self) -> &CancellationObligations {
        &self.accumulated_obligations
    }

    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub const fn total_decoded_bytes(&self) -> usize {
        self.total_decoded_bytes
    }

    pub const fn total_witnesses(&self) -> usize {
        self.total_witnesses
    }
}

#[derive(Clone, Debug)]
struct ValidatedNodeState {
    basis: Basis,
    uncertainty: UncertaintyClass,
    authority: AuthorityRequirements,
    privacy: PrivacyRestrictions,
    frontiers: Vec<FrontierRequirement>,
    obligations: CancellationObligations,
    pending_frontiers: Vec<FrontierRequirement>,
    witness_count: usize,
    bytes: usize,
    decoded_bytes: usize,
}

/// Validates a physical observation plan against operator/requirement laws,
/// authority/privacy boundaries, product frontiers, and resource envelopes.
pub fn validate_physical_plan(
    plan: &PhysicalPlan,
) -> Result<Knowledge<RehearsedPlanSummary>, PlanRefusalReason> {
    if plan.nodes.is_empty() || plan.nodes.len() > MAX_PLAN_NODES || plan.root >= plan.nodes.len() {
        return Err(PlanRefusalReason::EmptyOrOversizedPlan);
    }

    let mut seen_ids = BTreeSet::new();
    let mut total_edges = 0_usize;
    let mut states: Vec<ValidatedNodeState> = Vec::with_capacity(plan.nodes.len());

    for (idx, node) in plan.nodes.iter().enumerate() {
        if node.node_id == 0 || !seen_ids.insert(node.node_id) {
            return Err(PlanRefusalReason::DuplicateNodeId {
                node_id: node.node_id,
            });
        }

        let deps = operator_dependencies(&node.operator);
        if deps.len() > node.context.resources().max_inputs
            || deps.len() > plan.total_envelope.max_inputs()
        {
            return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
        }
        total_edges = total_edges
            .checked_add(deps.len())
            .ok_or(PlanRefusalReason::EmptyOrOversizedPlan)?;
        if total_edges > MAX_PLAN_EDGES {
            return Err(PlanRefusalReason::EmptyOrOversizedPlan);
        }

        for &dep in &deps {
            if dep >= idx {
                return Err(PlanRefusalReason::CycleOrForwardDependency {
                    node_index: idx,
                    dependency: dep,
                });
            }
        }

        for label in node.context.privacy.labels() {
            if !plan.allowed_privacy.labels().contains(label) {
                return Err(PlanRefusalReason::PrivacyBoundaryExceeded {
                    node_index: idx,
                    label: *label,
                });
            }
        }

        let state = validate_node(idx, node, &states, plan)?;
        for req_auth in state.authority.kinds() {
            if !plan.granted_authority.kinds().contains(req_auth) {
                return Err(PlanRefusalReason::MissingAuthorization {
                    node_index: idx,
                    required: *req_auth,
                });
            }
        }

        if state.bytes > plan.total_envelope.max_bytes()
            || state.decoded_bytes > plan.total_envelope.max_decoded_bytes()
            || state.witness_count > plan.total_envelope.max_witnesses()
            || state.frontiers.len() > plan.total_envelope.max_frontiers()
        {
            return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
        }

        states.push(state);
    }

    let root_state = &states[plan.root];
    if !root_state.pending_frontiers.is_empty() {
        return Knowledge::pending(
            root_state.pending_frontiers.clone(),
            root_state.pending_frontiers.len() as u64,
        )
        .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded {
            node_index: plan.root,
        });
    }

    let summary = RehearsedPlanSummary {
        rehearsal_only: true,
        node_count: plan.nodes.len(),
        edge_count: total_edges,
        root_basis: root_state.basis,
        root_uncertainty: root_state.uncertainty,
        required_authority: root_state.authority.clone(),
        accumulated_privacy: root_state.privacy.clone(),
        accumulated_frontiers: root_state.frontiers.clone(),
        accumulated_obligations: root_state.obligations.clone(),
        total_bytes: root_state.bytes,
        total_decoded_bytes: root_state.decoded_bytes,
        total_witnesses: root_state.witness_count,
    };

    Ok(Knowledge::known(summary, root_state.basis))
}

fn operator_dependencies(op: &PhysicalOperatorKind) -> Vec<usize> {
    match op {
        PhysicalOperatorKind::Tap { .. }
        | PhysicalOperatorKind::JudgeIndependent { .. }
        | PhysicalOperatorKind::AwaitClosedScope { .. } => Vec::new(),
        PhysicalOperatorKind::AuthorizeProjection { input, .. }
        | PhysicalOperatorKind::RequirePrefix { input, .. }
        | PhysicalOperatorKind::VerifyObject { input, .. }
        | PhysicalOperatorKind::DiscoverSimilar { input, .. }
        | PhysicalOperatorKind::RefineWitness { parent: input, .. }
        | PhysicalOperatorKind::Decode { input, .. } => vec![*input],
        PhysicalOperatorKind::LookupExact { closed_scope, .. } => {
            (*closed_scope).into_iter().collect()
        }
        PhysicalOperatorKind::Probe {
            input, fallback, ..
        } => {
            let mut deps = vec![*input];
            if let Some(fb) = fallback {
                deps.push(*fb);
            }
            deps
        }
        PhysicalOperatorKind::JoinEvidence { inputs } => inputs.clone(),
        PhysicalOperatorKind::EmitJudgment { evidence, judgment } => vec![*evidence, *judgment],
    }
}

fn validate_node(
    idx: usize,
    node: &PhysicalPlanNode,
    prior: &[ValidatedNodeState],
    plan: &PhysicalPlan,
) -> Result<ValidatedNodeState, PlanRefusalReason> {
    let ctx = &node.context;
    let req = ctx.requirement();
    let res = ctx.resources();

    match &node.operator {
        PhysicalOperatorKind::Tap {
            source_id, bytes, ..
        } => {
            if req.class != RequirementClass::Capture || req.uncertainty != UncertaintyClass::Exact
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *source_id == 0 {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *bytes > res.max_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority: make_auth(idx, &[AuthorityKind::Observe])?,
                privacy: ctx.privacy.clone(),
                frontiers: Vec::new(),
                obligations: ctx.cancellation.clone(),
                pending_frontiers: Vec::new(),
                witness_count: 1,
                bytes: *bytes,
                decoded_bytes: 0,
            })
        }
        PhysicalOperatorKind::AuthorizeProjection { input, projection } => {
            if req.class != RequirementClass::Projection
                || req.uncertainty != UncertaintyClass::Exact
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let up = &prior[*input];
            if up.basis != req.basis || projection.projection_id == 0 {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if let PhysicalOperatorKind::Tap {
                source_id,
                source_generation,
                ..
            } = plan.nodes[*input].operator
            {
                if projection.key.source != source_id
                    || projection.key.projection != projection.projection_id
                    || projection.key.source_epoch != source_generation
                {
                    return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                }
            } else {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[AuthorityKind::Project],
                1,
                0,
                0,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::RequirePrefix {
            input,
            requirement,
            claim_closed_prefix,
        } => {
            if req.class != RequirementClass::Prefix || req.uncertainty != UncertaintyClass::Exact {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let up = &prior[*input];
            if up.basis != req.basis {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let PhysicalOperatorKind::AuthorizeProjection { projection, .. } =
                &plan.nodes[*input].operator
            else {
                return Err(PlanRefusalReason::MissingAuthorization {
                    node_index: idx,
                    required: AuthorityKind::Project,
                });
            };
            if requirement.key != projection.key {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if requirement.closure == Some(0) {
                return Err(PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                });
            }
            let satisfied = plan.frontiers.satisfies(*requirement).map_err(|_| {
                PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                }
            })?;
            if *claim_closed_prefix
                && (!satisfied
                    || requirement.stage != FrontierStage::Authenticated
                    || requirement.closure.is_none())
            {
                return Err(PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                });
            }
            let pending = if satisfied {
                Vec::new()
            } else {
                vec![*requirement]
            };
            inherit_single(
                idx,
                up,
                ctx,
                &[AuthorityKind::Observe],
                1,
                0,
                0,
                vec![*requirement],
                pending,
            )
        }
        PhysicalOperatorKind::VerifyObject {
            input,
            declaration,
            verifier_law,
        } => {
            if req.class != RequirementClass::ExactObject
                || req.uncertainty != UncertaintyClass::Exact
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let up = &prior[*input];
            if up.basis != req.basis
                || declaration.object_id == 0
                || declaration.profile_id != req.basis.profile_id()
                || verifier_law.law_id == 0
                || verifier_law.profile_id != declaration.profile_id
                || verifier_law.input_generation != declaration.generation
                || verifier_law.output_generation != req.basis.semantic_generation()
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if !matches!(
                plan.nodes[*input].operator,
                PhysicalOperatorKind::RequirePrefix { .. }
            ) {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[AuthorityKind::Verify],
                1,
                0,
                0,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::LookupExact {
            key,
            value_bytes,
            closed_scope,
            require_absence_proof,
        } => {
            if req.class == RequirementClass::ApproximateDiscovery
                || matches!(req.uncertainty, UncertaintyClass::Approximate { .. })
            {
                return Err(PlanRefusalReason::AnnForAbsence { node_index: idx });
            }
            if req.class != RequirementClass::ExactLookup
                || req.uncertainty != UncertaintyClass::Exact
                || *key == 0
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *value_bytes > res.max_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            if *require_absence_proof {
                let Some(scope_idx) = closed_scope else {
                    return Err(PlanRefusalReason::UnclosedStream { node_index: idx });
                };
                match &plan.nodes[*scope_idx].operator {
                    PhysicalOperatorKind::DiscoverSimilar { .. } => {
                        return Err(PlanRefusalReason::AnnForAbsence { node_index: idx });
                    }
                    PhysicalOperatorKind::AwaitClosedScope { domain_closed, .. } => {
                        if !domain_closed {
                            return Err(PlanRefusalReason::UnclosedStream { node_index: idx });
                        }
                    }
                    _ => {
                        return Err(PlanRefusalReason::AnnForAbsence { node_index: idx });
                    }
                }
                let up = &prior[*scope_idx];
                if up.basis != req.basis || up.uncertainty != UncertaintyClass::Exact {
                    return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                }
                return inherit_single(
                    idx,
                    up,
                    ctx,
                    &[AuthorityKind::ExactRead],
                    2,
                    *value_bytes,
                    0,
                    Vec::new(),
                    Vec::new(),
                );
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority: make_auth(idx, &[AuthorityKind::ExactRead])?,
                privacy: ctx.privacy.clone(),
                frontiers: Vec::new(),
                obligations: ctx.cancellation.clone(),
                pending_frontiers: Vec::new(),
                witness_count: 2,
                bytes: *value_bytes,
                decoded_bytes: 0,
            })
        }
        PhysicalOperatorKind::DiscoverSimilar {
            input,
            query_id,
            residual_profile_id,
            candidates,
        } => {
            if matches!(
                req.class,
                RequirementClass::ExactLookup
                    | RequirementClass::Universal
                    | RequirementClass::ClosedScope
            ) || req.uncertainty == UncertaintyClass::Exact
            {
                return Err(PlanRefusalReason::AnnForAbsence { node_index: idx });
            }
            if req.class != RequirementClass::ApproximateDiscovery
                || !matches!(req.uncertainty, UncertaintyClass::Approximate { profile_id } if profile_id == *residual_profile_id)
                || *query_id == 0
                || *residual_profile_id == 0
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *candidates > res.max_candidates()
                || *candidates > plan.total_envelope.max_candidates()
            {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            let up = &prior[*input];
            if up.basis != req.basis {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[AuthorityKind::Discover],
                1,
                0,
                0,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::RefineWitness {
            parent,
            refined_bytes,
            law,
        } => {
            if req.class != RequirementClass::Refine
                || !matches!(
                    req.uncertainty,
                    UncertaintyClass::Exact | UncertaintyClass::Conservative
                )
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let up = &prior[*parent];
            if up.basis != req.basis
                || !matches!(
                    up.uncertainty,
                    UncertaintyClass::Exact | UncertaintyClass::Conservative
                )
                || law.law_id == 0
                || law.profile_id != req.basis.profile_id()
                || law.input_generation != req.basis.semantic_generation()
                || law.output_generation != req.basis.semantic_generation()
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *refined_bytes > res.max_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[],
                1,
                *refined_bytes,
                0,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::Decode {
            input,
            object_id,
            encoded_bytes,
            decoded_bytes,
            requirement,
            law,
        } => {
            let up = &prior[*input];
            if *object_id == 0 || up.basis != req.basis {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *encoded_bytes > res.max_bytes() || *decoded_bytes > res.max_decoded_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            match (requirement, law) {
                (DecodeRequirement::ExactCheckpoint, DecodeLaw::Lossy { .. }) => {
                    return Err(PlanRefusalReason::LossyForRestart { node_index: idx });
                }
                (DecodeRequirement::ExactCheckpoint, DecodeLaw::RegisteredLossless(decl)) => {
                    if req.class != RequirementClass::ExactCheckpoint
                        || req.uncertainty != UncertaintyClass::Exact
                        || up.uncertainty != UncertaintyClass::Exact
                    {
                        return Err(PlanRefusalReason::LossyForRestart { node_index: idx });
                    }
                    if decl.law_id == 0
                        || decl.profile_id != req.basis.profile_id()
                        || decl.input_generation != req.basis.codec_epoch()
                        || decl.output_generation != req.basis.semantic_generation()
                    {
                        return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                    }
                }
                (
                    DecodeRequirement::ApproximateValue { profile_id },
                    DecodeLaw::Lossy {
                        profile_id: law_profile,
                        error_bound_id,
                    },
                ) => {
                    if req.class != RequirementClass::ApproximateValue
                        || req.uncertainty
                            != (UncertaintyClass::Approximate {
                                profile_id: *profile_id,
                            })
                        || up.uncertainty != UncertaintyClass::Exact
                        || profile_id != law_profile
                        || *profile_id != req.basis.profile_id()
                        || *error_bound_id == 0
                    {
                        return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                    }
                }
                (
                    DecodeRequirement::ApproximateValue { profile_id },
                    DecodeLaw::RegisteredLossless(decl),
                ) => {
                    if req.class != RequirementClass::ApproximateValue
                        || req.uncertainty
                            != (UncertaintyClass::Approximate {
                                profile_id: *profile_id,
                            })
                        || up.uncertainty != UncertaintyClass::Exact
                        || *profile_id != decl.profile_id
                        || decl.profile_id != req.basis.profile_id()
                        || decl.input_generation != req.basis.codec_epoch()
                        || decl.output_generation != req.basis.semantic_generation()
                    {
                        return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                    }
                }
            }
            let combined = encoded_bytes
                .checked_add(*decoded_bytes)
                .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            if combined > res.max_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[],
                3,
                combined,
                *decoded_bytes,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::Probe {
            input,
            probe_id,
            bound_id,
            signed_margin,
            error_bound,
            fallback,
        } => {
            if req.class != RequirementClass::Probe
                || req.uncertainty
                    != (UncertaintyClass::Bounded {
                        bound_id: *bound_id,
                    })
                || *probe_id == 0
                || *bound_id == 0
                || *error_bound < 0
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let up = &prior[*input];
            if up.basis != req.basis {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let margin_inside_interval = signed_margin.unsigned_abs() <= (*error_bound as u64);
            if margin_inside_interval
                || matches!(up.uncertainty, UncertaintyClass::Approximate { .. })
            {
                let Some(fb_idx) = fallback else {
                    return Err(PlanRefusalReason::UnsatisfiedFallback { node_index: idx });
                };
                let fb_state = &prior[*fb_idx];
                if fb_state.basis != req.basis
                    || !matches!(
                        fb_state.uncertainty,
                        UncertaintyClass::Exact | UncertaintyClass::Conservative
                    )
                {
                    return Err(PlanRefusalReason::UnsatisfiedFallback { node_index: idx });
                }
            }
            inherit_single(
                idx,
                up,
                ctx,
                &[AuthorityKind::Probe],
                1,
                0,
                0,
                Vec::new(),
                Vec::new(),
            )
        }
        PhysicalOperatorKind::JudgeIndependent {
            submitted_bytes,
            profile_id,
        } => {
            if req.class != RequirementClass::IndependentJudgment
                || !matches!(req.uncertainty, UncertaintyClass::Opaque { calibration_id } if calibration_id != 0)
                || *profile_id != req.basis.profile_id()
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if *submitted_bytes > res.max_bytes() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority: make_auth(idx, &[AuthorityKind::Judge])?,
                privacy: ctx.privacy.clone(),
                frontiers: Vec::new(),
                obligations: ctx.cancellation.clone(),
                pending_frontiers: Vec::new(),
                witness_count: 2,
                bytes: *submitted_bytes,
                decoded_bytes: 0,
            })
        }
        PhysicalOperatorKind::AwaitClosedScope {
            requirement,
            domain_closed,
            marker_generation,
            final_sequence,
        } => {
            if req.class != RequirementClass::ClosedScope
                || req.uncertainty != UncertaintyClass::Exact
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if !domain_closed {
                return Err(PlanRefusalReason::UnclosedStream { node_index: idx });
            }
            if requirement.stage != FrontierStage::Authenticated
                || *marker_generation == 0
                || requirement.closure != Some(*marker_generation)
                || requirement.through != *final_sequence
            {
                return Err(PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                });
            }
            let satisfied = plan.frontiers.satisfies(*requirement).map_err(|_| {
                PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                }
            })?;
            if !satisfied {
                return Err(PlanRefusalReason::PrematurePrefixClosure {
                    node_index: idx,
                    requirement: *requirement,
                });
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority: make_auth(idx, &[AuthorityKind::CloseScope])?,
                privacy: ctx.privacy.clone(),
                frontiers: vec![*requirement],
                obligations: ctx.cancellation.clone(),
                pending_frontiers: Vec::new(),
                witness_count: 2,
                bytes: 0,
                decoded_bytes: 0,
            })
        }
        PhysicalOperatorKind::JoinEvidence { inputs } => {
            if req.class != RequirementClass::JoinedEvidence || inputs.is_empty() {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            if inputs.len() > res.max_inputs() {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            let mut authority = AuthorityRequirements::new(Vec::new())
                .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let mut privacy = ctx.privacy.clone();
            let mut obligations = ctx.cancellation.clone();
            let mut frontiers = Vec::new();
            let mut pending_frontiers = Vec::new();
            let mut witness_count = 0_usize;
            let mut bytes = 0_usize;
            let mut decoded_bytes = 0_usize;

            for &dep in inputs {
                let up = &prior[dep];
                if up.basis != req.basis || up.uncertainty != req.uncertainty {
                    return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
                }
                authority = authority
                    .union(&up.authority)
                    .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                privacy = privacy
                    .union(&up.privacy)
                    .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                obligations = obligations
                    .union(&up.obligations)
                    .map_err(|_| PlanRefusalReason::LawOrBasisMismatch { node_index: idx })?;
                frontiers = union_frontiers(&frontiers, &up.frontiers, res.max_frontiers())
                    .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                pending_frontiers = union_frontiers(
                    &pending_frontiers,
                    &up.pending_frontiers,
                    MAX_OPERATOR_FRONTIERS,
                )
                .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                witness_count = witness_count
                    .checked_add(up.witness_count)
                    .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                bytes = bytes
                    .checked_add(up.bytes)
                    .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
                decoded_bytes = decoded_bytes
                    .checked_add(up.decoded_bytes)
                    .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            }
            if witness_count > res.max_witnesses()
                || bytes > res.max_bytes()
                || decoded_bytes > res.max_decoded_bytes()
            {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority,
                privacy,
                frontiers,
                obligations,
                pending_frontiers,
                witness_count,
                bytes,
                decoded_bytes,
            })
        }
        PhysicalOperatorKind::EmitJudgment { evidence, judgment } => {
            if req.class != RequirementClass::EmittedJudgment
                || !matches!(req.uncertainty, UncertaintyClass::Opaque { calibration_id } if calibration_id != 0)
            {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let ev = &prior[*evidence];
            let jd = &prior[*judgment];
            if ev.basis != req.basis || jd.basis != req.basis || jd.uncertainty != req.uncertainty {
                return Err(PlanRefusalReason::LawOrBasisMismatch { node_index: idx });
            }
            let own_auth = make_auth(idx, &[AuthorityKind::Judge])?;
            let authority = ev
                .authority
                .union(&jd.authority)
                .and_then(|a| a.union(&own_auth))
                .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let privacy = ev
                .privacy
                .union(&jd.privacy)
                .and_then(|p| p.union(&ctx.privacy))
                .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let obligations = ev
                .obligations
                .union(&jd.obligations)
                .and_then(|o| o.union(&ctx.cancellation))
                .map_err(|_| PlanRefusalReason::LawOrBasisMismatch { node_index: idx })?;
            let frontiers = union_frontiers(&ev.frontiers, &jd.frontiers, res.max_frontiers())
                .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let pending_frontiers = union_frontiers(
                &ev.pending_frontiers,
                &jd.pending_frontiers,
                MAX_OPERATOR_FRONTIERS,
            )
            .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let witness_count = ev
                .witness_count
                .checked_add(jd.witness_count)
                .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let bytes = ev
                .bytes
                .checked_add(jd.bytes)
                .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            let decoded_bytes = ev
                .decoded_bytes
                .checked_add(jd.decoded_bytes)
                .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
            if witness_count > res.max_witnesses()
                || bytes > res.max_bytes()
                || decoded_bytes > res.max_decoded_bytes()
            {
                return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
            }
            Ok(ValidatedNodeState {
                basis: req.basis,
                uncertainty: req.uncertainty,
                authority,
                privacy,
                frontiers,
                obligations,
                pending_frontiers,
                witness_count,
                bytes,
                decoded_bytes,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inherit_single(
    idx: usize,
    up: &ValidatedNodeState,
    ctx: &OperatorContext,
    added_auth: &[AuthorityKind],
    added_witnesses: usize,
    added_bytes: usize,
    added_decoded_bytes: usize,
    added_frontiers: Vec<FrontierRequirement>,
    added_pending: Vec<FrontierRequirement>,
) -> Result<ValidatedNodeState, PlanRefusalReason> {
    let res = ctx.resources();
    let own_auth = make_auth(idx, added_auth)?;
    let authority = up
        .authority
        .union(&own_auth)
        .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let privacy = up
        .privacy
        .union(&ctx.privacy)
        .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let obligations = up
        .obligations
        .union(&ctx.cancellation)
        .map_err(|_| translate_obligation_err(idx))?;
    let frontiers = union_frontiers(&up.frontiers, &added_frontiers, res.max_frontiers())
        .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let pending_frontiers = union_frontiers(
        &up.pending_frontiers,
        &added_pending,
        MAX_OPERATOR_FRONTIERS,
    )
    .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let witness_count = up
        .witness_count
        .checked_add(added_witnesses)
        .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let bytes = up
        .bytes
        .checked_add(added_bytes)
        .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    let decoded_bytes = up
        .decoded_bytes
        .checked_add(added_decoded_bytes)
        .ok_or(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })?;
    if witness_count > res.max_witnesses()
        || bytes > res.max_bytes()
        || decoded_bytes > res.max_decoded_bytes()
    {
        return Err(PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx });
    }
    Ok(ValidatedNodeState {
        basis: ctx.requirement().basis,
        uncertainty: ctx.requirement().uncertainty,
        authority,
        privacy,
        frontiers,
        obligations,
        pending_frontiers,
        witness_count,
        bytes,
        decoded_bytes,
    })
}

fn make_auth(
    idx: usize,
    kinds: &[AuthorityKind],
) -> Result<AuthorityRequirements, PlanRefusalReason> {
    AuthorityRequirements::new(kinds.to_vec())
        .map_err(|_| PlanRefusalReason::ResourceEnvelopeExceeded { node_index: idx })
}

fn translate_obligation_err(idx: usize) -> PlanRefusalReason {
    let _ = Error::Binding;
    PlanRefusalReason::LawOrBasisMismatch { node_index: idx }
}
