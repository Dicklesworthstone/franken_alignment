//! Durable original learned-K/V generation in the existing two-key effect owner.
//! Saved tensors, draws, scores and outcomes are comparison material only.
//! Recovery requires an independently supplied exact recipe, then the original
//! numerical/authority replay and fence. No learned state or approval is imported.
mod config;
mod recovery;
mod pending;
mod preparation;
pub mod sidecar;
pub mod checkpoint;
#[cfg(test)]
mod checkpoint_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod admission_tests;
#[cfg(test)]
mod pending_tests;
#[cfg(test)]
mod preparation_tests;
#[cfg(test)]
mod continuation_tests;
pub use config::FileLearnedConfig;
pub use preparation::{FileLearnedReplayContinuation, FileLearnedIntentPreparation, FileLearnedStepPreparation, FileLearnedStepPreparationProgress, FileLearnedStepPreparationStatus};
pub use recovery::{FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus};

use super::{DecoderEvent, MAX_WITNESS_BYTES};
use super::super::{Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, Machine, Transition, journal, storage};
use super::super::super::{FileDeliverySnapshot, JournalFailure, JournalIo};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationEvent;
use crate::action::consequence::oversight::learned_host::HostedLearnedInspection;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone)]
pub(in super::super) enum LearnedEvent {
    Enable(Configuration),
    Sidecar(sidecar::SidecarEvent),
    Checkpoint(checkpoint::CheckpointEvent),
    Begin(LearnedStepIntent),
    Step { actor_revision: u64, position: u64, witness: Rc<[u8]> },
    Resume { actor_revision: u64, position: u64 },
}

/// Parsed bytes alone cannot construct the runtime recipe. Binding happens only
/// in the explicit recovery/read API after complete byte-for-byte comparison.
#[derive(Clone)]
pub(in super::super) struct Configuration {
    bytes: Rc<[u8]>,
    runtime: Option<Rc<FileLearnedConfig>>,
}
impl Configuration {
    pub(in super::super) fn new(config: FileLearnedConfig) -> Self {
        Self { bytes: Rc::clone(config.bytes()), runtime: Some(Rc::new(config)) }
    }
    pub(in super::super) fn runtime(&self) -> Result<Rc<FileLearnedConfig>, Error> {
        let config = self.runtime.as_ref().ok_or(Error::Incomplete)?;
        if config.bytes().as_ref() != self.bytes.as_ref() { return Err(Error::Binding); }
        Ok(Rc::clone(config))
    }
}

/// One immutable numerical operation. This is not an effect permit or a token
/// override: the original frozen prompt/sampler chooses the token at this position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedStepIntent {
    pub actor_revision: u64,
    pub position: u64,
}

/// Last acknowledged state and logical work. Paused state is historical and
/// cannot justify a new proposal, review, approval or dispatch before resume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedInspection {
    pub journal_revision: u64,
    pub paused: bool,
    pub pending: Option<LearnedStepIntent>,
    pub numerical: HostedLearnedInspection,
}

