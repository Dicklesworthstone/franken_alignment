//! Independently granted restriction of the original run's target ceiling.
//! Plans are observations; only the original native owner applies an intersection.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, FileNarrowError, FileNarrowPlanResult, FileNarrowRequest,
    FileNarrowResult, FileSupervisorNarrowPort,
};
use crate::action::ResolvedTarget;
use crate::address_resolver::Address;
use crate::tower::request::narrowing::validate_targets;
use crate::typestate_cx::PurposeTag;

/// One original native plan and its exact caller-selected restriction. Public
/// envelope fields cannot substitute another run, operation or target set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedNarrowingNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    operation: u64,
    retained_targets: Vec<ResolvedTarget>,
    native: FileNarrowPlanResult,
}

impl LinkedNarrowingNext {
    pub fn registration(&self) -> &Address { &self.registration }
    pub fn query_address(&self) -> &Address { &self.query_address }
    pub fn query_operation_id(&self) -> &str { &self.query_operation_id }
    pub fn query_idempotency_key(&self) -> Option<&str> { self.query_idempotency_key.as_deref() }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn retained_targets(&self) -> &[ResolvedTarget] { &self.retained_targets }
    pub fn native(&self) -> &FileNarrowPlanResult { &self.native }

    /// An executable structured command under the separately retained grant.
    /// It does not execute, reserve an operation or make an observation current.
    pub fn action(&self) -> Result<Option<TowerRequest>, TowerInstallError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Ok(None);
        };
        let Some(native) = value.request() else { return Ok(None); };
        if value.basis() != *basis || native.expected() != *basis
            || value.operation() != self.operation || native.operation() != self.operation
            || value.retained_targets() != self.retained_targets.as_slice()
            || native.retained_targets() != self.retained_targets.as_slice()
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut request = TowerRequest::new(
            format!("narrow-authority-{}", self.operation), self.registration.clone(), Verb::Narrow,
        );
        request.arguments = ReadArguments::Narrow {
            operation: self.operation, expected: *basis,
            retained_targets: self.retained_targets.clone(),
        };
        request.idempotency_key = Some(self.operation.to_string());
        Ok(Some(request))
    }
}

/// An acknowledged result tied to the exact requested restriction and original
/// predecessor. The native grant retains retries; this value contains no grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedNarrowing {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FileNarrowRequest,
    native: FileNarrowResult,
}

impl LinkedNarrowing {
    pub fn registration(&self) -> &Address { &self.registration }
    pub fn query_address(&self) -> &Address { &self.query_address }
    pub fn query_operation_id(&self) -> &str { &self.query_operation_id }
    pub fn attempted(&self) -> &FileNarrowRequest { &self.attempted }
    pub fn native(&self) -> &FileNarrowResult { &self.native }
}

impl PublicationTower {
    /// Install only a narrowing capability issued by this original owner.
    /// Rejected replacement withdraws the prior installation without spending
    /// either native grant. Equal numeric scope cannot establish ownership.
    pub fn set_narrow(
        &mut self,
        port: Option<FileSupervisorNarrowPort>,
    ) -> Result<(), TowerInstallError> {
        self.narrowing = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.narrowing = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_narrowing(
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
            ReadArguments::NarrowingNext { operation, retained_targets } if request.verb == Verb::Next => {
                if *operation == 0 || validate_targets(retained_targets).is_err()
                    || request.idempotency_key.as_ref().is_some_and(|key| key != &operation.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Some(port) = &self.narrowing else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
                };
                let native = port.next(*operation, retained_targets.clone());
                let reason = narrow_reason(&native);
                let linked = LinkedNarrowingNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    operation: *operation,
                    retained_targets: retained_targets.clone(),
                    native,
                };
                self.bounded_response(self.response(
                    &request, PublicationResult::NarrowingNext(linked), reason, true,
                ))
            }
            ReadArguments::Narrow { operation, expected, retained_targets } if request.verb == Verb::Narrow => {
                if request.address != self.mount || expected.scope != self.scope
                    || validate_targets(retained_targets).is_err()
                    || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) = FileNarrowRequest::new(*operation, *expected, retained_targets.clone()) else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let Some(port) = &self.narrowing else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
                };
                let native = port.narrow(attempted.clone());
                let reason = narrow_reason(&native);
                let linked = LinkedNarrowing {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    attempted,
                    native,
                };
                // A retry is meaningful only under the same retained grant.
                // Transport loss never triggers an implicit authority mutation.
                self.bounded_response(self.response(
                    &request, PublicationResult::Narrowing(linked), reason, false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }
}

fn narrow_reason<T>(result: &Result<ActorKnowledge<T, FileNarrowBasis>, FileNarrowError>) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown { reason: ActorUnknownReason::ControllerUnavailable }) => {
            Some(ReasonCode::LowerLayerUnverified)
        }
        Ok(ActorKnowledge::Withheld { authority_required: "supervisor_narrow" }) => {
            Some(ReasonCode::DeniedByPolicy)
        }
        Err(FileNarrowError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FileNarrowError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FileNarrowError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FileNarrowError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
