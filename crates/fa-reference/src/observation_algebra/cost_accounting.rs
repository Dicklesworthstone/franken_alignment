//! Bounded observation cost accounting and evidence-debt priority (FA-071, plan §§14.6, 14.9, 18.1, 18.7).
//!
//! Accounts for concrete observation obligations—transfer bytes, witness
//! refinement bytes, retained pins, receipt/revocation reconciliation work, and
//! existing evidence debt—linked to validated physical plan summaries (`FA-069`)
//! and rewrite certificates (`FA-070`).
//!
//! Enforces:
//! - Concrete vector accounting (never a metaphysical risk scalar);
//! - Strict priority for existing obligations and reconciliation before new or
//!   optional optimization work;
//! - Separate audit-floor funding so adversarial load cannot starve audits;
//! - Anti-amplification ceilings on forced refinement rounds;
//! - No refund of unknown in-flight effects (`FA-INV-006`).

use std::collections::BTreeMap;

use super::{Basis, Knowledge, physical_plan::RehearsedPlanSummary};

pub const MAX_TRACKED_ATTEMPTS: usize = 64;

/// Scheduling priority tiers from plan §14.9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservationWorkPriority {
    /// Highest priority: revocation/receipt reconciliation and existing evidence debt.
    ExistingObligationReconciliation,
    /// Separately funded randomized audit floor (§14.4).
    MandatoryAuditFloor,
    /// Required observation plan execution for an active proposal.
    RequiredPlanObservation,
    /// Lowest priority: optional optimization or speculative VOI exploration.
    OptionalOptimization,
}

/// Concrete multi-resource capacities for an observation cost pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationCostLimits {
    pub max_transfer_bytes: u64,
    pub max_refinement_bytes: u64,
    pub max_retained_pins: u64,
    pub max_reconciliation_units: u64,
    pub max_obligation_debt_units: u64,
    pub reserved_audit_floor_units: u64,
    pub max_refinement_rounds_per_attempt: u32,
}

/// Concrete resource charge vector for one observation operation or attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObservationCharge {
    pub transfer_bytes: u64,
    pub refinement_bytes: u64,
    pub retained_pins: u64,
    pub reconciliation_units: u64,
    pub obligation_debt_units: u64,
    pub audit_floor_units: u64,
    pub refinement_rounds: u32,
}

/// External effect/dispatch state used to enforce `FA-INV-006` during cancellation
/// or reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectDisposition {
    /// Attempt was never dispatched to an external adapter; reservation may be
    /// released after charging reconciliation work.
    AbortedBeforeDispatch,
    /// Dispatch outcome is unknown (timeout, crash, or lost receipt); charges and
    /// pins must remain held (`FA-INV-006`).
    UnknownInFlight,
    /// Effect is confirmed executed; charges are permanent.
    ConfirmedExecuted,
}

/// Realized execution and cold/failure telemetry for an observation attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RealizedObservationTelemetry {
    pub realized_transfer_bytes: u64,
    pub realized_refinement_bytes: u64,
    pub realized_reconciliation_units: u64,
    pub queue_delay_ms: u64,
    pub recovery_bytes: u64,
    pub retained_evidence_bytes: u64,
    pub cold_start: bool,
    pub attack_conditioned: bool,
    pub admitted_useful_effect: bool,
}

/// Typed refusal reasons for bounded observation cost accounting (FA-071).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostAccountingError {
    InvalidLimits,
    AttemptLimitExceeded,
    DuplicateAttemptId(u64),
    UnknownAttemptId(u64),
    ArithmeticOverflow,
    UnfundedExistingObligationDebt {
        outstanding_debt_units: u64,
        pending_reconciliations: usize,
    },
    MissingRequiredPinCharge {
        required_pins: u64,
        proposed_pins: u64,
    },
    ForcedRefinementAmplification {
        rounds: u32,
        max_allowed: u32,
    },
    UnderchargedPlanEnvelope,
    TransferBudgetExceeded,
    RefinementBudgetExceeded,
    PinBudgetExceeded,
    ReconciliationBudgetExceeded,
    ObligationDebtCeilingExceeded,
    AuditFloorBudgetExceeded,
    UnknownEffectRefundForbidden {
        attempt_id: u64,
    },
    ExecutedEffectRefundForbidden {
        attempt_id: u64,
    },
}

