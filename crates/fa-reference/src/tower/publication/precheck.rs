//! Explicit exact-policy disclosure from the same original publication owner.

use super::{
    ActorKnowledge, ActorUnknownReason, FileActorPrecheckPort, PrecheckWireResponse,
    PublicationResponse, PublicationResult, PublicationTower, ReadArguments, ReasonCode,
    TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome, precheck_wire::PrecheckWireError,
};
use crate::action::consequence::oversight::actor::ActorError;

impl PublicationTower {
    /// Trusted host provisioning only. The ordinary actor request API cannot
    /// install, renew or widen disclosure. Equal native scope numbers do not
    /// establish the same original owner allocation.
    ///
    /// Every replacement first withdraws this facade's old port, including a
    /// rejected replacement. It never revokes other copies or replenishes the
    /// original query grant. The supervisor must retain its disclosure custodian.
    pub fn set_precheck(
        &mut self,
        port: Option<FileActorPrecheckPort>,
    ) -> Result<(), TowerInstallError> {
        self.precheck = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.precheck = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_precheck(&self, mut request: TowerRequest) -> PublicationResponse {
        // The parent has already checked canonical request identity, caller
        // scope and mount generation. This read needs no operator or Permit tag.
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        let arguments = std::mem::replace(&mut request.arguments, ReadArguments::None);
        let ReadArguments::Precheck { query, proposal } = arguments else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        };
        if query == 0 {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        // A malformed Tower request never enters native evaluation. Once check()
        // is entered, the original disclosure owns all charging and revocation.
        if let Err(error) = proposal.validate() {
            return self.refusal(
                &request,
                if error == ActorError::Capacity {
                    ReasonCode::BudgetExhausted
                } else {
                    ReasonCode::IntegrityFailure
                },
                true,
            );
        }
        let Some(port) = &self.precheck else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = PrecheckWireResponse {
            query: Some(query),
            result: port
                .check(query, &proposal)
                .map_err(PrecheckWireError::from),
        };
        let reason = precheck_reason(&native);
        // No ticket, admission snapshot, journal event or whole-result cache is
        // created. Witness reuse and repeated-query costs stay with the native
        // grant. A pass remains explicitly precheck-only and review-required.
        self.bounded_response(self.response(
            &request,
            PublicationResult::Precheck(native),
            reason,
            true,
        ))
    }
}

fn precheck_reason(response: &PrecheckWireResponse) -> Option<ReasonCode> {
    match &response.result {
        Err(PrecheckWireError::MalformedRequest | PrecheckWireError::UnsupportedVersion) => {
            Some(ReasonCode::IntegrityFailure)
        }
        Err(PrecheckWireError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(PrecheckWireError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Err(PrecheckWireError::Withheld) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::DeniedByPolicy,
            ..
        }) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        }) => None,
        Ok(ActorKnowledge::Stale { .. }) => Some(ReasonCode::StaleGeneration),
        Ok(ActorKnowledge::Withheld { .. }) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        // These variants are not produced by the native precheck contract.
        // Never promote an unexpected observation to a successful precheck.
        Ok(_) => Some(ReasonCode::IntegrityFailure),
    }
}
