//! Campaign-required exact-policy promotion through the existing owning broker.
//! The separate governor is a local role, not a signature or independent label.

use super::OversightBroker;
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::gate::containment::session::policy::controller::PolicyChange;
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayLimits, MAX_REPLAY_INPUT_BYTES};
use crate::Error;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_POLICY_CAMPAIGNS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignDisposition { Pending, Approved, Rejected, Revoked, Promoted }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryStamp {
    sequence: u64,
    epoch: u64,
    policy_generation: u64,
    congress_generation: u64,
    history: (usize, usize),
    started_rounds: usize,
    actor_revision: u64,
}

#[derive(Debug)]
struct Binding {
    issuer: Rc<()>,
    id: u64,
    stamp: HistoryStamp,
    report: Rc<PolicyReplayReport>,
    disposition: Cell<CampaignDisposition>,
}

/// Immutable campaign evidence; cloning cannot clone approval or a governor.
#[derive(Clone, Debug)]
pub struct PolicyCampaignReview { binding: Rc<Binding> }

impl PolicyCampaignReview {
    pub fn id(&self) -> u64 { self.binding.id }
    pub fn report(&self) -> &PolicyReplayReport { &self.binding.report }
    pub fn disposition(&self) -> CampaignDisposition { self.binding.disposition.get() }
    pub fn control_sequence(&self) -> u64 { self.binding.stamp.sequence }
    pub fn revocation_epoch(&self) -> u64 { self.binding.stamp.epoch }
}

/// Exact-candidate approval, not an effect Permit. One successful promotion
/// permanently consumes it; a failed/stale attempt never refunds effect rights.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::policy_governance::PolicyPromotionPermit;
/// fn duplicate(key: PolicyPromotionPermit) { let _other = key.clone(); }
/// ```
#[derive(Debug)]
pub struct PolicyPromotionPermit {
    binding: Rc<Binding>,
    accepted_relaxations: bool,
}

#[derive(Debug)]
pub struct PolicyGovernor { issuer: Rc<()> }

impl PolicyGovernor {
    pub fn approve(&self, review: &PolicyCampaignReview, accept_newly_reviewable: bool) -> Result<PolicyPromotionPermit, Error> {
        self.matches(review)?;
        if review.disposition() != CampaignDisposition::Pending { return Err(Error::WrongState); }
        if review.report().requires_shadow() { return Err(Error::Incomplete); }
        if !accept_newly_reviewable && !review.report().newly_reviewable().is_empty() { return Err(Error::Binding); }
        review.binding.disposition.set(CampaignDisposition::Approved);
        Ok(PolicyPromotionPermit { binding: Rc::clone(&review.binding), accepted_relaxations: accept_newly_reviewable })
    }

    pub fn reject(&self, review: &PolicyCampaignReview) -> Result<(), Error> {
        self.matches(review)?;
        match review.disposition() {
            CampaignDisposition::Rejected => Ok(()),
            CampaignDisposition::Pending => { review.binding.disposition.set(CampaignDisposition::Rejected); Ok(()) }
            _ => Err(Error::WrongState),
        }
    }

    pub fn revoke(&self, review: &PolicyCampaignReview) -> Result<(), Error> {
        self.matches(review)?;
        match review.disposition() {
            CampaignDisposition::Revoked => Ok(()),
            CampaignDisposition::Pending | CampaignDisposition::Approved => {
                review.binding.disposition.set(CampaignDisposition::Revoked); Ok(())
            }
            _ => Err(Error::WrongState),
        }
    }

    fn matches(&self, review: &PolicyCampaignReview) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &review.binding.issuer) { return Err(Error::Binding); }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyPromotion {
    pub campaign: u64,
    pub report: Rc<PolicyReplayReport>,
    pub change: PolicyChange,
    pub accepted_relaxations: bool,
}

#[derive(Debug)]
pub(super) struct PolicyCampaignGate {
    issuer: Rc<()>,
    limits: ReplayLimits,
    max_campaigns: usize,
    retained_bytes: usize,
    reviews: BTreeMap<u64, Rc<Binding>>,
    promotions: Vec<PolicyPromotion>,
}

impl OversightBroker {
    /// Freeze the mandatory mode before work begins. No disable, governor getter,
    /// actor reset or ordinary replace_policy call can bypass it afterwards.
    pub fn enable_policy_campaigns(&mut self, limits: ReplayLimits, max_campaigns: usize) -> Result<PolicyGovernor, Error> {
        if self.policy_campaigns.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        limits.validate()?;
        if max_campaigns == 0 { return Err(Error::InvalidInput); }
        if max_campaigns > MAX_POLICY_CAMPAIGNS { return Err(Error::Limit); }
        let issuer = Rc::new(());
        self.policy_campaigns = Some(PolicyCampaignGate { issuer: Rc::clone(&issuer), limits,
            max_campaigns, retained_bytes: 0, reviews: BTreeMap::new(), promotions: Vec::new() });
        Ok(PolicyGovernor { issuer })
    }

