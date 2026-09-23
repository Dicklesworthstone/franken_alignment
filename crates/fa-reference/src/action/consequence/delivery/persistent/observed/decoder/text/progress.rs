//! Resumable byte output over ORIGINAL durable generation progress.
//! Offsets describe retained evidence, not transport acknowledgment or permission.
mod cancellation;

use super::{ByteBpe, FileDecoderConfig, FileOversight, FileOversightProfile,
    FileTextGenerationCommand, JournalError, Machine, PreparedText, MAX_FILE_TOKENIZER_BYTES,
    replay_text_history, storage};
use super::super::{FileDecoderInspection, progress::FileGenerationProgress};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, text::TextOutputError,
};
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::Error;
use std::fmt;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

/// An immutable cumulative projection, never a fabricated completed report.
/// The native progress retains pending/refused/held/failed distinctions, original
/// IDs, spent budgets and generation revision. Only released IDs become bytes.
/// A live-owner result follows acknowledged storage; a read-only canonical
/// snapshot below can describe a visible but unacknowledged replacement.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::progress::FileTextGenerationProgress;
/// fn grant(progress: FileTextGenerationProgress) -> Permit { progress }
/// ```
#[derive(Clone)]
pub struct FileTextGenerationProgress {
    command: Rc<FileTextGenerationCommand>,
    numerical: FileGenerationProgress,
    output: Result<Vec<u8>, Error>,
}
impl FileTextGenerationProgress {
    pub fn command(&self) -> &FileTextGenerationCommand { &self.command }
    pub fn numerical(&self) -> &FileGenerationProgress { &self.numerical }
    pub fn generation_revision(&self) -> u64 { self.numerical.generation_revision() }
    pub fn is_complete(&self) -> bool { self.numerical.is_complete() }
    pub fn finish(&self) -> Option<Result<GenerationFinish, Error>> { self.numerical.finish() }
    pub fn bytes(&self) -> Result<&[u8], Error> {
        self.output.as_deref().map_err(|error| *error)
    }
    /// Strict whole-prefix interpretation. Split or invalid UTF-8 remains exact
    /// bytes; neither a stop token nor a held computation is decoded as text.
    pub fn utf8(&self) -> Result<&str, TextOutputError> {
        std::str::from_utf8(self.bytes().map_err(TextOutputError::Contract)?)
            .map_err(TextOutputError::InvalidUtf8)
    }

    /// Read from the caller's retained BYTE cursor, not a token count or journal
    /// revision. Old generation retries return current cumulative progress, so
    /// append only this suffix after retaining its end for THIS request.
    /// A cursor ahead of this snapshot refuses, including usize::MAX. A cursor
    /// at its end returns an empty range. A boundary inside a UTF-8 character or
    /// multi-byte token is still a valid byte cursor; no normalization occurs.
    /// The caller owns durable acknowledgment and delivery to any external sink.
    pub fn delta_from(&self, byte_offset: usize) -> Result<FileTextDelta<'_>, Error> {
        let complete = self.bytes()?;
        let bytes = complete.get(byte_offset..).ok_or(Error::Stale)?;
        Ok(FileTextDelta { request: self.command.id(),
            generation_revision: self.generation_revision(),
            byte_range: byte_offset..complete.len(), bytes })
    }

    fn from_prepared(command: Rc<FileTextGenerationCommand>, prepared: PreparedText,
        numerical: FileGenerationProgress) -> Self
    {
        // The native convenience tokens() is empty for an admission refusal.
        // Preserve that refusal instead of manufacturing an empty text success.
        // Even an unexpected decoding failure keeps the acknowledged native
        // progress inspectable rather than losing the committed work counters.
        let output = match numerical.finish() {
            Some(Err(error)) => Err(error),
            _ => prepared.decode_output(numerical.tokens()),
        };
        Self { command, numerical, output }
    }
}
impl fmt::Debug for FileTextGenerationProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTextGenerationProgress").field("command", &self.command)
            .field("generation_revision", &self.generation_revision())
            .field("finish", &self.finish())
            .field("output_bytes", &self.output.as_ref().map(Vec::len)).finish_non_exhaustive()
    }
}

/// Borrowed bytes from one immutable original-request projection. They may span
/// several missed responses, but cannot include an unacknowledged next token.
/// This contains neither a live owner nor an actor/publication capability.
pub struct FileTextDelta<'a> {
    request: u64,
    generation_revision: u64,
    byte_range: Range<usize>,
    bytes: &'a [u8],
}
impl FileTextDelta<'_> {
    pub fn request(&self) -> u64 { self.request }
    pub fn generation_revision(&self) -> u64 { self.generation_revision }
    pub fn byte_range(&self) -> Range<usize> { self.byte_range.clone() }
    pub fn bytes(&self) -> &[u8] { self.bytes }
}
impl fmt::Debug for FileTextDelta<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTextDelta").field("request", &self.request)
            .field("generation_revision", &self.generation_revision)
            .field("byte_range", &self.byte_range).finish_non_exhaustive()
    }
}

