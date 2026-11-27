//! Original learned evaluation, accounting and topology custody in one owner.
//! Independent contracts stay complete across the sole numerical recovery.
use super::{
    Error, EvaluationProtocol, FileAndonRecoveryRequirements, FileHistoryAnchor, FileLearnedConfig,
    FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, FileOversight,
    FileOversightProfile, FileOversightRoles, FileStreamProposal, JournalError, PublicationLimits,
    RecoveryReserve, StopRequest, check_profile, check_recovery_reserve,
};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::{
    Event,
    consistency::ConsistencyEvent,
    credibility::{CredibilityEvent, andon::FileAndonConfig},
    guarded::{
        FileCredentialRegistration, FileGuardSet,
        learned::prepare_reserved,
        mediated::{FileMediatedRoles, FileTopologyRequirement},
    },
    shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory},
};
use crate::action::consequence::{
    mediation::AuthorityGraph, oversight::joint_credibility::JointPromotionPolicy,
};
use std::{fmt, path::Path};

/// One sealed learned recovery with exact accounting, optional joint policy,
/// publication contract, original/current topology and availability requirements.
/// A retained history anchor is mandatory. The same native cursor reconstructs
/// all numerical witnesses once; every independent check precedes Ready, a
/// recovery Fence or the release of either evaluator or topology custody.
///
/// Source checkpoints remain independently retained input to the caller's native
/// topology-source recovery. Matching this graph does not certify a fresh source,
/// restore an old cut, or grant effect authority.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::learned::FileMediatedAndonLearnedRecovery;
/// fn bypass(run: FileMediatedAndonLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::learned::FileMediatedAndonLearnedRecovery;
/// fn premature(run: &FileMediatedAndonLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "finish complete original recovery, or drop it without returning live roles"]
pub struct FileMediatedAndonLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileAndonRecoveryRequirements,
    topology: FileTopologyRequirement,
    protocol: EvaluationProtocol,
    recipe_publication: Option<PublicationLimits>,
    failure: Option<Error>,
}

impl fmt::Debug for FileMediatedAndonLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMediatedAndonLearnedRecovery")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Install the original learned recipe, marginal evaluator, Andon ledger,
    /// guarded perimeter, uncertified topology and terminal reserve in ONE first
    /// acknowledged image. Native configuration and event capacity are checked
    /// before storage exists. No initial cut, label or human effect key is made.
    pub fn create_mediated_andon_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph,
        protocol: EvaluationProtocol,
        accounting: &FileAndonConfig,
        config: FileLearnedConfig,
        reserve: RecoveryReserve,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        create(
            directory.as_ref(),
            profile,
            guards,
            registration,
            topology,
            protocol,
            None,
            accounting,
            config,
            reserve,
        )
    }

    /// The same atomic startup with an independently selected joint promotion
    /// policy. Its replay ceilings remain part of exact recovery requirements.
    /// Labels, topology observations and human approvals retain separate custody.
    /// Held-out activation and raw/owned prediction use their distinct lanes.
    pub fn create_joint_mediated_andon_guarded_with_learned_generation_reserved(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        topology: AuthorityGraph,
        protocol: EvaluationProtocol,
        joint: JointPromotionPolicy,
        accounting: &FileAndonConfig,
        config: FileLearnedConfig,
        reserve: RecoveryReserve,
    ) -> Result<(Self, FileMediatedRoles), JournalError> {
        create(
            directory.as_ref(),
            profile,
            guards,
            registration,
            topology,
            protocol,
            Some(joint),
            accounting,
            config,
            reserve,
        )
    }

    /// Bind the original canonical history, complete recipe and independently
    /// selected accounting, optional joint/publication policies and topology
    /// before event zero. Wrong original/current graphs, evaluator activations,
    /// windows or publication bindings refuse without numerical work or writes.
    /// Final availability, floors, effective policy and every original guard
    /// are checked before Ready. No caller-supplied summary seeds the machine.
    pub fn begin_open_mediated_andon_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements,
        topology: &FileTopologyRequirement,
        protocol: &EvaluationProtocol,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<FileMediatedAndonLearnedRecovery, JournalError> {
        check_profile(&expected.base().guards)?;
        check_recipe(config)?;
        let inner = Self::begin_open_with_learned_generation(directory, profile, config)?;
        let (profile, events, identity) = inner.guarded_history();
        anchor.check(profile, identity, events)?;
        // Recipe binding does not hydrate a separately enabled learned
        // predictor. Its supplied-source owner and held-out qualification have
        // distinct custody; refuse those activations before event zero.
        if events.iter().any(|event| {
            matches!(
                event,
                Event::Consistency(ConsistencyEvent::EnableLearned(_))
                    | Event::Credibility(CredibilityEvent::EnableHeldOutJoint(_))
            )
        }) {
            return Err(Error::Binding.into());
        }
        let recipe_publication = config
            .required_publication_validation()
            .map(|profile| profile.limits);
        expected.preflight_evaluated_with_topology(
            events,
            protocol,
            recipe_publication,
            Some(topology),
        )?;
        Ok(FileMediatedAndonLearnedRecovery {
            inner,
            expected: expected.clone(),
            topology: topology.clone(),
            protocol: protocol.clone(),
            recipe_publication,
            failure: None,
        })
    }
}