    pub fn policy_campaigns_required(&self) -> bool { self.policy_campaigns.is_some() }

    pub fn request_policy_campaign(
        &mut self, id: u64, expected_sequence: u64, expected_epoch: u64, candidate: Policy,
    ) -> Result<PolicyCampaignReview, Error> {
        if id == 0 { return Err(Error::InvalidInput); }
        let stamp = self.policy_campaign_stamp();
        if stamp.sequence != expected_sequence || stamp.epoch != expected_epoch { return Err(Error::Stale); }
        let gate = self.policy_campaigns.as_ref().ok_or(Error::Incomplete)?;
        if gate.reviews.contains_key(&id) || gate.reviews.values().any(|binding|
            binding.stamp == stamp && binding.report.candidate_policy() == &candidate)
        { return Err(Error::Duplicate); }
        if gate.reviews.len() >= gate.max_campaigns { return Err(Error::Limit); }
        // No caller-selected sample: include every complete proposal (including
        // exact denials) and every applied review under the current exact policy.
        let report = self.delivery.controller().replay_candidate_policy(candidate, gate.limits)?;
        let retained = gate.retained_bytes.checked_add(report.input_bytes()).ok_or(Error::Limit)?;
        if retained > MAX_REPLAY_INPUT_BYTES { return Err(Error::Limit); }
        let binding = Rc::new(Binding { issuer: Rc::clone(&gate.issuer), id, stamp,
            report: Rc::new(report), disposition: Cell::new(CampaignDisposition::Pending) });
        let gate = self.policy_campaigns.as_mut().expect("configured campaign gate");
        gate.retained_bytes = retained;
        gate.reviews.insert(id, Rc::clone(&binding));
        Ok(PolicyCampaignReview { binding })
    }

    /// Retained evidence only; a lost one-use approval is not recovered here.
    pub fn policy_campaign(&self, id: u64) -> Result<PolicyCampaignReview, Error> {
        let gate = self.policy_campaigns.as_ref().ok_or(Error::Incomplete)?;
        let binding = gate.reviews.get(&id).ok_or(Error::Missing)?;
        Ok(PolicyCampaignReview { binding: Rc::clone(binding) })
    }

    pub fn promote_policy(&mut self, permit: &PolicyPromotionPermit) -> Result<PolicyPromotion, Error> {
        let gate = self.policy_campaigns.as_ref().ok_or(Error::Incomplete)?;
        let binding = &permit.binding;
        if !Rc::ptr_eq(&gate.issuer, &binding.issuer)
            || !gate.reviews.get(&binding.id).is_some_and(|actual| Rc::ptr_eq(actual, binding))
        { return Err(Error::Binding); }
        if binding.disposition.get() != CampaignDisposition::Approved { return Err(Error::WrongState); }
        if self.policy_campaign_stamp() != binding.stamp { return Err(Error::Stale); }
        if binding.report.requires_shadow() { return Err(Error::Incomplete); }
        if !permit.accepted_relaxations && !binding.report.newly_reviewable().is_empty() { return Err(Error::Binding); }
        // Invoke the original transaction; it preserves dispatched liabilities,
        // narrower ceilings and suspension while fencing/refunding old reservations.
        // No user callback or interleaving can change the approval in this model.
        let change = self.delivery.replace_policy(binding.stamp.sequence, binding.stamp.epoch,
            binding.report.candidate_policy().clone())?;
        let promotion = PolicyPromotion { campaign: binding.id, report: Rc::clone(&binding.report),
            change, accepted_relaxations: permit.accepted_relaxations };
        binding.disposition.set(CampaignDisposition::Promoted);
        for slot in self.inputs.values_mut() { slot.approved = None; }
        self.policy_campaigns.as_mut().expect("configured campaign gate").promotions.push(promotion.clone());
        Ok(promotion)
    }

    pub fn policy_promotions(&self) -> Result<&[PolicyPromotion], Error> {
        Ok(&self.policy_campaigns.as_ref().ok_or(Error::Incomplete)?.promotions)
    }

    fn policy_campaign_stamp(&self) -> HistoryStamp {
        let state = self.inspect();
        let controller = self.delivery.controller();
        HistoryStamp { sequence: state.sequence, epoch: state.ledger.epoch,
            policy_generation: controller.policy().generation(),
            congress_generation: controller.congress_policy().generation,
            history: controller.policy_replay_revision(), started_rounds: self.started_rounds.len(),
            actor_revision: self.actor_revision() }
    }
}
