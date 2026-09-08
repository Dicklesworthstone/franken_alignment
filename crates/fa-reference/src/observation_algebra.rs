//! Bounded, pure transfer laws for the observation algebra in plan §14.7.
//!
//! This module is an executable reference algebra, not a plan executor.  Its
//! authority entries are requirements, its laws are caller-declared reference
//! assumptions, and its cancellation entries are obligations for a future
//! runtime to discharge.  It performs no I/O, authentication, helper inference,
//! decoding, scheduling, or permit minting.

use std::collections::BTreeSet;

use crate::{
    Error,
    full_input::{ActualHelperInput, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, OpaqueJudgment},
    product_frontier::{
        FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    },
    witness::{DomainClosure, DomainProjection, WitnessSnapshot},
};

pub const MAX_OPERATOR_INPUTS: usize = 16;
pub const MAX_OPERATOR_OUTPUTS: usize = 16;
pub const MAX_OPERATOR_WITNESSES: usize = 32;
pub const MAX_OPERATOR_FRONTIERS: usize = 16;
pub const MAX_OPERATOR_PRIVACY_LABELS: usize = 16;
pub const MAX_OPERATOR_OBLIGATIONS: usize = 16;
pub const MAX_OPERATOR_BYTES: usize = MAX_SUBMITTED_BYTES + MAX_PROFILE_BYTES;
pub const MAX_OPERATOR_DECODED_BYTES: usize = 64 * 1024;
pub const MAX_OPERATOR_CANDIDATES: usize = 128;

/// Identity and interpretation inputs retained with every known value.  Epoch
/// zero is intentionally valid as the explicit initial epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasisIdentity {
    pub subject_id: u64,
    pub question_id: u64,
    pub profile_id: u64,
    pub semantic_generation: u64,
    pub policy_epoch: u64,
    pub model_epoch: u64,
    pub tokenizer_epoch: u64,
    pub codec_epoch: u64,
    pub control_seq: u64,
}

/// The epistemic strength of a `Knowledge::Known` claim (plan §2.4), distinct
/// from its tower layer or value role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimClass {
    Invariant,
    Proof,
    BoundedModel,
    Statistical,
    Benchmark,
    Slo,
    Hypothesis,
}

/// A scoped proof state.  `Declared` is a caller assertion to the reference
/// model, never a cryptographic or host-authentication claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopedProof {
    Declared { scope_id: u64, generation: u64 },
    Gapped { scope_id: u64, first_missing: u64 },
    Unsupported { scope_id: u64 },
    Expired { scope_id: u64, generation: u64 },
}

/// The four non-interchangeable evidence predicates from plan §7.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofStates {
    pub authentic_origin: ScopedProof,
    pub complete_for_contract: ScopedProof,
    pub semantically_valid: ScopedProof,
    pub available_for_replay: ScopedProof,
}

/// A private, complete basis constructor prevents a known result from omitting
/// any of the mandatory proof-state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Basis {
    identity: BasisIdentity,
    claim_class: ClaimClass,
    proofs: ProofStates,
}

impl Basis {
    pub const fn new(
        identity: BasisIdentity,
        claim_class: ClaimClass,
        proofs: ProofStates,
    ) -> Self {
        Self {
            identity,
            claim_class,
            proofs,
        }
    }

    pub const fn identity(self) -> BasisIdentity {
        self.identity
    }

    pub const fn claim_class(self) -> ClaimClass {
        self.claim_class
    }

    pub const fn proofs(self) -> ProofStates {
        self.proofs
    }

    pub const fn profile_id(self) -> u64 {
        self.identity.profile_id
    }

    pub const fn question_id(self) -> u64 {
        self.identity.question_id
    }

    pub const fn semantic_generation(self) -> u64 {
        self.identity.semantic_generation
    }

    pub const fn codec_epoch(self) -> u64 {
        self.identity.codec_epoch
    }

    pub const fn policy_epoch(self) -> u64 {
        self.identity.policy_epoch
    }

    pub const fn model_epoch(self) -> u64 {
        self.identity.model_epoch
    }

    pub const fn tokenizer_epoch(self) -> u64 {
        self.identity.tokenizer_epoch
    }

    pub const fn control_seq(self) -> u64 {
        self.identity.control_seq
    }
}

/// A named, complete domain established by `AwaitClosedScope`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedDomain {
    domain: DomainProjection,
    revision: u64,
    control_cut: u64,
    semantic_epoch: u64,
    requirement: FrontierRequirement,
    marker: TrustedClosingMarker,
}

impl ClosedDomain {
    fn new(
        domain: DomainProjection,
        revision: u64,
        control_cut: u64,
        semantic_epoch: u64,
        requirement: FrontierRequirement,
        marker: TrustedClosingMarker,
    ) -> Result<Self, Error> {
        if requirement.key != domain.projection()
            || requirement.stage != FrontierStage::Authenticated
            || requirement.through != marker.final_sequence
            || requirement.closure != Some(marker.marker_generation)
            || marker.key != domain.projection()
            || marker.marker_generation == 0
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            domain,
            revision,
            control_cut,
            semantic_epoch,
            requirement,
            marker,
        })
    }

    pub fn domain_id(&self) -> u64 {
        self.domain.domain_id()
    }

    pub fn domain(&self) -> DomainProjection {
        self.domain
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn control_cut(&self) -> u64 {
        self.control_cut
    }

    pub fn semantic_epoch(&self) -> u64 {
        self.semantic_epoch
    }

    pub fn requirement(&self) -> FrontierRequirement {
        self.requirement
    }

    /// This is caller-supplied reference input, not an authentication proof.
    pub fn marker(&self) -> TrustedClosingMarker {
        self.marker
    }
}

/// The sole epistemic wrapper permitted for reference-algebra outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Knowledge<T> {
    Known {
        value: T,
        basis: Basis,
    },
    Pending {
        frontiers: Vec<FrontierRequirement>,
        expected_cost: u64,
    },
    Unknown {
        reason: UnknownReason,
    },
    Withheld {
        authority_required: AuthorityKind,
    },
    Stale {
        basis: Basis,
        current_generation: u64,
    },
    Absent {
        domain: ClosedDomain,
    },
}

impl<T> Knowledge<T> {
    pub fn known(value: T, basis: Basis) -> Self {
        Self::Known { value, basis }
    }

    pub fn pending(frontiers: Vec<FrontierRequirement>, expected_cost: u64) -> Result<Self, Error> {
        validate_frontiers(&frontiers)?;
        Ok(Self::Pending {
            frontiers,
            expected_cost,
        })
    }

    pub fn unknown(reason: UnknownReason) -> Self {
        Self::Unknown { reason }
    }

    pub fn withheld(authority_required: AuthorityKind) -> Self {
        Self::Withheld { authority_required }
    }

    pub fn stale(basis: Basis, current_generation: u64) -> Self {
        Self::Stale {
            basis,
            current_generation,
        }
    }

    pub fn absent(domain: ClosedDomain) -> Self {
        Self::Absent { domain }
    }

