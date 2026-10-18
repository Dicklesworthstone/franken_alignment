//! Cooperative mediation recovery over the SAME guarded cursor and native fence.
//! Neither a partially replayed machine nor the weaker base cursor is exported.
use super::{FileGuardedRecovery, FileGuardedRecoveryProgress, FileGuardedRecoveryStatus,
    FileHistoryAnchor, FileOversight, FileOversightProfile, JournalError, Machine,
    RecoveryReserve, MAX_JOURNAL_EVENTS, journal, storage};
use super::super::mediated::{FileMediatedRequirements, FileMediatedRoles,
    check_history, check_machine};
use super::FilePublicationRecoveryRequirements;
use std::fmt;
use std::path::Path;

mod reconciled;
mod stopped;
pub use reconciled::ReconciledMediatedOwner;

/// One frozen canonical image and exclusive writer lock. A poll executes at most
/// its requested number of ORIGINAL transitions, then returns control. Reading,
/// decoding, history/anchor checks and final replacement remain synchronous. One
/// event may contain numerical work; this is not a wall-clock latency guarantee.
///
/// Exact initial/current topology, pre-fence availability, optional prediction
/// and evaluation, effective policy, credentials and external floors remain
/// mandatory. Ready is only private reconstruction, never fresh source evidence.
/// The original fence withdraws old coverage, eligibility and approvals. A newer
/// graph/inventory and fresh native certification are still needed afterward.
/// Learned generation keeps its distinct recipe-bound recovery entry points.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRecovery;
/// fn bypass(run: FileMediatedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRecovery;
/// fn premature(run: &FileMediatedRecovery) { run.roles(); }
/// ```
#[must_use = "finish mediated recovery or drop it without issuing an owner or roles"]
pub struct FileMediatedRecovery {
    inner: FileGuardedRecovery,
    expected: FileMediatedRequirements,
}

impl fmt::Debug for FileMediatedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMediatedRecovery").field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze and preflight the exact mediation profile before ANY replay.
    /// No cleanup, recovery fence or new role occurs at construction or Ready.
    pub fn begin_open_mediated_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements) -> Result<FileMediatedRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileMediatedRecovery::begin_store(store, profile, expected, None)
    }

    /// Check the independently retained exact prefix of the SAME locked image
    /// before replay, in addition to the complete original mediation requirements.
    /// An anchored prefix never excuses an invalid or mismatching suffix.
    pub fn begin_open_mediated_guarded_anchored(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileMediatedRequirements,
        anchor: &FileHistoryAnchor) -> Result<FileMediatedRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileMediatedRecovery::begin_store(store, profile, expected, Some(anchor))
    }
}

impl FileMediatedRecovery {
    pub(in super::super) fn begin_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, anchor: Option<&FileHistoryAnchor>)
        -> Result<Self, JournalError>
    {
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &canonical)?;
        if let Some(anchor) = anchor { anchor.check(&profile, store.identity(), &events)?; }
        check_history(&events, expected)?;
        let machine = Machine::new(&profile)?;
        // This cursor is never returned to callers. Its base requirements alone
        // deliberately reject mediation; every public path below uses our full
        // native checker instead of downgrading to that base profile.
        let inner = FileGuardedRecovery { profile, expected: expected.oversight.clone(),
            store, canonical, events, machine, replayed: 0, attempted: 0,
            status: FileGuardedRecoveryStatus::Replaying };
        Ok(Self { inner, expected: expected.clone() })
    }

    /// Match an existing reserve before the first transition; never install or
    /// refill one. A late/mismatching selection consumes the cursor without writes.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        Ok(Self { inner: self.inner.require_recovery_reserve(expected)?, expected: self.expected })
    }

    /// Pin the independently selected publication bootstrap on the SAME locked
    /// canonical image before the first replay attempt. None fields require
    /// absence; no feed, freshness, fallback or joint gate is silently omitted.
    /// A mismatch or late call consumes recovery without a clock, fence, cleanup
    /// or role. The original checker owns uniqueness and complete field binding;
    /// matching publication settings never bypasses our mediation requirements.
    pub fn require_publication_profile(self, expected: FilePublicationRecoveryRequirements)
        -> Result<Self, JournalError>
    {
        Ok(Self { inner: self.inner.require_publication_profile(expected)?, expected: self.expected })
    }

    pub fn progress(&self) -> FileGuardedRecoveryProgress { self.inner.progress() }

    /// Stale/invalid polls do no work. The original cursor latches both original
    /// transition failures/unwinds AND final requirement-check failures/unwinds.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileGuardedRecoveryProgress, JournalError>
    {
        let expected = &self.expected;
        self.inner.advance_checked_with(expected_events, max_events, Machine::apply,
            |run| check_machine(&run.profile, &run.machine, &run.events, expected))
    }

    /// Release the original lock without cleaning staging or changing canonical
    /// bytes. Failed/interrupted status survives; cancellation grants no roles.
    pub fn cancel(self) -> FileGuardedRecoveryProgress { self.inner.cancel() }

    /// Recheck the complete profile and use the ORIGINAL one-fence publication.
    /// No second replay or live owner escapes before replacement acknowledgment.
    /// The separate topology, prediction and evaluator roles are provisioned only
    /// against that returned owner's new issuer. Pending effects are not resent.
    pub fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        let expected = self.expected;
        let (host, oversight) = self.inner.finish_checked(
            |run| check_machine(&run.profile, &run.machine, &run.events, &expected))?;
        let roles = FileMediatedRoles::provision(&host, oversight,
            expected.prediction.is_some(), expected.evaluation.is_some());
        Ok((host, roles))
    }

    pub(in super::super) fn finish_synchronously(mut self)
        -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        while self.progress().status == FileGuardedRecoveryStatus::Replaying {
            self.advance(self.progress().replayed_events, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }
}

#[cfg(test)]
mod tests;
