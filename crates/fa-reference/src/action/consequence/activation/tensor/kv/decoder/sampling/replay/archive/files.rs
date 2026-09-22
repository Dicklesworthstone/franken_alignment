//! Portable learned checkpoints through bounded operator-file streams.
//! File creation is exclusive; save acknowledgment is not a directory-fsync or
//! hostile-path guarantee. Files contain sensitive model and execution state.
use super::{ArchiveLimits, GenerationArchive, GenerationCheckpoint, GenerationReplay,
    ReplayBudget, ReplayReceipt, ReplayableGeneration, stream::ArchiveIoError};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::archive::files::{
    create_archive_file, open_regular_file,
};
pub use crate::action::consequence::activation::tensor::kv::decoder::sampling::archive::files::{
    ArchiveFileError, ArchiveFileOperation,
};
use crate::Error;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

impl GenerationCheckpoint {
    /// Admit the complete representation before creating anything, then stream
    /// the original encoder through a bounded buffer into a NEW private file.
    /// Never overwrite a prior archive or symlink. Success includes file sync,
    /// not parent-directory durability. Failure can leave a partial/full file.
    pub fn save_archive_new(&self, path: impl AsRef<Path>, limits: ArchiveLimits)
        -> Result<usize, ArchiveFileError>
    {
        self.archive_layout(limits).map_err(ArchiveFileError::Format)?;
        let file = create_archive_file(path.as_ref())?;
        let mut writer = BufWriter::new(file);
        let written = self.write_archive_to(&mut writer, limits);
        // Do not let BufWriter::drop silently retry buffered bytes after error.
        // On success write_archive_to has already flushed the complete archive.
        let (file, _) = writer.into_parts();
        let written = written.map_err(|e| stream_error(e, ArchiveFileOperation::Write))?;
        file.sync_all().map_err(|e| ArchiveFileError::Io {
            operation: ArchiveFileOperation::Sync, kind: e.kind(),
        })?;
        Ok(written.encoded_bytes)
    }
}
impl ReplayableGeneration {
    /// Open only a bounded regular file, then compare its recipe incrementally.
    /// The independent recipe, not the file, owns the model, monitors, prompt
    /// and budget ceilings. Only the admitted state image is retained, not an
    /// archive-sized copy of model parameters. Buffered IO may prefetch bytes.
    /// This neither writes/fences anything nor changes the current owner.
    pub fn read_archive_file(&self, path: impl AsRef<Path>, limits: ArchiveLimits)
        -> Result<GenerationArchive, ArchiveFileError>
    {
        limits.check().map_err(ArchiveFileError::Format)?;
        let file = open_regular_file(path.as_ref(), limits.bytes)?;
        let mut reader = BufReader::new(file);
        GenerationArchive::read_archive_from(&mut reader, self, limits)
            .map_err(|e| stream_error(e, ArchiveFileOperation::Read))
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

fn stream_error(error: ArchiveIoError, operation: ArchiveFileOperation) -> ArchiveFileError {
    match error {
        ArchiveIoError::Refused(error) => ArchiveFileError::Format(error),
        // A truncated regular file is still malformed input, not a transport
        // success or a numerical replay failure. Preserve the file API category.
        ArchiveIoError::Io(error) if operation == ArchiveFileOperation::Read
            && error.kind() == io::ErrorKind::UnexpectedEof => ArchiveFileError::Format(Error::Incomplete),
        ArchiveIoError::Io(error) => ArchiveFileError::Io { operation, kind: error.kind() },
    }
}
