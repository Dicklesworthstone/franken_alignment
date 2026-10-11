//! Bounded, versioned replay of the reference rights state machine.
//!
//! Replay reconstructs accounting only: it never dispatches an external effect
//! or verifies a remote receipt. Run/policy identifiers and recovery anchors must
//! come from the caller's trusted context. CRC-32 detects accidental damage; it
//! is NOT authentication, a signature, or protection against a malicious writer.
#![forbid(unsafe_code)]

#[cfg(unix)]
pub mod file;
pub mod replay;

use fa_reference::{Effect, Rights};
use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Read};

const MAGIC: &[u8; 8] = b"FARJ\0\0\0\x01";
const HEADER_LEN: usize = 84;
const HARD_FRAME_LIMIT: u32 = 1_048_576;

/// Opaque, externally established identifiers; this crate does not hash policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub run: [u8; 32],
    pub policy: [u8; 32],
    pub total_units: u64,
}

/// Retain an acknowledged head outside the journal to detect rollback behind it.
/// The checksum is only an accidental-corruption check, not an authenticator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub sequence: u64,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_frame_bytes: u32,
    pub max_records: u64,
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_frame_bytes: HARD_FRAME_LIMIT,
            max_records: 100_000,
            max_total_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Reserve { id: u64, effect: Effect },
    Dispatch { id: u64, effect: Effect },
    MarkUnknown { id: u64 },
    AbortBeforeDispatch { id: u64 },
    /// Trusted reference input. Does not authenticate a remote effect receipt.
    Reconcile { id: u64, occurred: bool },
    RevokeEpoch,
    ResetToCheckpoint { keep: BTreeSet<u64> },
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Model(fa_reference::Error),
    InvalidInput,
    Limit,
    Corrupt(&'static str),
    Binding,
    BehindAnchor,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rights journal: {self:?}")
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<fa_reference::Error> for Error {
    fn from(value: fa_reference::Error) -> Self {
        Self::Model(value)
    }
}

/// An in-memory codec/replay state, not a durable commit or execution authority.
#[derive(Clone, Debug)]
pub struct Journal {
    identity: Identity,
    limits: Limits,
    head: Head,
    encoded_len: u64,
    rights: Rights,
}

impl Journal {
    pub fn new(identity: Identity, limits: Limits) -> Result<Self, Error> {
        if identity.run == [0; 32] || identity.policy == [0; 32] {
            return Err(Error::InvalidInput);
        }
        if limits.max_frame_bytes < 13
            || limits.max_frame_bytes > HARD_FRAME_LIMIT
            || limits.max_records == 0
            || limits.max_records > 1_000_000
            || limits.max_total_bytes < HEADER_LEN as u64
        {
            return Err(Error::Limit);
        }
        let header = encode_header(identity);
        Ok(Self {
            identity,
            limits,
            head: Head { sequence: 0, checksum: crc32(&header[..80]) },
            encoded_len: HEADER_LEN as u64,
            rights: Rights::new(identity.total_units),
        })
    }

    pub fn identity(&self) -> Identity {
        self.identity
    }

    pub fn head(&self) -> Head {
        self.head
    }

    pub fn encoded_len(&self) -> u64 {
        self.encoded_len
    }

    pub fn rights(&self) -> &Rights {
        &self.rights
    }

    pub fn header(&self) -> Vec<u8> {
        encode_header(self.identity)
    }

    /// Applies an event in memory and returns its canonical encoded record.
    /// Failure leaves this value unchanged. Persisting these bytes is a separate
    /// operation; this method by itself makes no durability claim.
    pub fn append(&mut self, event: &Event) -> Result<Vec<u8>, Error> {
        let payload = encode_event(event, self.limits.max_frame_bytes as usize - 12)?;
        let sequence = self.head.sequence.checked_add(1).ok_or(Error::Limit)?;
        let body_len = 12 + payload.len();
        let encoded_len = self.check_growth(body_len)?;
        let mut frame = Vec::with_capacity(body_len + 8);
        frame.extend_from_slice(&(body_len as u32).to_le_bytes());
        frame.extend_from_slice(&sequence.to_le_bytes());
        frame.extend_from_slice(&self.head.checksum.to_le_bytes());
        frame.extend_from_slice(&payload);
        let checksum = crc32(&frame);
        frame.extend_from_slice(&checksum.to_le_bytes());
        let mut candidate = self.rights.clone();
        apply_event(&mut candidate, event)?;
        self.rights = candidate;
        self.head = Head { sequence, checksum };
        self.encoded_len = encoded_len;
        Ok(frame)
    }

    /// Strict, streaming replay. Partial headers/records, malformed transitions,
    /// wrong context, reordered records and rollback behind `minimum` fail closed.
    /// A valid suffix beyond the anchor is replayed: an interrupted write may
    /// have persisted despite its caller not receiving an acknowledgement.
    pub fn replay(
        mut reader: impl Read,
        identity: Identity,
        minimum: Head,
        limits: Limits,
    ) -> Result<Self, Error> {
        let mut journal = Self::new(identity, limits)?;
        let mut header = [0; HEADER_LEN];
        reader.read_exact(&mut header)?;
        if header.as_slice() != journal.header().as_slice() {
            return Err(Error::Binding);
        }
        let mut anchor_seen = minimum == journal.head;
        if minimum.sequence == 0 && !anchor_seen {
            return Err(Error::Binding);
        }
        loop {
            let mut length = [0; 4];
            // Only EOF *between* complete records is a clean end of stream.
            let count = loop {
                match reader.read(&mut length[..1]) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    other => break other?,
                }
            };
            if count == 0 {
                break;
            }
            reader.read_exact(&mut length[1..])?;
            let body_len = u32::from_le_bytes(length) as usize;
            let encoded_len = journal.check_growth(body_len)?;
            let mut body = vec![0; body_len];
            reader.read_exact(&mut body)?;
            let mut checksum_bytes = [0; 4];
            reader.read_exact(&mut checksum_bytes)?;
            let checksum = u32::from_le_bytes(checksum_bytes);
            let actual = crc32_parts(&[&length, &body]);
            if checksum != actual {
                return Err(Error::Corrupt("record checksum"));
            }
            let mut cursor = Cursor(&body);
            let sequence = cursor.u64()?;
            let previous = cursor.u32()?;
            if Some(sequence) != journal.head.sequence.checked_add(1)
                || previous != journal.head.checksum
            {
                return Err(Error::Corrupt("record ordering or chain"));
            }
            let event = decode_event(cursor.0)?;
            // No per-record state clone during replay: any error discards the
            // entire local replay state, rather than returning a usable prefix.
            apply_event(&mut journal.rights, &event)?;
            journal.head = Head { sequence, checksum };
            journal.encoded_len = encoded_len;
            if sequence == minimum.sequence {
                if journal.head != minimum {
                    return Err(Error::Binding);
                }
                anchor_seen = true;
            }
        }
        if !anchor_seen {
            return Err(Error::BehindAnchor);
        }
        Ok(journal)
    }

    fn check_growth(&self, body_len: usize) -> Result<u64, Error> {
        if body_len < 13
            || body_len > self.limits.max_frame_bytes as usize
            || self.head.sequence >= self.limits.max_records
        {
            return Err(Error::Limit);
        }
        self.encoded_len
            .checked_add(body_len as u64 + 8)
            .filter(|&length| length <= self.limits.max_total_bytes)
            .ok_or(Error::Limit)
    }
}

