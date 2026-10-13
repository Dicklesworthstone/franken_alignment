//! Native evaluation/Andon recovery through the ORIGINAL cooperative engine.
//! No alternative reducer, duplicate historical replay, or early evaluator role.
use super::{FileGuardedRecovery, FileGuardedRecoveryProgress, FileGuardedRecoveryStatus,
    FileOversight, FileOversightProfile, JournalError, Machine, RecoveryReserve,
    FileHistoryAnchor, MAX_JOURNAL_EVENTS, storage};
use super::super::evaluation::andon::FileAndonRecoveryRequirements;
use super::super::evaluation::FileEvaluatedOversightRoles;
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use std::fmt;
use std::path::Path;

/// Owns one locked canonical cut, the original private replay cursor and the
/// complete independent requirements. No raw-cursor or partially replayed owner
/// can escape. Dropping/cancelling never cleans staging or changes the journal.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::FileAndonRecovery;
/// fn bypass(cursor: FileAndonRecovery) { cursor.into_inner(); }
/// ```
#[must_use = "finish verified recovery or cancel/drop it without issuing roles"]
pub struct FileAndonRecovery {
    inner: FileGuardedRecovery,
    expected: FileAndonRecoveryRequirements,
    protocol: EvaluationProtocol,
}
impl fmt::Debug for FileAndonRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileAndonRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Retain the exclusive Store while validating the exact anchor, native
    /// protocol, full window inventory and original decoder/guard configuration.
    /// No historical event executes until advance; only complete verified replay
    /// can enter the single original fence and release fresh, separate roles.
    pub fn begin_open_andon_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol,
        anchor: &FileHistoryAnchor) -> Result<FileAndonRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileAndonRecovery::begin_store(store, profile, expected, protocol, anchor)
    }
}

impl FileAndonRecovery {
    pub(in super::super) fn begin_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements, protocol: &EvaluationProtocol,
        anchor: &FileHistoryAnchor) -> Result<Self, JournalError>
    {
        let inner = FileGuardedRecovery::begin_store(store, profile, expected.base(), Some(anchor))?;
        expected.preflight_evaluated(&inner.events, protocol)?;
        Ok(Self { inner, expected: expected.clone(), protocol: protocol.clone() })
    }

    pub fn progress(&self) -> FileGuardedRecoveryProgress { self.inner.progress() }

    /// Pin the ORIGINAL reserve before any event attempt; do not enlarge it or
    /// consume terminal capacity on behalf of ordinary recovery.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        Ok(Self { inner: self.inner.require_recovery_reserve(expected)?, ..self })
    }

    /// At most max_events original applications, with exact predecessor checks
    /// and the existing failure/unwind latch. Logical event budgets are NOT
    /// wall-time deadlines. A single event, decoding or final sync can be slow.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileGuardedRecoveryProgress, JournalError>
    {
        let expected = &self.expected;
        let protocol = &self.protocol;
        self.inner.advance_checked_with(expected_events, max_events, Machine::apply,
            |run| expected.check_replayed(&run.profile, &run.machine, &run.events, protocol))
    }

    pub fn cancel(self) -> FileGuardedRecoveryProgress { self.inner.cancel() }

    /// Fence the ALREADY VERIFIED native machine exactly once through the same
    /// encoding/capacity/canonical-comparison/persistence path as base recovery.
    /// Pending labels and receipts survive; old keys and clock eligibility do not.
    /// No second historical replay occurs and no role escapes an uncertain write.
    pub fn finish(self) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        let Self { inner, expected, protocol } = self;
        let (host, oversight) = inner.finish_checked(
            |run| expected.check_replayed(&run.profile, &run.machine, &run.events, &protocol))?;
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    pub(in super::super) fn finish_synchronously(mut self)
        -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError>
    {
        while self.progress().status == FileGuardedRecoveryStatus::Replaying {
            self.advance(self.progress().replayed_events, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }
}

#[cfg(test)]
mod tests;