/// Immutable snapshot of accounted observation costs, denominators, and
/// cold/failure telemetry (`FA-071`).
///
/// An accounting report is an observation/planning value, never an execution
/// permit:
///
/// ```compile_fail,E0599
/// use fa_reference::observation_algebra::cost_accounting::ObservationCostReport;
/// fn forge_permit(report: ObservationCostReport) {
///     let _ = report.into_permit();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationCostReport {
    basis: Basis,
    used_transfer_bytes: u64,
    used_refinement_bytes: u64,
    active_retained_pins: u64,
    used_reconciliation_units: u64,
    outstanding_obligation_debt_units: u64,
    used_audit_floor_units: u64,
    unconditional_attempts: u64,
    attack_conditioned_attempts: u64,
    admitted_useful_effects: u64,
    held_or_denied_attempts: u64,
    unresolved_in_flight_attempts: u64,
    cold_start_attempts: u64,
    total_queue_delay_ms: u64,
    total_recovery_bytes: u64,
    total_retained_evidence_bytes: u64,
}

impl ObservationCostReport {
    pub const fn basis(&self) -> Basis {
        self.basis
    }

    pub const fn used_transfer_bytes(&self) -> u64 {
        self.used_transfer_bytes
    }

    pub const fn used_refinement_bytes(&self) -> u64 {
        self.used_refinement_bytes
    }

    pub const fn active_retained_pins(&self) -> u64 {
        self.active_retained_pins
    }

    pub const fn used_reconciliation_units(&self) -> u64 {
        self.used_reconciliation_units
    }

    pub const fn outstanding_obligation_debt_units(&self) -> u64 {
        self.outstanding_obligation_debt_units
    }

    pub const fn used_audit_floor_units(&self) -> u64 {
        self.used_audit_floor_units
    }

    pub const fn unconditional_attempts(&self) -> u64 {
        self.unconditional_attempts
    }

    pub const fn attack_conditioned_attempts(&self) -> u64 {
        self.attack_conditioned_attempts
    }

    pub const fn admitted_useful_effects(&self) -> u64 {
        self.admitted_useful_effects
    }

    pub const fn held_or_denied_attempts(&self) -> u64 {
        self.held_or_denied_attempts
    }

    pub const fn unresolved_in_flight_attempts(&self) -> u64 {
        self.unresolved_in_flight_attempts
    }

    pub const fn cold_start_attempts(&self) -> u64 {
        self.cold_start_attempts
    }

    pub const fn total_queue_delay_ms(&self) -> u64 {
        self.total_queue_delay_ms
    }

    pub const fn total_recovery_bytes(&self) -> u64 {
        self.total_recovery_bytes
    }

    pub const fn total_retained_evidence_bytes(&self) -> u64 {
        self.total_retained_evidence_bytes
    }
}

#[derive(Clone, Debug)]
struct AttemptRecord {
    charge: ObservationCharge,
    disposition: Option<EffectDisposition>,
    reconciled: bool,
}

/// Bounded observation cost ledger enforcing obligation priority, pin retention,
/// anti-amplification ceilings, and non-refund of unknown effects (`FA-071`).
#[derive(Clone, Debug)]
pub struct ObservationCostLedger {
    basis: Basis,
    limits: ObservationCostLimits,
    used_transfer_bytes: u64,
    used_refinement_bytes: u64,
    active_retained_pins: u64,
    used_reconciliation_units: u64,
    outstanding_obligation_debt_units: u64,
    used_audit_floor_units: u64,
    attempts: BTreeMap<u64, AttemptRecord>,
    unconditional_attempts: u64,
    attack_conditioned_attempts: u64,
    admitted_useful_effects: u64,
    held_or_denied_attempts: u64,
    cold_start_attempts: u64,
    total_queue_delay_ms: u64,
    total_recovery_bytes: u64,
    total_retained_evidence_bytes: u64,
}