fn check_recipe(config: &FileLearnedConfig) -> Result<(), Error> {
    if config.required_held_out_joint().is_some()
        || config.required_pre_output_forecast().is_some()
        || config.required_owned_pre_output_forecast().is_some()
    {
        return Err(Error::Binding);
    }
    Ok(())
}

fn create(
    directory: &Path,
    profile: FileOversightProfile,
    guards: &FileGuardSet,
    registration: Option<FileCredentialRegistration<'_>>,
    topology: AuthorityGraph,
    protocol: EvaluationProtocol,
    joint: Option<JointPromotionPolicy>,
    accounting: &FileAndonConfig,
    config: FileLearnedConfig,
    reserve: RecoveryReserve,
) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
    check_recipe(&config)?;
    let prepared = prepare_reserved(profile, guards, registration, config, reserve)?;
    let prepared = match joint {
        Some(joint) => prepared.jointly_evaluated(protocol.clone(), joint)?,
        None => prepared.evaluated(protocol.clone())?,
    };
    let prepared = prepared
        .with_andon(accounting)?
        .mediated(topology.clone())?
        .checked_mediated(guards, Some(&protocol), &topology)?
        .checked_event_capacity()?;
    let (host, oversight) = prepared.create(directory)?;
    let roles = FileMediatedRoles::provision(&host, oversight, false, true);
    Ok((host, roles))
}

impl FileMediatedAndonLearnedRecovery {
    /// Require the exact existing reserve before replay. Nothing is installed,
    /// enlarged or refilled; the original encoder owns final capacity admission.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        check_recovery_reserve(&self.inner, expected)?;
        Ok(self)
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure {
            progress.status = FileLearnedRecoveryStatus::Failed(error);
        }
        progress
    }

    /// Replay at most max_events original transitions at the exact predecessor.
    /// A final contract mismatch latches failure rather than exposing the inner
    /// cursor's Ready state. Event counts are not wall-time or I/O guarantees.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileLearnedRecoveryProgress, JournalError> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.check_replayed_with_topology(
                profile,
                self.inner.verified_guarded_machine()?,
                events,
                &self.protocol,
                self.recipe_publication,
                Some(&self.topology),
            ) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.check_replayed_with_topology(
            profile,
            self.inner.verified_guarded_machine()?,
            events,
            &self.protocol,
            self.recipe_publication,
            Some(&self.topology),
        )?;
        Ok(())
    }

    /// Select only the original Message after complete contract verification,
    /// without a Fence, source refresh or newly issued role.
    pub fn learned_text_stream_message_request(
        &self,
        request: u64,
    ) -> Result<FileStreamProposal, JournalError> {
        self.check_ready()?;
        self.inner.learned_text_stream_message_request(request)
    }

    /// Select the original Finish and exact accepted predecessor from this
    /// fully checked numerical history before any live recovery mutation.
    pub fn learned_text_stream_finish_request(
        &self,
        after_request: u64,
        request: u64,
    ) -> Result<FileStreamProposal, JournalError> {
        self.check_ready()?;
        self.inner
            .learned_text_stream_finish_request(after_request, request)
    }

    /// Acknowledge the sole original recovery Fence, then return fresh separate
    /// human, evaluator and topology custody. Numerical work stays paused; old
    /// approvals, identity, clock and topology availability stay withdrawn.
    /// Labels, accounting costs, incidents and unknown liabilities survive.
    pub fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileMediatedRoles::provision(&host, oversight, false, true);
        Ok((host, roles))
    }

    /// Obtain the native terminal predecessor only after all independent
    /// contracts pass. No live observer, evaluator or cleanup is produced.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Apply the original clock-free Stop/Fence without first recovering a live
    /// owner. The same exact numerical history and unresolved charges survive.
    pub fn finish_admission_stop(
        self,
        request: StopRequest,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Preserve the acknowledged terminal cut using the original combined
    /// Stop/Fence/drain. Guard failures precede clock access and all mutation.
    pub fn finish_stopped_with_anchor<F>(
        self,
        request: StopRequest,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        self.check_ready()?;
        self.inner.finish_stopped_with_anchor(request, clock)
    }

    /// Move the same complete verified owner into authenticated Stop-only
    /// custody. It cannot return a live topology observer or evaluator.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(self, operation: u64)
        -> Result<crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError>
{
        self.check_ready()?;
        self.inner.into_recovery_stop(operation)
    }
}
