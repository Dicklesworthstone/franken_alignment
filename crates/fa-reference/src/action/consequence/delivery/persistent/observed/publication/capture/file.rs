//! Read one bounded immutable producer file, never a retained in-memory cache.
//! The operator owns the directory/producer. Producers replace by atomic rename;
//! this is not an adversarial filesystem sandbox or a cryptographic identity.
use super::{FilePublicationCapture, MAX_CAPTURE_BYTES};
use super::super::witnesses::producer::{PublicationProducerImage, PublicationProducerProfile, MAX_PRODUCER_BYTES};
use crate::action::FrozenAction;
use crate::Error;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCaptureError { Data(Error), Io(io::ErrorKind) }
impl From<Error> for FileCaptureError { fn from(error: Error) -> Self { Self::Data(error) } }
impl From<io::Error> for FileCaptureError { fn from(error: io::Error) -> Self { Self::Io(error.kind()) } }
impl std::fmt::Display for FileCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "publication capture: {self:?}") }
}
impl std::error::Error for FileCaptureError {}
impl FileCaptureError {
    /// I/O failure is missing evidence, never a permissive empty observation.
    pub fn contract_error(self) -> Error {
        match self { Self::Data(error) => error, Self::Io(_) => Error::Incomplete }
    }
}

/// A fixed producer/path selection with no cached positive observation and no
/// public reader-override trait. Constructing it performs no file I/O, so the
/// owner can commit unavailability before a read can block, fail or unwind.
#[derive(Debug)]
pub struct PublicationInputFile {
    path: PathBuf,
    source: u64,
    producer: Option<(PublicationProducerProfile, u64, FrozenAction)>,
}
impl PublicationInputFile {
    pub fn new(path: impl AsRef<Path>, source: u64) -> Result<Self, Error> {
        if source == 0 || path.as_ref().as_os_str().is_empty() { return Err(Error::InvalidInput); }
        if path.as_ref().as_os_str().as_bytes().len() > 4_096 { return Err(Error::Limit); }
        Ok(Self { path: path.as_ref().to_owned(), source, producer: None })
    }
    /// Read the snapshot projection of one coupled producer image. The action
    /// binding is fixed at construction; each acquisition still reopens the file.
    pub fn from_producer(path: impl AsRef<Path>, profile: PublicationProducerProfile,
        attempt: u64, action: &FrozenAction) -> Result<Self, Error>
    {
        profile.check()?;
        if action.spec().scope != profile.scope { return Err(Error::Binding); }
        let mut reader = Self::new(path, profile.source)?;
        reader.producer = Some((profile, attempt, action.clone()));
        Ok(reader)
    }
    pub fn source(&self) -> u64 { self.source }

    /// Read original review data or current observations. Only the durable owner
    /// can install these for a source-bound attempt, through its withdrawal/read
    /// cycle. A standalone read cannot refresh an owner's eligibility.
    pub fn read_capture(&self) -> Result<FilePublicationCapture, FileCaptureError> {
        let max_bytes = if self.producer.is_some() { MAX_PRODUCER_BYTES } else { MAX_CAPTURE_BYTES };
        let before = regular(&self.path, max_bytes)?;
        let file = File::open(&self.path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || stamp(&before) != stamp(&opened) { return Err(Error::Stale.into()); }
        if opened.len() > max_bytes as u64 { return Err(Error::Limit.into()); }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(opened.len() as usize).map_err(|_| Error::Limit)?;
        (&file).take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes { return Err(Error::Limit.into()); }
        // Reject detected in-place mutation or replacement during acquisition.
        // Equal metadata is not a cryptographic guarantee against a hostile host.
        if stamp(&opened) != stamp(&file.metadata()?) || stamp(&opened) != stamp(&regular(&self.path, max_bytes)?) {
            return Err(Error::Stale.into());
        }
        let capture = match &self.producer {
            None => FilePublicationCapture::from_bytes(&bytes)?,
            Some((profile, attempt, action)) => {
                let image = PublicationProducerImage::from_bytes(&bytes)?;
                if image.profile() != *profile { return Err(Error::Binding.into()); }
                image.capture(*attempt, action)?
            }
        };
        if capture.identity().source != self.source { return Err(Error::Binding.into()); }
        Ok(capture)
    }
}
fn regular(path: &Path, max_bytes: usize) -> Result<Metadata, FileCaptureError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(Error::Binding.into()); }
    if metadata.len() > max_bytes as u64 { return Err(Error::Limit.into()); }
    Ok(metadata)
}
fn stamp(metadata: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (metadata.dev(), metadata.ino(), metadata.len(), metadata.mtime(), metadata.mtime_nsec(),
        metadata.ctime(), metadata.ctime_nsec())
}
