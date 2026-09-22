//! Byte-exact text requests through the ORIGINAL durable monitored decoder.
//! Tokenization is input preparation, never an alternative inference/permit path.
mod codec;
pub mod progress;
pub(super) use codec::{read, write};
#[cfg(test)]
mod tests;

use super::{DecoderEvent, FileDecoderConfig};
use super::generation::{FileGenerationCommand, FileGenerationReceipt};
use super::super::{BaseEvent, Event, FileOversight, FileOversightProfile, FileHumanReviewer,
    JournalError, Machine, journal, storage};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_GENERATION_TOKENS, MAX_SAMPLING_ENTRIES, MAX_STOP_TOKENS,
    text::{PreparedText, TextGenerationReport, TextGenerationRequest, MAX_PREFIX_CONTROLS,
        prepare_text_request},
    tokenizer::{ByteBpe, TokenizationWork, MAX_DECODE_BYTES, MAX_HEAP_POPS, MAX_INPUT_BYTES,
        MAX_PAIR_LOOKUPS},
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::Error;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

/// This durable profile is deliberately smaller than the native archive limit.
pub const MAX_FILE_TOKENIZER_BYTES: usize = 8 * 1024 * 1024;
/// Original prompt/control/stop bytes retained across ALL text intents, including
/// completed, held and refused generations. Not an allocator/physical-work cap.
pub const MAX_FILE_TEXT_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Immutable supervisor input. Every field participates in exact retry equality;
/// even unused output/tokenization allowances cannot be changed under this ID.
#[derive(Clone, PartialEq, Eq)]
pub struct FileTextGenerationCommand {
    id: u64,
    actor_revision: u64,
    position: u64,
    request: TextGenerationRequest,
}
impl FileTextGenerationCommand {
    pub fn new(id: u64, actor_revision: u64, position: u64, request: TextGenerationRequest)
        -> Result<Self, Error>
    {
        let command = Self { id, actor_revision, position, request };
        command.check()?;
        Ok(command)
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn position(&self) -> u64 { self.position }
    pub fn request(&self) -> &TextGenerationRequest { &self.request }

    pub(crate) fn check(&self) -> Result<usize, Error> {
        if self.id == 0 { return Err(Error::InvalidInput); }
        let r = &self.request;
        if r.prompt.len() > MAX_INPUT_BYTES || r.prompt.len() > r.tokenization.input_bytes
            || r.tokenization.input_bytes > MAX_INPUT_BYTES
            || r.tokenization.pair_lookups > MAX_PAIR_LOOKUPS || r.tokenization.heap_pops > MAX_HEAP_POPS
            || r.max_new_tokens > MAX_GENERATION_TOKENS || r.stop_tokens.len() > MAX_STOP_TOKENS
            || r.prefix_controls.len() > MAX_PREFIX_CONTROLS || r.max_output_bytes > MAX_DECODE_BYTES
            || r.generation.scalar_products > MAX_DECODER_PRODUCTS
            || r.generation.sampling_entries > MAX_SAMPLING_ENTRIES
        { return Err(Error::Limit); }
        let ids = r.prefix_controls.len().checked_add(r.stop_tokens.len()).ok_or(Error::Limit)?;
        r.prompt.len().checked_add(ids.checked_mul(4).ok_or(Error::Limit)?).ok_or(Error::Limit)
    }

    pub(crate) fn compile(&self, tokenizer: &ByteBpe)
        -> Result<(PreparedText, FileGenerationCommand), Error>
    {
        self.check()?;
        // The same admission as TextDecoder, including complete prompt coverage,
        // control classification and output allocation BEFORE numerical work.
        let prepared = prepare_text_request(tokenizer, self.request.clone(), &mut TokenizationWork::default())?;
        let numerical = FileGenerationCommand::new(self.id, self.actor_revision,
            self.position, prepared.numerical.clone())?;
        Ok((prepared, numerical))
    }
}
impl fmt::Debug for FileTextGenerationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTextGenerationCommand").field("id", &self.id)
            .field("actor_revision", &self.actor_revision).field("position", &self.position)
            .field("prompt_bytes", &self.request.prompt.len())
            .field("max_new_tokens", &self.request.max_new_tokens).finish_non_exhaustive()
    }
}

/// Historical supervisor observations, not a publication or approval. The
/// original numerical receipt remains inspectable even if its result refused.
/// Only IDs released by its mandatory monitor are decoded into output bytes.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationReceipt;
/// fn grant(receipt: FileTextGenerationReceipt) -> Permit { receipt }
/// ```
#[derive(Clone, Debug)]
pub struct FileTextGenerationReceipt {
    command: Rc<FileTextGenerationCommand>,
    numerical: FileGenerationReceipt,
    result: Result<TextGenerationReport, Error>,
}
impl FileTextGenerationReceipt {
    pub fn command(&self) -> &FileTextGenerationCommand { &self.command }
    pub fn numerical(&self) -> &FileGenerationReceipt { &self.numerical }
    pub fn result(&self) -> Result<&TextGenerationReport, Error> {
        self.result.as_ref().map_err(|error| *error)
    }
    fn from_prepared(command: Rc<FileTextGenerationCommand>, prepared: PreparedText,
        numerical: FileGenerationReceipt) -> Self
    {
        let result = numerical.result().map(|report| prepared.finish(report.clone()));
        Self { command, numerical, result }
    }
}

