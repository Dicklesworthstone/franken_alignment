//! Bounded composition of reference reads and branch-only policy experiments.
//!
//! This facade consumes host-provisioned evidence and caller grants. Reads do
//! not change the snapshot; paired replay requires a retained ExperimentCx and
//! records its outcome only in the owned branch journal. It does not authenticate
//! declarations, execute live effects, or qualify a deployment. Native mutation
//! owners remain separate; unconnected verbs explicitly refuse.

use std::collections::{BTreeMap, BTreeSet};

use crate::address_resolver::{
    Address, AddressResolver, CallerScope, Noun, RelatedEdge, RelationKind, ResolvedObject,
    ResolverPlane,
};
use crate::event_journal::{DomainEventJournal, JournalScope, SourceFrontierEntry, TailBatch};
use crate::explain_affordances::{
    Affordance, PredicateNode, PredicateTree, WhyHeldReport, fa_next, fa_why_held,
    generate_affordances_for_tree,
};
use crate::observation_algebra::{AuthorityKind, Basis, Knowledge, ScopedProof, UnknownReason};
use crate::situation_rehearsal::{SituationReport, fold_situation_report};
use crate::typestate_cx::{ExperimentCx, PurposeTag};

mod dispatch_replay;
mod intake;
#[cfg(unix)]
pub mod publication;
pub mod replay;
mod request;
#[cfg(unix)]
pub mod router;
pub mod vocabulary;
mod wire;
use dispatch_replay::{CachedReplay, ReplayRegistration};
pub use dispatch_replay::{MAX_REPLAY_OPERATIONS, MAX_REPLAY_RECORDS, MAX_REPLAY_RETAINED_BYTES};
pub use replay::{ObservedReplayRecord, ObservedReplayReport, ReplayPlanMetadata};
pub use request::{
    MAX_REQUEST_BYTES, MAX_REQUEST_DEPTH, MAX_REQUEST_ITEMS, MAX_REQUEST_STRING_BYTES,
    REQUEST_SCHEMA, TowerDecodingError, decode_request, encode_request,
};
pub use vocabulary::{ENVELOPE_FIELDS, ReasonCode, UnknownVerb, Verb, VerbClass};
pub use wire::TowerEncodingError;

pub const RESPONSE_SCHEMA: &str = "fa.tower_response/1";
pub const CONNECTED_VERBS: [Verb; 9] = [
    Verb::Get,
    Verb::Related,
    Verb::Tail,
    Verb::Situation,
    Verb::Explain,
    Verb::WhyHeld,
    Verb::Next,
    Verb::Replay,
    Verb::ReplayPlan,
];

/// Explicit bounds on the immutable imported snapshot and every query.
/// One admitted read or uncached experiment spends one logical operation unit,
/// including an unknown result. Exact cached experiments do not spend again.
/// These units are not measurements of inference, elapsed time, or byte movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TowerLimits {
    pub max_objects: usize,
    pub max_edges: usize,
    pub max_journal_events: usize,
    pub max_explanations: usize,
    pub max_page_events: usize,
    pub max_response_bytes: usize,
    pub max_read_units: u64,
}

impl Default for TowerLimits {
    fn default() -> Self {
        Self {
            max_objects: 256,
            max_edges: 512,
            max_journal_events: 512,
            max_explanations: 64,
            max_page_events: 64,
            max_response_bytes: 256 * 1024,
            max_read_units: 256,
        }
    }
}

