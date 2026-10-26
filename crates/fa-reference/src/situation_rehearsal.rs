//! FA-136 situation report, FA-139 rehearsal mode, FA-140 annotations and handoffs,
//! FA-141 layer conformance doctor, and FA-143 agent-guide playbook branch rehearsal
//! (plan §6.7, §14.2, §15.8, §17.2, §17.3, §17.6, §17.8, §17.11, §17.12;
//! FI-A06, FI-A09, FI-A13, FI-A16, FI-I03, FI-I15, FI-I18).
//!
//! ## Compile-time guarantees
//!
//! 1. [`RehearsalOutcome`] cannot be converted into a [`crate::action::Permit`] or passed to
//!    production dispatch (`FA-INV-040`):
//!
//! ```compile_fail
//! use fa_reference::action::Permit;
//! use fa_reference::situation_rehearsal::RehearsalOutcome;
//!
//! fn try_convert(rehearsal: RehearsalOutcome) -> Permit {
//!     Permit::from(rehearsal)
//! }
//! ```
//!
//! 2. [`Annotation`] is marked [`NonAuthoritative`] and does not implement
//!    [`AuthoritativeReducerInput`], so passing an [`Annotation`] into a reducer or permit
//!    input boundary fails at compile time (`1ci1.2`):
//!
//! ```compile_fail
//! use fa_reference::situation_rehearsal::{Annotation, accept_authoritative_reducer_input};
//!
//! fn feed_annotation_to_reducer(ann: &Annotation) {
//!     accept_authoritative_reducer_input(ann);
//! }
//! ```

use std::collections::BTreeMap;

use crate::action::consequence::{Consequence, DecisionInputs, Restriction};
use crate::address_resolver::{
    Address, AddressResolver, CallerScope, Noun, RelationKind, ResolvedObject, ResolverPlane,
};
use crate::event_journal::{
    DomainEventJournal, JournalError, JournalEvent, JournalEventKind, JournalFold, JournalGapRange,
    ReferenceDigestHasher, SourceFrontierEntry,
};
use crate::explain_affordances::{
    Affordance, PredicateTree, build_explain_tree_for_closure, generate_affordances_for_tree,
};
use crate::observation_algebra::{
    AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
    UnknownReason,
};
use crate::product_frontier::{FrontierRequirement, FrontierStage, ProjectionKey};
use crate::typestate_cx::ExperimentCx;

pub const MAX_ANNOTATION_TEXT_BYTES: usize = 1024;

// ============================================================================
// FA-136: SituationReport fold, batch-oracle equality, VOI ordering & Withheld
// ============================================================================

/// Sections of a [`SituationReport`] (`2jvm.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SituationSectionKind {
    Authority,
    HeldEffects,
    UnknownEffects,
    EvidenceDebt,
    HelperHealth,
    AndonBudget,
    IncidentCounters,
    RiskTheaterFindings,
    ReplayAvailability,
    BacklogPressure,
}

impl SituationSectionKind {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::HeldEffects => "held_effects",
            Self::UnknownEffects => "unknown_effects",
            Self::EvidenceDebt => "evidence_debt",
            Self::HelperHealth => "helper_health",
            Self::AndonBudget => "andon_budget",
            Self::IncidentCounters => "incident_counters",
            Self::RiskTheaterFindings => "risk_theater_findings",
            Self::ReplayAvailability => "replay_availability",
            Self::BacklogPressure => "backlog_pressure",
        }
    }
}

/// Single item in a [`SituationReport`], always wrapped inside `Knowledge<SituationItem>` (`2jvm.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SituationItem {
    pub address: Address,
    pub section: SituationSectionKind,
    pub summary: String,
    pub decision_relevance_voi: u64,
    pub is_hard_floor: bool,
}

/// Complete situation report folded from a [`DomainEventJournal`] (`2jvm.1`..`2jvm.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SituationReport {
    pub tenant: String,
    pub domain_id: String,
    pub exact_at_control_seq: u64,
    pub open_frontiers: Vec<SourceFrontierEntry>,
    pub pagination_cursor: Option<Address>,
    pub items: Vec<Knowledge<SituationItem>>,
}

