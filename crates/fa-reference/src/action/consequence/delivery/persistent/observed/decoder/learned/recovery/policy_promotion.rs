//! Explicit offline governance on the original sealed learned replay.
//! The original Request/Approve/Promote reducers remain the sole policy gate.
use super::{BaseEvent, Event, FileLearnedRecovery, FileLearnedRecoveryStatus,
    JournalError, Machine, Transition, journal};
use crate::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, JournalCapacity, governance::{PolicyUpdate, PolicyUpdateReceipt},
    observed::{governance::campaigns::CampaignEvent, guarded::anchored::FileHistoryAnchor},
};
use crate::action::consequence::{
    gate::containment::session::policy::Policy,
    oversight::policy_governance::CampaignDisposition,
    policy_campaign::{PolicyReplayReport, ReplayLimits},
};
use crate::Error;

/// Exact independently selected promotion predecessor and candidate. This is
/// trusted governance intent, not an actor request or a reusable approval key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedPolicyPromotionRequest {
    revision: u64,
    update: PolicyUpdate,
}
impl FileLearnedPolicyPromotionRequest {
    pub fn new(revision: u64, update: PolicyUpdate) -> Result<Self, Error> {
        if revision == 0 { return Err(Error::InvalidInput); }
        usize::try_from(revision).map_err(|_| Error::Limit)?;
        Ok(Self { revision, update })
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn update(&self) -> &PolicyUpdate { &self.update }
}

/// Complete historical comparison at the selected ORIGINAL cut, including on
/// an acknowledged-operation retry. This value supplies no governor or permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedPolicyPromotionReview {
    request: FileLearnedPolicyPromotionRequest,
    snapshot: FileDeliverySnapshot,
    report: PolicyReplayReport,
    limits: ReplayLimits,
}
impl FileLearnedPolicyPromotionReview {
    pub fn request(&self) -> &FileLearnedPolicyPromotionRequest { &self.request }
    pub fn snapshot(&self) -> &FileDeliverySnapshot { &self.snapshot }
    pub fn report(&self) -> &PolicyReplayReport { &self.report }
    pub fn limits(&self) -> ReplayLimits { self.limits }
}

/// Acknowledged native policy receipt and current immutable history. A retry
/// may report a historical promotion while the current policy has moved on.
/// No unfenced owner, governor, human reviewer or effect capability escapes.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedPolicyPromotionHistory;
/// fn no_authority(history: FileLearnedPolicyPromotionHistory) { history.governor(); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedPolicyPromotionHistory {
    receipt: PolicyUpdateReceipt,
    review: FileLearnedPolicyPromotionReview,
    current_policy: Policy,
    snapshot: FileDeliverySnapshot,
    anchor: FileHistoryAnchor,
    appended_events: usize,
    accepted_newly_reviewable: bool,
}
impl FileLearnedPolicyPromotionHistory {
    pub fn receipt(&self) -> &PolicyUpdateReceipt { &self.receipt }
    pub fn review(&self) -> &FileLearnedPolicyPromotionReview { &self.review }
    pub fn current_policy(&self) -> &Policy { &self.current_policy }
    pub fn snapshot(&self) -> &FileDeliverySnapshot { &self.snapshot }
    pub fn history_anchor(&self) -> &FileHistoryAnchor { &self.anchor }
    pub fn appended_events(&self) -> usize { self.appended_events }
    pub fn accepted_newly_reviewable(&self) -> bool { self.accepted_newly_reviewable }
}

pub(super) struct Selection {
    request: FileLearnedPolicyPromotionRequest,
    limits: ReplayLimits,
    review: Option<FileLearnedPolicyPromotionReview>,
}
impl Selection {
    // Called inside the SAME replay quantum before the selected original event
    // (or at Ready for a fresh operation). No second numerical replay occurs.
    pub(super) fn capture(&mut self, machine: &Machine, revision: usize) -> Result<(), Error> {
        if revision as u64 != self.request.revision || self.review.is_some() { return Ok(()); }
        if !machine.broker.policy_campaigns_required() || machine.broker.action_consistency_required() {
            return Err(Error::Binding);
        }
        let snapshot = machine.snapshot(revision);
        if snapshot.control.sequence != self.request.update.expected_control_sequence()
            || snapshot.control.ledger.epoch != self.request.update.expected_authority_epoch() {
            return Err(Error::Stale);
        }
        let report = machine.broker.replay_candidate_policy(
            self.request.update.policy().clone(), self.limits)?;
        self.review = Some(FileLearnedPolicyPromotionReview {
            request: self.request.clone(), snapshot, report, limits: self.limits,
        });
        Ok(())
    }
}

