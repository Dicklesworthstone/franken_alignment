//! Action-scoped producer observations and durable acquire-before-use cycles.
//! A concrete bounded file reader supplies bytes; producer identity and atomic
//! replacement discipline remain operator assumptions, not authentication.
mod file;
pub mod completion;
pub mod heartbeat;
pub use file::{FileCaptureError, PublicationInputFile};
use super::witnesses::{FilePublicationEvidence, FilePublicationInputs, MAX_PUBLICATION_PACKET_BYTES};
use super::witness_gate::WitnessEvent;
use super::super::{Event, FileOversight, JournalError, JournalFailure, JournalIo, Machine, Transition, journal};
use crate::action::FrozenAction;
use crate::action::consequence::delivery::publication_gate::PublicationSourceStatus;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::action_frame;
use crate::witness::WitnessRequest;
use crate::Error;
use std::io;
use std::rc::Rc;

pub const MAX_CAPTURE_BYTES: usize = 4 * 1_048_576;
const DOMAIN: &[u8; 8] = b"FAPCAP01";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCaptureIdentity {
    pub source: u64,
    pub generation: u64,
}

/// Immutable producer interchange. Neither this packet nor its identity is a
/// permission: the durable owner binds its attempt and COMPLETE original action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePublicationCapture {
    attempt: u64,
    identity: FileCaptureIdentity,
    frame: Vec<u8>,
    inputs: FilePublicationInputs,
}
impl FilePublicationCapture {
    pub fn new(attempt: u64, identity: FileCaptureIdentity, action: &FrozenAction,
        inputs: FilePublicationInputs) -> Result<Self, Error>
    {
        check_identity(identity)?;
        let capture = Self { attempt, identity, frame: action_frame(action), inputs };
        capture.to_bytes()?;
        Ok(capture)
    }
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn identity(&self) -> FileCaptureIdentity { self.identity }
    pub fn inputs(&self) -> &FilePublicationInputs { &self.inputs }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_CAPTURE_BYTES);
        w.raw(DOMAIN)?; w.u64(self.attempt)?;
        write_identity(&mut w, self.identity)?;
        w.blob(&self.frame)?; w.blob(&self.inputs.to_bytes()?)?;
        Ok(w.finish())
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_CAPTURE_BYTES { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
        let attempt = r.u64()?;
        let identity = read_identity(&mut r)?;
        let frame = r.blob(MAX_CAPTURE_BYTES)?.to_vec();
        let inputs = FilePublicationInputs::from_bytes(r.blob(MAX_PUBLICATION_PACKET_BYTES)?)?;
        r.end()?;
        let capture = Self { attempt, identity, frame, inputs };
        if capture.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(capture)
    }
    pub(in super::super) fn check_action(&self, attempt: u64, action: &FrozenAction) -> Result<(), Error> {
        if self.attempt != attempt || self.frame != action_frame(action) { return Err(Error::Binding); }
        Ok(())
    }
}