/// Read-only canonical-image data, not a writable recovered owner. Its complete
/// enclosing image may be newer than the requested generation and may reflect
/// an unacknowledged directory-sync cut. No fresh time, lock or role is created.
#[derive(Clone, Debug)]
pub struct FileTextProgressSnapshot {
    pub text: FileTextGenerationProgress,
    pub publication: FileDeliverySnapshot,
    pub numerical: FileDecoderInspection,
}

impl FileOversight {
    /// Discover a pending TEXT intent without a separately retained request-ID
    /// list. A pending bare-ID command returns None here; consult the original
    /// pending_decoder_generation to distinguish that case from no pending work.
    /// This is the original complete input, never a resumed cursor or effect key.
    pub fn pending_decoder_text(&self) -> Result<Option<FileTextGenerationCommand>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let Some(pending) = self.machine.pending_decoder_generation() else { return Ok(None); };
        Ok(self.machine.decoder_text_command(pending.id()).map(|command| command.as_ref().clone()))
    }

    /// Read only acknowledged native progress and decode its released IDs under
    /// the exact journal-bound tokenizer. No inference, source read or clock runs.
    pub fn decoder_text_progress(&self, id: u64) -> Result<FileTextGenerationProgress, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        project_progress(&self.machine, id)
    }

    /// Advance at most ONE original monitored token, then expose the cumulative
    /// byte projection only after its original witness/result cut is acknowledged.
    /// begin_decoder_text must have frozen the input already; this accepts no
    /// prompt, tokenizer, stop override, cursor import, extra budget or new seed.
    ///
    /// An old generation revision reads CURRENT retained progress, never reruns
    /// a token. New work needs the current journal revision, unpaused owner and
    /// original source/clock admission. After reopen, fresh time and explicit
    /// resume preserve the reconstructed cursor and its cumulative budget.
    /// Output storage is admitted through shared text preparation BEFORE calling
    /// the original advance_decoder_generation. Errors after native commitment
    /// remain in the returned projection with its native progress, not a rewind.
    pub fn advance_decoder_text(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64) -> Result<FileTextGenerationProgress, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let command = Rc::clone(self.machine.decoder_text_command(id).ok_or(Error::Missing)?);
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if !current.is_complete() && generation_revision == current.generation_revision()
            && journal_revision != self.revision() { return Err(Error::Stale.into()); }
        let (prepared, numerical_command) = command.compile(self.machine.decoder_tokenizer()?)?;
        if current.command() != &numerical_command { return Err(Error::Binding.into()); }
        let next = self.advance_decoder_generation(journal_revision, id, generation_revision)?;
        Ok(FileTextGenerationProgress::from_prepared(command, prepared, next))
    }

    /// Inspect a complete canonical image with exact independently retained
    /// model/monitor/sampler AND tokenizer bytes checked before numerical replay.
    /// This never cleans the store, fences a key, resumes generation or creates a
    /// reviewer. It can inspect the actual disk cut after a live owner's fault.
    /// The returned bytes are evidence of this image, not storage acknowledgment
    /// or permission to deliver them outside the control boundary.
    pub fn read_decoder_text_progress(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileDecoderConfig, tokenizer: &ByteBpe, id: u64)
        -> Result<FileTextProgressSnapshot, JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(expected.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = replay_text_history(profile, expected, &canonical, &identity, &bytes)?;
        Ok(FileTextProgressSnapshot {
            text: project_progress(&machine, id)?,
            publication: machine.snapshot(events.len()),
            numerical: FileDecoderInspection { journal_revision: events.len() as u64,
                paused: machine.decoder_paused(), numerical: machine.broker.hosted_decoder()? },
        })
    }
}

fn project_progress(machine: &Machine, id: u64) -> Result<FileTextGenerationProgress, JournalError> {
    let command = Rc::clone(machine.decoder_text_command(id).ok_or(Error::Missing)?);
    let numerical = machine.decoder_generation_progress(id)?;
    let (prepared, expected) = command.compile(machine.decoder_tokenizer()?)?;
    if numerical.command() != &expected { return Err(Error::Binding.into()); }
    Ok(FileTextGenerationProgress::from_prepared(command, prepared, numerical))
}
