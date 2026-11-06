//! Privileged situation reads through a separately installed native capability.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileInspectionResult, FileSupervisorInspectionPort,
};
use crate::action::consequence::oversight::actor::ActorError;

impl PublicationTower {
    /// Host-only installation of a supervisor read capability. Actor/precheck
    /// privileges never acquire it. The same original owner allocation is
    /// required; matching numeric scope does not establish that relationship.
    /// Every replacement first withdraws the older port, including rejection.
    /// Other clones and the original custodian keep their existing allowance.
    pub fn set_inspection(
        &mut self,
        port: Option<FileSupervisorInspectionPort>,
    ) -> Result<(), TowerInstallError> {
        self.inspection = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.inspection = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_situation(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Common syntax, tenant/plane/authority and registration checks have
        // already run. Native situation details require the separate operator
        // role as well, before discovering whether this grant is installed.
        if !access.operator {
            return self.withheld(&request);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        if request.arguments != ReadArguments::None {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.inspection else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = port.situation();
        let reason = inspection_reason(&native);
        // The native basis remains inside the original result profile. Mount
        // generation, journal revision, control sequence and recorded time are
        // different coordinates and cannot replace one another in the envelope.
        self.bounded_response(self.response(
            &request,
            PublicationResult::Situation(native),
            reason,
            true,
        ))
    }
}

fn inspection_reason(result: &FileInspectionResult) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_inspection",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        // Native inspection cannot yield a pending action, invented absence,
        // stale grant snapshot, or effect/idempotency result.
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
