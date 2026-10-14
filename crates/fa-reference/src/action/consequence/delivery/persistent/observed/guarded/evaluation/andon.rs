//! Atomic guarded bootstrap and anchored native evaluation/Andon recovery.
//! Every label, pending case, receipt and incident is recomputed by the original
//! Machine. No scores, label roles or compact accounting seeds are imported.
pub mod joint;

use super::{FileCredibilitySnapshot, FileEvaluatedOversightRoles};
pub use super::super::recovery::FileAndonRecovery;
use super::super::{FileCredentialRegistration, FileGuardSet, FileRecoveryRequirements};
use super::super::bootstrap::PreparedGuardedBootstrap;
use super::super::anchored::FileHistoryAnchor;
use super::super::super::{Event, FileOversight, FileOversightProfile,
    JournalError, Machine, journal, storage};
use super::super::super::credibility::{CredibilityEvent, andon::FileAndonConfig};
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use crate::action::consequence::oversight::credibility::native_andon::NativeEvaluatedFalseHold;
use crate::action::consequence::oversight::joint_credibility::JointPromotionPolicy;
use crate::round::review::andon::{AndonBudget, AndonLedger};
use crate::Error;
use std::path::Path;

/// Operator-retained expectations, never discovered from the journal under
/// recovery. The complete current window inventory includes later registrations.
/// The independent exact history anchor is additionally mandatory on open/read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonRecoveryRequirements {
    base: FileRecoveryRequirements,
    config: FileAndonConfig,
    windows: Vec<AndonBudget>,
    minimum_evaluation_revision: u64,
    // None requires the original marginal-only profile, never any policy on disk.
    joint: Option<JointPromotionPolicy>,
}
impl FileAndonRecoveryRequirements {
    pub fn new(base: FileRecoveryRequirements, config: FileAndonConfig,
        windows: Vec<AndonBudget>, minimum_evaluation_revision: u64) -> Result<Self, Error>
    {
        let inventory = FileAndonConfig::new(config.identity(), windows)?;
        let ledger = inventory.accounting()?;
        for initial in config.windows() {
            if ledger.windows().get(&initial.window).map(|state| state.budget) != Some(*initial) {
                return Err(Error::Binding);
            }
        }
        Ok(Self { base, config, windows: inventory.windows().to_vec(), minimum_evaluation_revision, joint: None })
    }
    pub fn base(&self) -> &FileRecoveryRequirements { &self.base }
    pub fn config(&self) -> &FileAndonConfig { &self.config }
    pub fn windows(&self) -> &[AndonBudget] { &self.windows }
    pub fn minimum_evaluation_revision(&self) -> u64 { self.minimum_evaluation_revision }

    // Shared by synchronous historical reads and the sealed cooperative cursor.
    // No machine or original-event importer is exposed outside guarded recovery.
    pub(in super::super) fn preflight_evaluated(&self, events: &[Event],
        protocol: &EvaluationProtocol) -> Result<(), Error>
    {
        self.preflight(events)?;
        self.base.guards.check_decoder_config(events)?;
        joint::check_protocol(events, protocol, self.joint)
    }

    pub(in super::super) fn check_replayed(&self, profile: &FileOversightProfile,
        machine: &Machine, events: &[Event], protocol: &EvaluationProtocol) -> Result<(), Error>
    {
        self.base.check_evaluated(profile, machine, events, Some(protocol))?;
        if machine.andon_contract() != Some(&self.config)
            || machine.broker.joint_credibility_policy() != self.joint
        { return Err(Error::Binding); }
        let source = machine.broker.andon_source()?;
        if source.revision() < self.minimum_evaluation_revision { return Err(Error::Stale); }
        if !source.ledger().windows().values().map(|state| state.budget).eq(self.windows.iter().copied()) {
            return Err(Error::Binding);
        }
        Ok(())
    }

    fn preflight(&self, events: &[Event]) -> Result<(), Error> {
        let mut activations = events.iter().filter_map(|event| match event {
            Event::Credibility(CredibilityEvent::EnableAndon(config)) => Some(config),
            _ => None,
        });
        if activations.next() != Some(&self.config) || activations.next().is_some() {
            return Err(Error::Binding);
        }
        let mut inventory = self.config.accounting()?;
        for event in events {
            if let Event::Credibility(CredibilityEvent::AndonWindow(window)) = event
                && !inventory.register_window(*window)? { return Err(Error::Duplicate); }
        }
        if !inventory.windows().values().map(|state| state.budget).eq(self.windows.iter().copied()) {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

/// Historical data at one validated canonical cut. Pending labels remain in
/// the credibility report; complete accounting is not a claim they resolved.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::FileNativeAndonSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn authorize(snapshot: FileNativeAndonSnapshot) -> FilePermit { snapshot }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNativeAndonSnapshot {
    pub credibility: FileCredibilitySnapshot,
    pub accounting: AndonLedger,
    pub receipts: Vec<NativeEvaluatedFalseHold>,
    pub source_revision: u64,
}

impl FileOversight {
    /// Validate all guards, native evaluation and accounting before storage is
    /// created. The ORIGINAL first replacement publishes them together; a failed
    /// or uncertain write returns neither owner nor evaluator/human roles.
    pub fn create_andon_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        protocol: EvaluationProtocol, config: &FileAndonConfig)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?
            .evaluated(protocol)?.with_andon(config)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Pin the complete accounting/evaluation contracts, all original guards,
    /// independent counter floors AND exact history before cleanup or fencing.
    /// Native replay restores source labels and receipts, not old role brands or
    /// approval keys. A fresh clock is required for every NEW Benign assessment.
    pub fn open_andon_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_andon_guarded_store(store, profile, expected, protocol, anchor)
    }

    fn open_andon_guarded_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        FileAndonRecovery::begin_store(store, profile, expected, protocol, anchor)?
            .finish_synchronously()
    }

    /// Read a historical canonical image beside a locked/faulted owner. The
    /// same anchor, inventory, floors and original replay apply. No cleanup,
    /// fresh clock, helper invocation, fence or evaluator role is issued.
    pub fn read_andon_guarded(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol, anchor: &FileHistoryAnchor)
        -> Result<FileNativeAndonSnapshot, JournalError>
    {
        profile.delivery.limits.check()?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, expected, protocol, anchor)?;
        let source = machine.broker.andon_source()?;
        let receipts = source.receipts().iter().map(|receipt| (**receipt).clone()).collect();
        Ok(FileNativeAndonSnapshot {
            credibility: FileCredibilitySnapshot { journal: machine.snapshot(events.len()),
                report: machine.broker.credibility_report()?, promotions: machine.broker.credibility_promotions()?.to_vec() },
            accounting: source.ledger().clone(), receipts, source_revision: source.revision(),
        })
    }
}

fn checked_image(profile: &FileOversightProfile, identity: &Path, bytes: &[u8],
    expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol, anchor: &FileHistoryAnchor)
    -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    anchor.check(profile, identity, &events)?;
    expected.preflight_evaluated(&events, protocol)?;
    let machine = Machine::replay(profile, &events)?;
    expected.check_replayed(profile, &machine, &events, protocol)?;
    Ok((events, machine))
}

#[cfg(test)]
mod tests;
