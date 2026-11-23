//! FA-116 control-overhead accounting, FA-119 graduated autonomy, FA-125 risk-theater
//! standing-query detector, FA-126 control receipts & independent verifier, FA-127 assurance
//! profiles & profile doctor, FA-131 attack-capsule interchange, and FA-142 policy disclosure
//! & witness-cached `fa precheck` (plan §4.3, §6.7, §7.6, §8.3, §14.1, §15.8, §16.6, §17.1,
//! §18.1; FI-A06, FI-A09, FI-A13, FI-A16, FI-A17, FI-I15).

use std::collections::{BTreeMap, BTreeSet};

use crate::action::consequence::Consequence;
use crate::address_resolver::{Address, Noun};
use crate::event_journal::{DomainEventJournal, ReferenceDigestHasher};
use crate::observation_algebra::ClaimClass;
use crate::typestate_cx::ExperimentCx;

/// Marker trait for governance/evaluator surfaces that must never be serialized to the actor (`FA-INV-008`).
pub trait NotActorVisible {}

// ============================================================================
// FA-127 & FA-142: Assurance profiles, policy disclosure & witness-cached precheck
// ============================================================================

/// Policy disclosure levels declared in an [`AssuranceProfile`] (`lo65.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDisclosureLevel {
    None,
    ExactPredicates,
    Full,
}

/// Declared information-leakage table required when `policy_disclosure` is `ExactPredicates` or `Full` (`lo65.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeakageTable {
    pub bits_per_precheck_ceiling: u32,
    pub disclosed_predicate_families: Vec<String>,
    pub rate_limit_per_window: u32,
}

/// Profile claim kinds mapped to required executable conformance tests (`icw.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProfileClaimKind {
    BrokeredMediation,
    WhiteBoxCapture,
    DecisionReplayGrade,
    IndependentCohortRoster,
}

/// Signed assurance profile vector (`icw.1`, `lo65.1`, plan §4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssuranceProfile {
    pub profile_id: String,
    pub version: u32,
    pub mediation_by_family: BTreeMap<String, String>,
    pub coverage_contract_ids: Vec<String>,
    pub replay_grade: String,
    pub roster_class: String,
    pub autonomy_floor: AutonomyGrade,
    pub policy_disclosure: PolicyDisclosureLevel,
    pub leakage_table: Option<LeakageTable>,
    pub claims: Vec<ProfileClaimKind>,
    pub operator_signature: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    UnsignedProfileRefused,
    MissingMediationVector,
    MissingLeakageTableForDisclosure { level: PolicyDisclosureLevel },
    ClaimMissingRequiredTests { claim: ProfileClaimKind },
    UnexecutedOrFailedClaimCannotCertify {
        claim: ProfileClaimKind,
        status: &'static str,
    },
    PrecheckDisabledUnderNoDisclosure,
}

impl AssuranceProfile {
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.operator_signature == [0u8; 32] {
            return Err(ProfileError::UnsignedProfileRefused);
        }
        if self.mediation_by_family.is_empty() {
            return Err(ProfileError::MissingMediationVector);
        }
        if matches!(
            self.policy_disclosure,
            PolicyDisclosureLevel::ExactPredicates | PolicyDisclosureLevel::Full
        ) && self.leakage_table.is_none()
        {
            return Err(ProfileError::MissingLeakageTableForDisclosure {
                level: self.policy_disclosure,
            });
        }
        Ok(())
    }
}

/// Execution outcome of a required test suite for a profile claim (`icw.2`, `icw.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimTestExecutionStatus {
    Passed,
    Failed,
    Skipped,
    NotExecuted,
}

/// Signed conformance report emitted by `fa doctor --profile` (`icw.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileConformanceReport {
    pub profile_id: String,
    pub certified_claims: Vec<ProfileClaimKind>,
    pub report_digest: [u8; 32],
}

/// Execute `fa doctor --profile`: maps every claim in `profile` to its required test suites and
/// refuses to certify if any required test is missing, `Skipped`, `NotExecuted`, or `Failed`
/// (`SKIP` is never `PASS`, `icw.2`, `icw.3`).
pub fn fa_doctor_profile(
    profile: &AssuranceProfile,
    required_tests_registry: &BTreeMap<ProfileClaimKind, Vec<&'static str>>,
    executed_results: &BTreeMap<&'static str, ClaimTestExecutionStatus>,
) -> Result<ProfileConformanceReport, ProfileError> {
    profile.validate()?;
    let mut certified_claims = Vec::new();
    let mut hasher = ReferenceDigestHasher::new();
    hasher.update(b"fa-profile-conformance-v1:");
    hasher.update(profile.profile_id.as_bytes());

    for &claim in &profile.claims {
        let Some(test_ids) = required_tests_registry.get(&claim) else {
            return Err(ProfileError::ClaimMissingRequiredTests { claim });
        };
        if test_ids.is_empty() {
            return Err(ProfileError::ClaimMissingRequiredTests { claim });
        }
        for &tid in test_ids {
            match executed_results
                .get(tid)
                .copied()
                .unwrap_or(ClaimTestExecutionStatus::NotExecuted)
            {
                ClaimTestExecutionStatus::Passed => {
                    hasher.update(tid.as_bytes());
                }
                ClaimTestExecutionStatus::Skipped => {
                    return Err(ProfileError::UnexecutedOrFailedClaimCannotCertify {
                        claim,
                        status: "skipped",
                    });
                }
                ClaimTestExecutionStatus::NotExecuted => {
                    return Err(ProfileError::UnexecutedOrFailedClaimCannotCertify {
                        claim,
                        status: "not_executed",
                    });
                }
                ClaimTestExecutionStatus::Failed => {
                    return Err(ProfileError::UnexecutedOrFailedClaimCannotCertify {
                        claim,
                        status: "failed",
                    });
                }
            }
        }
        certified_claims.push(claim);
    }

    Ok(ProfileConformanceReport {
        profile_id: profile.profile_id.clone(),
        certified_claims,
        report_digest: hasher.finalize(),
    })
}

