//! Bounded, standalone anchor transport for a separately trusted custodian.
//! This framing is not authentication. Original journal validation happens at
//! anchored recovery against the independently supplied profile and Store.

use super::FileHistoryAnchor;
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, MAX_JOURNAL_EVENTS};
use crate::Error;
use std::fmt;
use std::io::{self, Read, Write};

const DOMAIN: &[u8; 8] = b"FAHANC\0\x01";
const HEADER_BYTES: usize = 24;
pub const MAX_HISTORY_ANCHOR_BYTES: usize = MAX_JOURNAL_BYTES + HEADER_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileHistoryAnchorError {
    Contract(Error),
    Io(io::ErrorKind),
}
impl From<Error> for FileHistoryAnchorError {
    fn from(error: Error) -> Self { Self::Contract(error) }
}
impl From<io::Error> for FileHistoryAnchorError {
    fn from(error: io::Error) -> Self { Self::Io(error.kind()) }
}
impl fmt::Display for FileHistoryAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for FileHistoryAnchorError {}

fn check_budget(max_bytes: usize) -> Result<(), Error> {
    if !(HEADER_BYTES..=MAX_HISTORY_ANCHOR_BYTES).contains(&max_bytes) {
        return Err(Error::Limit);
    }
    Ok(())
}

impl FileHistoryAnchor {
    /// Export sensitive anchor data to the operator's independent custodian.
    /// Preflight the entire byte budget before the first write. The writer is
    /// borrowed: this does not flush, sync, replace files, or silently retry an
    /// I/O failure. A partial failed write is NOT a retained/acknowledged anchor.
    /// The caller owns private storage, atomic publication and durability.
    pub fn write_to(&self, writer: &mut impl Write, max_bytes: usize)
        -> Result<usize, FileHistoryAnchorError>
    {
        check_budget(max_bytes)?;
        let len = self.canonical.len();
        if len == 0 || len > MAX_JOURNAL_BYTES || self.revision > MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        let total = HEADER_BYTES.checked_add(len).ok_or(Error::Limit)?;
        if total > max_bytes { return Err(Error::Limit.into()); }
        let mut header = [0_u8; HEADER_BYTES];
        header[..8].copy_from_slice(DOMAIN);
        header[8..16].copy_from_slice(&(self.revision as u64).to_le_bytes());
        header[16..24].copy_from_slice(&(len as u64).to_le_bytes());
        writer.write_all(&header)?;
        writer.write_all(&self.canonical)?;
        Ok(total)
    }