impl ObservationCostLedger {
    pub fn new(basis: Basis, limits: ObservationCostLimits) -> Result<Self, CostAccountingError> {
        if limits.max_transfer_bytes == 0
            || limits.max_refinement_bytes == 0
            || limits.max_retained_pins == 0
            || limits.max_reconciliation_units == 0
            || limits.max_obligation_debt_units == 0
            || limits.reserved_audit_floor_units == 0
            || limits.max_refinement_rounds_per_attempt == 0
        {
            return Err(CostAccountingError::InvalidLimits);
        }
        Ok(Self {
            basis,
            limits,
            used_transfer_bytes: 0,
            used_refinement_bytes: 0,
            active_retained_pins: 0,
            used_reconciliation_units: 0,
            outstanding_obligation_debt_units: 0,
            used_audit_floor_units: 0,
            attempts: BTreeMap::new(),
            unconditional_attempts: 0,
            attack_conditioned_attempts: 0,
            admitted_useful_effects: 0,
            held_or_denied_attempts: 0,
            cold_start_attempts: 0,
            total_queue_delay_ms: 0,
            total_recovery_bytes: 0,
            total_retained_evidence_bytes: 0,
        })
    }

    /// Admits and charges a validated physical plan execution (`FA-069`) under
    /// the declared priority tier.
    pub fn admit_plan_charge(
        &mut self,
        attempt_id: u64,
        priority: ObservationWorkPriority,
        plan_summary: &RehearsedPlanSummary,
        charge: ObservationCharge,
    ) -> Result<(), CostAccountingError> {
        if attempt_id == 0 || self.attempts.contains_key(&attempt_id) {
            return Err(CostAccountingError::DuplicateAttemptId(attempt_id));
        }
        if self.attempts.len() >= MAX_TRACKED_ATTEMPTS {
            return Err(CostAccountingError::AttemptLimitExceeded);
        }

        let unresolved_reconciliations = self
            .attempts
            .values()
            .filter(|a| {
                !a.reconciled && matches!(a.disposition, Some(EffectDisposition::UnknownInFlight))
            })
            .count();

        // Priority rule (§14.9): optional optimization and new non-reconciliation work
        // cannot starve existing obligation debt or unresolved reconciliation backlog.
        if priority == ObservationWorkPriority::OptionalOptimization
            && (self.outstanding_obligation_debt_units > 0 || unresolved_reconciliations > 0)
        {
            return Err(CostAccountingError::UnfundedExistingObligationDebt {
                outstanding_debt_units: self.outstanding_obligation_debt_units,
                pending_reconciliations: unresolved_reconciliations,
            });
        }
        if priority == ObservationWorkPriority::RequiredPlanObservation
            && self.outstanding_obligation_debt_units >= self.limits.max_obligation_debt_units
        {
            return Err(CostAccountingError::UnfundedExistingObligationDebt {
                outstanding_debt_units: self.outstanding_obligation_debt_units,
                pending_reconciliations: unresolved_reconciliations,
            });
        }

        // Anti-amplification check (§14.9).
        if charge.refinement_rounds > self.limits.max_refinement_rounds_per_attempt {
            return Err(CostAccountingError::ForcedRefinementAmplification {
                rounds: charge.refinement_rounds,
                max_allowed: self.limits.max_refinement_rounds_per_attempt,
            });
        }

        // Required pin charge check: every obligation or witness in the rehearsed plan
        // must be backed by a retained pin charge.
        let required_pins = (plan_summary.accumulated_obligations().entries().len()
            + usize::from(plan_summary.total_witnesses() > 0)) as u64;
        if charge.retained_pins < required_pins {
            return Err(CostAccountingError::MissingRequiredPinCharge {
                required_pins,
                proposed_pins: charge.retained_pins,
            });
        }

        // Charge must cover the rehearsed plan's minimum transfer/decode envelope.
        let min_transfer = plan_summary.total_bytes() as u64;
        let min_refinement = plan_summary.total_decoded_bytes() as u64;
        if charge.transfer_bytes < min_transfer || charge.refinement_bytes < min_refinement {
            return Err(CostAccountingError::UnderchargedPlanEnvelope);
        }

        let next_transfer = self
            .used_transfer_bytes
            .checked_add(charge.transfer_bytes)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_transfer > self.limits.max_transfer_bytes {
            return Err(CostAccountingError::TransferBudgetExceeded);
        }

        let next_refinement = self
            .used_refinement_bytes
            .checked_add(charge.refinement_bytes)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_refinement > self.limits.max_refinement_bytes {
            return Err(CostAccountingError::RefinementBudgetExceeded);
        }

        let next_pins = self
            .active_retained_pins
            .checked_add(charge.retained_pins)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_pins > self.limits.max_retained_pins {
            return Err(CostAccountingError::PinBudgetExceeded);
        }

        let next_reconciliation = self
            .used_reconciliation_units
            .checked_add(charge.reconciliation_units)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_reconciliation > self.limits.max_reconciliation_units {
            return Err(CostAccountingError::ReconciliationBudgetExceeded);
        }

        let next_debt = self
            .outstanding_obligation_debt_units
            .checked_add(charge.obligation_debt_units)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_debt > self.limits.max_obligation_debt_units {
            return Err(CostAccountingError::ObligationDebtCeilingExceeded);
        }

        let next_audit = self
            .used_audit_floor_units
            .checked_add(charge.audit_floor_units)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_audit > self.limits.reserved_audit_floor_units {
            return Err(CostAccountingError::AuditFloorBudgetExceeded);
        }

        self.used_transfer_bytes = next_transfer;
        self.used_refinement_bytes = next_refinement;
        self.active_retained_pins = next_pins;
        self.used_reconciliation_units = next_reconciliation;
        self.outstanding_obligation_debt_units = next_debt;
        self.used_audit_floor_units = next_audit;

        self.attempts.insert(
            attempt_id,
            AttemptRecord {
                charge,
                disposition: None,
                reconciled: false,
            },
        );
        Ok(())
    }

