//! Cooperative strict replay using the original wire decoder and rights reducer.
//! A checked prefix is never exported as recovered accounting.

mod anchor;
pub use anchor::HistoryAnchor;

use crate::{Cursor, Error, Head, Identity, Journal, Limits, apply_event, crc32_parts, decode_event};
use std::fmt;
use std::io::{self, Read};

/// Per-call logical work. Journal Limits independently bound the entire input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayBudget {
    /// Calls to the supplied Read, including Interrupted and EOF attempts.
    pub read_calls: u64,
    /// Bytes returned by Read; buffering below this interface is separate.
    pub read_bytes: u64,
    /// Complete-record verification/reducer attempts, including a failed one.
    pub checked_records: u64,
}

impl ReplayBudget {
    pub const UNLIMITED: Self = Self {
        read_calls: u64::MAX,
        read_bytes: u64::MAX,
        checked_records: u64::MAX,
    };
}

/// Completed reads and attempted checks, retained even after an error or cancel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayWork {
    pub read_calls: u64,
    pub read_bytes: u64,
    pub checked_records: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayStatus {
    Reading,
    /// Clean EOF and the independently supplied minimum have both been checked.
    Ready,
    /// A returned error or unwind permanently disqualified the private prefix.
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayReport {
    pub status: ReplayStatus,
    pub spent: ReplayWork,
    pub total: ReplayWork,
}

#[derive(Debug)]
pub enum ReplayError {
    Journal(Error),
    NotReady,
    Failed,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cooperative rights replay: {self:?}")
    }
}

impl std::error::Error for ReplayError {}

impl From<Error> for ReplayError {
    fn from(error: Error) -> Self {
        Self::Journal(error)
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Header,
    Length,
    Record { body_len: usize, encoded_len: u64 },
}

/// Own one reader and one private reconstruction. No cloned cursor, prefix
/// ledger, mutable reader, imported checkpoint or caller-selected resume offset
/// is exposed. Cancellation/drop discards the reconstruction without a refund,
/// file repair, or external effect. A new reader must start at the original header.
///
/// Each poll limits reads and complete-record checks, NOT CPU instructions or
/// elapsed time. One record check includes its bounded checksum/decoding and the
/// original rights reducer. That reducer may inspect already retained rights.
/// There is at most one frame buffer (max_frame_bytes + 4, or the selected fixed header), plus
/// original decoded event/rights storage. Anchored replay also retains its exact
/// independent prefix. A Read may block or buffer internally.
/// Errors other than Interrupted, including WouldBlock, are terminal; an I/O
/// error is not reinterpreted as temporary EOF or a reusable partial history.
///
/// ```compile_fail,E0599
/// use fa_journal::replay::Replay;
/// fn no_prefix(run: &Replay<&[u8]>) { let _ = run.rights(); }
/// ```
/// ```compile_fail,E0599
/// use fa_journal::replay::Replay;
/// fn no_fork(run: Replay<&[u8]>) { let _ = run.clone(); }
/// ```
#[must_use = "finish only complete replay, or cancel/drop its private reconstruction"]
pub struct Replay<R> {
    reader: R,
    journal: Journal,
    minimum: Head,
    anchor_seen: bool,
    exact_anchor: Option<HistoryAnchor>,
    phase: Phase,
    buffer: Vec<u8>,
    filled: usize,
    length: [u8; 4],
    status: ReplayStatus,
    total: ReplayWork,
}

impl Journal {
    /// Validate immutable limits/context without reading or applying any record.
    /// The original synchronous replay remains an independent framing oracle.
    pub fn begin_replay<R: Read>(
        reader: R,
        identity: Identity,
        minimum: Head,
        limits: Limits,
    ) -> Result<Replay<R>, Error> {
        Self::new(identity, limits)?.replay_cursor(reader, minimum)
    }

    pub(crate) fn replay_cursor<R: Read>(self, reader: R, minimum: Head) -> Result<Replay<R>, Error> {
        let header_len = self.header().len();
        let journal = self;
        Ok(Replay {
            reader,
            anchor_seen: minimum == journal.head(),
            exact_anchor: None,
            journal,
            minimum,
            phase: Phase::Header,
            buffer: vec![0; header_len],
            filled: 0,
            length: [0; 4],
            status: ReplayStatus::Reading,
            total: ReplayWork::default(),
        })
    }
}

impl<R: Read> Replay<R> {
    /// Resume the exact buffered byte offset. A zero read budget does no reads;
    /// an already buffered record can still be checked with checked_records > 0.
    /// Reads may buffer one record ahead of the record-check allowance. A zero
    /// budget in all dimensions does no reads or record checks. Interrupted reads
    /// spend a call, so even a perpetually interrupted reader yields to the host.
    /// On Err, progress() retains all work and finish() permanently refuses.
    pub fn advance(&mut self, budget: ReplayBudget) -> Result<ReplayReport, ReplayError> {
        match self.status {
            ReplayStatus::Ready => return Ok(self.progress()),
            ReplayStatus::Failed => return Err(ReplayError::Failed),
            ReplayStatus::Reading => {}
        }
        // An unwind in Read, allocation, decoding or the original reducer must
        // never expose a possibly changed prefix as resumable recovered state.
        self.status = ReplayStatus::Failed;
        let mut spent = ReplayWork::default();
        let status = self.advance_inner(budget, &mut spent)?;
        self.status = status;
        Ok(ReplayReport { status, spent, total: self.total })
    }

