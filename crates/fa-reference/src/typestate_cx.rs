//! Purpose contexts as typestates and runtime defense-in-depth tags (FA-117,
//! plan §§6.3, 6.6; invariants FA-INV-009, FA-INV-025, FA-INV-033, FA-INV-038).
//!
//! Defines the five purpose-scoped context newtypes (`CaptureCx`, `JudgeCx`,
//! `ExperimentCx`, `PermitCx`, `MaintenanceCx`) with private fields constructed
//! solely from `RootCx`. No conversion exists between purpose types, and
//! production dispatch (`Broker::dispatch`) accepts `&PermitCx` only.
//! At cross-process/adapter boundaries where static types are erased, capability
//! tokens carry runtime `PurposeTag`s verified before execution.

use crate::{
    Snapshot,
    action::{FrozenAction, Permit, ReferenceAuthority},
};

pub const MAX_CAPTURE_WINDOW_BYTES: usize = 65_536;
pub const MAX_ELICITATION_PROMPT_BYTES: usize = 16_384;

/// Runtime purpose tag carried across process/adapter boundaries for
/// defense-in-depth verification (`6ga.3`, `FA-INV-038`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PurposeTag {
    Capture = 1,
    Judge = 2,
    Experiment = 3,
    Permit = 4,
    Maintenance = 5,
}

impl PurposeTag {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Judge => "judge",
            Self::Experiment => "experiment",
            Self::Permit => "permit",
            Self::Maintenance => "maintenance",
        }
    }
}

/// Inner execution context descriptor wrapped by the five purpose newtypes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CxInner {
    tenant_id: u64,
    run_id: u64,
    branch_id: u64,
    epoch: u64,
    purpose: PurposeTag,
}

/// Control-nucleus root context (`6ga.1`). Only the control nucleus may mint
/// purpose-specific contexts from `RootCx`.
#[derive(Debug)]
pub struct RootCx {
    tenant_id: u64,
    run_id: u64,
    epoch: u64,
    next_token_nonce: u64,
}

impl RootCx {
    pub const fn new_nucleus(tenant_id: u64, run_id: u64, epoch: u64) -> Self {
        Self {
            tenant_id,
            run_id,
            epoch,
            next_token_nonce: 1,
        }
    }

    pub const fn mint_capture(&self) -> CaptureCx {
        CaptureCx(CxInner {
            tenant_id: self.tenant_id,
            run_id: self.run_id,
            branch_id: 0,
            epoch: self.epoch,
            purpose: PurposeTag::Capture,
        })
    }

    pub const fn mint_judge(&self) -> JudgeCx {
        JudgeCx(CxInner {
            tenant_id: self.tenant_id,
            run_id: self.run_id,
            branch_id: 0,
            epoch: self.epoch,
            purpose: PurposeTag::Judge,
        })
    }

    pub const fn mint_experiment(&self, branch_id: u64) -> Result<ExperimentCx, PurposeGateError> {
        if branch_id == 0 {
            return Err(PurposeGateError::InvalidBranchId);
        }
        Ok(ExperimentCx(CxInner {
            tenant_id: self.tenant_id,
            run_id: self.run_id,
            branch_id,
            epoch: self.epoch,
            purpose: PurposeTag::Experiment,
        }))
    }

    pub const fn mint_permit(&self, authorization_seq: u64) -> Result<PermitCx, PurposeGateError> {
        if authorization_seq == 0 {
            return Err(PurposeGateError::MissingAuthorizationEvent);
        }
        Ok(PermitCx {
            inner: CxInner {
                tenant_id: self.tenant_id,
                run_id: self.run_id,
                branch_id: 0,
                epoch: self.epoch,
                purpose: PurposeTag::Permit,
            },
            authorization_seq,
        })
    }

    pub const fn mint_maintenance(&self) -> MaintenanceCx {
        MaintenanceCx(CxInner {
            tenant_id: self.tenant_id,
            run_id: self.run_id,
            branch_id: 0,
            epoch: self.epoch,
            purpose: PurposeTag::Maintenance,
        })
    }

