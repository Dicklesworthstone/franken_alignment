//! Record every original step, not just the last quiet review of a generation.
//! The complete final cache covers all appended K/V: synchronous generation has
//! no reset path. Do not copy the growing cache into every per-token witness.
use super::{Machine, Transition, DecoderEvent, StepRequest, Writer, MAX_WITNESS_BYTES,
    write_decoder_result, error_tag};
use super::super::super::decoder::generation::{FileGenerationCommand, FileGenerationReceipt,
    MAX_FILE_GENERATIONS, MAX_FILE_GENERATION_STEPS, write_command};
use crate::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus, ReviewedStep};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledStep,
    generation::{self, GenerationFinish, GenerationOwner, GenerationReport}, host::replay::review_bytes};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderProfile, DecoderWork};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct GenerationHistory {
    records: BTreeMap<u64, FileGenerationReceipt>,
    requested_steps: usize,
    pending: Option<Rc<FileGenerationCommand>>,
}

impl Machine {
    pub(in super::super::super) fn recorded_decoder_generation(&self, id: u64) -> Option<&FileGenerationReceipt> {
        self.decoder.as_ref()?.generations.records.get(&id)
    }
    pub(in super::super::super) fn pending_decoder_generation(&self) -> Option<&FileGenerationCommand> {
        self.decoder.as_ref()?.generations.pending.as_deref()
    }
    pub(in super::super::super) fn check_decoder_generation(&self, command: &FileGenerationCommand) -> Result<(), Error> {
        let count = command.check()?;
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        if state.paused || !self.clock_ready { return Err(Error::Incomplete); }
        let history = &state.generations;
        if history.records.contains_key(&command.id()) { return Err(Error::Duplicate); }
        if let Some(pending) = &history.pending {
            return if pending.as_ref() == command { Ok(()) }
                else { Err(if pending.id() == command.id() { Error::Binding } else { Error::WrongState }) };
        }
        if history.records.len() >= MAX_FILE_GENERATIONS
            || history.requested_steps.checked_add(count).ok_or(Error::Overflow)? > MAX_FILE_GENERATION_STEPS
        { return Err(Error::Limit); }
        Ok(())
    }
    pub(in super::super::super) fn check_decoder_generation_intent(&self, command: &FileGenerationCommand) -> Result<(), Error> {
        self.check_decoder_generation(command)?;
        if self.pending_decoder_generation().is_some() { return Err(Error::Duplicate); }
        let actual = self.broker.hosted_decoder()?;
        if actual.actor_revision != command.actor_revision() || actual.position != command.position() {
            return Err(Error::Stale);
        }
        if actual.status != MonitoringStatus::Ready || self.broker.inspect().suspended {
            return Err(Error::WrongState);
        }
        Ok(())
    }
    pub(super) fn apply_decoder_generation_intent(&mut self, command: &Rc<FileGenerationCommand>) -> Result<Transition, Error> {
        self.check_decoder_generation_intent(command)?;
        let count = command.check()?;
        let history = &mut self.decoder.as_mut().ok_or(Error::Incomplete)?.generations;
        history.requested_steps = history.requested_steps.checked_add(count).ok_or(Error::Overflow)?;
        history.pending = Some(Rc::clone(command));
        Ok(Transition::Unit)
    }
    fn retain_generation(&mut self, receipt: FileGenerationReceipt) -> Result<(), Error> {
        let count = receipt.command().check()?;
        let history = &mut self.decoder.as_mut().ok_or(Error::Incomplete)?.generations;
        // Do not recheck paused after the actual stop. An intent has ALREADY
        // reserved its history capacity; completion never charges it twice.
        if history.records.contains_key(&receipt.command().id()) { return Err(Error::Duplicate); }
        let total = match &history.pending {
            Some(pending) if pending.as_ref() == receipt.command() => history.requested_steps,
            Some(_) => return Err(Error::Binding),
            // Compatibility for existing tag-6 journals produced before intents.
            // The live preparation path below never creates a new intentless run.
            None => history.requested_steps.checked_add(count).ok_or(Error::Overflow)?,
        };
        if history.records.len() >= MAX_FILE_GENERATIONS || total > MAX_FILE_GENERATION_STEPS { return Err(Error::Limit); }
        history.records.insert(receipt.command().id(), receipt);
        history.requested_steps = total;
        history.pending = None;
        Ok(())
    }
    pub(in super::super::super) fn prepare_decoder_generation(&mut self, command: Rc<FileGenerationCommand>)
        -> Result<DecoderEvent, Error>
    {
        if self.pending_decoder_generation() != Some(command.as_ref()) { return Err(Error::Binding); }
        let (receipt, witness) = self.execute_decoder_generation(Rc::clone(&command))?;
        self.retain_generation(receipt)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(DecoderEvent::Generate(command, witness.into()))
    }
    pub(super) fn apply_decoder_generation(&mut self, command: &Rc<FileGenerationCommand>, expected: &[u8])
        -> Result<Transition, Error>
    {
        let (receipt, witness) = self.execute_decoder_generation(Rc::clone(command))?;
        if witness.as_slice() != expected { return Err(Error::Binding); }
        self.retain_generation(receipt)?;
        Ok(Transition::Unit)
    }
    fn execute_decoder_generation(&mut self, command: Rc<FileGenerationCommand>)
        -> Result<(FileGenerationReceipt, Vec<u8>), Error>
    {
        self.check_decoder_generation(&command)?;
        let mut transcript = Writer::new(MAX_WITNESS_BYTES); transcript.raw(b"FADGEN\x01")?;
        // Bind even unused budget/stop fields and the ID. Two commands producing
        // identical tokens must not be able to substitute comparison material.
        write_command(&mut transcript, &command)?;
        let already_stopped = self.broker.stop_receipt().is_some();
        let admission = self.broker.prepare_hosted_generation(
            command.actor_revision(), command.position(), command.request());
        self.finish_decoder_stop(already_stopped)?;
        let mut owner = RecordingOwner { machine: self, transcript, witness_failure: None };
        let result = admission.and_then(|()| generation::drive(&mut owner, command.position(), command.request().clone()));
        // A comparison-buffer or cleanup failure is NOT a native numerical
        // result. Never acknowledge a partial transcript as an ordinary failure.
        if let Some(error) = owner.witness_failure { return Err(error); }
        owner.transcript.u8(0)?; // End of the actual attempted-step sequence.
        owner.machine.write_decoder_state(&mut owner.transcript)?;
        write_result(&mut owner.transcript, &result)?;
        owner.machine.write_decoder_stop_witness(&mut owner.transcript)?;
        Ok((FileGenerationReceipt::recorded(command, result), owner.transcript.finish()))
    }
}

