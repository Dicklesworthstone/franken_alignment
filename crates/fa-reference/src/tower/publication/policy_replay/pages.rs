//! Issued native campaign continuations through one original registration.

use super::{
    ActorError, ActorKnowledge, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerRequest, reason,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplayPageResult, MAX_POLICY_REPLAY_PAGE_QUERIES,
};
use crate::address_resolver::Address;

/// A native page sealed to the original registration, campaign and request
/// cursor. Replacing public envelope fields cannot relabel a retained page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedPolicyReplayPage {
    native: FilePolicyReplayPageResult,
    registration: Address,
    campaign: u64,
    request_cursor: Option<u64>,
}

impl LinkedPolicyReplayPage {
    pub fn native(&self) -> &FilePolicyReplayPageResult {
        &self.native
    }
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn campaign(&self) -> u64 {
        self.campaign
    }
    pub fn request_cursor(&self) -> Option<u64> {
        self.request_cursor
    }
}

impl PublicationTower {
    /// Common operator/purpose/mount admission already ran in the parent.
    pub(super) fn dispatch_policy_replay_page(
        &mut self,
        request: TowerRequest,
    ) -> PublicationResponse {
        let (campaign, request_cursor) = match &request.arguments {
            ReadArguments::PolicyReplayPaged {
                campaign,
                expected_policy_generation,
                candidate,
            } => {
                if *expected_policy_generation == 0 {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                if crate::tower::request::policy::validate_paged_wire(candidate).is_err() {
                    return self.refusal(&request, ReasonCode::BudgetExhausted, false);
                }
                (*campaign, None)
            }
            ReadArguments::PolicyReplayContinue { campaign, cursor } if *cursor != 0 => {
                (*campaign, Some(*cursor))
            }
            _ => return self.refusal(&request, ReasonCode::IntegrityFailure, false),
        };
        if campaign == 0
            || request.idempotency_key.as_deref() != Some(campaign.to_string().as_str())
        {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        }
        let Some(port) = &self.policy_replay else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
        };
        let native = match &request.arguments {
            ReadArguments::PolicyReplayPaged {
                expected_policy_generation,
                candidate,
                ..
            } => port.replay_candidate_policy_page(
                campaign,
                *expected_policy_generation,
                candidate.clone(),
            ),
            ReadArguments::PolicyReplayContinue { cursor, .. } => {
                match self
                    .policy_replay_cursors
                    .get(cursor)
                    .filter(|native| native.campaign() == campaign)
                {
                    Some(native) => port.continue_policy_replay(campaign, native),
                    None => Err(ActorError::IdempotencyConflict),
                }
            }
            _ => unreachable!("admitted page arguments"),
        };
        let status = reason(&native);
        let response = self.bounded_response(self.response(
            &request,
            PublicationResult::PolicyReplayPage(LinkedPolicyReplayPage {
                native,
                registration: self.mount.clone(),
                campaign,
                request_cursor,
            }),
            status,
            true,
        ));
        if let PublicationResult::PolicyReplayPage(linked) = &response.result
            && let Ok(ActorKnowledge::Known { value, .. }) = linked.native()
            && let Some(cursor) = value.continuation()
        {
            // These are weak native handles only, never another corpus/result
            // cache. Keep old issued handles for exact retry after output loss.
            if let Some(original) = self.policy_replay_cursors.get(&cursor.id()) {
                if original != cursor {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
            } else {
                if self.policy_replay_cursors.len() >= MAX_POLICY_REPLAY_PAGE_QUERIES {
                    return self.refusal(&request, ReasonCode::BudgetExhausted, true);
                }
                self.policy_replay_cursors
                    .insert(cursor.id(), cursor.clone());
            }
        }
        response
    }
}
