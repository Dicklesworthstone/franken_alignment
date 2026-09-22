//! Bounded stream transport for portable replay expectations.
//!
//! Recipe bytes are compared as they arrive against the independently supplied
//! original recipe. Import retains only the bounded state image, not a second
//! copy of model/codebook parameters. Neither successful I/O nor matching bytes
//! certify a captured checkpoint: the original numerical replay is still required.

use super::wire::{Reader, Writer};
use super::{
    ARCHIVE_HEADER_BYTES, ArchiveLimits, GenerationArchive, GenerationCheckpoint,
    ReplayableGeneration, binding, state, DOMAIN,
};
use crate::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::rc::Rc;

#[derive(Debug)]
pub enum ArchiveIoError {
    Refused(Error),
    Io(io::Error),
}

impl fmt::Display for ArchiveIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => write!(f, "generation archive refused: {error:?}"),
            Self::Io(error) => write!(f, "generation archive I/O failed: {error}"),
        }
    }
}
impl std::error::Error for ArchiveIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Refused(_) => None,
        }
    }
}
impl From<Error> for ArchiveIoError {
    fn from(error: Error) -> Self { Self::Refused(error) }
}
impl From<io::Error> for ArchiveIoError {
    fn from(error: io::Error) -> Self { Self::Io(error) }
}

/// Exact logical bytes accepted and flushed by the supplied writer. Flushing is
/// not fsync, file authenticity, atomic publication, or permission to act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveWrite {
    pub encoded_bytes: usize,
    pub recipe_bytes: usize,
    pub state_bytes: usize,
    pub positions: usize,
}

impl GenerationCheckpoint {
    /// Preflight the complete extent before the first write. Emission uses the
    /// same V1 encoder as `encode_archive`, without allocating its archive Vec.
    /// On an I/O error the sink may contain a prefix; callers must not publish it.
    pub fn write_archive_to<W: Write + ?Sized>(
        &self, output: &mut W, limits: ArchiveLimits,
    ) -> Result<ArchiveWrite, ArchiveIoError> {
        let layout = self.archive_layout(limits)?;
        let recipe_bytes = layout.recipe_bytes;
        let state_bytes = layout.state_bytes;
        let encoded_bytes = layout.encoded_bytes;
        let mut io_error = None;
        let result = {
            let mut sink = |bytes: &[u8]| {
                output.write_all(bytes).map_err(|error| {
                    io_error = Some(error);
                    Error::Incomplete
                })
            };
            let mut writer = Writer::sink(&mut sink, encoded_bytes);
            self.write_archive_body(&mut writer, recipe_bytes, state_bytes)
                .and_then(|()| writer.complete())
        };
        transport_result(result, io_error)?;
        output.flush()?;
        Ok(ArchiveWrite { encoded_bytes, recipe_bytes, state_bytes, positions: self.positions() })
    }
}

impl GenerationArchive {
    /// Read one EXACT archive followed by EOF. The header is admitted before
    /// reading its body, and an invalid total/recipe length reads no body bytes.
    /// At most the admitted extent plus one trailing-byte probe is consumed.
    /// `Interrupted` is retried; `WouldBlock` and every other I/O failure are NOT
    /// an end-of-file certificate. Apply transport deadlines outside this API.
    ///
    /// Failure leaves the reader partially consumed. No mutable or partially
    /// reconstructed generator is returned; dispose of that transport on error.
    pub fn read_archive_from<R: Read + ?Sized>(
        input: &mut R, intended: &ReplayableGeneration, limits: ArchiveLimits,
    ) -> Result<Self, ArchiveIoError> {
        limits.check()?;
        if limits.bytes < ARCHIVE_HEADER_BYTES { return Err(Error::Limit.into()); }
        let mut header = [0; ARCHIVE_HEADER_BYTES];
        input.read_exact(&mut header)?;
        let mut reader = Reader::new(&header);
        if reader.take(DOMAIN.len())? != DOMAIN { return Err(Error::InvalidInput.into()); }
        let recipe_bytes = reader.count(limits.recipe_bytes)?;
        let state_bytes = reader.count(limits.state.state_bytes)?;
        reader.end()?;
        let encoded_bytes = ARCHIVE_HEADER_BYTES.checked_add(recipe_bytes)
            .and_then(|n| n.checked_add(state_bytes)).ok_or(Error::Limit)?;
        if encoded_bytes > limits.bytes { return Err(Error::Limit.into()); }

        // A shorter/longer declared recipe cannot turn omitted/extra fields into
        // defaults. Count it before asking the transport for any recipe bytes.
        let mut count = Writer::count(limits.recipe_bytes);
        binding::write(&mut count, &intended.recipe)?;
        if count.len() != recipe_bytes { return Err(Error::Binding.into()); }
        let mut io_error = None;
        let result = {
            let mut buffer = [0_u8; 4096];
            let mut compare = |expected: &[u8]| {
                for chunk in expected.chunks(buffer.len()) {
                    let actual = &mut buffer[..chunk.len()];
                    input.read_exact(actual).map_err(|error| {
                        io_error = Some(error);
                        Error::Incomplete
                    })?;
                    if &*actual != chunk { return Err(Error::Binding); }
                }
                Ok(())
            };
            let mut writer = Writer::sink(&mut compare, recipe_bytes);
            binding::write(&mut writer, &intended.recipe).and_then(|()| writer.complete())
        };
        transport_result(result, io_error)?;

        let mut bytes = Vec::new();
        bytes.try_reserve_exact(state_bytes).map_err(|_| Error::Limit)?;
        bytes.resize(state_bytes, 0);
        input.read_exact(&mut bytes)?;
        let mut tail = [0];
        loop {
            match input.read(&mut tail) {
                Ok(0) => break,
                Ok(_) => return Err(Error::Binding.into()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let expected = state::read(&bytes, &intended.recipe, limits)?;
        Ok(Self { recipe: Rc::clone(&intended.recipe), expected: Rc::new(expected),
            encoded_bytes, recipe_bytes })
    }
}

fn transport_result(result: Result<(), Error>, io_error: Option<io::Error>) -> Result<(), ArchiveIoError> {
    match io_error {
        Some(error) => Err(ArchiveIoError::Io(error)),
        None => result.map_err(ArchiveIoError::Refused),
    }
}
