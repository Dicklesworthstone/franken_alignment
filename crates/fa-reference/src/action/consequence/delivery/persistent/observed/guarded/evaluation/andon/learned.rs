//! Complete independent accounting requirements over the original learned replay.
//! One lock, one numerical reconstruction and one acknowledged finalization.
use super::super::super::{
    FileOversight, FileOversightProfile, FileOversightRoles, JournalError,
    anchored::FileHistoryAnchor, learned::check_profile, learned_recovery::check_recovery_reserve,
};
use super::super::FileEvaluatedOversightRoles;
use super::FileAndonRecoveryRequirements;
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{
    StopRequest,
    persistent::{
        MAX_JOURNAL_EVENTS, RecoveryReserve,
        observed::{
            decoder::learned::{
                FileLearnedConfig, FileLearnedRecovery, FileLearnedRecoveryProgress,
                FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent,
            },
            shutdown::recovery::{
                FileAdmissionStoppedHistory, FileStoppedHistory, FileStoppedRecovery,
            },
        },
    },
    publication_gate::PublicationLimits,
};
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use std::{fmt, path::Path};

/// A sealed original learned recovery with independently retained evaluation,
/// Andon and guard requirements. Its exact history anchor is mandatory; neither
/// matching counters nor an accounting summary can replace that original prefix.
///
/// The same learned cursor retains its exclusive store, canonical bytes and
/// numerical witness checks. No candidate, old key, mutable owner or evaluator
/// escapes before full verification and the sole native acknowledgment. Dropping
/// the cursor, including at Ready, performs no staging cleanup or journal write.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::learned::FileAndonLearnedRecovery;
/// fn bypass(run: FileAndonLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::learned::FileAndonLearnedRecovery;
/// fn early_evaluator(run: &FileAndonLearnedRecovery) { run.evaluator(); }
/// ```
#[must_use = "finish verified recovery, or drop it without releasing live roles"]
pub struct FileAndonLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileAndonRecoveryRequirements,
    protocol: EvaluationProtocol,
    recipe_publication: Option<PublicationLimits>,
    failure: Option<Error>,
}

impl fmt::Debug for FileAndonLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileAndonLearnedRecovery")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Bind the complete learned recipe, exact retained history prefix, native
    /// evaluation protocol, accounting identity and ALL registered windows
    /// before the first numerical replay event. The original guard inventory,
    /// current policy, credential epoch, publication contract, optional joint
    /// policy and independent floors must also pass before Ready or any write.
    /// None in the requirements means required absence, never an on-disk choice.
    ///
    /// Required raw and owned predictors need their distinct role-preserving
    /// recovery composition. This evaluator cannot replace either observer.
    /// Initial framing/binding, one original event and final I/O remain
    /// synchronous; event quanta are not wall-time or preemption guarantees.
    pub fn begin_open_andon_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements,
        protocol: &EvaluationProtocol,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<FileAndonLearnedRecovery, JournalError> {
        check_profile(&expected.base().guards)?;
        if config.required_pre_output_forecast().is_some()
            || config.required_owned_pre_output_forecast().is_some()
        {
            return Err(Error::Binding.into());
        }
        let inner = Self::begin_open_with_learned_generation(directory, profile, config)?;
        let (profile, events, identity) = inner.guarded_history();
        anchor.check(profile, identity, events)?;
        // The complete independent recipe is already bound by the original
        // cursor. Its internal witness activation is not a top-level journal
        // event, but still requires an exact independent publication selection.
        let recipe_publication = config
            .required_publication_validation()
            .map(|profile| profile.limits);
        expected.preflight_evaluated_with_publication(events, protocol, recipe_publication)?;
        Ok(FileAndonLearnedRecovery {
            inner,
            expected: expected.clone(),
            protocol: protocol.clone(),
            recipe_publication,
            failure: None,
        })
    }

    /// Synchronous completion of the SAME cooperative cursor and sole recovery
    /// Fence. Use begin/advance for Stop/deadline arbitration between events;
    /// neither form repeats the historical numerical work during finalization.
    pub fn open_andon_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements,
        protocol: &EvaluationProtocol,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        let mut run = Self::begin_open_andon_guarded_anchored_with_learned_generation(
            directory, profile, expected, protocol, config, anchor,
        )?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, MAX_JOURNAL_EVENTS)?;
        }
        run.finish()
    }
}

