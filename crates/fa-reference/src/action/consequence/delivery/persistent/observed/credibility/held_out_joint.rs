//! Persist the ORIGINAL held-out joint policy, never scores or replay verdicts.
//! The ordinary activation/withdrawal and publication paths remain authoritative.

pub mod publication;

pub use crate::action::consequence::gate::containment::session::policy::controller::credibility::joint::{
    HeldOutJointBudget, HeldOutJointPolicy, HeldOutJointReport,
};
use super::CredibilityEvent;
use super::super::{BaseEvent, Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, Machine, journal, storage};
use super::super::super::{FileDeliverySnapshot, codec::shared::{Reader, Writer}};
use crate::Error;
use std::path::Path;

/// Historical data from one canonical image; no writer or reviewer is issued.
/// Neither this report nor its retained policy is a live qualification claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHeldOutJointSnapshot {
    pub journal: FileDeliverySnapshot,
    pub policy: HeldOutJointPolicy,
    pub promotions: Vec<(u64, HeldOutJointReport)>,
}

impl FileOversight {
    /// One immutable bootstrap event. Late selection and conflicting evaluation
    /// lanes refuse; the configured policy cannot disappear on later activation.
    pub fn enable_held_out_joint(&mut self, revision: u64, policy: HeldOutJointPolicy)
        -> Result<(), JournalError>
    {
        self.transact(revision, Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)))?;
        Ok(())
    }
    pub fn held_out_joint_policy(&self) -> Result<Option<HeldOutJointPolicy>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.held_out_joint_policy())
    }
    pub fn held_out_joint_report(&self, operation: u64) -> Result<Option<&HeldOutJointReport>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.held_out_joint_report(operation).map_err(Into::into)
    }

    /// Prepare the native guard and mandatory two-key/first-publication profile
    /// BEFORE storage, then publish ONE first image before returning either role.
    pub fn create_with_held_out_joint(directory: impl AsRef<Path>, profile: FileOversightProfile,
        policy: HeldOutJointPolicy) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let prepared = Prepared::new(profile, policy)?;
        prepared.publish(storage::Store::create(directory.as_ref())?)
    }

    /// Pin this joint policy and the existing bootstrap before cleanup/fencing.
    /// This does not add independent history floors or pin other runtime guards;
    /// those retain their original replay checks. No guard is disabled here.
    pub fn open_with_held_out_joint(directory: impl AsRef<Path>, profile: FileOversightProfile,
        policy: HeldOutJointPolicy) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        Self::open_held_out_joint_store(storage::Store::open(directory.as_ref())?, profile, policy)
    }
    fn open_held_out_joint_store(store: storage::Store, profile: FileOversightProfile,
        policy: HeldOutJointPolicy) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(&profile, store.identity(), &bytes, policy)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        // Original recovery withdraws old keys/qualification and preserves every
        // pending liability. A historical report never clears that invalidation.
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Inspect beside a locked or faulted owner, without cleanup or mutation.
    /// Every report is recomputed by native activation during pure RAM replay.
    pub fn read_held_out_joint(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        policy: HeldOutJointPolicy) -> Result<FileHeldOutJointSnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, policy)?;
        let promotions = machine.broker.credibility_changes().map(|change| {
            let report = machine.broker.held_out_joint_report(change.operation)?
                .ok_or(Error::Binding)?.clone();
            Ok((change.operation, report))
        }).collect::<Result<Vec<_>, Error>>()?;
        Ok(FileHeldOutJointSnapshot { journal: machine.snapshot(events.len()), policy, promotions })
    }
}

// Private preparation/storage seam for the original barrier tests. No caller
// can inject a mutable machine, saved outcome, callback or alternative endpoint.
struct Prepared { profile: FileOversightProfile, events: Vec<Event>, machine: Machine }
impl Prepared {
    fn new(profile: FileOversightProfile, policy: HeldOutJointPolicy) -> Result<Self, JournalError> {
        let events = vec![Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy))];
        let machine = Machine::replay(&profile, &events)?;
        Ok(Self { profile, events, machine })
    }
    fn publish(self, store: storage::Store) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        let bytes = journal::encode(&self.profile, store.identity(), &self.events)?;
        store.replace(&bytes)?;
        Ok(FileOversight::owner(self.profile, store, self.events, self.machine))
    }
}
fn checked_image(profile: &FileOversightProfile, identity: &Path, bytes: &[u8],
    policy: HeldOutJointPolicy) -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    let mut selected = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)) => Some(*policy),
        _ => None,
    });
    if selected.next() != Some(policy) || selected.next().is_some() { return Err(Error::Binding.into()); }
    let machine = Machine::replay(profile, &events)?;
    if machine.broker.held_out_joint_policy() != Some(policy) { return Err(Error::Binding.into()); }
    Ok((events, machine))
}

pub(super) fn write_policy(w: &mut Writer, p: HeldOutJointPolicy) -> Result<(), Error> {
    for value in [p.id(), p.generation(), p.minimum_safe_roots(), p.minimum_violation_roots(),
        p.maximum_escape_ppm(), p.maximum_false_stop_ppm(),
        u64::try_from(p.budget().cases).map_err(|_| Error::Limit)?,
        u64::try_from(p.budget().member_outcomes).map_err(|_| Error::Limit)?] { w.u64(value)?; }
    Ok(())
}
pub(super) fn read_policy(r: &mut Reader<'_>) -> Result<HeldOutJointPolicy, Error> {
    HeldOutJointPolicy::new(r.u64()?, r.u64()?, r.u64()?, r.u64()?, r.u64()?, r.u64()?,
        HeldOutJointBudget { cases: usize::try_from(r.u64()?).map_err(|_| Error::Limit)?,
            member_outcomes: usize::try_from(r.u64()?).map_err(|_| Error::Limit)? })
}

#[cfg(test)]
mod tests;