fn encode_header(identity: Identity) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&identity.run);
    header.extend_from_slice(&identity.policy);
    header.extend_from_slice(&identity.total_units.to_le_bytes());
    let checksum = crc32(&header);
    header.extend_from_slice(&checksum.to_le_bytes());
    header
}

fn apply_event(rights: &mut Rights, event: &Event) -> Result<(), Error> {
    match event {
        Event::Reserve { id, effect } => rights.reserve(*id, effect.clone())?,
        Event::Dispatch { id, effect } => rights.dispatch(*id, effect)?,
        Event::MarkUnknown { id } => rights.mark_unknown(*id)?,
        Event::AbortBeforeDispatch { id } => rights.abort_before_dispatch(*id)?,
        Event::Reconcile { id, occurred } => rights.reconcile(*id, *occurred)?,
        Event::RevokeEpoch => rights.revoke_epoch()?,
        Event::ResetToCheckpoint { keep } => {
            for id in keep {
                rights.state(*id)?;
            }
            rights.reset_to_checkpoint(keep)?;
        }
    }
    if !rights.conserved() {
        return Err(Error::Corrupt("rights conservation"));
    }
    Ok(())
}

fn encode_event(event: &Event, limit: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    match event {
        Event::Reserve { id, effect } | Event::Dispatch { id, effect } => {
            out.push(if matches!(event, Event::Reserve { .. }) { 1 } else { 2 });
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&effect.units.to_le_bytes());
            put_bytes(&mut out, effect.principal.as_bytes(), limit)?;
            put_bytes(&mut out, effect.resolved_target.as_bytes(), limit)?;
            put_bytes(&mut out, &effect.payload, limit)?;
        }
        Event::MarkUnknown { id } => {
            out.push(3);
            out.extend_from_slice(&id.to_le_bytes());
        }
        Event::AbortBeforeDispatch { id } => {
            out.push(4);
            out.extend_from_slice(&id.to_le_bytes());
        }
        Event::Reconcile { id, occurred } => {
            out.push(5);
            out.extend_from_slice(&id.to_le_bytes());
            out.push(u8::from(*occurred));
        }
        Event::RevokeEpoch => out.push(6),
        Event::ResetToCheckpoint { keep } => {
            let required = keep.len().checked_mul(8).and_then(|n| n.checked_add(5));
            if required.is_none_or(|n| n > limit) {
                return Err(Error::Limit);
            }
            out.push(7);
            out.extend_from_slice(&(keep.len() as u32).to_le_bytes());
            for id in keep {
                out.extend_from_slice(&id.to_le_bytes());
            }
        }
    }
    if out.len() > limit {
        return Err(Error::Limit);
    }
    Ok(out)
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<(), Error> {
    let end = out.len().checked_add(4).and_then(|n| n.checked_add(bytes.len()));
    if end.is_none_or(|n| n > limit) {
        return Err(Error::Limit);
    }
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        if count > self.0.len() {
            return Err(Error::Corrupt("short event"));
        }
        let (value, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn bytes(&mut self) -> Result<&'a [u8], Error> {
        let length = self.u32()? as usize;
        self.take(length)
    }

    fn string(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| Error::Corrupt("invalid UTF-8"))
    }
}

