//! Strict streaming I/O and exact-prefix comparison. Neither input is rewritten.
use super::ledger::Ledger;
use super::wire;
use super::{
    ErrorKind, ExpectedContext, Head, Input, Location, ReadBudget, VerificationReport, VerifyError,
};
use std::io::{self, Read};

struct Source<R> {
    reader: R,
    input: Input,
    offset: u64,
}

impl<R: Read> Source<R> {
    fn new(reader: R, input: Input) -> Self {
        Self {
            reader,
            input,
            offset: 0,
        }
    }

    fn location(&self, sequence: Option<u64>) -> Location {
        Location {
            input: self.input,
            offset: self.offset,
            sequence,
        }
    }

    fn read(
        &mut self,
        bytes: &mut [u8],
        sequence: Option<u64>,
        budget: &mut ReadBudget,
    ) -> Result<usize, VerifyError> {
        loop {
            budget.spend(self.location(sequence))?;
            match self.reader.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    let mut failure = VerifyError::at(
                        ErrorKind::Io,
                        self.location(sequence),
                        "input read failed",
                    );
                    failure.io_kind = Some(error.kind());
                    return Err(failure);
                }
                Ok(count) => {
                    if count > bytes.len() {
                        return Err(VerifyError::at(
                            ErrorKind::Io,
                            self.location(sequence),
                            "reader returned an impossible byte count",
                        ));
                    }
                    self.offset = self.offset.checked_add(count as u64).ok_or_else(|| {
                        VerifyError::at(
                            ErrorKind::ArithmeticOverflow,
                            self.location(sequence),
                            "input offset overflowed",
                        )
                    })?;
                    return Ok(count);
                }
            }
        }
    }

    fn exact(
        &mut self,
        mut bytes: &mut [u8],
        sequence: Option<u64>,
        budget: &mut ReadBudget,
    ) -> Result<(), VerifyError> {
        while !bytes.is_empty() {
            let count = self.read(bytes, sequence, budget)?;
            if count == 0 {
                return Err(VerifyError::at(
                    ErrorKind::TruncatedInput,
                    self.location(sequence),
                    "input ended inside a header or record",
                ));
            }
            bytes = &mut bytes[count..];
        }
        Ok(())
    }
}

struct Anchor<A> {
    source: Source<A>,
    header_len: u64,
    matched: u64,
    // A single byte distinguishes EOF at a record boundary from a longer
    // anchor. That byte is compared when the next journal chunk is available.
    pending: Option<u8>,
    prefix: Option<(Head, u64)>,
}

impl<A: Read> Anchor<A> {
    fn new(reader: A, header_len: u64) -> Self {
        Self {
            source: Source::new(reader, Input::Anchor),
            header_len,
            matched: 0,
            pending: None,
            prefix: None,
        }
    }

    fn mismatch(&self, offset: u64, sequence: Option<u64>) -> VerifyError {
        VerifyError::at(
            ErrorKind::AnchorMismatch,
            Location {
                input: Input::Anchor,
                offset,
                sequence,
            },
            "retained prefix differs from the journal at this exact byte",
        )
    }

    fn compare(
        &mut self,
        bytes: &[u8],
        sequence: Option<u64>,
        budget: &mut ReadBudget,
    ) -> Result<(), VerifyError> {
        if self.prefix.is_some() {
            return Ok(());
        }
        let mut consumed = 0;
        if let Some(byte) = self.pending.take() {
            if bytes.first().copied() != Some(byte) {
                return Err(self.mismatch(self.matched, sequence));
            }
            consumed = 1;
            self.matched += 1;
        }
        let mut buffer = [0; 8192];
        while consumed < bytes.len() {
            let width = buffer.len().min(bytes.len() - consumed);
            let count = self.source.read(&mut buffer[..width], sequence, budget)?;
            if count == 0 {
                let (kind, message) = if self.matched < self.header_len {
                    (
                        ErrorKind::AnchorTooShort,
                        "retained prefix must include the complete selected header",
                    )
                } else {
                    (
                        ErrorKind::AnchorNotRecordBoundary,
                        "retained prefix ended inside a record",
                    )
                };
                return Err(VerifyError::at(
                    kind,
                    Location {
                        input: Input::Anchor,
                        offset: self.matched,
                        sequence,
                    },
                    message,
                ));
            }
            let actual = &bytes[consumed..consumed + count];
            if let Some(index) = buffer[..count].iter().zip(actual).position(|(a, b)| a != b) {
                return Err(self.mismatch(self.matched + index as u64, sequence));
            }
            consumed += count;
            self.matched += count as u64;
        }
        Ok(())
    }

    fn boundary(&mut self, head: Head, budget: &mut ReadBudget) -> Result<(), VerifyError> {
        if self.prefix.is_some() {
            return Ok(());
        }
        let mut byte = [0];
        let sequence = if head.sequence == 0 {
            None
        } else {
            Some(head.sequence)
        };
        if self.source.read(&mut byte, sequence, budget)? == 0 {
            self.prefix = Some((head, self.matched));
        } else {
            self.pending = Some(byte[0]);
        }
        Ok(())
    }

    fn finish(self) -> Result<(Head, u64), VerifyError> {
        self.prefix.ok_or_else(|| {
            VerifyError::at(
                ErrorKind::AnchorBeyondJournal,
                Location {
                    input: Input::Anchor,
                    offset: self.matched,
                    sequence: None,
                },
                "journal ended before the entire independently supplied prefix",
            )
        })
    }
}

fn u32_at(bytes: &[u8], start: usize) -> u32 {
    u32::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
    ])
}

fn u64_at(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
        bytes[start + 4],
        bytes[start + 5],
        bytes[start + 6],
        bytes[start + 7],
    ])
}

