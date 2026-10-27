//! Keep both topology and pre-output predictor custody through original learned
//! recovery. The exact recipe owns the predictor; no separate enable or capture
//! hydration, second numerical replay, or statistical reset is introduced.
use super::{AuthorityGraph, CredibilityEvent, EvaluationProtocol, Event,
    FileConsistencyConfig, FileCredentialRegistration, FileGuardSet, FileMediatedRoles,
    FileOversight, FileOversightProfile, FileOversightRoles, FileRecoveryRequirements,
    FileTopologyRequirement, JournalError, Machine, MediationEvent, PreparedGuardedBootstrap, storage};
use super::super::{anchored::FileHistoryAnchor, learned::check_profile,
    learned_recovery::check_recovery_reserve};
use super::super::super::consistency::{ConsistencyEvent, learned::FileLearnedConsistencyConfig};
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use super::super::super::shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory, FileStoppedRecovery};
use super::super::super::stream::FileStreamProposal;
use crate::action::consequence::delivery::credential_broker::{
    CredentialChangeReceipt, CredentialRotationRequest, CredentialRevocationRequest,
};
use crate::action::consequence::delivery::persistent::observed::credential::FileCredentialStatus;
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopRequest, persistent::RecoveryReserve};
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
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
    pub(in super::super) fn check_recipe(&self, config: &FileLearnedConfig) -> Result<(), Error> {
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
        create(directory.as_ref(), profile, guards, registration, topology, prediction, evaluation, config, None)
    }

    /// Reserve the original terminal tail before any guard or numerical work
    /// in the same first canonical image. The exact raw/owned predictor remains
    /// installed only by the original learned Enable; complete guards, topology
    /// and reserve admission precede storage and release of any separate role.
    pub fn create_predictive_mediated_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph, prediction: FileMediatedPredictor,
        evaluation: Option<EvaluationProtocol>, config: FileLearnedConfig, reserve: RecoveryReserve,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        create(directory.as_ref(), profile, guards, registration, topology, prediction, evaluation, config, Some(reserve))
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

