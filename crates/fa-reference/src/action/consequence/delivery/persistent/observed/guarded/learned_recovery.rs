//! Cooperative learned recovery with the original complete guard/role contract.
//! The inner candidate, original reset and sole persistence boundary stay sealed.
use super::{FileOversight, FileOversightProfile, FileOversightRoles, FileRecoveryRequirements,
    JournalError, anchored::FileHistoryAnchor, learned::check_profile};
use super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use super::{BaseEvent, Event};
use std::fmt;
use std::path::Path;

/// Holds the SAME original exclusive recovery, not a second writer or replay
/// engine. Guard requirements are frozen independently before event execution.
/// No partial role bundle, base-owner extraction or mutable machine is exposed.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::learned_recovery::FileGuardedLearnedRecovery;
/// fn bypass(run: FileGuardedLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::learned_recovery::FileGuardedLearnedRecovery;
/// fn early_role(run: &FileGuardedLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "advance and finish guarded recovery, or drop it without changing storage"]
pub struct FileGuardedLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileRecoveryRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FileGuardedLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGuardedLearnedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Match the original decoder inventory and exact learned recipe before any
    /// numerical replay. Completion additionally checks the full original guard,
    /// effective-policy, credential-epoch and independent recovery-floor contract.
    pub fn begin_open_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements, config: &FileLearnedConfig)
        -> Result<FileGuardedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Check an independently retained exact prefix on the SAME canonical bytes
    /// and exclusive lock before numerical replay. Genuine append-only successors
    /// are accepted by the original anchor checker; divergent/truncated cuts are
    /// not. This does not authenticate observations or advance the held anchor.
    pub fn begin_open_guarded_anchored_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FileGuardedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FileRecoveryRequirements,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileGuardedLearnedRecovery, JournalError>
{
    check_profile(&expected.guards)?;
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    expected.guards.check_decoder_config(events)?;
    Ok(FileGuardedLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

impl FileGuardedLearnedRecovery {
    /// Require exactly the independently selected original logical reserve
    /// from the SAME locked canonical history, before the first replay step.
    /// Missing or different reserves return no owner and perform no cleanup or
    /// fence. No reserve is installed or enlarged during recovery. The original
    /// decoder already verifies its unique pre-work position and both limits;
    /// finish still verifies the retained canonical cut before writing.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        let progress = self.inner.progress();
        if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        let (_, events, _) = self.inner.guarded_history();
        let mut reserves = events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(*actual),
            _ => None,
        });
        if reserves.next() != Some(expected) || reserves.next().is_some() {
            return Err(Error::Binding.into());
        }
        Ok(self)
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// Execute at most the requested count of ORIGINAL journal transitions.
    /// A guard mismatch is retained as terminal, never reported as Ready or
    /// recoverable by changing requirements. Costs and numerical failures stay
    /// those of the original replay. Event quanta are not wall-clock bounds.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.check(profile, self.inner.verified_guarded_machine()?, events) {
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
        self.expected.check(profile, machine, events)?;
        Ok(())
    }

    /// Publish the original recovery fence, then provision ALL original roles.
    /// No numerical history is replayed twice. Saved identity/clock eligibility
    /// and old keys stay withdrawn; a pending reset is quarantined as before.
    pub fn finish(self) -> Result<(FileOversight, FileOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }

    /// Check the complete guard contract before AND after executing the exact
    /// independently selected original pending reset. Only the existing atomic
    /// completion-plus-fence replacement exposes owner/roles. Completed matching
    /// commands do not reset again; interrupted or changed commands still refuse.
    /// Original native refusals remain acknowledged results, not permitting keys.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileOversightRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }

    #[cfg(test)]
    pub(in super::super) fn fail_once(&self, operation: super::super::super::JournalIo) {
        self.inner.fail_once(operation);
    }
}