#[derive(Clone)]
pub(in super::super) struct SourceBinding {
    attempt: u64,
    identity: FileCaptureIdentity,
    frame: Vec<u8>,
    pub(in super::super) evidence: FilePublicationEvidence,
}
impl SourceBinding {
    fn new(capture: FilePublicationCapture, requests: Vec<WitnessRequest>) -> Result<Self, Error> {
        Ok(Self { attempt: capture.attempt, identity: capture.identity, frame: capture.frame,
            evidence: FilePublicationEvidence::new(capture.inputs, requests)? })
    }
    pub(in super::super) fn identity(&self) -> FileCaptureIdentity { self.identity }
    pub(in super::super) fn check_action(&self, attempt: u64, action: &FrozenAction) -> Result<(), Error> {
        if self.attempt != attempt || self.frame != action_frame(action) { return Err(Error::Binding); }
        Ok(())
    }
    pub(in super::super) fn write(&self, w: &mut Writer) -> Result<(), Error> {
        w.u64(self.attempt)?; write_identity(w, self.identity)?; w.blob(&self.frame)?;
        w.blob(&self.evidence.to_bytes()?)
    }
    pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self { attempt: r.u64()?, identity: read_identity(r)?, frame: r.blob(MAX_CAPTURE_BYTES)?.to_vec(),
            evidence: FilePublicationEvidence::from_bytes(r.blob(MAX_PUBLICATION_PACKET_BYTES)?)? })
    }
}
fn check_identity(identity: FileCaptureIdentity) -> Result<(), Error> {
    if identity.source == 0 || identity.generation == 0 { return Err(Error::InvalidInput); }
    Ok(())
}
fn write_identity(w: &mut Writer, identity: FileCaptureIdentity) -> Result<(), Error> {
    check_identity(identity)?; w.u64(identity.source)?; w.u64(identity.generation)
}
fn read_identity(r: &mut Reader<'_>) -> Result<FileCaptureIdentity, Error> {
    let identity = FileCaptureIdentity { source: r.u64()?, generation: r.u64()? };
    check_identity(identity)?; Ok(identity)
}

impl FileOversight {
    /// Bind original reviewed requirements AND their producer in one journal
    /// transaction. Neither step can survive without the other. The original
    /// capture does not count as a current read for authorization.
    pub fn bind_publication_file_source(&mut self, revision: u64, attempt: u64,
        original: FilePublicationCapture, requests: Vec<WitnessRequest>) -> Result<(), JournalError>
    {
        original.check_action(attempt, self.machine.actions.get(&attempt).ok_or(Error::Missing)?)?;
        let binding = SourceBinding::new(original, requests)?;
        self.transact(revision, Event::PublicationWitness(WitnessEvent::SourceBind(attempt, Rc::new(binding))))?;
        Ok(())
    }

    pub fn publication_source(&self, attempt: u64) -> Result<Option<PublicationSourceStatus>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_source(attempt)?)
    }

    /// Withdraw current eligibility DURABLY before opening the file. An I/O or
    /// decoding failure returns an inner error with unavailable inputs retained.
    /// A persistence/replay failure returns an outer error; no candidate result
    /// is usable. There is no caller-supplied positive-input fallback on this path.
    pub fn refresh_publication_from_file(&mut self, revision: u64, attempt: u64,
        source: &PublicationInputFile) -> Result<Result<FileCaptureIdentity, FileCaptureError>, JournalError>
    {
        let expected = self.begin_publication_capture(revision, attempt, source.source())?;
        let capture = match source.read_capture() { Ok(capture) => capture, Err(error) => return Ok(Err(error)) };
        let identity = capture.identity();
        self.finish_publication_capture(attempt, expected, capture)?;
        Ok(Ok(identity))
    }

    // The driver uses the same two operations around BOTH provider reads, so a
    // caught unwind from committee capture cannot preserve old witness inputs.
    pub(in super::super) fn begin_publication_capture(&mut self, revision: u64,
        attempt: u64, source: u64) -> Result<u64, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let retained = self.machine.broker.publication_source(attempt)?.ok_or(Error::Incomplete)?;
        if retained.source != source { return Err(Error::Binding.into()); }
        let expected = self.publication_input_revision(attempt)?;
        self.record_publication_inputs(revision, attempt, expected, None)
    }

    pub(in super::super) fn finish_publication_capture(&mut self, attempt: u64,
        expected: u64, capture: FilePublicationCapture) -> Result<u64, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.machine.broker.publication_input_revision(attempt)? != expected { return Err(Error::Stale.into()); }
        let event = Event::PublicationWitness(WitnessEvent::Captured(attempt, expected, Rc::new(capture)));
        self.check_source_admission(&event)?;
        // A malformed/regressing observation cannot be forgotten in favor of a
        // prior quiet producer generation. Recovery fences every old effect key.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        match self.persist_candidate(event, bytes, candidate, result)? {
            Transition::Inputs(revision) => Ok(revision),
            _ => unreachable!("captured publication inputs"),
        }
    }
}
