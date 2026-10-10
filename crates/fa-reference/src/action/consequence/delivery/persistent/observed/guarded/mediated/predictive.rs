//! Keep both topology and pre-output predictor custody through original learned
//! recovery. The exact recipe owns the predictor; no separate enable or capture
//! hydration, second numerical replay, or statistical reset is introduced.
use super::{AuthorityGraph, CredibilityEvent, EvaluationProtocol, Event,
    FileConsistencyConfig, FileCredentialRegistration, FileGuardSet, FileMediatedRoles,
    FileOversight, FileOversightProfile, FileOversightRoles, FileRecoveryRequirements,
    FileTopologyRequirement, JournalError, Machine, MediationEvent, PreparedGuardedBootstrap, storage};
use super::super::{anchored::FileHistoryAnchor, learned::check_profile};
use super::super::super::consistency::{ConsistencyEvent, learned::FileLearnedConsistencyConfig};
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use std::fmt;
use std::path::Path;

/// Independently retained predictor configuration, including its original source
/// mode, pre-output timing, complete numerical fingerprint and finite budgets.
/// A raw residual forecast and an owned learned-K/V forecast remain distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileMediatedPredictor {
    Raw(FileConsistencyConfig),
    Owned(FileLearnedConsistencyConfig),
}
impl FileMediatedPredictor {
    fn check_recipe(&self, config: &FileLearnedConfig) -> Result<(), Error> {
        let matches = match self {
            Self::Raw(prediction) => config.required_pre_output_forecast() == Some(prediction)
                && config.required_owned_pre_output_forecast().is_none(),
            Self::Owned(prediction) => prediction.uses_owned_generation()
                && config.required_owned_pre_output_forecast() == Some(prediction)
                && config.required_pre_output_forecast().is_none(),
        };
        if !matches { return Err(Error::Binding); }
        Ok(())
    }
    pub(in super::super) fn consistency(&self) -> &FileConsistencyConfig {
        match self { Self::Raw(config) => config, Self::Owned(config) => config.consistency() }
    }
    // The inner raw registration alone omits the owned representation, source
    // side, per-job work, lifetime work and inventory ceilings.
    pub(in super::super) fn check_machine(&self, machine: &Machine) -> Result<(), Error> {
        let owned = match self { Self::Raw(_) => None, Self::Owned(config) => Some(config) };
        if machine.learned_consistency.as_deref() != owned
            || machine.broker.owned_learned_consistency_required() != owned.is_some()
        { return Err(Error::Binding); }
        Ok(())
    }
}

/// All requirements are held outside the rollbackable journal. Availability
/// describes the selected disk image before the original fence withdraws it.
/// None requires the marginal evaluator to be absent; joint profiles are refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePredictiveMediatedRequirements {
    pub oversight: FileRecoveryRequirements,
    pub topology: FileTopologyRequirement,
    pub prediction: FileMediatedPredictor,
    pub evaluation: Option<EvaluationProtocol>,
}

/// One original locked learned recovery, sealed until every independent guard,
/// exact predictor, graph and counter requirement matches. Ready grants no roles.
/// The original reset retains loss, sampled-work and lifetime boundaries.
/// The returned FileMediatedRoles always contains a consistency observer.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::predictive::FilePredictiveMediatedLearnedRecovery;
/// fn bypass(run: FilePredictiveMediatedLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::predictive::FilePredictiveMediatedLearnedRecovery;
/// fn premature(run: &FilePredictiveMediatedLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "finish the original predictive mediated recovery or drop it without changing storage"]
pub struct FilePredictiveMediatedLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FilePredictiveMediatedRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FilePredictiveMediatedLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePredictiveMediatedLearnedRecovery")
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze all native guards, a required recipe-owned raw or owned predictor,
    /// optional marginal evaluation and uncertified topology before creating
    /// storage. The first canonical replacement contains the complete profile;
    /// no owner or separate role escapes an unacknowledged write.
    pub fn create_predictive_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph, prediction: FileMediatedPredictor,
        evaluation: Option<EvaluationProtocol>, config: FileLearnedConfig,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        check_profile(guards)?;
        prediction.check_recipe(&config)?;
        let mut prepared_guards = guards.clone();
        if let Some(policy) = config.required_policy_source() {
            if guards.source != Some(policy) { return Err(Error::Binding.into()); }
            prepared_guards.source = None;
        }
        // The sole learned Enable installs its pinned predictor and policy
        // source itself. Never append a second predictor/source Enable.
        let mut prepared = PreparedGuardedBootstrap::prepare(profile, &prepared_guards, registration)?
            .learned(config)?;
        if let Some(protocol) = &evaluation { prepared = prepared.evaluated(protocol.clone())?; }
        let prepared = prepared.mediated(topology.clone())?
            .checked_predictive_mediated(guards, evaluation.as_ref(), &prediction, &topology)?;
        let store = storage::Store::create(directory.as_ref())?;
        let (host, oversight) = prepared.publish(store)?;
        let roles = FileMediatedRoles::provision(&host, oversight, true, evaluation.is_some());
        Ok((host, roles))
    }

    /// Bind both original recipe and predictor before numerical replay, then
    /// check the complete mediation, evaluation, guard, policy and floor inputs.
    pub fn begin_open_predictive_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FilePredictiveMediatedRequirements, config: &FileLearnedConfig,
    ) -> Result<FilePredictiveMediatedLearnedRecovery, JournalError> {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Require the exact independently retained prefix of the SAME locked
    /// canonical image before replay. Equal counters do not identify a fork.
    pub fn begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FilePredictiveMediatedRequirements, config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<FilePredictiveMediatedLearnedRecovery, JournalError> {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }

    /// Synchronous consumer of the same original cooperative recovery and fence.
    /// Returned roles hold custody; fresh numerical/source/identity qualification
    /// and a newer graph/inventory with a native cut are still required.
    pub fn open_predictive_mediated_guarded_with_learned_generation(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FilePredictiveMediatedRequirements, config: &FileLearnedConfig,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        let mut recovery = Self::begin_open_predictive_mediated_guarded_with_learned_generation(
            directory, profile, expected, config)?;
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            recovery.advance(recovery.progress().replayed_events, 1)?;
        }
        recovery.finish()
    }
}

