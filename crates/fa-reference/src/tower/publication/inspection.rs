//! Privileged situation reads through a separately installed native capability.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileInspectionResult, FileSupervisorInspectionPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// An original native observation paired with the registration that issued it.
/// Its sealed registration prevents mutable envelope fields from moving the
/// captured request links to a different generation or run. It carries data,
/// not a ticket, explanation grant, source custodian or publication authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedSituation {
    native: FileInspectionResult,
    registration: Address,
    query_address: Address,
}

impl LinkedSituation {
    pub fn native(&self) -> &FileInspectionResult {
        &self.native
    }

    pub fn registration(&self) -> &Address {
        &self.registration
    }

    /// The exact original query address, including an omitted generation.
    /// Returned links always use the separately sealed registered generation.
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
}

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
        let cursor = match request.arguments {
            ReadArguments::None | ReadArguments::SituationLinks { cursor: None } => None,
            ReadArguments::SituationPage { cursor }
            | ReadArguments::SituationLinks {
                cursor: Some(cursor),
            } if cursor != 0 => Some(cursor),
            _ => return self.refusal(&request, ReasonCode::IntegrityFailure, true),
        };
        let Some(port) = &self.inspection else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = match cursor {
            None => port.situation(),
            Some(cursor) => {
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
        };
        let response = self.inspection_response(&request, native);
        // Retain at most one exact native continuation from an encodable
        // response. Reconnects keep this original facade; replacements, output
        // refusals and completed enumeration cannot rebind an old handle.
        let retained = match &response.result {
            PublicationResult::Situation(native) => Some(native),
            PublicationResult::SituationLinks(linked) => Some(linked.native()),
            _ => None,
        };
        self.inspection_cursor = match retained {
            Some(Ok(ActorKnowledge::Known { value, .. })) => {
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
        let result = if matches!(request.arguments, ReadArguments::SituationLinks { .. }) {
            PublicationResult::SituationLinks(LinkedSituation {
                native,
                registration: self.mount.clone(),
                query_address: request.address.clone(),
            })
        } else {
            PublicationResult::Situation(native)
        };
        // The native basis remains inside the original result profile. Mount
        // generation, journal revision, control sequence and recorded time are
        // different coordinates and cannot replace one another in the envelope.
        self.bounded_response(self.response(
            request,
            result,
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