/// Minimal actor-visible output of `fa precheck` (`lo65.2`, `lo65.4`).
/// The actor-surface filter strips everything beyond these two variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrecheckResult {
    DeniedByPolicy { effect_family: String },
    PrecheckPassed { review_required: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedPrecheckEntry {
    pub proposal_digest: [u8; 32],
    pub policy_epoch: u64,
    pub witness_generation: u64,
    pub result: PrecheckResult,
}

/// Witness-cached exact-predicate precheck engine (`lo65.2`, `lo65.3`, `lo65.4`).
/// Emits zero permits, zero journal events, and zero helper calls (`FA-INV-040`).
#[derive(Debug, Default)]
pub struct WitnessCachedPrecheckEngine {
    current_policy_epoch: u64,
    cache: BTreeMap<([u8; 32], u64), CachedPrecheckEntry>,
    aggregate_precheck_count: u64,
    helper_calls_made: u64,
}

impl WitnessCachedPrecheckEngine {
    pub fn new(initial_policy_epoch: u64) -> Self {
        Self {
            current_policy_epoch: initial_policy_epoch,
            cache: BTreeMap::new(),
            aggregate_precheck_count: 0,
            helper_calls_made: 0,
        }
    }

    pub const fn aggregate_precheck_count(&self) -> u64 {
        self.aggregate_precheck_count
    }

    pub const fn helper_calls_made(&self) -> u64 {
        self.helper_calls_made
    }

    pub fn advance_policy_epoch(&mut self, new_epoch: u64) {
        if new_epoch > self.current_policy_epoch {
            self.current_policy_epoch = new_epoch;
            self.cache.clear();
        }
    }

    pub fn fa_precheck(
        &mut self,
        profile: &AssuranceProfile,
        proposal_digest: [u8; 32],
        effect_family: &str,
        exact_predicate_allows: bool,
        review_required: bool,
        witness_generation: u64,
        journal: &DomainEventJournal,
    ) -> Result<(PrecheckResult, bool), ProfileError> {
        profile.validate()?;
        if profile.policy_disclosure == PolicyDisclosureLevel::None {
            return Err(ProfileError::PrecheckDisabledUnderNoDisclosure);
        }

        let before_journal_len = journal.len();
        self.aggregate_precheck_count += 1;

        let key = (proposal_digest, self.current_policy_epoch);
        if let Some(hit) = self.cache.get(&key)
            && hit.witness_generation == witness_generation
        {
            debug_assert_eq!(journal.len(), before_journal_len);
            return Ok((hit.result.clone(), true));
        }

        let result = if !exact_predicate_allows {
            PrecheckResult::DeniedByPolicy {
                effect_family: effect_family.to_owned(),
            }
        } else {
            PrecheckResult::PrecheckPassed { review_required }
        };

        self.cache.insert(
            key,
            CachedPrecheckEntry {
                proposal_digest,
                policy_epoch: self.current_policy_epoch,
                witness_generation,
                result: result.clone(),
            },
        );
        debug_assert_eq!(journal.len(), before_journal_len);
        Ok((result, false))
    }
}

// ============================================================================
// FA-126: Control receipts and independent verifier (`fa verify`)
// ============================================================================

/// Portable control receipt issued only at admitted dispatch transition (`ahb.1`, `ahb.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlReceiptV1 {
    pub receipt_id: String,
    pub attempt_id: u64,
    pub closure_digest: [u8; 32],
    pub permit_conjuncts_verified: bool,
    pub claim_class: ClaimClass,
    pub profile_id: String,
    pub signing_principal: String,
    pub operator_signature: [u8; 32],
    pub redacted_fields: Vec<String>,
}

impl ControlReceiptV1 {
    pub fn compute_expected_signature(
        receipt_id: &str,
        attempt_id: u64,
        closure_digest: &[u8; 32],
        permit_conjuncts_verified: bool,
        profile_id: &str,
        signing_principal: &str,
    ) -> [u8; 32] {
        let mut hasher = ReferenceDigestHasher::new();
        hasher.update(b"fa-control-receipt-v1:");
        hasher.update(receipt_id.as_bytes());
        hasher.update(&attempt_id.to_be_bytes());
        hasher.update(closure_digest);
        hasher.update(&[u8::from(permit_conjuncts_verified)]);
        hasher.update(profile_id.as_bytes());
        hasher.update(signing_principal.as_bytes());
        hasher.finalize()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptError {
    UnadmittedEffectCannotMintReceipt,
    ActorKeyForbiddenForReceiptSigning,
    ForgedOrTamperedSignature,
    PermitConjunctsNotSatisfied,
    ReplayedReceiptForSecondDispatch { receipt_id: String },
}

/// Issue a [`ControlReceiptV1`] at dispatch admission (`ahb.2`).
/// Unadmitted attempts (`dispatch_admitted == false`) have no code path to produce a receipt.
pub fn issue_dispatch_receipt(
    receipt_id: &str,
    attempt_id: u64,
    dispatch_admitted: bool,
    closure_digest: [u8; 32],
    profile_id: &str,
    signing_principal: &str,
    redacted_fields: Vec<String>,
) -> Result<ControlReceiptV1, ReceiptError> {
    if !dispatch_admitted {
        return Err(ReceiptError::UnadmittedEffectCannotMintReceipt);
    }
    if signing_principal.starts_with("actor:") {
        return Err(ReceiptError::ActorKeyForbiddenForReceiptSigning);
    }
    let operator_signature = ControlReceiptV1::compute_expected_signature(
        receipt_id,
        attempt_id,
        &closure_digest,
        true,
        profile_id,
        signing_principal,
    );
    Ok(ControlReceiptV1 {
        receipt_id: receipt_id.to_owned(),
        attempt_id,
        closure_digest,
        permit_conjuncts_verified: true,
        claim_class: ClaimClass::Invariant,
        profile_id: profile_id.to_owned(),
        signing_principal: signing_principal.to_owned(),
        operator_signature,
        redacted_fields,
    })
}

/// Structured verification report returned by `IndependentReceiptVerifier::fa_verify` (`ahb.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptVerificationReport {
    pub receipt_id: String,
    pub verified_properties: Vec<&'static str>,
    pub unverifiable_properties: Vec<&'static str>,
    pub redacted_properties: Vec<String>,
}

/// Independent receipt verifier sharing no state with the broker (`ahb.3`, `ahb.4`).
#[derive(Debug, Default)]
pub struct IndependentReceiptVerifier {
    seen_receipt_dispatches: BTreeSet<(String, u64)>,
}

impl IndependentReceiptVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fa_verify(
        &mut self,
        receipt: &ControlReceiptV1,
        target_dispatch_attempt_id: u64,
    ) -> Result<ReceiptVerificationReport, ReceiptError> {
        if receipt.signing_principal.starts_with("actor:") {
            return Err(ReceiptError::ActorKeyForbiddenForReceiptSigning);
        }
        if !receipt.permit_conjuncts_verified {
            return Err(ReceiptError::PermitConjunctsNotSatisfied);
        }
        let expected_sig = ControlReceiptV1::compute_expected_signature(
            &receipt.receipt_id,
            receipt.attempt_id,
            &receipt.closure_digest,
            receipt.permit_conjuncts_verified,
            &receipt.profile_id,
            &receipt.signing_principal,
        );
        if receipt.operator_signature != expected_sig {
            return Err(ReceiptError::ForgedOrTamperedSignature);
        }
        if target_dispatch_attempt_id != receipt.attempt_id
            || !self
                .seen_receipt_dispatches
                .insert((receipt.receipt_id.clone(), target_dispatch_attempt_id))
        {
            return Err(ReceiptError::ReplayedReceiptForSecondDispatch {
                receipt_id: receipt.receipt_id.clone(),
            });
        }

        Ok(ReceiptVerificationReport {
            receipt_id: receipt.receipt_id.clone(),
            verified_properties: vec![
                "operator_signature",
                "closure_digest_binding",
                "permit_conjuncts",
                "profile_reference",
            ],
            unverifiable_properties: if receipt.redacted_fields.is_empty() {
                Vec::new()
            } else {
                vec!["redacted_payload_content"]
            },
            redacted_properties: receipt.redacted_fields.clone(),
        })
    }
}