    /// Mints a cross-process capability token tagged with the caller's exact purpose (`6ga.3`).
    pub fn mint_capability_token(&mut self, purpose: PurposeTag) -> CapabilityToken {
        let nonce = self.next_token_nonce;
        self.next_token_nonce = self.next_token_nonce.saturating_add(1);
        CapabilityToken {
            tenant_id: self.tenant_id,
            run_id: self.run_id,
            epoch: self.epoch,
            nonce,
            minted_purpose: purpose,
        }
    }
}

/// Capture-purpose context (`6ga.1`). Exposes activation/observation capture
/// capabilities only; cannot judge, experiment, or dispatch production effects.
///
/// ```compile_fail,E0308
/// use fa_reference::typestate_cx::{Broker, CaptureCx, PermitCx};
/// fn forbidden(cx: &CaptureCx) -> &PermitCx {
///     cx
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct CaptureCx(CxInner);

impl CaptureCx {
    pub const fn tenant_id(&self) -> u64 {
        self.0.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.0.run_id
    }

    pub const fn epoch(&self) -> u64 {
        self.0.epoch
    }

    pub const fn purpose(&self) -> PurposeTag {
        self.0.purpose
    }
}

/// Judge-purpose context (`6ga.1`). Exposes read-only frozen packet inspection
/// and verdict emission; cannot mutate policy or dispatch effects.
///
/// ```compile_fail,E0308
/// use fa_reference::typestate_cx::{JudgeCx, PermitCx};
/// fn forbidden(cx: &JudgeCx) -> &PermitCx {
///     cx
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct JudgeCx(CxInner);

impl JudgeCx {
    pub const fn tenant_id(&self) -> u64 {
        self.0.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.0.run_id
    }

    pub const fn epoch(&self) -> u64 {
        self.0.epoch
    }

    pub const fn purpose(&self) -> PurposeTag {
        self.0.purpose
    }
}

/// Experiment-purpose context (`6ga.1`, `FA-INV-009`, `FA-INV-033`, `FA-INV-038`).
/// Restricted to isolated rehearsal/counterfactual branches (`branch_id != 0`).
/// Cannot convert to `PermitCx` or invoke `Broker::dispatch`:
///
/// ```compile_fail,E0308
/// use fa_reference::typestate_cx::{Broker, CapabilityToken, ExperimentCx};
/// use fa_reference::action::{FrozenAction, Permit, ReferenceAuthority};
/// use fa_reference::Snapshot;
/// fn attempt_dispatch_from_experiment(
///     cx: &ExperimentCx,
///     token: &CapabilityToken,
///     action: &FrozenAction,
///     auth: &mut ReferenceAuthority,
///     permit: &Permit,
///     snapshot: &Snapshot,
/// ) {
///     let _ = Broker::dispatch(cx, token, action, auth, permit, snapshot);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::typestate_cx::ExperimentCx;
/// fn try_into_permit(cx: ExperimentCx) {
///     let _ = cx.as_permit();
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ExperimentCx(CxInner);

impl ExperimentCx {
    pub const fn tenant_id(&self) -> u64 {
        self.0.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.0.run_id
    }

    pub const fn branch_id(&self) -> u64 {
        self.0.branch_id
    }

    pub const fn epoch(&self) -> u64 {
        self.0.epoch
    }

    pub const fn purpose(&self) -> PurposeTag {
        self.0.purpose
    }
}

/// Permit-purpose context (`6ga.1`, `FA-INV-038`). The sole context type
/// accepted by `Broker::dispatch`. Constructed only by `RootCx::mint_permit`
/// with a non-zero authorization sequence.
///
/// ```compile_fail,E0451
/// use fa_reference::typestate_cx::PermitCx;
/// fn forge() -> PermitCx {
///     PermitCx { inner: todo!(), authorization_seq: 1 }
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct PermitCx {
    inner: CxInner,
    authorization_seq: u64,
}

impl PermitCx {
    pub const fn tenant_id(&self) -> u64 {
        self.inner.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.inner.run_id
    }

    pub const fn epoch(&self) -> u64 {
        self.inner.epoch
    }

    pub const fn authorization_seq(&self) -> u64 {
        self.authorization_seq
    }

    pub const fn purpose(&self) -> PurposeTag {
        self.inner.purpose
    }
}