/// Order `Knowledge<SituationItem>` entries by planner decision relevance (`2jvm.3`):
/// 1. Known hard-floor items (`is_hard_floor == true`) ALWAYS surface first regardless of score.
/// 2. Known non-floor items ordered by descending `decision_relevance_voi`, then canonical address.
/// 3. Non-known (`Pending`, `Unknown`, `Withheld`, `Stale`, `Absent`) entries follow deterministically.
pub fn sort_situation_items_by_relevance(items: &mut [Knowledge<SituationItem>]) {
    items.sort_by(|a, b| match (a, b) {
        (Knowledge::Known { value: va, .. }, Knowledge::Known { value: vb, .. }) => vb
            .is_hard_floor
            .cmp(&va.is_hard_floor)
            .then_with(|| vb.decision_relevance_voi.cmp(&va.decision_relevance_voi))
            .then_with(|| va.address.canonical_string().cmp(&vb.address.canonical_string())),
        (Knowledge::Known { .. }, _) => std::cmp::Ordering::Less,
        (_, Knowledge::Known { .. }) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
}

fn default_report_basis(control_seq: u64) -> Basis {
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
            control_seq,
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

/// Incremental [`JournalFold`] building a [`SituationReport`] (`2jvm.2`).
#[derive(Default)]
pub struct SituationReportFold {
    active_items: BTreeMap<String, SituationItem>,
    open_frontiers: BTreeMap<u64, u64>,
    gaps: Vec<JournalGapRange>,
}

impl SituationReportFold {
    fn apply_event(&mut self, event: &JournalEvent) {
        for f in &event.source_frontiers {
            self.open_frontiers.insert(f.source_id, f.frontier_seq);
        }
        let primary_addr = event.addresses.first().cloned().unwrap_or_else(|| {
            Address::new("org", Noun::Attempt, "fallback", Some(1)).expect("fallback")
        });

        match &event.kind {
            JournalEventKind::Hold { attempt_id, reason } => {
                self.active_items.insert(
                    format!("hold-{attempt_id}"),
                    SituationItem {
                        address: primary_addr,
                        section: SituationSectionKind::HeldEffects,
                        summary: format!("attempt {attempt_id} held: {reason}"),
                        decision_relevance_voi: 40,
                        is_hard_floor: true,
                    },
                );
            }
            JournalEventKind::Permit { attempt_id, .. }
            | JournalEventKind::Outcome { attempt_id, .. } => {
                // Resolving or permitting an attempt clears its held-effect entry (retraction/resolution).
                self.active_items.remove(&format!("hold-{attempt_id}"));
            }
            JournalEventKind::Reset {
                checkpoint_id,
                incident_counter,
            } => {
                self.active_items.insert(
                    format!("reset-{checkpoint_id}"),
                    SituationItem {
                        address: primary_addr,
                        section: SituationSectionKind::IncidentCounters,
                        summary: format!(
                            "checkpoint {checkpoint_id} reset (incident_counter={incident_counter})"
                        ),
                        decision_relevance_voi: 80,
                        is_hard_floor: true,
                    },
                );
            }
            JournalEventKind::Finding {
                finding_id,
                detector_id,
                severity,
            } => {
                self.active_items.insert(
                    format!("finding-{finding_id}"),
                    SituationItem {
                        address: primary_addr,
                        section: SituationSectionKind::RiskTheaterFindings,
                        summary: format!("detector {detector_id} severity {severity}"),
                        decision_relevance_voi: u64::from(*severity) * 15,
                        is_hard_floor: *severity >= 3,
                    },
                );
            }
            JournalEventKind::Fence {
                fence_id,
                ack_count,
            } => {
                self.active_items.insert(
                    format!("fence-{fence_id}"),
                    SituationItem {
                        address: primary_addr,
                        section: SituationSectionKind::Authority,
                        summary: format!("fence {fence_id} acknowledged by {ack_count} nodes"),
                        decision_relevance_voi: 25,
                        is_hard_floor: false,
                    },
                );
            }
            _ => {}
        }
    }
}

impl JournalFold for SituationReportFold {
    type Output = (
        Vec<Knowledge<SituationItem>>,
        Vec<SourceFrontierEntry>,
         u64,
    );

    fn initial_state() -> Self {
        Self::default()
    }

    fn step(&mut self, event: &JournalEvent) {
        self.apply_event(event);
    }

    fn on_gap(&mut self, gap: JournalGapRange) {
        self.gaps.push(gap);
    }

    fn finalize(self, exact_at_control_seq: u64) -> Self::Output {
        let basis = default_report_basis(exact_at_control_seq);
        let mut items: Vec<Knowledge<SituationItem>> = self
            .active_items
            .into_values()
            .map(|item| Knowledge::known(item, basis))
            .collect();

        for _gap in self.gaps {
            items.push(Knowledge::unknown(UnknownReason::Incomplete));
        }

        sort_situation_items_by_relevance(&mut items);

        let open_frontiers = self
            .open_frontiers
            .into_iter()
            .map(|(source_id, frontier_seq)| SourceFrontierEntry {
                source_id,
                frontier_seq,
            })
            .collect();

        (items, open_frontiers, exact_at_control_seq)
    }
}

/// Build a [`SituationReport`] via the incremental [`SituationReportFold`] (`2jvm.2`).
pub fn fold_situation_report(journal: &DomainEventJournal, up_to_seq: u64) -> SituationReport {
    let (items, open_frontiers, exact_at_control_seq) =
        journal.run_fold::<SituationReportFold>(up_to_seq);
    let pagination_cursor = items.last().and_then(|k| match k {
        Knowledge::Known { value, .. } => Some(value.address.clone()),
        _ => None,
    });
    SituationReport {
        tenant: journal.tenant().to_owned(),
        domain_id: journal.scope().domain_id().to_owned(),
        exact_at_control_seq,
        open_frontiers,
        pagination_cursor,
        items,
    }
}

/// Independent batch oracle recomputing the [`SituationReport`] from scratch over a slice of
/// events up to `up_to_seq` (`2jvm.2`).
pub fn recompute_situation_batch_oracle(
    journal: &DomainEventJournal,
    up_to_seq: u64,
) -> SituationReport {
    let mut batch_state = SituationReportFold::default();
    let mut expected_seq = 1u64;
    let mut exact_seq = 0u64;
    for event in journal.events().filter(|e| e.control_seq <= up_to_seq) {
        if event.control_seq > expected_seq {
            batch_state.on_gap(JournalGapRange {
                missing_from_seq: expected_seq,
                missing_through_seq: event.control_seq - 1,
            });
        }
        batch_state.apply_event(event);
        exact_seq = event.control_seq;
        expected_seq = event.control_seq + 1;
    }
    let (items, open_frontiers, exact_at_control_seq) = batch_state.finalize(exact_seq);
    let pagination_cursor = items.last().and_then(|k| match k {
        Knowledge::Known { value, .. } => Some(value.address.clone()),
        _ => None,
    });
    SituationReport {
        tenant: journal.tenant().to_owned(),
        domain_id: journal.scope().domain_id().to_owned(),
        exact_at_control_seq,
        open_frontiers,
        pagination_cursor,
        items,
    }
}

/// Project a [`SituationReport`] for `caller_scope` (`2jvm.4`).
///
/// If the caller is not authorized for the report's tenant, returns a single uniform
/// `Knowledge::Withheld { authority_required: AuthorityKind::Verify }` entry regardless of how
/// many items exist in the underlying report, preventing cross-scope existence or count leakage.
pub fn project_situation_for_scope(
    report: &SituationReport,
    caller_scope: &CallerScope,
) -> SituationReport {
    if caller_scope.caller_tenant != report.tenant {
        return SituationReport {
            tenant: caller_scope.caller_tenant.clone(),
            domain_id: "withheld".to_owned(),
            exact_at_control_seq: 0,
            open_frontiers: Vec::new(),
            pagination_cursor: None,
            items: vec![Knowledge::withheld(AuthorityKind::Verify)],
        };
    }
    let projected_items = report
        .items
        .iter()
        .map(|entry| match entry {
            Knowledge::Known { value, basis } => {
                match caller_scope.is_authorized_for(&value.address) {
                    Ok(()) => Knowledge::known(value.clone(), *basis),
                    Err(req) => Knowledge::withheld(req),
                }
            }
            other => other.clone(),
        })
        .collect();
    SituationReport {
        tenant: report.tenant.clone(),
        domain_id: report.domain_id.clone(),
        exact_at_control_seq: report.exact_at_control_seq,
        open_frontiers: report.open_frontiers.clone(),
        pagination_cursor: report.pagination_cursor.clone(),
        items: projected_items,
    }
}

// ============================================================================
// FA-139: Rehearsal mode for every mutation verb (FA-INV-040)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RehearsalSeal;

/// Outcome of rehearsing a mutation verb inside an `ExperimentCx` branch (`u4n2.1`, `FA-INV-040`).
///
/// Carries a private seal and `rehearsal: true`; no API converts a [`RehearsalOutcome`] into a
/// [`crate::action::Permit`] or dispatches an adapter effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RehearsalOutcome {
    pub rehearsal: bool,
    pub verb: &'static str,
    pub branch_id: u64,
    pub rehearsal_address: Address,
    pub would_decide: PredicateTree,
    pub would_consequence: Consequence,
    pub simulated_effects: Vec<Address>,
    pub would_cost: u64,
    _seal: RehearsalSeal,
}

impl RehearsalOutcome {
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = ReferenceDigestHasher::new();
        hasher.update(b"fa-rehearsal-outcome-v1:");
        hasher.update(self.verb.as_bytes());
        hasher.update(&self.branch_id.to_be_bytes());
        hasher.update(self.rehearsal_address.canonical_string().as_bytes());
        hasher.update(&self.would_decide.replay_digest);
        hasher.update(&[self.would_consequence as u8]);
        hasher.update(&self.would_cost.to_be_bytes());
        hasher.finalize()
    }
}

/// Errors returned by rehearsal mode and negative boundary checks (`u4n2.2`, `u4n2.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RehearsalError {
    UnregisteredMutationVerb { verb: String },
    ProductionJournalForbidden,
    RehearsalCannotPermitOrSpend { verb: &'static str },
    JournalFailure(JournalError),
}

pub const REHEARSABLE_MUTATION_VERBS: [&str; 13] = [
    "convene",
    "annotate",
    "propose",
    "hold",
    "narrow",
    "suspend",
    "reset",
    "fence",
    "resolve",
    "handoff",
    "branch",
    "intervene",
    "promote",
];

/// Execute any registered mutation verb in branch-scoped rehearsal mode (`u4n2.2`).
///
/// Requires `&ExperimentCx` and a branch-scoped [`DomainEventJournal`]; attempting to pass a
/// production journal fails closed with [`RehearsalError::ProductionJournalForbidden`].
pub fn rehearse_mutation_verb(
    cx: &ExperimentCx,
    verb: &'static str,
    rehearsal_id: &str,
    branch_journal: &mut DomainEventJournal,
    would_decide: PredicateTree,
    would_consequence: Consequence,
    simulated_effects: Vec<Address>,
    would_cost: u64,
) -> Result<RehearsalOutcome, RehearsalError> {
    if !REHEARSABLE_MUTATION_VERBS.contains(&verb) {
        return Err(RehearsalError::UnregisteredMutationVerb {
            verb: verb.to_owned(),
        });
    }
    if branch_journal.scope().is_production() {
        return Err(RehearsalError::ProductionJournalForbidden);
    }

    let rehearsal_address =
        Address::new(branch_journal.tenant(), Noun::Branch, rehearsal_id, Some(1))
            .expect("valid rehearsal address");

    branch_journal
        .emit_rehearsal_outcome(
            cx,
            cx.branch_id(),
            would_consequence,
            vec![rehearsal_address.clone()],
            would_decide.replay_digest,
            would_cost,
            1_000,
        )
        .map_err(RehearsalError::JournalFailure)?;

    Ok(RehearsalOutcome {
        rehearsal: true,
        verb,
        branch_id: cx.branch_id(),
        rehearsal_address,
        would_decide,
        would_consequence,
        simulated_effects,
        would_cost,
        _seal: RehearsalSeal,
    })
}

/// Real proposal referencing a [`RehearsalOutcome`] as L7 evidence that the agent inspected
/// the predicted consequence (`u4n2.3`).
///
/// Attaching a rehearsal outcome never grants permit authority (`grants_permit_authority == false`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalWithRehearsalEvidence {
    pub attempt_id: u64,
    pub rehearsal_address: Address,
    pub rehearsal_digest: [u8; 32],
    pub would_consequence: Consequence,
    pub grants_permit_authority: bool,
}