impl FileOversight {
    // Both the intent and its outcome are ordinary work. The terminal recovery
    // tail cannot complete either one. Check before replay or numerical work;
    // this promises record slots, not future witness bytes or disk space.
    pub(in super::super) fn check_learned_event_capacity(&self, revision: u64, records: usize)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if records > self.journal_capacity()?.ordinary_remaining().events {
            return Err(Error::Limit.into());
        }
        Ok(())
    }

    /// Install the ORIGINAL owned learned generator and source-dependent effect
    /// gate before any proposals. All usual committee and human keys still apply.
    /// The prompt, sampler, codec, complete probe roster and budgets are frozen.
    pub fn enable_learned_generation(&mut self, revision: u64, config: FileLearnedConfig)
        -> Result<(), JournalError>
    {
        self.transact(revision, Event::Decoder(DecoderEvent::Learned(
            LearnedEvent::Enable(Configuration::new(config)))))?;
        Ok(())
    }
    pub fn learned_generation_required(&self) -> bool { self.machine.learned_contract().is_some() }
    pub fn learned_generation_inspection(&self) -> Result<FileLearnedInspection, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileLearnedInspection { journal_revision: self.revision(), paused: self.machine.learned_paused(),
            pending: self.machine.pending_learned_step(),
            numerical: self.machine.broker.hosted_learned_generation()? })
    }

    /// Durably freeze the next original numerical operation BEFORE inference.
    /// An interrupted intent survives recovery and blocks all new permitting work
    /// until this exact step is recomputed and its outcome has been acknowledged.
    pub fn begin_learned_step(&mut self, revision: u64, actor_revision: u64, position: u64)
        -> Result<(), JournalError>
    {
        let mut preparation = self.cached_learned_intent(revision, actor_revision, position)?;
        while preparation.progress().status == FileLearnedStepPreparationStatus::Replaying {
            let completed = preparation.progress().replayed_events;
            preparation.advance(self, completed, 1)?;
        }
        let carry = preparation.finish_with_continuation(self)?;
        self.learned_replay = Some(carry);
        Ok(())
    }

    /// Write-ahead intent plus acknowledged outcome. A matching pending intent
    /// is completed rather than replaced. No returned token predates its durable
    /// completion. A fresh operation advances the journal twice, not once.
    pub fn advance_learned_generation(&mut self, revision: u64, actor_revision: u64, position: u64)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if self.machine.pending_learned_step().is_none() {
            self.begin_learned_step(revision, actor_revision, position)?;
        }
        self.complete_learned_step(self.revision(), actor_revision, position)
    }

    /// Outer Err: no acknowledged outcome and no candidate result. The durable
    /// intent remains a barrier, including after a crash or storage failure.
    /// Inner Err: the original numerical error and resulting state WERE persisted.
    /// A held sample stays withheld. A numerical or storage failure cannot silently
    /// fall back to the prior quiet state in this owner. Stale calls do no work.
    pub fn complete_learned_step(&mut self, revision: u64, actor_revision: u64, position: u64)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        let mut preparation = self.cached_learned_completion(revision, actor_revision, position)?;
        while preparation.progress().status == FileLearnedStepPreparationStatus::Replaying {
            let completed = preparation.progress().replayed_events;
            preparation.advance(self, completed, 1)?;
        }
        let (result, carry) = preparation.finish_with_continuation(self)?;
        self.learned_replay = Some(carry);
        Ok(result)
    }

    // Shared by synchronous and cooperative completion, before replay and
    // again before the next original numerical operation. No candidate bypass.
    fn check_learned_completion(&self, revision: u64, actor_revision: u64, position: u64)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.preflight_learned_step(actor_revision, position)?;
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step {
            actor_revision, position, witness: Rc::from(&b""[..]),
        }));
        self.check_source_admission(&shape)?;
        self.check_learned_event_capacity(revision, 1)?;
        Ok(())
    }

    // Only fully replayed, exact-owner preparations call this private boundary.
    fn persist_prepared_learned_step(&mut self, revision: u64, actor_revision: u64,
        position: u64, candidate: Machine) -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        self.persist_prepared_learned_step_retaining(revision, actor_revision, position, candidate)
            .map(|(result, _)| result)
    }

    // Preserve the former acknowledged machine only AFTER successful completion.
    // This shares all numerical, witness, poison and storage behavior above.
    fn persist_prepared_learned_step_retaining(&mut self, revision: u64, actor_revision: u64,
        position: u64, mut candidate: Machine)
        -> Result<(Result<Rc<GenerationEvent>, Error>, Machine), JournalError>
    {
        self.check_learned_completion(revision, actor_revision, position)?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        // This is the existing numerical-transaction poison boundary. Nothing
        // external executes in the private candidate; only the journal is a sink.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let (event, result) = candidate.prepare_learned_step(actor_revision, position)?;
        let event = Event::Decoder(DecoderEvent::Learned(event));
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        match self.persist_candidate_retaining(event, bytes, candidate, result)? {
            (Transition::Learned(result), retired) => Ok((result, retired)),
            _ => unreachable!("learned numerical transition"),
        }
    }

    /// Resume only a completely reconstructed, originally quiet owner under a
    /// fresh caller-observed clock. This changes no numerical state, monitor,
    /// work allowance, effect epoch, old approval or unknown-effect liability.
    pub fn resume_learned_generation(&mut self, revision: u64, actor_revision: u64, position: u64)
        -> Result<(), JournalError>
    {
        self.transact(revision, Event::Decoder(DecoderEvent::Learned(
            LearnedEvent::Resume { actor_revision, position })))?;
        Ok(())
    }

    /// Supply the complete intended recipe INDEPENDENTLY of the archive. Generic
    /// open cannot hydrate this profile. Match before inference, cleanup or fence;
    /// then execute every original step and compare its exact saved witness.
    /// Recovery pauses inference and revokes old effect keys through the SAME
    /// original fence reducer as every other FileOversight profile. This is the
    /// synchronous consumer of the SAME bounded event-by-event recovery path.
    pub fn open_with_learned_generation(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileLearnedConfig) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let mut recovery = Self::begin_open_with_learned_generation(directory, profile, expected)?;
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = recovery.progress().replayed_events;
            recovery.advance(completed, 1)?;
        }
        recovery.finish()
    }

    /// Pure historical publication projection. Matching and replay return no
    /// numerical owner, live observation, helper session, human role or permit.
    pub fn read_publication_with_learned_generation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileLearnedConfig) -> Result<FileDeliverySnapshot, JournalError>
    {
        super::super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        bind_history(&mut events, expected)?;
        Ok(Machine::replay(profile, &events)?.snapshot(events.len()))
    }
}

