//! Bounded grouping of the original canonical records, not a new wire format.
//! Groups are all-or-nothing in memory, NOT atomic multi-record disk writes.

use crate::{Error, Event, Head, Journal, apply_event, crc32, encode_event};

pub const MAX_BATCH_RECORDS: usize = 1_024;
pub const MAX_BATCH_ENCODED_BYTES: usize = 8 * 1024 * 1024;

/// Additional per-call ceilings. The journal's original frame, record and byte
/// limits remain mandatory and cumulative; a group never replenishes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchLimits {
    pub max_records: usize,
    /// Sum of complete wire records, including lengths, chains and checksums.
    /// Excludes the existing journal, its private clone and caller-owned events.
    pub max_encoded_bytes: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self { max_records: MAX_BATCH_RECORDS, max_encoded_bytes: MAX_BATCH_ENCODED_BYTES }
    }
}

impl Journal {
    /// Validate and append an ordered, nonempty group without publishing a
    /// partially applied prefix on failure. Bytes and final state equal repeated
    /// successful `append` calls; there are no batch markers or reordered events.
    /// The original single-record method remains an independent framing oracle.
    ///
    /// This is only in-memory accounting. Persist the entire returned buffer
    /// before acknowledging its final head. A failed physical write can leave
    /// any prefix, including complete records: do not assume group rollback,
    /// refund unknown effects, or retry from a stale in-memory ledger.
    pub fn append_batch(&mut self, events: &[Event], limits: BatchLimits) -> Result<Vec<u8>, Error> {
        let (candidate, bytes) = self.prepare_batch(events, limits)?;
        *self = candidate;
        Ok(bytes)
    }

    // The existing file owner publishes this SAME candidate only after sync.
    // There is no public prepared-state import, callback or mutable ledger view.
    pub(crate) fn prepare_batch(&self, events: &[Event], limits: BatchLimits)
        -> Result<(Self, Vec<u8>), Error>
    {
        if events.is_empty() { return Err(Error::InvalidInput); }
        if limits.max_records == 0 || limits.max_records > MAX_BATCH_RECORDS
            || limits.max_encoded_bytes == 0 || limits.max_encoded_bytes > MAX_BATCH_ENCODED_BYTES
            || events.len() > limits.max_records { return Err(Error::Limit); }
        let count = u64::try_from(events.len()).map_err(|_| Error::Limit)?;
        if self.head.sequence.checked_add(count).is_none_or(|end| end > self.limits.max_records) {
            return Err(Error::Limit);
        }
        // Every original record has at least 20 framing bytes and one tag.
        // Reject impossible groups before cloning the retained rights history.
        let minimum = events.len().checked_mul(21).ok_or(Error::Limit)?;
        if minimum > limits.max_encoded_bytes
            || self.encoded_len.checked_add(minimum as u64)
                .is_none_or(|end| end > self.limits.max_total_bytes) { return Err(Error::Limit); }
        let mut candidate = self.clone();
        let mut bytes = Vec::new();
        for event in events {
            let remaining = (limits.max_encoded_bytes - bytes.len()) as u64;
            let remaining = remaining.min(candidate.limits.max_total_bytes - candidate.encoded_len);
            if remaining < 21 { return Err(Error::Limit); }
            // Restrict the ORIGINAL encoder before copying any payload. The
            // bounded cast is safe even when the lifetime byte limit is huge.
            let payload_limit = (remaining - 20).min(u64::from(candidate.limits.max_frame_bytes - 12));
            let payload = encode_event(event, payload_limit as usize)?;
            let body_len = 12 + payload.len();
            let encoded_len = candidate.check_growth(body_len)?;
            let sequence = candidate.head.sequence.checked_add(1).ok_or(Error::Limit)?;
            bytes.try_reserve_exact(body_len + 8).map_err(|_| Error::Limit)?;
            let start = bytes.len();
            bytes.extend_from_slice(&(body_len as u32).to_le_bytes());
            bytes.extend_from_slice(&sequence.to_le_bytes());
            bytes.extend_from_slice(&candidate.head.checksum.to_le_bytes());
            bytes.extend_from_slice(&payload);
            let checksum = crc32(&bytes[start..]);
            bytes.extend_from_slice(&checksum.to_le_bytes());
            // A failed reducer (including an unwind) can change only this
            // private candidate. No original rights are cloned for each record.
            apply_event(&mut candidate.rights, event)?;
            candidate.head = Head { sequence, checksum };
            candidate.encoded_len = encoded_len;
        }
        Ok((candidate, bytes))
    }
}
