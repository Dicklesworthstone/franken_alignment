//! Separately granted diagnosis of the original owner's recorded publication state.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisResult, FileSupervisorHoldDiagnosisPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// An original observation sealed to its exact query, native request and host
/// registration. Public outer response fields cannot move a Known, Unknown,
/// Withheld or failed read to another request or registration generation.
/// This is observation data, not a live eligibility certificate or an action key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedHoldDiagnosis {
    registration: Address,
    query_address: Address,
    request: u64,
    native: FileHoldDiagnosisResult,
}

impl LinkedHoldDiagnosis {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn request(&self) -> u64 {
        self.request
    }
    pub fn native(&self) -> &FileHoldDiagnosisResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install finite access to recorded publication blockers from the same
    /// original owner. Actor, precheck, inspection and historical explanation
    /// grants cannot enable this read. A rejected replacement withdraws the
    /// older installation without revoking or spending either native grant.
    pub fn set_hold_diagnosis(
        &mut self,
        port: Option<FileSupervisorHoldDiagnosisPort>,
    ) -> Result<(), TowerInstallError> {
        self.hold_diagnosis = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.hold_diagnosis = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_hold_diagnosis(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Common bounded syntax, caller scope and registration checks precede
        // this read. Do not reveal installation or request existence to actors.
        if !access.operator {
            return self.withheld(&request);
        }
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        };
        if request.arguments != ReadArguments::None {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.hold_diagnosis else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = port.why_held(key);
        let reason = diagnosis_reason(&native);
        let linked = LinkedHoldDiagnosis {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            request: key,
            native,
        };
        // Every entered native query spends once. Encoding never captures a
        // second cut, refunds spent work, or evaluates a live effect gate.
        self.bounded_response(self.response(
            &request,
            PublicationResult::HoldDiagnosis(linked),
            reason,
            true,
        ))
    }
}

fn diagnosis_reason(result: &FileHoldDiagnosisResult) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_hold_diagnosis",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