// ============================================================================
// FA-116: Control-overhead ratio accounting
// ============================================================================

/// Task outcome classification for control-overhead accounting (`qyo.1`, plan §18.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CostOutcomeClass {
    UsefulCompleted,
    Held,
    Denied,
    Unknown,
}

/// Nine cost components tracked per task (`qyo.1`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlCostComponents {
    pub capture: u64,
    pub transfer: u64,
    pub encoding: u64,
    pub helper_inference: u64,
    pub gate_cpu: u64,
    pub persistence: u64,
    pub network: u64,
    pub replay: u64,
    pub rendering: u64,
}

impl ControlCostComponents {
    pub const fn total_control_cost(&self) -> u64 {
        self.capture
            + self.transfer
            + self.encoding
            + self.helper_inference
            + self.gate_cpu
            + self.persistence
            + self.network
            + self.replay
            + self.rendering
    }
}

/// Per-task cost attribution record (`qyo.1`, `qyo.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlCostRecord {
    pub task_id: String,
    pub contract_generation: u64,
    pub actor_inference_cost: u64,
    pub components: ControlCostComponents,
    pub outcome: CostOutcomeClass,
}

/// Aggregated control-overhead report preserving held, denied, and unknown tasks separately (`qyo.3`, `qyo.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlOverheadSummary {
    pub useful_completed_count: u64,
    pub held_count: u64,
    pub denied_count: u64,
    pub unknown_count: u64,
    pub total_control_cost_all_outcomes: u64,
    pub total_actor_cost_useful_completed: u64,
    /// Numerator / denominator (basis points: `total_control_cost_all_outcomes * 10_000 / max(total_actor_cost_useful_completed, 1)`).
    pub overhead_ratio_bps_per_useful_task: u64,
}

pub fn summarize_control_overhead(records: &[ControlCostRecord]) -> ControlOverheadSummary {
    let mut useful_completed_count = 0u64;
    let mut held_count = 0u64;
    let mut denied_count = 0u64;
    let mut unknown_count = 0u64;
    let mut total_control_cost_all_outcomes = 0u64;
    let mut total_actor_cost_useful_completed = 0u64;

    for rec in records {
        total_control_cost_all_outcomes += rec.components.total_control_cost();
        match rec.outcome {
            CostOutcomeClass::UsefulCompleted => {
                useful_completed_count += 1;
                total_actor_cost_useful_completed += rec.actor_inference_cost;
            }
            CostOutcomeClass::Held => held_count += 1,
            CostOutcomeClass::Denied => denied_count += 1,
            CostOutcomeClass::Unknown => unknown_count += 1,
        }
    }

    let denom = total_actor_cost_useful_completed.max(1);
    let overhead_ratio_bps_per_useful_task =
        (total_control_cost_all_outcomes * 10_000) / denom;

    ControlOverheadSummary {
        useful_completed_count,
        held_count,
        denied_count,
        unknown_count,
        total_control_cost_all_outcomes,
        total_actor_cost_useful_completed,
        overhead_ratio_bps_per_useful_task,
    }
}

// ============================================================================
// FA-119: Graduated autonomy: actor authority ledger and grades (FA-INV-036)
// ============================================================================

/// Ordered autonomy grades (`q23.1`). Grade selects observation lanes, never skips the audit floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AutonomyGrade {
    Propose = 0,
    Reviewed = 1,
    Sampled = 2,
    Bounded = 3,
}

/// Control-nucleus autonomy state (`NotActorVisible`, `q23.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutonomyState {
    pub principal: String,
    pub run_id: u64,
    pub family: String,
    pub grade: AutonomyGrade,
    pub floor: AutonomyGrade,
    pub since_control_seq: u64,
    pub passport_digest: [u8; 32],
    pub audit_floor_active: bool,
}

impl NotActorVisible for AutonomyState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutonomyError {
    ActorSelfSummaryForbiddenForLedger,
    UnsignedEvaluatorReport,
    InsufficientDwellTime { elapsed_seqs: u64, minimum_required: u64 },
    AuditFloorCannotBeSkippedAtBoundedGrade,
    PromotionAboveSingleStepForbidden,
}

/// Append-only evaluator-labeled actor track-record ledger (`q23.2`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActorTrackRecordLedger {
    pub confirmed_clean_effects: u64,
    pub holds: u64,
    pub confirmed_violations: u64,
    pub andon_consumption_caused: u64,
    pub identity_incidents: u64,
    pub shadow_campaign_passes: u64,
}

impl ActorTrackRecordLedger {
    pub fn record_evaluator_entry(
        &mut self,
        source_principal: &str,
        clean_effect: bool,
        violation: bool,
    ) -> Result<(), AutonomyError> {
        if source_principal.starts_with("actor:") {
            return Err(AutonomyError::ActorSelfSummaryForbiddenForLedger);
        }
        if clean_effect {
            self.confirmed_clean_effects += 1;
        }
        if violation {
            self.confirmed_violations += 1;
        }
        Ok(())
    }
}

impl AutonomyState {
    pub fn new(
        principal: &str,
        run_id: u64,
        family: &str,
        floor: AutonomyGrade,
        control_seq: u64,
        passport_digest: [u8; 32],
    ) -> Self {
        Self {
            principal: principal.to_owned(),
            run_id,
            family: family.to_owned(),
            grade: floor,
            floor,
            since_control_seq: control_seq,
            passport_digest,
            audit_floor_active: true,
        }
    }

    /// Automatic ordered demotion on consequence >= `NarrowAuthority`, confirmed violation,
    /// identity incident, or coverage loss (`q23.3`).
    pub fn demote_on_incident(
        &mut self,
        consequence: Consequence,
        identity_incident: bool,
        coverage_loss: bool,
        control_seq: u64,
    ) {
        if matches!(
            consequence,
            Consequence::NarrowAuthority | Consequence::SuspendRun | Consequence::ResetToCheckpoint
        ) || identity_incident
            || coverage_loss
        {
            self.grade = self.floor;
            self.since_control_seq = control_seq;
        }
    }

    /// Model passport change immediately resets the grade to `floor` (`q23.4`, `q23.5`).
    pub fn on_passport_change(&mut self, new_passport_digest: [u8; 32], control_seq: u64) {
        if self.passport_digest != new_passport_digest {
            self.passport_digest = new_passport_digest;
            self.grade = self.floor;
            self.since_control_seq = control_seq;
        }
    }

