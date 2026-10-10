//! Keep original topology-observer custody across learned numerical recovery.
//! One locked history, original reset, and original recovery-fence transaction.
use super::{AuthorityGraph, CredibilityEvent, EvaluationProtocol, Event,
    FileCredentialRegistration, FileGuardSet, FileMediatedRequirements, FileMediatedRoles,
    FileOversight, FileOversightProfile, FileOversightRoles, JournalError, Machine,
    MediationEvent, PreparedGuardedBootstrap, storage};
use super::super::{anchored::FileHistoryAnchor, learned::check_profile};
use super::super::super::consistency::ConsistencyEvent;
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use std::fmt;
use std::path::Path;

/// The original exclusive learned recovery with an independently frozen graph,
/// guard and optional marginal-evaluation contract. Ready exposes only progress;
/// topology, human and evaluator custody are released after acknowledgment.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::learned::FileMediatedLearnedRecovery;
/// fn bypass(recovery: FileMediatedLearnedRecovery) { recovery.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::learned::FileMediatedLearnedRecovery;
/// fn premature(recovery: &FileMediatedLearnedRecovery) { recovery.roles(); }
/// ```
#[must_use = "finish the original mediated recovery or drop it without changing storage"]
pub struct FileMediatedLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileMediatedRequirements,
    failure: Option<Error>,
}

impl fmt::Debug for FileMediatedLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMediatedLearnedRecovery")
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Validate every native guard, the exact learned recipe, optional evaluator
    /// and initially uncertified topology before creating storage. Publish them
    /// in one first canonical image before returning any owner or separate role.
    /// A recipe-owned policy source must match the independent guard declaration
    /// exactly and is installed once by the original learned Enable.
    ///
    /// Predictor-pinned recipes belong to their distinct custody profile and
    /// are refused here. No inference, fresh identity, cut or key is invented.
    pub fn create_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph, evaluation: Option<EvaluationProtocol>, config: FileLearnedConfig,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        check_profile(guards)?;
        check_recipe(&config)?;
        let mut prepared_guards = guards.clone();
        if let Some(policy) = config.required_policy_source() {
            if guards.source != Some(policy) { return Err(Error::Binding.into()); }
            prepared_guards.source = None;
        }
        let mut prepared = PreparedGuardedBootstrap::prepare(profile, &prepared_guards, registration)?
            .learned(config)?;
        if let Some(protocol) = &evaluation { prepared = prepared.evaluated(protocol.clone())?; }
        let prepared = prepared.mediated(topology.clone())?
            .checked_mediated(guards, evaluation.as_ref(), &topology)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileMediatedRoles::provision(&host, oversight, false, evaluation.is_some());
        Ok((host, roles))
    }

    /// Bind the exact original recipe, topology inventory and evaluator before
    /// numerical replay. Current availability, all guards, policy, credential
    /// epoch and independent floors must match before Ready or a recovery write.
    pub fn begin_open_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, config: &FileLearnedConfig,
    ) -> Result<FileMediatedLearnedRecovery, JournalError> {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Additionally compare the independently retained exact original prefix
    /// against the SAME locked canonical image before any numerical replay.
    /// Counter floors alone do not distinguish equal-counter forks.
    pub fn begin_open_mediated_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, config: &FileLearnedConfig, anchor: &FileHistoryAnchor,
    ) -> Result<FileMediatedLearnedRecovery, JournalError> {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }

    /// Synchronous consumer of the same cooperative recovery. The single native
    /// fence withdraws topology availability and old approvals, pauses numerical
    /// work and preserves unknown effects. A new observer is custody, not a cut.
    pub fn open_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, config: &FileLearnedConfig,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        let mut recovery = Self::begin_open_mediated_guarded_with_learned_generation(
            directory, profile, expected, config)?;
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            recovery.advance(recovery.progress().replayed_events, 1)?;
        }
        recovery.finish()
    }
}

fn check_recipe(config: &FileLearnedConfig) -> Result<(), Error> {
    if config.required_pre_output_forecast().is_some()
        || config.required_owned_pre_output_forecast().is_some()
    { return Err(Error::Binding); }
    Ok(())
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FileMediatedRequirements,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileMediatedLearnedRecovery, JournalError>
{
    check_profile(&expected.oversight.guards)?;
    check_recipe(config)?;
    if expected.prediction.is_some() { return Err(Error::Binding.into()); }
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    expected.oversight.guards.check_mediated_replay_config(events, None, Some(&expected.topology.initial))?;
    // This profile deliberately admits only the exact optional marginal
    // evaluator and no raw or learned predictor, including standalone enables.
    if events.iter().any(|event| matches!(event,
        Event::Credibility(CredibilityEvent::EnableJoint(_, _))
        | Event::Credibility(CredibilityEvent::EnableHeldOutJoint(_))
        | Event::Consistency(ConsistencyEvent::EnableLearned(_))))
    { return Err(Error::Binding.into()); }
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() {
        return Err(Error::Binding.into());
    }
    // Compare the final supplied graph before expensive execution, while full
    // original replay still verifies every intervening update and cut result.
    let current = events.iter().rev().find_map(|event| match event {
        Event::Mediation(MediationEvent::Update(update)) => update.next.as_ref(),
        Event::Mediation(MediationEvent::Enable(graph)) => Some(graph), _ => None,
    });
    if current != Some(&expected.topology.current) { return Err(Error::Binding.into()); }
    Ok(FileMediatedLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

fn check_machine(profile: &FileOversightProfile, machine: &Machine, events: &[Event],
    expected: &FileMediatedRequirements) -> Result<(), Error>
{
    expected.oversight.check_mediated(profile, machine, events, expected.evaluation.as_ref(),
        None, Some(&expected.topology.initial))?;
    let actual = machine.mediation_snapshot(events.len() as u64)?;
    if actual.graph != expected.topology.current || actual.available != expected.topology.available {
        return Err(Error::Binding);
    }
    Ok(())
}

impl FileMediatedLearnedRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// Apply a bounded number of original reducer events and all their numerical
    /// witnesses. A final topology/guard mismatch is sticky and never reports
    /// Ready. An event is a work unit, not a bound on wall time or filesystem I/O.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = check_machine(profile, self.inner.verified_guarded_machine()?, events, &self.expected) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let (profile, events, _) = self.inner.guarded_history();
        check_machine(profile, self.inner.verified_guarded_machine()?, events, &self.expected)?;
        Ok(())
    }

    /// Recheck all independent requirements, then acknowledge the one original
    /// fence before provisioning separate topology, human, identity, governance
    /// and optional evaluator custody. The original fence withdraws the old cut;
    /// renewed permitting needs a newer graph/inventory and native certification.
    pub fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileMediatedRoles::provision(&host, oversight, false, self.expected.evaluation.is_some());
        Ok((host, roles))
    }

    /// Complete only the exact independently retained pending original reset.
    /// Check topology and every guard before and after the SAME native reset;
    /// publish its completion and the original fence in one replacement. Exact
    /// completed retries add only a fence, never another audit or incident.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }
}
