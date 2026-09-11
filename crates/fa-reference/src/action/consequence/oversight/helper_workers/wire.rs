//! Exact bounded helper request bytes. Transport framing is never model input.
//! The original ActualHelperInput bytes, profile, parts and omissions round-trip.
//! This deliberately retains the reference-only FNV commitment profile.

use super::{HelperPort, MAX_WORKER_SALT_BYTES};
use crate::Error;
use crate::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS,
};
use crate::reducer::MAX_IDENTIFIER_BYTES;
use crate::round::{Digest, Verdict, commitment};

pub const REQUEST_HEADER_BYTES: usize = 9;
pub const MAX_HELPER_FRAME_BYTES: usize = 96 * 1024;
pub const REVEAL_REQUEST: u8 = b'R';
const MAGIC: &[u8; 5] = b"FAHW1";

/// Decoded data for a single helper. It contains no peer views, vote weights,
/// control-ledger handle, effect permit or operator credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerInput {
    round: u64,
    member: String,
    root: [u8; 32],
    actual: ActualHelperInput,
    salt_limit: usize,
}

impl WorkerInput {
    pub fn round(&self) -> u64 { self.round }
    pub fn member(&self) -> &str { &self.member }
    pub fn actual_input(&self) -> &ActualHelperInput { &self.actual }
    pub fn salt_limit(&self) -> usize { self.salt_limit }

    pub fn commitment_frame(&self, verdict: Verdict, salt: &[u8]) -> Result<[u8; 9], Error> {
        if salt.len() > self.salt_limit { return Err(Error::Limit); }
        let digest = commitment(self.round, &self.member, &self.root, verdict, salt)?;
        let mut bytes = [0; 9];
        bytes[0] = b'C';
        bytes[1..].copy_from_slice(&digest.to_be_bytes());
        Ok(bytes)
    }

    /// Send only after the supervising channel's one-byte R reveal request.
    pub fn reveal_frame(&self, verdict: Verdict, salt: &[u8]) -> Result<Vec<u8>, Error> {
        if salt.len() > self.salt_limit { return Err(Error::Limit); }
        let mut bytes = vec![b'R', verdict_tag(verdict)];
        bytes.extend_from_slice(&(salt.len() as u16).to_be_bytes());
        bytes.extend_from_slice(salt);
        Ok(bytes)
    }
}

/// Inspect the fixed header before allocating a body buffer.
pub fn request_frame_len(header: &[u8]) -> Result<usize, Error> {
    if header.len() != REQUEST_HEADER_BYTES || &header[..5] != MAGIC { return Err(Error::InvalidInput); }
    let body = u32::from_be_bytes(header[5..9].try_into().map_err(|_| Error::InvalidInput)?) as usize;
    let total = REQUEST_HEADER_BYTES.checked_add(body).ok_or(Error::Limit)?;
    if total > MAX_HELPER_FRAME_BYTES { return Err(Error::Limit); }
    Ok(total)
}

pub fn encode_request(port: &HelperPort) -> Result<Vec<u8>, Error> {
    let request = port.request();
    let actual = request.view().actual_input();
    let profile = actual.input_profile();
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&request.round().to_be_bytes());
    out.extend_from_slice(request.evidence_root());
    out.extend_from_slice(&u16::try_from(request.member().len()).map_err(|_| Error::Limit)?.to_be_bytes());
    out.extend_from_slice(request.member().as_bytes());
    out.extend_from_slice(&(port.salt_limit as u16).to_be_bytes());
    for value in [profile.profile_id, profile.model_epoch, profile.tokenizer_epoch, profile.policy_epoch] {
        out.extend_from_slice(&value.to_be_bytes());
    }
    blob(&mut out, &profile.profile_bytes)?;
    blob(&mut out, actual.submitted_bytes())?;
    out.extend_from_slice(&(actual.ordered_parts().len() as u16).to_be_bytes());
    for part in actual.ordered_parts() {
        out.extend_from_slice(&u32::try_from(part.span.start).map_err(|_| Error::Limit)?.to_be_bytes());
        out.extend_from_slice(&u32::try_from(part.span.end).map_err(|_| Error::Limit)?.to_be_bytes());
        match part.kind {
            PartKind::Question => out.push(1),
            PartKind::Prompt => out.push(2),
            PartKind::Instruction => out.push(3),
            PartKind::ToolSchema { schema_id } => { out.push(4); out.extend_from_slice(&schema_id.to_be_bytes()); }
            PartKind::Evidence { source_id, transform_id } => {
                out.push(5); out.extend_from_slice(&source_id.to_be_bytes()); out.extend_from_slice(&transform_id.to_be_bytes());
            }
            PartKind::Delimiter => out.push(6),
            PartKind::Other => out.push(7),
        }
    }
    out.extend_from_slice(&(actual.omissions().len() as u16).to_be_bytes());
    for omission in actual.omissions() {
        match omission {
            Omission::ClosedAbsent { domain_id, trusted_closure_marker_id } => {
                out.extend_from_slice(&domain_id.to_be_bytes());
                out.extend_from_slice(&trusted_closure_marker_id.to_be_bytes());
            }
            _ => return Err(Error::Incomplete),
        }
    }
    if out.len() > MAX_HELPER_FRAME_BYTES { return Err(Error::Limit); }
    let length = u32::try_from(out.len() - REQUEST_HEADER_BYTES).map_err(|_| Error::Limit)?;
    out[5..9].copy_from_slice(&length.to_be_bytes());
    Ok(out)
}

