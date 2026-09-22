//! A file-owned learned generation, never a publication/effect authority.
//! Each numerical attempt is marked durably BEFORE execution. Only a complete
//! original archive is published after a quiet step; interruption cannot reopen
//! its older quiet prefix. Recovery replays the independently supplied recipe.
mod storage;
#[cfg(test)]
mod tests;

use super::{ArchiveLimits, GenerationArchive};
use super::super::{ReplayBudget, ReplayReceipt, ReplayableGeneration};
use super::super::super::monitored::{GenerationEvent, GenerationStatus, LearnedGeneration};
use super::wire::{Reader, Writer};
use crate::Error;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::rc::Rc;

const DOMAIN: &[u8; 8] = b"FALGF\0\0\x01";
const HEADER_BYTES: usize = 41;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationFileIo { Directory, Lock, Read, Stage, Write, FileSync, Rename, DirectorySync, Cleanup }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationFileError {
    Contract(Error),
    Io { operation: GenerationFileIo, kind: io::ErrorKind, replacement_may_be_visible: bool },
    /// Actual native terminal/held observation; no resumable checkpoint was made.
    Stopped(GenerationStatus),
    Busy,
    InvalidFile,
    /// A step was started, but no complete successor was acknowledged on disk.
    /// This is not permission to rerun it or restore the older quiet prefix.
    Interrupted { revision: u64, position: u64 },
    Unavailable,
}
impl From<Error> for GenerationFileError {
    fn from(error: Error) -> Self { Self::Contract(error) }
}
impl fmt::Display for GenerationFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for GenerationFileError {}

/// Explicit externally retained lower bounds, not a signature or fork detector.
/// Counters alone cannot detect equal-counter replacement or a stale floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationFileFloor { pub revision: u64, pub position: u64 }

/// Acknowledged numerical checkpoint, NOT an endpoint receipt or effect permit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationFileCommit {
    pub revision: u64,
    pub position: u64,
    pub archive_bytes: usize,
}
impl GenerationFileCommit {
    pub fn floor(self) -> GenerationFileFloor {
        GenerationFileFloor { revision: self.revision, position: self.position }
    }
}

/// Actual original decoder/audit output, exposed only after its checkpoint's
/// file and directory synchronization succeeded. No new vote is manufactured.
#[derive(Clone, Debug)]
pub struct CommittedGenerationEvent {
    pub event: Rc<GenerationEvent>,
    pub checkpoint: GenerationFileCommit,
}

/// One cooperating Unix writer. Files contain sensitive model/state bytes and
/// require an operator-controlled private directory. This is not encryption,
/// hostile-filesystem containment, automatic resend, or a new effect journal.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::file::FileGeneration;
/// fn bypass(owner: &mut FileGeneration) { owner.generation_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::file::FileGeneration;
/// fn revive(owner: &mut FileGeneration) { owner.reset(); }
/// ```
pub struct FileGeneration {
    store: storage::Store,
    run: ReplayableGeneration,
    limits: ArchiveLimits,
    committed: GenerationFileCommit,
    fault: Option<GenerationFileError>,
}
impl fmt::Debug for FileGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGeneration").field("committed", &self.committed)
            .field("fault", &self.fault).finish_non_exhaustive()
    }
}
impl FileGeneration {
    /// Create only from an unadvanced original owner. Every later computation
    /// goes through the write-ahead boundary below; a held prefix cannot be adopted.
    pub fn create(directory: impl AsRef<Path>, run: ReplayableGeneration, limits: ArchiveLimits)
        -> Result<Self, GenerationFileError>
    {
        limits.check()?;
        if run.generation().position() != 0 || !run.generation().status().is_active() {
            return Err(Error::WrongState.into());
        }
        let archive = run.checkpoint(limits.state)?.encode_archive(limits)?;
        let store = storage::Store::create(directory.as_ref())?;
        let bytes = frame(store.identity(), 0, 0, Some(&archive))?;
        store.replace(&bytes)?;
        Ok(Self { store, run, limits, committed: GenerationFileCommit {
            revision: 0, position: 0, archive_bytes: archive.len(),
        }, fault: None })
    }

    /// The intended model, codec, probes, prompt and original budgets come from
    /// the caller's independently constructed owner, never the saved recipe.
    /// Match the whole file and replay all positions BEFORE cleanup or return.
    pub fn open(directory: impl AsRef<Path>, intended: &ReplayableGeneration,
        limits: ArchiveLimits, budget: ReplayBudget, minimum: GenerationFileFloor)
        -> Result<(Self, ReplayReceipt), GenerationFileError>
    {
        limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(file_limit(limits)?)?;
        let saved = parse(&bytes, store.identity(), limits, minimum)?;
        let archive = GenerationArchive::decode(saved.archive, intended, limits)?;
        if archive.positions() as u64 != saved.commit.position { return Err(Error::Binding.into()); }
        let (run, receipt) = archive.replay(budget)?;
        store.confirm_and_cleanup()?;
        Ok((Self { store, run, limits, committed: saved.commit, fault: None }, receipt))
    }