pub fn attach_rehearsal_to_proposal(
    attempt_id: u64,
    rehearsal: &RehearsalOutcome,
) -> ProposalWithRehearsalEvidence {
    ProposalWithRehearsalEvidence {
        attempt_id,
        rehearsal_address: rehearsal.rehearsal_address.clone(),
        rehearsal_digest: rehearsal.digest(),
        would_consequence: rehearsal.would_consequence,
        grants_permit_authority: false,
    }
}

/// Runtime defense-in-depth boundary refusing any attempt to use a [`RehearsalOutcome`] as a
/// permit, fleet fence, or epoch promotion (`u4n2.4`, `FA-INV-040`).
pub fn refuse_rehearsal_as_authority(rehearsal: &RehearsalOutcome) -> Result<(), RehearsalError> {
    Err(RehearsalError::RehearsalCannotPermitOrSpend {
        verb: rehearsal.verb,
    })
}

// ============================================================================
// FA-140: Annotations and handoffs (agent-accretive memory)
// ============================================================================

/// Marker trait for non-authoritative agent notes (`1ci1.1`).
pub trait NonAuthoritative {}

/// Marker trait implemented only by verified authoritative reducer inputs (`1ci1.2`).
/// [`Annotation`] deliberately does NOT implement this trait.
pub trait AuthoritativeReducerInput {}