    /// Import a required prefix ONLY from an independently trusted custodian,
    /// not from the actor or the same rollbackable journal being recovered.
    /// Framing, event-count and byte bounds are checked before allocating the
    /// retained payload. This does not replay, authenticate, grant a permit or
    /// validate the embedded journal: open_guarded_anchored must still match it
    /// exactly against the original canonical prefix and independent profile.
    ///
    /// The reader must be a finite standalone archive. A one-byte EOF probe
    /// rejects trailing data, including at the exact declared size limit.
    /// The caller supplies I/O deadlines for blocking readers. No I/O fault is
    /// translated into an empty/default anchor or ignored trailing evidence.
    pub fn read_trusted(reader: &mut impl Read, max_bytes: usize)
        -> Result<Self, FileHistoryAnchorError>
    {
        check_budget(max_bytes)?;
        let mut header = [0_u8; HEADER_BYTES];
        reader.read_exact(&mut header)?;
        if &header[..8] != DOMAIN { return Err(Error::Binding.into()); }
        let revision = u64::from_le_bytes(header[8..16].try_into().map_err(|_| Error::InvalidInput)?);
        let len = u64::from_le_bytes(header[16..24].try_into().map_err(|_| Error::InvalidInput)?);
        if revision > MAX_JOURNAL_EVENTS as u64 || len == 0 || len > MAX_JOURNAL_BYTES as u64 {
            return Err(Error::Limit.into());
        }
        let revision = usize::try_from(revision).map_err(|_| Error::Limit)?;
        let len = usize::try_from(len).map_err(|_| Error::Limit)?;
        if HEADER_BYTES.checked_add(len).ok_or(Error::Limit)? > max_bytes {
            return Err(Error::Limit.into());
        }
        let mut canonical = Vec::new();
        canonical.try_reserve_exact(len).map_err(|_| Error::Limit)?;
        canonical.resize(len, 0);
        reader.read_exact(&mut canonical)?;
        let mut trailing = [0_u8; 1];
        loop {
            match reader.read(&mut trailing) {
                Ok(0) => break,
                Ok(_) => return Err(Error::InvalidInput.into()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self { revision, canonical })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // Deliberately opaque transport bytes, not a valid canonical journal. These
    // framing tests claim no recovery or authentication; real-file controls live
    // in anchored/tests.rs and run through the original journal and authority.
    fn sample() -> FileHistoryAnchor {
        FileHistoryAnchor { revision: 2, canonical: vec![31, 41, 59] }
    }
    fn manual() -> Vec<u8> {
        vec![70, 65, 72, 65, 78, 67, 0, 1,
            2, 0, 0, 0, 0, 0, 0, 0,
            3, 0, 0, 0, 0, 0, 0, 0, 31, 41, 59]
    }

    #[test]
    fn manual_bytes_round_trip_at_exact_budget_and_write_preflight_is_atomic() {
        let bytes = manual();
        let mut output = Vec::new();
        assert_eq!(sample().write_to(&mut output, bytes.len() - 1),
            Err(FileHistoryAnchorError::Contract(Error::Limit)));
        assert!(output.is_empty());
        assert_eq!(sample().write_to(&mut output, bytes.len()), Ok(bytes.len()));
        assert_eq!(output, bytes);
        assert_eq!(FileHistoryAnchor::read_trusted(&mut Cursor::new(&bytes), bytes.len()), Ok(sample()));
        let mut input = Cursor::new(&bytes);
        assert_eq!(FileHistoryAnchor::read_trusted(&mut input, bytes.len() - 1),
            Err(FileHistoryAnchorError::Contract(Error::Limit)));
        assert_eq!(input.position(), HEADER_BYTES as u64);
        for limit in [0, HEADER_BYTES - 1, MAX_HISTORY_ANCHOR_BYTES + 1] {
            let mut input = Cursor::new(&bytes);
            assert_eq!(FileHistoryAnchor::read_trusted(&mut input, limit),
                Err(FileHistoryAnchorError::Contract(Error::Limit)));
            assert_eq!(input.position(), 0);
        }
    }

    #[test]
    fn bad_domain_and_oversized_declarations_refuse_before_payload_read() {
        for index in 0..8 {
            let mut bytes = manual();
            bytes[index] ^= 1;
            let mut input = Cursor::new(bytes);
            assert_eq!(FileHistoryAnchor::read_trusted(&mut input, MAX_HISTORY_ANCHOR_BYTES),
                Err(FileHistoryAnchorError::Contract(Error::Binding)));
            assert_eq!(input.position(), HEADER_BYTES as u64);
        }
        for (start, value) in [(8, MAX_JOURNAL_EVENTS as u64 + 1),
            (8, u64::MAX), (16, MAX_JOURNAL_BYTES as u64 + 1), (16, u64::MAX), (16, 0)]
        {
            let mut bytes = manual();
            bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
            let mut input = Cursor::new(bytes);
            assert_eq!(FileHistoryAnchor::read_trusted(&mut input, MAX_HISTORY_ANCHOR_BYTES),
                Err(FileHistoryAnchorError::Contract(Error::Limit)));
            assert_eq!(input.position(), HEADER_BYTES as u64);
        }
    }

    #[test]
    fn every_truncation_and_trailing_bytes_refuse() {
        let bytes = manual();
        for cut in 0..bytes.len() {
            assert_eq!(FileHistoryAnchor::read_trusted(&mut Cursor::new(&bytes[..cut]), bytes.len()),
                Err(FileHistoryAnchorError::Io(io::ErrorKind::UnexpectedEof)));
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(FileHistoryAnchor::read_trusted(&mut Cursor::new(trailing), bytes.len()),
            Err(FileHistoryAnchorError::Contract(Error::InvalidInput)));
        assert_eq!(FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), 27), Ok(sample()));
    }

    #[test]
    fn interrupted_and_fragmented_reads_work_but_write_failure_is_not_retried() {
        struct Fragmented { input: Cursor<Vec<u8>>, interrupted: bool }
        impl Read for Fragmented {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let count = out.len().min(1);
                self.input.read(&mut out[..count])
            }
        }
        let mut input = Fragmented { input: Cursor::new(manual()), interrupted: false };
        assert_eq!(FileHistoryAnchor::read_trusted(&mut input, 27), Ok(sample()));
        struct FailedWriter { calls: usize, bytes: Vec<u8> }
        impl Write for FailedWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls == 2 { return Err(io::ErrorKind::PermissionDenied.into()); }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> { panic!("caller owns flushing") }
        }
        let mut writer = FailedWriter { calls: 0, bytes: Vec::new() };
        assert_eq!(sample().write_to(&mut writer, 27),
            Err(FileHistoryAnchorError::Io(io::ErrorKind::PermissionDenied)));
        assert_eq!(writer.calls, 2);
        assert_eq!(writer.bytes, manual()[..HEADER_BYTES]);
    }
}