    /// Inspection performs no work. Counts are not a validated-head claim.
    pub fn progress(&self) -> ReplayReport {
        ReplayReport { status: self.status, spent: ReplayWork::default(), total: self.total }
    }

    /// No usable accounting can escape before both clean EOF and anchor coverage.
    pub fn finish(self) -> Result<Journal, ReplayError> {
        self.into_parts().map(|(_, journal)| journal)
    }

    /// Drop the reader/private ledger and retain only non-authoritative costs.
    pub fn cancel(self) -> ReplayWork {
        self.total
    }

    // The locked file adapter needs the SAME reader for final synchronization.
    pub(crate) fn into_parts(self) -> Result<(R, Journal), ReplayError> {
        match self.status {
            ReplayStatus::Ready => Ok((self.reader, self.journal)),
            ReplayStatus::Reading => Err(ReplayError::NotReady),
            ReplayStatus::Failed => Err(ReplayError::Failed),
        }
    }

    fn advance_inner(&mut self, budget: ReplayBudget, spent: &mut ReplayWork)
        -> Result<ReplayStatus, Error>
    {
        loop {
            if self.filled == self.buffer.len() {
                match self.phase {
                    Phase::Header => {
                        if self.buffer != self.journal.header()
                            || (self.minimum.sequence == 0 && !self.anchor_seen)
                        {
                            return Err(Error::Binding);
                        }
                        self.resize_buffer(4)?;
                        self.phase = Phase::Length;
                    }
                    Phase::Length => {
                        self.length.copy_from_slice(&self.buffer);
                        let body_len = u32::from_le_bytes(self.length) as usize;
                        // Validate hostile sizes and all lifetime bounds BEFORE
                        // allocation, payload reads or a reducer attempt.
                        let encoded_len = self.journal.check_growth(body_len)?;
                        self.resize_buffer(body_len + 4)?;
                        self.phase = Phase::Record { body_len, encoded_len };
                    }
                    Phase::Record { body_len, encoded_len } => {
                        if spent.checked_records == budget.checked_records {
                            return Ok(ReplayStatus::Reading);
                        }
                        self.total.checked_records = self.total.checked_records.checked_add(1)
                            .ok_or(Error::Limit)?;
                        spent.checked_records += 1;
                        self.check_record(body_len, encoded_len)?;
                        self.resize_buffer(4)?;
                        self.phase = Phase::Length;
                    }
                }
                continue;
            }
            if spent.read_calls == budget.read_calls || spent.read_bytes == budget.read_bytes {
                return Ok(ReplayStatus::Reading);
            }
            let amount = (budget.read_bytes - spent.read_bytes)
                .min((self.buffer.len() - self.filled) as u64) as usize;
            self.total.read_calls = self.total.read_calls.checked_add(1).ok_or(Error::Limit)?;
            spent.read_calls += 1;
            let count = match self.reader.read(&mut self.buffer[self.filled..self.filled + amount]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                other => other?,
            };
            if count > amount {
                return Err(Error::Corrupt("reader overreported length"));
            }
            if count == 0 {
                if matches!(self.phase, Phase::Length) && self.filled == 0 {
                    return if self.anchor_seen && self.exact_anchor.as_ref()
                        .is_none_or(|anchor| self.total.read_bytes >= anchor.encoded_len())
                    { Ok(ReplayStatus::Ready) }
                        else { Err(Error::BehindAnchor) };
                }
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
            let offset = self.total.read_bytes;
            self.total.read_bytes = self.total.read_bytes.checked_add(count as u64)
                .ok_or(Error::Limit)?;
            spent.read_bytes += count as u64;
            if let Some(anchor) = &self.exact_anchor {
                anchor.check_read(offset, &self.buffer[self.filled..self.filled + count])?;
            }
            self.filled += count;
        }
    }

    fn resize_buffer(&mut self, size: usize) -> Result<(), Error> {
        self.buffer.clear();
        self.buffer.try_reserve_exact(size).map_err(|_| Error::Limit)?;
        self.buffer.resize(size, 0);
        self.filled = 0;
        Ok(())
    }

    fn check_record(&mut self, body_len: usize, encoded_len: u64) -> Result<(), Error> {
        let (body, trailer) = self.buffer.split_at(body_len);
        let mut checksum_bytes = [0; 4];
        checksum_bytes.copy_from_slice(trailer);
        let checksum = u32::from_le_bytes(checksum_bytes);
        if checksum != crc32_parts(&[&self.length, body]) {
            return Err(Error::Corrupt("record checksum"));
        }
        let mut cursor = Cursor(body);
        let sequence = cursor.u64()?;
        let previous = cursor.u32()?;
        if Some(sequence) != self.journal.head.sequence.checked_add(1)
            || previous != self.journal.head.checksum
        {
            return Err(Error::Corrupt("record ordering or chain"));
        }
        let event = decode_event(cursor.0)?;
        self.journal.check_event_growth(&event, body_len)?;
        apply_event(&mut self.journal.rights, &event)?;
        self.journal.head = Head { sequence, checksum };
        self.journal.encoded_len = encoded_len;
        if sequence == self.minimum.sequence {
            if self.journal.head != self.minimum { return Err(Error::Binding); }
            self.anchor_seen = true;
        }
        Ok(())
    }
}
