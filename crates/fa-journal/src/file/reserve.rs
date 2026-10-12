//! Persist the complete independently selected reserve in the original file owner.
use super::{Error, FileJournal, FileRecovery};
use crate::{Head, Identity, Journal, JournalCapacity, Limits, RecoveryReserve};
use crate::replay::{ReplayBudget, ReplayStatus};
use std::path::Path;

impl FileJournal {
    /// Create a new v2 journal whose FIRST durable header binds all lifetime
    /// limits and the recovery reserve. Invalid profiles do not create a file.
    /// Uses the original exclusive create, writer lock and file/directory sync.
    /// No physical disk reservation or external-effect authority is supplied.
    pub fn create_with_recovery_reserve(path: impl AsRef<Path>, identity: Identity,
        limits: Limits, reserve: RecoveryReserve) -> Result<Self, Error>
    {
        Self::create_journal(path, Journal::new_with_recovery_reserve(identity, limits, reserve)?)
    }

    /// Exact independently pinned recovery profile, not a hint read from disk.
    /// No legacy fallback, wider allowance, new epoch, repair or silent migration.
    /// Return a locked private reconstruction; only finish exposes the writer.
    pub fn begin_open_with_recovery_reserve(path: impl AsRef<Path>, identity: Identity,
        minimum: Head, limits: Limits, reserve: RecoveryReserve) -> Result<FileRecovery, Error>
    {
        Self::begin_open_journal(path, minimum,
            Journal::new_with_recovery_reserve(identity, limits, reserve)?)
    }

    /// Drive the same cooperative recovery through the original synchronization
    /// barriers. Complete unacknowledged suffixes consume space and retain their
    /// original liabilities; recovery does not reset either capacity dimension.
    pub fn open_with_recovery_reserve(path: impl AsRef<Path>, identity: Identity,
        minimum: Head, limits: Limits, reserve: RecoveryReserve) -> Result<Self, Error>
    {
        let mut recovery = Self::begin_open_with_recovery_reserve(path, identity, minimum, limits, reserve)?;
        while recovery.progress().status == ReplayStatus::Reading {
            recovery.advance(ReplayBudget::UNLIMITED)?;
        }
        recovery.finish()
    }

    /// Capacity of this acknowledged usable owner, not guessed free space after
    /// an uncertain write. No stale balance escapes the original poison boundary.
    pub fn capacity(&self) -> Result<JournalCapacity, Error> {
        if self.poisoned { return Err(Error::Poisoned); }
        Ok(self.journal.capacity())
    }
}

#[cfg(test)]
mod tests;
