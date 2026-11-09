//! Separately granted complete-corpus candidate replay on the original owner.

use super::{
    ActorError, ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult,
    PublicationTower, ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplayResult, FileSupervisorPolicyReplayPort,
};
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

/// Original native campaign data bound to the exact Tower registration and key
/// that issued it. Mutable outer envelope fields cannot move a saved result to
/// another registration generation. This observation carries no governance key.
///
/// ```compile_fail,E0451
/// use fa_reference::tower::publication::LinkedPolicyReplay;
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePolicyReplayResult;
/// use fa_reference::address_resolver::Address;
/// fn forge(native: FilePolicyReplayResult, registration: Address) -> LinkedPolicyReplay {
///     LinkedPolicyReplay { native, registration, campaign: 1 }
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedPolicyReplay {
    native: FilePolicyReplayResult,
    registration: Address,
    campaign: u64,
}

impl LinkedPolicyReplay {
    pub fn native(&self) -> &FilePolicyReplayResult {
        &self.native
    }
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn campaign(&self) -> u64 {
        self.campaign
    }
}

impl PublicationTower {
    /// Install explicit governance disclosure from the exact original owner.
    /// An actor, precheck, inspection or explanation grant cannot supply it.
    /// A rejected replacement also withdraws the previous installation.
    pub fn set_policy_replay(
        &mut self,
        port: Option<FileSupervisorPolicyReplayPort>,
    ) -> Result<(), TowerInstallError> {
        self.policy_replay = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.policy_replay = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_policy_replay(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Admit the independently configured caller before disclosing whether
        // a governance port or any historical corpus exists.
        if !access.operator {
            return self.withheld(&request);
        }
        if !access.allowed_purposes.contains(&PurposeTag::Experiment) {
            return self.refusal(&request, ReasonCode::PurposeMismatch, false);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        }
        let ReadArguments::PolicyReplay {
            campaign,
            expected_policy_generation,
            candidate,
        } = &request.arguments
        else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        };
        if *campaign == 0
            || *expected_policy_generation == 0
            || request.idempotency_key.as_deref() != Some(campaign.to_string().as_str())
        {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        }
        if super::super::request::policy::validate_wire(candidate).is_err() {
            return self.refusal(&request, ReasonCode::BudgetExhausted, false);
        }
        let Some(port) = &self.policy_replay else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
        };
        let result =
            port.replay_candidate_policy(*campaign, *expected_policy_generation, candidate.clone());
        let reason = reason(&result);
        // An exact retry obtains the native grant's original result and cut.
        // This adapter owns no second corpus, replay engine or result cache.
        self.bounded_response(self.response(
            &request,
            PublicationResult::PolicyReplay(LinkedPolicyReplay {
                native: result,
                registration: self.mount.clone(),
                campaign: *campaign,
            }),
            reason,
            true,
        ))
    }
}

fn reason(result: &FilePolicyReplayResult) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Stale { .. }) => Some(ReasonCode::StaleGeneration),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_policy_replay",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Err(ActorError::Withheld) => Some(ReasonCode::DeniedByPolicy),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
