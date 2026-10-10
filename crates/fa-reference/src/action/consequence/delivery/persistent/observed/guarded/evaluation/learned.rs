//! Recover evaluated learned generation without losing independent evaluator custody.
//! Original numerical replay, reset and the sole completion/fence write stay sealed.
mod bootstrap;
use super::{EvaluationProtocol, FileEvaluatedOversightRoles, FileOversightRoles,
    FileRecoveryRequirements, check_protocol};
use super::super::{anchored::FileHistoryAnchor, learned::check_profile};
use super::super::super::{Event, FileOversight, FileOversightProfile, JournalError};
use super::super::super::credibility::CredibilityEvent;
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use std::fmt;
use std::path::Path;

/// One original exclusive learned recovery with a separately retained evaluation
/// protocol and complete guard contract. No candidate, old key or partial role
/// bundle is available until the original canonical replacement acknowledges.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::learned::FileEvaluatedLearnedRecovery;
/// fn bypass(run: FileEvaluatedLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::learned::FileEvaluatedLearnedRecovery;
/// fn early_role(run: &FileEvaluatedLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "finish evaluated recovery, or drop it without changing storage"]
pub struct FileEvaluatedLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileRecoveryRequirements,
    protocol: EvaluationProtocol,
    failure: Option<Error>,
}

impl fmt::Debug for FileEvaluatedLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileEvaluatedLearnedRecovery")
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Bind the exact numerical recipe, evaluation protocol and numerical guard
    /// inventory before replay. The remaining guards, effective policy, credential
    /// epoch and independent floors must match before Ready or any recovery write.
    pub fn begin_open_evaluated_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol, config: &FileLearnedConfig)
        -> Result<FileEvaluatedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, protocol, config, None)
    }

    /// Additionally compare the independently retained original exact prefix on
    /// the SAME canonical bytes and exclusive lock before numerical replay.
    /// Counter floors alone cannot reject an equal-counter history fork.
    pub fn begin_open_evaluated_guarded_anchored_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol, config: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FileEvaluatedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, protocol, config, Some(anchor))
    }

    /// Synchronous consumer of the SAME cooperative recovery. It executes one
    /// original recovery fence and returns the original separately held roles;
    /// it neither repeats numerical replay nor renews evidence or old approvals.
    pub fn open_evaluated_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        protocol: &EvaluationProtocol, config: &FileLearnedConfig)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let mut recovery = Self::begin_open_evaluated_guarded_with_learned_generation(
            directory, profile, expected, protocol, config)?;
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = recovery.progress().replayed_events;
            recovery.advance(completed, 1)?;
        }
        recovery.finish()
    }
}

fn begin(directory: &Path, profile: FileOversightProfile,
    expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileEvaluatedLearnedRecovery, JournalError>
{
    check_profile(&expected.guards)?;
    // These pins install additional observer roles inside learned Enable. An
    // evaluated-only recovery must not discard them or replay their numerics
    // under an incomplete independently supplied observer contract.
    if config.required_pre_output_forecast().is_some()
        || config.required_owned_pre_output_forecast().is_some()
    { return Err(Error::Binding.into()); }
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    expected.guards.check_decoder_config(events)?;
    check_protocol(events, protocol)?;
    // Joint qualification is a distinct independently pinned contract. Neither
    // a sole joint enable nor a malformed mixed history is this profile.
    if events.iter().any(|event| matches!(event,
        Event::Credibility(CredibilityEvent::EnableJoint(_, _))))
    { return Err(Error::Binding.into()); }
    Ok(FileEvaluatedLearnedRecovery { inner, expected: expected.clone(),
        protocol: protocol.clone(), failure: None })
}

impl FileEvaluatedLearnedRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// Apply a bounded count of ORIGINAL journal events. Numerical work and
    /// witnesses retain their original limits. A final guard mismatch is sticky;
    /// changing the caller's requirements cannot repair this private candidate.
    /// One event, final encoding and filesystem I/O remain synchronous.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.check_evaluated(profile,
                self.inner.verified_guarded_machine()?, events, Some(&self.protocol)) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let machine = self.inner.verified_guarded_machine()?;
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.check_evaluated(profile, machine, events, Some(&self.protocol))?;
        Ok(())
    }

    /// Recheck all requirements, publish the original fence, THEN provision the
    /// human/identity/governance roles and independent evaluator. Retained labels,
    /// pending/censored denominators and lifetime false-stop costs do not reset.
    /// Old evaluator tickets and effect keys do not belong to the returned owner.
    pub fn finish(self) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Complete only an independently matched, already recorded original reset.
    /// Check the full evaluator/guard contract BEFORE and AFTER the SAME native
    /// reset, then acknowledge completion and fencing in one canonical image.
    /// Original reset refusals stay refusals; pending interrupted work cannot be
    /// rebound to a newer epoch. An exact completed retry performs no new reset.
    /// The owner remains paused and unknown effect charges remain outstanding.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }
}