fn begin(directory: &Path, profile: FileOversightProfile,
    expected: &FilePredictiveMediatedRequirements, config: &FileLearnedConfig,
    anchor: Option<&FileHistoryAnchor>) -> Result<FilePredictiveMediatedLearnedRecovery, JournalError>
{
    check_profile(&expected.oversight.guards)?;
    expected.prediction.check_recipe(config)?;
    let mut inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    if let FileMediatedPredictor::Owned(prediction) = &expected.prediction {
        inner = inner.bind_owned_prediction(prediction)?;
    }
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    check_history(events, expected)?;
    Ok(FilePredictiveMediatedLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

fn check_history(events: &[Event], expected: &FilePredictiveMediatedRequirements) -> Result<(), Error> {
    // Prediction belongs to the single bound learned Enable, not an independent
    // journal Enable. Reject standalone raw/sample-decoder gates before replay.
    expected.oversight.guards.check_mediated_replay_config(
        events, None, Some(&expected.topology.initial))?;
    for event in events {
        match event {
            Event::Credibility(CredibilityEvent::EnableJoint(_, _)
                | CredibilityEvent::EnableHeldOutJoint(_)) => return Err(Error::Binding),
            Event::Consistency(event) => match (event, &expected.prediction) {
                (ConsistencyEvent::ForecastHostedRequest(..), FileMediatedPredictor::Raw(_))
                | (ConsistencyEvent::ForecastOwnedLearnedRequest(..), FileMediatedPredictor::Owned(_))
                | (ConsistencyEvent::Unavailable | ConsistencyEvent::Expire(..), _) => {}
                // No supplied capture, alternate mode or separate enable can
                // be imported into this required pre-output profile.
                _ => return Err(Error::Binding),
            },
            _ => {}
        }
    }
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() {
        return Err(Error::Binding);
    }
    let current = events.iter().rev().find_map(|event| match event {
        Event::Mediation(MediationEvent::Update(update)) => update.next.as_ref(),
        Event::Mediation(MediationEvent::Enable(graph)) => Some(graph), _ => None,
    });
    if current != Some(&expected.topology.current) { return Err(Error::Binding); }
    Ok(())
}

fn check_machine(profile: &FileOversightProfile, machine: &Machine, events: &[Event],
    expected: &FilePredictiveMediatedRequirements) -> Result<(), Error>
{
    expected.prediction.check_machine(machine)?;
    expected.oversight.check_mediated(profile, machine, events, expected.evaluation.as_ref(),
        Some(expected.prediction.consistency()), Some(&expected.topology.initial))?;
    let actual = machine.mediation_snapshot(events.len() as u64)?;
    if actual.graph != expected.topology.current || actual.available != expected.topology.available {
        return Err(Error::Binding);
    }
    Ok(())
}

impl FilePredictiveMediatedLearnedRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// Execute at most max_events original reductions and their original
    /// numerical witnesses. Final guard mismatches are sticky and never expose
    /// Ready. An event is a work unit, not a bound on synchronous I/O or wall time.
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

    /// Acknowledge exactly the original fence before returning separate human,
    /// forecast, topology, identity, governance and optional evaluator custody.
    /// Unanswered forecasts lose coverage; unknown effects remain charged.
    pub fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileMediatedRoles::provision(&host, oversight, true, self.expected.evaluation.is_some());
        Ok((host, roles))
    }

    /// Complete only the exact independently retained original pending reset,
    /// checking the full owned/raw fingerprint and all requirements around it.
    /// Reset completion and the single fence share the original replacement.
    /// Exact completed retries only fence: no extra incident, audit, source job,
    /// alpha budget, fresh forecast or erasure of abandoned sampled output.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }
}