pub fn decode_request(bytes: &[u8]) -> Result<WorkerInput, Error> {
    if bytes.len() > MAX_HELPER_FRAME_BYTES { return Err(Error::Limit); }
    let header = bytes.get(..REQUEST_HEADER_BYTES).ok_or(Error::Incomplete)?;
    if request_frame_len(header)? != bytes.len() { return Err(Error::InvalidInput); }
    let mut input = Reader { bytes: &bytes[REQUEST_HEADER_BYTES..], offset: 0 };
    let round = input.u64()?;
    let root = input.take(32)?.try_into().map_err(|_| Error::InvalidInput)?;
    let member_length = input.u16()? as usize;
    if member_length == 0 { return Err(Error::InvalidInput); }
    if member_length > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
    let member = std::str::from_utf8(input.take(member_length)?).map_err(|_| Error::InvalidInput)?.to_owned();
    let salt_limit = input.u16()? as usize;
    if salt_limit == 0 || salt_limit > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
    let profile_id = input.u64()?;
    let model_epoch = input.u64()?;
    let tokenizer_epoch = input.u64()?;
    let policy_epoch = input.u64()?;
    let profile_bytes = input.blob(MAX_PROFILE_BYTES)?.to_vec();
    let submitted = input.blob(MAX_SUBMITTED_BYTES)?.to_vec();
    let part_count = input.u16()? as usize;
    if part_count > MAX_SUBMITTED_PARTS { return Err(Error::Limit); }
    let mut parts = Vec::with_capacity(part_count);
    for _ in 0..part_count {
        let start = input.u32()? as usize;
        let end = input.u32()? as usize;
        let kind = match input.byte()? {
            1 => PartKind::Question, 2 => PartKind::Prompt, 3 => PartKind::Instruction,
            4 => PartKind::ToolSchema { schema_id: input.u64()? },
            5 => PartKind::Evidence { source_id: input.u64()?, transform_id: input.u64()? },
            6 => PartKind::Delimiter, 7 => PartKind::Other,
            _ => return Err(Error::InvalidInput),
        };
        parts.push(SubmittedPart { span: ByteSpan { start, end }, kind });
    }
    let omission_count = input.u16()? as usize;
    if omission_count > MAX_OMISSIONS { return Err(Error::Limit); }
    let mut omissions = Vec::with_capacity(omission_count);
    for _ in 0..omission_count {
        omissions.push(Omission::ClosedAbsent {
            domain_id: input.u64()?, trusted_closure_marker_id: input.u64()?,
        });
    }
    if input.offset != input.bytes.len() { return Err(Error::InvalidInput); }
    let actual = ActualHelperInput::new(submitted, InputProfileBinding {
        profile_id, profile_bytes, model_epoch, tokenizer_epoch, policy_epoch,
    }, parts, omissions)?;
    Ok(WorkerInput { round, member, root, actual, salt_limit })
}

pub(super) fn decode_commitment(bytes: &[u8]) -> Result<Digest, Error> {
    if bytes.len() != 9 || bytes[0] != b'C' { return Err(Error::InvalidInput); }
    Ok(u64::from_be_bytes(bytes[1..].try_into().map_err(|_| Error::InvalidInput)?))
}

pub(super) fn reveal_frame_len(header: &[u8], salt_limit: usize) -> Result<usize, Error> {
    if header.len() != 4 || header[0] != b'R' { return Err(Error::InvalidInput); }
    verdict_from_tag(header[1])?;
    let salt = u16::from_be_bytes([header[2], header[3]]) as usize;
    if salt > salt_limit || salt > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
    Ok(4 + salt)
}

pub(super) fn decode_reveal(bytes: &[u8], salt_limit: usize) -> Result<(Verdict, &[u8]), Error> {
    let header = bytes.get(..4).ok_or(Error::Incomplete)?;
    if reveal_frame_len(header, salt_limit)? != bytes.len() { return Err(Error::InvalidInput); }
    Ok((verdict_from_tag(bytes[1])?, &bytes[4..]))
}

fn verdict_tag(verdict: Verdict) -> u8 {
    match verdict { Verdict::Allow => 1, Verdict::Hold => 2, Verdict::Deny => 3, Verdict::Abstain => 4 }
}
fn verdict_from_tag(tag: u8) -> Result<Verdict, Error> {
    match tag { 1 => Ok(Verdict::Allow), 2 => Ok(Verdict::Hold), 3 => Ok(Verdict::Deny),
        4 => Ok(Verdict::Abstain), _ => Err(Error::InvalidInput) }
}
fn blob(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    out.extend_from_slice(&u32::try_from(bytes.len()).map_err(|_| Error::Limit)?.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Limit)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, Error> { Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| Error::InvalidInput)?)) }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::InvalidInput)?)) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::InvalidInput)?)) }
    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], Error> {
        let length = self.u32()? as usize;
        if length > maximum { return Err(Error::Limit); }
        self.take(length)
    }
}
