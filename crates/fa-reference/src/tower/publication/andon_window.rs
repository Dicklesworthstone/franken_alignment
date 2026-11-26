//! Register one independently selected accounting window on its original owner.
//! Plans cannot choose budgets, reset accounting or confer effect authority.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, FileAndonWindowError, FileAndonWindowPlanResult, FileAndonWindowRequest,
    FileAndonWindowResult, FileSupervisorAndonWindowPort,
};
use crate::address_resolver::Address;
use crate::round::review::andon::AndonBudget;
use crate::tower::request::andon_window::{encode_basis, encode_budget};
use crate::typestate_cx::PurposeTag;

/// Historical plan sealed to the original registration and exact query.
/// Registration still requires the original independently retained window grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedAndonWindowNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    operation: u64,
    native: FileAndonWindowPlanResult,
}

impl LinkedAndonWindowNext {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn query_idempotency_key(&self) -> Option<&str> {
        self.query_idempotency_key.as_deref()
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn native(&self) -> &FileAndonWindowPlanResult {
        &self.native
    }

    pub fn action(&self) -> Result<Option<TowerRequest>, TowerInstallError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Ok(None);
        };
        let Some(native) = value.request() else {
            return Ok(None);
        };
        if value.basis() != *basis
            || native.expected() != *basis
            || value.operation() != self.operation
            || native.operation() != self.operation
            || native.expected_budget() != value.budget()
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut request = TowerRequest::new(
            format!("register-andon-window-{}", self.operation),
            self.registration.clone(),
            Verb::RegisterAndonWindow,
        );
        request.arguments = ReadArguments::RegisterAndonWindow {
            operation: self.operation,
            expected: *basis,
            expected_budget: value.budget(),
        };
        request.idempotency_key = Some(self.operation.to_string());
        Ok(Some(request))
    }
}

/// One acknowledged original-window registration or refusal at the attempted cut.
/// A receipt is historical accounting evidence, never an effect permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedAndonWindowRegistration {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FileAndonWindowRequest,
    native: FileAndonWindowResult,
}

impl LinkedAndonWindowRegistration {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn attempted(&self) -> &FileAndonWindowRequest {
        &self.attempted
    }
    pub fn native(&self) -> &FileAndonWindowResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install a weak port to one independently selected original budget.
    /// Even equal numeric scopes must share the original owner. A rejected
    /// replacement withdraws the previous port without registering anything.
    pub fn set_andon_window(
        &mut self,
        port: Option<FileSupervisorAndonWindowPort>,
    ) -> Result<(), TowerInstallError> {
        self.andon_window = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.andon_window = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_andon_window(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        if !access.operator {
            return self.withheld(&request);
        }
        if !access.allowed_purposes.contains(&PurposeTag::Permit) {
            return self.refusal(&request, ReasonCode::PurposeMismatch, false);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        }
        match &request.arguments {
            ReadArguments::AndonWindowNext { operation } if request.verb == Verb::Next => {
                if *operation == 0
                    || request
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|key| key != &operation.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Some(port) = &self.andon_window else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
                };
                let native = port.next(*operation);
                let reason = registration_reason(&native);
                let linked = LinkedAndonWindowNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    operation: *operation,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::AndonWindowNext(linked),
                    reason,
                    true,
                ))
            }
            ReadArguments::RegisterAndonWindow {
                operation,
                expected,
                expected_budget,
            } if request.verb == Verb::RegisterAndonWindow => {
                if request.address != self.mount
                    || expected.scope != self.scope
                    || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) =
                    FileAndonWindowRequest::new(*operation, *expected, *expected_budget)
                else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let Some(port) = &self.andon_window else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
                };
                // Reserve a complete response before entering any original
                // journal or numerical replay work; never acknowledge a prefix.
                if !self.andon_window_response_fits(&request, attempted) {
                    return self.refusal(&request, ReasonCode::BudgetExhausted, false);
                }
                let native = port.register(attempted);
                let reason = registration_reason(&native);
                let linked = LinkedAndonWindowRegistration {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    attempted,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::AndonWindowRegistration(linked),
                    reason,
                    false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }

    fn andon_window_response_fits(
        &self,
        request: &TowerRequest,
        attempted: FileAndonWindowRequest,
    ) -> bool {
        // This inert result encodes the actual registration, request labels,
        // predecessor and longest native-error envelope. It never calls a port.
        let probe = LinkedAndonWindowRegistration {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            attempted,
            native: Err(FileAndonWindowError::Unavailable),
        };
        let response = self.response(
            request,
            PublicationResult::AndonWindowRegistration(probe),
            Some(ReasonCode::LowerLayerUnverified),
            false,
        );
        let Ok(encoded) = response.to_json() else {
            return false;
        };
        let upper_basis = FileAndonWindowBasis {
            scope: attempted.expected().scope,
            journal_revision: u64::MAX,
            control_sequence: u64::MAX,
            authority_epoch: u64::MAX,
            evaluation_revision: u64::MAX,
            congress_generation: u64::MAX,
            policy_generation: u64::MAX,
            recorded_at: Some(ElapsedTick(u64::MAX)),
            accounting: attempted.expected().accounting,
        };
        let basis = encode_basis(upper_basis);
        let budget = encode_budget(AndonBudget {
            window: u64::MAX,
            start: u64::MAX - 1,
            end: u64::MAX,
            false_stops_allowed: u64::MAX,
            cost_per_stop: u64::MAX,
        });
        // Include both full basis values and the longest valid budget without
        // subtracting the inert error. This bounds Known and unavailable forms,
        // including escaped request labels and all canonical decimal widths.
        let receipt = format!(
            r#","knowledge":{{"state":"known","basis":{basis},"value":{{"before":{basis},"budget":{budget},"registered":false}}}}"#
        );
        encoded
            .len()
            .checked_add(receipt.len())
            .is_some_and(|bytes| bytes <= self.limits.max_response_bytes)
    }
}

fn registration_reason<T>(
    result: &Result<ActorKnowledge<T, FileAndonWindowBasis>, FileAndonWindowError>,
) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_andon_window",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(FileAndonWindowError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FileAndonWindowError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FileAndonWindowError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FileAndonWindowError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
