//! Exact-prefix recovery and evidence capture on the original locked descriptor.
use super::{Error, FileJournal, FileRecovery, READ_AHEAD};
use crate::replay::{HistoryAnchor, ReplayBudget, ReplayStatus};
use crate::{Identity, Journal, Limits, RecoveryReserve};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

impl FileJournal {
    /// Consume a healthy acknowledged writer and capture its exact history in
    /// original bounded replay polls. Retain its head independently first.
    /// The same file descriptor/lock and original configuration are retained;
    /// no usable owner escapes until full head, extent and rights parity.
    ///
    /// Finish with FileRecovery::finish_with_history_anchor. Cancellation/drop
    /// closes this writer without changing bytes or refunding any liability;
    /// recover under the independently retained minimum to resume work. A
    /// poisoned writer is refused. This is not a durable Stop acknowledgment.
    /// No payload read occurs until advance; opening metadata and seeking are
    /// synchronous. Capture allocation and replay obey the original lifetime
    /// limits, not an elapsed-time bound. The synchronous borrowing API remains
    /// available as history_anchor and as an independent comparison oracle.
    pub fn begin_history_anchor(mut self) -> Result<FileRecovery, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let initial = Journal::configured(
            self.journal.identity, self.journal.limits, self.journal.recovery_reserve,
        )?;
        self.poisoned = true;
        if self.file.metadata()?.len() != self.journal.encoded_len() {
            return Err(changed_extent().into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        let replay = initial.replay_cursor_capturing(
            BufReader::with_capacity(READ_AHEAD, self.file), self.journal.head(),
        )?;
        Ok(FileRecovery { replay, directory: None, acknowledged: Some(self.journal) })
    }

    /// Begin exclusive v1 recovery using independently retained exact bytes.
    /// Reject a different identity or limit profile before accessing the path.
    /// All original locking, read budgets, suffix replay and sync barriers apply.
    pub fn begin_open_anchored(
        path: impl AsRef<Path>,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
    ) -> Result<FileRecovery, Error> {
        let journal = Journal::new(identity, limits)?;
        Self::begin_open_checked(path, anchor.head(), journal, Some(anchor))
    }

    /// Begin v2 recovery with an independently selected recovery-tail reserve.
    /// Neither the reserve nor the lifetime limits are inferred from disk.
    pub fn begin_open_anchored_with_recovery_reserve(
        path: impl AsRef<Path>,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
        reserve: RecoveryReserve,
    ) -> Result<FileRecovery, Error> {
        let journal = Journal::new_with_recovery_reserve(identity, limits, reserve)?;
        Self::begin_open_checked(path, anchor.head(), journal, Some(anchor))
    }

    /// Finish strict anchored replay and synchronization before returning a
    /// writer. Never create, repair or truncate a file, or redispatch an effect.
    pub fn open_anchored(
        path: impl AsRef<Path>,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
    ) -> Result<Self, Error> {
        complete(Self::begin_open_anchored(path, identity, anchor, limits)?)
    }

    pub fn open_anchored_with_recovery_reserve(
        path: impl AsRef<Path>,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
        reserve: RecoveryReserve,
    ) -> Result<Self, Error> {
        complete(Self::begin_open_anchored_with_recovery_reserve(
            path, identity, anchor, limits, reserve,
        )?)
    }

    /// Capture evidence of this healthy owner's acknowledged history, using the
    /// SAME locked descriptor. The original decoder/reducer must reproduce its
    /// exact head, extent and rights. No file is written and no lock is released.
    /// Keep the returned bytes independently before relying on them at recovery.
    /// This does not persist that independent copy or authenticate its freshness.
    ///
    /// Capture is synchronous, bounded by the acknowledged encoded length and
    /// configured replay limits, NOT a cooperative poll or elapsed-time bound.
    /// It temporarily stores the full prefix and another reconstruction. It
    /// never exports an unacknowledged tail after a failed commit. I/O, content
    /// inconsistency or unwinding after I/O starts poisons the owner; only a
    /// complete success restores it. Allocation refusal before I/O does not.
    /// The original trusted-directory/advisory-lock assumptions still apply.
    pub fn history_anchor(&mut self) -> Result<HistoryAnchor, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let expected = self.journal.encoded_len();
        let size = usize::try_from(expected).map_err(|_| crate::Error::Limit)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|_| crate::Error::Limit)?;
        bytes.resize(size, 0);

        self.poisoned = true;
        if self.file.metadata()?.len() != expected {
            return Err(changed_extent().into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_exact(&mut bytes)?;
        if self.file.metadata()?.len() != expected
            || self.file.seek(SeekFrom::End(0))? != expected
        {
            return Err(changed_extent().into());
        }
        let anchor = HistoryAnchor::from_acknowledged(bytes, &self.journal)?;
        self.poisoned = false;
        Ok(anchor)
    }
}

fn complete(mut recovery: FileRecovery) -> Result<FileJournal, Error> {
    while recovery.progress().status == ReplayStatus::Reading {
        recovery.advance(ReplayBudget::UNLIMITED)?;
    }
    recovery.finish()
}

fn changed_extent() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "journal extent changed during anchor capture")
}
