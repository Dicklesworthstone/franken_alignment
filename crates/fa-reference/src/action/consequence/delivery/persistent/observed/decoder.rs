//! Monitored numerical execution inside the ORIGINAL durable oversight owner.
//! Persisted witnesses are compared with recomputation, never imported as state.
mod config;
mod codec;
pub mod checkpoint;
pub use config::FileDecoderConfig;
pub(super) use codec::{read, write};
#[cfg(test)]
mod tests;

use super::{BaseEvent, Event, FileOversight, FileOversightProfile, FileHumanReviewer,
    JournalError, Machine, Transition, journal, storage};
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::action::consequence::oversight::decoder_host::HostedDecoderInspection;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

pub(super) const MAX_WITNESS_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum StepRequest {
    Forced { revision: u64, position: u64, token: u32, products: u64 },
    Sampled { revision: u64, position: u64, products: u64, vocabulary: usize },
}
#[derive(Clone)]
pub(super) enum DecoderEvent {
    Enable(Rc<FileDecoderConfig>),
    Step(StepRequest, Rc<[u8]>),
    Resume { revision: u64, position: u64 },
    Checkpoint(checkpoint::CheckpointRequest, Rc<[u8]>),
}

/// The state at the last acknowledged journal cut. A recovered prefix is paused
/// until explicit resume; this read does not disclose held token IDs or logits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDecoderInspection {
    pub journal_revision: u64,
    pub paused: bool,
    pub numerical: HostedDecoderInspection,
}

impl FileOversight {
    /// Freeze actual model bytes, complete monitor/sampler configuration and the
    /// original decoder gate before any work. First publication is also guarded.
    /// There is no model replacement, unchecked ActorState import or reseed path.
    pub fn enable_decoder(&mut self, revision: u64, config: FileDecoderConfig) -> Result<(), JournalError> {
        self.transact(revision, Event::Decoder(DecoderEvent::Enable(Rc::new(config))))?; Ok(())
    }
    pub fn decoder_required(&self) -> bool { self.machine.decoder_contract().is_some() }
    pub fn decoder_inspection(&self) -> Result<FileDecoderInspection, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileDecoderInspection { journal_revision: self.revision(), paused: self.machine.decoder_paused(),
            numerical: self.machine.broker.hosted_decoder()? })
    }

    /// Outer error: no acknowledged durable transition. Inner error: the original
    /// numerical call refused or failed, and its resulting state WAS committed.
    /// A computed held token remains withheld and its work/draw are not refunded.
    /// The predecessor is one-use: a successful step cannot execute twice at it.
    pub fn advance_decoder_forced(&mut self, revision: u64, expected_actor_revision: u64,
        expected_position: u64, token: u32, budget: DecoderBudget)
        -> Result<Result<MonitoredStep, Error>, JournalError>
    {
        match self.transact_decoder(revision, StepRequest::Forced { revision: expected_actor_revision,
            position: expected_position, token, products: budget.scalar_products })? {
            Transition::DecoderForced(result) => Ok(*result),
            _ => unreachable!("forced numerical transition"),
        }
    }
    pub fn advance_decoder_sampled(&mut self, revision: u64, expected_actor_revision: u64,
        expected_position: u64, budget: SampleBudget)
        -> Result<Result<MonitoredSampledStep, Error>, JournalError>
    {
        match self.transact_decoder(revision, StepRequest::Sampled { revision: expected_actor_revision,
            position: expected_position, products: budget.decoder.scalar_products, vocabulary: budget.sampling.vocabulary })? {
            Transition::DecoderSampled(result) => Ok(*result),
            _ => unreachable!("sampled numerical transition"),
        }
    }

    /// Recovery re-executes and compares the ENTIRE recorded numerical history.
    /// It does not release its old outputs. Explicit resume requires a fresh clock,
    /// the exact recovered predecessor and an originally Ready numerical owner.
    /// A held/failed run cannot be resumed or rerolled through this operation.
    pub fn resume_decoder(&mut self, revision: u64, actor_revision: u64, position: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::Decoder(DecoderEvent::Resume { revision: actor_revision, position }))?; Ok(())
    }

    /// Pin the exact configuration BEFORE cleanup or recovery writes. Matching
    /// labels are insufficient: all original parameter/configuration bytes match.
    /// A numerical mismatch during replay refuses, never imports saved approvals.
    pub fn open_with_decoder(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileDecoderConfig) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let machine = Machine::replay(&profile, &events)?;
        if machine.decoder_contract() != Some(expected) { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, human))
    }

    fn transact_decoder(&mut self, revision: u64, request: StepRequest) -> Result<Transition, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        // The temporary shape is used only for admission, never encoded or stored.
        self.check_source_admission(&Event::Decoder(DecoderEvent::Step(request, Rc::from(&b""[..]))))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let (event, result) = candidate.prepare_decoder_step(request)?;
        let event = Event::Decoder(event);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        // No second current-step execution, clock callback or external operation
        // intervenes. Reuse the ORIGINAL canonical replacement/poisoning boundary.
        self.persist_candidate(event, bytes, candidate, result)
    }
}
