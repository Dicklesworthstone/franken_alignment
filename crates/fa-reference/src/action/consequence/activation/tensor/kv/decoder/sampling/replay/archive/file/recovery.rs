//! Bounded replay while retaining the original file-owner lock and exact cut.
//! No task, thread, callback-based storage or alternative generator is introduced.
use super::{ArchiveLimits, FileGeneration, GenerationFileCommit, GenerationFileError,
    GenerationFileFloor, ReplayBudget, ReplayReceipt, ReplayableGeneration, file_limit, parse, storage};
use super::super::GenerationArchive;
use super::super::super::{GenerationReplay, ReplayStatus};
use crate::Error;
use std::fmt;
use std::path::Path;

/// A locked, incomplete reconstruction. Only the original verifier's status and
/// work receipt are visible. The receipt is numerical evidence, not successful
/// file recovery: `finish` still must confirm the exact canonical cut and storage.
/// Dropping this object releases its lock, without cleanup or a saved candidate.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::file::FileGenerationRecovery;
/// fn early_output(recovery: &FileGenerationRecovery) { recovery.generation(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::file::FileGenerationRecovery;
/// fn fork(recovery: FileGenerationRecovery) { let _ = recovery.clone(); }
/// ```
#[must_use = "advance and finish recovery, or drop it without exposing a numerical owner"]
pub struct FileGenerationRecovery {
    store: storage::Store,
    replay: GenerationReplay,
    limits: ArchiveLimits,
    committed: GenerationFileCommit,
    canonical: Vec<u8>,
}
impl fmt::Debug for FileGenerationRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGenerationRecovery").field("status", &self.status()).finish_non_exhaustive()
    }
}
impl FileGeneration {
    /// Read and validate the exact archive, recipe and floors before beginning
    /// token work. The same exclusive lock survives every host-selected quantum.
    /// Bounds are checked for the WHOLE replay, not refilled for each advance.
    pub fn begin_open(directory: impl AsRef<Path>, intended: &ReplayableGeneration,
        limits: ArchiveLimits, budget: ReplayBudget, minimum: GenerationFileFloor)
        -> Result<FileGenerationRecovery, GenerationFileError>
    {
        limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(file_limit(limits)?)?;
        let saved = parse(&bytes, store.identity(), limits, minimum)?;
        let committed = saved.commit;
        let archive = GenerationArchive::decode(saved.archive, intended, limits)?;
        if archive.positions() as u64 != committed.position { return Err(Error::Binding.into()); }
        let replay = archive.begin_replay(budget)?;
        Ok(FileGenerationRecovery { store, replay, limits, committed, canonical: bytes })
    }
}
impl FileGenerationRecovery {
    pub fn status(&self) -> ReplayStatus { self.replay.status() }
    pub fn receipt(&self) -> Option<&ReplayReceipt> { self.replay.receipt() }

    /// At most this many original token computations, followed by the original
    /// final-state comparison when complete. This does not preempt blocked file
    /// I/O or bound one token's wall time; no incomplete logits/tokens escape.
    pub fn advance(&mut self, positions: usize) -> Result<ReplayStatus, GenerationFileError> {
        Ok(self.replay.advance(positions)?)
    }

    /// No storage cleanup and no live owner before COMPLETE native verification.
    /// Also reject a changed canonical file before cleaning staged evidence. The
    /// cooperating lock is still the exclusion contract; rereading is not a claim
    /// of atomic containment against a hostile writer or directory replacement.
    pub fn finish(self) -> Result<(FileGeneration, ReplayReceipt), GenerationFileError> {
        let Self { store, replay, limits, committed, canonical } = self;
        let (run, receipt) = replay.finish()?;
        if store.read(file_limit(limits)?)? != canonical { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        Ok((FileGeneration { store, run, limits, committed, fault: None }, receipt))
    }
}
