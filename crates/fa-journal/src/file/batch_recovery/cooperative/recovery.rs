//! Recover an original interrupted group without first reopening a writable owner.
//! The original prefix decoder, group encoder/reducer and suffix comparer are shared.
use super::{
    BatchAnchor, BatchInspectionStatus, BatchInspectionWork, BatchLimits, Error, Event,
    FileBatchInspection, FileJournal, HistoryAnchor, InspectionOrigin, Journal, admit_batch,
};
use crate::file::{lock, parent_directory};
use crate::{Identity, Limits, RecoveryReserve};
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::Path;

impl FileJournal {
    /// Open and exclusively lock an EXISTING v1 journal for exact interrupted-
    /// group recovery. Supply the independently retained pre-group history and
    /// original ordered events, never a guessed resume offset or fresh group.
    /// Validate all context/admission before opening; read no payload here.
    ///
    /// Unlike open_anchored followed by begin_resume_batch_anchored, this replays
    /// the pre-group prefix ONCE, prepares the complete original group, and
    /// compares its surviving suffix once. Only an exact original record boundary
    /// may describe the stored extent. Torn records, forks and unrelated tails
    /// refuse without repair, truncation, appending, or a usable partial owner.
    ///
    /// Drive with BatchInspectionBudget. Ready proves byte/reducer checks, NOT
    /// synchronization. finish() synchronizes the stored file and directory and
    /// returns only its current accounting/status. resume() performs those SAME
    /// barriers before transferring only the missing bytes to FileBatchCommit.
    /// Even a complete group on disk cannot skip recovery synchronization.
    /// The original writer then syncs any newly appended bytes before acknowledgment.
    ///
    /// No external effect is dispatched, inferred, retried or refunded. Read-only
    /// accounting and unknown obligations retain the original reducer semantics.
    /// Cancellation releases the lock without writing; all work remains reported.
    /// An independent anchor's import work is separate. No host-time or hostile-
    /// writer guarantee is added to the trusted-directory/advisory-lock profile.
    pub fn begin_recover_batch_anchored(
        path: impl AsRef<Path>,
        identity: Identity,
        history: HistoryAnchor,
        limits: Limits,
        events: Vec<Event>,
        batch_limits: BatchLimits,
    ) -> Result<FileBatchInspection, Error> {
        begin(path.as_ref(), history, Journal::new(identity, limits)?, events, batch_limits)
    }

    /// Explicit independently selected v2 profile. The complete reserve and
    /// lifetime limits must match the retained history and original header.
    /// Recovery cannot widen the tail, replenish capacity or silently use v1.
    pub fn begin_recover_batch_anchored_with_recovery_reserve(
        path: impl AsRef<Path>,
        identity: Identity,
        history: HistoryAnchor,
        limits: Limits,
        reserve: RecoveryReserve,
        events: Vec<Event>,
        batch_limits: BatchLimits,
    ) -> Result<FileBatchInspection, Error> {
        begin(path.as_ref(), history,
            Journal::new_with_recovery_reserve(identity, limits, reserve)?, events, batch_limits)
    }
}

fn begin(path: &Path, history: HistoryAnchor, initial: Journal, events: Vec<Event>,
    limits: BatchLimits) -> Result<FileBatchInspection, Error>
{
    history.check_context(&initial)?;
    admit_batch(&events, limits)?;
    let anchor = BatchAnchor {
        identity: initial.identity(), head: history.head(), encoded_len: history.encoded_len(),
    };
    let directory = parent_directory(path)?;
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    lock(&file)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular journal file").into());
    }
    let extent = metadata.len();
    if extent > initial.limits.max_total_bytes { return Err(crate::Error::Limit.into()); }
    if extent < anchor.encoded_len { return Err(crate::Error::BehindAnchor.into()); }
    let replay = initial.replay_cursor_anchored(file.take(anchor.encoded_len), history)?;
    Ok(FileBatchInspection {
        replay: Some(replay), file: None,
        origin: InspectionOrigin::Recovery { minimum: anchor.head, extent, directory, matched: None },
        candidate: None, retain_candidate: true, anchor, events, limits,
        covered: 0, prepared: 0, records: Vec::new(), compared: 0,
        stage: BatchInspectionStatus::ReplayPrefix, failed: false,
        total: BatchInspectionWork::default(),
    })
}

#[cfg(test)]
mod tests;