/// Maintenance-purpose context (`6ga.1`). Restricted to bounded compaction and
/// guarded index rewrites; cannot mint permits or dispatch effects.
///
/// ```compile_fail,E0308
/// use fa_reference::typestate_cx::{MaintenanceCx, PermitCx};
/// fn forbidden(cx: &MaintenanceCx) -> &PermitCx {
///     cx
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct MaintenanceCx(CxInner);

impl MaintenanceCx {
    pub const fn tenant_id(&self) -> u64 {
        self.0.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.0.run_id
    }

    pub const fn epoch(&self) -> u64 {
        self.0.epoch
    }

    pub const fn purpose(&self) -> PurposeTag {
        self.0.purpose
    }
}

/// Cross-process capability token carrying a runtime purpose tag for
/// defense-in-depth checks at adapter boundaries (`6ga.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityToken {
    tenant_id: u64,
    run_id: u64,
    epoch: u64,
    nonce: u64,
    minted_purpose: PurposeTag,
}

impl CapabilityToken {
    pub const fn tenant_id(&self) -> u64 {
        self.tenant_id
    }

    pub const fn run_id(&self) -> u64 {
        self.run_id
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn nonce(&self) -> u64 {
        self.nonce
    }

    pub const fn minted_purpose(&self) -> PurposeTag {
        self.minted_purpose
    }
}

/// Stable refusal reason when a cross-process token or context violates its
/// purpose boundary (`6ga.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenialReason {
    PurposeMismatch {
        expected: PurposeTag,
        actual: PurposeTag,
    },
    ScopeOrEpochMismatch,
}

/// Errors returned by purpose-gated APIs (`6ga.1`..`6ga.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurposeGateError {
    InvalidBranchId,
    MissingAuthorizationEvent,
    PayloadLimitExceeded,
    DeniedByPolicy { reason: DenialReason },
    AuthorityError(crate::Error),
}

/// Captured observation window emitted only under `&CaptureCx` (`6ga.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedWindow {
    pub tenant_id: u64,
    pub run_id: u64,
    pub epoch: u64,
    pub byte_len: usize,
}

/// Helper judgment emitted only under `&JudgeCx` (`6ga.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelperVerdictRecord {
    pub tenant_id: u64,
    pub run_id: u64,
    pub epoch: u64,
    pub window_bytes: usize,
    pub hold_recommended: bool,
}

/// Counterfactual elicitation output emitted only under `&ExperimentCx` (`6ga.2`, `FA-INV-040`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RehearsalSimResult {
    pub tenant_id: u64,
    pub run_id: u64,
    pub branch_id: u64,
    pub rehearsal_only: bool,
    pub prompt_bytes: usize,
}

/// Compaction receipt emitted only under `&MaintenanceCx` (`6ga.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactionReceipt {
    pub tenant_id: u64,
    pub run_id: u64,
    pub epoch: u64,
    pub compacted_segments: u64,
}

/// Sidecar encoder gated by `&CaptureCx` (`6ga.2`).
pub struct Sidecar;

impl Sidecar {
    pub fn encode(cx: &CaptureCx, payload: &[u8]) -> Result<CapturedWindow, PurposeGateError> {
        if payload.len() > MAX_CAPTURE_WINDOW_BYTES {
            return Err(PurposeGateError::PayloadLimitExceeded);
        }
        Ok(CapturedWindow {
            tenant_id: cx.tenant_id(),
            run_id: cx.run_id(),
            epoch: cx.epoch(),
            byte_len: payload.len(),
        })
    }
}

/// Helper judge gated by `&JudgeCx` (`6ga.2`).
pub struct Helper;

impl Helper {
    pub fn judge(
        cx: &JudgeCx,
        window: &CapturedWindow,
        hold_recommended: bool,
    ) -> Result<HelperVerdictRecord, PurposeGateError> {
        if cx.tenant_id() != window.tenant_id
            || cx.run_id() != window.run_id
            || cx.epoch() != window.epoch
        {
            return Err(PurposeGateError::DeniedByPolicy {
                reason: DenialReason::ScopeOrEpochMismatch,
            });
        }
        Ok(HelperVerdictRecord {
            tenant_id: cx.tenant_id(),
            run_id: cx.run_id(),
            epoch: cx.epoch(),
            window_bytes: window.byte_len,
            hold_recommended,
        })
    }
}

