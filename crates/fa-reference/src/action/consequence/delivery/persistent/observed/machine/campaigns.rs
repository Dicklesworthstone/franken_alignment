//! Persist INPUTS to the original native campaign gate, never asserted reports.
use super::{Machine, Transition};
use super::super::governance::campaigns::CampaignEvent;
use super::super::super::governance::PolicyUpdate;
use crate::action::consequence::oversight::policy_governance::{
    CampaignDisposition, PolicyGovernor, PolicyPromotionPermit,
};
use crate::Error;
use std::collections::BTreeMap;

pub(super) struct CampaignState {
    governor: PolicyGovernor,
    requests: BTreeMap<u64, PolicyUpdate>,
    permits: BTreeMap<u64, PolicyPromotionPermit>,
}

impl Machine {
    pub(in super::super) fn campaign_request(&self, id: u64) -> Option<&PolicyUpdate> {
        self.campaigns.as_ref()?.requests.get(&id)
    }

    pub(super) fn apply_campaign(&mut self, event: &CampaignEvent) -> Result<Transition, Error> {
        match event {
            CampaignEvent::Enable(limits, maximum) => {
                if self.campaigns.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let governor = self.broker.enable_policy_campaigns(*limits, *maximum)?;
                self.campaigns = Some(CampaignState { governor, requests: BTreeMap::new(), permits: BTreeMap::new() });
            }
            CampaignEvent::Request(update) => {
                if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let state = self.campaigns.as_mut().ok_or(Error::Incomplete)?;
                if state.requests.contains_key(&update.operation()) { return Err(Error::Duplicate); }
                self.broker.request_policy_campaign(update.operation(), update.expected_control_sequence(),
                    update.expected_authority_epoch(), update.policy().clone())?;
                // The native gate bounds campaign count and retained replay bytes.
                // This additional exact request is bounded by the same count and
                // original policy syntax limits; it is not a second corpus.
                state.requests.insert(update.operation(), update.clone());
            }
            CampaignEvent::Approve(id, accept) => {
                if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let review = self.broker.policy_campaign(*id)?;
                let state = self.campaigns.as_mut().ok_or(Error::Incomplete)?;
                let permit = state.governor.approve(&review, *accept)?;
                state.permits.insert(*id, permit);
            }
            CampaignEvent::Reject(id) | CampaignEvent::Revoke(id) => {
                let review = self.broker.policy_campaign(*id)?;
                let state = self.campaigns.as_mut().ok_or(Error::Incomplete)?;
                match event {
                    CampaignEvent::Reject(_) => state.governor.reject(&review)?,
                    _ => state.governor.revoke(&review)?,
                }
                state.permits.remove(id);
            }
            CampaignEvent::Promote(id) => return self.promote_campaign(*id),
        }
        Ok(Transition::Unit)
    }

    fn promote_campaign(&mut self, id: u64) -> Result<Transition, Error> {
        if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        let update = self.campaign_request(id).ok_or(Error::Missing)?.clone();
        self.policy_updates.preflight(&update)?;
        let state = self.campaigns.as_ref().ok_or(Error::Incomplete)?;
        // Native promotion checks the complete history stamp, exact candidate,
        // independently issued approval and shadow/relaxation rules AGAIN.
        let promotion = self.broker.promote_policy(state.permits.get(&id).ok_or(Error::Missing)?)?;
        self.campaigns.as_mut().expect("configured campaign gate").permits.remove(&id);
        for attempt in &promotion.change.cancelled { self.automatic.remove(attempt); }
        self.sessions.clear();
        self.withdraw_keys()?;
        // Keep admitted envelopes and their actual outcomes. Only the original
        // policy transaction decides which undispatched rights may be refunded.
        let receipt = self.policy_updates.record(&update, promotion.change);
        Ok(Transition::PolicyUpdated(receipt))
    }

    /// Recovery and terminal stop revoke, rather than reconstruct for the caller,
    /// every outstanding native approval. Completed/rejected history is retained.
    pub(super) fn withdraw_policy_campaigns(&mut self) -> Result<(), Error> {
        let Some(state) = &mut self.campaigns else { return Ok(()); };
        for id in state.requests.keys() {
            let review = self.broker.policy_campaign(*id)?;
            if matches!(review.disposition(), CampaignDisposition::Pending | CampaignDisposition::Approved) {
                state.governor.revoke(&review)?;
            }
        }
        state.permits.clear();
        Ok(())
    }
}