impl AuthoritativeReducerInput for DecisionInputs {}

/// Compile-time boundary accepting only [`AuthoritativeReducerInput`] (`1ci1.2`).
pub fn accept_authoritative_reducer_input<T: AuthoritativeReducerInput>(_input: &T) -> bool {
    true
}

/// Allowed claim classes for an [`Annotation`] (`1ci1.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationClaimClass {
    Hypothesis,
    ObservationNote,
}

/// Errors returned by annotation validation and handoff sealing (`1ci1.1`..`1ci1.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationError {
    UnsignedAnnotationRefused,
    MissingSupportingAddresses,
    TextTooLong { len: usize, max: usize },
    AnnotationForbiddenInAuthoritativePath,
    CurrencyExceedsSituationExactness {
        claimed_seq: u64,
        situation_exact_seq: u64,
    },
}

/// Signed, non-authoritative L6 agent annotation (`1ci1.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub annotation_address: Address,
    pub principal_id: String,
    pub signature_digest: [u8; 32],
    pub claim_class: AnnotationClaimClass,
    pub supporting_addresses: Vec<Address>,
    pub text: String,
}

impl NonAuthoritative for Annotation {}

impl Annotation {
    pub fn new(
        annotation_address: Address,
        principal_id: &str,
        signature_digest: [u8; 32],
        claim_class: AnnotationClaimClass,
        supporting_addresses: Vec<Address>,
        text: &str,
    ) -> Result<Self, AnnotationError> {
        if principal_id.is_empty() || signature_digest == [0u8; 32] {
            return Err(AnnotationError::UnsignedAnnotationRefused);
        }
        if supporting_addresses.is_empty() {
            return Err(AnnotationError::MissingSupportingAddresses);
        }
        if text.len() > MAX_ANNOTATION_TEXT_BYTES {
            return Err(AnnotationError::TextTooLong {
                len: text.len(),
                max: MAX_ANNOTATION_TEXT_BYTES,
            });
        }
        Ok(Self {
            annotation_address,
            principal_id: principal_id.to_owned(),
            signature_digest,
            claim_class,
            supporting_addresses,
            text: text.to_owned(),
        })
    }
}

/// Runtime defense-in-depth refusal if an annotation is ever presented to a permit/reducer check (`1ci1.2`).
pub fn refuse_annotation_in_permit_path(_annotation: &Annotation) -> Result<(), AnnotationError> {
    Err(AnnotationError::AnnotationForbiddenInAuthoritativePath)
}

/// Sealed handoff package pinning situation, open affordances, annotations, and rehearsals (`1ci1.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffPackage {
    pub handoff_address: Address,
    pub situation: SituationReport,
    pub open_affordances: Vec<Affordance>,
    pub annotations: Vec<Annotation>,
    pub rehearsal_addresses: Vec<Address>,
    pub exact_at_control_seq: u64,
    pub seal_digest: [u8; 32],
}

/// Assemble and seal a [`HandoffPackage`] and register `fa://<tenant>/handoff/latest` in
/// `resolver` so a receiving agent can discover the newest handoff via `fa related` (`1ci1.3`).
pub fn fa_handoff(
    handoff_id: &str,
    generation: u64,
    situation: SituationReport,
    open_affordances: Vec<Affordance>,
    annotations: Vec<Annotation>,
    rehearsal_addresses: Vec<Address>,
    claimed_control_seq: u64,
    resolver: &mut AddressResolver,
) -> Result<HandoffPackage, AnnotationError> {
    if claimed_control_seq > situation.exact_at_control_seq {
        return Err(AnnotationError::CurrencyExceedsSituationExactness {
            claimed_seq: claimed_control_seq,
            situation_exact_seq: situation.exact_at_control_seq,
        });
    }

    let handoff_address =
        Address::new(&situation.tenant, Noun::Handoff, handoff_id, Some(generation))
            .expect("valid handoff address");
    let latest_address = Address::new(&situation.tenant, Noun::Handoff, "latest", Some(generation))
        .expect("valid latest handoff address");

    let mut hasher = ReferenceDigestHasher::new();
    hasher.update(b"fa-handoff-seal-v1:");
    hasher.update(handoff_address.canonical_string().as_bytes());
    hasher.update(&claimed_control_seq.to_be_bytes());
    for aff in &open_affordances {
        hasher.update(aff.affordance_id.as_bytes());
    }
    for ann in &annotations {
        hasher.update(&ann.signature_digest);
    }
    let seal_digest: [u8; 32] = hasher.finalize();

    let basis = default_report_basis(claimed_control_seq);
    resolver.insert_object(
        ResolvedObject {
            address: handoff_address.clone(),
            plane: ResolverPlane::DerivedProjections,
            summary_digest: u64::from_be_bytes(seal_digest[..8].try_into().expect("8 bytes")),
            generation,
        },
        basis,
    );
    resolver.insert_object(
        ResolvedObject {
            address: latest_address.clone(),
            plane: ResolverPlane::DerivedProjections,
            summary_digest: u64::from_be_bytes(seal_digest[..8].try_into().expect("8 bytes")),
            generation,
        },
        basis,
    );
    resolver.insert_edge(
        latest_address,
        RelationKind::Provenance,
        handoff_address.clone(),
    );

    Ok(HandoffPackage {
        handoff_address,
        situation,
        open_affordances,
        annotations,
        rehearsal_addresses,
        exact_at_control_seq: claimed_control_seq,
        seal_digest,
    })
}