pub(in super::super) fn bind_history(events: &mut [Event], expected: &FileLearnedConfig) -> Result<(), Error> {
    let mut found = false;
    for event in events {
        if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(config))) = event {
            if found { return Err(Error::Duplicate); }
            if config.bytes.as_ref() != expected.bytes().as_ref() { return Err(Error::Binding); }
            config.runtime = Some(Rc::new(expected.clone()));
            found = true;
        }
    }
    if !found { return Err(Error::Incomplete); }
    Ok(())
}

pub(in super::super) fn write(w: &mut Writer, event: &LearnedEvent) -> Result<(), Error> {
    match event {
        LearnedEvent::Checkpoint(event) => { w.u8(5)?; checkpoint::write(w, event)?; }
        LearnedEvent::Sidecar(event) => { w.u8(4)?; sidecar::write(w, event)?; }
        LearnedEvent::Begin(intent) => {
            w.u8(3)?; w.u64(intent.actor_revision)?; w.u64(intent.position)?;
        }
        LearnedEvent::Enable(config) => {
            check_config_bytes(&config.bytes)?;
            w.u8(0)?; w.blob(&config.bytes)?;
        }
        LearnedEvent::Step { actor_revision, position, witness } => {
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            w.u8(1)?; w.u64(*actor_revision)?; w.u64(*position)?; w.blob(witness)?;
        }
        LearnedEvent::Resume { actor_revision, position } => {
            w.u8(2)?; w.u64(*actor_revision)?; w.u64(*position)?;
        }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<LearnedEvent, Error> {
    Ok(match r.u8()? {
        0 => {
            let bytes = r.blob(config::MAX_CONFIG_BYTES)?;
            check_config_bytes(bytes)?;
            LearnedEvent::Enable(Configuration { bytes: Rc::from(bytes), runtime: None })
        }
        1 => {
            let actor_revision = r.u64()?; let position = r.u64()?;
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            LearnedEvent::Step { actor_revision, position, witness: Rc::from(witness) }
        }
        2 => LearnedEvent::Resume { actor_revision: r.u64()?, position: r.u64()? },
        3 => LearnedEvent::Begin(LearnedStepIntent { actor_revision: r.u64()?, position: r.u64()? }),
        4 => LearnedEvent::Sidecar(sidecar::read(r)?),
        5 => LearnedEvent::Checkpoint(checkpoint::read(r)?),
        _ => return Err(Error::InvalidInput),
    })
}
fn check_config_bytes(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > config::MAX_CONFIG_BYTES { return Err(Error::Limit); }
    if !bytes.starts_with(config::DOMAIN) { return Err(Error::InvalidInput); }
    Ok(())
}
