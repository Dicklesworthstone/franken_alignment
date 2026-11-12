//! Separately granted explicit successors to an ORIGINAL applied HoldEffect.
use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileConveneBasis, FileConveneError, FileConvenePlanResult, FileConveneRequest,
    FileConveneResult, FileSupervisorConvenePort,
};
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedConveneNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    request: u64,
    round: u64,
    native: FileConvenePlanResult,
}

impl LinkedConveneNext {
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
    pub fn request(&self) -> u64 {
        self.request
    }
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn native(&self) -> &FileConvenePlanResult {
        &self.native
    }

    /// Data for one explicit successor request; this neither queues nor opens it.
    pub fn action(&self) -> Result<Option<TowerRequest>, TowerInstallError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Ok(None);
        };
        let Some(native) = value.convene_request() else {
            return Ok(None);
        };
        if value.basis() != *basis
            || native.expected() != *basis
            || native.request() != self.request
            || native.round() != self.round
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut request = TowerRequest::new(
            format!("convene-{}", self.round),
            super::request_address_for(basis.scope, &self.registration, self.request)?,
            Verb::Convene,
        );
        request.idempotency_key = Some(self.round.to_string());
        request.arguments = ReadArguments::Convene {
            round: self.round,
            expected: *basis,
        };
        Ok(Some(request))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedConvene {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FileConveneRequest,
    native: FileConveneResult,
}

impl LinkedConvene {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn attempted(&self) -> &FileConveneRequest {
        &self.attempted
    }
    pub fn native(&self) -> &FileConveneResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install only an original same-owner grant. Replacement withdraws the old
    /// view even when the supplied replacement is foreign or otherwise invalid.
    pub fn set_convene(
        &mut self,
        port: Option<FileSupervisorConvenePort>,
    ) -> Result<(), TowerInstallError> {
        self.convening = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.convening = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_convening(
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
        let Some(port) = &self.convening else {
            return self.refusal(
                &request,
                ReasonCode::CapabilityUnsupported,
                request.verb == Verb::Next,
            );
        };
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        };
        match &request.arguments {
            ReadArguments::ConveneNext { round } if request.verb == Verb::Next => {
                if *round == 0
                    || request
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|value| value != &round.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let native = port.next(key, *round);
                let reason = convene_reason(&native);
                let linked = LinkedConveneNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    request: key,
                    round: *round,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::ConveneNext(linked),
                    reason,
                    true,
                ))
            }
            ReadArguments::Convene { round, expected } if request.verb == Verb::Convene => {
                if expected.scope != self.scope
                    || expected.request != key
                    || request.idempotency_key.as_deref() != Some(round.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) = FileConveneRequest::new(key, *round, *expected) else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let native = port.convene(attempted);
                let reason = convene_reason(&native);
                let linked = LinkedConvene {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    attempted,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::Convene(linked),
                    reason,
                    false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }
}

fn convene_reason<T>(
    result: &Result<ActorKnowledge<T, FileConveneBasis>, FileConveneError>,
) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::OutcomeUnknown,
        }) => Some(ReasonCode::MissingRequiredObservation),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_convene",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(FileConveneError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FileConveneError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FileConveneError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FileConveneError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
