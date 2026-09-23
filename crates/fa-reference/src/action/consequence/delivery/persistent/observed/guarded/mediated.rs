//! Recover topology-observer custody together with the original guarded owner.
//! Exact graphs are independent requirements, not assumed live capture or a cut.
mod inspection;
pub use inspection::{FileMediatedSnapshot, FileTopologyUpdateRecord};

use super::{FileCredentialRegistration, FileGuardSet, FileOversightRoles, FileRecoveryRequirements};
use super::bootstrap::PreparedGuardedBootstrap;
use super::super::{BaseEvent, Event, FileOversight, FileOversightProfile, JournalError, Machine, journal, storage};
use super::super::mediation::{FileMediationObserver, MediationEvent};
use super::super::consistency::{FileConsistencyConfig, FileConsistencyObserver};
use super::super::credibility::{CredibilityEvent, FileIndependentEvaluator};
use crate::action::consequence::mediation::AuthorityGraph;
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

/// Pin both the original configuration and the last registered topology. The
/// latter survives withdrawal, so None must not silently accept an older graph.
/// Availability describes the selected disk image BEFORE the recovery fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTopologyRequirement {
    pub initial: AuthorityGraph,
    pub current: AuthorityGraph,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediatedRequirements {
    pub oversight: FileRecoveryRequirements,
    pub topology: FileTopologyRequirement,
    /// None requires absence, not acceptance of an unpinned optional gate.
    pub prediction: Option<FileConsistencyConfig>,
    pub evaluation: Option<EvaluationProtocol>,
}

/// All roles are returned once for SEPARATE custody. No old certificate,
/// forecast, identity match, human effect key or provider secret is returned.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRoles;
/// fn duplicate(roles: FileMediatedRoles) { let _ = roles.clone(); }
/// ```
#[derive(Debug)]
pub struct FileMediatedRoles {
    pub oversight: FileOversightRoles,
    pub topology_observer: FileMediationObserver,
    pub consistency_observer: Option<FileConsistencyObserver>,
    pub evaluator: Option<FileIndependentEvaluator>,
}
impl FileMediatedRoles {
    pub(super) fn provision(host: &FileOversight, oversight: FileOversightRoles, predicted: bool, evaluated: bool) -> Self {
        Self { oversight, topology_observer: FileMediationObserver { issuer: Rc::clone(&host.issuer) },
            consistency_observer: predicted.then(|| FileConsistencyObserver { issuer: Rc::clone(&host.issuer) }),
            evaluator: evaluated.then(|| FileIndependentEvaluator { issuer: Rc::clone(&host.issuer) }) }
    }
}

impl FileOversight {
    /// Original perimeter, numerical, predictive and evaluation bootstraps plus
    /// an UNCERTIFIED topology, in ONE first canonical replacement. Every native
    /// validation precedes storage creation. No extra write or live-role getter.
    pub fn create_mediated_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph, prediction: Option<FileConsistencyConfig>, evaluation: Option<EvaluationProtocol>)
        -> Result<(Self, FileMediatedRoles), JournalError>
    {
        let predicted = prediction.is_some(); let evaluated = evaluation.is_some();
        let mut prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?;
        if let Some(protocol) = evaluation { prepared = prepared.evaluated(protocol)?; }
        if let Some(config) = prediction { prepared = prepared.predictive(config)?; }
        let prepared = prepared.mediated(topology)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileMediatedRoles::provision(&host, oversight, predicted, evaluated);
        Ok((host, roles))
    }

    /// Check original AND current exact graphs, availability, every optional
    /// gate and external history floors BEFORE cleanup or the ONE native fence.
    /// That fence withdraws graph coverage. The new observer must register a
    /// newer graph/inventory and obtain a native cut before new work is admitted.
    pub fn open_mediated_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements) -> Result<(Self, FileMediatedRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_mediated_store(store, profile, expected)
    }

    fn open_mediated_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileMediatedRequirements) -> Result<(Self, FileMediatedRoles), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(&profile, store.identity(), &bytes, expected)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileMediatedRoles::provision(&host, oversight,
            expected.prediction.is_some(), expected.evaluation.is_some());
        Ok((host, roles))
    }
}

pub(super) fn check_topology(events: &[Event], expected: Option<&AuthorityGraph>) -> Result<(), Error> {
    let mut configurations = events.iter().filter_map(|event| match event {
        Event::Mediation(MediationEvent::Enable(graph)) => Some(graph), _ => None,
    });
    if configurations.next() != expected || configurations.next().is_some() { return Err(Error::Binding); }
    Ok(())
}

fn checked_image(profile: &FileOversightProfile, identity: &Path, bytes: &[u8],
    expected: &FileMediatedRequirements) -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    let machine = checked_events(profile, &events, expected)?;
    Ok((events, machine))
}

// Shared native validator after a caller-specific, internal history preflight.
// No public event import or role-provisioning bypass is exposed.
pub(super) fn checked_events(profile: &FileOversightProfile, events: &[Event],
    expected: &FileMediatedRequirements) -> Result<Machine, JournalError>
{
    expected.oversight.guards.check_mediated_replay_config(events,
        expected.prediction.as_ref(), Some(&expected.topology.initial))?;
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() { return Err(Error::Binding.into()); }
    // Preflight the final input topology too before expensive numerical replay.
    // Replay, not this scan, validates all intervening transitions and results.
    let current = events.iter().rev().find_map(|event| match event {
        Event::Mediation(MediationEvent::Update(update)) => update.next.as_ref(),
        Event::Mediation(MediationEvent::Enable(graph)) => Some(graph), _ => None,
    });
    if current != Some(&expected.topology.current) { return Err(Error::Binding.into()); }
    let machine = Machine::replay(profile, events)?;
    expected.oversight.check_mediated(profile, &machine, events, expected.evaluation.as_ref(),
        expected.prediction.as_ref(), Some(&expected.topology.initial))?;
    let actual = machine.mediation_snapshot(events.len() as u64)?;
    if actual.graph != expected.topology.current || actual.available != expected.topology.available {
        return Err(Error::Binding.into());
    }
    Ok(machine)
}

#[cfg(test)]
mod storage_tests;
