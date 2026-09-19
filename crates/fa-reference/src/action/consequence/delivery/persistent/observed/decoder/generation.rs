//! One acknowledged journal cut for a complete monitored generation request.
//! Request IDs identify historical results, never a live stream or an effect key.
use super::{DecoderEvent, FileOversight, JournalError, Machine, Transition, Event, journal};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationReport, GenerationRequest, MAX_GENERATION_TOKENS,
    MAX_SAMPLING_ENTRIES, MAX_STOP_TOKENS,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

pub const MAX_FILE_GENERATIONS: usize = 128;
/// Conservatively retained requested steps, including stopped/refused requests.
/// This bounds the history independently of actual output length or journal IO.
pub const MAX_FILE_GENERATION_STEPS: usize = 65_536;

#[derive(Clone, PartialEq, Eq)]
pub struct FileGenerationCommand {
    id: u64,
    actor_revision: u64,
    position: u64,
    request: GenerationRequest,
}
impl FileGenerationCommand {
    pub fn new(id: u64, actor_revision: u64, position: u64, request: GenerationRequest) -> Result<Self, Error> {
        let command = Self { id, actor_revision, position, request };
        command.check()?;
        Ok(command)
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn position(&self) -> u64 { self.position }
    pub fn request(&self) -> &GenerationRequest { &self.request }
    pub(in super::super) fn check(&self) -> Result<usize, Error> {
        let r = &self.request;
        let count = r.prompt.len().checked_add(r.max_new_tokens).ok_or(Error::Overflow)?;
        if self.id == 0 || count == 0 { return Err(Error::InvalidInput); }
        if count > MAX_GENERATION_TOKENS || r.stop_tokens.len() > MAX_STOP_TOKENS
            || r.budget.scalar_products > MAX_DECODER_PRODUCTS
            || r.budget.sampling_entries > MAX_SAMPLING_ENTRIES { return Err(Error::Limit); }
        let stops: BTreeSet<_> = r.stop_tokens.iter().copied().collect();
        if stops.len() != r.stop_tokens.len() { return Err(Error::Duplicate); }
        Ok(count)
    }
}
impl fmt::Debug for FileGenerationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGenerationCommand").field("id", &self.id)
            .field("actor_revision", &self.actor_revision).field("position", &self.position)
            .field("prompt_tokens", &self.request.prompt.len())
            .field("max_new_tokens", &self.request.max_new_tokens).finish_non_exhaustive()
    }
}

/// Historical supervisor data. Only reviewed continuation IDs occur in a report;
/// a stored failure/hold cannot be turned into a newly attempted random draw.
/// Cloning a receipt does not clone a numerical owner, approval or permission.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::generation::FileGenerationReceipt;
/// fn grant(receipt: FileGenerationReceipt) -> Permit { receipt }
/// ```
#[derive(Clone)]
pub struct FileGenerationReceipt {
    command: Rc<FileGenerationCommand>,
    result: Rc<Result<GenerationReport, Error>>,
}
impl FileGenerationReceipt {
    pub fn command(&self) -> &FileGenerationCommand { &self.command }
    /// Err is the recorded native admission refusal. An Ok report can itself
    /// terminate with Held, Failed or BudgetExhausted; inspect its finish value.
    pub fn result(&self) -> Result<&GenerationReport, Error> {
        self.result.as_ref().as_ref().map_err(|error| *error)
    }
    pub(in super::super) fn recorded(command: Rc<FileGenerationCommand>, result: Result<GenerationReport, Error>) -> Self {
        Self { command, result: Rc::new(result) }
    }
}
impl fmt::Debug for FileGenerationReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGenerationReceipt").field("command", &self.command)
            .field("result", &self.result).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Historical lookup only. Recovery may return this receipt while inference
    /// is paused; the receipt does not resume inference or restore old keys.
    /// An unacknowledged live owner cannot expose a speculative result.
    pub fn decoder_generation(&self, id: u64) -> Result<FileGenerationReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.recorded_decoder_generation(id).cloned().ok_or_else(|| Error::Missing.into())
    }

    /// Execute through the ORIGINAL bounded driver and per-token hosted methods.
    /// All outputs are withheld until canonical replacement is acknowledged.
    /// A matching ID returns the historical receipt without inference or IO,
    /// even with a stale journal predecessor or a recovery-paused decoder.
    /// Every command field must match; conflicting IDs cannot reroll or revise
    /// budgets, prompts, stop IDs, numerical positions or actor predecessors.
    ///
    /// New commands require a fresh clock, resumed decoder and current journal
    /// revision. Native failures are recorded; witness/encoding/storage failures
    /// instead poison the owner, with no successful receipt or quiet fallback.
    pub fn generate_decoder(&mut self, revision: u64, command: FileGenerationCommand)
        -> Result<FileGenerationReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        command.check()?;
        if let Some(recorded) = self.machine.recorded_decoder_generation(command.id) {
            if recorded.command() != &command { return Err(Error::Binding.into()); }
            return Ok(recorded.clone());
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.clock_ready() || !self.decoder_required() || self.machine.decoder_paused() {
            return Err(Error::Incomplete.into());
        }
        self.machine.check_decoder_generation(&command)?;
        let id = command.id;
        let command = Rc::new(command);
        self.check_source_admission(&Event::Decoder(DecoderEvent::Generate(
            Rc::clone(&command), Rc::from(&b""[..]))))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        // Close the original owner BEFORE computation. A caught unwind, failed
        // witness or ambiguous write must not reopen its older quiet prefix.
        self.fault = Some(super::super::super::JournalFailure {
            operation: super::super::super::JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false,
        });
        let event = Event::Decoder(candidate.prepare_decoder_generation(command)?);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        self.decoder_generation(id)
    }
}

pub(in super::super) fn write_command(w: &mut Writer, command: &FileGenerationCommand) -> Result<(), Error> {
    command.check()?;
    w.u64(command.id)?; w.u64(command.actor_revision)?; w.u64(command.position)?;
    let request = &command.request;
    w.count(request.prompt.len())?;
    for token in &request.prompt { w.u32(*token)?; }
    w.count(request.max_new_tokens)?; w.count(request.stop_tokens.len())?;
    for token in &request.stop_tokens { w.u32(*token)?; }
    w.u64(request.budget.scalar_products)?; w.u64(request.budget.sampling_entries)
}
pub(super) fn read_command(r: &mut Reader<'_>) -> Result<FileGenerationCommand, Error> {
    let id = r.u64()?; let actor_revision = r.u64()?; let position = r.u64()?;
    let count = r.count(MAX_GENERATION_TOKENS)?;
    let mut prompt = Vec::new(); prompt.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { prompt.push(r.u32()?); }
    let max_new_tokens = r.count(MAX_GENERATION_TOKENS)?;
    if count.checked_add(max_new_tokens).ok_or(Error::Overflow)? > MAX_GENERATION_TOKENS { return Err(Error::Limit); }
    let count = r.count(MAX_STOP_TOKENS)?;
    let mut stop_tokens = Vec::new(); stop_tokens.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { stop_tokens.push(r.u32()?); }
    let budget = GenerationBudget { scalar_products: r.u64()?, sampling_entries: r.u64()? };
    FileGenerationCommand::new(id, actor_revision, position, GenerationRequest { prompt, max_new_tokens, stop_tokens, budget })
}
