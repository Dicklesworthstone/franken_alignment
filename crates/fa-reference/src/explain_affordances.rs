//! FA-137 explain trees / `fa why-held` and FA-138 affordances / `fa next` / `fa propose`
//! (plan §6.7, §11.1, §14.1, §14.2, §14.8, §17.3, §17.8, §23; FI-A06, FI-A09, FI-A13, FI-I18).
//!
//! Explanations are deterministic [`PredicateTree`] objects built from closures, reducer traces,
//! and journal events. Human-readable prose is rendered exclusively from a [`PredicateTree`]
//! ([`render_human_report`]) and never the reverse (`FA-INV-016`).
//!
//! Unsatisfied predicate nodes yield typed [`Affordance`]s. `fa next` ranks admissible
//! affordances by bounded value-of-information per unit cost while surfacing hard floors first,
//! and `fa propose` executes an affordance idempotently and returns a [`DecisionCard`].

use std::collections::BTreeMap;

use crate::action::consequence::{Consequence, DecisionInputs, Rule, decide};
use crate::address_resolver::{Address, Noun};
use crate::event_journal::{
    DomainEventJournal, JournalError, JournalEventKind, ReferenceDigestHasher, SourceFrontierEntry,
    rule_tag,
};
use crate::observation_algebra::{Basis, Knowledge, UnknownReason};
use crate::product_frontier::FrontierRequirement;
use crate::typestate_cx::PurposeTag;

pub const MAX_PREDICATE_TREE_DEPTH: usize = 16;
pub const MAX_PREDICATE_TREE_NODES: usize = 256;

/// Epistemic claim classification for an explanation predicate node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplainClaimClass {
    ExactInvariant,
    ScopedProof,
    BoundedModel,
    EmpiricalVerdict,
    PolicyRule,
}

impl ExplainClaimClass {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ExactInvariant => "exact_invariant",
            Self::ScopedProof => "scoped_proof",
            Self::BoundedModel => "bounded_model",
            Self::EmpiricalVerdict => "empirical_verdict",
            Self::PolicyRule => "policy_rule",
        }
    }
}

/// Node kinds in a [`PredicateTree`] (`3luh.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredicateNodeKind {
    Conjunct,
    PolicyPredicate {
        predicate_id: String,
    },
    HelperVerdict {
        helper_id: u64,
        recommendation: Consequence,
    },
    ReducerRule {
        rule: Rule,
    },
    Consequence {
        consequence: Consequence,
    },
    EvidenceRef {
        domain_id: u64,
    },
}

impl PredicateNodeKind {
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Conjunct => "conjunct",
            Self::PolicyPredicate { .. } => "policy_predicate",
            Self::HelperVerdict { .. } => "helper_verdict",
            Self::ReducerRule { .. } => "reducer_rule",
            Self::Consequence { .. } => "consequence",
            Self::EvidenceRef { .. } => "evidence_ref",
        }
    }
}

/// Errors returned by explain-tree construction, rendering lints, and affordance execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExplainError {
    MissingEvidenceAddresses {
        node_id: String,
    },
    TreeDepthExceeded {
        depth: usize,
        max: usize,
    },
    TreeSizeExceeded {
        nodes: usize,
        max: usize,
    },
    IntegrityFailure {
        reason: String,
    },
    UnrenderedProseRejected {
        reason: String,
    },
    FloorSkippingForbidden {
        affordance_id: String,
    },
    AuthorityEscalationRefused {
        required: PurposeTag,
        actual: PurposeTag,
    },
    RefinementStormBlocked {
        reason: String,
    },
    PredecessorSeqMismatch {
        expected: u64,
        actual: u64,
    },
    JournalFailure(JournalError),
}

/// Single node in a [`PredicateTree`]. Every node is wrapped in `Knowledge<bool>` and
/// carries at least one canonical `fa://` evidence address (`3luh.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateNode {
    pub node_id: String,
    pub kind: PredicateNodeKind,
    pub claim_class: ExplainClaimClass,
    pub status: Knowledge<bool>,
    pub evidence_addresses: Vec<Address>,
    pub children: Vec<PredicateNode>,
}

