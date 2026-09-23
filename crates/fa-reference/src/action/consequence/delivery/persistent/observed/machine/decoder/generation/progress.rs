//! One acknowledged cursor transition over the original numerical operation.
//! The cursor is rebuilt by replay, never loaded from its comparison witness.
mod cancellation;

use super::{Machine, Transition, DecoderEvent, FileGenerationReceipt,
    RecordingOwner, Writer, MAX_WITNESS_BYTES, write_command, write_result, review_bytes};
use super::super::super::super::decoder::progress::{FileGenerationProgress, check_progress_key};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationReport, incremental::{GenerationCursor, GenerationProgress},
};
use crate::Error;
use std::rc::Rc;

pub(super) struct ProgressState {
    cursor: GenerationCursor,
    actor_revision: u64,
}
enum Advancement {
    Pending(GenerationCursor),
    Complete(Result<GenerationReport, Error>),
}

impl Machine {
    pub(in super::super::super::super) fn decoder_generation_progress(&self, id: u64)
        -> Result<FileGenerationProgress, Error>
    {
        let history = &self.decoder.as_ref().ok_or(Error::Incomplete)?.generations;
        let revision = history.progress_revisions.get(&id).copied().unwrap_or(0);
        if let Some(receipt) = history.records.get(&id) {
            return Ok(FileGenerationProgress::recorded(revision, receipt.clone()));
        }
        let command = history.pending.as_ref().ok_or(Error::Missing)?;
        if command.id() != id { return Err(Error::Missing); }
        let progress = history.progress.as_ref().map(|state| state.cursor.progress());
        Ok(FileGenerationProgress::pending(Rc::clone(command), revision, progress))
    }

    pub(in super::super::super::super) fn check_decoder_generation_progress(&self, id: u64, revision: u64)
        -> Result<(), Error>
    {
        check_progress_key(id, revision)?;
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        if state.paused || !self.clock_ready { return Err(Error::Incomplete); }
        let history = &state.generations;
        let command = history.pending.as_ref().ok_or(Error::Incomplete)?;
        if command.id() != id { return Err(Error::Binding); }
        if history.progress_revisions.get(&id).copied().unwrap_or(0) != revision { return Err(Error::Stale); }
        if revision >= command.check()? as u64 { return Err(Error::Limit); }
        if let Some(progress) = &history.progress {
            let actual = self.broker.hosted_decoder()?;
            if actual.actor_revision != progress.actor_revision || actual.position != progress.cursor.position() {
                return Err(Error::Stale);
            }
            if progress.cursor.is_complete() { return Err(Error::WrongState); }
        } else if revision != 0 { return Err(Error::Binding); }
        Ok(())
    }

    pub(in super::super::super::super) fn prepare_decoder_generation_progress(&mut self, id: u64, revision: u64)
        -> Result<DecoderEvent, Error>
    {
        let witness = self.execute_decoder_generation_progress(id, revision)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(DecoderEvent::AdvanceGeneration { id, revision, witness: witness.into() })
    }
    pub(in super::super) fn apply_decoder_generation_progress(&mut self, id: u64, revision: u64, expected: &[u8])
        -> Result<Transition, Error>
    {
        let witness = self.execute_decoder_generation_progress(id, revision)?;
        if witness.as_slice() != expected { return Err(Error::Binding); }
        Ok(Transition::Unit)
    }

    fn execute_decoder_generation_progress(&mut self, id: u64, revision: u64) -> Result<Vec<u8>, Error> {
        self.check_decoder_generation_progress(id, revision)?;
        let next_revision = revision.checked_add(1).ok_or(Error::Overflow)?;
        let (command, previous) = {
            let history = &mut self.decoder.as_mut().ok_or(Error::Incomplete)?.generations;
            (Rc::clone(history.pending.as_ref().ok_or(Error::Incomplete)?), history.progress.take())
        };
        let mut transcript = Writer::new(MAX_WITNESS_BYTES);
        transcript.raw(b"FADGPR\x01")?;
        write_command(&mut transcript, &command)?;
        transcript.u64(revision)?;
        // Reuse original full-prompt/host admission ONCE. A recovered cursor
        // keeps the original remaining allowance instead of admitting a suffix
        // as a new prompt or resetting its sampling budget.
        let cursor = match previous {
            Some(state) => Ok(state.cursor),
            None => {
                let stopped = self.broker.stop_receipt().is_some();
                let admission = self.broker.prepare_hosted_generation(
                    command.actor_revision(), command.position(), command.request());
                self.finish_decoder_stop(stopped)?;
                admission.and_then(|()| GenerationCursor::new(&self.broker, command.position(), command.request().clone()))
            }
        };
        let mut owner = RecordingOwner { machine: self, transcript, witness_failure: None };
        let advanced = match cursor {
            Err(error) => Advancement::Complete(Err(error)),
            Ok(mut cursor) => {
                cursor.advance(&mut owner)?;
                if cursor.is_complete() { Advancement::Complete(Ok(cursor.into_report()?)) }
                else { Advancement::Pending(cursor) }
            }
        };
        if let Some(error) = owner.witness_failure { return Err(error); }
        owner.transcript.u8(0)?;
        // Each optional progress cut independently binds the actual full state.
        // This intentionally costs more bytes/replay than the compact whole-run
        // path; it buys resumable boundaries, not a serving-throughput claim.
        owner.machine.write_decoder_state(&mut owner.transcript)?;
        match &advanced {
            Advancement::Pending(cursor) => {
                owner.transcript.u8(0)?;
                write_partial(&mut owner.transcript, &cursor.progress())?;
            }
            Advancement::Complete(result) => {
                owner.transcript.u8(1)?;
                write_result(&mut owner.transcript, result)?;
            }
        }
        owner.machine.write_decoder_stop_witness(&mut owner.transcript)?;
        let actor_revision = owner.machine.broker.actor_revision();
        match advanced {
            Advancement::Pending(cursor) => {
                owner.machine.decoder.as_mut().ok_or(Error::Incomplete)?.generations.progress =
                    Some(ProgressState { cursor, actor_revision });
            }
            Advancement::Complete(result) => {
                owner.machine.retain_generation(FileGenerationReceipt::recorded(command, result))?;
            }
        }
        owner.machine.decoder.as_mut().ok_or(Error::Incomplete)?.generations.progress_revisions.insert(id, next_revision);
        Ok(owner.transcript.finish())
    }
}

fn write_partial(w: &mut Writer, progress: &GenerationProgress) -> Result<(), Error> {
    if progress.finish().is_some() { return Err(Error::Binding); }
    w.u64(progress.start_position())?; w.u64(progress.position())?;
    w.count(progress.requested_prompt_tokens())?; w.count(progress.reviewed_prompt_tokens())?;
    w.count(progress.tokens().len())?;
    for token in progress.tokens() { w.u32(*token)?; }
    let work = progress.work();
    w.u64(work.admitted_scalar_products)?; w.u64(work.admitted_sampling_entries)?; w.count(work.attempted_samples)?;
    match progress.last_review() {
        None => w.u8(0)?, Some(review) => { w.u8(1)?; w.blob(&review_bytes(review)?)?; }
    }
    Ok(())
}