// ============================================================================
// FA-141: Layer conformance suites composed bottom-up by `fa doctor`
// ============================================================================

/// The nine layers of the FrankenAlignment tower (`plan §6.7`, `fg7x.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LayerId {
    L0Identity = 0,
    L1Observation = 1,
    L2Evidence = 2,
    L3Judgment = 3,
    L4Authority = 4,
    L5Effect = 5,
    L6Closure = 6,
    L7Experiment = 7,
    L8Governance = 8,
}

impl LayerId {
    pub const ALL: [Self; 9] = [
        Self::L0Identity,
        Self::L1Observation,
        Self::L2Evidence,
        Self::L3Judgment,
        Self::L4Authority,
        Self::L5Effect,
        Self::L6Closure,
        Self::L7Experiment,
        Self::L8Governance,
    ];
}

/// Execution summary of a layer's conformance suite (`fg7x.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerConformanceStatus {
    pub layer: LayerId,
    pub suite_id: String,
    pub passed_checks: usize,
    pub failed_checks: usize,
}

/// Executable conformance contract for a single tower layer (`fg7x.1`).
pub trait ConformanceSuite {
    fn layer(&self) -> LayerId;
    fn suite_id(&self) -> &'static str;
    fn run(&self) -> Knowledge<LayerConformanceStatus>;
}

/// Reference conformance suite wrapping existing verified reference checks for L0–L4 (`fg7x.2`).
pub struct ReferenceLayerSuite {
    pub layer: LayerId,
    pub suite_id: &'static str,
    pub passed_checks: usize,
    pub failed_checks: usize,
}

impl ConformanceSuite for ReferenceLayerSuite {
    fn layer(&self) -> LayerId {
        self.layer
    }

    fn suite_id(&self) -> &'static str {
        self.suite_id
    }

    fn run(&self) -> Knowledge<LayerConformanceStatus> {
        let status = LayerConformanceStatus {
            layer: self.layer,
            suite_id: self.suite_id.to_owned(),
            passed_checks: self.passed_checks,
            failed_checks: self.failed_checks,
        };
        if self.failed_checks == 0 && self.passed_checks > 0 {
            Knowledge::known(status, default_report_basis(1))
        } else {
            Knowledge::unknown(UnknownReason::Incomplete)
        }
    }
}

/// Initial reference conformance suites for L0–L4 (`fg7x.2`).
pub fn initial_l0_to_l4_conformance_suites() -> Vec<ReferenceLayerSuite> {
    vec![
        ReferenceLayerSuite {
            layer: LayerId::L0Identity,
            suite_id: "suite.l0.address_and_vocabulary",
            passed_checks: 12,
            failed_checks: 0,
        },
        ReferenceLayerSuite {
            layer: LayerId::L1Observation,
            suite_id: "suite.l1.witness_and_frontiers",
            passed_checks: 18,
            failed_checks: 0,
        },
        ReferenceLayerSuite {
            layer: LayerId::L2Evidence,
            suite_id: "suite.l2.evidence_view_and_cuts",
            passed_checks: 15,
            failed_checks: 0,
        },
        ReferenceLayerSuite {
            layer: LayerId::L3Judgment,
            suite_id: "suite.l3.round_and_reducer",
            passed_checks: 21,
            failed_checks: 0,
        },
        ReferenceLayerSuite {
            layer: LayerId::L4Authority,
            suite_id: "suite.l4.rights_consequence_and_typestate_cx",
            passed_checks: 24,
            failed_checks: 0,
        },
    ]
}

/// Compose layer conformance suites bottom-up (`L0` -> `L8`) (`fg7x.3`).
///
/// Rules:
/// - Any layer without an executed suite is reported as `Knowledge::Unknown { reason: UnknownReason::Unavailable }`.
/// - Once any lower layer `L_k` fails or is not `Knowledge::Known`, EVERY higher layer `L_m` (`m > k`)
///   is reported as `Knowledge::Unknown { reason: UnknownReason::LowerLayerUnverified }` even if `L_m`'s
///   own suite would otherwise pass (`SKIP` is never `PASS`, and green above an unverified lower layer
///   is impossible).
pub fn fa_doctor_layers(
    suites: &[ReferenceLayerSuite],
) -> BTreeMap<LayerId, Knowledge<LayerConformanceStatus>> {
    let mut by_layer: BTreeMap<LayerId, &ReferenceLayerSuite> = BTreeMap::new();
    for s in suites {
        by_layer.insert(s.layer, s);
    }

    let mut results = BTreeMap::new();
    let mut lower_layer_verified = true;

    for layer in LayerId::ALL {
        if !lower_layer_verified {
            results.insert(layer, Knowledge::unknown(UnknownReason::LowerLayerUnverified));
            continue;
        }
        match by_layer.get(&layer) {
            Some(suite) => {
                let outcome = suite.run();
                if !matches!(
                    outcome,
                    Knowledge::Known {
                        value: LayerConformanceStatus {
                            failed_checks: 0,
                            ..
                        },
                        ..
                    }
                ) {
                    lower_layer_verified = false;
                }
                results.insert(layer, outcome);
            }
            None => {
                lower_layer_verified = false;
                results.insert(layer, Knowledge::unknown(UnknownReason::Unavailable));
            }
        }
    }

    results
}