impl PredicateNode {
    pub fn new(
        node_id: &str,
        kind: PredicateNodeKind,
        claim_class: ExplainClaimClass,
        status: Knowledge<bool>,
        evidence_addresses: Vec<Address>,
        children: Vec<PredicateNode>,
    ) -> Result<Self, ExplainError> {
        if evidence_addresses.is_empty() {
            return Err(ExplainError::MissingEvidenceAddresses {
                node_id: node_id.to_owned(),
            });
        }
        Ok(Self {
            node_id: node_id.to_owned(),
            kind,
            claim_class,
            status,
            evidence_addresses,
            children,
        })
    }

    /// True if and only if the node's status is `Knowledge::Known { value: true, .. }`.
    pub fn is_satisfied(&self) -> bool {
        matches!(self.status, Knowledge::Known { value: true, .. })
    }
}

/// Validated, bounded predicate tree explaining a closure, receipt, or round (`3luh.1`, `3luh.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateTree {
    pub subject_address: Address,
    pub root: PredicateNode,
    pub node_count: usize,
    pub max_depth: usize,
    pub replay_digest: [u8; 32],
}

impl PredicateTree {
    pub fn new(subject_address: Address, root: PredicateNode) -> Result<Self, ExplainError> {
        let mut node_count = 0usize;
        let mut max_depth = 0usize;
        validate_node_bounds(&root, 1, &mut node_count, &mut max_depth)?;
        let replay_digest = compute_tree_digest(&subject_address, &root);
        Ok(Self {
            subject_address,
            root,
            node_count,
            max_depth,
            replay_digest,
        })
    }
}

fn validate_node_bounds(
    node: &PredicateNode,
    depth: usize,
    node_count: &mut usize,
    max_depth: &mut usize,
) -> Result<(), ExplainError> {
    if depth > MAX_PREDICATE_TREE_DEPTH {
        return Err(ExplainError::TreeDepthExceeded {
            depth,
            max: MAX_PREDICATE_TREE_DEPTH,
        });
    }
    if node.evidence_addresses.is_empty() {
        return Err(ExplainError::MissingEvidenceAddresses {
            node_id: node.node_id.clone(),
        });
    }
    *node_count += 1;
    if *node_count > MAX_PREDICATE_TREE_NODES {
        return Err(ExplainError::TreeSizeExceeded {
            nodes: *node_count,
            max: MAX_PREDICATE_TREE_NODES,
        });
    }
    if depth > *max_depth {
        *max_depth = depth;
    }
    for child in &node.children {
        validate_node_bounds(child, depth + 1, node_count, max_depth)?;
    }
    Ok(())
}

fn compute_tree_digest(subject: &Address, root: &PredicateNode) -> [u8; 32] {
    let mut hasher = ReferenceDigestHasher::new();
    hasher.update(b"fa-predicate-tree-v1:");
    hasher.update(subject.canonical_string().as_bytes());
    hash_node(&mut hasher, root);
    hasher.finalize()
}

fn hash_node(hasher: &mut ReferenceDigestHasher, node: &PredicateNode) {
    hasher.update(node.node_id.as_bytes());
    hasher.update(b"|");
    hasher.update(node.kind.kind_tag().as_bytes());
    hasher.update(b"|");
    hasher.update(node.claim_class.tag().as_bytes());
    hasher.update(b"|");
    hasher.update(status_tag(&node.status).as_bytes());
    for addr in &node.evidence_addresses {
        hasher.update(b"@");
        hasher.update(addr.canonical_string().as_bytes());
    }
    hasher.update(b"(");
    for child in &node.children {
        hash_node(hasher, child);
    }
    hasher.update(b")");
}

fn status_tag(status: &Knowledge<bool>) -> String {
    match status {
        Knowledge::Known { value, .. } => format!("known:{value}"),
        Knowledge::Pending { .. } => "pending".to_owned(),
        Knowledge::Unknown { reason } => format!("unknown:{reason:?}"),
        Knowledge::Withheld { authority_required } => format!("withheld:{authority_required:?}"),
        Knowledge::Stale {
            basis,
            current_generation,
        } => format!(
            "stale:{}<{current_generation}",
            basis.semantic_generation()
        ),
        Knowledge::Absent { domain } => format!("absent:{}", domain.domain().domain_id()),
    }
}

