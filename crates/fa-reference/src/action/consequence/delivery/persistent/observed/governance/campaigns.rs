//! Durable campaign approval over the ORIGINAL policy-replay and promotion gate.
//! These roles are process-local capabilities, not signatures or human identity.
//! Journal replay reconstructs native keys only inside the locked owner; recovery
//! revokes outstanding campaigns before returning a fresh governor role.

mod codec;
pub(in super::super) use codec::{read, write};
#[cfg(test)]
mod tests;

use super::super::{Event, FileHumanReviewer, FileOversight, FileOversightProfile, JournalError, Transition};
use super::super::super::governance::{PolicyUpdate, PolicyUpdateReceipt};
use crate::action::consequence::oversight::policy_governance::{CampaignDisposition, PolicyCampaignReview};
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayLimits};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone)]
pub(in super::super) enum CampaignEvent {
    Enable(ReplayLimits, usize),
    Request(PolicyUpdate),
    Approve(u64, bool),
    Reject(u64),
    Revoke(u64),
    Promote(u64),
}

/// Snapshot of retained campaign evidence at one acknowledged journal revision.
/// Its disposition is historical: request policy_campaign again for current state.
/// Cloning a review never clones a governor or a promotion key.
#[derive(Clone, Debug)]
pub struct FilePolicyCampaignReview {
    issuer: Rc<()>,
    evidence: PolicyCampaignReview,
    revision: u64,
}
impl FilePolicyCampaignReview {
    pub fn id(&self) -> u64 { self.evidence.id() }
    pub fn report(&self) -> &PolicyReplayReport { self.evidence.report() }
    pub fn observed_revision(&self) -> u64 { self.revision }
    pub fn observed_disposition(&self) -> CampaignDisposition { self.evidence.disposition() }
    pub fn control_sequence(&self) -> u64 { self.evidence.control_sequence() }
    pub fn revocation_epoch(&self) -> u64 { self.evidence.revocation_epoch() }
}

/// One acknowledged approval in this live owner. Not serializable or cloneable;
/// a lost key cannot be recovered by querying an approved campaign.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::governance::campaigns::FilePolicyPromotionPermit;
/// fn duplicate(key: FilePolicyPromotionPermit) { let _ = key.clone(); }
/// ```
#[derive(Debug)]
pub struct FilePolicyPromotionPermit { issuer: Rc<()>, campaign: u64 }
impl FilePolicyPromotionPermit { pub fn campaign(&self) -> u64 { self.campaign } }

/// Provision separately from the actor, helpers and human effect reviewer. This
/// role can approve/reject/revoke exact campaigns, never grant an effect permit.
/// There is no governor getter on a running host. Explicit reopen re-provisions
/// a role only after the original durable recovery fence has revoked old keys.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::governance::campaigns::FilePolicyGovernor;
/// fn duplicate(role: FilePolicyGovernor) { let _ = role.clone(); }
/// ```
#[derive(Debug)]
pub struct FilePolicyGovernor { issuer: Rc<()> }

impl FileOversight {
    /// Irreversible opt-in before any proposal or actor request. The native gate
    /// owns limits, full-corpus replay and promotion rules. Old journal profiles
    /// remain explicit legacy profiles; enabling cannot relabel their history.
    pub fn enable_policy_campaigns(&mut self, revision: u64, limits: ReplayLimits,
        max_campaigns: usize) -> Result<FilePolicyGovernor, JournalError>
    {
        self.transact(revision, Event::Campaign(CampaignEvent::Enable(limits, max_campaigns)))?;
        Ok(FilePolicyGovernor { issuer: Rc::clone(&self.issuer) })
    }

    /// Configured mode, not a health or approval assertion.
    pub fn policy_campaigns_required(&self) -> bool { self.machine.broker.policy_campaigns_required() }

