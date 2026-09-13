//! Cooperating Unix owner lock and one canonical replacement. Only this module
//! performs I/O; journal replay always uses a memory-only publication endpoint.
use super::{JournalError, JournalFailure, JournalIo, MAX_JOURNAL_BYTES};
use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(super) const CANONICAL: &str = "delivery.bin";
const PENDING: &str = "delivery.pending";
const LOCK: &str = "delivery.lock";

pub(super) struct Store { root: PathBuf, owner: File }
fn failed(operation: JournalIo, error: io::Error, replacement_may_be_visible: bool) -> JournalError {
    JournalError::Io(JournalFailure { operation, kind: error.kind(), replacement_may_be_visible })
}
fn io_result<T>(value: io::Result<T>, operation: JournalIo, visible: bool) -> Result<T, JournalError> {
    value.map_err(|error| failed(operation, error, visible))
}
fn regular(path: &Path, operation: JournalIo) -> Result<fs::Metadata, JournalError> {
    let metadata = io_result(fs::symlink_metadata(path), operation, false)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(JournalError::InvalidFile); }
    Ok(metadata)
}

pub(super) fn identity(path: &Path) -> Result<PathBuf, JournalError> {
    let meta = io_result(fs::symlink_metadata(path), JournalIo::Directory, false)?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0 {
        return Err(JournalError::InvalidFile);
    }
    io_result(fs::canonicalize(path), JournalIo::Directory, false)
}
impl Store {
    pub(super) fn create(path: &Path) -> Result<Self, JournalError> {
        io_result(DirBuilder::new().mode(0o700).create(path), JournalIo::Directory, false)?;
        let root = identity(path)?;
        let file = io_result(OpenOptions::new().read(true).write(true).create_new(true).mode(0o600)
            .open(root.join(LOCK)), JournalIo::Lock, false)?;
        let store = Self::locked(root, file)?;
        io_result(store.owner.sync_all(), JournalIo::FileSync, false)?;
        let parent = store.root.parent().ok_or(JournalError::InvalidFile)?;
        io_result(File::open(parent).and_then(|file| file.sync_all()), JournalIo::DirectorySync, false)?;
        Ok(store)
    }
    pub(super) fn open(path: &Path) -> Result<Self, JournalError> {
        let root = identity(path)?;
        let lock = root.join(LOCK);
        regular(&lock, JournalIo::Lock)?;
        let file = io_result(OpenOptions::new().read(true).write(true).open(lock), JournalIo::Lock, false)?;
        let metadata = io_result(file.metadata(), JournalIo::Lock, false)?;
        if !metadata.is_file() { return Err(JournalError::InvalidFile); }
        Self::locked(root, file)
    }
    fn locked(root: PathBuf, file: File) -> Result<Self, JournalError> {
        match file.try_lock() {
            Ok(()) => Ok(Self { root, owner: file }),
            Err(TryLockError::WouldBlock) => Err(JournalError::Busy),
            Err(TryLockError::Error(error)) => Err(failed(JournalIo::Lock, error, false)),
        }
    }
    pub(super) fn identity(&self) -> &Path { &self.root }
    pub(super) fn read(&self, limit: usize) -> Result<Vec<u8>, JournalError> { read(&self.root.join(CANONICAL), limit) }

    pub(super) fn replace(&self, bytes: &[u8]) -> Result<(), JournalError> {
        let pending = self.root.join(PENDING);
        // Leftover staged bytes block this owner; only verified exclusive reopen
        // may discard them. Neither an I/O error nor a staging file is an outcome.
        let mut file = io_result(OpenOptions::new().write(true).create_new(true).mode(0o600)
            .open(&pending), JournalIo::Stage, false)?;
        io_result(file.write_all(bytes), JournalIo::Write, false)?;
        io_result(file.sync_all(), JournalIo::FileSync, false)?;
        io_result(fs::rename(&pending, self.root.join(CANONICAL)), JournalIo::Rename, true)?;
        io_result(File::open(&self.root).and_then(|directory| directory.sync_all()), JournalIo::DirectorySync, true)
    }

    pub(super) fn confirm_and_cleanup(&self) -> Result<(), JournalError> {
        regular(&self.root.join(CANONICAL), JournalIo::Read)?;
        io_result(File::open(self.root.join(CANONICAL)).and_then(|file| file.sync_all()), JournalIo::FileSync, false)?;
        io_result(File::open(&self.root).and_then(|directory| directory.sync_all()), JournalIo::DirectorySync, false)?;
        let pending = self.root.join(PENDING);
        match fs::symlink_metadata(&pending) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(JournalError::InvalidFile); }
                io_result(fs::remove_file(pending), JournalIo::Cleanup, false)?;
                io_result(File::open(&self.root).and_then(|directory| directory.sync_all()), JournalIo::DirectorySync, false)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed(JournalIo::Cleanup, error, false)),
        }
        Ok(())
    }
}

pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, JournalError> {
    if limit == 0 || limit > MAX_JOURNAL_BYTES { return Err(crate::Error::Limit.into()); }
    let before = regular(path, JournalIo::Read)?;
    if before.len() > limit as u64 { return Err(crate::Error::Limit.into()); }
    let file = io_result(File::open(path), JournalIo::Read, false)?;
    let opened = io_result(file.metadata(), JournalIo::Read, false)?;
    if !opened.is_file() { return Err(JournalError::InvalidFile); }
    if opened.len() > limit as u64 { return Err(crate::Error::Limit.into()); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(opened.len()).map_err(|_| crate::Error::Limit)?)
        .map_err(|_| crate::Error::Limit)?;
    io_result(file.take(limit as u64 + 1).read_to_end(&mut bytes), JournalIo::Read, false)?;
    if bytes.len() > limit { return Err(crate::Error::Limit.into()); }
    Ok(bytes)
}