/// Construct a deterministic [`PredicateTree`] from a closure's [`DecisionInputs`] and reducer
/// evaluation (`3luh.2`).
pub fn build_explain_tree_for_closure(
    closure_address: Address,
    helper_id: u64,
    inputs: DecisionInputs,
    evidence_addresses: Vec<Address>,
    basis: Basis,
    pending_frontier: FrontierRequirement,
) -> Result<PredicateTree, ExplainError> {
    let decision = decide(inputs);
    let empirical_consequence: Consequence = inputs.empirical.into();
    let empirical_ok = empirical_consequence == Consequence::Continue;

    let mut children = Vec::new();
    children.push(PredicateNode::new(
        "helper-verdict",
        PredicateNodeKind::HelperVerdict {
            helper_id,
            recommendation: empirical_consequence,
        },
        ExplainClaimClass::EmpiricalVerdict,
        Knowledge::Known {
            value: empirical_ok,
            basis,
        },
        evidence_addresses.clone(),
        Vec::new(),
    )?);

    let disqualifier_clear = !inputs.exact_disqualifier;
    children.push(PredicateNode::new(
        "policy-no-exact-disqualifier",
        PredicateNodeKind::PolicyPredicate {
            predicate_id: "no_exact_disqualifier".to_owned(),
        },
        ExplainClaimClass::ExactInvariant,
        Knowledge::Known {
            value: disqualifier_clear,
            basis,
        },
        evidence_addresses.clone(),
        Vec::new(),
    )?);

    let mandatory_status = if inputs.mandatory_absent {
        Knowledge::Pending {
            frontiers: vec![pending_frontier],
            expected_cost: 2,
        }
    } else {
        Knowledge::Known { value: true, basis }
    };
    children.push(PredicateNode::new(
        "evidence-mandatory-present",
        PredicateNodeKind::EvidenceRef {
            domain_id: pending_frontier.key.source,
        },
        ExplainClaimClass::ExactInvariant,
        mandatory_status,
        evidence_addresses.clone(),
        Vec::new(),
    )?);

    let contradiction_status = if inputs.contradiction {
        Knowledge::Unknown {
            reason: UnknownReason::ConflictingEvidence,
        }
    } else {
        Knowledge::Known { value: true, basis }
    };
    children.push(PredicateNode::new(
        "reducer-consistency",
        PredicateNodeKind::ReducerRule {
            rule: if inputs.contradiction {
                Rule::Contradiction
            } else {
                Rule::EmpiricalRecommendation
            },
        },
        ExplainClaimClass::ScopedProof,
        contradiction_status,
        evidence_addresses.clone(),
        Vec::new(),
    )?);

    let root_satisfied = decision.consequence == Consequence::Continue;
    let root = PredicateNode::new(
        "closure-consequence-root",
        PredicateNodeKind::Consequence {
            consequence: decision.consequence,
        },
        ExplainClaimClass::ScopedProof,
        Knowledge::Known {
            value: root_satisfied,
            basis,
        },
        evidence_addresses,
        children,
    )?;

    PredicateTree::new(closure_address, root)
}

/// Verify that a replayed [`PredicateTree`] is byte-identical in digest and structure to a
/// stored tree (`3luh.2`, `3luh.5`).
pub fn verify_explain_tree_replay(
    stored: &PredicateTree,
    replayed: &PredicateTree,
) -> Result<(), ExplainError> {
    if stored.replay_digest != replayed.replay_digest || stored != replayed {
        return Err(ExplainError::IntegrityFailure {
            reason: "replayed PredicateTree diverged from stored tree".to_owned(),
        });
    }
    Ok(())
}

/// Summary of an unsatisfied node returned by the pure `why-held` filter (`3luh.5`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateNodeSummary {
    pub node_id: String,
    pub kind: PredicateNodeKind,
    pub claim_class: ExplainClaimClass,
    pub status: Knowledge<bool>,
    pub evidence_addresses: Vec<Address>,
}

/// Pure filter returning all unsatisfied `Knowledge<bool>`-wrapped nodes in deterministic
/// pre-order traversal, without importing or attaching affordance types (`3luh.5`).
pub fn filter_unsatisfied_nodes(tree: &PredicateTree) -> Vec<PredicateNodeSummary> {
    let mut out = Vec::new();
    collect_unsatisfied(&tree.root, &mut out);
    out
}

