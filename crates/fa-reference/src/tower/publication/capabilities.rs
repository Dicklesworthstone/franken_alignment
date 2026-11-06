//! Discover only the original explicitly granted exact-precheck boundary.

use super::{
    ActorKnowledge, ActorUnknownReason, PrecheckCapabilitiesResponse, PublicationResponse,
    PublicationResult, PublicationTower, ReadArguments, ReasonCode, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::precheck_wire::PrecheckWireError;

impl PublicationTower {
    pub(super) fn dispatch_capabilities(&self, mut request: TowerRequest) -> PublicationResponse {
        // Common identity, caller admission and mount generation were checked
        // before entering this method. A grant cannot describe a foreign run.
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        let arguments = std::mem::replace(&mut request.arguments, ReadArguments::None);
        let ReadArguments::Capabilities {
            query,
            expected_policy_epoch,
        } = arguments
        else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        };
        if query == 0 {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.precheck else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        // Native discovery shares the original grant's finite query allowance
        // with precheck and every port clone. A repeated label spends again;
        // no response cache, source refresh, snapshot, ticket or event is made.
        let native = PrecheckCapabilitiesResponse {
            query: Some(query),
            result: port
                .capabilities(query, expected_policy_epoch)
                .map_err(PrecheckWireError::from),
        };
        let reason = capabilities_reason(&native);
        self.bounded_response(self.response(
            &request,
            PublicationResult::Capabilities(native),
            reason,
            true,
        ))
    }
}

fn capabilities_reason(response: &PrecheckCapabilitiesResponse) -> Option<ReasonCode> {
    match &response.result {
        Err(PrecheckWireError::MalformedRequest | PrecheckWireError::UnsupportedVersion) => {
            Some(ReasonCode::IntegrityFailure)
        }
        Err(PrecheckWireError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(PrecheckWireError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Err(PrecheckWireError::Withheld) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Stale { .. }) => Some(ReasonCode::StaleGeneration),
        Ok(ActorKnowledge::Withheld { .. }) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        // Native discovery cannot supply Pending, Absent or any other unknown
        // result. Do not turn an unexpected observation into useful discovery.
        Ok(_) => Some(ReasonCode::IntegrityFailure),
    }
}
