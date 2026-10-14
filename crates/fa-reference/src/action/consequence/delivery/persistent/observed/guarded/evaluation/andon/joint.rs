//! Compose original joint promotion safeguards with durable native Andon accounting.
//! All requirements are independently chosen; absence is not a wildcard. The
//! existing journal, native reweighting rule and joint replay remain authoritative.
use super::{CredibilityEvent, EvaluationProtocol, Event, FileAndonConfig,
    FileAndonRecoveryRequirements, FileCredentialRegistration, FileEvaluatedOversightRoles,
    FileGuardSet, FileOversight, FileOversightProfile, JointPromotionPolicy, JournalError,
    PreparedGuardedBootstrap, storage};
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::Error;
use std::path::Path;

impl FileAndonRecoveryRequirements {
    /// Explicit opt-in before recovery. Pin EVERY original joint-policy field,
    /// including replay-work ceilings. No policy is learned from the journal.
    pub fn require_joint(mut self, policy: JointPromotionPolicy) -> Result<Self, Error> {
        if self.joint.is_some() { return Err(Error::Duplicate); }
        self.joint = Some(policy);
        Ok(self)
    }

    pub fn joint_policy(&self) -> Option<JointPromotionPolicy> { self.joint }
}

// Match exactly one original evaluation activation before any event executes.
// Mixed, duplicate, missing or differently scoped activations cannot pass by
// selecting only the matching kind and silently ignoring the other kind.
pub(super) fn check_protocol(events: &[Event], protocol: &EvaluationProtocol,
    joint: Option<JointPromotionPolicy>) -> Result<(), Error>
{
    let mut activations = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(actual)) => Some((actual, None)),
        Event::Credibility(CredibilityEvent::EnableJoint(actual, policy)) => Some((actual, Some(*policy))),
        _ => None,
    });
    if activations.next() != Some((protocol, joint)) || activations.next().is_some() {
        return Err(Error::Binding);
    }
    Ok(())
}

impl FileOversight {
    /// Publish ordinary guards, the ORIGINAL jointly evaluated protocol and
    /// native accounting in one first image. Never return a pre-accounting or
    /// marginal-only owner. This is NEW domain creation, not history recovery.
    pub fn create_joint_andon_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol, policy: JointPromotionPolicy, config: &FileAndonConfig)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?
            .jointly_evaluated(protocol, policy)?.with_andon(config)?;
        publish(prepared, directory.as_ref())
    }

    /// Same reserved-first bootstrap used by live publication, with the joint
    /// policy explicitly present. Existing unreserved/marginal constructors and
    /// all original encodings, capacity classes and promotion rules are unchanged.
    pub fn create_joint_andon_guarded_reserved(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol, policy: JointPromotionPolicy, config: &FileAndonConfig,
        reserve: RecoveryReserve) -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare_reserved(profile, guards, registration, reserve)?
            .jointly_evaluated(protocol, policy)?.with_andon(config)?;
        publish(prepared, directory.as_ref())
    }
}

fn publish(prepared: PreparedGuardedBootstrap, directory: &Path)
    -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError>
{
    let store = storage::Store::create(directory)?;
    let (host, oversight) = prepared.publish(store)?;
    let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
    Ok((host, roles))
}

#[cfg(test)]
mod tests;