    /// Perform normal exclusive recovery, then provision a fresh governor for an
    /// already-guarded journal. Missing guard is an error, not permission to enable
    /// it late. This calls open(), including its recovery fence, before that check.
    pub fn open_with_policy_governor(directory: impl AsRef<Path>, profile: FileOversightProfile)
        -> Result<(Self, FileHumanReviewer, FilePolicyGovernor), JournalError>
    {
        let (host, human) = Self::open(directory, profile)?;
        if !host.policy_campaigns_required() { return Err(Error::Incomplete.into()); }
        let governor = FilePolicyGovernor { issuer: Rc::clone(&host.issuer) };
        Ok((host, human, governor))
    }

    /// Request replay of this EXACT proposed policy update. An exact retry returns
    /// its historical campaign, not a new corpus or approval; ID conflicts refuse.
    pub fn request_policy_campaign(&mut self, revision: u64, update: &PolicyUpdate)
        -> Result<FilePolicyCampaignReview, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(original) = self.machine.campaign_request(update.operation()) {
            if original != update { return Err(Error::Binding.into()); }
            return self.policy_campaign(update.operation());
        }
        self.transact(revision, Event::Campaign(CampaignEvent::Request(update.clone())))?;
        self.policy_campaign(update.operation())
    }

    pub fn policy_campaign(&self, id: u64) -> Result<FilePolicyCampaignReview, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FilePolicyCampaignReview { issuer: Rc::clone(&self.issuer),
            evidence: self.machine.broker.policy_campaign(id)?, revision: self.revision() })
    }

    /// The original native one-shot promotion, old-reservation cancellations,
    /// human-key withdrawal and policy receipt commit in one canonical replacement.
    /// Exact retry returns only an already acknowledged receipt. It never promotes
    /// again, revives an old effect key, or refunds an admitted/unknown effect.
    pub fn promote_policy_campaign(&mut self, revision: u64, key: &FilePolicyPromotionPermit)
        -> Result<PolicyUpdateReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &key.issuer) { return Err(Error::Binding.into()); }
        let update = self.machine.campaign_request(key.campaign).ok_or(Error::Missing)?;
        if let Some(receipt) = self.machine.policy_updates.retry(update)? { return Ok(receipt); }
        match self.transact(revision, Event::Campaign(CampaignEvent::Promote(key.campaign)))? {
            Transition::PolicyUpdated(receipt) => Ok(receipt),
            _ => unreachable!("campaign promotion transition"),
        }
    }
}

impl FilePolicyGovernor {
    fn check(&self, host: &FileOversight, review: &FilePolicyCampaignReview) -> Result<(), JournalError> {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) || !Rc::ptr_eq(&review.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    /// Explicitly accept the observed newly-reviewable set when relaxing policy.
    /// Missing observations cannot be waived by this flag. Approval is returned
    /// only AFTER persistence; repeated approval cannot recover a lost key.
    pub fn approve(&self, host: &mut FileOversight, revision: u64,
        review: &FilePolicyCampaignReview, accept_newly_reviewable: bool)
        -> Result<FilePolicyPromotionPermit, JournalError>
    {
        self.check(host, review)?;
        host.transact(revision, Event::Campaign(CampaignEvent::Approve(review.id(), accept_newly_reviewable)))?;
        Ok(FilePolicyPromotionPermit { issuer: Rc::clone(&self.issuer), campaign: review.id() })
    }

    pub fn reject(&self, host: &mut FileOversight, revision: u64,
        review: &FilePolicyCampaignReview) -> Result<(), JournalError>
    {
        self.check(host, review)?;
        host.transact(revision, Event::Campaign(CampaignEvent::Reject(review.id())))?;
        Ok(())
    }

    pub fn revoke(&self, host: &mut FileOversight, revision: u64,
        review: &FilePolicyCampaignReview) -> Result<(), JournalError>
    {
        self.check(host, review)?;
        host.transact(revision, Event::Campaign(CampaignEvent::Revoke(review.id())))?;
        Ok(())
    }
}