pub(super) fn verify<J: Read, A: Read>(
    journal: J,
    anchor: A,
    expected: ExpectedContext,
    budget: &mut ReadBudget,
) -> Result<VerificationReport, VerifyError> {
    // Reject unsupported configuration before touching either reader.
    let wanted = wire::expected_header(expected)?;
    let format_version = wanted[7];
    let mut source = Source::new(journal, Input::Journal);
    let mut prefix = Anchor::new(anchor, wanted.len() as u64);
    let mut header = vec![0; wanted.len()];
    source.exact(&mut header[..8], None, budget)?;
    if &header[..7] != b"FARJ\0\0\0" {
        return Err(VerifyError::at(
            ErrorKind::InvalidMagic,
            Location {
                input: Input::Journal,
                offset: 0,
                sequence: None,
            },
            "input is not a FARJ rights journal",
        ));
    }
    if header[7] != format_version {
        return Err(VerifyError::at(
            ErrorKind::ContextMismatch,
            Location {
                input: Input::Journal,
                offset: 7,
                sequence: None,
            },
            "journal version differs from the independently selected profile",
        ));
    }
    source.exact(&mut header[8..], None, budget)?;
    let header_checksum = u32_at(&header, header.len() - 4);
    if wire::crc32(&[&header[..header.len() - 4]]) != header_checksum {
        return Err(VerifyError::at(
            ErrorKind::ChecksumMismatch,
            Location {
                input: Input::Journal,
                offset: header.len() as u64 - 4,
                sequence: None,
            },
            "header CRC does not match its bytes",
        ));
    }
    if let Some(index) = header.iter().zip(&wanted).position(|(a, b)| a != b) {
        return Err(VerifyError::at(
            ErrorKind::ContextMismatch,
            Location {
                input: Input::Journal,
                offset: index as u64,
                sequence: None,
            },
            "header differs from the independently supplied identity or configuration",
        ));
    }
    let mut head = Head {
        sequence: 0,
        checksum: header_checksum,
    };
    prefix.compare(&header, None, budget)?;
    prefix.boundary(head, budget)?;
    let mut ledger = Ledger::new(expected.total_units);
    loop {
        let start = source.offset;
        let next_sequence = head.sequence.checked_add(1).ok_or_else(|| {
            VerifyError::at(
                ErrorKind::ArithmeticOverflow,
                Location {
                    input: Input::Journal,
                    offset: start,
                    sequence: None,
                },
                "record sequence overflowed",
            )
        })?;
        let at = Location {
            input: Input::Journal,
            offset: start,
            sequence: Some(next_sequence),
        };
        let mut length = [0; 4];
        if source.read(&mut length[..1], Some(next_sequence), budget)? == 0 {
            break;
        }
        if head.sequence >= expected.limits.max_records
            || start
                .checked_add(21)
                .is_none_or(|minimum| minimum > expected.limits.max_total_bytes)
        {
            return Err(VerifyError::at(
                ErrorKind::LimitExceeded,
                at,
                "record exceeds the selected lifetime capacity",
            ));
        }
        source.exact(&mut length[1..], Some(next_sequence), budget)?;
        let body_len = u32::from_le_bytes(length);
        if !(13..=expected.limits.max_frame_bytes).contains(&body_len) {
            return Err(VerifyError::at(
                ErrorKind::LimitExceeded,
                at,
                "frame length is outside the selected bounds",
            ));
        }
        let end = start
            .checked_add(u64::from(body_len) + 8)
            .filter(|end| *end <= expected.limits.max_total_bytes)
            .ok_or_else(|| {
                VerifyError::at(
                    ErrorKind::LimitExceeded,
                    at,
                    "record exceeds the selected lifetime byte capacity",
                )
            })?;
        // Length and cumulative capacity were validated before allocating.
        let mut frame = vec![0; body_len as usize + 4];
        source.exact(&mut frame, Some(next_sequence), budget)?;
        let body = &frame[..body_len as usize];
        let checksum = u32_at(&frame, body_len as usize);
        if wire::crc32(&[&length, body]) != checksum {
            return Err(VerifyError::at(
                ErrorKind::ChecksumMismatch,
                at,
                "record CRC does not match its bytes",
            ));
        }
        if u64_at(body, 0) != next_sequence {
            return Err(VerifyError::at(
                ErrorKind::InvalidSequence,
                at,
                "record sequence is not the next consecutive number",
            ));
        }
        if u32_at(body, 8) != head.checksum {
            return Err(VerifyError::at(
                ErrorKind::InvalidPreviousChecksum,
                at,
                "record predecessor CRC does not match the preceding head",
            ));
        }
        prefix.compare(&length, Some(next_sequence), budget)?;
        prefix.compare(&frame, Some(next_sequence), budget)?;
        let event = wire::decode_event(&body[12..], start + 16, next_sequence)?;
        if let Some(reserve) = expected.recovery {
            if !event.recovery_eligible()
                && (head.sequence >= expected.limits.max_records - reserve.records
                    || end > expected.limits.max_total_bytes - reserve.bytes)
            {
                return Err(VerifyError::at(
                    ErrorKind::LimitExceeded,
                    at,
                    "ordinary event would consume the independently reserved recovery tail",
                ));
            }
        }
        ledger.apply(
            event,
            Location {
                offset: start + 16,
                ..at
            },
        )?;
        head = Head {
            sequence: next_sequence,
            checksum,
        };
        prefix.boundary(head, budget)?;
    }
    let (prefix_head, prefix_bytes) = prefix.finish()?;
    Ok(VerificationReport {
        format_version,
        prefix_head,
        prefix_bytes,
        final_head: head,
        final_bytes: source.offset,
        accounting: ledger.accounting(),
    })
}
