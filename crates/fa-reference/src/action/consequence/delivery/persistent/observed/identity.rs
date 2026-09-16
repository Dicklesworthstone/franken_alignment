//! Durable model-identity checks using the original broker's measurement gate.
//! The observer is separately provisioned custody, not authenticated hardware.
mod codec;
pub mod decoder;
pub(super) use codec::{read, write};
#[cfg(test)]
mod tests;

use super::{BaseEvent, Event, FileHumanReviewer, FileOversight, FileOversightProfile, JournalError, Machine, Transition, journal, storage};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::SourceFrame;
use crate::action::consequence::activation::identity::{ModelManifest, ModelPassport};
use crate::action::consequence::oversight::identity::{IdentityChallenge, IdentityInstallation, IdentityPolicy, IdentityReport, IdentityStatus};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone)]
pub(super) enum IdentityEvent {
    Enable(Rc<ModelPassport>, IdentityPolicy),
    Begin(u64, u64, u64),
    Manifest(u64, ModelManifest, ElapsedTick),
    Anchor(u64, u64, SourceFrame, ElapsedTick),
    Apply(u64, u64, u64),
    Unavailable(u64),
}

/// Original registered stimuli and measurement context, not an approval key.
/// Cloning retains evidence only. A challenge from a previous owner cannot be
/// used to continue observations after recovery, even with identical numeric IDs.
#[derive(Clone, Debug)]
pub struct FileIdentityChallenge { issuer: Rc<()>, evidence: IdentityChallenge }
impl FileIdentityChallenge {
    pub fn evidence(&self) -> &IdentityChallenge { &self.evidence }
    pub fn id(&self) -> u64 { self.evidence.id() }
}

/// A native measurement refusal can itself change state (for example expiry).
/// Such changes are COMMITTED. Mismatches also attempt the original suspension
/// transaction in the same replacement; an error there cannot erase the latch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileIdentityObservation {
    pub measurement: Result<IdentityReport, Error>,
    pub containment: Option<Result<IdentityInstallation, Error>>,
}

/// Return once at enable or pinned reopen, and provision outside actor/helper
/// authority. The role supplies manifests/frames, never an asserted match result.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::identity::FileIdentityObserver;
/// fn duplicate(role: FileIdentityObserver) { let _ = role.clone(); }
/// ```
#[derive(Debug)]
pub struct FileIdentityObserver { issuer: Rc<()> }

impl FileOversight {
    /// Enable before any proposal/request. No disable or passport replacement
    /// API exists. Every first publication also uses the original live guard.
    pub fn enable_identity_checks(&mut self, revision: u64, passport: ModelPassport,
        policy: IdentityPolicy) -> Result<FileIdentityObserver, JournalError>
    {
        self.transact(revision, Event::Identity(IdentityEvent::Enable(Rc::new(passport), policy)))?;
        Ok(FileIdentityObserver { issuer: Rc::clone(&self.issuer) })
    }
    pub fn identity_checks_required(&self) -> bool { self.machine.identity_contract().is_some() }
    pub fn identity_status(&self) -> Result<IdentityStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.identity_checks_required() && !self.clock_ready() { return Err(Error::Incomplete.into()); }
        Ok(self.machine.broker.identity_status()?)
    }
    pub fn identity_basis(&self) -> Result<u64, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.identity_basis()?)
    }
    pub fn identity_report(&self, check: u64) -> Result<IdentityReport, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.identity_report(check)?)
    }
    pub fn identity_installation(&self, check: u64) -> Result<Option<IdentityInstallation>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.identity_installation(check)?)
    }

    /// Outer failure: no acknowledged transaction. Inner refusal: the original
    /// begin was attempted and its state is committed, including capacity-driven
    /// withdrawal of a prior live identity. Always inspect both result layers.
    pub fn begin_identity_check(&mut self, revision: u64, check: u64,
        expected_sequence: u64, expected_actor_revision: u64)
        -> Result<Result<FileIdentityChallenge, Error>, JournalError>
    {
        match self.transact(revision, Event::Identity(IdentityEvent::Begin(check, expected_sequence, expected_actor_revision)))? {
            Transition::IdentityBegun(result) => Ok(result.map(|evidence| FileIdentityChallenge { issuer: Rc::clone(&self.issuer), evidence })),
            _ => unreachable!("identity begin transition"),
        }
    }

    /// Install only a completed native measurement. Matching is eligibility for
    /// a NEW congress, never automatic action approval or a recovered effect key.
    pub fn apply_identity_check(&mut self, revision: u64, challenge: &FileIdentityChallenge,
        expected_sequence: u64, expected_epoch: u64) -> Result<IdentityInstallation, JournalError>
    {
        self.check_identity_challenge(challenge)?;
        match self.transact(revision, Event::Identity(IdentityEvent::Apply(challenge.id(), expected_sequence, expected_epoch)))? {
            Transition::IdentityApplied(result) => Ok(*result),
            _ => unreachable!("identity installation transition"),
        }
    }
    pub fn identity_unavailable(&mut self, revision: u64, expected_basis: u64) -> Result<u64, JournalError> {
        self.transact(revision, Event::Identity(IdentityEvent::Unavailable(expected_basis)))?;
        self.identity_basis()
    }
    fn check_identity_challenge(&self, challenge: &FileIdentityChallenge) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &challenge.issuer) { return Err(Error::Binding.into()); }
        Ok(())
    }

    /// Pin the independently retained passport AND check policy before cleanup,
    /// recovery writes or role provisioning. Reuse the original recovery fence;
    /// old measurements remain historical and no pending challenge is resumed.
    pub fn open_with_identity_observer(directory: impl AsRef<Path>, profile: FileOversightProfile,
        passport: &ModelPassport, policy: IdentityPolicy)
        -> Result<(Self, FileHumanReviewer, FileIdentityObserver), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let machine = Machine::replay(&profile, &events)?;
        if machine.identity_contract() != Some((passport, policy)) { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let observer = FileIdentityObserver { issuer: Rc::clone(&host.issuer) };
        Ok((host, human, observer))
    }
}

impl FileIdentityObserver {
    fn check(&self, host: &FileOversight, challenge: &FileIdentityChallenge) -> Result<(), JournalError> {
        host.check_identity_challenge(challenge)?;
        if !Rc::ptr_eq(&host.issuer, &self.issuer) { return Err(Error::Binding.into()); }
        Ok(())
    }
    pub fn observe_manifest(&self, host: &mut FileOversight, revision: u64,
        challenge: &FileIdentityChallenge, manifest: ModelManifest, now: ElapsedTick)
        -> Result<FileIdentityObservation, JournalError>
    {
        self.check(host, challenge)?;
        match host.transact(revision, Event::Identity(IdentityEvent::Manifest(challenge.id(), manifest, now)))? {
            Transition::IdentityObserved(result) => Ok(*result),
            _ => unreachable!("identity manifest transition"),
        }
    }
    pub fn observe_anchor(&self, host: &mut FileOversight, revision: u64,
        challenge: &FileIdentityChallenge, anchor: u64, frame: &SourceFrame, now: ElapsedTick)
        -> Result<FileIdentityObservation, JournalError>
    {
        self.check(host, challenge)?;
        match host.transact(revision, Event::Identity(IdentityEvent::Anchor(challenge.id(), anchor, frame.clone(), now)))? {
            Transition::IdentityObserved(result) => Ok(*result),
            _ => unreachable!("identity anchor transition"),
        }
    }
}
