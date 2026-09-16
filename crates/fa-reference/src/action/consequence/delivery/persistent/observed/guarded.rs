//! Recover a composed guarded owner without dropping a separately held role.
//! Requirements are independently retained operator data, not authority or a
//! signature. Every configured gate is matched before cleanup or recovery writes.

mod bootstrap;
pub use bootstrap::FileCredentialRegistration;

use super::credential::FileCredentialPolicy;
use super::decoder::{DecoderEvent, FileDecoderConfig};
use super::governance::campaigns::{CampaignEvent, FilePolicyGovernor};
use super::identity::FileIdentityObserver;
use super::source::FileSourcePolicy;
use super::{BaseEvent, Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, Machine, journal, storage};
use crate::action::consequence::activation::identity::ModelPassport;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::oversight::identity::IdentityPolicy;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;
use crate::action::consequence::oversight::policy_governance::MAX_POLICY_CAMPAIGNS;
use crate::action::consequence::policy_campaign::ReplayLimits;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileIdentityRequirement {
    pub passport: ModelPassport,
    pub policy: IdentityPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCampaignRequirement {
    pub limits: ReplayLimits,
    pub max_campaigns: usize,
}

/// Exact gate inventory: None requires ABSENCE, never "accept what is on disk".
/// First-publication revalidation is always mandatory for this entry point.
/// A policy-source replacement requires the independently retained source
/// generation here to change; recovery cannot silently accept another source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGuardSet {
    pub stream: Option<StreamProfile>,
    pub decoder: Option<FileDecoderConfig>,
    /// None requires manual-reset mode; Some pins the exact terminal-stop policy.
    pub decoder_stop: Option<HostedStopPolicy>,
    pub source: Option<FileSourcePolicy>,
    pub identity: Option<FileIdentityRequirement>,
    pub campaigns: Option<FileCampaignRequirement>,
    pub credential: Option<FileCredentialPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCredentialEpoch {
    pub generation: u64,
    pub revoked: bool,
}

/// Lower bounds retained outside the rollbackable journal. They reject a cut
/// older than these counters, NOT equal-counter forks or a stale external floor.
/// No zero/default floor is silently supplied by the recovery implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRecoveryFloor {
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecoveryRequirements {
    pub guards: FileGuardSet,
    pub effective_policy: Policy,
    /// Must be Some exactly when guards.credential is Some. Pin terminal
    /// revocation too; an older active credential must not be accepted instead.
    pub credential_epoch: Option<FileCredentialEpoch>,
    pub minimum: FileRecoveryFloor,
}

/// Provision each role to its separate custodian. No role is stored in an actor
/// port, and this bundle contains no effect key or resumed campaign approval.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileOversightRoles;
/// fn duplicate(roles: FileOversightRoles) { let _ = roles.clone(); }
/// ```
#[derive(Debug)]
pub struct FileOversightRoles {
    pub human: FileHumanReviewer,
    pub identity_observer: Option<FileIdentityObserver>,
    pub policy_governor: Option<FilePolicyGovernor>,
}

impl FileGuardSet {
    fn validate(&self) -> Result<(), Error> {
        if self.decoder_stop.is_some() && self.decoder.is_none() { return Err(Error::InvalidInput); }
        if let Some(credential) = &self.credential { credential.check()?; }
        if let Some(campaigns) = self.campaigns {
            campaigns.limits.validate()?;
            if campaigns.max_campaigns == 0 { return Err(Error::InvalidInput); }
            if campaigns.max_campaigns > MAX_POLICY_CAMPAIGNS { return Err(Error::Limit); }
        }
        Ok(())
    }

    // Reject an unexpected numerical configuration BEFORE replay can execute
    // its tokens. This is only a preflight; full semantic replay still validates
    // every enable, step, witness and recovery transition afterwards.
    fn check_decoder_config(&self, events: &[Event]) -> Result<(), Error> {
        let mut configured = events.iter().filter_map(|event| match event {
            Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()),
            _ => None,
        });
        if configured.next() != self.decoder.as_ref() || configured.next().is_some() {
            return Err(Error::Binding);
        }
        let mut stopping = events.iter().filter_map(|event| match event {
            Event::Decoder(DecoderEvent::StopPolicy(policy)) => Some(*policy),
            _ => None,
        });
        if stopping.next() != self.decoder_stop || stopping.next().is_some() {
            return Err(Error::Binding);
        }
        Ok(())
    }

    fn check(&self, machine: &Machine, events: &[Event]) -> Result<(), Error> {
        self.validate()?;
        if !machine.publication_guard { return Err(Error::Incomplete); }
        // Replay has already checked every transition, including uniqueness and
        // bootstrap order. Reading this original event does not invent a second
        // campaign configuration or bypass the native gate's limits.
        let campaigns = events.iter().find_map(|event| match event {
            Event::Campaign(CampaignEvent::Enable(limits, max_campaigns)) =>
                Some(FileCampaignRequirement { limits: *limits, max_campaigns: *max_campaigns }),
            _ => None,
        });
        let stream = machine.broker.stream_state().map(|(_, view)| view.profile());
        let source = machine.file_source_status().map(|status| status.policy);
        let identity = self.identity.as_ref().map(|expected| (&expected.passport, expected.policy));
        if stream != self.stream || source != self.source
            || machine.identity_contract() != identity || campaigns != self.campaigns
            || machine.broker.policy_campaigns_required() != self.campaigns.is_some()
            || machine.credential_policy.as_ref() != self.credential.as_ref()
            || machine.decoder_contract() != self.decoder.as_ref()
            || machine.broker.hosted_stop_policy() != self.decoder_stop
        { return Err(Error::Binding); }
        Ok(())
    }
}