    /// Records realized trace telemetry and disposition for a charged attempt.
    pub fn record_realized_telemetry(
        &mut self,
        attempt_id: u64,
        disposition: EffectDisposition,
        telemetry: RealizedObservationTelemetry,
    ) -> Result<(), CostAccountingError> {
        let Some(record) = self.attempts.get_mut(&attempt_id) else {
            return Err(CostAccountingError::UnknownAttemptId(attempt_id));
        };
        if record.disposition.is_some() {
            return Err(CostAccountingError::DuplicateAttemptId(attempt_id));
        }

        if telemetry.realized_transfer_bytes > record.charge.transfer_bytes {
            let delta = telemetry.realized_transfer_bytes - record.charge.transfer_bytes;
            let next = self
                .used_transfer_bytes
                .checked_add(delta)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
            if next > self.limits.max_transfer_bytes {
                return Err(CostAccountingError::TransferBudgetExceeded);
            }
            self.used_transfer_bytes = next;
            record.charge.transfer_bytes = telemetry.realized_transfer_bytes;
        }

        if telemetry.realized_refinement_bytes > record.charge.refinement_bytes {
            let delta = telemetry.realized_refinement_bytes - record.charge.refinement_bytes;
            let next = self
                .used_refinement_bytes
                .checked_add(delta)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
            if next > self.limits.max_refinement_bytes {
                return Err(CostAccountingError::RefinementBudgetExceeded);
            }
            self.used_refinement_bytes = next;
            record.charge.refinement_bytes = telemetry.realized_refinement_bytes;
        }

        if telemetry.realized_reconciliation_units > record.charge.reconciliation_units {
            let delta =
                telemetry.realized_reconciliation_units - record.charge.reconciliation_units;
            let next = self
                .used_reconciliation_units
                .checked_add(delta)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
            if next > self.limits.max_reconciliation_units {
                return Err(CostAccountingError::ReconciliationBudgetExceeded);
            }
            self.used_reconciliation_units = next;
            record.charge.reconciliation_units = telemetry.realized_reconciliation_units;
        }

        self.unconditional_attempts = self
            .unconditional_attempts
            .checked_add(1)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if telemetry.attack_conditioned {
            self.attack_conditioned_attempts = self
                .attack_conditioned_attempts
                .checked_add(1)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
        }
        if telemetry.admitted_useful_effect
            && matches!(disposition, EffectDisposition::ConfirmedExecuted)
        {
            self.admitted_useful_effects = self
                .admitted_useful_effects
                .checked_add(1)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
        } else if matches!(disposition, EffectDisposition::AbortedBeforeDispatch) {
            self.held_or_denied_attempts = self
                .held_or_denied_attempts
                .checked_add(1)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
        }
        if telemetry.cold_start {
            self.cold_start_attempts = self
                .cold_start_attempts
                .checked_add(1)
                .ok_or(CostAccountingError::ArithmeticOverflow)?;
        }

        self.total_queue_delay_ms = self
            .total_queue_delay_ms
            .checked_add(telemetry.queue_delay_ms)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        self.total_recovery_bytes = self
            .total_recovery_bytes
            .checked_add(telemetry.recovery_bytes)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        self.total_retained_evidence_bytes = self
            .total_retained_evidence_bytes
            .checked_add(telemetry.retained_evidence_bytes)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;

        record.disposition = Some(disposition);
        Ok(())
    }