fn decode_event(bytes: &[u8]) -> Result<Event, Error> {
    let mut cursor = Cursor(bytes);
    let tag = cursor.u8()?;
    let event = match tag {
        1 | 2 => {
            let id = cursor.u64()?;
            let effect = Effect {
                units: cursor.u64()?,
                principal: cursor.string()?,
                resolved_target: cursor.string()?,
                payload: cursor.bytes()?.to_vec(),
            };
            if tag == 1 { Event::Reserve { id, effect } } else { Event::Dispatch { id, effect } }
        }
        3 => Event::MarkUnknown { id: cursor.u64()? },
        4 => Event::AbortBeforeDispatch { id: cursor.u64()? },
        5 => {
            let id = cursor.u64()?;
            let occurred = match cursor.u8()? {
                0 => false,
                1 => true,
                _ => return Err(Error::Corrupt("noncanonical boolean")),
            };
            Event::Reconcile { id, occurred }
        }
        6 => Event::RevokeEpoch,
        7 => {
            let count = cursor.u32()? as usize;
            if count > cursor.0.len() / 8 {
                return Err(Error::Corrupt("checkpoint length"));
            }
            let mut keep = BTreeSet::new();
            let mut previous = None;
            for _ in 0..count {
                let id = cursor.u64()?;
                if previous.is_some_and(|last| last >= id) {
                    return Err(Error::Corrupt("noncanonical checkpoint set"));
                }
                keep.insert(id);
                previous = Some(id);
            }
            Event::ResetToCheckpoint { keep }
        }
        _ => return Err(Error::Corrupt("unknown event")),
    };
    if !cursor.0.is_empty() {
        return Err(Error::Corrupt("trailing event bytes"));
    }
    Ok(event)
}

fn crc32(bytes: &[u8]) -> u32 {
    crc32_parts(&[bytes])
}

fn crc32_parts(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for bytes in parts {
        for &byte in *bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
            }
        }
    }
    !crc
}