impl FileRecoveryRequirements {
    fn check(&self, profile: &FileOversightProfile, machine: &Machine,
        events: &[Event]) -> Result<(), Error>
    {
        self.guards.check(machine, events)?;
        if self.guards.credential.is_some() != self.credential_epoch.is_some()
            || self.credential_epoch.is_some_and(|epoch| epoch.generation == 0)
        { return Err(Error::InvalidInput); }
        let credential_epoch = machine.credential_policy.as_ref().map(|_| FileCredentialEpoch {
            generation: machine.credential_generation, revoked: machine.credential_revoked,
        });
        if credential_epoch != self.credential_epoch
            || machine.policy_updates.current(&profile.delivery.policy) != &self.effective_policy
        { return Err(Error::Binding); }
        let revision = u64::try_from(events.len()).map_err(|_| Error::Limit)?;
        let control = machine.broker.inspect();
        if revision < self.minimum.journal_revision
            || control.sequence < self.minimum.control_sequence
            || control.ledger.epoch < self.minimum.authority_epoch
        { return Err(Error::Stale); }
        Ok(())
    }
}

impl FileOversightRoles {
    // Called only after acknowledged creation/recovery. Deliberately no public
    // constructor, clone or live-owner role getter. Credentials are not recovered.
    fn provision(host: &FileOversight, human: FileHumanReviewer) -> Self {
        let identity_observer = host.identity_checks_required().then(|| FileIdentityObserver {
            issuer: Rc::clone(&host.issuer),
        });
        let policy_governor = host.policy_campaigns_required().then(|| FilePolicyGovernor {
            issuer: Rc::clone(&host.issuer),
        });
        Self { human, identity_observer, policy_governor }
    }
}

impl FileOversight {
    /// One exclusive open and ONE original fence, followed by all independently
    /// held roles. Pin the full guard inventory, exact effective policy, credential
    /// epoch and history floor BEFORE cleanup or recovery changes the journal.
    /// Saved source/identity/clock eligibility and all old approvals stay withdrawn.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    /// fn regain_roles(host: &FileOversight) { let _ = host.roles(); }
    /// ```
    pub fn open_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements) -> Result<(Self, FileOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_guarded_store(store, profile, expected)
    }

    // Shared by the public entry point and private Store-barrier regressions.
    // No public storage/callback interface can issue a replacement owner.
    fn open_guarded_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements) -> Result<(Self, FileOversightRoles), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        expected.guards.check_decoder_config(&events)?;
        let machine = Machine::replay(&profile, &events)?;
        expected.check(&profile, &machine, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }
}

#[cfg(test)]
mod tests;
