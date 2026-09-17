//! Atomic prediction bootstrap and recovery of separately held observer custody.
use super::{FileCredentialRegistration, FileGuardSet, FileOversightRoles, FileRecoveryRequirements};
use super::bootstrap::PreparedGuardedBootstrap;
use super::super::{BaseEvent, Event, FileDeliverySnapshot, FileOversight, FileOversightProfile,
    JournalError, Machine, journal, storage};
use super::super::consistency::{ConsistencyEvent, FileConsistencyConfig, FileConsistencyObserver, FileConsistencySnapshot};
use super::super::credibility::{CredibilityEvent, FileIndependentEvaluator};
use crate::action::consequence::oversight::credibility::{CredibilityReport, EvaluationProtocol};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePredictiveRequirements {
    pub oversight: FileRecoveryRequirements,
    pub prediction: FileConsistencyConfig,
    /// Exact presence: None rejects an installed evaluation protocol.
    pub evaluation: Option<EvaluationProtocol>,
}

/// Assign each role to its own custodian. Reopening never restores old forecasts
/// as fresh observations, even though their historical records remain retained.
#[derive(Debug)]
pub struct FilePredictiveRoles {
    pub oversight: FileOversightRoles,
    pub consistency_observer: FileConsistencyObserver,
    pub evaluator: Option<FileIndependentEvaluator>,
}
impl FilePredictiveRoles {
    fn provision(host: &FileOversight, oversight: FileOversightRoles, evaluated: bool) -> Self {
        Self { oversight, consistency_observer: FileConsistencyObserver { issuer: Rc::clone(&host.issuer) },
            evaluator: evaluated.then(|| FileIndependentEvaluator { issuer: Rc::clone(&host.issuer) }) }
    }
}

/// Historical data from ONE canonical image, not fresh evidence or permission.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::FilePredictiveSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn grant(snapshot: FilePredictiveSnapshot) -> FilePermit { snapshot }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePredictiveSnapshot {
    pub journal: FileDeliverySnapshot,
    pub consistency: FileConsistencySnapshot,
    pub credibility: Option<CredibilityReport>,
}
impl FileOversight {
    /// Validate every original gate transition, then publish ONCE before exposing
    /// any owner or role. Prediction is not an effect key or calibrated identity.
    pub fn create_predictive_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        prediction: FileConsistencyConfig, evaluation: Option<EvaluationProtocol>)
        -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let evaluated = evaluation.is_some();
        let mut prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?;
        if let Some(protocol) = evaluation { prepared = prepared.evaluated(protocol)?; }
        let prepared = prepared.predictive(prediction)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FilePredictiveRoles::provision(&host, oversight, evaluated);
        Ok((host, roles))
    }

    /// Pin exact coefficients, calibration, category, lifetime bounds, optional
    /// evaluator and all existing guards before replay or cleanup. The ONE native
    /// fence retains evidence and turns an unanswered forecast into lost coverage.
    pub fn open_predictive_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FilePredictiveRequirements) -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_predictive_store(store, profile, expected)
    }

    fn open_predictive_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FilePredictiveRequirements) -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(&profile, store.identity(), &bytes, expected)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FilePredictiveRoles::provision(&host, oversight, expected.evaluation.is_some());
        Ok((host, roles))
    }

    /// Read beside a locked/faulted owner. No writer lock, cleanup, time update,
    /// fence or role is created. Full historical numerical replay is extra work.
    pub fn read_predictive_consistency(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FilePredictiveRequirements) -> Result<FilePredictiveSnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, expected)?;
        let credibility = expected.evaluation.as_ref().map(|_| machine.broker.credibility_report()).transpose()?;
        Ok(FilePredictiveSnapshot { journal: machine.snapshot(events.len()),
            consistency: machine.consistency_snapshot(events.len() as u64)?, credibility })
    }
}

pub(super) fn check_prediction(events: &[Event], expected: Option<&FileConsistencyConfig>) -> Result<(), Error> {
    let mut configurations = events.iter().filter_map(|event| match event {
        Event::Consistency(ConsistencyEvent::Enable(config)) => Some(config.as_ref()), _ => None,
    });
    if configurations.next() != expected || configurations.next().is_some() { return Err(Error::Binding); }
    Ok(())
}
fn checked_image(profile: &FileOversightProfile, identity: &Path, bytes: &[u8],
    expected: &FilePredictiveRequirements) -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    expected.oversight.guards.check_replay_config(&events, Some(&expected.prediction))?;
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() { return Err(Error::Binding.into()); }
    let machine = Machine::replay(profile, &events)?;
    expected.oversight.check_predictive(profile, &machine, &events,
        expected.evaluation.as_ref(), Some(&expected.prediction))?;
    Ok((events, machine))
}

#[cfg(test)]
mod storage_tests;
