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
        self.inspection_cursor = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.inspection = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_situation(
        &mut self,
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
        if !matches!(
            request.arguments,
            ReadArguments::None | ReadArguments::SituationPage { cursor: 1.. }
        ) {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.inspection else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = match request.arguments {
            ReadArguments::None => port.situation(),
            ReadArguments::SituationPage { cursor } => {
                let Some(original) = self
                    .inspection_cursor
                    .as_ref()
                    .filter(|original| original.id() == cursor)
                else {
                    // An unissued or retired scalar cannot address native data.
                    // Preserve a different valid cursor and its unspent budget.
                    return self
                        .inspection_response(&request, Err(ActorError::IdempotencyConflict));
                };
                port.situation_page(Some(original))
            }
            _ => unreachable!("inspection arguments admitted above"),
        };
        let response = self.inspection_response(&request, native);
        // Retain at most one exact native continuation from an encodable
        // response. Reconnects keep this original facade; replacements, output
        // refusals and completed enumeration cannot rebind an old handle.
        self.inspection_cursor = match &response.result {
            PublicationResult::Situation(Ok(ActorKnowledge::Known { value, .. })) => {
                value.continuation().cloned()
            }
            _ => None,
        };
        response
    }

    fn inspection_response(
        &self,
        request: &TowerRequest,
        native: FileInspectionResult,
    ) -> PublicationResponse {
        let reason = inspection_reason(&native);
        // The native basis remains inside the original result profile. Mount
        // generation, journal revision, control sequence and recorded time are
        // different coordinates and cannot replace one another in the envelope.
        self.bounded_response(self.response(
            request,
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
        Err(ActorError::IdempotencyConflict) => Some(ReasonCode::IntegrityFailure),
        // Native inspection cannot yield a pending action, invented absence,
        // stale grant snapshot, or effect/idempotency result.
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