    /// Historical last acknowledgment remains visible on fault; it is not a
    /// statement that a later attempted checkpoint could not have reached disk.
    pub fn last_commit(&self) -> GenerationFileCommit { self.committed }
    pub fn failure(&self) -> Option<&GenerationFileError> { self.fault.as_ref() }
    pub fn generation(&self) -> Result<&LearnedGeneration, GenerationFileError> {
        if self.fault.is_some() { return Err(GenerationFileError::Unavailable); }
        Ok(self.run.generation())
    }

    /// No automatic retry, even after an ambiguous final synchronization. A new
    /// owner must inspect/replay actual canonical bytes with independent floors.
    /// Invalid expected predecessors refuse BEFORE any disk or model operation.
    pub fn advance(&mut self, expected_revision: u64, expected_position: u64)
        -> Result<CommittedGenerationEvent, GenerationFileError>
    {
        if self.fault.is_some() { return Err(GenerationFileError::Unavailable); }
        if expected_revision != self.committed.revision || expected_position != self.committed.position {
            return Err(Error::Stale.into());
        }
        if !self.run.generation().status().is_active() { return Err(Error::WrongState.into()); }
        let next = expected_position.checked_add(1).ok_or(Error::Overflow)?;
        if next > self.limits.state.positions as u64 { return Err(Error::Limit.into()); }
        let revision = expected_revision.checked_add(1).ok_or(Error::Overflow)?;
        let pending = frame(self.store.identity(), revision, expected_position, None)?;
        // Arm before any write or computation. Unwinding and every returned error
        // keep this owner closed, including allocation/encoding after inference.
        self.fault = Some(GenerationFileError::Interrupted { revision, position: expected_position });
        let result: Result<CommittedGenerationEvent, GenerationFileError> = (|| {
            self.store.replace(&pending)?;
            let event = self.run.advance(expected_position)?;
            if event.accepted().is_none() || !event.audit().complete_quiet() {
                return Err(GenerationFileError::Stopped(event.status()));
            }
            // The original checkpoint refuses held/failed/incomplete audits.
            // Its older prefix is already unavailable durably, not recoverable
            // merely by reopening this same directory after a detected alarm.
            let archive = self.run.checkpoint(self.limits.state)?.encode_archive(self.limits)?;
            if self.run.generation().position() != next || event.accepted().is_none()
                || !event.audit().complete_quiet() { return Err(Error::Binding.into()); }
            let bytes = frame(self.store.identity(), revision, next, Some(&archive))?;
            self.store.replace(&bytes)?;
            let checkpoint = GenerationFileCommit { revision, position: next, archive_bytes: archive.len() };
            Ok(CommittedGenerationEvent { event, checkpoint })
        })();
        match result {
            Ok(result) => {
                self.committed = result.checkpoint;
                self.fault = None;
                Ok(result)
            }
            Err(error) => { self.fault = Some(error.clone()); Err(error) }
        }
    }
}

fn file_limit(limits: ArchiveLimits) -> Result<usize, Error> {
    limits.check()?;
    limits.bytes.checked_add(HEADER_BYTES + MAX_PATH_BYTES).ok_or(Error::Limit)
}
fn frame(identity: &Path, revision: u64, position: u64, archive: Option<&[u8]>) -> Result<Vec<u8>, Error> {
    let path = identity.as_os_str().as_bytes();
    if path.is_empty() || path.len() > MAX_PATH_BYTES { return Err(Error::Limit); }
    let archive = archive.map(|bytes| (0_u8, bytes)).unwrap_or((1, &[]));
    let total = HEADER_BYTES.checked_add(path.len()).and_then(|n| n.checked_add(archive.1.len()))
        .ok_or(Error::Limit)?;
    let mut w = Writer::collect(total)?;
    w.bytes(DOMAIN)?; w.u64(revision)?; w.u64(position)?; w.bytes(&[archive.0])?;
    w.blob(path)?; w.blob(archive.1)?;
    w.finish()
}
struct Saved<'a> { commit: GenerationFileCommit, archive: &'a [u8] }
fn parse<'a>(bytes: &'a [u8], identity: &Path, limits: ArchiveLimits, minimum: GenerationFileFloor)
    -> Result<Saved<'a>, GenerationFileError>
{
    if bytes.len() > file_limit(limits)? { return Err(Error::Limit.into()); }
    let mut r = Reader::new(bytes);
    if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::InvalidInput.into()); }
    let revision = r.u64()?; let position = r.u64()?; let tag = r.take(1)?[0];
    let path_bytes = r.count(MAX_PATH_BYTES)?;
    if r.take(path_bytes)? != identity.as_os_str().as_bytes() { return Err(Error::Binding.into()); }
    let length = r.count(limits.bytes)?;
    let archive = r.take(length)?;
    r.end()?;
    if revision < minimum.revision || position < minimum.position { return Err(Error::Stale.into()); }
    match tag {
        0 if revision == position && !archive.is_empty() => Ok(Saved {
            commit: GenerationFileCommit { revision, position, archive_bytes: archive.len() }, archive,
        }),
        1 if position.checked_add(1) == Some(revision) && archive.is_empty() => {
            Err(GenerationFileError::Interrupted { revision, position })
        }
        _ => Err(Error::Binding.into()),
    }
}