// Crate-private composition of existing owners. No public implementation point,
// arbitrary callback, imported report or alternative numerical engine exists.
struct RecordingOwner<'a> {
    machine: &'a mut Machine,
    transcript: Writer,
    witness_failure: Option<Error>,
}
impl RecordingOwner<'_> {
    fn step(&mut self, request: StepRequest) -> Result<Transition, Error> {
        let prepared = (|| {
            let result = self.machine.execute_decoder_step(request)?;
            self.transcript.u8(1)?;
            write_decoder_result(&mut self.transcript, &result)?;
            // Includes original per-token logits, loop counts and exact interval
            // reviews; changing an early score cannot hide behind a final match.
            match &result {
                Transition::DecoderForced(result) => match result.as_ref() {
                    Ok(MonitoredStep::Released(step)) => write_released_review(&mut self.transcript, step)?,
                    Ok(MonitoredStep::Held(review)) => self.transcript.blob(&review_bytes(review)?)?,
                    Err(_) => {}
                },
                Transition::DecoderSampled(result) => match result.as_ref() {
                    Ok(MonitoredSampledStep::Released(step)) => write_released_review(&mut self.transcript, step.reviewed())?,
                    Ok(MonitoredSampledStep::Held(review)) => self.transcript.blob(&review_bytes(review)?)?,
                    Err(_) => {}
                },
                _ => return Err(Error::Binding),
            }
            Ok(result)
        })();
        if let Err(error) = &prepared { self.witness_failure = Some(*error); }
        prepared
    }
}
impl GenerationOwner for RecordingOwner<'_> {
    fn generation_profile(&self) -> Result<&DecoderProfile, Error> { self.machine.broker.generation_profile() }
    fn generation_position(&self) -> Result<u64, Error> { self.machine.broker.generation_position() }
    fn generation_status(&self) -> Result<MonitoringStatus, Error> { self.machine.broker.generation_status() }
    fn generation_estimate(&self, tokens: usize) -> Result<DecoderWork, Error> { self.machine.broker.generation_estimate(tokens) }
    fn generation_forced(&mut self, position: u64, token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        match self.step(StepRequest::Forced { revision: self.machine.broker.actor_revision(), position,
            token, products: budget.scalar_products })? {
            Transition::DecoderForced(result) => *result,
            _ => Err(Error::Binding),
        }
    }
    fn generation_sampled(&mut self, position: u64, budget: SampleBudget) -> Result<MonitoredStep, Error> {
        match self.step(StepRequest::Sampled { revision: self.machine.broker.actor_revision(), position,
            products: budget.decoder.scalar_products, vocabulary: budget.sampling.vocabulary })? {
            Transition::DecoderSampled(result) => (*result).map(MonitoredSampledStep::into_monitored),
            _ => Err(Error::Binding),
        }
    }
}

fn write_released_review(w: &mut Writer, reviewed: &ReviewedStep) -> Result<(), Error> {
    w.blob(&review_bytes(reviewed.review())?)?;
    let step = reviewed.step();
    w.count(step.logits.len())?;
    for value in step.logits.iter() { w.u32(value.to_bits())?; }
    let work = step.work;
    for value in [work.tokens, work.matrix_products, work.attention_products,
        work.attention_exponentials, work.normalization_coordinates, work.rotary_pairs,
        work.gate_coordinates, work.cache_values_appended] { w.u64(value)?; }
    Ok(())
}
fn write_result(w: &mut Writer, result: &Result<GenerationReport, Error>) -> Result<(), Error> {
    let report = match result {
        Err(error) => { w.u8(0)?; return w.u8(error_tag(*error)); }
        Ok(report) => { w.u8(1)?; report }
    };
    w.u64(report.start_position())?; w.u64(report.end_position())?;
    w.count(report.requested_prompt_tokens())?; w.count(report.reviewed_prompt_tokens())?;
    w.count(report.tokens().len())?;
    for token in report.tokens() { w.u32(*token)?; }
    match report.finish() {
        GenerationFinish::TokenLimit => w.u8(0)?, GenerationFinish::StopToken => w.u8(1)?,
        GenerationFinish::BudgetExhausted => w.u8(2)?, GenerationFinish::Held => w.u8(3)?,
        GenerationFinish::Failed(error) => { w.u8(4)?; w.u8(error_tag(error))?; }
    }
    let work = report.work();
    w.u64(work.admitted_scalar_products)?; w.u64(work.admitted_sampling_entries)?; w.count(work.attempted_samples)?;
    match report.last_review() {
        None => w.u8(0)?, Some(review) => { w.u8(1)?; w.blob(&review_bytes(review)?)?; }
    }
    Ok(())
}
