//! Inspect source, admission and disclosure from ONE original canonical image.
use super::{Event, FileOversight, FileRequestStatus, FileTextMessageRequest, JournalError, Machine};
use super::super::{FileStreamSnapshot, StreamProfile, check_contract};
use super::super::super::{FileOversightProfile, journal, storage};
use super::super::super::decoder::{FileDecoderConfig,
    text::{MAX_FILE_TOKENIZER_BYTES, replay_text_events}};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    text::TextGenerationReport, tokenizer::ByteBpe,
};
use crate::Error;
use std::path::Path;

/// Historical source, original request disposition and the enclosing stream cut.
/// Published and receipt-confirmed stream prefixes remain distinct. An admitted
/// source is not evidence that the message was dispatched or seen by a recipient.
/// A readable canonical image may be visible after an unacknowledged replacement.
/// No live owner, helper role, human key, permit or fresh observation is returned.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageSnapshot;
/// fn publish(snapshot: FileTextMessageSnapshot) -> Permit { snapshot }
/// ```
#[derive(Clone, Debug)]
pub struct FileTextMessageSnapshot {
    pub source: FileTextMessageRequest,
    pub status: FileRequestStatus,
    pub generation: TextGenerationReport,
    pub stream: FileStreamSnapshot,
}

impl FileOversight {
    /// Inspect acknowledged provenance without replaying the live model or
    /// changing authority. Later inference, cancellation, fences and loss of
    /// clock/source readiness do not erase the earlier historical association.
    /// A faulted owner refuses; use the explicit canonical-image reader below.
    pub fn decoder_text_message_snapshot(&self, request: u64)
        -> Result<FileTextMessageSnapshot, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        extract(&self.machine, &self.events, request)
    }

    /// Resolve a lost reply or inspect history beside a locked/faulted owner.
    /// Read one bounded canonical file, pin the independently supplied stream,
    /// model/monitor/sampler and tokenizer BEFORE numerical replay, then validate
    /// the ENTIRE history before extracting source and disposition together.
    ///
    /// The existing text validator and original numerical/authority reducers do
    /// the work. No second file read can mix source from one cut with status from
    /// another. No writer lock, cleanup, fence, source/clock callback, submission
    /// retry or role creation occurs. Missing means no source-linked request in
    /// THIS valid image, not proof that no physical work or remote effect occurred.
    ///
    /// This is a read-only local reference profile, not an authenticated head or
    /// anti-rollback guarantee. It cannot replace the existing composed anchored
    /// recovery APIs for creating an independently constrained writable owner.
    pub fn read_decoder_text_message(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileDecoderConfig, tokenizer: &ByteBpe, stream: StreamProfile, request: u64)
        -> Result<FileTextMessageSnapshot, JournalError>
    {
        if request == 0 { return Err(Error::InvalidInput.into()); }
        profile.delivery.limits.check()?;
        if !tokenizer.binds(expected.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        check_contract(&events, stream)?;
        let machine = replay_text_events(profile, expected, &canonical, &events)?;
        extract(&machine, &events, request)
    }
}

fn extract(machine: &Machine, events: &[Event], request: u64)
    -> Result<FileTextMessageSnapshot, JournalError>
{
    if request == 0 { return Err(Error::InvalidInput.into()); }
    let source = events.iter().find_map(|event| match event {
        Event::TextMessage(source, _) if source.request == request => Some(source.as_ref()),
        _ => None,
    }).ok_or(Error::Missing)?;
    let status = machine.requests.status(request)?;
    let command = machine.decoder_text_command(source.generation).ok_or(Error::Missing)?;
    let progress = machine.decoder_generation_progress(source.generation)?;
    if progress.generation_revision() != source.generation_revision { return Err(Error::Binding.into()); }
    let receipt = progress.receipt().ok_or(Error::Incomplete)?;
    let (prepared, expected) = command.compile(machine.decoder_tokenizer()?)?;
    if receipt.command() != &expected { return Err(Error::Binding.into()); }
    let generation = prepared.finish(receipt.result()?.clone());
    // Interpretation failure is explicit, never an empty successful message.
    generation.bytes()?;
    Ok(FileTextMessageSnapshot { source: source.clone(), status, generation,
        stream: machine.stream_snapshot(events.len())? })
}