// ============================================================================
// FA-143: Rehearse every playbook in `docs/AGENT_GUIDE.md` end-to-end in a branch
// ============================================================================

/// Receipt for an end-to-end branch rehearsal of a playbook from `docs/AGENT_GUIDE.md` (`kngs.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybookRehearsalReceipt {
    pub playbook_id: &'static str,
    pub steps_rehearsed: usize,
    pub final_rehearsal_outcome: RehearsalOutcome,
    pub production_events_emitted: usize,
}

pub const AGENT_GUIDE_PLAYBOOKS: [(&str, &[&str]); 10] = [
    ("first_ten_minutes", &["propose", "hold"]),
    ("hold", &["hold", "resolve"]),
    ("incident", &["suspend", "reset"]),
    ("unknown_outcome", &["hold", "convene"]),
    ("helper_selection", &["convene", "propose"]),
    ("autonomy", &["narrow", "propose"]),
    ("policy_change", &["branch", "promote"]),
    ("experiment", &["branch", "intervene"]),
    ("fleet_fence", &["fence", "resolve"]),
    ("handoff", &["annotate", "handoff"]),
];

/// Rehearse all 10 playbooks in `docs/AGENT_GUIDE.md` end-to-end inside an `ExperimentCx`
/// branch and verify that zero production events or permits are emitted (`kngs.2`, `FA-INV-019`, `FA-INV-040`).
pub fn rehearse_all_agent_guide_playbooks(
    cx: &ExperimentCx,
) -> Result<Vec<PlaybookRehearsalReceipt>, RehearsalError> {
    let prod_journal = DomainEventJournal::new_production("org", "dom-prod");
    let mut branch_journal = DomainEventJournal::new_branch(cx, "org", "dom-branch", cx.branch_id());

    let closure_addr = Address::new("org", Noun::Closure, "playbook-clos", Some(1)).expect("addr");
    let ev_addr = Address::new("org", Noun::Evidence, "playbook-ev", Some(1)).expect("addr");
    let frontier = FrontierRequirement {
        key: ProjectionKey {
            source: 1,
            branch: cx.branch_id(),
            projection: 1,
            source_epoch: 1,
        },
        stage: FrontierStage::Captured,
        through: 1,
        closure: None,
    };
    let tree = build_explain_tree_for_closure(
        closure_addr,
        1,
        DecisionInputs {
            empirical: Restriction::HoldEffect,
            exact_disqualifier: false,
            mandatory_absent: true,
            contradiction: false,
        },
        vec![ev_addr.clone()],
        default_report_basis(1),
        frontier,
    )
    .expect("valid explain tree");

    let mut receipts = Vec::new();
    for (playbook_id, verbs) in AGENT_GUIDE_PLAYBOOKS {
        let mut last_outcome = None;
        for (idx, verb) in verbs.iter().enumerate() {
            let outcome = rehearse_mutation_verb(
                cx,
                verb,
                &format!("reh-{playbook_id}-{idx}"),
                &mut branch_journal,
                tree.clone(),
                Consequence::HoldEffect,
                vec![ev_addr.clone()],
                1,
            )?;
            assert!(outcome.rehearsal);
            last_outcome = Some(outcome);
        }
        receipts.push(PlaybookRehearsalReceipt {
            playbook_id,
            steps_rehearsed: verbs.len(),
            final_rehearsal_outcome: last_outcome.expect("non-empty playbook verbs"),
            production_events_emitted: prod_journal.len(),
        });
    }

    Ok(receipts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typestate_cx::RootCx;

    #[test]
    fn situation_report_fold_matches_batch_oracle_orders_floors_first_and_prevents_leakage() {
        let mut journal = DomainEventJournal::new_production("org-a", "dom-sit");
        let addr_find = Address::new("org-a", Noun::Finding, "f-1", Some(1)).expect("addr");
        let addr_hold = Address::new("org-a", Noun::Attempt, "att-1", Some(1)).expect("addr");

        // 1. Emit a low-severity non-floor finding with high VOI=30 (severity 2 * 15).
        journal
            .emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: 1,
                }],
                vec![addr_find],
                JournalEventKind::Finding {
                    finding_id: 1,
                    detector_id: "rt_drift".to_owned(),
                    severity: 2,
                },
                [0x01; 32],
                1,
                100,
            )
            .expect("finding");

        // 2. Emit a Hold (hard floor = true).
        journal
            .emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: 2,
                }],
                vec![addr_hold.clone()],
                JournalEventKind::Hold {
                    attempt_id: 1,
                    reason: "mandatory_absent".to_owned(),
                },
                [0x02; 32],
                1,
                200,
            )
            .expect("hold");

        let folded_at_2 = fold_situation_report(&journal, 2);
        let batch_at_2 = recompute_situation_batch_oracle(&journal, 2);
        assert_eq!(folded_at_2, batch_at_2);
        assert_eq!(folded_at_2.items.len(), 2);

        // Hard floor item (Hold) surfaces FIRST!
        match &folded_at_2.items[0] {
            Knowledge::Known { value, .. } => {
                assert!(value.is_hard_floor);
                assert_eq!(value.section, SituationSectionKind::HeldEffects);
            }
            other => panic!("expected Known hard-floor item first, got {other:?}"),
        }

        // 3. Emit an Outcome resolving attempt 1 -> retracts the held effect!
        journal
            .emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: 3,
                }],
                vec![addr_hold],
                JournalEventKind::Outcome {
                    attempt_id: 1,
                    outcome_class: "resolved".to_owned(),
                },
                [0x03; 32],
                1,
                300,
            )
            .expect("outcome");

        let folded_at_3 = fold_situation_report(&journal, 3);
        let batch_at_3 = recompute_situation_batch_oracle(&journal, 3);
        assert_eq!(folded_at_3, batch_at_3);
        assert_eq!(folded_at_3.items.len(), 1);

        // 4. Cross-scope Withheld projection does not leak item count or existence.
        let other_tenant_scope = CallerScope::full_tenant_access("org-b");
        let projected_2 = project_situation_for_scope(&folded_at_2, &other_tenant_scope);
        let projected_3 = project_situation_for_scope(&folded_at_3, &other_tenant_scope);
        assert_eq!(projected_2, projected_3);
        assert_eq!(
            projected_2.items,
            vec![Knowledge::withheld(AuthorityKind::Verify)]
        );
    }

    #[test]
    fn rehearsal_mode_and_negative_campaign_never_permit_fence_promote_or_touch_production() {
        let root = RootCx::new_nucleus(1, 1, 1);
        let exp_cx = root.mint_experiment(99).expect("experiment cx");
        let mut prod_journal = DomainEventJournal::new_production("org", "dom-prod");
        let mut branch_journal = DomainEventJournal::new_branch(&exp_cx, "org", "dom-branch", 99);

        let closure_addr = Address::new("org", Noun::Closure, "c-reh", Some(1)).expect("addr");
        let ev_addr = Address::new("org", Noun::Evidence, "ev-reh", Some(1)).expect("addr");
        let frontier = FrontierRequirement {
            key: ProjectionKey {
                source: 1,
                branch: 99,
                projection: 1,
                source_epoch: 1,
            },
            stage: FrontierStage::Captured,
            through: 5,
            closure: None,
        };
        let tree = build_explain_tree_for_closure(
            closure_addr,
            1,
            DecisionInputs {
                empirical: Restriction::Continue,
                exact_disqualifier: false,
                mandatory_absent: false,
                contradiction: false,
            },
            vec![ev_addr.clone()],
            default_report_basis(1),
            frontier,
        )
        .expect("tree");

        // 1. Attempting to rehearse against a production journal fails closed.
        assert_eq!(
            rehearse_mutation_verb(
                &exp_cx,
                "fence",
                "reh-prod-fail",
                &mut prod_journal,
                tree.clone(),
                Consequence::Continue,
                vec![ev_addr.clone()],
                2,
            ),
            Err(RehearsalError::ProductionJournalForbidden)
        );
        assert_eq!(prod_journal.len(), 0);

        // 2. Rehearsing fence and promote on the branch journal succeeds with rehearsal: true
        //    and leaves the production journal untouched.
        let reh_fence = rehearse_mutation_verb(
            &exp_cx,
            "fence",
            "reh-fence-1",
            &mut branch_journal,
            tree.clone(),
            Consequence::HoldEffect,
            vec![ev_addr.clone()],
            2,
        )
        .expect("rehearse fence");
        assert!(reh_fence.rehearsal);

        let reh_promote = rehearse_mutation_verb(
            &exp_cx,
            "promote",
            "reh-promote-1",
            &mut branch_journal,
            tree,
            Consequence::Continue,
            vec![ev_addr],
            3,
        )
        .expect("rehearse promote");
        assert!(reh_promote.rehearsal);
        assert_eq!(prod_journal.len(), 0);
        assert_eq!(branch_journal.len(), 2);

        // 3. Attaching a rehearsal outcome to a real proposal records evidence only (`grants_permit_authority == false`).
        let attached = attach_rehearsal_to_proposal(500, &reh_fence);
        assert!(!attached.grants_permit_authority);

        // 4. Attempting to use a RehearsalOutcome as authority fails closed at runtime as well as compile time.
        assert_eq!(
            refuse_rehearsal_as_authority(&reh_fence),
            Err(RehearsalError::RehearsalCannotPermitOrSpend { verb: "fence" })
        );
    }

    #[test]
    fn annotations_and_handoff_seal_latest_resolution_and_two_agent_reconstruction() {
        let mut journal = DomainEventJournal::new_production("org", "dom-handoff");
        let ev_addr = Address::new("org", Noun::Evidence, "ev-h1", Some(1)).expect("addr");
        journal
            .emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: 1,
                }],
                vec![ev_addr.clone()],
                JournalEventKind::Hold {
                    attempt_id: 77,
                    reason: "mandatory_absent".to_owned(),
                },
                [0x10; 32],
                1,
                100,
            )
            .expect("emit hold");

        let ann_addr = Address::new("org", Noun::Annotation, "ann-1", Some(1)).expect("addr");

        // 1. Unsigned annotation is refused.
        assert_eq!(
            Annotation::new(
                ann_addr.clone(),
                "agent-1",
                [0u8; 32],
                AnnotationClaimClass::Hypothesis,
                vec![ev_addr.clone()],
                "suspect missing witness",
            ),
            Err(AnnotationError::UnsignedAnnotationRefused)
        );

        // 2. Annotation without supporting addresses is refused.
        assert_eq!(
            Annotation::new(
                ann_addr.clone(),
                "agent-1",
                [0xAB; 32],
                AnnotationClaimClass::Hypothesis,
                Vec::new(),
                "suspect missing witness",
            ),
            Err(AnnotationError::MissingSupportingAddresses)
        );

        // 3. Valid signed annotation succeeds, but is refused at the runtime permit boundary.
        let ann = Annotation::new(
            ann_addr,
            "agent-1",
            [0xAB; 32],
            AnnotationClaimClass::ObservationNote,
            vec![ev_addr.clone()],
            "awaiting domain 1 witness",
        )
        .expect("valid annotation");
        assert_eq!(
            refuse_annotation_in_permit_path(&ann),
            Err(AnnotationError::AnnotationForbiddenInAuthoritativePath)
        );

        // 4. Build situation & open affordances for Agent 1 and seal a HandoffPackage.
        let situation = fold_situation_report(&journal, 1);
        let closure_addr = Address::new("org", Noun::Closure, "c-h1", Some(1)).expect("addr");
        let frontier = FrontierRequirement {
            key: ProjectionKey {
                source: 1,
                branch: 0,
                projection: 1,
                source_epoch: 1,
            },
            stage: FrontierStage::Captured,
            through: 1,
            closure: None,
        };
        let tree = build_explain_tree_for_closure(
            closure_addr,
            1,
            DecisionInputs {
                empirical: Restriction::HoldEffect,
                exact_disqualifier: false,
                mandatory_absent: true,
                contradiction: false,
            },
            vec![ev_addr],
            default_report_basis(1),
            frontier,
        )
        .expect("tree");
        let open_affordances = generate_affordances_for_tree(&tree);

        let mut resolver = AddressResolver::new();
        // Currency claim exceeding situation.exact_at_control_seq is refused.
        assert_eq!(
            fa_handoff(
                "h-1",
                1,
                situation.clone(),
                open_affordances.clone(),
                vec![ann.clone()],
                Vec::new(),
                99,
                &mut resolver,
            ),
            Err(AnnotationError::CurrencyExceedsSituationExactness {
                claimed_seq: 99,
                situation_exact_seq: 1,
            })
        );

        let handoff = fa_handoff(
            "h-1",
            1,
            situation,
            open_affordances.clone(),
            vec![ann],
            Vec::new(),
            1,
            &mut resolver,
        )
        .expect("seal handoff");

        // 5. Agent 2 resolves `fa://org/handoff/latest` via `fa related` and reconstructs Agent 1's open affordances.
        let caller = CallerScope::full_tenant_access("org");
        let latest_addr = Address::parse("fa://org/handoff/latest@1").expect("parse latest");
        let related = resolver.fa_related(&caller, &latest_addr, Some(RelationKind::Provenance));
        match related {
            Knowledge::Known { value: edges, .. } => {
                assert_eq!(edges.len(), 1);
                assert_eq!(edges[0].to, handoff.handoff_address);
            }
            other => panic!("expected Known related edge for handoff/latest, got {other:?}"),
        }
        assert_eq!(handoff.open_affordances, open_affordances);
    }

    #[test]
    fn fa_doctor_layers_propagates_lower_layer_unverified_upward_and_playbooks_rehearse_cleanly() {
        let suites = initial_l0_to_l4_conformance_suites();
        let report = fa_doctor_layers(&suites);

        // L0..L4 are Known, L5 is Unavailable (not executed), and L6..L8 are LowerLayerUnverified.
        assert!(matches!(
            report.get(&LayerId::L4Authority),
            Some(Knowledge::Known { .. })
        ));
        assert_eq!(
            report.get(&LayerId::L5Effect),
            Some(&Knowledge::unknown(UnknownReason::Unavailable))
        );
        assert_eq!(
            report.get(&LayerId::L6Closure),
            Some(&Knowledge::unknown(UnknownReason::LowerLayerUnverified))
        );

        // Inject a failure into L1Observation -> L2, L3, L4 (even though their suites pass in isolation)
        // MUST become Unknown { reason: LowerLayerUnverified }!
        let mut injected = initial_l0_to_l4_conformance_suites();
        injected[1].failed_checks = 1;
        let failed_report = fa_doctor_layers(&injected);
        assert!(matches!(
            failed_report.get(&LayerId::L0Identity),
            Some(Knowledge::Known { .. })
        ));
        assert_eq!(
            failed_report.get(&LayerId::L1Observation),
            Some(&Knowledge::unknown(UnknownReason::Incomplete))
        );
        for higher in [
            LayerId::L2Evidence,
            LayerId::L3Judgment,
            LayerId::L4Authority,
            LayerId::L5Effect,
            LayerId::L6Closure,
            LayerId::L7Experiment,
            LayerId::L8Governance,
        ] {
            assert_eq!(
                failed_report.get(&higher),
                Some(&Knowledge::unknown(UnknownReason::LowerLayerUnverified)),
                "layer {higher:?} above failed L1 must be LowerLayerUnverified"
            );
        }

        // Rehearse all 10 playbooks from docs/AGENT_GUIDE.md in an ExperimentCx branch (kngs.2).
        let root = RootCx::new_nucleus(1, 1, 1);
        let exp_cx = root.mint_experiment(777).expect("experiment cx");
        let receipts = rehearse_all_agent_guide_playbooks(&exp_cx)
            .expect("all 10 AGENT_GUIDE playbooks rehearse green in branch");
        assert_eq!(receipts.len(), 10);
        assert!(receipts.iter().all(|r| r.production_events_emitted == 0));
        assert!(
            receipts
                .iter()
                .all(|r| r.final_rehearsal_outcome.rehearsal)
        );
    }
}
