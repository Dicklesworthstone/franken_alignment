//! Bounded, lossless presentation of a native human request, never an approval
//! capability. The original journal's action primitives and view codec are reused.
//! This is a plaintext reference protocol for a preauthenticated private channel.
use super::super::{FileHumanRequest, views};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, MAX_PAYLOAD_BYTES};
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::evidence_view::EvidenceViewManifest;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_REVIEW_BYTES: usize = 16 * 1024 * 1024;
pub const OFFER_HEADER_BYTES: usize = 16;
pub const DECISION_BYTES: usize = 73;
pub const RECEIPT_BYTES: usize = 81;
const OFFER: &[u8; 8] = b"FAHRVW\0\x01";
const DECISION: &[u8; 8] = b"FAHRDC\0\x01";
const RECEIPT: &[u8; 8] = b"FAHRRC\0\x01";

/// Exact correlation data only. The host must never reuse a nonce for a
/// different offer. A nonce is not authentication or a cryptographic commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewBinding {
    pub session: [u8; 32],
    pub request: u64,
    pub reviewer: u64,
    pub attempt: u64,
    pub offer_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDecision { Approve, Reject, Revoke }
impl ReviewDecision {
    fn tag(self) -> u8 { match self { Self::Approve => 1, Self::Reject => 2, Self::Revoke => 3 } }
    fn decode(tag: u8) -> Result<Self, Error> {
        match tag { 1 => Ok(Self::Approve), 2 => Ok(Self::Reject), 3 => Ok(Self::Revoke), _ => Err(Error::InvalidInput) }
    }
}

/// Read-only presentation. There is no decoded HumanRequest, reviewer role,
/// automatic permit or human key. UI callers must escape untrusted evidence and
/// payload text; decoding does not choose an answer or establish human identity.
#[derive(Clone, PartialEq, Eq)]
pub struct ReviewPacket {
    binding: ReviewBinding,
    clock_domain: u64,
    control_sequence: u64,
    input_revision: u64,
    policy_generation: u64,
    created_at: ElapsedTick,
    expires_at: ElapsedTick,
    disposition: HumanDisposition,
    action: FrozenAction,
    views: BTreeMap<String, EvidenceViewManifest>,
}
impl fmt::Debug for ReviewPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewPacket").field("binding", &self.binding)
            .field("members", &self.views.len()).field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}
impl ReviewPacket {
    pub fn binding(&self) -> ReviewBinding { self.binding }
    pub fn clock_domain(&self) -> u64 { self.clock_domain }
    pub fn control_sequence(&self) -> u64 { self.control_sequence }
    pub fn input_revision(&self) -> u64 { self.input_revision }
    pub fn policy_generation(&self) -> u64 { self.policy_generation }
    pub fn created_at(&self) -> ElapsedTick { self.created_at }
    pub fn expires_at(&self) -> ElapsedTick { self.expires_at }
    /// Recorded at offer creation, not a current approval or expiry guarantee.
    pub fn disposition(&self) -> HumanDisposition { self.disposition }
    pub fn action(&self) -> &FrozenAction { &self.action }
    pub fn views(&self) -> &BTreeMap<String, EvidenceViewManifest> { &self.views }

