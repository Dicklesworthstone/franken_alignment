//! Concrete producer heartbeat acquisition; never renew from a cached image.
//! The producer/path and shared elapsed clock remain operator trust boundaries.
mod bootstrap;
use super::FileCaptureError;
use super::super::super::{FileOversight, JournalError, JournalFailure, JournalIo};
use super::super::witness_gate::freshness::{read_heartbeat, write_heartbeat};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::delivery::publication_gate::changes::freshness::{PublicationHeartbeat, PublicationFreshnessStatus};
use crate::Error;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const DOMAIN: &[u8; 8] = b"FAPHBT01";
pub const PUBLICATION_HEARTBEAT_BYTES: usize = 48;

impl PublicationHeartbeat {
    /// Producer interchange, not a permit. Write a new immutable file and rename
    /// it atomically. Increase generation when any field changes; never rewrite
    /// produced_at merely because a consumer reads the file again.
    pub fn to_bytes(self) -> Result<Vec<u8>, Error> {
        if self.source == 0 || self.clock_domain == 0 || self.generation == 0 { return Err(Error::InvalidInput); }
        let mut w = Writer::new(PUBLICATION_HEARTBEAT_BYTES);
        w.raw(DOMAIN)?; write_heartbeat(&mut w, self)?;
        Ok(w.finish())
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > PUBLICATION_HEARTBEAT_BYTES { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
        let heartbeat = read_heartbeat(&mut r)?;
        r.end()?;
        if heartbeat.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(heartbeat)
    }
}

/// Fixed source selection, no cached positive result or caller-defined reader.
/// This is bounded local I/O, not an adversarial-filesystem or real-time sandbox.
#[derive(Debug)]
pub struct PublicationHeartbeatFile {
    path: PathBuf,
    source: u64,
}
impl PublicationHeartbeatFile {
    pub fn new(path: impl AsRef<Path>, source: u64) -> Result<Self, Error> {
        let path = path.as_ref();
        if source == 0 || path.as_os_str().is_empty() { return Err(Error::InvalidInput); }
        if path.as_os_str().as_bytes().len() > 4_096 { return Err(Error::Limit); }
        Ok(Self { path: path.to_owned(), source })
    }
    pub fn source(&self) -> u64 { self.source }
    pub fn read_heartbeat(&self) -> Result<PublicationHeartbeat, FileCaptureError> {
        let before = regular(&self.path)?;
        let file = File::open(&self.path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || stamp(&before) != stamp(&opened) { return Err(Error::Stale.into()); }
        let mut bytes = Vec::with_capacity(PUBLICATION_HEARTBEAT_BYTES + 1);
        (&file).take(PUBLICATION_HEARTBEAT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > PUBLICATION_HEARTBEAT_BYTES { return Err(Error::Limit.into()); }
        if stamp(&opened) != stamp(&file.metadata()?) || stamp(&opened) != stamp(&regular(&self.path)?) {
            return Err(Error::Stale.into());
        }
        let heartbeat = PublicationHeartbeat::from_bytes(&bytes)?;
        if heartbeat.source != self.source { return Err(Error::Binding.into()); }
        Ok(heartbeat)
    }
}
fn regular(path: &Path) -> Result<Metadata, FileCaptureError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(Error::Binding.into()); }
    if metadata.len() > PUBLICATION_HEARTBEAT_BYTES as u64 { return Err(Error::Limit.into()); }
    if metadata.len() < PUBLICATION_HEARTBEAT_BYTES as u64 { return Err(Error::Incomplete.into()); }
    Ok(metadata)
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())
}

impl FileOversight {
    /// Withdraw DURABLY, reread the concrete file, sample trusted time AFTER the
    /// read, then acknowledge the original heartbeat event. A successful read can
    /// still produce status.eligibility == Err: no permitting fallback is made.
    ///
    /// File failure is an inner error with the withdrawal retained. Installation,
    /// clock, allocation or storage failure is an outer error and leaves this live
    /// owner unavailable. Neither the source nor an old report is needed to settle
    /// an already executed effect through the original receipt path.
    pub fn refresh_publication_heartbeat<F>(&mut self, revision: u64,
        source: &PublicationHeartbeatFile, mut clock: F)
        -> Result<Result<PublicationFreshnessStatus, FileCaptureError>, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.publication_changes_unavailable(revision, source.source())?;
        let expected = self.revision();
        let heartbeat = match source.read_heartbeat() {
            Ok(heartbeat) => heartbeat,
            Err(error) => return Ok(Err(error)),
        };
        // Caught clock unwind must not forget a newly read producer generation.
        // Only fenced recovery can make this owner writable after that failure.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        let now = clock();
        Ok(Ok(self.finish_publication_heartbeat(expected, heartbeat, now)?))
    }
}