    /// Governed promotion requiring a signed evaluator report and minimum dwell time (`q23.4`, `FA-INV-036`).
    pub fn promote_with_evaluator_report(
        &mut self,
        target_grade: AutonomyGrade,
        evaluator_principal: &str,
        evaluator_signature: [u8; 32],
        current_control_seq: u64,
        min_dwell_seqs: u64,
        keep_audit_floor_active: bool,
    ) -> Result<(), AutonomyError> {
        if evaluator_principal.starts_with("actor:") {
            return Err(AutonomyError::ActorSelfSummaryForbiddenForLedger);
        }
        if evaluator_signature == [0u8; 32] {
            return Err(AutonomyError::UnsignedEvaluatorReport);
        }
        if !keep_audit_floor_active {
            return Err(AutonomyError::AuditFloorCannotBeSkippedAtBoundedGrade);
        }
        let elapsed = current_control_seq.saturating_sub(self.since_control_seq);
        if elapsed < min_dwell_seqs {
            return Err(AutonomyError::InsufficientDwellTime {
                elapsed_seqs: elapsed,
                minimum_required: min_dwell_seqs,
            });
        }
        if (target_grade as u8) > (self.grade as u8) + 1 {
            return Err(AutonomyError::PromotionAboveSingleStepForbidden);
        }
        self.grade = target_grade;
        self.since_control_seq = current_control_seq;
        self.audit_floor_active = true;
        Ok(())
    }
}

// ============================================================================
// FA-125: Risk-theater detector: standing queries over the control plane
// ============================================================================

/// The six standing query families of the risk-theater detector (`iew.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskTheaterFamily {
    ConsequenceDrift,
    AndonClosureMode,
    RosterChurnNearHolds,
    CoverageErosion,
    AutonomyInflation,
    AuditFloorStarvation,
}

impl RiskTheaterFamily {
    pub const ALL: [Self; 6] = [
        Self::ConsequenceDrift,
        Self::AndonClosureMode,
        Self::RosterChurnNearHolds,
        Self::CoverageErosion,
        Self::AutonomyInflation,
        Self::AuditFloorStarvation,
    ];
}

/// Canonical `-old, +new` control-plane mutation tuple fed to the risk-theater detector (`iew.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneDelta {
    pub frontier_seq: u64,
    pub family: RiskTheaterFamily,
    pub key: String,
    /// A nonzero signed Z-set weight. Retractions must name the exact live tuple
    /// and cannot make its retained multiplicity negative.
    pub multiplicity: i64,
    /// The nonnegative metric VALUE of the tuple, not a replacement value on
    /// deletion. A change is an exact `-old, +new` pair.
    pub metric_delta: i64,
    pub support_address: Address,
}

/// A complete ordered input boundary for the bounded reference detector.
///
/// Sequences start at one and are contiguous, including empty boundaries. All
/// tuples in a boundary carry its sequence; multiple changes at that sequence
/// are allowed. `closed` is an assertion by the independent input owner, not a
/// completeness proof manufactured by this reference projection. Native journal
/// ingestion and authenticated closure remain separate integration obligations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneFrontier {
    pub frontier_seq: u64,
    pub closed: bool,
    pub deltas: Vec<ControlPlaneDelta>,
}

pub const MAX_RISK_THEATER_SUPPORTS: usize = 512;
pub const MAX_RISK_THEATER_FRONTIER_DELTAS: usize = 256;
pub const MAX_RISK_THEATER_COMPONENT_BYTES: usize = 256;
pub const MAX_RISK_THEATER_HISTORY_FRONTIERS: usize = 1024;
pub const MAX_RISK_THEATER_HISTORY_DELTAS: usize = 4096;

/// Refusal is distinct from a complete, empty finding set. Every failed update
/// leaves the preceding closed snapshot intact, including its exact sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RiskTheaterError {
    IncompleteThresholds,
    NonpositiveThreshold,
    UnclosedFrontier,
    NoClosedFrontier,
    UnexpectedFrontier { expected: u64, actual: u64 },
    DeltaFrontierMismatch,
    MissingHistoryFrontier,
    FrontierExhausted,
    FrontierDeltaLimit,
    HistoryLimit,
    SupportLimit,
    InvalidKey,
    InvalidSupportAddress,
    ZeroMultiplicity,
    NegativeMetric,
    MissingRetractionSupport,
    TupleMismatch,
    NegativeMultiplicity,
    MultiplicityOverflow,
    ScoreOverflow,
    OracleMismatch,
}

/// Exact retained support, including weights below the finding threshold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskTheaterSupport {
    pub family: RiskTheaterFamily,
    pub key: String,
    pub multiplicity: i64,
    pub metric_value: i64,
    pub support_address: Address,
}

impl NotActorVisible for RiskTheaterSupport {}

/// Support-backed threshold crossing for a separate governance incident consumer
/// (`iew.4`). Marked [`NotActorVisible`]; this value neither opens a native incident
/// nor supplies a policy write path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskTheaterFinding {
    pub family: RiskTheaterFamily,
    pub current_score: i64,
    pub threshold: i64,
    pub support_addresses: Vec<Address>,
}

impl NotActorVisible for RiskTheaterFinding {}

/// Complete bounded projection at one explicitly closed input frontier.
///
/// Thresholds are immutable configuration of this reference evaluator; this
/// type does not claim that their source has native governance authority. Scores
/// and supports for ALL six families are retained even when no finding opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskTheaterSnapshot {
    pub frontier_seq: u64,
    pub thresholds: BTreeMap<RiskTheaterFamily, i64>,
    pub scores: BTreeMap<RiskTheaterFamily, i64>,
    pub supports: Vec<RiskTheaterSupport>,
    pub findings: Vec<RiskTheaterFinding>,
}

impl NotActorVisible for RiskTheaterSnapshot {}

fn validate_risk_theater_thresholds(
    thresholds: &BTreeMap<RiskTheaterFamily, i64>,
) -> Result<(), RiskTheaterError> {
    if RiskTheaterFamily::ALL
        .iter()
        .any(|family| !thresholds.contains_key(family))
    {
        return Err(RiskTheaterError::IncompleteThresholds);
    }
    if thresholds.values().any(|threshold| *threshold <= 0) {
        return Err(RiskTheaterError::NonpositiveThreshold);
    }
    Ok(())
}