    pub(super) fn capture(request: &FileHumanRequest, clock_domain: u64, revision: u64,
        disposition: HumanDisposition, session: [u8; 32]) -> Result<Self, Error>
    {
        let evidence = request.evidence();
        let packet = Self {
            binding: ReviewBinding { session, request: evidence.id(), reviewer: evidence.reviewer_id(),
                attempt: evidence.attempt(), offer_revision: revision },
            clock_domain, control_sequence: evidence.control_sequence(), input_revision: evidence.input_revision(),
            policy_generation: evidence.policy_generation(), created_at: evidence.created_at(),
            expires_at: evidence.expires_at(), disposition, action: evidence.action().clone(),
            views: evidence.inputs().views().clone(),
        };
        packet.validate()?;
        Ok(packet)
    }
    fn validate(&self) -> Result<(), Error> {
        if self.binding.session == [0; 32] || self.binding.request == 0 || self.binding.reviewer == 0
            || self.binding.attempt == 0 || self.clock_domain == 0 || self.input_revision == 0
            || self.expires_at <= self.created_at || self.expires_at > self.action.spec().deadline
            || self.views.is_empty() { return Err(Error::InvalidInput); }
        if !matches!(self.disposition, HumanDisposition::Pending | HumanDisposition::Approved) {
            return Err(Error::WrongState);
        }
        // Exactly the current durable profile: it admits no private required
        // witnesses. Never silently omit unsupported fields in a presentation.
        if !self.action.spec().required_witnesses.is_empty() { return Err(Error::Binding); }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut w = Writer::new(MAX_REVIEW_BYTES);
        w.raw(OFFER)?; w.u64(0)?;
        for value in [self.binding.request, self.binding.reviewer, self.binding.attempt,
            self.binding.offer_revision, self.clock_domain, self.control_sequence,
            self.input_revision, self.policy_generation, self.created_at.0, self.expires_at.0] { w.u64(value)?; }
        w.u8(if self.disposition == HumanDisposition::Pending { 0 } else { 1 })?;
        let action = self.action.spec();
        w.u32(action.version)?; w.scope(action.scope)?;
        w.target(action.target.ok_or(Error::Incomplete)?)?; w.blob(&action.payload)?;
        w.u64(action.policy_epoch)?; w.u64(action.deadline.0)?; w.u64(action.units)?;
        views::write(&mut w, &self.views)?;
        // The correlation nonce comes AFTER all evidence, never instead of it.
        w.raw(&self.binding.session)?;
        let mut bytes = w.finish();
        let length = u64::try_from(bytes.len()).map_err(|_| Error::Limit)?;
        bytes[8..16].copy_from_slice(&length.to_be_bytes());
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let length = offer_frame_len(bytes.get(..OFFER_HEADER_BYTES).ok_or(Error::Incomplete)?)?;
        if bytes.len() != length { return Err(Error::InvalidInput); }
        let mut r = Reader::new(&bytes[OFFER_HEADER_BYTES..]);
        let request = r.u64()?; let reviewer = r.u64()?; let attempt = r.u64()?; let offer_revision = r.u64()?;
        let clock_domain = r.u64()?; let control_sequence = r.u64()?; let input_revision = r.u64()?;
        let policy_generation = r.u64()?; let created_at = ElapsedTick(r.u64()?); let expires_at = ElapsedTick(r.u64()?);
        let disposition = match r.u8()? { 0 => HumanDisposition::Pending, 1 => HumanDisposition::Approved,
            _ => return Err(Error::InvalidInput) };
        let version = r.u32()?; let scope = r.scope()?; let target = Some(r.target()?);
        let payload = r.blob(MAX_PAYLOAD_BYTES)?.to_vec();
        let action = FrozenAction::freeze(ActionSpec { version, scope, target, payload,
            required_witnesses: Vec::new(), policy_epoch: r.u64()?, deadline: ElapsedTick(r.u64()?), units: r.u64()? })?;
        let views = views::read(&mut r)?;
        let session = r.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
        r.end()?;
        let packet = Self { binding: ReviewBinding { session, request, reviewer, attempt, offer_revision },
            clock_domain, control_sequence, input_revision, policy_generation, created_at, expires_at,
            disposition, action, views };
        packet.validate()?;
        // Original view ordering and metadata validation must remain lossless.
        if packet.encode()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(packet)
    }
    /// Encoding an explicit UI decision is not evidence that it was received,
    /// persisted or approved. No decision exists until the caller supplies one.
    pub fn decision_frame(&self, decision: ReviewDecision) -> [u8; DECISION_BYTES] {
        let mut bytes = [0; DECISION_BYTES];
        bytes[..8].copy_from_slice(DECISION);
        write_binding(&mut bytes[8..72], self.binding);
        bytes[72] = decision.tag();
        bytes
    }
    pub fn receipt(&self, bytes: &[u8]) -> Result<ReviewReceipt, Error> {
        let receipt = ReviewReceipt::decode(bytes)?;
        if receipt.binding != self.binding { return Err(Error::Binding); }
        Ok(receipt)
    }
}

/// Check before allocating the body. The declared length is the COMPLETE frame.
pub fn offer_frame_len(header: &[u8]) -> Result<usize, Error> {
    if header.len() != OFFER_HEADER_BYTES || &header[..8] != OFFER { return Err(Error::InvalidInput); }
    let count = u64::from_be_bytes(header[8..16].try_into().map_err(|_| Error::Incomplete)?);
    let count = usize::try_from(count).map_err(|_| Error::Limit)?;
    if count > MAX_REVIEW_BYTES { return Err(Error::Limit); }
    if count <= OFFER_HEADER_BYTES { return Err(Error::InvalidInput); }
    Ok(count)
}

pub(super) fn decode_decision(bytes: &[u8], expected: ReviewBinding) -> Result<ReviewDecision, Error> {
    if bytes.len() != DECISION_BYTES || &bytes[..8] != DECISION { return Err(Error::InvalidInput); }
    if read_binding(&bytes[8..72])? != expected { return Err(Error::Binding); }
    ReviewDecision::decode(bytes[72])
}

/// A historical committed reviewer operation, NOT a transferable second key and
/// NOT an acknowledgment that dispatch or publication occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewReceipt {
    pub binding: ReviewBinding,
    pub decision: ReviewDecision,
    pub revision: u64,
}
impl ReviewReceipt {
    pub(super) fn encode(self) -> [u8; RECEIPT_BYTES] {
        let mut bytes = [0; RECEIPT_BYTES];
        bytes[..8].copy_from_slice(RECEIPT);
        write_binding(&mut bytes[8..72], self.binding);
        bytes[72] = self.decision.tag();
        bytes[73..].copy_from_slice(&self.revision.to_be_bytes());
        bytes
    }
    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != RECEIPT_BYTES || &bytes[..8] != RECEIPT { return Err(Error::InvalidInput); }
        let receipt = Self { binding: read_binding(&bytes[8..72])?, decision: ReviewDecision::decode(bytes[72])?,
            revision: u64::from_be_bytes(bytes[73..].try_into().map_err(|_| Error::Incomplete)?) };
        if receipt.revision <= receipt.binding.offer_revision { return Err(Error::Stale); }
        Ok(receipt)
    }
}
fn write_binding(bytes: &mut [u8], binding: ReviewBinding) {
    bytes[..32].copy_from_slice(&binding.session);
    for (chunk, value) in bytes[32..].chunks_exact_mut(8).zip([
        binding.request, binding.reviewer, binding.attempt, binding.offer_revision,
    ]) { chunk.copy_from_slice(&value.to_be_bytes()); }
}
fn read_binding(bytes: &[u8]) -> Result<ReviewBinding, Error> {
    let mut r = Reader::new(bytes);
    let binding = ReviewBinding { session: r.take(32)?.try_into().map_err(|_| Error::Incomplete)?,
        request: r.u64()?, reviewer: r.u64()?, attempt: r.u64()?, offer_revision: r.u64()? };
    r.end()?;
    Ok(binding)
}