impl FileLearnedRecovery {
    pub(in super::super::super::super) fn select_policy_promotion(mut self,
        request: FileLearnedPolicyPromotionRequest, limits: ReplayLimits) -> Result<Self, JournalError>
    {
        if self.replayed != 0 || self.status != FileLearnedRecoveryStatus::Replaying
            || self.promotion.is_some() || self.investigation.is_some() {
            return Err(Error::WrongState.into());
        }
        limits.validate()?;
        let at = usize::try_from(request.revision).map_err(|_| Error::Limit)?;
        if at > self.events.len() { return Err(Error::Stale.into()); }
        // A historical selection must name the exact original campaign event,
        // never an arbitrary earlier favorable cut or a partially approved key.
        if at < self.events.len() && !matches!(self.events.get(at),
            Some(Event::Campaign(CampaignEvent::Request(actual))) if actual == &request.update) {
            return Err(Error::Binding.into());
        }
        // The plain and mediated guarded wrappers retain their own complete
        // independent inventories. A mediated caller additionally seals this
        // cursor to its exact retained source checkpoint before replay.
        if self.expected.text_stream_profile().is_none() {
            return Err(Error::Binding.into());
        }
        self.promotion = Some(Selection { request, limits, review: None });
        Ok(self)
    }

    pub(in super::super::super::super) fn policy_promotion_review(&self)
        -> Result<&FileLearnedPolicyPromotionReview, JournalError>
    {
        self.verified_guarded_machine()?;
        let review = self.promotion.as_ref().and_then(|selection| selection.review.as_ref())
            .ok_or(Error::Incomplete)?;
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        Ok(review)
    }

    pub(in super::super::super::super) fn prepare_policy_promotion(mut self,
        reviewed: &FileLearnedPolicyPromotionReview, accept_newly_reviewable: bool)
        -> Result<(Self, PolicyUpdateReceipt, usize), JournalError>
    {
        if self.policy_promotion_review()? != reviewed { return Err(Error::Binding.into()); }
        let request = &reviewed.request;
        let id = request.update.operation();
        let at = usize::try_from(request.revision).map_err(|_| Error::Limit)?;
        // Historical receipts cannot be used to reapprove a pending/revoked
        // campaign. The exact atomic original triple and acceptance bit bind
        // a retry BEFORE any candidate transition or staging cleanup.
        if let Some(receipt) = self.machine.policy_updates.retry(&request.update)? {
            if !matches!(self.events.get(at + 1), Some(Event::Campaign(
                CampaignEvent::Approve(actual, accepted))) if *actual == id && *accepted == accept_newly_reviewable)
                || !matches!(self.events.get(at + 2), Some(Event::Campaign(
                    CampaignEvent::Promote(actual))) if *actual == id) {
                return Err(Error::Binding.into());
            }
            let retained = self.machine.broker.policy_campaign(id)?;
            if retained.disposition() != CampaignDisposition::Promoted
                || retained.report() != &reviewed.report { return Err(Error::Binding.into()); }
            return Ok((self, receipt, 0));
        }
        if self.events.len() != at { return Err(Error::Stale.into()); }
        let reserve = self.events.iter().find_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(reserve)) => Some(*reserve), _ => None,
        });
        let capacity = JournalCapacity::new(self.profile.delivery.limits, reserve,
            self.events.len(), self.canonical.len());
        if capacity.ordinary_remaining().events < 3 { return Err(Error::Limit.into()); }
        self.events.try_reserve(3).map_err(|_| Error::Limit)?;
        self.status = FileLearnedRecoveryStatus::Interrupted;
        self.append_policy_event(CampaignEvent::Request(request.update.clone()))?;
        if self.machine.broker.policy_campaign(id)?.report() != &reviewed.report {
            return Err(Error::Binding.into());
        }
        self.append_policy_event(CampaignEvent::Approve(id, accept_newly_reviewable))?;
        let receipt = match self.append_policy_event(CampaignEvent::Promote(id))? {
            Transition::PolicyUpdated(receipt) => receipt,
            _ => return Err(Error::WrongState.into()),
        };
        self.status = FileLearnedRecoveryStatus::Ready;
        Ok((self, receipt, 3))
    }

    fn append_policy_event(&mut self, campaign: CampaignEvent) -> Result<Transition, JournalError> {
        let event = Event::Campaign(campaign);
        // Each native event pays its original ordinary event/byte allowance.
        // All three stay private until the single canonical replacement below.
        journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let transition = self.machine.apply(&event)?;
        self.events.push(event);
        self.replayed = self.events.len();
        Ok(transition)
    }

    pub(in super::super::super::super) fn finish_policy_promotion(self,
        receipt: PolicyUpdateReceipt, appended_events: usize, accept_newly_reviewable: bool)
        -> Result<FileLearnedPolicyPromotionHistory, JournalError>
    {
        let machine = self.verified_guarded_machine()?;
        let review = self.promotion.as_ref().and_then(|selection| selection.review.clone())
            .ok_or(Error::Incomplete)?;
        let snapshot = machine.snapshot(self.events.len());
        let current_policy = machine.policy_updates.current(&self.profile.delivery.policy).clone();
        let bytes = if appended_events == 0 { self.canonical.clone() }
            else { journal::encode(&self.profile, self.store.identity(), &self.events)? };
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        self.store.confirm_and_cleanup()?;
        if appended_events != 0 { self.store.replace(&bytes)?; }
        let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), bytes);
        Ok(FileLearnedPolicyPromotionHistory { receipt, review, current_policy, snapshot,
            anchor, appended_events, accepted_newly_reviewable: accept_newly_reviewable })
    }
}