fn validate_risk_theater_delta(delta: &ControlPlaneDelta) -> Result<(), RiskTheaterError> {
    if delta.key.is_empty() || delta.key.len() > MAX_RISK_THEATER_COMPONENT_BYTES {
        return Err(RiskTheaterError::InvalidKey);
    }
    let address = &delta.support_address;
    // Address has public fields. Revalidate without formatting or cloning an
    // unbounded string; an otherwise valid, oversized address is also refused.
    let valid_component = |value: &str| {
        !value.is_empty()
            && value.len() <= MAX_RISK_THEATER_COMPONENT_BYTES
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    };
    if !valid_component(&address.tenant) || !valid_component(&address.id) {
        return Err(RiskTheaterError::InvalidSupportAddress);
    }
    if delta.multiplicity == 0 {
        return Err(RiskTheaterError::ZeroMultiplicity);
    }
    if delta.metric_delta < 0 {
        return Err(RiskTheaterError::NegativeMetric);
    }
    Ok(())
}

/// Incremental risk-theater score/support kernel (`iew.3`, `iew.4`).
///
/// Updates commit atomically at consecutive closed frontiers. This is a bounded
/// reference projection, not the six native ledger query definitions, a journal
/// completeness verifier, an incident writer, or an actor authorization filter.
/// The previous unbounded tuple-at-a-time API is deliberately replaced: it could
/// publish partial results, overflow, and remove a support with the wrong value.
#[derive(Clone, Debug)]
pub struct RiskTheaterDetector {
    thresholds: BTreeMap<RiskTheaterFamily, i64>,
    scores: BTreeMap<RiskTheaterFamily, i64>,
    active_supports: BTreeMap<(RiskTheaterFamily, String), RiskTheaterSupport>,
    closed_frontier: u64,
}

impl RiskTheaterDetector {
    pub fn new(thresholds: BTreeMap<RiskTheaterFamily, i64>) -> Result<Self, RiskTheaterError> {
        validate_risk_theater_thresholds(&thresholds)?;
        Ok(Self {
            thresholds,
            scores: RiskTheaterFamily::ALL
                .into_iter()
                .map(|family| (family, 0))
                .collect(),
            active_supports: BTreeMap::new(),
            closed_frontier: 0,
        })
    }

    pub fn closed_frontier(&self) -> Option<u64> {
        (self.closed_frontier != 0).then_some(self.closed_frontier)
    }

    /// Apply the complete next boundary or retain the whole previous snapshot.
    ///
    /// Individual tuple multiplicities and contributions must fit i64; family
    /// scores must fit i64 at closure. The staging map is bounded by the previous
    /// support limit plus the input batch limit, permitting insert/delete pairs
    /// without imposing a spurious intermediate support-capacity restriction.
    pub fn apply_frontier(
        &mut self,
        frontier: &ControlPlaneFrontier,
    ) -> Result<RiskTheaterSnapshot, RiskTheaterError> {
        if !frontier.closed {
            return Err(RiskTheaterError::UnclosedFrontier);
        }
        let expected = self
            .closed_frontier
            .checked_add(1)
            .ok_or(RiskTheaterError::FrontierExhausted)?;
        if frontier.frontier_seq != expected {
            return Err(RiskTheaterError::UnexpectedFrontier {
                expected,
                actual: frontier.frontier_seq,
            });
        }
        if frontier.deltas.len() > MAX_RISK_THEATER_FRONTIER_DELTAS {
            return Err(RiskTheaterError::FrontierDeltaLimit);
        }
        for delta in &frontier.deltas {
            if delta.frontier_seq != frontier.frontier_seq {
                return Err(RiskTheaterError::DeltaFrontierMismatch);
            }
            validate_risk_theater_delta(delta)?;
        }

        let mut supports = self.active_supports.clone();
        let mut scores: BTreeMap<_, _> = self
            .scores
            .iter()
            .map(|(family, score)| (*family, i128::from(*score)))
            .collect();
        for delta in &frontier.deltas {
            let key = (delta.family, delta.key.clone());
            let old = supports.get(&key);
            if let Some(old) = old {
                if old.metric_value != delta.metric_delta
                    || old.support_address != delta.support_address
                {
                    return Err(RiskTheaterError::TupleMismatch);
                }
            } else if delta.multiplicity < 0 {
                return Err(RiskTheaterError::MissingRetractionSupport);
            }
            let previous_weight = old.map_or(0, |support| support.multiplicity);
            let next_weight = previous_weight
                .checked_add(delta.multiplicity)
                .ok_or(RiskTheaterError::MultiplicityOverflow)?;
            if next_weight < 0 {
                return Err(RiskTheaterError::NegativeMultiplicity);
            }
            let previous_contribution = previous_weight
                .checked_mul(delta.metric_delta)
                .ok_or(RiskTheaterError::ScoreOverflow)?;
            let next_contribution = next_weight
                .checked_mul(delta.metric_delta)
                .ok_or(RiskTheaterError::ScoreOverflow)?;
            let score = scores.get_mut(&delta.family).expect("complete family map");
            *score = score
                .checked_add(i128::from(next_contribution) - i128::from(previous_contribution))
                .ok_or(RiskTheaterError::ScoreOverflow)?;
            if next_weight == 0 {
                supports.remove(&key);
            } else {
                supports.insert(
                    key,
                    RiskTheaterSupport {
                        family: delta.family,
                        key: delta.key.clone(),
                        multiplicity: next_weight,
                        metric_value: delta.metric_delta,
                        support_address: delta.support_address.clone(),
                    },
                );
            }
        }
        if supports.len() > MAX_RISK_THEATER_SUPPORTS {
            return Err(RiskTheaterError::SupportLimit);
        }
        let scores = scores
            .into_iter()
            .map(|(family, score)| {
                i64::try_from(score)
                    .map(|score| (family, score))
                    .map_err(|_| RiskTheaterError::ScoreOverflow)
            })
            .collect::<Result<_, _>>()?;
        let candidate = Self {
            thresholds: self.thresholds.clone(),
            scores,
            active_supports: supports,
            closed_frontier: frontier.frontier_seq,
        };
        let snapshot = candidate.snapshot(frontier.frontier_seq)?;
        *self = candidate;
        Ok(snapshot)
    }

