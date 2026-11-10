//! Separately granted completed-congress reads from one original native owner.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileCompletedReviewExplanation, FileRefinementExplanationResult, FileReviewExplanationResult,
    FileSupervisorReviewExplanationPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// Original result sealed to the request address, registration and selected
/// round. Rebuilding an outer response cannot relabel another owner's history,
/// including Unknown/Withheld results with no native basis of their own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedReviewExplanation {
    registration: Address,
    round: u64,
    native: FileReviewExplanationResult,
}

impl LinkedReviewExplanation {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn native(&self) -> &FileReviewExplanationResult {
        &self.native
    }
}

/// A completed refinement sealed to the same original request and round.
/// Its retained judgment did not apply a consequence or supply publication keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedRefinementExplanation {
    registration: Address,
    round: u64,
    native: FileRefinementExplanationResult,
}

impl LinkedRefinementExplanation {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn native(&self) -> &FileRefinementExplanationResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install disclosure of original completed policy/congress judgments and
    /// their actual application and effect observations. Existing admission,
    /// precheck and situation grants do not enable this stronger read.
    /// A rejected replacement withdraws the previous installation.
    pub fn set_review_explanation(
        &mut self,
        port: Option<FileSupervisorReviewExplanationPort>,
    ) -> Result<(), TowerInstallError> {
        self.review_explanation = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.review_explanation = Some(port);
        }
        Ok(())
    }

    // The common dispatch path checks caller scope, operator access and mount
    // generation before reaching this separately granted historical selector.
    pub(super) fn dispatch_review_explanation(&self, request: TowerRequest) -> PublicationResponse {
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        };
        let ReadArguments::ReviewExplanation { round } = request.arguments else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        };
        if round == 0 {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.review_explanation else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = match port.explain_review_completion(key, round) {
            Ok(ActorKnowledge::Known {
                value: FileCompletedReviewExplanation::Refined(value),
                basis,
            }) => {
                let linked = LinkedRefinementExplanation {
                    registration: request.address.clone(),
                    round,
                    native: Ok(ActorKnowledge::Known { value, basis }),
                };
                return self.bounded_response(self.response(
                    &request,
                    PublicationResult::RefinementExplanation(linked),
                    None,
                    true,
                ));
            }
            Ok(ActorKnowledge::Known {
                value: FileCompletedReviewExplanation::Applied(value),
                basis,
            }) => Ok(ActorKnowledge::Known { value, basis }),
            Ok(ActorKnowledge::Unknown { reason }) => Ok(ActorKnowledge::Unknown { reason }),
            Ok(ActorKnowledge::Withheld { authority_required }) => {
                Ok(ActorKnowledge::Withheld { authority_required })
            }
            Err(error) => Err(error),
            Ok(_) => return self.refusal(&request, ReasonCode::IntegrityFailure, true),
        };
        let reason = review_reason(&native);
        let linked = LinkedReviewExplanation {
            registration: request.address.clone(),
            round,
            native,
        };
        // Each entered query consumes its original allowance, even if complete
        // output exceeds this facade's bound. No encoding retry runs replay.
        self.bounded_response(self.response(
            &request,
            PublicationResult::ReviewExplanation(linked),
            reason,
            true,
        ))
    }
}

fn review_reason(result: &FileReviewExplanationResult) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_review_explanation",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
