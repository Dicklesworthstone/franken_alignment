//! Privileged historical admission explanations from the original native owner.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileExplanationResult, FileSupervisorExplanationPort,
};
use crate::action::consequence::oversight::actor::ActorError;

impl PublicationTower {
    /// Install a separately issued policy-explanation capability. Neither an
    /// actor ticket, precheck disclosure nor situation inspection grants it.
    /// Rejected replacement withdraws the previous installation as well.
    pub fn set_explanation(
        &mut self,
        port: Option<FileSupervisorExplanationPort>,
    ) -> Result<(), TowerInstallError> {
        self.explanation = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.explanation = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_explanation(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Ordinary actor grants cannot learn whether this disclosure is
        // installed or whether the selected native request exists.
        if !access.operator {
            return self.withheld(&request);
        }
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        };
        if request.arguments != ReadArguments::None {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.explanation else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = port.explain_request(key);
        let reason = explanation_reason(&native);
        // The original admission and ending cuts remain distinct inside the
        // native profile. Neither becomes a synthetic global Tower basis.
        self.bounded_response(self.response(
            &request,
            PublicationResult::Explanation(native),
            reason,
            true,
        ))
    }
}

fn explanation_reason(result: &FileExplanationResult) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_explanation",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
