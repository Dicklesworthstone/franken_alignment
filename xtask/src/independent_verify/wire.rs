//! FARJ wire syntax, independently implemented from its documented byte format.
use super::{ErrorKind, ExpectedContext, Input, Location, MAX_FRAME_BYTES, MAX_INPUT_BYTES, MAX_RECORDS, VerifyError};

pub(super) struct Effect<'a> {
    pub units: u64,
    pub principal_nonempty: bool,
    pub target_nonempty: bool,
    /// Canonical bytes from units through payload. Equal encodings imply equal
    /// effects because all lengths, UTF-8 and trailing bytes are checked first.
    pub encoded: &'a [u8],
}

pub(super) enum Event<'a> {
    Reserve { id: u64, effect: Effect<'a> },
    Dispatch { id: u64, effect: Effect<'a> },
    MarkUnknown { id: u64 },
    Abort { id: u64 },
    Reconcile { id: u64, occurred: bool },
    Revoke,
    Reset { keep: Vec<u64> },
}

impl Event<'_> {
    pub fn recovery_eligible(&self) -> bool {
        matches!(self, Self::MarkUnknown { .. } | Self::Abort { .. } | Self::Reconcile { .. } | Self::Revoke)
    }
}

pub(super) fn expected_header(context: ExpectedContext) -> Result<Vec<u8>, VerifyError> {
    if context.run == [0; 32] || context.policy == [0; 32] {
        return Err(VerifyError::context(ErrorKind::InvalidContext, "run and policy identifiers must be independently supplied and nonzero"));
    }
    let limits = context.limits;
    if !(13..=MAX_FRAME_BYTES).contains(&limits.max_frame_bytes)
        || !(1..=MAX_RECORDS).contains(&limits.max_records)
        || limits.max_total_bytes < 84
    {
        return Err(VerifyError::context(ErrorKind::InvalidContext, "selected lifetime limits are outside the native FARJ bounds"));
    }
    if limits.max_total_bytes > MAX_INPUT_BYTES {
        return Err(VerifyError::context(ErrorKind::UnsupportedProfile, "this verifier profile supports configured histories of at most 128 MiB"));
    }
    if let Some(reserve) = context.recovery {
        if reserve.records == 0
            || reserve.records > limits.max_records
            || reserve.records.checked_mul(21).is_none_or(|minimum| reserve.bytes < minimum)
            || 120_u64.checked_add(reserve.bytes).is_none_or(|minimum| minimum > limits.max_total_bytes)
        {
            return Err(VerifyError::context(ErrorKind::InvalidContext, "recovery capacity must fit both independently selected lifetime dimensions"));
        }
    }
    let mut bytes = Vec::with_capacity(if context.recovery.is_some() { 120 } else { 84 });
    bytes.extend_from_slice(b"FARJ\0\0\0");
    bytes.push(if context.recovery.is_some() { 2 } else { 1 });
    bytes.extend_from_slice(&context.run);
    bytes.extend_from_slice(&context.policy);
    bytes.extend_from_slice(&context.total_units.to_le_bytes());
    if let Some(reserve) = context.recovery {
        bytes.extend_from_slice(&limits.max_frame_bytes.to_le_bytes());
        bytes.extend_from_slice(&limits.max_records.to_le_bytes());
        bytes.extend_from_slice(&limits.max_total_bytes.to_le_bytes());
        bytes.extend_from_slice(&reserve.records.to_le_bytes());
        bytes.extend_from_slice(&reserve.bytes.to_le_bytes());
    }
    let checksum = crc32(&[&bytes]);
    bytes.extend_from_slice(&checksum.to_le_bytes());
    Ok(bytes)
}

pub(super) fn decode_event(bytes: &[u8], offset: u64, sequence: u64) -> Result<Event<'_>, VerifyError> {
    let mut cursor = Cursor { bytes, position: 0, offset, sequence };
    let tag = cursor.byte()?;
    let event = match tag {
        1 | 2 => {
            let id = cursor.u64()?;
            let effect_start = cursor.position;
            let units = cursor.u64()?;
            let principal_nonempty = !cursor.text()?.is_empty();
            let target_nonempty = !cursor.text()?.is_empty();
            cursor.field()?;
            let effect = Effect {
                units,
                principal_nonempty,
                target_nonempty,
                encoded: &bytes[effect_start..cursor.position],
            };
            if tag == 1 { Event::Reserve { id, effect } } else { Event::Dispatch { id, effect } }
        }
        3 => Event::MarkUnknown { id: cursor.u64()? },
        4 => Event::Abort { id: cursor.u64()? },
        5 => {
            let id = cursor.u64()?;
            let occurred = match cursor.byte()? {
                0 => false,
                1 => true,
                _ => return Err(cursor.error("reconciliation boolean must be exactly zero or one")),
            };
            Event::Reconcile { id, occurred }
        }
        6 => Event::Revoke,
        7 => {
            let count = cursor.u32()? as usize;
            if count > (bytes.len() - cursor.position) / 8 {
                return Err(cursor.error("checkpoint count exceeds the remaining frame"));
            }
            let mut keep = Vec::with_capacity(count);
            for _ in 0..count {
                let id = cursor.u64()?;
                if keep.last().is_some_and(|previous| *previous >= id) {
                    return Err(cursor.error("checkpoint identifiers must be strictly increasing and unique"));
                }
                keep.push(id);
            }
            Event::Reset { keep }
        }
        _ => return Err(cursor.error("unknown FARJ event tag")),
    };
    if cursor.position != bytes.len() {
        return Err(cursor.error("trailing event bytes are not canonical"));
    }
    Ok(event)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    offset: u64,
    sequence: u64,
}

impl<'a> Cursor<'a> {
    fn error(&self, explanation: &'static str) -> VerifyError {
        VerifyError::at(ErrorKind::NoncanonicalEvent, Location {
            input: Input::Journal,
            offset: self.offset + self.position as u64,
            sequence: Some(self.sequence),
        }, explanation)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], VerifyError> {
        if count > self.bytes.len() - self.position {
            return Err(self.error("event field extends beyond its frame"));
        }
        let start = self.position;
        self.position += count;
        Ok(&self.bytes[start..self.position])
    }

    fn byte(&mut self) -> Result<u8, VerifyError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, VerifyError> {
        let mut encoded = [0; 4];
        encoded.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(encoded))
    }

    fn u64(&mut self) -> Result<u64, VerifyError> {
        let mut encoded = [0; 8];
        encoded.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(encoded))
    }

    fn field(&mut self) -> Result<&'a [u8], VerifyError> {
        let count = self.u32()? as usize;
        self.take(count)
    }

    fn text(&mut self) -> Result<&'a str, VerifyError> {
        let bytes = self.field()?;
        std::str::from_utf8(bytes).map_err(|_| self.error("effect text is not valid UTF-8"))
    }
}

// Reflected IEEE CRC-32. The table is derived here, not imported from the native
// journal. CRC protects accidental integrity only, never authenticity.
const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut remainder = index as u32;
        let mut bit = 0;
        while bit < 8 {
            remainder = if remainder & 1 == 0 { remainder >> 1 } else { (remainder >> 1) ^ 0xedb8_8320 };
            bit += 1;
        }
        table[index] = remainder;
        index += 1;
    }
    table
}

const CRC_TABLE: [u32; 256] = crc_table();

pub(super) fn crc32(parts: &[&[u8]]) -> u32 {
    let mut remainder = u32::MAX;
    for part in parts {
        for &byte in *part {
            remainder = CRC_TABLE[((remainder ^ u32::from(byte)) & 255) as usize] ^ (remainder >> 8);
        }
    }
    !remainder
}
