//! Private checkpoint files, using the repository's cooperating Unix lock and
//! sync/rename discipline. This storage contains no action, permit or rights.
use super::{GenerationFileError as Failure, GenerationFileIo as Stage};
use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) const CANONICAL: &str = "generation.bin";
pub(super) const PENDING: &str = "generation.pending";
const LOCK: &str = "generation.lock";

pub(super) struct Store {
    root: PathBuf,
    _owner: File,
    #[cfg(test)]
    failure: std::cell::Cell<Option<(Stage, usize)>>,
}
fn io_result<T>(result: io::Result<T>, operation: Stage, visible: bool) -> Result<T, Failure> {
    result.map_err(|error| Failure::Io { operation, kind: error.kind(), replacement_may_be_visible: visible })
}
fn regular(path: &Path, stage: Stage) -> Result<fs::Metadata, Failure> {
    let metadata = io_result(fs::symlink_metadata(path), stage, false)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1 { return Err(Failure::InvalidFile); }
    Ok(metadata)
}
fn directory(path: &Path) -> Result<PathBuf, Failure> {
    let meta = io_result(fs::symlink_metadata(path), Stage::Directory, false)?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.mode() & 0o077 != 0 {
        return Err(Failure::InvalidFile);
    }
    io_result(fs::canonicalize(path), Stage::Directory, false)
}
fn sync_directory(root: &Path, visible: bool) -> Result<(), Failure> {
    io_result(File::open(root).and_then(|dir| dir.sync_all()), Stage::DirectorySync, visible)
}
impl Store {
    pub(super) fn create(path: &Path) -> Result<Self, Failure> {
        io_result(DirBuilder::new().mode(0o700).create(path), Stage::Directory, false)?;
        let root = directory(path)?;
        let lock = io_result(OpenOptions::new().read(true).write(true).create_new(true).mode(0o600)
            .open(root.join(LOCK)), Stage::Lock, false)?;
        io_result(lock.sync_all(), Stage::FileSync, false)?;
        let store = Self::locked(root, lock)?;
        sync_directory(&store.root, false)?;
        sync_directory(store.root.parent().ok_or(Failure::InvalidFile)?, false)?;
        Ok(store)
    }
    pub(super) fn open(path: &Path) -> Result<Self, Failure> {
        let root = directory(path)?;
        let path = root.join(LOCK);
        let before = regular(&path, Stage::Lock)?;
        let lock = io_result(OpenOptions::new().read(true).write(true).open(&path), Stage::Lock, false)?;
        let opened = io_result(lock.metadata(), Stage::Lock, false)?;
        if before.dev() != opened.dev() || before.ino() != opened.ino() { return Err(Failure::InvalidFile); }
        Self::locked(root, lock)
    }
    fn locked(root: PathBuf, lock: File) -> Result<Self, Failure> {
        match lock.try_lock() {
            Ok(()) => Ok(Self { root, _owner: lock,
                #[cfg(test)]
                failure: std::cell::Cell::new(None),
            }),
            Err(TryLockError::WouldBlock) => Err(Failure::Busy),
            Err(TryLockError::Error(error)) => Err(Failure::Io {
                operation: Stage::Lock, kind: error.kind(), replacement_may_be_visible: false,
            }),
        }
    }
    pub(super) fn identity(&self) -> &Path { &self.root }
    pub(super) fn read(&self, maximum: usize) -> Result<Vec<u8>, Failure> {
        let path = self.root.join(CANONICAL);
        let before = regular(&path, Stage::Read)?;
        if before.len() > maximum as u64 { return Err(crate::Error::Limit.into()); }
        let file = io_result(File::open(&path), Stage::Read, false)?;
        let opened = io_result(file.metadata(), Stage::Read, false)?;
        if before.dev() != opened.dev() || before.ino() != opened.ino() { return Err(Failure::InvalidFile); }
        if opened.len() > maximum as u64 { return Err(crate::Error::Limit.into()); }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(usize::try_from(opened.len()).map_err(|_| crate::Error::Limit)?)
            .map_err(|_| crate::Error::Limit)?;
        io_result(file.take(maximum as u64 + 1).read_to_end(&mut bytes), Stage::Read, false)?;
        if bytes.len() > maximum { return Err(crate::Error::Limit.into()); }
        Ok(bytes)
    }
    pub(super) fn replace(&self, bytes: &[u8]) -> Result<(), Failure> {
        let pending = self.root.join(PENDING);
        #[cfg(test)]
        self.at_barrier(Stage::Stage, false)?;
        let mut file = io_result(OpenOptions::new().write(true).create_new(true).mode(0o600)
            .open(&pending), Stage::Stage, false)?;
        #[cfg(test)]
        self.at_barrier(Stage::Write, false)?;
        io_result(file.write_all(bytes), Stage::Write, false)?;
        #[cfg(test)]
        self.at_barrier(Stage::FileSync, false)?;
        io_result(file.sync_all(), Stage::FileSync, false)?;
        #[cfg(test)]
        self.at_barrier(Stage::Rename, true)?;
        io_result(fs::rename(&pending, self.root.join(CANONICAL)), Stage::Rename, true)?;
        #[cfg(test)]
        self.at_barrier(Stage::DirectorySync, true)?;
        sync_directory(&self.root, true)
    }
    /// Only a fully verified canonical checkpoint may remove leftover staging.
    /// A malformed recipe, stale floor or interrupted marker never reaches here.
    pub(super) fn confirm_and_cleanup(&self) -> Result<(), Failure> {
        let path = self.root.join(CANONICAL);
        regular(&path, Stage::Read)?;
        io_result(File::open(path).and_then(|file| file.sync_all()), Stage::FileSync, false)?;
        sync_directory(&self.root, false)?;
        let pending = self.root.join(PENDING);
        match fs::symlink_metadata(&pending) {
            Ok(_) => {
                regular(&pending, Stage::Cleanup)?;
                io_result(fs::remove_file(pending), Stage::Cleanup, false)?;
                sync_directory(&self.root, false)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Failure::Io {
                operation: Stage::Cleanup, kind: error.kind(), replacement_may_be_visible: false,
            }),
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn fail_once(&self, stage: Stage, skip: usize) { self.failure.set(Some((stage, skip))); }
    #[cfg(test)]
    fn at_barrier(&self, stage: Stage, visible: bool) -> Result<(), Failure> {
        if let Some((wanted, skip)) = self.failure.get() && wanted == stage {
            if skip != 0 { self.failure.set(Some((wanted, skip - 1))); }
            else {
                self.failure.set(None);
                return Err(Failure::Io { operation: stage, kind: io::ErrorKind::Other,
                    replacement_may_be_visible: visible });
            }
        }
        Ok(())
    }
}