impl TowerLimits {
    fn valid(self) -> bool {
        (1..=1024).contains(&self.max_objects)
            && (1..=4096).contains(&self.max_edges)
            && (1..=4096).contains(&self.max_journal_events)
            && (1..=256).contains(&self.max_explanations)
            && (1..=128).contains(&self.max_page_events)
            && (16 * 1024..=1024 * 1024).contains(&self.max_response_bytes)
            && (1..=1024).contains(&self.max_read_units)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TowerInstallError {
    InvalidLimits,
    InvalidAddress,
    SnapshotMismatch,
    DuplicateObject,
    MissingSource,
    InvalidExplanation,
    ReplayContextRequired,
    InvalidReplaySource,
    LimitExceeded,
}

/// Host-supplied reference grants; never deserialize these from an actor request.
#[derive(Clone, Debug)]
pub struct TowerAccess {
    scope: CallerScope,
    operator: bool,
    allowed_purposes: Vec<PurposeTag>,
    affordance_budget: u64,
}

impl TowerAccess {
    pub fn new(
        scope: CallerScope,
        operator: bool,
        allowed_purposes: Vec<PurposeTag>,
        affordance_budget: u64,
    ) -> Self {
        Self {
            scope,
            operator,
            allowed_purposes,
            affordance_budget,
        }
    }

    fn full_domain(&self, tenant: &str) -> bool {
        self.scope.caller_tenant == tenant
            && [
                ResolverPlane::ControlLedger,
                ResolverPlane::EvidenceStore,
                ResolverPlane::DerivedProjections,
                ResolverPlane::ExperimentStore,
            ]
            .iter()
            .all(|plane| self.scope.allowed_planes.contains(plane))
            && [
                AuthorityKind::Verify,
                AuthorityKind::ExactRead,
                AuthorityKind::Judge,
                AuthorityKind::Probe,
            ]
            .iter()
            .all(|authority| self.scope.granted_authorities.contains(authority))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadArguments {
    None,
    Related {
        relation: Option<RelationKind>,
    },
    Tail {
        since_seq: u64,
        max_events: usize,
    },
    Next {
        max_cost: u64,
    },
    Replay {
        interventions: Vec<crate::action::consequence::experiment::Intervention>,
    },
    Propose {
        request: u64,
        proposal: crate::action::consequence::oversight::actor::ActorProposal,
    },
    Precheck {
        query: u64,
        proposal: crate::action::consequence::oversight::actor::ActorProposal,
    },
    Capabilities {
        query: u64,
        expected_policy_epoch: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TowerRequest {
    pub operation_id: String,
    pub address: Address,
    pub verb: Verb,
    pub arguments: ReadArguments,
    pub idempotency_key: Option<String>,
}

impl TowerRequest {
    pub fn new(operation_id: impl Into<String>, address: Address, verb: Verb) -> Self {
        Self {
            operation_id: operation_id.into(),
            address,
            verb,
            arguments: ReadArguments::None,
            idempotency_key: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TowerValue {
    Object(ResolvedObject),
    Related(Vec<RelatedEdge>),
    Tail(TailBatch),
    Situation(SituationReport),
    Explain(PredicateTree),
    WhyHeld(WhyHeldReport),
    Next(Vec<Affordance>),
    Replay(TowerRehearsalOutcome),
    ReplayPlan(TowerReplayPlan),
}

/// Inspected registration metadata, not a completed or authenticated replay.
/// Sensitive native fields remain withheld unless the caller has full scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TowerReplayPlan {
    pub branch_id: u64,
    pub branch_policy_epoch: u64,
    pub source_address: Address,
    pub source_basis: Basis,
    pub metadata: Knowledge<ReplayPlanMetadata>,
}

/// An actual paired policy experiment, never a receipt verifier or live approval.
/// Source and branch histories have distinct bases and control-sequence domains.
///
/// ```compile_fail,E0308
/// use fa_reference::{action::Permit, tower::TowerRehearsalOutcome};
/// fn cannot_authorize(outcome: TowerRehearsalOutcome) -> Permit { outcome }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TowerRehearsalOutcome {
    pub rehearsal: bool,
    pub branch_id: u64,
    pub source_address: Address,
    pub source_basis: Basis,
    pub source: ObservedReplayReport,
    _seal: RehearsalSeal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RehearsalSeal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseStatus {
    Ok,
    Error,
}

/// Exactly the sixteen common fields from registry/vocabulary.json.
/// Observed values and costs retain their Knowledge wrappers. Request identity,
/// syntax, static resource limits and retry classification are L0 metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TowerEnvelope<L = TowerLimits> {
    pub schema_version: &'static str,
    pub operation_id: String,
    pub status: ResponseStatus,
    pub reason_code: Option<ReasonCode>,
    pub address: Address,
    pub generation: Knowledge<u64>,
    pub control_seq: Knowledge<u64>,
    pub source_frontiers: Knowledge<Vec<SourceFrontierEntry>>,
    pub missing: Vec<Address>,
    pub limits: L,
    pub budgets_remaining: Knowledge<u64>,
    pub realized_cost: Knowledge<u64>,
    pub affordances: Knowledge<Vec<Affordance>>,
    pub retry_safe: bool,
    pub idempotency_key: Option<String>,
    pub would_change_this: Vec<Address>,
}

/// Wire form: {"envelope": {the sixteen fields}, "result": Knowledge<TowerValue>}.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TowerResponse<R = Knowledge<TowerValue>, L = TowerLimits> {
    pub envelope: TowerEnvelope<L>,
    pub result: R,
}

#[derive(Clone, Debug)]
struct Explanation {
    tree: PredicateTree,
    basis: Basis,
}

/// Provision bounded reference evidence before serving requests. Imports remain
/// fixed; an enabled paired replay may append only to the owned branch journal.
pub struct TowerBuilder {
    resolver: AddressResolver,
    journal: DomainEventJournal,
    domain_address: Address,
    snapshot_basis: Basis,
    frontiers: Vec<SourceFrontierEntry>,
    limits: TowerLimits,
    remaining_read_units: u64,
    objects: BTreeMap<Address, (u64, Basis)>,
    edge_count: usize,
    explanations: BTreeMap<Address, Explanation>,
    replay_context: Option<ExperimentCx>,
    replays: BTreeMap<Address, ReplayRegistration>,
    replay_retained_bytes: usize,
    replay_cache: BTreeMap<String, CachedReplay>,
}

impl TowerBuilder {
    pub fn new(
        journal: DomainEventJournal,
        basis: Basis,
        limits: TowerLimits,
        read_units: u64,
    ) -> Result<Self, TowerInstallError> {
        if !limits.valid() || read_units > limits.max_read_units {
            return Err(TowerInstallError::InvalidLimits);
        }
        if basis.control_seq() != journal.last_control_seq() {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        if journal.len() > limits.max_journal_events {
            return Err(TowerInstallError::LimitExceeded);
        }
        let (kind, id) = match journal.scope() {
            JournalScope::Production { domain_id } => (Noun::Run, domain_id.clone()),
            JournalScope::Branch { branch_id, .. } => (Noun::Branch, branch_id.to_string()),
        };
        let domain_address = Address::new(
            journal.tenant(),
            kind,
            id,
            Some(basis.semantic_generation()),
        )
        .map_err(|_| TowerInstallError::InvalidAddress)?;
        validate_address(&domain_address)?;
        if journal.scope().domain_id().len() > 128 {
            return Err(TowerInstallError::LimitExceeded);
        }
        let mut frontiers = BTreeMap::<u64, u64>::new();
        let mut previous = 0u64;
        let mut first_gap = None;
        for event in journal.events() {
            if event.encode_wire_canonical().len() > 16 * 1024 {
                return Err(TowerInstallError::LimitExceeded);
            }
            for address in &event.addresses {
                validate_address(address)?;
            }
            if let Some(next) = previous.checked_add(1) {
                if event.control_seq > next {
                    first_gap.get_or_insert(next);
                }
            }
            previous = event.control_seq;
            for frontier in &event.source_frontiers {
                let seq = frontiers.entry(frontier.source_id).or_default();
                if frontier.frontier_seq < *seq {
                    return Err(TowerInstallError::SnapshotMismatch);
                }
                *seq = frontier.frontier_seq;
            }
        }
        if frontiers.len() > 128 {
            return Err(TowerInstallError::LimitExceeded);
        }
        let mut proofs = basis.proofs();
        // An imported declaration of completeness cannot conceal a journal gap.
        // Preserve every other proof state, including already unsupported ones.
        if let (Some(first_missing), ScopedProof::Declared { scope_id, .. }) =
            (first_gap, proofs.complete_for_contract)
        {
            proofs.complete_for_contract = ScopedProof::Gapped {
                scope_id,
                first_missing,
            };
        }
        let snapshot_basis = Basis::new(basis.identity(), basis.claim_class(), proofs);
        Ok(Self {
            resolver: AddressResolver::new(),
            journal,
            domain_address,
            snapshot_basis,
            frontiers: frontiers
                .into_iter()
                .map(|(source_id, frontier_seq)| SourceFrontierEntry {
                    source_id,
                    frontier_seq,
                })
                .collect(),
            limits,
            remaining_read_units: read_units,
            objects: BTreeMap::new(),
            edge_count: 0,
            explanations: BTreeMap::new(),
            replay_context: None,
            replays: BTreeMap::new(),
            replay_retained_bytes: 0,
            replay_cache: BTreeMap::new(),
        })
    }

    pub fn domain_address(&self) -> &Address {
        &self.domain_address
    }

    pub fn insert_object(
        &mut self,
        object: ResolvedObject,
        basis: Basis,
    ) -> Result<(), TowerInstallError> {
        validate_address(&object.address)?;
        if object.address.tenant != self.journal.tenant()
            || object.plane != object.address.kind.plane()
            || object
                .address
                .generation
                .is_some_and(|g| g != object.generation)
            || object.generation != basis.semantic_generation()
            || basis.control_seq() > self.snapshot_basis.control_seq()
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let key = object.address.without_generation();
        if self.objects.contains_key(&key) {
            return Err(TowerInstallError::DuplicateObject);
        }
        if self.objects.len() >= self.limits.max_objects {
            return Err(TowerInstallError::LimitExceeded);
        }
        self.objects.insert(key, (object.generation, basis));
        self.resolver.insert_object(object, basis);
        Ok(())
    }

    pub fn insert_edge(
        &mut self,
        from: Address,
        relation: RelationKind,
        to: Address,
    ) -> Result<(), TowerInstallError> {
        validate_address(&from)?;
        validate_address(&to)?;
        let Some((generation, _)) = self.objects.get(&from.without_generation()) else {
            return Err(TowerInstallError::MissingSource);
        };
        if from.generation.is_some_and(|g| g != *generation) {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        if self.edge_count >= self.limits.max_edges {
            return Err(TowerInstallError::LimitExceeded);
        }
        self.resolver.insert_edge(from, relation, to);
        self.edge_count += 1;
        Ok(())
    }

    pub fn insert_explanation(
        &mut self,
        tree: PredicateTree,
        basis: Basis,
    ) -> Result<(), TowerInstallError> {
        validate_address(&tree.subject_address)?;
        validate_node(&tree.root, 1, &mut BTreeSet::new(), basis)?;
        let rebuilt = PredicateTree::new(tree.subject_address.clone(), tree.root.clone())
            .map_err(|_| TowerInstallError::InvalidExplanation)?;
        if rebuilt != tree {
            return Err(TowerInstallError::InvalidExplanation);
        }
        let key = tree.subject_address.without_generation();
        let Some((generation, object_basis)) = self.objects.get(&key) else {
            return Err(TowerInstallError::MissingSource);
        };
        if basis != *object_basis
            || basis.control_seq() != self.snapshot_basis.control_seq()
            || tree
                .subject_address
                .generation
                .is_some_and(|g| g != *generation)
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        if self.explanations.contains_key(&key) {
            return Err(TowerInstallError::DuplicateObject);
        }
        if self.explanations.len() >= self.limits.max_explanations {
            return Err(TowerInstallError::LimitExceeded);
        }
        self.explanations.insert(key, Explanation { tree, basis });
        Ok(())
    }

    pub fn build(self) -> ReferenceTower {
        ReferenceTower { snapshot: self }
    }
}

pub struct ReferenceTower {
    snapshot: TowerBuilder,
}

impl ReferenceTower {
    pub fn domain_address(&self) -> &Address {
        &self.snapshot.domain_address
    }
    pub fn remaining_read_units(&self) -> u64 {
        self.snapshot.remaining_read_units
    }
    pub fn journal_control_seq(&self) -> u64 {
        self.snapshot.journal.last_control_seq()
    }

    /// Execute a connected bounded read or isolated paired replay. Unsupported
    /// mutations never reach ProposalEngine or a native effect owner.
    pub fn dispatch(&mut self, access: &TowerAccess, request: TowerRequest) -> TowerResponse {
        let address_valid = validate_address(&request.address).is_ok();
        if request.operation_id.is_empty()
            || request.operation_id.len() > 128
            || request
                .idempotency_key
                .as_ref()
                .is_some_and(|key| key.is_empty() || key.len() > 128)
            || !address_valid
        {
            // Bounded echoes prevent an invalid request from creating an unbounded error.
            let mut normalized = request;
            normalized.operation_id = "<invalid-request>".to_owned();
            normalized.idempotency_key = None;
            if !address_valid {
                normalized.address =
                    Address::new("invalid", Noun::Run, "request", None).expect("constant address");
            }
            return self.refusal(&normalized, ReasonCode::IntegrityFailure, false);
        }
        let request = request;
        let disclose_accounting =
            access.operator && access.full_domain(self.snapshot.journal.tenant());
        if access.scope.is_authorized_for(&request.address).is_err()
            || access.scope.caller_tenant != self.snapshot.journal.tenant()
        {
            return self.withheld(&request);
        }
        if !request.verb.is_connected() {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
        }
        if request.verb == Verb::Replay {
            return self.dispatch_replay(access, request);
        }
        if request.verb == Verb::ReplayPlan {
            return self.dispatch_replay_plan(access, request);
        }
        let needs_domain = matches!(
            request.verb,
            Verb::Tail | Verb::Situation | Verb::Explain | Verb::WhyHeld | Verb::Next
        );
        let needs_operator = matches!(
            request.verb,
            Verb::Tail | Verb::Situation | Verb::WhyHeld | Verb::Next
        );
        if (needs_domain && !access.full_domain(self.snapshot.journal.tenant()))
            || (needs_operator && !access.operator)
        {
            return self.withheld(&request);
        }
        let args_ok = match (&request.verb, &request.arguments) {
            (_, ReadArguments::None) => true,
            (Verb::Related, ReadArguments::Related { .. }) => true,
            (Verb::Tail, ReadArguments::Tail { max_events, .. }) => {
                *max_events > 0 && *max_events <= self.snapshot.limits.max_page_events
            }
            (Verb::Next, ReadArguments::Next { .. }) => true,
            _ => false,
        };
        if !args_ok {
            return self.refusal(&request, ReasonCode::BudgetExhausted, disclose_accounting);
        }
        if matches!(request.verb, Verb::Tail | Verb::Situation)
            && request.address.without_generation()
                != self.snapshot.domain_address.without_generation()
        {
            return self.refusal(
                &request,
                ReasonCode::MissingRequiredObservation,
                disclose_accounting,
            );
        }
        if self.snapshot.remaining_read_units == 0 {
            return self.refusal(&request, ReasonCode::BudgetExhausted, disclose_accounting);
        }

        self.snapshot.remaining_read_units -= 1;
        let mut affordances = Knowledge::unknown(UnknownReason::Unavailable);
        let mut observed_frontiers = Knowledge::unknown(UnknownReason::Unavailable);
        let result = match request.verb {
            Verb::Get => self
                .snapshot
                .resolver
                .fa_get(&access.scope, &request.address)
                .map(TowerValue::Object),
            Verb::Related => {
                let relation = match request.arguments {
                    ReadArguments::Related { relation } => relation,
                    _ => None,
                };
                self.snapshot
                    .resolver
                    .fa_related(&access.scope, &request.address, relation)
                    .map(TowerValue::Related)
            }
            Verb::Tail | Verb::Situation => {
                let basis = self.snapshot.snapshot_basis;
                match requested_generation(&request.address, basis) {
                    Some(knowledge) => knowledge,
                    None => {
                        observed_frontiers =
                            Knowledge::known(self.snapshot.frontiers.clone(), basis);
                        if request.verb == Verb::Tail {
                            let (since_seq, max_events) = match request.arguments {
                                ReadArguments::Tail {
                                    since_seq,
                                    max_events,
                                } => (since_seq, max_events),
                                _ => (0, self.snapshot.limits.max_page_events),
                            };
                            Knowledge::known(
                                TowerValue::Tail(
                                    self.snapshot.journal.fa_tail(since_seq, max_events),
                                ),
                                basis,
                            )
                        } else {
                            let mut report = fold_situation_report(
                                &self.snapshot.journal,
                                self.snapshot.journal.last_control_seq(),
                            );
                            // The fold retains each item's causal event sequence,
                            // but uses an illustrative generation-one context.
                            // Bind the context to this imported snapshot while
                            // preserving that causal cut and all gap notices.
                            for item in &mut report.items {
                                if let Knowledge::Known {
                                    basis: item_basis, ..
                                } = item
                                {
                                    let mut identity = basis.identity();
                                    identity.control_seq = item_basis.control_seq();
                                    *item_basis =
                                        Basis::new(identity, basis.claim_class(), basis.proofs());
                                }
                            }
                            Knowledge::known(TowerValue::Situation(report), basis)
                        }
                    }
                }
            }
            Verb::Explain | Verb::WhyHeld | Verb::Next => {
                match self
                    .snapshot
                    .explanations
                    .get(&request.address.without_generation())
                {
                    None => Knowledge::unknown(UnknownReason::MissingExactDomain),
                    Some(explanation) => {
                        if !node_visible(&explanation.tree.root, &access.scope) {
                            Knowledge::withheld(AuthorityKind::Judge)
                        } else if let Some(knowledge) =
                            requested_generation(&request.address, explanation.basis)
                        {
                            knowledge
                        } else {
                            let all = generate_affordances_for_tree(&explanation.tree);
                            let visible: Vec<_> = all
                                .into_iter()
                                .filter(|a| {
                                    access.scope.is_authorized_for(&a.target).is_ok()
                                        && a.would_change.iter().all(|address| {
                                            access.scope.is_authorized_for(address).is_ok()
                                        })
                                })
                                .collect();
                            let requested_budget = match request.arguments {
                                ReadArguments::Next { max_cost } => max_cost,
                                _ => access.affordance_budget,
                            };
                            let permitted = fa_next(
                                &visible,
                                &access.allowed_purposes,
                                requested_budget.min(access.affordance_budget),
                            );
                            affordances = Knowledge::known(permitted.clone(), explanation.basis);
                            let value = match request.verb {
                                Verb::Explain => TowerValue::Explain(explanation.tree.clone()),
                                Verb::WhyHeld => {
                                    let mut report = fa_why_held(
                                        &explanation.tree,
                                        explanation.basis.control_seq(),
                                    );
                                    for item in &mut report.items {
                                        item.affordances
                                            .retain(|affordance| permitted.contains(affordance));
                                    }
                                    TowerValue::WhyHeld(report)
                                }
                                _ => TowerValue::Next(permitted),
                            };
                            Knowledge::known(value, explanation.basis)
                        }
                    }
                }
            }
            _ => unreachable!("connected read vocabulary is exhaustive above"),
        };
        let mut response = self.response(&request, result, 1, disclose_accounting);
        response.envelope.affordances = affordances;
        response.envelope.source_frontiers = observed_frontiers;
        // Read work has already occurred. A refused oversized encoding does not
        // refund the allowance and can never turn a truncated value into Known.
        match response.to_json() {
            Ok(_) => response,
            Err(error) => {
                response.envelope.status = ResponseStatus::Error;
                response.envelope.reason_code = Some(match error {
                    TowerEncodingError::OutputLimit => ReasonCode::BudgetExhausted,
                    TowerEncodingError::InvalidKnowledge => ReasonCode::IntegrityFailure,
                });
                response.result = Knowledge::unknown(UnknownReason::Incomplete);
                response.envelope.affordances = Knowledge::unknown(UnknownReason::Unavailable);
                response.envelope.source_frontiers = Knowledge::unknown(UnknownReason::Unavailable);
                response.envelope.missing = vec![request.address.clone()];
                response.envelope.would_change_this = vec![request.address];
                response
            }
        }
    }

    fn response(
        &self,
        request: &TowerRequest,
        result: Knowledge<TowerValue>,
        cost: u64,
        disclose_budget: bool,
    ) -> TowerResponse {
        let basis = result.basis();
        let reason_code = match &result {
            Knowledge::Known { .. } | Knowledge::Absent { .. } => None,
            Knowledge::Withheld { .. } => Some(ReasonCode::DeniedByPolicy),
            Knowledge::Stale { .. } => Some(ReasonCode::StaleGeneration),
            Knowledge::Unknown {
                reason: UnknownReason::IncompatibleGeneration,
            } => Some(ReasonCode::StaleGeneration),
            Knowledge::Unknown {
                reason: UnknownReason::LowerLayerUnverified,
            } => Some(ReasonCode::LowerLayerUnverified),
            _ => Some(ReasonCode::MissingRequiredObservation),
        };
        let meta = |f: fn(Basis) -> u64| {
            basis.map_or_else(
                || Knowledge::unknown(UnknownReason::Unavailable),
                |basis| Knowledge::known(f(basis), basis),
            )
        };
        let accounting = |value| {
            if disclose_budget {
                Knowledge::known(value, self.snapshot.snapshot_basis)
            } else {
                Knowledge::withheld(AuthorityKind::Verify)
            }
        };
        let remedies = if reason_code.is_some() {
            vec![request.address.clone()]
        } else {
            Vec::new()
        };
        TowerResponse {
            envelope: TowerEnvelope {
                schema_version: RESPONSE_SCHEMA,
                operation_id: request.operation_id.clone(),
                status: if reason_code.is_some() {
                    ResponseStatus::Error
                } else {
                    ResponseStatus::Ok
                },
                reason_code,
                address: request.address.clone(),
                generation: meta(Basis::semantic_generation),
                control_seq: meta(Basis::control_seq),
                source_frontiers: Knowledge::unknown(UnknownReason::Unavailable),
                missing: remedies.clone(),
                limits: self.snapshot.limits,
                budgets_remaining: accounting(self.snapshot.remaining_read_units),
                realized_cost: accounting(cost),
                affordances: Knowledge::unknown(UnknownReason::Unavailable),
                retry_safe: request.verb.class() == VerbClass::Read,
                idempotency_key: request.idempotency_key.clone(),
                would_change_this: remedies,
            },
            result,
        }
    }

    fn refusal(
        &self,
        request: &TowerRequest,
        reason: ReasonCode,
        disclose_budget: bool,
    ) -> TowerResponse {
        let mut response = self.response(
            request,
            Knowledge::unknown(UnknownReason::Unavailable),
            0,
            disclose_budget,
        );
        response.envelope.reason_code = Some(reason);
        response
    }

    fn withheld(&self, request: &TowerRequest) -> TowerResponse {
        let mut response = self.response(
            request,
            Knowledge::withheld(AuthorityKind::Verify),
            0,
            false,
        );
        response.envelope.generation = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.control_seq = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.source_frontiers = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.affordances = Knowledge::withheld(AuthorityKind::Verify);
        response
    }
}

fn validate_address(address: &Address) -> Result<(), TowerInstallError> {
    if address.tenant.len() > 256
        || address.id.len() > 256
        || address.canonical_string().len() > 256
        || Address::new(
            address.tenant.clone(),
            address.kind,
            address.id.clone(),
            address.generation,
        )
        .is_err()
    {
        return Err(TowerInstallError::InvalidAddress);
    }
    Ok(())
}

fn validate_node(
    node: &PredicateNode,
    depth: usize,
    ids: &mut BTreeSet<String>,
    expected_basis: Basis,
) -> Result<(), TowerInstallError> {
    if depth > 16
        || ids.len() >= 256
        || node.node_id.is_empty()
        || node.node_id.len() > 128
        || !ids.insert(node.node_id.clone())
        || node.evidence_addresses.is_empty()
        || node.evidence_addresses.len() > 32
    {
        return Err(TowerInstallError::InvalidExplanation);
    }
    if let crate::explain_affordances::PredicateNodeKind::PolicyPredicate { predicate_id } =
        &node.kind
    {
        if predicate_id.len() > 128 {
            return Err(TowerInstallError::InvalidExplanation);
        }
    }
    if let Knowledge::Pending { frontiers, .. } = &node.status {
        if frontiers.len() > crate::observation_algebra::MAX_OPERATOR_FRONTIERS {
            return Err(TowerInstallError::InvalidExplanation);
        }
    }
    if let Knowledge::Known { basis, .. } = &node.status {
        if basis.semantic_generation() != expected_basis.semantic_generation()
            || basis.policy_epoch() != expected_basis.policy_epoch()
            || basis.model_epoch() != expected_basis.model_epoch()
            || basis.tokenizer_epoch() != expected_basis.tokenizer_epoch()
            || basis.codec_epoch() != expected_basis.codec_epoch()
            || basis.control_seq() != expected_basis.control_seq()
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
    }
    for address in &node.evidence_addresses {
        validate_address(address)?;
    }
    for child in &node.children {
        validate_node(child, depth + 1, ids, expected_basis)?;
    }
    Ok(())
}

fn node_visible(node: &PredicateNode, scope: &CallerScope) -> bool {
    node.evidence_addresses
        .iter()
        .all(|address| scope.is_authorized_for(address).is_ok())
        && node.children.iter().all(|child| node_visible(child, scope))
}

fn requested_generation(address: &Address, basis: Basis) -> Option<Knowledge<TowerValue>> {
    match address.generation {
        Some(generation) if generation < basis.semantic_generation() => {
            Some(Knowledge::stale(basis, basis.semantic_generation()))
        }
        Some(generation) if generation > basis.semantic_generation() => {
            Some(Knowledge::unknown(UnknownReason::IncompatibleGeneration))
        }
        _ => None,
    }
}