    ///Attempts to release or reconcile an attempt's obligation debt and pins.
    ///
    /// Unknown in-flight effects can never be refunded (`FA-INV-006`).
    pub fn reconcile_or_refund_attempt(
        &mut self,
        attempt_id: u64,
        reconciliation_work: u64,
        request_budget_refund: bool,
    ) -> Result<(), CostAccountingError> {
        let Some(record) = self.attempts.get_mut(&attempt_id) else {
            return Err(CostAccountingError::UnknownAttemptId(attempt_id));
        };
        let Some(disposition) = record.disposition else {
            return Err(CostAccountingError::UnknownEffectRefundForbidden { attempt_id });
        };

        if request_budget_refund {
            match disposition {
                EffectDisposition::UnknownInFlight => {
                    return Err(CostAccountingError::UnknownEffectRefundForbidden { attempt_id });
                }
                EffectDisposition::ConfirmedExecuted => {
                    return Err(CostAccountingError::ExecutedEffectRefundForbidden { attempt_id });
                }
                EffectDisposition::AbortedBeforeDispatch => {}
            }
        }

        if matches!(disposition, EffectDisposition::UnknownInFlight) {
            return Err(CostAccountingError::UnknownEffectRefundForbidden { attempt_id });
        }

        if record.reconciled {
            return Ok(());
        }

        let next_reconcile = self
            .used_reconciliation_units
            .checked_add(reconciliation_work)
            .ok_or(CostAccountingError::ArithmeticOverflow)?;
        if next_reconcile > self.limits.max_reconciliation_units {
            return Err(CostAccountingError::ReconciliationBudgetExceeded);
        }
        self.used_reconciliation_units = next_reconcile;

        self.outstanding_obligation_debt_units = self
            .outstanding_obligation_debt_units
            .saturating_sub(record.charge.obligation_debt_units);
        if matches!(disposition, EffectDisposition::AbortedBeforeDispatch) {
            self.active_retained_pins = self
                .active_retained_pins
                .saturating_sub(record.charge.retained_pins);
        }
        record.reconciled = true;
        Ok(())
    }