fn collect_unsatisfied(node: &PredicateNode, out: &mut Vec<PredicateNodeSummary>) {
    if !node.is_satisfied() {
        out.push(PredicateNodeSummary {
            node_id: node.node_id.clone(),
            kind: node.kind.clone(),
            claim_class: node.claim_class,
            status: node.status.clone(),
            evidence_addresses: node.evidence_addresses.clone(),
        });
    }
    for child in &node.children {
        collect_unsatisfied(child, out);
    }
}

/// IO-free renderer producing human-legible explanation strictly from `PredicateTree` (`3luh.4`).
pub fn render_human_report(tree: &PredicateTree) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "EXPLAIN_REPORT subject={} nodes={} max_depth={}",
        tree.subject_address.canonical_string(),
        tree.node_count,
        tree.max_depth
    ));
    render_node_lines(&tree.root, 0, &mut lines);
    lines.join("\n")
}

fn render_node_lines(node: &PredicateNode, indent: usize, lines: &mut Vec<String>) {
    let pad = "  ".repeat(indent);
    let addrs: Vec<String> = node
        .evidence_addresses
        .iter()
        .map(Address::canonical_string)
        .collect();
    lines.push(format!(
        "{pad}- node={} kind={} claim={} status={} evidence=[{}]",
        node.node_id,
        node.kind.kind_tag(),
        node.claim_class.tag(),
        status_tag(&node.status),
        addrs.join(",")
    ));
    for child in &node.children {
        render_node_lines(child, indent + 1, lines);
    }
}

/// Lint verifying that human report prose was generated strictly from `render_human_report(tree)`
/// and references every node's canonical address (`3luh.4`, `3luh.5`).
pub fn lint_explanation_prose_matches_tree(
    prose: &str,
    tree: &PredicateTree,
) -> Result<(), ExplainError> {
    let expected = render_human_report(tree);
    if prose != expected {
        return Err(ExplainError::UnrenderedProseRejected {
            reason: "explanation prose was not produced by render_human_report(tree)".to_owned(),
        });
    }
    Ok(())
}

/// Typed next-action affordance generated from an unsatisfied predicate node (`o7j5.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Affordance {
    pub affordance_id: String,
    pub verb: &'static str,
    pub target: Address,
    pub authority_required: PurposeTag,
    pub cost_estimate: u64,
    pub expected_information_gain: u64,
    pub would_change: Vec<Address>,
    pub unsatisfied_node_id: String,
    pub is_hard_floor: bool,
    pub rehearsable: bool,
}

/// Generate registered affordances for every unsatisfied node in `tree` (`o7j5.1`).
///
/// Exact disqualifiers (`PolicyPredicate { predicate_id: "no_exact_disqualifier" }`) never
/// generate floor-skipping permit affordances; they only offer `narrow` or `hold` containment
/// actions. Missing mandatory evidence generates a `get` / `resolve` observation affordance
/// marked `is_hard_floor: true`.
pub fn generate_affordances_for_tree(tree: &PredicateTree) -> Vec<Affordance> {
    let unsatisfied = filter_unsatisfied_nodes(tree);
    let mut affordances = Vec::new();

    for node in unsatisfied {
        let target = node
            .evidence_addresses
            .first()
            .cloned()
            .unwrap_or_else(|| tree.subject_address.clone());
        match &node.kind {
            PredicateNodeKind::EvidenceRef { .. } => {
                affordances.push(Affordance {
                    affordance_id: format!("aff-resolve-{}", node.node_id),
                    verb: "resolve",
                    target,
                    authority_required: PurposeTag::Capture,
                    cost_estimate: 2,
                    expected_information_gain: 20,
                    would_change: vec![tree.subject_address.clone()],
                    unsatisfied_node_id: node.node_id,
                    is_hard_floor: true,
                    rehearsable: true,
                });
            }
            PredicateNodeKind::ReducerRule {
                rule: Rule::Contradiction,
            } => {
                affordances.push(Affordance {
                    affordance_id: format!("aff-convene-{}", node.node_id),
                    verb: "convene",
                    target,
                    authority_required: PurposeTag::Judge,
                    cost_estimate: 5,
                    expected_information_gain: 30,
                    would_change: vec![tree.subject_address.clone()],
                    unsatisfied_node_id: node.node_id,
                    is_hard_floor: false,
                    rehearsable: true,
                });
            }
            PredicateNodeKind::PolicyPredicate { .. } => {
                // Disqualifier predicates are hard floors: never offer a bypass/skip action;
                // only offer containment narrowing.
                affordances.push(Affordance {
                    affordance_id: format!("aff-narrow-{}", node.node_id),
                    verb: "narrow",
                    target,
                    authority_required: PurposeTag::Permit,
                    cost_estimate: 1,
                    expected_information_gain: 10,
                    would_change: vec![tree.subject_address.clone()],
                    unsatisfied_node_id: node.node_id,
                    is_hard_floor: true,
                    rehearsable: true,
                });
            }
            PredicateNodeKind::HelperVerdict { .. } => {
                affordances.push(Affordance {
                    affordance_id: format!("aff-rehearse-{}", node.node_id),
                    verb: "rehearse",
                    target,
                    authority_required: PurposeTag::Experiment,
                    cost_estimate: 3,
                    expected_information_gain: 15,
                    would_change: vec![tree.subject_address.clone()],
                    unsatisfied_node_id: node.node_id,
                    is_hard_floor: false,
                    rehearsable: true,
                });
            }
            PredicateNodeKind::Consequence { .. }
            | PredicateNodeKind::Conjunct
            | PredicateNodeKind::ReducerRule { .. } => {}
        }
    }

    affordances
}

