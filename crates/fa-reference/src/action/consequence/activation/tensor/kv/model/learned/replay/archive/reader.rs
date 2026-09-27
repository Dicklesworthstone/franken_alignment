//! Bounded reader for operator-selected fit archives, not an alternate executor.
use super::{LearnedKvFitArchive, LearnedKvFitBinding, MAX_FIT_ARCHIVE_BYTES, validate_binding};
use crate::Error;
use std::io::{self, Read};

pub const MAX_FIT_ARCHIVE_READ_CALLS: usize = 65_536;

/// One caller-owned allowance across successes, refusals and retries. Returned
/// bytes are charged; every attempted read, including Interrupted and the EOF
/// probe, consumes a call. This is not physical storage/latency accounting.
#[derive(Debug)]
pub struct FitArchiveReadBudget {
    bytes: usize,
    calls: usize,
    consumed_bytes: usize,
    used_calls: usize,
}
impl FitArchiveReadBudget {
    /// Leave at least one byte of capacity for the EOF probe. A full allowance
    /// cannot establish that an unobserved tail is absent by issuing a zero-read.
    pub fn new(bytes: usize, calls: usize) -> Result<Self, Error> {
        if bytes == 0 || calls == 0 || bytes > MAX_FIT_ARCHIVE_BYTES + 1
            || calls > MAX_FIT_ARCHIVE_READ_CALLS { return Err(Error::Limit); }
        Ok(Self { bytes, calls, consumed_bytes: 0, used_calls: 0 })
    }
    pub fn remaining_bytes(&self) -> usize { self.bytes }
    pub fn remaining_calls(&self) -> usize { self.calls }
    pub fn consumed_bytes(&self) -> usize { self.consumed_bytes }
    pub fn used_calls(&self) -> usize { self.used_calls }
}

#[derive(Debug)]
pub enum FitArchiveReadError { Contract(Error), Io(io::ErrorKind) }
impl From<Error> for FitArchiveReadError { fn from(error: Error) -> Self { Self::Contract(error) } }
impl std::fmt::Display for FitArchiveReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for FitArchiveReadError {}

impl LearnedKvFitArchive {
    /// Read a complete bounded input from an explicitly supplied reader (for
    /// example an operator-opened regular file). No filename, download, training
    /// job, fit, callback-selected model or authority is obtained from archive
    /// contents. A parsed return value still requires explicit original replay.
    ///
    /// No progress object escapes on malformed/truncated/over-budget input or
    /// I/O failure. The caller's spent read budget is never replenished, even
    /// when the failure occurred after the entire apparent prefix was read.
    pub fn read<R: Read + ?Sized>(source: &mut R, expected: &LearnedKvFitBinding,
        budget: &mut FitArchiveReadBudget) -> Result<Self, FitArchiveReadError>
    {
        validate_binding(expected)?;
        let mut bytes = Vec::new(); let mut block = [0_u8; 8192];
        loop {
            if budget.calls == 0 || budget.bytes == 0 { return Err(Error::Limit.into()); }
            let remaining = MAX_FIT_ARCHIVE_BYTES.checked_sub(bytes.len()).ok_or(Error::Limit)?;
            let capacity = block.len().min(budget.bytes).min(remaining + 1);
            budget.calls -= 1; budget.used_calls += 1;
            match source.read(&mut block[..capacity]) {
                Ok(0) => return Self::decode(&bytes, expected, MAX_FIT_ARCHIVE_BYTES).map_err(Into::into),
                Ok(count) => {
                    if count > capacity {
                        // A reader violating Read's contract cannot panic this
                        // parser or create uncharged additional input capacity.
                        budget.bytes -= capacity; budget.consumed_bytes += capacity;
                        return Err(Error::InvalidInput.into());
                    }
                    budget.bytes -= count; budget.consumed_bytes += count;
                    if count > remaining { return Err(Error::Limit.into()); }
                    bytes.try_reserve(count).map_err(|_| Error::Limit)?;
                    bytes.extend_from_slice(&block[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(FitArchiveReadError::Io(error.kind())),
            }
        }
    }
}