/// Elicitation harness gated by `&ExperimentCx` (`6ga.2`).
pub struct Elicitation;

impl Elicitation {
    pub fn run(cx: &ExperimentCx, prompt: &str) -> Result<RehearsalSimResult, PurposeGateError> {
        if prompt.len() > MAX_ELICITATION_PROMPT_BYTES {
            return Err(PurposeGateError::PayloadLimitExceeded);
        }
        Ok(RehearsalSimResult {
            tenant_id: cx.tenant_id(),
            run_id: cx.run_id(),
            branch_id: cx.branch_id(),
            rehearsal_only: true,
            prompt_bytes: prompt.len(),
        })
    }
}

/// Maintenance runner gated by `&MaintenanceCx` (`6ga.2`).
pub struct Maintenance;

impl Maintenance {
    pub fn compact(
        cx: &MaintenanceCx,
        segments: u64,
    ) -> Result<CompactionReceipt, PurposeGateError> {
        if segments == 0 {
            return Err(PurposeGateError::PayloadLimitExceeded);
        }
        Ok(CompactionReceipt {
            tenant_id: cx.tenant_id(),
            run_id: cx.run_id(),
            epoch: cx.epoch(),
            compacted_segments: segments,
        })
    }
}

/// Production effect broker gated by `&PermitCx` (`6ga.2`, `6ga.3`, `FA-INV-038`).
pub struct Broker;

impl Broker {
    /// Defense-in-depth runtime check at the adapter/process boundary (`6ga.3`):
    /// refuses any capability token minted under a non-`Permit` purpose with
    /// `DeniedByPolicy { reason: PurposeMismatch }`.
    pub fn verify_adapter_token(
        cx: &PermitCx,
        token: &CapabilityToken,
    ) -> Result<(), PurposeGateError> {
        if token.minted_purpose != PurposeTag::Permit {
            return Err(PurposeGateError::DeniedByPolicy {
                reason: DenialReason::PurposeMismatch {
                    expected: PurposeTag::Permit,
                    actual: token.minted_purpose,
                },
            });
        }
        if token.tenant_id != cx.tenant_id()
            || token.run_id != cx.run_id()
            || token.epoch != cx.epoch()
        {
            return Err(PurposeGateError::DeniedByPolicy {
                reason: DenialReason::ScopeOrEpochMismatch,
            });
        }
        Ok(())
    }

    /// Dispatches a production effect. Accepts `&PermitCx` only (`FA-INV-038`)
    /// and verifies the runtime adapter token's `PurposeTag` (`6ga.3`).
    pub fn dispatch(
        cx: &PermitCx,
        token: &CapabilityToken,
        final_action: &FrozenAction,
        authority: &mut ReferenceAuthority,
        permit: &Permit,
        snapshot: &Snapshot,
    ) -> Result<(), PurposeGateError> {
        Self::verify_adapter_token(cx, token)?;
        authority
            .dispatch(permit, final_action, snapshot)
            .map_err(PurposeGateError::AuthorityError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Judgment, ReadWitness,
        action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope},
    };
    use std::collections::BTreeMap;