/// Single entry in a `WhyHeldReport` pairing an unsatisfied node with its affordances (`3luh.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhyHeldItem {
    pub node: PredicateNodeSummary,
    pub affordances: Vec<Affordance>,
}

/// Output of `fa why-held` (`3luh.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhyHeldReport {
    pub subject_address: Address,
    pub exact_at_control_seq: u64,
    pub items: Vec<WhyHeldItem>,
}

/// Pair each unsatisfied node in `tree` with its generated affordances (`3luh.3`).
pub fn fa_why_held(tree: &PredicateTree, exact_at_control_seq: u64) -> WhyHeldReport {
    let nodes = filter_unsatisfied_nodes(tree);
    let all_affordances = generate_affordances_for_tree(tree);
    let items = nodes
        .into_iter()
        .map(|node| {
            let affordances = all_affordances
                .iter()
                .filter(|a| a.unsatisfied_node_id == node.node_id)
                .cloned()
                .collect();
            WhyHeldItem { node, affordances }
        })
        .collect();
    WhyHeldReport {
        subject_address: tree.subject_address.clone(),
        exact_at_control_seq,
        items,
    }
}

/// Rank admissible affordances by bounded VOI per unit cost within the caller's allowed
/// purposes and cost budget (`o7j5.2`).
///
/// Hard floors (`is_hard_floor == true`) always surface before optional refinements regardless
/// of score, and tie-breaking is strictly deterministic by `(cost_estimate, affordance_id)`.
pub fn fa_next(
    affordances: &[Affordance],
    allowed_purposes: &[PurposeTag],
    max_cost_budget: u64,
) -> Vec<Affordance> {
    let mut filtered: Vec<Affordance> = affordances
        .iter()
        .filter(|a| {
            allowed_purposes.contains(&a.authority_required)
                && a.cost_estimate <= max_cost_budget
                && a.verb != "skip-floor"
        })
        .cloned()
        .collect();

    filtered.sort_by(|a, b| {
        // Hard floors always surface first.
        b.is_hard_floor
            .cmp(&a.is_hard_floor)
            .then_with(|| {
                let a_cost = a.cost_estimate.max(1) as u128;
                let b_cost = b.cost_estimate.max(1) as u128;
                let lhs = (a.expected_information_gain as u128) * b_cost;
                let rhs = (b.expected_information_gain as u128) * a_cost;
                rhs.cmp(&lhs)
            })
            .then_with(|| a.cost_estimate.cmp(&b.cost_estimate))
            .then_with(|| a.affordance_id.cmp(&b.affordance_id))
    });

    filtered
}

