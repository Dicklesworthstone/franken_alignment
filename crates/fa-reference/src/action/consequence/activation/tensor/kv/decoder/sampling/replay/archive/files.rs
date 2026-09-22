//! Portable learned checkpoints through the existing bounded operator-file IO.
//! File creation is exclusive; save acknowledgment is not a directory-fsync or
//! hostile-path guarantee. Files contain sensitive model and execution state.
use super::{ArchiveLimits, GenerationArchive, GenerationCheckpoint, GenerationReplay,
    ReplayBudget, ReplayReceipt, ReplayableGeneration};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::archive::files::{
    read_regular_bytes, save_bytes_new,
};
pub use crate::action::consequence::activation::tensor::kv::decoder::sampling::archive::files::{
    ArchiveFileError, ArchiveFileOperation,
};
use std::path::Path;

impl GenerationCheckpoint {
    /// Validate and encode first, then use the original exclusive create, Unix
    /// mode 0600 and file sync. Never overwrite a prior archive or symlink.
    /// On write/sync failure a partial or full file can remain, without success.
    pub fn save_archive_new(&self, path: impl AsRef<Path>, limits: ArchiveLimits)
        -> Result<usize, ArchiveFileError>
    {
        let bytes = self.encode_archive(limits).map_err(ArchiveFileError::Format)?;
        save_bytes_new(path.as_ref(), &bytes)
    }
}
impl ReplayableGeneration {
    /// Only read a bounded regular-file image. The independent recipe, not the
    /// file, owns the expected model, monitors, prompt and budget ceilings.
    /// This neither writes/fences anything nor changes the current owner.
    pub fn read_archive_file(&self, path: impl AsRef<Path>, limits: ArchiveLimits)
        -> Result<GenerationArchive, ArchiveFileError>
    {
        limits.check().map_err(ArchiveFileError::Format)?;
        let bytes = read_regular_bytes(path.as_ref(), limits.bytes)?;
        self.decode_archive(&bytes, limits).map_err(ArchiveFileError::Format)
    }
    /// Whole-prefix work admission happens before the original generator starts.
    /// The returned cursor exposes no partially reconstructed owner or logits.
    pub fn begin_replay_file(&self, path: impl AsRef<Path>, limits: ArchiveLimits, budget: ReplayBudget)
        -> Result<GenerationReplay, ArchiveFileError>
    {
        self.read_archive_file(path, limits)?.begin_replay(budget).map_err(ArchiveFileError::Replay)
    }
    /// New computation only after complete comparison. Parsed data never installs
    /// cache/RNG, changes the recipe, replenishes work budgets or clears a hold.
    pub fn replay_file(&self, path: impl AsRef<Path>, limits: ArchiveLimits, budget: ReplayBudget)
        -> Result<(ReplayableGeneration, ReplayReceipt), ArchiveFileError>
    {
        self.read_archive_file(path, limits)?.replay(budget).map_err(ArchiveFileError::Replay)
    }
}