    #[test]
    fn purpose_contexts_gate_their_designated_apis_and_preserve_scope_isolation() {
        let root = RootCx::new_nucleus(7, 42, 3);
        let capture_cx = root.mint_capture();
        let judge_cx = root.mint_judge();
        let experiment_cx = root.mint_experiment(9).unwrap();
        let maintenance_cx = root.mint_maintenance();

        assert_eq!(capture_cx.purpose(), PurposeTag::Capture);
        assert_eq!(judge_cx.purpose(), PurposeTag::Judge);
        assert_eq!(experiment_cx.purpose(), PurposeTag::Experiment);
        assert_eq!(experiment_cx.branch_id(), 9);
        assert_eq!(maintenance_cx.purpose(), PurposeTag::Maintenance);

        // Branch 0 is reserved for production and cannot be used for ExperimentCx.
        assert_eq!(
            root.mint_experiment(0),
            Err(PurposeGateError::InvalidBranchId)
        );
        // PermitCx requires a non-zero authorization sequence.
        assert_eq!(
            root.mint_permit(0),
            Err(PurposeGateError::MissingAuthorizationEvent)
        );

        let window = Sidecar::encode(&capture_cx, b"activation-frame").unwrap();
        let verdict = Helper::judge(&judge_cx, &window, true).unwrap();
        assert!(verdict.hold_recommended);

        let sim = Elicitation::run(&experiment_cx, "counterfactual-prompt").unwrap();
        assert!(sim.rehearsal_only);
        assert_eq!(sim.branch_id, 9);

        let receipt = Maintenance::compact(&maintenance_cx, 4).unwrap();
        assert_eq!(receipt.compacted_segments, 4);
    }

    #[test]
    fn runtime_defense_in_depth_refuses_non_permit_tokens_at_broker_boundary() {
        let mut root = RootCx::new_nucleus(1, 10, 1);
        let permit_cx = root.mint_permit(55).unwrap();

        for forbidden_purpose in [
            PurposeTag::Capture,
            PurposeTag::Judge,
            PurposeTag::Experiment,
            PurposeTag::Maintenance,
        ] {
            let laundered_token = root.mint_capability_token(forbidden_purpose);
            assert_eq!(
                Broker::verify_adapter_token(&permit_cx, &laundered_token),
                Err(PurposeGateError::DeniedByPolicy {
                    reason: DenialReason::PurposeMismatch {
                        expected: PurposeTag::Permit,
                        actual: forbidden_purpose,
                    }
                })
            );
        }

        // A genuine Permit-tagged token from the same nucleus scope passes verification
        // and drives real ReferenceAuthority dispatch.
        let valid_token = root.mint_capability_token(PurposeTag::Permit);
        assert_eq!(
            Broker::verify_adapter_token(&permit_cx, &valid_token),
            Ok(())
        );

        let scope = Scope {
            tenant: 1,
            principal: 2,
            run: 10,
            branch: 1,
            authority: 1,
            purpose: Purpose::Effect,
        };
        let mut authority = ReferenceAuthority::new(scope, 10, 4).unwrap();
        authority.observe_time(ElapsedTick(1)).unwrap();

        let mut values = BTreeMap::new();
        values.insert(1, b"v".to_vec());
        let snapshot = Snapshot {
            values,
            semantic_epoch: 1,
            complete: true,
        };
        let witness = ReadWitness::Exact {
            key: 1,
            value: Some(b"v".to_vec()),
        };
        let action = FrozenAction::freeze(ActionSpec {
            version: 1,
            scope,
            target: Some(ResolvedTarget {
                adapter: 1,
                object: 2,
                contract_version: 1,
                expected_version: 1,
                generation: 1,
            }),
            payload: b"sink".to_vec(),
            required_witnesses: vec![witness.clone()],
            policy_epoch: 0,
            deadline: ElapsedTick(10),
            units: 3,
        })
        .unwrap();

        authority.propose(1, action.clone()).unwrap();
        authority.prepare(1).unwrap();
        authority.begin_review(1).unwrap();
        let judgment = Judgment::capture(&snapshot, vec![witness]).unwrap();
        let permit = authority.authorize(1, &judgment, &snapshot).unwrap();

        // Attempting to dispatch with an Experiment-minted token fails closed without spending the permit!
        let exp_token = root.mint_capability_token(PurposeTag::Experiment);
        assert_eq!(
            Broker::dispatch(
                &permit_cx,
                &exp_token,
                &action,
                &mut authority,
                &permit,
                &snapshot
            ),
            Err(PurposeGateError::DeniedByPolicy {
                reason: DenialReason::PurposeMismatch {
                    expected: PurposeTag::Permit,
                    actual: PurposeTag::Experiment,
                }
            })
        );

        // Dispatching with the valid Permit token succeeds once.
        assert_eq!(
            Broker::dispatch(
                &permit_cx,
                &valid_token,
                &action,
                &mut authority,
                &permit,
                &snapshot
            ),
            Ok(())
        );
    }
}