    pub fn basis(&self) -> Option<Basis> {
        match self {
            Self::Known { basis, .. } | Self::Stale { basis, .. } => Some(*basis),
            Self::Pending { .. }
            | Self::Unknown { .. }
            | Self::Withheld { .. }
            | Self::Absent { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    Cancelled,
    Incomplete,
    IncompatibleGeneration,
    MissingExactDomain,
    LowerLayerUnverified,
    ConflictingEvidence,
    Unavailable,
}

/// Requirement tags are descriptive and are checked by the concrete operator
/// functions below; they are never authority capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequirementClass {
    Capture,
    Projection,
    Prefix,
    ExactObject,
    ExactLookup,
    Universal,
    ApproximateDiscovery,
    Refine,
    ExactCheckpoint,
    ApproximateValue,
    Probe,
    IndependentJudgment,
    ClosedScope,
    JoinedEvidence,
    EmittedJudgment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UncertaintyClass {
    Exact,
    Conservative,
    Approximate { profile_id: u64 },
    Opaque { calibration_id: u64 },
    Bounded { bound_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationRequirement {
    pub class: RequirementClass,
    pub uncertainty: UncertaintyClass,
    pub basis: Basis,
}

/// A requirement only names a required authority class.  It cannot convey a
/// permit, credential, or any right to dispatch an effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthorityKind {
    Observe,
    Project,
    Verify,
    ExactRead,
    Discover,
    Probe,
    Judge,
    CloseScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityRequirements {
    kinds: Vec<AuthorityKind>,
}

impl AuthorityRequirements {
    pub fn new(mut kinds: Vec<AuthorityKind>) -> Result<Self, Error> {
        if kinds.len() > MAX_OPERATOR_INPUTS {
            return Err(Error::Limit);
        }
        kinds.sort_unstable();
        kinds.dedup();
        Ok(Self { kinds })
    }

    pub fn kinds(&self) -> &[AuthorityKind] {
        &self.kinds
    }

    fn union(&self, other: &Self) -> Result<Self, Error> {
        if self.kinds.len().saturating_add(other.kinds.len()) > MAX_OPERATOR_INPUTS {
            return Err(Error::Limit);
        }
        let mut kinds = self.kinds.clone();
        kinds.extend_from_slice(&other.kinds);
        Self::new(kinds)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrivacyLabel {
    pub purpose_id: u64,
    pub transform_id: u64,
}

/// A privacy restriction is a set, not a numeric clearance.  Transfer only
/// unions restrictions; this module has no declassification operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyRestrictions {
    labels: Vec<PrivacyLabel>,
}

impl PrivacyRestrictions {
    pub fn new(labels: Vec<PrivacyLabel>) -> Result<Self, Error> {
        let labels = canonical_labels(labels)?;
        Ok(Self { labels })
    }

    pub fn labels(&self) -> &[PrivacyLabel] {
        &self.labels
    }

    fn union(&self, other: &Self) -> Result<Self, Error> {
        let mut labels = self.labels.clone();
        for label in &other.labels {
            if !labels.contains(label) {
                if labels.len() >= MAX_OPERATOR_PRIVACY_LABELS {
                    return Err(Error::Limit);
                }
                labels.push(*label);
            }
        }
        Self::new(labels)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObligationKind {
    RetainInput,
    RetainWitness,
    PreserveFrontier,
    ResolveBeforeRelease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Obligation {
    pub id: u64,
    pub kind: ObligationKind,
}

/// Declared cancellation work.  Retaining this value is not execution of the
/// obligation, and cancellation can never erase it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancellationObligations {
    entries: Vec<Obligation>,
}

impl CancellationObligations {
    pub fn new(entries: Vec<Obligation>) -> Result<Self, Error> {
        if entries.len() > MAX_OPERATOR_OBLIGATIONS {
            return Err(Error::Limit);
        }
        if entries.iter().any(|entry| entry.id == 0) {
            return Err(Error::InvalidInput);
        }
        let mut ids = BTreeSet::new();
        if entries.iter().any(|entry| !ids.insert(entry.id)) {
            return Err(Error::Duplicate);
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[Obligation] {
        &self.entries
    }

    fn union(&self, other: &Self) -> Result<Self, Error> {
        let mut entries = self.entries.clone();
        for candidate in &other.entries {
            match entries.iter().find(|existing| existing.id == candidate.id) {
                Some(existing) if existing.kind == candidate.kind => {}
                Some(_) => return Err(Error::Binding),
                None => entries.push(*candidate),
            }
        }
        if entries.len() > MAX_OPERATOR_OBLIGATIONS {
            return Err(Error::Limit);
        }
        Self::new(entries)
    }
}

/// Per-node reference limits.  Constructors reject values beyond the fixed
/// reference profile, and each transfer checks its relevant limit first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceEnvelope {
    max_inputs: usize,
    max_outputs: usize,
    max_witnesses: usize,
    max_frontiers: usize,
    max_bytes: usize,
    max_decoded_bytes: usize,
    max_candidates: usize,
}

impl ResourceEnvelope {
    pub fn new(
        max_inputs: usize,
        max_outputs: usize,
        max_witnesses: usize,
        max_frontiers: usize,
        max_bytes: usize,
        max_decoded_bytes: usize,
        max_candidates: usize,
    ) -> Result<Self, Error> {
        if max_inputs == 0
            || max_outputs == 0
            || max_witnesses == 0
            || max_frontiers == 0
            || max_bytes == 0
            || max_decoded_bytes == 0
            || max_candidates == 0
            || max_inputs > MAX_OPERATOR_INPUTS
            || max_outputs > MAX_OPERATOR_OUTPUTS
            || max_witnesses > MAX_OPERATOR_WITNESSES
            || max_frontiers > MAX_OPERATOR_FRONTIERS
            || max_bytes > MAX_OPERATOR_BYTES
            || max_decoded_bytes > MAX_OPERATOR_DECODED_BYTES
            || max_candidates > MAX_OPERATOR_CANDIDATES
        {
            return Err(Error::Limit);
        }
        Ok(Self {
            max_inputs,
            max_outputs,
            max_witnesses,
            max_frontiers,
            max_bytes,
            max_decoded_bytes,
            max_candidates,
        })
    }

    pub const fn max_inputs(&self) -> usize {
        self.max_inputs
    }
    pub const fn max_outputs(&self) -> usize {
        self.max_outputs
    }
    pub const fn max_witnesses(&self) -> usize {
        self.max_witnesses
    }
    pub const fn max_frontiers(&self) -> usize {
        self.max_frontiers
    }
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }
    pub const fn max_decoded_bytes(&self) -> usize {
        self.max_decoded_bytes
    }
    pub const fn max_candidates(&self) -> usize {
        self.max_candidates
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadWitness {
    /// Exact captured source bytes retained across cancellation and later
    /// transfers.  The bytes share storage with `CapturedObservation`.
    CapturedBytes {
        source_id: u64,
        source_generation: u64,
        profile_id: u64,
        bytes: std::sync::Arc<[u8]>,
    },
    ExactObject {
        object_id: u64,
        generation: u64,
        profile_id: u64,
    },
    ExactValue {
        key: u64,
        version: u64,
        value: Vec<u8>,
        role_id: u64,
    },
    AbsentKey {
        key: u64,
        domain: ClosedDomain,
    },
    WitnessSnapshot {
        domain: DomainProjection,
        revision: u64,
        control_cut: u64,
        semantic_epoch: u64,
    },
    ClosedDomain(ClosedDomain),
    Derived {
        law_id: u64,
        base_generation: u64,
        profile_id: u64,
    },
    /// Exact encoded bytes consumed by `Decode`.
    EncodedBytes {
        object_id: u64,
        profile_id: u64,
        codec_epoch: u64,
        bytes: Vec<u8>,
    },
    /// Exact decoded bytes produced by `Decode`.  The `Arc` is shared with the
    /// decoded value; it is still one retained byte witness, not a digest-only
    /// stand-in.
    DecodedBytes {
        object_id: u64,
        profile_id: u64,
        codec_epoch: u64,
        bytes: std::sync::Arc<[u8]>,
    },
    Epoch(Basis),
    Frontier(FrontierRequirement),
    FullInput(ActualHelperInput),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessSet {
    entries: Vec<ReadWitness>,
}

impl WitnessSet {
    pub fn new(entries: Vec<ReadWitness>, bounds: ResourceEnvelope) -> Result<Self, Error> {
        validate_witnesses(&entries, bounds)?;
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[ReadWitness] {
        &self.entries
    }

    fn union(&self, other: &Self, bounds: ResourceEnvelope) -> Result<Self, Error> {
        if self.entries.len().saturating_add(other.entries.len()) > bounds.max_witnesses {
            return Err(Error::Limit);
        }
        let bytes = witness_bytes(&self.entries)?
            .checked_add(witness_bytes(&other.entries)?)
            .ok_or(Error::Overflow)?;
        if bytes > bounds.max_bytes {
            return Err(Error::Limit);
        }
        let mut entries = self.entries.clone();
        entries.extend_from_slice(&other.entries);
        Ok(Self { entries })
    }

    fn union_owned(&self, other: Self, bounds: ResourceEnvelope) -> Result<Self, Error> {
        if self.entries.len().saturating_add(other.entries.len()) > bounds.max_witnesses {
            return Err(Error::Limit);
        }
        let bytes = witness_bytes(&self.entries)?
            .checked_add(witness_bytes(&other.entries)?)
            .ok_or(Error::Overflow)?;
        if bytes > bounds.max_bytes {
            return Err(Error::Limit);
        }
        let mut entries = self.entries.clone();
        entries.extend(other.entries);
        Ok(Self { entries })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawDeclaration {
    pub law_id: u64,
    pub input_generation: u64,
    pub output_generation: u64,
    pub profile_id: u64,
}

impl LawDeclaration {
    pub fn new(
        law_id: u64,
        input_generation: u64,
        output_generation: u64,
        profile_id: u64,
    ) -> Result<Self, Error> {
        if law_id == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            law_id,
            input_generation,
            output_generation,
            profile_id,
        })
    }
}

/// Context proposed for a successful transfer.  Its cancellation entries are
/// newly admitted obligations; already-active obligations remain on the
/// borrowed input transfer and are unioned only when construction succeeds.
/// This is a pure reference boundary, not a runtime liability/discharge claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorContext {
    requirement: ObservationRequirement,
    privacy: PrivacyRestrictions,
    cancellation: CancellationObligations,
    resources: ResourceEnvelope,
}

impl OperatorContext {
    pub fn new(
        requirement: ObservationRequirement,
        privacy: PrivacyRestrictions,
        cancellation: CancellationObligations,
        resources: ResourceEnvelope,
    ) -> Self {
        Self {
            requirement,
            privacy,
            cancellation,
            resources,
        }
    }

    pub fn requirement(&self) -> ObservationRequirement {
        self.requirement
    }

    pub fn resources(&self) -> ResourceEnvelope {
        self.resources
    }
}

/// A completed pure transfer or a cancellation that preserves its obligations.
///
/// Transform operators borrow this value.  A refused pure reference transfer
/// therefore leaves the caller's retained witnesses and cancellation work
/// available; this type does not model a runtime's durable discharge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer<T> {
    origin_basis: Basis,
    uncertainty: UncertaintyClass,
    value: Knowledge<T>,
    witnesses: WitnessSet,
    frontiers: Vec<FrontierRequirement>,
    privacy: PrivacyRestrictions,
    authority: AuthorityRequirements,
    cancellation: CancellationObligations,
    resources: ResourceEnvelope,
    cancelled: bool,
}

impl<T> Transfer<T> {
    pub fn origin_basis(&self) -> Basis {
        self.origin_basis
    }

    pub fn uncertainty(&self) -> UncertaintyClass {
        self.uncertainty
    }

    pub fn value(&self) -> &Knowledge<T> {
        &self.value
    }

    pub fn witnesses(&self) -> &WitnessSet {
        &self.witnesses
    }

    pub fn frontiers(&self) -> &[FrontierRequirement] {
        &self.frontiers
    }

    pub fn privacy(&self) -> &PrivacyRestrictions {
        &self.privacy
    }

    pub fn authority(&self) -> &AuthorityRequirements {
        &self.authority
    }

    pub fn cancellation(&self) -> &CancellationObligations {
        &self.cancellation
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn resources(&self) -> ResourceEnvelope {
        self.resources
    }

    /// Cancelling a reference transfer never discards its declared obligation.
    /// Existing `Pending` and `Unknown` states retain their exact state.
    pub fn cancel(mut self) -> Self {
        self.value = match self.value {
            Knowledge::Pending {
                frontiers,
                expected_cost,
            } => Knowledge::Pending {
                frontiers,
                expected_cost,
            },
            Knowledge::Unknown { reason } => Knowledge::Unknown { reason },
            Knowledge::Known { .. }
            | Knowledge::Withheld { .. }
            | Knowledge::Stale { .. }
            | Knowledge::Absent { .. } => Knowledge::Unknown {
                reason: UnknownReason::Cancelled,
            },
        };
        self.cancelled = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedInput {
    source_id: u64,
    source_generation: u64,
    bytes: Vec<u8>,
}

impl CapturedInput {
    pub fn new(source_id: u64, source_generation: u64, bytes: Vec<u8>) -> Result<Self, Error> {
        if source_id == 0 {
            return Err(Error::InvalidInput);
        }
        if bytes.len() > MAX_OPERATOR_BYTES {
            return Err(Error::Limit);
        }
        Ok(Self {
            source_id,
            source_generation,
            bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedObservation {
    source_id: u64,
    source_generation: u64,
    bytes: std::sync::Arc<[u8]>,
}

impl CapturedObservation {
    pub fn source_id(&self) -> u64 {
        self.source_id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn source_generation(&self) -> u64 {
        self.source_generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionSpec {
    pub key: ProjectionKey,
    pub projection_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedProjection {
    observation: CapturedObservation,
    spec: ProjectionSpec,
}

impl AuthorizedProjection {
    pub fn observation(&self) -> &CapturedObservation {
        &self.observation
    }

    pub fn key(&self) -> ProjectionKey {
        self.spec.key
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixObservation {
    projection: AuthorizedProjection,
    requirement: FrontierRequirement,
}

impl PrefixObservation {
    pub fn projection(&self) -> &AuthorizedProjection {
        &self.projection
    }

    pub fn requirement(&self) -> FrontierRequirement {
        self.requirement
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectDeclaration {
    pub object_id: u64,
    pub generation: u64,
    pub profile_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedObject {
    declaration: ObjectDeclaration,
    verifier_law: LawDeclaration,
}

impl VerifiedObject {
    pub fn declaration(&self) -> ObjectDeclaration {
        self.declaration
    }

    pub fn verifier_law(&self) -> LawDeclaration {
        self.verifier_law
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactValue {
    key: u64,
    value: Vec<u8>,
    generation: u64,
}

impl ExactValue {
    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimilarCandidates {
    query_id: u64,
    profile_id: u64,
    candidates: Vec<u64>,
    residual_profile_id: u64,
}

impl SimilarCandidates {
    pub fn query_id(&self) -> u64 {
        self.query_id
    }

    pub fn profile_id(&self) -> u64 {
        self.profile_id
    }

    pub fn candidates(&self) -> &[u64] {
        &self.candidates
    }

    pub fn residual_profile_id(&self) -> u64 {
        self.residual_profile_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefinedWitness {
    parent: WitnessSet,
    refined: WitnessSet,
    law: LawDeclaration,
}

impl RefinedWitness {
    pub fn parent(&self) -> &WitnessSet {
        &self.parent
    }

    pub fn refined(&self) -> &WitnessSet {
        &self.refined
    }

    pub fn law(&self) -> LawDeclaration {
        self.law
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedObject {
    pub object_id: u64,
    pub bytes: Vec<u8>,
    pub profile_id: u64,
    pub codec_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedObject {
    object_id: u64,
    bytes: std::sync::Arc<[u8]>,
    profile_id: u64,
    codec_epoch: u64,
    restart_grade: RestartGrade,
}

impl DecodedObject {
    pub fn object_id(&self) -> u64 {
        self.object_id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn restart_grade(&self) -> RestartGrade {
        self.restart_grade
    }

    pub fn profile_id(&self) -> u64 {
        self.profile_id
    }

    pub fn codec_epoch(&self) -> u64 {
        self.codec_epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartGrade {
    Exact,
    Approximate { error_bound_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeRequirement {
    ExactCheckpoint,
    ApproximateValue { profile_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeLaw {
    /// Caller-declared registered equivalence; arbitrary byte equality is not
    /// accepted as a substitute for this declaration.
    RegisteredLossless(LawDeclaration),
    Lossy {
        profile_id: u64,
        error_bound_id: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeDecision {
    CertifiedPositive,
    CertifiedNegative,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeResult {
    probe_id: u64,
    decision: ProbeDecision,
    bound_id: u64,
}

impl ProbeResult {
    pub fn probe_id(&self) -> u64 {
        self.probe_id
    }

    pub fn decision(&self) -> ProbeDecision {
        self.decision
    }

    pub fn bound_id(&self) -> u64 {
        self.bound_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndependentJudgment {
    judgment: OpaqueJudgment,
}

impl IndependentJudgment {
    pub fn judgment(&self) -> &OpaqueJudgment {
        &self.judgment
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinedEvidence {
    member_count: usize,
}

impl JoinedEvidence {
    pub fn member_count(&self) -> usize {
        self.member_count
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedScope {
    domain: ClosedDomain,
}

impl ClosedScope {
    pub fn domain(&self) -> &ClosedDomain {
        &self.domain
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedJudgment {
    basis: Basis,
    evidence_members: usize,
}

impl EmittedJudgment {
    pub fn basis(&self) -> Basis {
        self.basis
    }

    pub fn evidence_members(&self) -> usize {
        self.evidence_members
    }
}

/// `Tap`: records bounded supplied bytes and their capture identity.
pub fn tap(
    input: CapturedInput,
    context: OperatorContext,
) -> Result<Transfer<CapturedObservation>, Error> {
    require_class(&context, RequirementClass::Capture)?;
    require_exact(&context)?;
    if input.bytes.len() > context.resources.max_bytes {
        return Err(Error::Limit);
    }
    let bytes = std::sync::Arc::<[u8]>::from(input.bytes);
    let witness = WitnessSet::new(
        vec![ReadWitness::CapturedBytes {
            source_id: input.source_id,
            source_generation: input.source_generation,
            profile_id: context.requirement.basis.profile_id(),
            bytes: bytes.clone(),
        }],
        context.resources,
    )?;
    completed(
        Knowledge::known(
            CapturedObservation {
                source_id: input.source_id,
                source_generation: input.source_generation,
                bytes,
            },
            context.requirement.basis,
        ),
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Observe])?,
    )
}

/// `AuthorizeProjection`: binds a captured observation to one named view.
pub fn authorize_projection(
    input: &Transfer<CapturedObservation>,
    spec: ProjectionSpec,
    context: OperatorContext,
) -> Result<Transfer<AuthorizedProjection>, Error> {
    require_class(&context, RequirementClass::Projection)?;
    require_exact(&context)?;
    if spec.projection_id == 0
        || input.value_as_known().is_some_and(|observation| {
            spec.key.source != observation.source_id()
                || spec.key.projection != spec.projection_id
                || spec.key.source_epoch != observation.source_generation()
        })
    {
        return Err(Error::Binding);
    }
    ensure_context_matches(input, &context)?;
    let witness = append_witness(
        input,
        ReadWitness::Epoch(context.requirement.basis),
        context.resources,
    )?;
    let value = map_knowledge(&input.value, |observation| AuthorizedProjection {
        observation: observation.clone(),
        spec,
    });
    derived(
        input,
        value,
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Project])?,
    )
}

/// `RequirePrefix`: turns a named contiguous frontier into positive prefix
/// evidence.  A missing member remains pending.
pub fn require_prefix(
    input: &Transfer<AuthorizedProjection>,
    frontiers: &ProductFrontiers,
    requirement: FrontierRequirement,
    context: OperatorContext,
) -> Result<Transfer<PrefixObservation>, Error> {
    require_class(&context, RequirementClass::Prefix)?;
    require_exact(&context)?;
    if requirement.key
        != input
            .value_as_known()
            .map(|view| view.key())
            .unwrap_or(requirement.key)
    {
        return Err(Error::Binding);
    }
    ensure_context_matches(input, &context)?;
    let witness = append_witness(input, ReadWitness::Frontier(requirement), context.resources)?;
    let holds = frontiers.satisfies(requirement)?;
    let value = match &input.value {
        Knowledge::Known { .. } if !holds => Knowledge::pending(vec![requirement], 1)?,
        _ => map_knowledge(&input.value, |projection| PrefixObservation {
            projection: projection.clone(),
            requirement,
        }),
    };
    derived(
        input,
        value,
        witness,
        vec![requirement],
        context,
        authority(&[AuthorityKind::Observe])?,
    )
}

/// `VerifyObject`: applies a caller-declared registered verifier law to an
/// exact object identity.  It does not claim cryptographic verification.
pub fn verify_object(
    input: &Transfer<PrefixObservation>,
    declaration: ObjectDeclaration,
    verifier_law: LawDeclaration,
    context: OperatorContext,
) -> Result<Transfer<VerifiedObject>, Error> {
    require_class(&context, RequirementClass::ExactObject)?;
    require_exact(&context)?;
    ensure_context_matches(input, &context)?;
    if declaration.object_id == 0
        || declaration.profile_id != context.requirement.basis.profile_id()
        || verifier_law.profile_id != declaration.profile_id
        || verifier_law.input_generation != declaration.generation
        || verifier_law.output_generation != context.requirement.basis.semantic_generation()
        || input.value_as_known().is_some_and(|prefix| {
            let captured = prefix.projection().observation();
            declaration.object_id != captured.source_id()
                || declaration.generation != captured.source_generation()
        })
    {
        return Err(Error::Binding);
    }
    let witness = append_witness(
        input,
        ReadWitness::ExactObject {
            object_id: declaration.object_id,
            generation: declaration.generation,
            profile_id: declaration.profile_id,
        },
        context.resources,
    )?;
    let value = map_knowledge(&input.value, |_| VerifiedObject {
        declaration,
        verifier_law,
    });
    derived(
        input,
        value,
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Verify])?,
    )
}

/// `LookupExact`: yields an exact value from a typed witness snapshot, or
/// `Absent` only by using a same-domain, same-cut exact `ClosedScope`.
pub fn lookup_exact(
    snapshot: &WitnessSnapshot,
    key: u64,
    closed_scope: Option<&Transfer<ClosedScope>>,
    context: OperatorContext,
) -> Result<Transfer<ExactValue>, Error> {
    require_class(&context, RequirementClass::ExactLookup)?;
    require_exact(&context)?;
    if snapshot.semantic_epoch() != context.requirement.basis.semantic_generation()
        || snapshot.control_cut() != context.requirement.basis.control_seq()
    {
        return completed(
            Knowledge::stale(context.requirement.basis, snapshot.semantic_epoch()),
            WitnessSet::new(Vec::new(), context.resources)?,
            Vec::new(),
            context,
            authority(&[AuthorityKind::ExactRead])?,
        );
    }
    let snapshot_witness = ReadWitness::WitnessSnapshot {
        domain: snapshot.domain_input().domain(),
        revision: snapshot.revision(),
        control_cut: snapshot.control_cut(),
        semantic_epoch: snapshot.semantic_epoch(),
    };
    let mut entries = vec![snapshot_witness];
    let value = match snapshot.entry(key) {
        Some(entry) => {
            if entry.value().len() > context.resources.max_bytes {
                return Err(Error::Limit);
            }
            entries.push(ReadWitness::ExactValue {
                key,
                version: entry.version(),
                value: entry.value().to_vec(),
                role_id: context.requirement.basis.question_id(),
            });
            Knowledge::known(
                ExactValue {
                    key,
                    value: entry.value().to_vec(),
                    generation: snapshot.semantic_epoch(),
                },
                context.requirement.basis,
            )
        }
        None => match closed_scope {
            Some(scope) => return lookup_absent_from_scope(snapshot, key, scope, entries, context),
            None => Knowledge::unknown(UnknownReason::MissingExactDomain),
        },
    };
    completed(
        value,
        WitnessSet::new(entries, context.resources)?,
        Vec::new(),
        context,
        authority(&[AuthorityKind::ExactRead])?,
    )
}

fn lookup_absent_from_scope(
    snapshot: &WitnessSnapshot,
    key: u64,
    scope: &Transfer<ClosedScope>,
    mut entries: Vec<ReadWitness>,
    context: OperatorContext,
) -> Result<Transfer<ExactValue>, Error> {
    if scope.uncertainty != UncertaintyClass::Exact {
        return Err(Error::Binding);
    }
    ensure_context_matches(scope, &context)?;
    let value = match scope.value() {
        Knowledge::Known { value, basis }
            if *basis == context.requirement.basis
                && value.domain.domain() == snapshot.domain_input().domain()
                && value.domain.revision() == snapshot.revision()
                && value.domain.control_cut() == snapshot.control_cut()
                && value.domain.semantic_epoch() == snapshot.semantic_epoch()
                && matches!(snapshot.domain_input().closure(), DomainClosure::Closed(marker) if marker == value.domain.marker()) =>
        {
            entries.push(ReadWitness::AbsentKey {
                key,
                domain: value.domain.clone(),
            });
            Knowledge::absent(value.domain.clone())
        }
        Knowledge::Known { .. } => Knowledge::unknown(UnknownReason::IncompatibleGeneration),
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => Knowledge::pending(frontiers.clone(), *expected_cost)?,
        Knowledge::Unknown { reason } => Knowledge::unknown(*reason),
        Knowledge::Withheld { authority_required } => Knowledge::withheld(*authority_required),
        Knowledge::Stale {
            basis,
            current_generation,
        } => Knowledge::stale(*basis, *current_generation),
        Knowledge::Absent { .. } => Knowledge::unknown(UnknownReason::Incomplete),
    };
    let own_witnesses = WitnessSet::new(entries, context.resources)?;
    let witnesses = scope.witnesses.union(&own_witnesses, context.resources)?;
    let privacy = scope.privacy.union(&context.privacy)?;
    let authority = scope
        .authority
        .union(&authority(&[AuthorityKind::ExactRead])?)?;
    let cancellation = scope.cancellation.union(&context.cancellation)?;
    Transfer {
        origin_basis: scope.origin_basis,
        uncertainty: context.requirement.uncertainty,
        value,
        witnesses,
        frontiers: scope.frontiers.clone(),
        privacy,
        authority,
        cancellation,
        resources: context.resources,
        cancelled: scope.cancelled,
    }
    .checked()
}

/// `DiscoverSimilar`: produces only a bounded candidate set; no candidate
/// result, including an empty one, is an absence or universal-proof result.
pub fn discover_similar(
    input: &Transfer<VerifiedObject>,
    query_id: u64,
    residual_profile_id: u64,
    candidates: Vec<u64>,
    context: OperatorContext,
) -> Result<Transfer<SimilarCandidates>, Error> {
    require_class(&context, RequirementClass::ApproximateDiscovery)?;
    ensure_context_matches(input, &context)?;
    if !matches!(context.requirement.uncertainty, UncertaintyClass::Approximate { profile_id } if profile_id == residual_profile_id)
        || query_id == 0
        || residual_profile_id == 0
        || candidates.len() > context.resources.max_candidates
    {
        return Err(Error::Limit);
    }
    let witness = append_witness(
        input,
        ReadWitness::Derived {
            law_id: residual_profile_id,
            base_generation: context.requirement.basis.semantic_generation(),
            profile_id: context.requirement.basis.profile_id(),
        },
        context.resources,
    )?;
    let value = map_knowledge(&input.value, |_| SimilarCandidates {
        query_id,
        profile_id: context.requirement.basis.profile_id(),
        candidates,
        residual_profile_id,
    });
    derived(
        input,
        value,
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Discover])?,
    )
}

/// `RefineWitness`: accepts only a caller-declared registered deterministic
/// law.  It has no path from an opaque explanation to a narrower witness.
pub fn refine_witness(
    parent: WitnessSet,
    refined: WitnessSet,
    law: LawDeclaration,
    context: OperatorContext,
) -> Result<Transfer<RefinedWitness>, Error> {
    require_class(&context, RequirementClass::Refine)?;
    require_refinable(&context)?;
    if law.profile_id != context.requirement.basis.profile_id()
        || law.input_generation != context.requirement.basis.semantic_generation()
        || law.output_generation != context.requirement.basis.semantic_generation()
    {
        return Err(Error::Binding);
    }
    let witnesses = parent.union(&refined, context.resources)?;
    completed(
        Knowledge::known(
            RefinedWitness {
                parent,
                refined,
                law,
            },
            context.requirement.basis,
        ),
        witnesses,
        Vec::new(),
        context,
        AuthorityRequirements::new(Vec::new())?,
    )
}

/// `Decode`: an exact checkpoint accepts only an explicit registered lossless
/// law.  Matching arbitrary encoded and decoded bytes is deliberately useless.
pub fn decode(
    input: &Transfer<VerifiedObject>,
    encoded: EncodedObject,
    decoded_bytes: Vec<u8>,
    requirement: DecodeRequirement,
    law: DecodeLaw,
    context: OperatorContext,
) -> Result<Transfer<DecodedObject>, Error> {
    match requirement {
        DecodeRequirement::ExactCheckpoint => {
            require_class(&context, RequirementClass::ExactCheckpoint)?;
            require_exact(&context)?;
        }
        DecodeRequirement::ApproximateValue { profile_id } => {
            require_class(&context, RequirementClass::ApproximateValue)?;
            require_approximate(&context, profile_id)?;
        }
    }
    if encoded.bytes.len() > context.resources.max_bytes
        || decoded_bytes.len() > context.resources.max_decoded_bytes
    {
        return Err(Error::Limit);
    }
    if input.uncertainty != UncertaintyClass::Exact
        || encoded.profile_id != context.requirement.basis.profile_id()
        || encoded.codec_epoch != context.requirement.basis.codec_epoch()
    {
        return Err(Error::Binding);
    }
    ensure_context_matches(input, &context)?;
    if input.value_as_known().is_some_and(|verified| {
        let declaration = verified.declaration();
        encoded.object_id != declaration.object_id
            || encoded.profile_id != declaration.profile_id
            || declaration.generation != verified.verifier_law().input_generation
    }) {
        return Err(Error::Binding);
    }
    let restart_grade = match (requirement, law) {
        (DecodeRequirement::ExactCheckpoint, DecodeLaw::RegisteredLossless(declaration)) => {
            if declaration.profile_id != encoded.profile_id
                || declaration.input_generation != encoded.codec_epoch
                || declaration.output_generation != context.requirement.basis.semantic_generation()
            {
                return Err(Error::Binding);
            }
            RestartGrade::Exact
        }
        (DecodeRequirement::ExactCheckpoint, DecodeLaw::Lossy { .. }) => {
            return Err(Error::InvalidInput);
        }
        (
            DecodeRequirement::ApproximateValue { profile_id },
            DecodeLaw::Lossy {
                profile_id: law_profile,
                error_bound_id,
            },
        ) if profile_id == law_profile
            && profile_id == encoded.profile_id
            && error_bound_id != 0 =>
        {
            RestartGrade::Approximate { error_bound_id }
        }
        (
            DecodeRequirement::ApproximateValue { profile_id },
            DecodeLaw::RegisteredLossless(declaration),
        ) if profile_id == declaration.profile_id
            && declaration.profile_id == encoded.profile_id
            && declaration.input_generation == encoded.codec_epoch
            && declaration.output_generation == context.requirement.basis.semantic_generation() =>
        {
            RestartGrade::Exact
        }
        _ => return Err(Error::Binding),
    };
    let law_id = match law {
        DecodeLaw::RegisteredLossless(declaration) => declaration.law_id,
        DecodeLaw::Lossy { error_bound_id, .. } => error_bound_id,
    };
    let retained_bytes = witness_bytes(input.witnesses.entries())?
        .checked_add(encoded.bytes.len())
        .and_then(|total| total.checked_add(decoded_bytes.len()))
        .ok_or(Error::Overflow)?;
    if retained_bytes > context.resources.max_bytes {
        return Err(Error::Limit);
    }
    let decoded = std::sync::Arc::<[u8]>::from(decoded_bytes);
    let witness = append_witnesses(
        input,
        vec![
            ReadWitness::Derived {
                law_id,
                base_generation: encoded.codec_epoch,
                profile_id: encoded.profile_id,
            },
            ReadWitness::EncodedBytes {
                object_id: encoded.object_id,
                profile_id: encoded.profile_id,
                codec_epoch: encoded.codec_epoch,
                bytes: encoded.bytes,
            },
            ReadWitness::DecodedBytes {
                object_id: encoded.object_id,
                profile_id: encoded.profile_id,
                codec_epoch: encoded.codec_epoch,
                bytes: decoded.clone(),
            },
        ],
        context.resources,
    )?;
    let value = map_knowledge(&input.value, |_| DecodedObject {
        object_id: encoded.object_id,
        bytes: decoded,
        profile_id: encoded.profile_id,
        codec_epoch: encoded.codec_epoch,
        restart_grade,
    });
    derived(
        input,
        value,
        witness,
        Vec::new(),
        context,
        AuthorityRequirements::new(Vec::new())?,
    )
}

/// `Probe`: certifies a sign only outside the declared error interval.
pub fn probe(
    input: &Transfer<DecodedObject>,
    probe_id: u64,
    signed_margin: i64,
    error_bound: i64,
    bound_id: u64,
    context: OperatorContext,
) -> Result<Transfer<ProbeResult>, Error> {
    require_class(&context, RequirementClass::Probe)?;
    require_bounded(&context, bound_id)?;
    ensure_context_matches(input, &context)?;
    if probe_id == 0 || bound_id == 0 || error_bound < 0 {
        return Err(Error::InvalidInput);
    }
    let magnitude = signed_margin.unsigned_abs();
    let bound = error_bound as u64;
    let witness = append_witness(
        input,
        ReadWitness::Epoch(context.requirement.basis),
        context.resources,
    )?;
    let value = match &input.value {
        Knowledge::Known { .. } if magnitude <= bound => {
            Knowledge::unknown(UnknownReason::Incomplete)
        }
        Knowledge::Known { .. } => map_knowledge(&input.value, |_| ProbeResult {
            probe_id,
            decision: if signed_margin > 0 {
                ProbeDecision::CertifiedPositive
            } else {
                ProbeDecision::CertifiedNegative
            },
            bound_id,
        }),
        _ => propagate_non_known(&input.value),
    };
    derived(
        input,
        value,
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Probe])?,
    )
}

/// `JudgeIndependent`: captures the whole already-validated actual helper view.
/// It does not execute a helper or accept a self-reported dependency subset.
pub fn judge_independent(
    actual_input: &ActualHelperInput,
    context: OperatorContext,
) -> Result<Transfer<IndependentJudgment>, Error> {
    require_class(&context, RequirementClass::IndependentJudgment)?;
    require_opaque(&context)?;
    let profile = actual_input.input_profile();
    if profile.profile_id != context.requirement.basis.profile_id()
        || profile.policy_epoch != context.requirement.basis.policy_epoch()
        || profile.model_epoch != context.requirement.basis.model_epoch()
        || profile.tokenizer_epoch != context.requirement.basis.tokenizer_epoch()
    {
        return Err(Error::Binding);
    }
    if actual_input.submitted_bytes().len() > context.resources.max_bytes {
        return Err(Error::Limit);
    }
    let witness = WitnessSet::new(
        vec![
            ReadWitness::FullInput(actual_input.clone()),
            ReadWitness::Epoch(context.requirement.basis),
        ],
        context.resources,
    )?;
    completed(
        Knowledge::known(
            IndependentJudgment {
                judgment: OpaqueJudgment::capture(actual_input, b"ignored by the algebra"),
            },
            context.requirement.basis,
        ),
        witness,
        Vec::new(),
        context,
        authority(&[AuthorityKind::Judge])?,
    )
}

/// `JoinEvidence`: combines homogeneous evidence transfers only when all are
/// known under the same basis.  It borrows its inputs so refusal retains their
/// witnesses and cancellation obligations, and unions privacy, authority,
/// frontiers and obligations conservatively.
pub fn join_evidence<T>(
    inputs: &[Transfer<T>],
    context: OperatorContext,
) -> Result<Transfer<JoinedEvidence>, Error> {
    require_class(&context, RequirementClass::JoinedEvidence)?;
    if inputs.is_empty() || inputs.len() > context.resources.max_inputs {
        return Err(Error::Limit);
    }
    if inputs
        .iter()
        .any(|input| input.uncertainty != context.requirement.uncertainty)
    {
        return Err(Error::Binding);
    }
    let origin_basis = inputs[0].origin_basis;
    if origin_basis != context.requirement.basis {
        return Err(Error::Binding);
    }
    let first_basis = inputs[0].value.basis();
    let incompatible_basis = inputs
        .iter()
        .any(|input| input.origin_basis != origin_basis);
    let mut witnesses = WitnessSet::new(Vec::new(), context.resources)?;
    let mut privacy = context.privacy.clone();
    let mut authority = AuthorityRequirements::new(Vec::new())?;
    let mut cancellation = context.cancellation.clone();
    let mut frontiers = Vec::new();
    for input in inputs {
        witnesses = witnesses.union(&input.witnesses, context.resources)?;
        privacy = privacy.union(&input.privacy)?;
        authority = authority.union(&input.authority)?;
        cancellation = cancellation.union(&input.cancellation)?;
        frontiers = union_frontiers(
            &frontiers,
            &input.frontiers,
            context.resources.max_frontiers,
        )?;
    }
    let value = if incompatible_basis {
        Knowledge::unknown(UnknownReason::ConflictingEvidence)
    } else if inputs
        .iter()
        .all(|input| matches!(input.value, Knowledge::Known { .. }))
    {
        Knowledge::known(
            JoinedEvidence {
                member_count: inputs.len(),
            },
            first_basis.ok_or(Error::Binding)?,
        )
    } else if let Some(pending) = inputs.iter().find_map(|input| match &input.value {
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => Some((frontiers.clone(), *expected_cost)),
        _ => None,
    }) {
        Knowledge::pending(pending.0, pending.1)?
    } else {
        Knowledge::unknown(UnknownReason::Incomplete)
    };
    Transfer {
        origin_basis,
        uncertainty: context.requirement.uncertainty,
        value,
        witnesses,
        frontiers,
        privacy,
        authority,
        cancellation,
        resources: context.resources,
        cancelled: inputs.iter().any(|input| input.cancelled),
    }
    .checked()
}

/// `AwaitClosedScope`: succeeds only at a caller-declared registered closing
/// assumption and a satisfied product-frontier requirement.
pub fn await_closed_scope(
    snapshot: &WitnessSnapshot,
    frontiers: &ProductFrontiers,
    requirement: FrontierRequirement,
    context: OperatorContext,
) -> Result<Transfer<ClosedScope>, Error> {
    require_class(&context, RequirementClass::ClosedScope)?;
    require_exact(&context)?;
    if snapshot.semantic_epoch() != context.requirement.basis.semantic_generation()
        || snapshot.control_cut() != context.requirement.basis.control_seq()
        || requirement.stage != FrontierStage::Authenticated
    {
        return Err(Error::Binding);
    }
    let DomainClosure::Closed(marker) = snapshot.domain_input().closure() else {
        return Err(Error::Incomplete);
    };
    if requirement.key != snapshot.domain_input().domain().projection()
        || requirement.key != marker.key
        || requirement.through != marker.final_sequence
        || requirement.closure != Some(marker.marker_generation)
    {
        return Err(Error::InvalidInput);
    }
    let witness = WitnessSet::new(
        vec![
            ReadWitness::WitnessSnapshot {
                domain: snapshot.domain_input().domain(),
                revision: snapshot.revision(),
                control_cut: snapshot.control_cut(),
                semantic_epoch: snapshot.semantic_epoch(),
            },
            ReadWitness::Frontier(requirement),
        ],
        context.resources,
    )?;
    let value = if frontiers.satisfies(requirement)? {
        Knowledge::known(
            ClosedScope {
                domain: ClosedDomain::new(
                    snapshot.domain_input().domain(),
                    snapshot.revision(),
                    snapshot.control_cut(),
                    snapshot.semantic_epoch(),
                    requirement,
                    marker,
                )?,
            },
            context.requirement.basis,
        )
    } else {
        Knowledge::pending(vec![requirement], 1)?
    };
    completed(
        value,
        witness,
        vec![requirement],
        context,
        authority(&[AuthorityKind::CloseScope])?,
    )
}

/// `EmitJudgment`: records a bounded, compatible judgment/evidence pairing.
/// It deliberately has no permit or effect-bearing output type.
pub fn emit_judgment(
    evidence: &Transfer<JoinedEvidence>,
    judgment: &Transfer<IndependentJudgment>,
    context: OperatorContext,
) -> Result<Transfer<EmittedJudgment>, Error> {
    require_class(&context, RequirementClass::EmittedJudgment)?;
    require_opaque(&context)?;
    if context.requirement.basis != evidence.origin_basis
        || context.requirement.uncertainty != judgment.uncertainty
    {
        return Err(Error::Binding);
    }
    let incompatible_basis = judgment.origin_basis != evidence.origin_basis
        || matches!(
            (&evidence.value, &judgment.value),
            (
                Knowledge::Known { basis: evidence_basis, .. },
                Knowledge::Known { basis: judgment_basis, .. }
            ) if evidence_basis != judgment_basis
        );
    let witnesses = evidence
        .witnesses
        .union(&judgment.witnesses, context.resources)?;
    let privacy = evidence
        .privacy
        .union(&judgment.privacy)?
        .union(&context.privacy)?;
    let authority = evidence
        .authority
        .union(&judgment.authority)?
        .union(&authority(&[AuthorityKind::Judge])?)?;
    let cancellation = evidence
        .cancellation
        .union(&judgment.cancellation)?
        .union(&context.cancellation)?;
    let frontiers = union_frontiers(
        &evidence.frontiers,
        &judgment.frontiers,
        context.resources.max_frontiers,
    )?;
    let cancelled = evidence.cancelled || judgment.cancelled;
    let value = if incompatible_basis {
        Knowledge::unknown(UnknownReason::IncompatibleGeneration)
    } else {
        match (&evidence.value, &judgment.value) {
            (
                Knowledge::Known {
                    value: evidence, ..
                },
                Knowledge::Known { .. },
            ) => Knowledge::known(
                EmittedJudgment {
                    basis: context.requirement.basis,
                    evidence_members: evidence.member_count,
                },
                context.requirement.basis,
            ),
            (
                Knowledge::Pending {
                    frontiers,
                    expected_cost,
                },
                _,
            )
            | (
                _,
                Knowledge::Pending {
                    frontiers,
                    expected_cost,
                },
            ) => Knowledge::pending(frontiers.clone(), *expected_cost)?,
            (Knowledge::Unknown { reason }, _) | (_, Knowledge::Unknown { reason }) => {
                Knowledge::unknown(*reason)
            }
            (Knowledge::Withheld { authority_required }, _)
            | (_, Knowledge::Withheld { authority_required }) => {
                Knowledge::withheld(*authority_required)
            }
            _ => Knowledge::unknown(UnknownReason::Incomplete),
        }
    };
    Transfer {
        origin_basis: evidence.origin_basis,
        uncertainty: context.requirement.uncertainty,
        value,
        witnesses,
        frontiers,
        privacy,
        authority,
        cancellation,
        resources: context.resources,
        cancelled,
    }
    .checked()
}

impl<T> Transfer<T> {
    fn checked(self) -> Result<Self, Error> {
        validate_frontiers(&self.frontiers)?;
        if self.frontiers.len() > self.resources.max_frontiers {
            return Err(Error::Limit);
        }
        Ok(self)
    }

    fn value_as_known(&self) -> Option<&T> {
        match &self.value {
            Knowledge::Known { value, .. } => Some(value),
            Knowledge::Pending { .. }
            | Knowledge::Unknown { .. }
            | Knowledge::Withheld { .. }
            | Knowledge::Stale { .. }
            | Knowledge::Absent { .. } => None,
        }
    }
}

fn completed<T>(
    value: Knowledge<T>,
    witnesses: WitnessSet,
    frontiers: Vec<FrontierRequirement>,
    context: OperatorContext,
    authority: AuthorityRequirements,
) -> Result<Transfer<T>, Error> {
    Transfer {
        origin_basis: context.requirement.basis,
        uncertainty: context.requirement.uncertainty,
        value,
        witnesses,
        frontiers,
        privacy: context.privacy,
        authority,
        cancellation: context.cancellation,
        resources: context.resources,
        cancelled: false,
    }
    .checked()
}

fn derived<I, O>(
    input: &Transfer<I>,
    value: Knowledge<O>,
    witnesses: WitnessSet,
    frontiers: Vec<FrontierRequirement>,
    context: OperatorContext,
    authority: AuthorityRequirements,
) -> Result<Transfer<O>, Error> {
    let frontiers = union_frontiers(
        &input.frontiers,
        &frontiers,
        context.resources.max_frontiers,
    )?;
    let privacy = input.privacy.union(&context.privacy)?;
    let authority = input.authority.union(&authority)?;
    let cancellation = input.cancellation.union(&context.cancellation)?;
    Transfer {
        origin_basis: input.origin_basis,
        uncertainty: context.requirement.uncertainty,
        value,
        witnesses,
        frontiers,
        privacy,
        authority,
        cancellation,
        resources: context.resources,
        cancelled: input.cancelled,
    }
    .checked()
}

fn ensure_context_matches<T>(input: &Transfer<T>, context: &OperatorContext) -> Result<(), Error> {
    if input.origin_basis != context.requirement.basis {
        Err(Error::Binding)
    } else {
        Ok(())
    }
}

fn append_witness<T>(
    input: &Transfer<T>,
    additional: ReadWitness,
    resources: ResourceEnvelope,
) -> Result<WitnessSet, Error> {
    append_witnesses(input, vec![additional], resources)
}

fn append_witnesses<T>(
    input: &Transfer<T>,
    additions: Vec<ReadWitness>,
    resources: ResourceEnvelope,
) -> Result<WitnessSet, Error> {
    let extra = WitnessSet::new(additions, resources)?;
    input.witnesses.union_owned(extra, resources)
}

fn map_knowledge<T, U>(input: &Knowledge<T>, map: impl FnOnce(&T) -> U) -> Knowledge<U> {
    match input {
        Knowledge::Known { value, basis } => Knowledge::known(map(value), *basis),
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => Knowledge::Pending {
            frontiers: frontiers.clone(),
            expected_cost: *expected_cost,
        },
        Knowledge::Unknown { reason } => Knowledge::unknown(*reason),
        Knowledge::Withheld { authority_required } => Knowledge::withheld(*authority_required),
        Knowledge::Stale {
            basis,
            current_generation,
        } => Knowledge::stale(*basis, *current_generation),
        Knowledge::Absent { domain } => Knowledge::absent(domain.clone()),
    }
}

fn propagate_non_known<T, U>(input: &Knowledge<T>) -> Knowledge<U> {
    match input {
        Knowledge::Known { .. } => Knowledge::unknown(UnknownReason::Incomplete),
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => Knowledge::Pending {
            frontiers: frontiers.clone(),
            expected_cost: *expected_cost,
        },
        Knowledge::Unknown { reason } => Knowledge::unknown(*reason),
        Knowledge::Withheld { authority_required } => Knowledge::withheld(*authority_required),
        Knowledge::Stale {
            basis,
            current_generation,
        } => Knowledge::stale(*basis, *current_generation),
        Knowledge::Absent { domain } => Knowledge::absent(domain.clone()),
    }
}

fn authority(kinds: &[AuthorityKind]) -> Result<AuthorityRequirements, Error> {
    AuthorityRequirements::new(kinds.to_vec())
}

fn require_class(context: &OperatorContext, class: RequirementClass) -> Result<(), Error> {
    if context.requirement.class == class {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn require_exact(context: &OperatorContext) -> Result<(), Error> {
    if matches!(context.requirement.uncertainty, UncertaintyClass::Exact) {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn require_approximate(context: &OperatorContext, profile_id: u64) -> Result<(), Error> {
    if matches!(context.requirement.uncertainty, UncertaintyClass::Approximate { profile_id: actual } if actual == profile_id)
    {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn require_bounded(context: &OperatorContext, bound_id: u64) -> Result<(), Error> {
    if matches!(context.requirement.uncertainty, UncertaintyClass::Bounded { bound_id: actual } if actual == bound_id)
    {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn require_opaque(context: &OperatorContext) -> Result<(), Error> {
    if matches!(context.requirement.uncertainty, UncertaintyClass::Opaque { calibration_id } if calibration_id != 0)
    {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn require_refinable(context: &OperatorContext) -> Result<(), Error> {
    if matches!(
        context.requirement.uncertainty,
        UncertaintyClass::Exact | UncertaintyClass::Conservative
    ) {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}

fn canonical_labels(mut labels: Vec<PrivacyLabel>) -> Result<Vec<PrivacyLabel>, Error> {
    if labels.len() > MAX_OPERATOR_PRIVACY_LABELS {
        return Err(Error::Limit);
    }
    labels.sort_unstable();
    labels.dedup();
    Ok(labels)
}

fn validate_frontiers(frontiers: &[FrontierRequirement]) -> Result<(), Error> {
    if frontiers.len() > MAX_OPERATOR_FRONTIERS {
        return Err(Error::Limit);
    }
    for (index, frontier) in frontiers.iter().enumerate() {
        if frontier.closure == Some(0) {
            return Err(Error::InvalidInput);
        }
        if frontiers[..index].contains(frontier) {
            return Err(Error::Duplicate);
        }
    }
    Ok(())
}

/// Exact frontier identity is idempotent during transfer composition.  Raw
/// construction remains strict through `validate_frontiers`; this helper only
/// merges already-valid frontier sets and invents no broader equivalence.
fn union_frontiers(
    left: &[FrontierRequirement],
    right: &[FrontierRequirement],
    limit: usize,
) -> Result<Vec<FrontierRequirement>, Error> {
    validate_frontiers(left)?;
    validate_frontiers(right)?;
    let capacity = left.len().checked_add(right.len()).ok_or(Error::Overflow)?;
    let mut merged = Vec::with_capacity(capacity);
    for frontier in left.iter().chain(right) {
        if !merged.contains(frontier) {
            if merged.len() == limit {
                return Err(Error::Limit);
            }
            merged.push(*frontier);
        }
    }
    Ok(merged)
}

fn validate_witnesses(entries: &[ReadWitness], bounds: ResourceEnvelope) -> Result<(), Error> {
    if entries.len() > bounds.max_witnesses {
        return Err(Error::Limit);
    }
    if witness_bytes(entries)? > bounds.max_bytes {
        return Err(Error::Limit);
    }
    Ok(())
}

fn witness_bytes(entries: &[ReadWitness]) -> Result<usize, Error> {
    entries.iter().try_fold(0_usize, |bytes, entry| {
        let added = match entry {
            ReadWitness::CapturedBytes { bytes, .. } => bytes.len(),
            ReadWitness::ExactValue { value, .. } => value.len(),
            ReadWitness::EncodedBytes { bytes, .. } => bytes.len(),
            ReadWitness::DecodedBytes { bytes, .. } => bytes.len(),
            ReadWitness::FullInput(input) => input
                .submitted_bytes()
                .len()
                .checked_add(input.input_profile().profile_bytes.len())
                .ok_or(Error::Overflow)?,
            ReadWitness::ExactObject { .. }
            | ReadWitness::AbsentKey { .. }
            | ReadWitness::ClosedDomain(_)
            | ReadWitness::Derived { .. }
            | ReadWitness::Epoch(_)
            | ReadWitness::Frontier(_)
            | ReadWitness::WitnessSnapshot { .. } => 0,
        };
        bytes.checked_add(added).ok_or(Error::Overflow)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_rejects_privately_constructed_nonexact_verified_transfer() {
        let basis = Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 2,
                profile_id: 3,
                semantic_generation: 4,
                policy_epoch: 0,
                model_epoch: 0,
                tokenizer_epoch: 0,
                codec_epoch: 5,
                control_seq: 0,
            },
            ClaimClass::BoundedModel,
            ProofStates {
                authentic_origin: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 0,
                },
                complete_for_contract: ScopedProof::Declared {
                    scope_id: 2,
                    generation: 0,
                },
                semantically_valid: ScopedProof::Declared {
                    scope_id: 3,
                    generation: 0,
                },
                available_for_replay: ScopedProof::Declared {
                    scope_id: 4,
                    generation: 0,
                },
            },
        );
        let resources = ResourceEnvelope::new(1, 1, 4, 1, 32, 32, 1).expect("valid bounds");
        let no_privacy = PrivacyRestrictions::new(Vec::new()).expect("empty privacy set");
        let no_obligations =
            CancellationObligations::new(Vec::new()).expect("empty obligation set");
        let input = Transfer {
            origin_basis: basis,
            uncertainty: UncertaintyClass::Approximate { profile_id: 3 },
            value: Knowledge::unknown(UnknownReason::Unavailable),
            witnesses: WitnessSet::new(Vec::new(), resources).expect("empty witness set"),
            frontiers: Vec::new(),
            privacy: no_privacy.clone(),
            authority: AuthorityRequirements::new(Vec::new()).expect("empty authority set"),
            cancellation: no_obligations.clone(),
            resources,
            cancelled: false,
        }
        .checked()
        .expect("private adversarial input");
        let context = OperatorContext::new(
            ObservationRequirement {
                class: RequirementClass::ExactCheckpoint,
                uncertainty: UncertaintyClass::Exact,
                basis,
            },
            no_privacy,
            no_obligations,
            resources,
        );

        let run = |candidate: &Transfer<VerifiedObject>| {
            decode(
                candidate,
                EncodedObject {
                    object_id: 7,
                    bytes: b"encoded".to_vec(),
                    profile_id: 3,
                    codec_epoch: 5,
                },
                b"decoded".to_vec(),
                DecodeRequirement::ExactCheckpoint,
                DecodeLaw::RegisteredLossless(
                    LawDeclaration::new(9, 5, 4, 3).expect("registered law"),
                ),
                context.clone(),
            )
        };

        let exact_input = Transfer {
            uncertainty: UncertaintyClass::Exact,
            ..input.clone()
        };
        assert!(matches!(
            run(&exact_input),
            Ok(output) if matches!(output.value(), Knowledge::Unknown { reason: UnknownReason::Unavailable })
        ));
        let result = run(&input);

        assert_eq!(result, Err(Error::Binding));
        assert!(matches!(input.value(), Knowledge::Unknown { .. }));
    }
}