    /// An explicit requested sequence prevents an old empty result from being
    /// mistaken for a successful query at a rejected or incomplete new frontier.
    pub fn snapshot(
        &self,
        expected_closed_frontier: u64,
    ) -> Result<RiskTheaterSnapshot, RiskTheaterError> {
        if self.closed_frontier == 0 {
            return Err(RiskTheaterError::NoClosedFrontier);
        }
        if self.closed_frontier != expected_closed_frontier {
            return Err(RiskTheaterError::UnexpectedFrontier {
                expected: self.closed_frontier,
                actual: expected_closed_frontier,
            });
        }
        let mut findings = Vec::new();
        for family in RiskTheaterFamily::ALL {
            let score = self.scores[&family];
            let threshold = self.thresholds[&family];
            if score >= threshold {
                let support_addresses = self
                    .active_supports
                    .values()
                    .filter(|support| support.family == family)
                    .map(|support| support.support_address.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                findings.push(RiskTheaterFinding {
                    family,
                    current_score: score,
                    threshold,
                    support_addresses,
                });
            }
        }
        Ok(RiskTheaterSnapshot {
            frontier_seq: self.closed_frontier,
            thresholds: self.thresholds.clone(),
            scores: self.scores.clone(),
            supports: self.active_supports.values().cloned().collect(),
            findings,
        })
    }

    pub fn evaluate_findings(
        &self,
        expected_closed_frontier: u64,
    ) -> Result<Vec<RiskTheaterFinding>, RiskTheaterError> {
        self.snapshot(expected_closed_frontier)
            .map(|snapshot| snapshot.findings)
    }

    /// Compare against a separately supplied complete history, including scores
    /// and below-threshold supports. A lost retraction cannot hide merely because
    /// neither projection currently crosses a threshold. This cannot detect a
    /// fact omitted from BOTH inputs or authenticate their independent origin.
    pub fn verify_against_history(
        &self,
        history: &[ControlPlaneFrontier],
    ) -> Result<RiskTheaterSnapshot, RiskTheaterError> {
        let snapshot = self.snapshot(self.closed_frontier)?;
        let oracle =
            recompute_risk_theater_batch_oracle(&self.thresholds, history, self.closed_frontier)?;
        if snapshot != oracle {
            return Err(RiskTheaterError::OracleMismatch);
        }
        Ok(snapshot)
    }
}

/// Independent batch oracle over an explicitly closed history prefix (`iew.3`).
///
/// Unlike the incremental score-delta algorithm, this groups source tuples by
/// key, reconstructs each key's live value from the beginning at EVERY closed
/// frontier, and sums the resulting relation from scratch. No detector method or
/// incremental reducer is called. It checks every intermediate closed boundary,
/// so later deletion cannot hide an earlier overflow or support-limit violation.
///
/// The requested prefix must contain sequences 1 through `up_to_frontier`;
/// later suffixes are irrelevant to that historical result. At most 1024
/// frontiers and 4096 deltas are examined (at most their product in reconstruction
/// visits), independently of the incremental detector's bounded retained state.
pub fn recompute_risk_theater_batch_oracle(
    thresholds: &BTreeMap<RiskTheaterFamily, i64>,
    history: &[ControlPlaneFrontier],
    up_to_frontier: u64,
) -> Result<RiskTheaterSnapshot, RiskTheaterError> {
    validate_risk_theater_thresholds(thresholds)?;
    if up_to_frontier == 0 {
        return Err(RiskTheaterError::NoClosedFrontier);
    }
    if up_to_frontier > MAX_RISK_THEATER_HISTORY_FRONTIERS as u64 {
        return Err(RiskTheaterError::HistoryLimit);
    }
    let prefix = history
        .get(..up_to_frontier as usize)
        .ok_or(RiskTheaterError::MissingHistoryFrontier)?;
    let mut total_deltas = 0usize;
    let mut by_key: BTreeMap<(RiskTheaterFamily, &str), Vec<&ControlPlaneDelta>> = BTreeMap::new();
    for (index, frontier) in prefix.iter().enumerate() {
        let expected = index as u64 + 1;
        if frontier.frontier_seq != expected {
            return Err(RiskTheaterError::UnexpectedFrontier {
                expected,
                actual: frontier.frontier_seq,
            });
        }
        if !frontier.closed {
            return Err(RiskTheaterError::UnclosedFrontier);
        }
        if frontier.deltas.len() > MAX_RISK_THEATER_FRONTIER_DELTAS {
            return Err(RiskTheaterError::FrontierDeltaLimit);
        }
        total_deltas = total_deltas
            .checked_add(frontier.deltas.len())
            .ok_or(RiskTheaterError::HistoryLimit)?;
        if total_deltas > MAX_RISK_THEATER_HISTORY_DELTAS {
            return Err(RiskTheaterError::HistoryLimit);
        }
        for delta in &frontier.deltas {
            if delta.frontier_seq != frontier.frontier_seq {
                return Err(RiskTheaterError::DeltaFrontierMismatch);
            }
            validate_risk_theater_delta(delta)?;
            by_key
                .entry((delta.family, delta.key.as_str()))
                .or_default()
                .push(delta);
        }
    }

    let mut result = None;
    for frontier in prefix {
        let mut supports = Vec::new();
        let mut sums: BTreeMap<_, i128> = thresholds.keys().map(|family| (*family, 0)).collect();
        for ((family, key), changes) in &by_key {
            let mut count = 0i128;
            let mut live: Option<&ControlPlaneDelta> = None;
            for change in changes {
                if change.frontier_seq > frontier.frontier_seq {
                    break;
                }
                if let Some(tuple) = live {
                    if tuple.metric_delta != change.metric_delta
                        || tuple.support_address != change.support_address
                    {
                        return Err(RiskTheaterError::TupleMismatch);
                    }
                } else if change.multiplicity < 0 {
                    return Err(RiskTheaterError::MissingRetractionSupport);
                }
                count = count
                    .checked_add(i128::from(change.multiplicity))
                    .ok_or(RiskTheaterError::MultiplicityOverflow)?;
                if count < 0 {
                    return Err(RiskTheaterError::NegativeMultiplicity);
                }
                if count > i128::from(i64::MAX) {
                    return Err(RiskTheaterError::MultiplicityOverflow);
                }
                let contribution = count
                    .checked_mul(i128::from(change.metric_delta))
                    .ok_or(RiskTheaterError::ScoreOverflow)?;
                if contribution > i128::from(i64::MAX) {
                    return Err(RiskTheaterError::ScoreOverflow);
                }
                live = if count == 0 { None } else { Some(*change) };
            }
            if let Some(tuple) = live {
                if supports.len() == MAX_RISK_THEATER_SUPPORTS {
                    return Err(RiskTheaterError::SupportLimit);
                }
                let contribution = count
                    .checked_mul(i128::from(tuple.metric_delta))
                    .ok_or(RiskTheaterError::ScoreOverflow)?;
                let sum = sums.get_mut(family).expect("complete family map");
                *sum = sum
                    .checked_add(contribution)
                    .ok_or(RiskTheaterError::ScoreOverflow)?;
                supports.push(RiskTheaterSupport {
                    family: *family,
                    key: (*key).to_owned(),
                    multiplicity: count as i64,
                    metric_value: tuple.metric_delta,
                    support_address: tuple.support_address.clone(),
                });
            }
        }
        let mut scores = BTreeMap::new();
        let mut findings = Vec::new();
        for (family, threshold) in thresholds {
            let score = i64::try_from(sums[family]).map_err(|_| RiskTheaterError::ScoreOverflow)?;
            scores.insert(*family, score);
            if score >= *threshold {
                let mut addresses = BTreeSet::new();
                for support in &supports {
                    if support.family == *family {
                        addresses.insert(support.support_address.clone());
                    }
                }
                findings.push(RiskTheaterFinding {
                    family: *family,
                    current_score: score,
                    threshold: *threshold,
                    support_addresses: addresses.into_iter().collect(),
                });
            }
        }
        result = Some(RiskTheaterSnapshot {
            frontier_seq: frontier.frontier_seq,
            thresholds: thresholds.clone(),
            scores,
            supports,
            findings,
        });
    }
    result.ok_or(RiskTheaterError::NoClosedFrontier)
}

// ============================================================================
// FA-131: Attack-capsule interchange (plan §16.6)
// ============================================================================

/// Portable attack-capsule specification (`sez.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackCapsule {
    pub capsule_id: String,
    pub version: u32,
    pub mechanism: String,
    pub information_set: String,
    pub budget: u64,
    pub target_profile_id: String,
    pub family: String,
    pub generator_ref: Option<String>,
    pub generator_requests_real_egress_or_credentials: bool,
    pub lineage_partition: String,
    pub result_receipt_digest: Option<[u8; 32]>,
    pub redaction_notes: String,
}

