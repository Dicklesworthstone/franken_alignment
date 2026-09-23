//! Evaluated guarded startup/recovery without a live evaluator-role getter.
pub mod joint;
use super::{FileCredentialRegistration, FileGuardSet, FileOversightRoles, FileRecoveryRequirements};
use super::bootstrap::PreparedGuardedBootstrap;
use super::super::{BaseEvent, Event, FileDeliverySnapshot, FileOversight, FileOversightProfile,
    JournalError, Machine, journal, storage};
use super::super::credibility::{CredibilityEvent, FileIndependentEvaluator};
use crate::action::consequence::oversight::credibility::{CredibilityPromotion, CredibilityReport, EvaluationProtocol};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

/// Separate custody for the independent evaluator and every existing role.
/// No effect permit or native label-minting object is reconstructed for an actor.
#[derive(Debug)]
pub struct FileEvaluatedOversightRoles {
    pub oversight: FileOversightRoles,
    pub evaluator: FileIndependentEvaluator,
}
impl FileEvaluatedOversightRoles {
    pub(super) fn provision(host: &FileOversight, oversight: FileOversightRoles) -> Self {
        Self { oversight, evaluator: FileIndependentEvaluator { issuer: Rc::clone(&host.issuer) } }
    }
}

/// One historical canonical cut, including the original denominator and weight
/// transitions. Not a fresh observation, authenticated head or live authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::FileCredibilitySnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn authorize(report: FileCredibilitySnapshot) -> FilePermit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilitySnapshot {
    pub journal: FileDeliverySnapshot,
    pub report: CredibilityReport,
    pub promotions: Vec<CredibilityPromotion>,
}

impl FileOversight {
    /// Prepare ALL existing guards plus the native evaluation protocol before
    /// creating storage. Return every separately held role only after the same
    /// original first-image publication acknowledges. No intermediate owner.
    pub fn create_evaluated_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol) -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?.evaluated(protocol)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Exact protocol, every existing guard and independent floors are checked
    /// before cleanup or the ONE original recovery fence. Labels, pending cases
    /// and promotions survive; old evaluator roles/tickets and approvals do not.
    pub fn open_evaluated_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_evaluated_guarded_store(store, profile, expected, protocol)
    }

    fn open_evaluated_guarded_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(&profile, store.identity(), &bytes, expected, protocol)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Inspect one entire canonical image beside a locked/faulted owner. No
    /// writer, cleanup, fence, clock update, helper invocation or role issuance.
    /// Native replay recomputes metrics/weights; saved reports are never imported.
    pub fn read_credibility(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol)
        -> Result<FileCredibilitySnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, expected, protocol)?;
        Ok(FileCredibilitySnapshot { journal: machine.snapshot(events.len()),
            report: machine.broker.credibility_report()?,
            promotions: machine.broker.credibility_promotions()?.to_vec() })
    }
}

fn check_protocol(events: &[Event], expected: &EvaluationProtocol) -> Result<(), Error> {
    let mut configured = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol),
        _ => None,
    });
    if configured.next() != Some(expected) || configured.next().is_some() { return Err(Error::Binding); }
    Ok(())
}
fn checked_image(profile: &FileOversightProfile, identity: &Path, bytes: &[u8],
    expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol) -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    let machine = checked_events(profile, &events, expected, protocol)?;
    Ok((events, machine))
}

// Shared native validator after a caller-specific, internal history preflight.
// No public event import or role-provisioning bypass is exposed.
pub(super) fn checked_events(profile: &FileOversightProfile, events: &[Event],
    expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol) -> Result<Machine, JournalError>
{
    // Refuse unexpected numerical/evaluation contracts before token execution.
    expected.guards.check_decoder_config(events)?;
    check_protocol(events, protocol)?;
    let machine = Machine::replay(profile, events)?;
    expected.check_evaluated(profile, &machine, events, Some(protocol))?;
    Ok(machine)
}

#[cfg(test)]
mod storage_tests;
