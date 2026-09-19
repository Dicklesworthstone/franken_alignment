//! Durable request intent precedes computation; its result is a separate cut.
//! Request IDs identify historical results, never a live stream or an effect key.
pub mod inspection;
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
/// Conservatively retained requested steps, including pending/refused requests.
/// This bounds history independently of output length or physical replay work.
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
    /// Historical lookup, not resume. A pending ID is Incomplete, not Missing;
    /// an unacknowledged live owner cannot expose a speculative result.
    pub fn decoder_generation(&self, id: u64) -> Result<FileGenerationReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(receipt) = self.machine.recorded_decoder_generation(id) { return Ok(receipt.clone()); }
        if self.machine.pending_decoder_generation().is_some_and(|pending| pending.id() == id) {
            return Err(Error::Incomplete.into());
        }
        Err(Error::Missing.into())
    }

    /// The original frozen INPUT, never an unacknowledged output or hidden draw.
    /// It survives recovery and cannot be cleared by a checkpoint reset or fence.
    pub fn pending_decoder_generation(&self) -> Result<Option<FileGenerationCommand>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.pending_decoder_generation().cloned())
    }

    /// Commit request identity, complete inputs and logical history reservation
    /// BEFORE computation. A matching retry is read-only. At most one intent may
    /// be outstanding; no other inference, proposal or checkpoint reset can skip
    /// it. Existing obligations can still be reconciled and the owner can stop.
    /// No new-request token is computed here. Ordinary durable replay can still
    /// recompute previous history; a quiet new-request result is not promised.
    pub fn begin_decoder_generation(&mut self, revision: u64, command: FileGenerationCommand)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        command.check()?;
        if let Some(recorded) = self.machine.recorded_decoder_generation(command.id) {
            return if recorded.command() == &command { Ok(()) } else { Err(Error::Binding.into()) };
        }
        if let Some(pending) = self.machine.pending_decoder_generation() {
            if pending == &command { return Ok(()); }
            let error = if pending.id() == command.id { Error::Binding } else { Error::WrongState };
            return Err(error.into());
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.clock_ready() || !self.decoder_required() || self.machine.decoder_paused() {
            return Err(Error::Incomplete.into());
        }
        self.machine.check_decoder_generation_intent(&command)?;
        // Both cuts need event slots. Byte/recovery-reserve limits are still
        // enforced by the original encoder; a later failure preserves the intent.
        if self.events.len().checked_add(2).ok_or(Error::Overflow)? > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        self.transact(revision, Event::Decoder(DecoderEvent::BeginGeneration(Rc::new(command))))?;
        Ok(())
    }

    /// Run the ORIGINAL bounded driver only after its exact intent is durable.
    /// A new invocation makes two acknowledged cuts: intent, then result. All
    /// output is withheld until the result's canonical replacement is acknowledged.
    /// Matching completed IDs return history without inference, time checks or IO.
    /// Conflicting inputs, budgets, stop IDs or predecessors cannot reroll an ID.
    ///
    /// An interrupted intent requires explicit fresh-clock recovery and resume,
    /// then this exact command, or permanent suspension. Physical computation may
    /// repeat after a crash; no exactly-once CPU claim or work refund is made.
    /// Native refusals/holds are recorded. Once new-request execution is entered,
    /// witness/encoding/storage failure leaves this owner unavailable and its
    /// acknowledged intent unresolved, rather than exposing speculative output.
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
        self.begin_decoder_generation(revision, command.clone())?;
        self.machine.check_decoder_generation(&command)?;
        let id = command.id;
        let command = Rc::new(command);
        self.check_source_admission(&Event::Decoder(DecoderEvent::Generate(
            Rc::clone(&command), Rc::from(&b""[..]))))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        // Close the original owner BEFORE computation. A caught unwind or failed
        // write cannot reopen its quiet prefix or abandon the durable command.
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
