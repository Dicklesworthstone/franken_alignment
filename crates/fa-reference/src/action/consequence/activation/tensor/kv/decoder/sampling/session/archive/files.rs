//! Operator-controlled checkpoint files, without executable import or authority.
//! Creation is exclusive; partial writes are not promoted to successful saves.

use super::{ArchiveLimits, ArchiveReplayReceipt, SampledArchive};
use super::super::{SampleBudget, SampledCheckpoint, SampledSession};
use super::super::super::SamplingPolicy;
use super::super::super::super::DecoderModel;
use crate::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFileOperation { Metadata, Open, Read, Create, Write, Sync }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFileError {
    Format(Error),
    Replay(Error),
    NotRegular,
    Io { operation: ArchiveFileOperation, kind: io::ErrorKind },
}
impl fmt::Display for ArchiveFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for ArchiveFileError {}
fn io_error(operation: ArchiveFileOperation, error: io::Error) -> ArchiveFileError {
    ArchiveFileError::Io { operation, kind: error.kind() }
}

impl SampledCheckpoint {
    /// Encode/validate before creating anything, then create a NEW file only.
    /// Existing files and symlinks are never overwritten. Unix files start mode
    /// 0600 (subject to umask). A write/sync error can leave a partial/full file;
    /// it is retained for inspection, not deleted or reported as a successful save.
    /// sync_all covers this file, not durable parent-directory publication.
    pub fn save_archive_new(&self, path: impl AsRef<Path>, limits: ArchiveLimits)
        -> Result<usize, ArchiveFileError>
    {
        let bytes = self.encode_archive(limits).map_err(ArchiveFileError::Format)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path.as_ref()).map_err(|e| io_error(ArchiveFileOperation::Create, e))?;
        write_archive(&mut file, &bytes)?;
        file.sync_all().map_err(|e| io_error(ArchiveFileOperation::Sync, e))?;
        Ok(bytes.len())
    }
}

impl SampledArchive {
    /// Bounded whole-file read from an explicitly supplied regular file. The
    /// operator must control the file and all ancestors and publish immutable
    /// bytes. Metadata checks are not a hostile-path/symlink-race sandbox.
    pub fn read_file(path: impl AsRef<Path>, model: &DecoderModel, expected: &SamplingPolicy,
        limits: ArchiveLimits) -> Result<Self, ArchiveFileError>
    {
        limits.check().map_err(ArchiveFileError::Format)?;
        let path = path.as_ref();
        let before = fs::symlink_metadata(path).map_err(|e| io_error(ArchiveFileOperation::Metadata, e))?;
        if before.file_type().is_symlink() || !before.is_file() { return Err(ArchiveFileError::NotRegular); }
        if before.len() > limits.bytes as u64 { return Err(ArchiveFileError::Format(Error::Limit)); }
        let file = File::open(path).map_err(|e| io_error(ArchiveFileOperation::Open, e))?;
        let opened = file.metadata().map_err(|e| io_error(ArchiveFileOperation::Metadata, e))?;
        if !opened.is_file() { return Err(ArchiveFileError::NotRegular); }
        if opened.len() > limits.bytes as u64 { return Err(ArchiveFileError::Format(Error::Limit)); }
        let bytes = read_archive(file, limits.bytes)?;
        Self::decode(&bytes, model, expected, limits).map_err(ArchiveFileError::Format)
    }
}

impl DecoderModel {
    /// One file-to-computation path. A correct file shape is not successful
    /// replay: propagate numerical mismatches separately and return no session.
    /// No current gate/monitor approval or production capability is restored.
    pub fn recompute_sampled_file(&self, path: impl AsRef<Path>, expected: &SamplingPolicy,
        replay_stream: u64, limits: ArchiveLimits, budget: SampleBudget)
        -> Result<(SampledSession, ArchiveReplayReceipt), ArchiveFileError>
    {
        let archive = SampledArchive::read_file(path, self, expected, limits)?;
        archive.recompute(self, replay_stream, budget).map_err(ArchiveFileError::Replay)
    }
}

fn read_archive<R: Read>(reader: R, limit: usize) -> Result<Vec<u8>, ArchiveFileError> {
    let mut bytes = Vec::new();
    // Read at most one byte beyond the limit to distinguish exact-size EOF from
    // an unindexed suffix. Interrupted filesystem I/O is not wall-clock bounded.
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)
        .map_err(|e| io_error(ArchiveFileOperation::Read, e))?;
    if bytes.len() > limit { return Err(ArchiveFileError::Format(Error::Limit)); }
    Ok(bytes)
}
fn write_archive<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), ArchiveFileError> {
    writer.write_all(bytes).map_err(|e| io_error(ArchiveFileOperation::Write, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ReadFailure { sent: bool }
    impl Read for ReadFailure {
        fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
            if target.is_empty() { return Ok(0); }
            if self.sent { return Err(io::ErrorKind::ConnectionReset.into()); }
            self.sent = true; target[0] = 9; Ok(1)
        }
    }
    struct WriteFailure { bytes: Vec<u8> }
    impl Write for WriteFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.is_empty() { return Ok(0); }
            if !self.bytes.is_empty() { return Err(io::ErrorKind::BrokenPipe.into()); }
            self.bytes.push(bytes[0]); Ok(1)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    #[test]
    fn failed_reads_and_partial_writes_do_not_return_success_or_hide_progress() {
        assert_eq!(read_archive(ReadFailure { sent: false }, 16), Err(ArchiveFileError::Io {
            operation: ArchiveFileOperation::Read, kind: io::ErrorKind::ConnectionReset,
        }));
        let mut writer = WriteFailure { bytes: Vec::new() };
        assert_eq!(write_archive(&mut writer, &[1, 2, 3]), Err(ArchiveFileError::Io {
            operation: ArchiveFileOperation::Write, kind: io::ErrorKind::BrokenPipe,
        }));
        assert_eq!(writer.bytes, &[1]);
        assert_eq!(read_archive(&[1, 2][..], 2).unwrap(), &[1, 2]);
        assert_eq!(read_archive(&[1, 2, 3][..], 2), Err(ArchiveFileError::Format(Error::Limit)));
    }
}