impl FileOversight {
    /// Install one exact native tokenizer before the first numerical position or
    /// actor request. It cannot be replaced, including after reset or recovery.
    /// Original-ID numerical APIs remain monitored; they do not acquire a text
    /// receipt unless an ORIGINAL text intent exists for their generation ID.
    pub fn enable_decoder_tokenizer(&mut self, revision: u64, tokenizer: ByteBpe)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let config = self.machine.decoder_contract().ok_or(Error::Incomplete)?;
        if !tokenizer.binds(config.profile()) { return Err(Error::Binding.into()); }
        let bytes = tokenizer.to_bytes()?;
        if bytes.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        self.transact(revision, Event::Decoder(DecoderEvent::Tokenizer(bytes.into())))?;
        Ok(())
    }

    /// Freeze raw prompt bytes AND the derived original-ID intent in ONE original
    /// journal cut, without computing a token. Numerical limits and historical
    /// reservations are still those of begin_decoder_generation. A conflicting
    /// text request or an existing bare-ID generation cannot capture this ID.
    pub fn begin_decoder_text(&mut self, revision: u64, command: FileTextGenerationCommand)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        command.check()?;
        if let Some(previous) = self.machine.decoder_text_command(command.id) {
            return if previous.as_ref() == &command { Ok(()) } else { Err(Error::Binding.into()) };
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_decoder_text_intent(&command)?;
        if self.events.len().checked_add(2).ok_or(Error::Overflow)? > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        self.transact(revision, Event::Decoder(DecoderEvent::TextIntent(Rc::new(command))))?;
        Ok(())
    }

    /// Exact retries return the original result without a draw, clock check or
    /// write. New work first commits the complete text intent, then delegates to
    /// generate_decoder's ORIGINAL result/witness transaction. Holds, numerical
    /// refusals, consumed draws and partial work remain in that native receipt.
    /// An unacknowledged result never escapes as text; recovery remains explicit.
    pub fn generate_decoder_text(&mut self, revision: u64, command: FileTextGenerationCommand)
        -> Result<FileTextGenerationReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        command.check()?;
        if let Some(previous) = self.machine.decoder_text_command(command.id) {
            if previous.as_ref() != &command { return Err(Error::Binding.into()); }
            if self.machine.recorded_decoder_generation(command.id).is_some() {
                return self.decoder_text_generation(command.id);
            }
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let (prepared, numerical) = command.compile(self.machine.decoder_tokenizer()?)?;
        let id = command.id;
        self.begin_decoder_text(revision, command)?;
        let command = Rc::clone(self.machine.decoder_text_command(id).ok_or(Error::Missing)?);
        let receipt = self.generate_decoder(self.revision(), numerical)?;
        Ok(FileTextGenerationReceipt::from_prepared(command, prepared, receipt))
    }

    /// Reconstruct ONLY the text projection of an acknowledged numerical result.
    /// This does not run inference, resume a paused owner, or publish an effect.
    /// Failed numerical admission remains Err; Held is not an empty success.
    pub fn decoder_text_generation(&self, id: u64) -> Result<FileTextGenerationReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let command = Rc::clone(self.machine.decoder_text_command(id).ok_or(Error::Missing)?);
        let receipt = self.decoder_generation(id)?;
        let (prepared, expected) = command.compile(self.machine.decoder_tokenizer()?)?;
        if receipt.command() != &expected { return Err(Error::Binding.into()); }
        Ok(FileTextGenerationReceipt::from_prepared(command, prepared, receipt))
    }

    /// Pin BOTH independently retained model/monitor/sampler configuration and
    /// exact tokenizer bytes before numerical replay, cleanup or recovery writes.
    /// The same exclusive Store supplies every comparison and the original fence.
    /// No saved time, human approval or sendable effect envelope is restored.
    /// Other optional guard roles follow the existing open_with_decoder scope;
    /// this is not a substitute for the separately composed guarded-role opener.
    pub fn open_with_text_decoder(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileDecoderConfig, tokenizer: &ByteBpe)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(expected.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = replay_text_history(&profile, expected, &canonical,
            store.identity(), &bytes)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, human))
    }
}

// Both the exclusive opener and read-only progress inspector use the same
// exact configuration pin BEFORE semantic/numerical replay. This never creates
// an owner, cleans storage, or claims a canonical image was acknowledged.
fn replay_text_history(profile: &FileOversightProfile, expected: &FileDecoderConfig,
    tokenizer_bytes: &[u8], identity: &Path, bytes: &[u8])
    -> Result<(Vec<Event>, Machine), JournalError>
{
    let events = journal::decode(profile, identity, bytes)?;
    let mut models = events.iter().filter_map(|event| match event {
        Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()), _ => None,
    });
    let mut tokenizers = events.iter().filter_map(|event| match event {
        Event::Decoder(DecoderEvent::Tokenizer(bytes)) => Some(bytes.as_ref()), _ => None,
    });
    if models.next() != Some(expected) || models.next().is_some()
        || tokenizers.next() != Some(tokenizer_bytes) || tokenizers.next().is_some()
    { return Err(Error::Binding.into()); }
    let machine = Machine::replay(profile, &events)?;
    if machine.decoder_contract() != Some(expected)
        || machine.decoder_tokenizer()?.to_bytes()?.as_slice() != tokenizer_bytes
    { return Err(Error::Binding.into()); }
    Ok((events, machine))
}