    /// Emits a typed `Knowledge<ObservationCostReport>` snapshot (`FA-INV-039`).
    pub fn snapshot_report(&self) -> Knowledge<ObservationCostReport> {
        let unresolved_in_flight_attempts = self
            .attempts
            .values()
            .filter(|a| matches!(a.disposition, Some(EffectDisposition::UnknownInFlight)))
            .count() as u64;

        let report = ObservationCostReport {
            basis: self.basis,
            used_transfer_bytes: self.used_transfer_bytes,
            used_refinement_bytes: self.used_refinement_bytes,
            active_retained_pins: self.active_retained_pins,
            used_reconciliation_units: self.used_reconciliation_units,
            outstanding_obligation_debt_units: self.outstanding_obligation_debt_units,
            used_audit_floor_units: self.used_audit_floor_units,
            unconditional_attempts: self.unconditional_attempts,
            attack_conditioned_attempts: self.attack_conditioned_attempts,
            admitted_useful_effects: self.admitted_useful_effects,
            held_or_denied_attempts: self.held_or_denied_attempts,
            unresolved_in_flight_attempts,
            cold_start_attempts: self.cold_start_attempts,
            total_queue_delay_ms: self.total_queue_delay_ms,
            total_recovery_bytes: self.total_recovery_bytes,
            total_retained_evidence_bytes: self.total_retained_evidence_bytes,
        };
        Knowledge::known(report, self.basis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observation_algebra::{
            AuthorityKind, AuthorityRequirements, BasisIdentity, CancellationObligations,
            ClaimClass, MAX_OPERATOR_BYTES, MAX_OPERATOR_CANDIDATES, MAX_OPERATOR_DECODED_BYTES,
            MAX_OPERATOR_FRONTIERS, MAX_OPERATOR_INPUTS, MAX_OPERATOR_OUTPUTS,
            MAX_OPERATOR_WITNESSES, Obligation, ObligationKind, ObservationRequirement,
            OperatorContext, PrivacyLabel, PrivacyRestrictions, ProofStates, RequirementClass,
            ResourceEnvelope, ScopedProof, UncertaintyClass,
            physical_plan::{
                PhysicalOperatorKind, PhysicalPlan, PhysicalPlanNode, validate_physical_plan,
            },
        },
        product_frontier::ProductFrontiers,
    };

    fn sample_basis() -> Basis {
        Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 2,
                profile_id: 3,
                semantic_generation: 1,
                policy_epoch: 1,
                model_epoch: 1,
                tokenizer_epoch: 1,
                codec_epoch: 1,
                control_seq: 1,
            },
            ClaimClass::Invariant,
            ProofStates {
                authentic_origin: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                complete_for_contract: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                semantically_valid: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
                available_for_replay: ScopedProof::Declared {
                    scope_id: 1,
                    generation: 1,
                },
            },
        )
    }

    fn sample_rehearsed_summary(basis: Basis) -> RehearsedPlanSummary {
        let auth = AuthorityRequirements::new(vec![AuthorityKind::Observe]).unwrap();
        let privacy = PrivacyRestrictions::new(vec![PrivacyLabel {
            purpose_id: 1,
            transform_id: 1,
        }])
        .unwrap();
        let obligations = CancellationObligations::new(vec![Obligation {
            id: 9,
            kind: ObligationKind::RetainInput,
        }])
        .unwrap();
        let envelope = ResourceEnvelope::new(
            MAX_OPERATOR_INPUTS,
            MAX_OPERATOR_OUTPUTS,
            MAX_OPERATOR_WITNESSES,
            MAX_OPERATOR_FRONTIERS,
            MAX_OPERATOR_BYTES,
            MAX_OPERATOR_DECODED_BYTES,
            MAX_OPERATOR_CANDIDATES,
        )
        .unwrap();
        let ctx = OperatorContext::new(
            ObservationRequirement {
                class: RequirementClass::Capture,
                uncertainty: UncertaintyClass::Exact,
                basis,
            },
            privacy.clone(),
            obligations,
            envelope,
        );
        let plan = PhysicalPlan {
            nodes: vec![PhysicalPlanNode {
                node_id: 1,
                operator: PhysicalOperatorKind::Tap {
                    source_id: 1,
                    source_generation: 1,
                    bytes: 256,
                },
                context: ctx,
            }],
            root: 0,
            granted_authority: auth,
            allowed_privacy: privacy,
            frontiers: ProductFrontiers::new(2, 8).unwrap(),
            total_envelope: envelope,
        };
        let Knowledge::Known { value, .. } = validate_physical_plan(&plan).unwrap() else {
            panic!("expected known summary");
        };
        value
    }

    fn sample_limits() -> ObservationCostLimits {
        ObservationCostLimits {
            max_transfer_bytes: 4096,
            max_refinement_bytes: 4096,
            max_retained_pins: 8,
            max_reconciliation_units: 100,
            max_obligation_debt_units: 50,
            reserved_audit_floor_units: 20,
            max_refinement_rounds_per_attempt: 4,
        }
    }

    #[test]
    fn charges_transfer_refinement_pins_reconciliation_and_tracks_cold_attack_denominators() {
        let basis = sample_basis();
        let summary = sample_rehearsed_summary(basis);
        let mut ledger = ObservationCostLedger::new(basis, sample_limits()).unwrap();

        ledger
            .admit_plan_charge(
                1,
                ObservationWorkPriority::RequiredPlanObservation,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 128,
                    retained_pins: 2,
                    reconciliation_units: 10,
                    obligation_debt_units: 15,
                    audit_floor_units: 5,
                    refinement_rounds: 2,
                },
            )
            .unwrap();

        ledger
            .record_realized_telemetry(
                1,
                EffectDisposition::ConfirmedExecuted,
                RealizedObservationTelemetry {
                    realized_transfer_bytes: 300,
                    realized_refinement_bytes: 128,
                    realized_reconciliation_units: 12,
                    queue_delay_ms: 18,
                    recovery_bytes: 64,
                    retained_evidence_bytes: 512,
                    cold_start: true,
                    attack_conditioned: true,
                    admitted_useful_effect: true,
                },
            )
            .unwrap();

        ledger.reconcile_or_refund_attempt(1, 8, false).unwrap();

        let Knowledge::Known { value: report, .. } = ledger.snapshot_report() else {
            panic!("expected known report");
        };
        assert_eq!(report.used_transfer_bytes(), 300);
        assert_eq!(report.used_refinement_bytes(), 128);
        assert_eq!(report.active_retained_pins(), 2);
        assert_eq!(report.used_reconciliation_units(), 20);
        assert_eq!(report.outstanding_obligation_debt_units(), 0);
        assert_eq!(report.used_audit_floor_units(), 5);
        assert_eq!(report.unconditional_attempts(), 1);
        assert_eq!(report.attack_conditioned_attempts(), 1);
        assert_eq!(report.admitted_useful_effects(), 1);
        assert_eq!(report.cold_start_attempts(), 1);
        assert_eq!(report.total_queue_delay_ms(), 18);
        assert_eq!(report.total_recovery_bytes(), 64);
        assert_eq!(report.total_retained_evidence_bytes(), 512);
    }

    #[test]
    fn existing_obligation_debt_blocks_optional_optimization_until_reconciled() {
        let basis = sample_basis();
        let summary = sample_rehearsed_summary(basis);
        let mut ledger = ObservationCostLedger::new(basis, sample_limits()).unwrap();

        ledger
            .admit_plan_charge(
                1,
                ObservationWorkPriority::RequiredPlanObservation,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 2,
                    reconciliation_units: 5,
                    obligation_debt_units: 20,
                    audit_floor_units: 0,
                    refinement_rounds: 1,
                },
            )
            .unwrap();

        // Optional optimization is refused while obligation debt > 0.
        assert_eq!(
            ledger.admit_plan_charge(
                2,
                ObservationWorkPriority::OptionalOptimization,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 2,
                    reconciliation_units: 5,
                    obligation_debt_units: 0,
                    audit_floor_units: 0,
                    refinement_rounds: 1,
                },
            ),
            Err(CostAccountingError::UnfundedExistingObligationDebt {
                outstanding_debt_units: 20,
                pending_reconciliations: 0,
            })
        );

        // Mandatory audit floor work is still admitted under its separate reservation.
        ledger
            .admit_plan_charge(
                3,
                ObservationWorkPriority::MandatoryAuditFloor,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 2,
                    reconciliation_units: 5,
                    obligation_debt_units: 0,
                    audit_floor_units: 10,
                    refinement_rounds: 1,
                },
            )
            .unwrap();
    }

    #[test]
    fn unknown_effect_refund_missing_pin_charge_and_refinement_amplification_are_refused() {
        let basis = sample_basis();
        let summary = sample_rehearsed_summary(basis);
        let mut ledger = ObservationCostLedger::new(basis, sample_limits()).unwrap();

        // Missing pin charge is refused.
        assert_eq!(
            ledger.admit_plan_charge(
                1,
                ObservationWorkPriority::RequiredPlanObservation,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 1,
                    reconciliation_units: 5,
                    obligation_debt_units: 5,
                    audit_floor_units: 0,
                    refinement_rounds: 1,
                },
            ),
            Err(CostAccountingError::MissingRequiredPinCharge {
                required_pins: 2,
                proposed_pins: 1,
            })
        );

        // Forced refinement amplification is refused.
        assert_eq!(
            ledger.admit_plan_charge(
                1,
                ObservationWorkPriority::RequiredPlanObservation,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 2,
                    reconciliation_units: 5,
                    obligation_debt_units: 5,
                    audit_floor_units: 0,
                    refinement_rounds: 9,
                },
            ),
            Err(CostAccountingError::ForcedRefinementAmplification {
                rounds: 9,
                max_allowed: 4,
            })
        );

        // Admit a valid attempt and mark its outcome UnknownInFlight.
        ledger
            .admit_plan_charge(
                1,
                ObservationWorkPriority::RequiredPlanObservation,
                &summary,
                ObservationCharge {
                    transfer_bytes: 256,
                    refinement_bytes: 64,
                    retained_pins: 2,
                    reconciliation_units: 5,
                    obligation_debt_units: 10,
                    audit_floor_units: 0,
                    refinement_rounds: 2,
                },
            )
            .unwrap();
        ledger
            .record_realized_telemetry(
                1,
                EffectDisposition::UnknownInFlight,
                RealizedObservationTelemetry::default(),
            )
            .unwrap();

        // Unknown in-flight effect cannot be refunded or have its debt cleared (FA-INV-006).
        assert_eq!(
            ledger.reconcile_or_refund_attempt(1, 5, true),
            Err(CostAccountingError::UnknownEffectRefundForbidden { attempt_id: 1 })
        );
        assert_eq!(
            ledger.reconcile_or_refund_attempt(1, 5, false),
            Err(CostAccountingError::UnknownEffectRefundForbidden { attempt_id: 1 })
        );
    }
}
