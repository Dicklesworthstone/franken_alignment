//! Jointly evaluated guarded bootstrap/recovery with separately returned roles.
//! The operator pins both evaluation contracts independently of journal bytes.
//! This reuses the existing store, native replay and single recovery fence.
use super::{FileCredibilitySnapshot, FileEvaluatedOversightRoles};
use super::super::{FileCredentialRegistration, FileGuardSet, FileOversightRoles, FileRecoveryRequirements};
use super::super::bootstrap::PreparedGuardedBootstrap;
use super::super::super::{BaseEvent, Event, FileOversight, FileOversightProfile, JournalError, Machine, journal, storage};
use super::super::super::credibility::CredibilityEvent;
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use crate::action::consequence::oversight::joint_credibility::{JointPromotionPolicy, JointPromotionReport};
use crate::Error;
use std::path::Path;

/// A historical canonical cut. No role, effect key, live clock, current source,
/// passing-population claim or authority can be recovered from this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileJointCredibilitySnapshot {
    pub credibility: FileCredibilitySnapshot,
    pub policy: JointPromotionPolicy,
    pub promotions: Vec<JointPromotionReport>,
}

impl FileOversight {
    /// Prepare every original guard AND both evaluation contracts before the
    /// first canonical image. No partially configured owner or evaluator escapes.
    pub fn create_jointly_evaluated_guarded(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol,
        joint: JointPromotionPolicy,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?
            .jointly_evaluated(protocol, joint)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Pin both contracts, the complete existing guard inventory, effective
    /// policy, credential epoch and independently retained floors BEFORE cleanup
    /// or the original recovery fence. There is no marginal-only fallback.
    pub fn open_jointly_evaluated_guarded(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol,
        joint: JointPromotionPolicy,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_jointly_evaluated_guarded_store(store, profile, expected, protocol, joint)
    }

    fn open_jointly_evaluated_guarded_store(
        store: storage::Store,
        profile: FileOversightProfile,
        expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol,
        joint: JointPromotionPolicy,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = checked_joint_image(&profile, store.identity(), &bytes, expected, protocol, joint)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Read-only replay beside a locked or faulted owner. Retain the native
    /// marginal denominator and joint promotion history; no saved score is trusted.
    /// Pending and censored labels remain visible, not automatically resolved.
    pub fn read_joint_credibility(
        directory: impl AsRef<Path>,
        profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol,
        joint: JointPromotionPolicy,
    ) -> Result<FileJointCredibilitySnapshot, JournalError> {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_joint_image(profile, &identity, &bytes, expected, protocol, joint)?;
        Ok(FileJointCredibilitySnapshot {
            credibility: FileCredibilitySnapshot {
                journal: machine.snapshot(events.len()),
                report: machine.broker.credibility_report()?,
                promotions: machine.broker.credibility_promotions()?.to_vec(),
            },
            policy: joint,
            promotions: machine.broker.joint_credibility_promotions()?.to_vec(),
        })
    }
}

fn checked_joint_image(
    profile: &FileOversightProfile,
    identity: &Path,
    bytes: &[u8],
    expected: &FileRecoveryRequirements,
    protocol: &EvaluationProtocol,
    joint: JointPromotionPolicy,
) -> Result<(Vec<Event>, Machine), JournalError> {
    let events = journal::decode(profile, identity, bytes)?;
    // These preflights happen before any recorded numerical token is executed.
    expected.guards.check_decoder_config(&events)?;
    let mut configured = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(p)) => Some((p, None)),
        Event::Credibility(CredibilityEvent::EnableJoint(p, policy)) => Some((p, Some(*policy))),
        _ => None,
    });
    if configured.next() != Some((protocol, Some(joint))) || configured.next().is_some() {
        return Err(Error::Binding.into());
    }
    let machine = Machine::replay(profile, &events)?;
    expected.check_evaluated(profile, &machine, &events, Some(protocol))?;
    if machine.broker.joint_credibility_policy() != Some(joint) {
        return Err(Error::Binding.into());
    }
    Ok((events, machine))
}

#[cfg(test)]
mod tests;
