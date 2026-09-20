//! Persist the snapshot and its derived feed through one existing replace/fsync.
//! The producer lock protects cooperating writers, not a compromised filesystem.
use super::{FilePublicationInputs, PublicationProducerImage, PublicationProducerProfile, MAX_PRODUCER_BYTES};
use super::super::super::capture::PublicationInputFile;
use super::super::super::capture::heartbeat::feed::PublicationFeedFile;
use crate::action::{ElapsedTick, FrozenAction};
use crate::action::consequence::delivery::persistent::{JournalError, storage};
use crate::Error;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProducerPublicationKind { Created, Replaced, AlreadyCurrent }

/// Acknowledged producer storage, NOT a consumer acquisition or effect receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProducerPublication {
    pub generation: u64,
    pub input_generation: u64,
    pub through: u64,
    pub retained_after: u64,
    pub encoded_bytes: usize,
    pub kind: ProducerPublicationKind,
}

/// Single writer for one bounded producer bundle. There is no caller-supplied
/// notification list, weaker second file, background task or effect capability.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::FilePublicationProducer;
/// fn duplicate(owner: FilePublicationProducer) { let _ = owner.clone(); }
/// ```
pub struct FilePublicationProducer {
    store: storage::Store,
    image: PublicationProducerImage,
    bytes: Vec<u8>,
    fault: Option<JournalError>,
}
impl std::fmt::Debug for FilePublicationProducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilePublicationProducer").field("generation", &self.image.generation())
            .field("fault", &self.fault).finish_non_exhaustive()
    }
}
impl FilePublicationProducer {
    pub fn create(directory: impl AsRef<Path>, profile: PublicationProducerProfile,
        initial: FilePublicationInputs, observed_at: ElapsedTick)
        -> Result<(Self, ProducerPublication), JournalError>
    {
        let image = PublicationProducerImage::new(profile, initial, observed_at)?;
        let bytes = image.to_bytes()?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&bytes)?;
        let owner = Self { store, image, bytes, fault: None };
        let report = owner.report(ProducerPublicationKind::Created);
        Ok((owner, report))
    }

    /// Pin the FULL independently expected profile and minimum heartbeat
    /// generation before durability confirmation/cleanup. Pending images are not
    /// promoted. A floor stored only in this same directory cannot detect rollback.
    pub fn open(directory: impl AsRef<Path>, expected: PublicationProducerProfile,
        minimum_generation: u64) -> Result<Self, JournalError>
    {
        expected.check()?;
        if minimum_generation == 0 { return Err(Error::InvalidInput.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(MAX_PRODUCER_BYTES)?;
        let image = PublicationProducerImage::from_bytes(&bytes)?;
        if image.profile() != expected { return Err(Error::Binding.into()); }
        if image.generation() < minimum_generation { return Err(Error::Stale.into()); }
        store.confirm_and_cleanup()?;
        Ok(Self { store, image, bytes, fault: None })
    }

    /// Last acknowledged data. On an ambiguous error the actual file may be newer.
    pub fn image(&self) -> &PublicationProducerImage { &self.image }
    pub fn failure(&self) -> Option<&JournalError> { self.fault.as_ref() }

    pub fn witness_reader(&self, attempt: u64, action: &FrozenAction) -> Result<PublicationInputFile, JournalError> {
        self.ensure_live()?;
        Ok(PublicationInputFile::from_producer(self.store.identity().join(storage::CANONICAL),
            self.image.profile(), attempt, action)?)
    }
    pub fn feed_reader(&self) -> Result<PublicationFeedFile, JournalError> {
        self.ensure_live()?;
        Ok(PublicationFeedFile::from_producer(self.store.identity().join(storage::CANONICAL), self.image.profile())?)
    }

    /// Build a complete successor, compare the actual canonical predecessor and
    /// replace ONCE. Exact retries verify the file but do not advance generations,
    /// emit duplicate notices or extend heartbeat time. Validation precedes I/O;
    /// any I/O failure or caught storage unwind quarantines this writer.
    pub fn publish(&mut self, expected_generation: u64, inputs: FilePublicationInputs,
        observed_at: ElapsedTick) -> Result<ProducerPublication, JournalError>
    {
        self.ensure_live()?;
        let requested = expected_generation.checked_add(1).ok_or(Error::Overflow)?;
        let retry = requested == self.image.generation();
        let next = if retry {
            if inputs != *self.image.inputs() || observed_at != self.image.batch().heartbeat().produced_at {
                return Err(Error::Binding.into());
            }
            None
        } else { Some(self.image.advance(expected_generation, inputs, observed_at)?) };
        let next_bytes = next.as_ref().map(PublicationProducerImage::to_bytes).transpose()?;
        // Close before the read as well as the replacement. Do not overwrite an
        // observed out-of-band producer change from an old in-memory image.
        self.fault = Some(JournalError::Unavailable);
        let observed = match self.store.read(MAX_PRODUCER_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => return self.fail(error),
        };
        if observed != self.bytes { return self.fail(Error::Binding.into()); }
        match (next, next_bytes) {
            (None, None) => {
                self.fault = None;
                Ok(self.report(ProducerPublicationKind::AlreadyCurrent))
            }
            (Some(next), Some(bytes)) => {
                if let Err(error) = self.store.replace(&bytes) { return self.fail(error); }
                self.image = next;
                self.bytes = bytes;
                self.fault = None;
                Ok(self.report(ProducerPublicationKind::Replaced))
            }
            _ => unreachable!("successor and its encoding are constructed together"),
        }
    }
    fn ensure_live(&self) -> Result<(), JournalError> {
        if self.fault.is_some() { Err(JournalError::Unavailable) } else { Ok(()) }
    }
    fn fail<T>(&mut self, error: JournalError) -> Result<T, JournalError> {
        self.fault = Some(error.clone());
        Err(error)
    }
    fn report(&self, kind: ProducerPublicationKind) -> ProducerPublication {
        ProducerPublication { generation: self.image.generation(), input_generation: self.image.input_generation(),
            through: self.image.batch().heartbeat().through, retained_after: self.image.batch().after(),
            encoded_bytes: self.bytes.len(), kind }
    }
}

impl crate::action::consequence::delivery::persistent::observed::FileOversight {
    /// Bind a producer reader to THIS owner's original frozen action, including
    /// all witnesses, scope, epochs and target bytes. No caller-created action or
    /// second journal is needed by actor-wire consumers. This constructs a reader
    /// only: it neither reads evidence nor grants current publication eligibility.
    pub fn publication_producer_reader(&self, attempt: u64, path: impl AsRef<Path>,
        expected: PublicationProducerProfile) -> Result<PublicationInputFile, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let action = self.machine.actions.get(&attempt).ok_or(Error::Missing)?;
        Ok(PublicationInputFile::from_producer(path, expected, attempt, action)?)
    }
}

#[cfg(test)]
mod storage_tests;