fn create(directory: &Path, profile: FileOversightProfile, guards: &FileGuardSet,
    registration: Option<FileCredentialRegistration<'_>>, topology: AuthorityGraph,
    prediction: FileMediatedPredictor, evaluation: Option<EvaluationProtocol>,
    config: FileLearnedConfig, reserve: Option<RecoveryReserve>)
    -> Result<(FileOversight, FileMediatedRoles), JournalError>
{
    check_profile(guards)?;
    prediction.check_recipe(&config)?;
    let mut prepared_guards = guards.clone();
    if let Some(policy) = config.required_policy_source() {
        if guards.source != Some(policy) { return Err(Error::Binding.into()); }
        prepared_guards.source = None;
    }
    let prepared = match reserve {
        Some(reserve) => PreparedGuardedBootstrap::prepare_reserved(profile, &prepared_guards, registration, reserve)?,
        None => PreparedGuardedBootstrap::prepare(profile, &prepared_guards, registration)?,
    };
    // The sole learned Enable installs its pinned predictor and policy source
    // itself. The optional reserve already occupies its original pre-work slot.
    let mut prepared = prepared.learned(config)?;
    if let Some(protocol) = &evaluation { prepared = prepared.evaluated(protocol.clone())?; }
    let prepared = prepared.mediated(topology.clone())?
        .checked_predictive_mediated(guards, evaluation.as_ref(), &prediction, &topology)?;
    let store = storage::Store::create(directory)?;
    let (host, oversight) = prepared.publish(store)?;
    let roles = FileMediatedRoles::provision(&host, oversight, true, evaluation.is_some());
    Ok((host, roles))
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
    /// Restrict the still-unreplayed original cursor to one exact credential
    /// maintenance operation after retained source-checkpoint admission.
    /// Only its Fence/Rotate/Revoke suffix may explain an unexported cut.
    /// No live roles, observer, generation control or alternate operation escape.
    pub fn require_credential_checkpoint(mut self,
        saved: &super::super::super::driver::topology::checkpoint::TopologyCheckpoint,
        request: super::credential::FileCredentialMaintenanceRequest)
        -> Result<super::credential::FileCredentialTopologyRecovery, JournalError>
    {
        let progress = self.progress();
        super::credential::prepare(&self.inner, progress, &mut self.expected.topology,
            &mut self.expected.oversight.minimum, saved, request)?;
        Ok(super::credential::FileCredentialTopologyRecovery::predictive(self, saved, request))
    }

    /// Inspect credential state only after the original numerical replay and
    /// every independent topology, predictor, guard and floor check succeeds.
    /// No cleanup, fence, source observation or live role is produced.
    pub fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.credential_status())
    }

    /// Check a rotation before the recovery Fence or staging cleanup. An exact
    /// retry returns its original receipt, independently of today's generation.
    pub fn check_credential_rotation(&self, request: CredentialRotationRequest)
        -> Result<Option<CredentialChangeReceipt>, JournalError>
    {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.check_credential_rotation(request)?)
    }

    /// Check the original revocation and retained-operation identity without
    /// mutating the journal or granting an observer or publication capability.
    pub fn check_credential_revocation(&self, request: CredentialRevocationRequest)
        -> Result<Option<CredentialChangeReceipt>, JournalError>
    {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.check_credential_revocation(request)?)
    }

    /// Restrict this still-unreplayed predictive cursor to one native terminal
    /// operation after exact retained-source admission. Only the same Stop's
    /// terminal suffix can advance the cut; no source work, weaker predictor or
    /// live continuation is inferred. All original floors, guards and terminal
    /// reserve remain required, including exact-cut initial stopping.
    pub fn require_terminal_checkpoint(mut self,
        saved: &super::super::super::driver::topology::checkpoint::TopologyCheckpoint,
        operation: u64) -> Result<super::terminal::FileTerminalTopologyRecovery, JournalError>
    {
        let progress = self.progress();
        super::terminal::prepare(&self.inner, progress, &mut self.expected.topology,
            &mut self.expected.oversight.minimum, saved, operation)?;
        Ok(super::terminal::FileTerminalTopologyRecovery::predictive(self, operation))
    }

    /// Require the exact original terminal reserve from this same locked
    /// canonical image before numerical replay. Missing, changed or late
    /// requirements refuse without mutation; no allowance is replenished.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        check_recovery_reserve(&self.inner, expected)?;
        Ok(self)
    }

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

    /// Select the original retained message after the complete independent
    /// predictor, topology, guard, policy and floor contract passes. This read
    /// issues no observer, new forecast, fence or live owner.
    pub fn learned_text_stream_message_request(&self, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        self.check_ready()?;
        self.inner.learned_text_stream_message_request(request)
    }

    /// Select the exact retained finish and confirmed predecessor under the
    /// same complete predictive mediation contract without changing storage.
    pub fn learned_text_stream_finish_request(&self, after_request: u64, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        self.check_ready()?;
        self.inner.learned_text_stream_finish_request(after_request, request)
    }

    /// Expose only the original historical actor observation after independent
    /// validation and the original canonical-cut check. No new forecast, clock,
    /// cleanup, fence or role escapes; pending liabilities remain unresolved.
    pub fn finish_receipt(self, request: u64, finish_after: Option<u64>)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    {
        self.check_ready()?;
        self.inner.finish_receipt(request, finish_after)
    }

    /// Reconcile the same fully validated numerical history through the
    /// original single-image Fence/Time/Sweep. Guard and selection failures
    /// precede clock and mutation; only the selected actor observation escapes.
    pub fn finish_reconciled_receipt<F>(self, request: u64, finish_after: Option<u64>, clock: F)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_reconciled_receipt(request, finish_after, clock)
    }

    /// Select the original stop predecessor after the complete predictor,
    /// topology, guard, policy and independent-floor requirements pass.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Withdraw admission through the original Stop/Fence without acquiring
    /// settlement time. Complete independent requirements, the retained reserve
    /// and canonical custody still apply; no live role or drain receipt escapes.
    pub fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Drain only the matching acknowledged stop through the original terminal
    /// event. A failed clock preserves that stop and its charged liabilities.
    /// An already-drained retry does not acquire time or append another event.
    pub fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        self.inner.finish_stopped_drain(request, clock)
    }

    /// Retain the same fully guarded combined terminal transaction while
    /// allowing the caller to refuse unavailable or late settlement time.
    /// Refusal returns neither a candidate receipt nor a history anchor.
    pub fn try_finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        self.inner.try_finish_stopped_with_anchor(request, clock)
    }

    /// Consume the same original replay directly into native stopped recovery.
    /// No ordinary fence, new forecast or observer precedes the original
    /// Stop/Fence/StopProgress transaction; all spent and unknown work survives.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped(request, clock)
    }

    /// Return that acknowledged stopped cut and its original private history
    /// anchor, preserving every predictive mediation requirement and the one
    /// canonical lock. Capturing it issues no role, replay or extra fence.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped_with_anchor(request, clock)
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