/// Decision card returned by `fa propose` (`o7j5.3`, plan §14.8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionCard {
    pub chosen: Affordance,
    pub alternatives_not_chosen: Vec<String>,
    pub predicted_cost: u64,
    pub realized_cost: u64,
    pub witness_addresses: Vec<Address>,
    pub fallback_verb: &'static str,
    pub new_control_seq: u64,
}

/// Stateful proposal executor enforcing idempotency, authority checks, floor-skipping refusal,
/// and admission limits against refinement storms (`o7j5.3`, `o7j5.4`).
#[derive(Debug)]
pub struct ProposalEngine {
    remaining_budget: u64,
    max_requests_per_window: usize,
    requests_in_window: usize,
    idempotency_cache: BTreeMap<(u64, String), DecisionCard>,
}

impl ProposalEngine {
    pub fn new(remaining_budget: u64, max_requests_per_window: usize) -> Self {
        Self {
            remaining_budget,
            max_requests_per_window,
            requests_in_window: 0,
            idempotency_cache: BTreeMap::new(),
        }
    }

    pub const fn remaining_budget(&self) -> u64 {
        self.remaining_budget
    }

    pub fn fa_propose(
        &mut self,
        caller_purpose: PurposeTag,
        expected_predecessor_seq: u64,
        idempotency_key: &str,
        chosen: &Affordance,
        alternatives: &[Affordance],
        journal: &mut DomainEventJournal,
    ) -> Result<DecisionCard, ExplainError> {
        let key = (expected_predecessor_seq, idempotency_key.to_owned());
        if let Some(cached) = self.idempotency_cache.get(&key) {
            return Ok(cached.clone());
        }

        if chosen.verb == "skip-floor" {
            return Err(ExplainError::FloorSkippingForbidden {
                affordance_id: chosen.affordance_id.clone(),
            });
        }

        if caller_purpose != chosen.authority_required {
            return Err(ExplainError::AuthorityEscalationRefused {
                required: chosen.authority_required,
                actual: caller_purpose,
            });
        }

        if expected_predecessor_seq != journal.last_control_seq() {
            return Err(ExplainError::PredecessorSeqMismatch {
                expected: expected_predecessor_seq,
                actual: journal.last_control_seq(),
            });
        }

        if self.requests_in_window >= self.max_requests_per_window {
            return Err(ExplainError::RefinementStormBlocked {
                reason: format!(
                    "window admission limit {} reached",
                    self.max_requests_per_window
                ),
            });
        }

        if chosen.cost_estimate > self.remaining_budget {
            return Err(ExplainError::RefinementStormBlocked {
                reason: format!(
                    "cost {} exceeds remaining budget {}",
                    chosen.cost_estimate, self.remaining_budget
                ),
            });
        }

        self.remaining_budget -= chosen.cost_estimate;
        self.requests_in_window += 1;

        let event = journal
            .emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: expected_predecessor_seq + 1,
                }],
                vec![chosen.target.clone()],
                JournalEventKind::Proposal {
                    attempt_id: expected_predecessor_seq + 1,
                },
                [0x44; 32],
                chosen.cost_estimate,
                1_000,
            )
            .map_err(ExplainError::JournalFailure)?;

        let alternatives_not_chosen = alternatives
            .iter()
            .filter(|a| a.affordance_id != chosen.affordance_id)
            .map(|a| a.affordance_id.clone())
            .collect();

        let card = DecisionCard {
            chosen: chosen.clone(),
            alternatives_not_chosen,
            predicted_cost: chosen.cost_estimate,
            realized_cost: chosen.cost_estimate,
            witness_addresses: vec![chosen.target.clone()],
            fallback_verb: "hold",
            new_control_seq: event.control_seq,
        };
        self.idempotency_cache.insert(key, card.clone());
        Ok(card)
    }

    /// Reference helper referencing `rule_tag` so explain trace and journal share rule naming.
    pub const fn canonical_rule_name(rule: Rule) -> &'static str {
        rule_tag(rule)
    }

    /// Reference helper for constructing closure subject addresses in tests.
    pub fn closure_address(tenant: &str, id: &str) -> Address {
        Address::new(tenant, Noun::Closure, id, Some(1)).expect("valid closure address")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::Restriction;
    use crate::observation_algebra::{BasisIdentity, ClaimClass, ProofStates, ScopedProof};
    use crate::product_frontier::{FrontierStage, ProjectionKey};

    fn sample_frontier() -> FrontierRequirement {
        FrontierRequirement {
            key: ProjectionKey {
                source: 1,
                branch: 0,
                projection: 1,
                source_epoch: 1,
            },
            stage: FrontierStage::Captured,
            through: 10,
            closure: None,
        }
    }

    fn sample_basis() -> Basis {
        let proof = ScopedProof::Declared {
            scope_id: 1,
            generation: 1,
        };
        Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 1,
                profile_id: 1,
                semantic_generation: 1,
                policy_epoch: 1,
                model_epoch: 1,
                tokenizer_epoch: 1,
                codec_epoch: 1,
                control_seq: 1,
            },
            ClaimClass::Invariant,
            ProofStates {
                authentic_origin: proof,
                complete_for_contract: proof,
                semantically_valid: proof,
                available_for_replay: proof,
            },
        )
    }

    #[test]
    fn explain_tree_determinism_replay_tamper_and_renderer_only_lint() {
        let subject = ProposalEngine::closure_address("org", "clos-10");
        let ev_addr = Address::new("org", Noun::Evidence, "ev-10", Some(1)).expect("addr");
        let inputs = DecisionInputs {
            empirical: Restriction::Continue,
            exact_disqualifier: false,
            mandatory_absent: true,
            contradiction: true,
        };

        let tree1 = build_explain_tree_for_closure(
            subject.clone(),
            7,
            inputs,
            vec![ev_addr.clone()],
            sample_basis(),
            sample_frontier(),
        )
        .expect("tree1");
        let tree2 = build_explain_tree_for_closure(
            subject.clone(),
            7,
            inputs,
            vec![ev_addr.clone()],
            sample_basis(),
            sample_frontier(),
        )
        .expect("tree2");

        verify_explain_tree_replay(&tree1, &tree2).expect("deterministic replay equal");

        // Tampering with a node status causes verify_explain_tree_replay to return IntegrityFailure.
        let mut tampered = tree2.clone();
        tampered.root.children[0].status = Knowledge::Known {
            value: false,
            basis: sample_basis(),
        };
        tampered.replay_digest = compute_tree_digest(&tampered.subject_address, &tampered.root);
        assert!(matches!(
            verify_explain_tree_replay(&tree1, &tampered),
            Err(ExplainError::IntegrityFailure { .. })
        ));

        // Node without evidence addresses is refused.
        let bad_node = PredicateNode::new(
            "bad-node",
            PredicateNodeKind::Conjunct,
            ExplainClaimClass::ExactInvariant,
            Knowledge::Known {
                value: true,
                basis: sample_basis(),
            },
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            bad_node,
            Err(ExplainError::MissingEvidenceAddresses {
                node_id: "bad-node".to_owned()
            })
        );

        // Pure why-held filter returns exactly the unsatisfied nodes (root + mandatory + contradiction).
        let unsatisfied = filter_unsatisfied_nodes(&tree1);
        assert_eq!(unsatisfied.len(), 3);

        // Human report renderer consumes the tree only and includes node addresses.
        let rendered = render_human_report(&tree1);
        assert!(rendered.contains("fa://org/closure/clos-10@1"));
        assert!(rendered.contains("fa://org/evidence/ev-10@1"));
        lint_explanation_prose_matches_tree(&rendered, &tree1).expect("renderer prose passes lint");
        assert!(matches!(
            lint_explanation_prose_matches_tree("ad-hoc freeform prose", &tree1),
            Err(ExplainError::UnrenderedProseRejected { .. })
        ));
    }

    #[test]
    fn why_held_and_fa_next_surface_hard_floors_first_without_skipping() {
        let subject = ProposalEngine::closure_address("org", "clos-20");
        let ev_addr = Address::new("org", Noun::Evidence, "ev-20", Some(1)).expect("addr");
        let inputs = DecisionInputs {
            empirical: Restriction::HoldEffect,
            exact_disqualifier: false,
            mandatory_absent: true,
            contradiction: true,
        };
        let tree = build_explain_tree_for_closure(
            subject,
            9,
            inputs,
            vec![ev_addr],
            sample_basis(),
            sample_frontier(),
        )
        .expect("tree");

        let report = fa_why_held(&tree, 42);
        assert_eq!(report.exact_at_control_seq, 42);
        assert!(!report.items.is_empty());

        let affordances = generate_affordances_for_tree(&tree);
        assert!(affordances.iter().all(|a| a.verb != "skip-floor"));

        let ranked = fa_next(
            &affordances,
            &[
                PurposeTag::Capture,
                PurposeTag::Judge,
                PurposeTag::Experiment,
            ],
            100,
        );
        assert_eq!(ranked.len(), 3);
        // The hard floor ("resolve" mandatory absent evidence) MUST rank first!
        assert!(ranked[0].is_hard_floor);
        assert_eq!(ranked[0].verb, "resolve");
    }

    #[test]
    fn fa_propose_idempotency_and_negative_campaign_floor_skip_storm_and_escalation() {
        let target = Address::new("org", Noun::Evidence, "ev-30", Some(1)).expect("addr");
        let valid_aff = Affordance {
            affordance_id: "aff-resolve-1".to_owned(),
            verb: "resolve",
            target: target.clone(),
            authority_required: PurposeTag::Capture,
            cost_estimate: 4,
            expected_information_gain: 20,
            would_change: vec![target.clone()],
            unsatisfied_node_id: "evidence-mandatory-present".to_owned(),
            is_hard_floor: true,
            rehearsable: true,
        };
        let alt_aff = Affordance {
            affordance_id: "aff-convene-2".to_owned(),
            verb: "convene",
            target: target.clone(),
            authority_required: PurposeTag::Judge,
            cost_estimate: 5,
            expected_information_gain: 25,
            would_change: vec![target.clone()],
            unsatisfied_node_id: "reducer-consistency".to_owned(),
            is_hard_floor: false,
            rehearsable: true,
        };

        let mut journal = DomainEventJournal::new_production("org", "dom-propose");
        let mut engine = ProposalEngine::new(6, 2);

        // 1. Authority escalation attempt (calling Capture affordance with Experiment purpose) fails closed.
        assert_eq!(
            engine.fa_propose(
                PurposeTag::Experiment,
                0,
                "idem-1",
                &valid_aff,
                std::slice::from_ref(&alt_aff),
                &mut journal,
            ),
            Err(ExplainError::AuthorityEscalationRefused {
                required: PurposeTag::Capture,
                actual: PurposeTag::Experiment,
            })
        );

        // 2. Floor-skipping affordance fails closed.
        let mut skip_aff = valid_aff.clone();
        skip_aff.verb = "skip-floor";
        assert_eq!(
            engine.fa_propose(
                PurposeTag::Capture,
                0,
                "idem-skip",
                &skip_aff,
                &[],
                &mut journal,
            ),
            Err(ExplainError::FloorSkippingForbidden {
                affordance_id: "aff-resolve-1".to_owned(),
            })
        );

        // 3. Valid execution succeeds and returns DecisionCard.
        let card1 = engine
            .fa_propose(
                PurposeTag::Capture,
                0,
                "idem-1",
                &valid_aff,
                std::slice::from_ref(&alt_aff),
                &mut journal,
            )
            .expect("propose succeeds");
        assert_eq!(card1.new_control_seq, 1);
        assert_eq!(card1.alternatives_not_chosen, vec!["aff-convene-2"]);
        assert_eq!(engine.remaining_budget(), 2);

        // 4. Idempotent replay with same (expected_predecessor_seq, idempotency_key) returns identical card without spending budget.
        let card1_replay = engine
            .fa_propose(
                PurposeTag::Capture,
                0,
                "idem-1",
                &valid_aff,
                std::slice::from_ref(&alt_aff),
                &mut journal,
            )
            .expect("idempotent replay succeeds");
        assert_eq!(card1, card1_replay);
        assert_eq!(engine.remaining_budget(), 2);
        assert_eq!(journal.len(), 1);

        // 5. Refinement storm / budget exhaustion fails closed.
        assert!(matches!(
            engine.fa_propose(
                PurposeTag::Judge,
                1,
                "idem-2",
                &alt_aff,
                &[],
                &mut journal,
            ),
            Err(ExplainError::RefinementStormBlocked { .. })
        ));
    }
}