impl AttackCapsule {
    /// Descriptive vs evaluable flag is derived strictly from generator presence (`sez.1`).
    pub const fn is_evaluable(&self) -> bool {
        self.generator_ref.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapsuleError {
    GeneratorAttemptsRealEffectOrEgress,
    NotHeldOutPartition { partition: String },
    ExportPermissionDenied,
    UnredactedSecretDetected,
    MissingResultReceiptOnExport,
}

/// Import an [`AttackCapsule`] inside `&ExperimentCx` (`sez.2`).
/// Enforces held-out lineage and rejects any generator that could reach real egress or credentials.
pub fn import_attack_capsule(
    _cx: &ExperimentCx,
    capsule: &AttackCapsule,
) -> Result<Address, CapsuleError> {
    if capsule.lineage_partition != "held_out_eval" {
        return Err(CapsuleError::NotHeldOutPartition {
            partition: capsule.lineage_partition.clone(),
        });
    }
    if capsule.generator_requests_real_egress_or_credentials {
        return Err(CapsuleError::GeneratorAttemptsRealEffectOrEgress);
    }
    Ok(Address::new("org", Noun::Branch, &capsule.capsule_id, Some(1)).expect("capsule address"))
}

/// Export an [`AttackCapsule`] with operator permission, secret redaction, and attached receipt (`sez.3`).
pub fn export_attack_capsule(
    mut capsule: AttackCapsule,
    operator_permission_granted: bool,
    raw_notes: &str,
) -> Result<AttackCapsule, CapsuleError> {
    if !operator_permission_granted {
        return Err(CapsuleError::ExportPermissionDenied);
    }
    if capsule.result_receipt_digest.is_none() {
        return Err(CapsuleError::MissingResultReceiptOnExport);
    }
    let redacted = raw_notes.replace("SECRET_TOKEN_", "[REDACTED]");
    if redacted.contains("SECRET_TOKEN_") {
        return Err(CapsuleError::UnredactedSecretDetected);
    }
    capsule.redaction_notes = redacted;
    Ok(capsule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typestate_cx::RootCx;

    #[test]
    fn assurance_profile_doctor_and_witness_cached_precheck_enforce_all_boundaries() {
        let mut mediation = BTreeMap::new();
        mediation.insert("net.egress".to_owned(), "brokered".to_owned());

        let mut profile = AssuranceProfile {
            profile_id: "prof-strict".to_owned(),
            version: 1,
            mediation_by_family: mediation,
            coverage_contract_ids: vec!["cov-1".to_owned()],
            replay_grade: "decision_replay".to_owned(),
            roster_class: "independent_cohort".to_owned(),
            autonomy_floor: AutonomyGrade::Propose,
            policy_disclosure: PolicyDisclosureLevel::ExactPredicates,
            leakage_table: None,
            claims: vec![
                ProfileClaimKind::BrokeredMediation,
                ProfileClaimKind::DecisionReplayGrade,
            ],
            operator_signature: [0x11; 32],
        };

        // 1. ExactPredicates disclosure without a leakage table is refused.
        assert_eq!(
            profile.validate(),
            Err(ProfileError::MissingLeakageTableForDisclosure {
                level: PolicyDisclosureLevel::ExactPredicates
            })
        );

        profile.leakage_table = Some(LeakageTable {
            bits_per_precheck_ceiling: 1,
            disclosed_predicate_families: vec!["net.egress".to_owned()],
            rate_limit_per_window: 64,
        });
        profile.validate().expect("valid profile with leakage table");

        // 2. fa_doctor_profile refuses Skipped or NotExecuted claims (`SKIP` is never `PASS`).
        let mut reg = BTreeMap::new();
        reg.insert(
            ProfileClaimKind::BrokeredMediation,
            vec!["test_bypass_campaign"],
        );
        reg.insert(
            ProfileClaimKind::DecisionReplayGrade,
            vec!["test_replay_restart"],
        );

        let mut executed = BTreeMap::new();
        executed.insert("test_bypass_campaign", ClaimTestExecutionStatus::Passed);
        executed.insert("test_replay_restart", ClaimTestExecutionStatus::Skipped);
        assert_eq!(
            fa_doctor_profile(&profile, &reg, &executed),
            Err(ProfileError::UnexecutedOrFailedClaimCannotCertify {
                claim: ProfileClaimKind::DecisionReplayGrade,
                status: "skipped",
            })
        );

        executed.insert("test_replay_restart", ClaimTestExecutionStatus::Passed);
        let cert = fa_doctor_profile(&profile, &reg, &executed).expect("all required tests passed");
        assert_eq!(cert.certified_claims.len(), 2);

        // 3. Witness-cached precheck emits no journal events or helper calls and invalidates on epoch bump.
        let journal = DomainEventJournal::new_production("org", "dom-precheck");
        let mut precheck = WitnessCachedPrecheckEngine::new(1);
        let (res1, cached1) = precheck
            .fa_precheck(&profile, [0xAA; 32], "net.egress", true, true, 10, &journal)
            .expect("first precheck");
        assert!(!cached1);
        assert_eq!(
            res1,
            PrecheckResult::PrecheckPassed {
                review_required: true
            }
        );

        let (res2, cached2) = precheck
            .fa_precheck(&profile, [0xAA; 32], "net.egress", true, true, 10, &journal)
            .expect("second precheck hits witness cache");
        assert!(cached2);
        assert_eq!(res1, res2);
        assert_eq!(journal.len(), 0);
        assert_eq!(precheck.helper_calls_made(), 0);

        precheck.advance_policy_epoch(2);
        let (_, cached_after_epoch) = precheck
            .fa_precheck(&profile, [0xAA; 32], "net.egress", true, true, 10, &journal)
            .expect("post-epoch precheck recomputes");
        assert!(!cached_after_epoch);
    }

    #[test]
    fn control_receipts_verifier_overhead_and_graduated_autonomy_fail_closed() {
        // 1. Unadmitted dispatch or actor-signed receipt is refused.
        assert_eq!(
            issue_dispatch_receipt("r-0", 1, false, [0x11; 32], "prof-1", "operator:1", vec![]),
            Err(ReceiptError::UnadmittedEffectCannotMintReceipt)
        );
        assert_eq!(
            issue_dispatch_receipt("r-0", 1, true, [0x11; 32], "prof-1", "actor:main", vec![]),
            Err(ReceiptError::ActorKeyForbiddenForReceiptSigning)
        );

        let receipt = issue_dispatch_receipt(
            "r-1",
            10,
            true,
            [0x22; 32],
            "prof-1",
            "operator:root",
            vec!["pii_field".to_owned()],
        )
        .expect("valid receipt");

        let mut verifier = IndependentReceiptVerifier::new();
        let report = verifier
            .fa_verify(&receipt, 10)
            .expect("first verification passes");
        assert_eq!(report.redacted_properties, vec!["pii_field"]);

        // Replaying the same receipt for a second dispatch fails closed.
        assert_eq!(
            verifier.fa_verify(&receipt, 10),
            Err(ReceiptError::ReplayedReceiptForSecondDispatch {
                receipt_id: "r-1".to_owned()
            })
        );

        // 2. Control-overhead accounting preserves Held/Denied/Unknown outcomes separately.
        let records = vec![
            ControlCostRecord {
                task_id: "t-1".to_owned(),
                contract_generation: 1,
                actor_inference_cost: 100,
                components: ControlCostComponents {
                    capture: 5,
                    helper_inference: 10,
                    gate_cpu: 5,
                    ..Default::default()
                },
                outcome: CostOutcomeClass::UsefulCompleted,
            },
            ControlCostRecord {
                task_id: "t-2".to_owned(),
                contract_generation: 1,
                actor_inference_cost: 80,
                components: ControlCostComponents {
                    capture: 5,
                    helper_inference: 15,
                    ..Default::default()
                },
                outcome: CostOutcomeClass::Held,
            },
        ];
        let summary = summarize_control_overhead(&records);
        assert_eq!(summary.useful_completed_count, 1);
        assert_eq!(summary.held_count, 1);
        assert_eq!(summary.total_control_cost_all_outcomes, 40);
        assert_eq!(summary.overhead_ratio_bps_per_useful_task, 4_000);

        // 3. Graduated autonomy: dwell-time enforcement, immediate demotion, and passport reset.
        let mut state =
            AutonomyState::new("actor-1", 1, "fs.write", AutonomyGrade::Propose, 10, [0x01; 32]);
        assert_eq!(
            state.promote_with_evaluator_report(
                AutonomyGrade::Reviewed,
                "evaluator:1",
                [0x99; 32],
                15,
                20,
                true,
            ),
            Err(AutonomyError::InsufficientDwellTime {
                elapsed_seqs: 5,
                minimum_required: 20,
            })
        );
        state
            .promote_with_evaluator_report(
                AutonomyGrade::Reviewed,
                "evaluator:1",
                [0x99; 32],
                35,
                20,
                true,
            )
            .expect("promotion after dwell succeeds");
        assert_eq!(state.grade, AutonomyGrade::Reviewed);

        // Passport change immediately resets grade to floor.
        state.on_passport_change([0x02; 32], 40);
        assert_eq!(state.grade, AutonomyGrade::Propose);
    }

    #[test]
    fn risk_theater_batch_oracle_equality_and_attack_capsule_isolation() {
        let thresholds = RiskTheaterFamily::ALL
            .into_iter()
            .map(|family| (family, 10))
            .collect::<BTreeMap<_, _>>();
        let mut detector =
            RiskTheaterDetector::new(thresholds.clone()).expect("complete thresholds");
        let addr = Address::new("org", Noun::Finding, "rt-1", Some(1)).expect("addr");

        let deltas = vec![
            ControlPlaneDelta {
                frontier_seq: 1,
                family: RiskTheaterFamily::ConsequenceDrift,
                key: "k1".to_owned(),
                multiplicity: 1,
                metric_delta: 12,
                support_address: addr.clone(),
            },
            ControlPlaneDelta {
                frontier_seq: 2,
                family: RiskTheaterFamily::ConsequenceDrift,
                key: "k1".to_owned(),
                multiplicity: -1,
                metric_delta: 12,
                support_address: addr,
            },
        ];

        let history = deltas
            .into_iter()
            .map(|delta| ControlPlaneFrontier {
                frontier_seq: delta.frontier_seq,
                closed: true,
                deltas: vec![delta],
            })
            .collect::<Vec<_>>();
        let first = detector.apply_frontier(&history[0]).expect("insert");
        assert_eq!(
            first,
            recompute_risk_theater_batch_oracle(&thresholds, &history, 1).expect("batch insert")
        );
        assert_eq!(detector.evaluate_findings(1).expect("closed").len(), 1);

        let second = detector
            .apply_frontier(&history[1])
            .expect("exact retraction");
        assert_eq!(
            second,
            recompute_risk_theater_batch_oracle(&thresholds, &history, 2)
                .expect("batch retraction")
        );
        assert!(detector.evaluate_findings(2).expect("closed").is_empty());

        // Attack capsule import/export under ExperimentCx.
        let root = RootCx::new_nucleus(1, 1, 1);
        let exp_cx = root.mint_experiment(55).expect("exp cx");
        let mut bad_capsule = AttackCapsule {
            capsule_id: "cap-1".to_owned(),
            version: 1,
            mechanism: "prompt_injection".to_owned(),
            information_set: "decisions_only".to_owned(),
            budget: 10,
            target_profile_id: "prof-strict".to_owned(),
            family: "net.egress".to_owned(),
            generator_ref: Some("gen-1".to_owned()),
            generator_requests_real_egress_or_credentials: true,
            lineage_partition: "held_out_eval".to_owned(),
            result_receipt_digest: Some([0x77; 32]),
            redaction_notes: String::new(),
        };
        assert!(bad_capsule.is_evaluable());
        assert_eq!(
            import_attack_capsule(&exp_cx, &bad_capsule),
            Err(CapsuleError::GeneratorAttemptsRealEffectOrEgress)
        );

        bad_capsule.generator_requests_real_egress_or_credentials = false;
        import_attack_capsule(&exp_cx, &bad_capsule).expect("safe generator imports cleanly");

        assert_eq!(
            export_attack_capsule(bad_capsule.clone(), false, "notes"),
            Err(CapsuleError::ExportPermissionDenied)
        );
        let exported =
            export_attack_capsule(bad_capsule, true, "used SECRET_TOKEN_123 in fixture")
                .expect("export with permission and redaction");
        assert!(!exported.redaction_notes.contains("SECRET_TOKEN_"));
        assert!(exported.redaction_notes.contains("[REDACTED]"));
    }
}