impl FileAndonLearnedRecovery {
    /// Pin the exact original terminal reserve before the first replay event.
    /// Recovery cannot install, enlarge or refill that reserve. The native
    /// framing and final encoder retain their original event/byte capacity law.
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

    /// Apply at most max_events ORIGINAL transitions at the exact predecessor.
    /// Full accounting, evaluation and guard mismatches latch failure instead of
    /// exposing the inner cursor's otherwise valid Ready state. Native numerical
    /// failures, interrupted candidates and cumulative work remain unchanged.
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
            if let Err(error) = self.expected.check_replayed_with_publication(
                profile,
                self.inner.verified_guarded_machine()?,
                events,
                &self.protocol,
                self.recipe_publication,
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
        let machine = self.inner.verified_guarded_machine()?;
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.check_replayed_with_publication(
            profile,
            machine,
            events,
            &self.protocol,
            self.recipe_publication,
        )?;
        Ok(())
    }

    /// Publish the original recovery Fence, then issue fresh separate human,
    /// identity/governance and evaluator custody. Numerical state stays paused;
    /// old roles, approval keys, clock and source eligibility stay withdrawn.
    /// Pending/censored labels, costs, incidents and unresolved effects survive.
    /// A failed or uncertain replacement returns no owner or role bundle.
    pub fn finish(self) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Complete only the independently matched original pending reset. Check
    /// ALL accounting and guard requirements before and after native preparation,
    /// then acknowledge completion and Fence in the same original replacement.
    /// Native refusals, interrupted work and exact retry semantics are unchanged.
    pub fn finish_pending_reset(
        mut self,
        intent: &FileLearnedResetIntent,
    ) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }

    /// Retain the original stream reset's exact kind and incident declaration.
    /// The same complete independent contract is checked before and after its
    /// private native transition; an old reset cannot be rebound to a new epoch.
    pub fn finish_pending_stream_reset(
        mut self,
        intent: &FileLearnedResetIntent,
        first_incident_sequence: u64,
    ) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        self.check_ready()?;
        self.inner = self
            .inner
            .prepare_pending_stream_reset(intent, first_incident_sequence)?;
        self.finish()
    }

    /// Return only the native terminal predecessor after complete verification.
    /// This performs no cleanup, Fence, clock observation or role provisioning.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Use the original clock-free Stop/Fence directly, without an ordinary
    /// recovery Fence or live evaluator first. Unknown liabilities stay charged.
    pub fn finish_admission_stop(
        self,
        request: StopRequest,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Drain only the same already acknowledged stop. A failed clock preserves
    /// that stop; an exact completed retry requires no clock or additional event.
    pub fn finish_stopped_drain<F>(
        self,
        request: StopRequest,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> Result<ElapsedTick, JournalError>,
    {
        self.check_ready()?;
        self.inner.finish_stopped_drain(request, clock)
    }

    /// The fallible-clock form retains the original combined terminal write.
    /// Contract and canonical-custody failures precede clock acquisition; a late
    /// or unavailable clock cannot return a candidate receipt or history anchor.
    pub fn try_finish_stopped_with_anchor<F>(
        self,
        request: StopRequest,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> Result<ElapsedTick, JournalError>,
    {
        self.check_ready()?;
        self.inner.try_finish_stopped_with_anchor(request, clock)
    }

    /// Apply the native Stop/Fence/drain as one acknowledged transaction. No
    /// numerical work repeats, no live roles escape, and exact drained retries
    /// preserve the original no-clock/no-write semantics even at capacity.
    pub fn finish_stopped<F>(
        self,
        request: StopRequest,
        clock: F,
    ) -> Result<FileStoppedRecovery, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        self.check_ready()?;
        self.inner.finish_stopped(request, clock)
    }

    /// Preserve the exact acknowledged terminal cut for independent retention
    /// without issuing a mutable owner, evaluator, human or publication role.
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

    /// Carry the same fully checked numerical/accounting history into the
    /// authenticated nonce-bound Stop-only exchange. The original terminal
    /// reserve is mandatory and neither installed nor renewed by this handoff.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(
        self,
        operation: u64,
    ) -> Result<
        crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop,
        JournalError,
    >{
        self.check_ready()?;
        self.inner.into_recovery_stop(operation)
    }
}
