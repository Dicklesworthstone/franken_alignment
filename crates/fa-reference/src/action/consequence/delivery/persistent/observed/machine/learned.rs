//! Original learned source, actor synchronization, and policy gate in RAM replay.
//! Expected witnesses only reject divergence; no saved state is ever installed.
mod witness;
mod sidecar;
mod checkpoint;

use super::{Machine, Transition};
use super::super::{BaseEvent, Event};
use super::super::decoder::{DecoderEvent, learned::{FileLearnedConfig, LearnedEvent, LearnedStepIntent}};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;
use std::rc::Rc;

pub(super) struct LearnedState {
    config: Rc<FileLearnedConfig>,
    paused: bool,
    pending: Option<LearnedStepIntent>,
    checkpoints: checkpoint::CheckpointHistory,
    sidecar_outcomes: std::collections::BTreeMap<u64, super::super::decoder::learned::sidecar::FileLearnedSidecarOutcome>,
    sidecars: std::collections::BTreeMap<u64, crate::action::consequence::oversight::learned_host::sidecar::LearnedSidecar>,
}

impl Machine {
    pub(in super::super) fn learned_contract(&self) -> Option<&FileLearnedConfig> {
        self.learned.as_ref().map(|state| state.config.as_ref())
    }
    pub(in super::super) fn learned_paused(&self) -> bool {
        self.learned.as_ref().is_some_and(|state| state.paused)
    }
    pub(super) fn pause_learned(&mut self) {
        if let Some(state) = &mut self.learned { state.paused = true; }
    }
    pub(in super::super) fn pending_learned_step(&self) -> Option<LearnedStepIntent> {
        self.learned.as_ref().and_then(|state| state.pending)
    }
    pub(in super::super) fn preflight_learned_step(&self, revision: u64, position: u64) -> Result<(), Error> {
        let pending = self.pending_learned_step().ok_or(Error::Incomplete)?;
        if pending != (LearnedStepIntent { actor_revision: revision, position }) { return Err(Error::Binding); }
        self.check_learned_position(revision, position)
    }
    fn check_learned_position(&self, revision: u64, position: u64) -> Result<(), Error> {
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        if state.paused || !self.clock_ready { return Err(Error::Incomplete); }
        let actual = self.broker.hosted_learned_generation()?;
        if actual.actor_revision != revision || actual.position != position { return Err(Error::Stale); }
        revision.checked_add(1).ok_or(Error::Overflow)?;
        if actual.host_failure.is_some() || !actual.status.is_active() || self.broker.inspect().suspended
            || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        Ok(())
    }
    pub(super) fn apply_learned(&mut self, event: &LearnedEvent) -> Result<Transition, Error> {
        match event {
            LearnedEvent::Checkpoint(event) => self.apply_learned_checkpoint(event),
            LearnedEvent::Sidecar(event) => self.apply_learned_sidecar(event),
            LearnedEvent::Begin(intent) => {
                if self.pending_learned_step().is_some() { return Err(Error::Duplicate); }
                self.check_learned_position(intent.actor_revision, intent.position)?;
                self.learned.as_mut().expect("checked learned source").pending = Some(*intent);
                Ok(Transition::Unit)
            }
            LearnedEvent::Enable(config) => {
                if self.decoder.is_some() || self.learned.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || !self.containment.updates.is_empty() || !self.containment.checkpoints.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let config = config.runtime()?;
                config.install(&mut self.broker)?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.learned = Some(LearnedState { config, paused: false, pending: None, checkpoints: checkpoint::CheckpointHistory::default(), sidecar_outcomes: std::collections::BTreeMap::new(), sidecars: std::collections::BTreeMap::new() });
                Ok(Transition::Unit)
            }
            LearnedEvent::Step { actor_revision, position, witness } => {
                let result = self.execute_learned_step(*actor_revision, *position)?;
                if self.learned_witness(&result)?.as_slice() != witness.as_ref() { return Err(Error::Binding); }
                Ok(result)
            }
            LearnedEvent::Resume { actor_revision, position } => {
                if !self.clock_ready { return Err(Error::Incomplete); }
                let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
                if !state.paused { return Err(Error::WrongState); }
                let actual = self.broker.hosted_learned_generation()?;
                if actual.actor_revision != *actor_revision || actual.position != *position { return Err(Error::Stale); }
                if actual.host_failure.is_some() || self.broker.inspect().suspended
                    || self.broker.stop_receipt().is_some()
                    || !(actual.status.is_active() || matches!(actual.status, GenerationStatus::Finished(_)))
                    || !matches!(actual.availability, LearnedAvailability::Empty | LearnedAvailability::Ready)
                { return Err(Error::WrongState); }
                self.learned.as_mut().expect("checked learned source").paused = false;
                Ok(Transition::Unit)
            }
        }
    }
    pub(in super::super) fn prepare_learned_step(&mut self, revision: u64, position: u64)
        -> Result<(LearnedEvent, Transition), Error>
    {
        self.check_decoder_admission(&Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step {
            actor_revision: revision, position, witness: Rc::from(&b""[..]),
        })))?;
        let result = self.execute_learned_step(revision, position)?;
        let witness = self.learned_witness(&result)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok((LearnedEvent::Step { actor_revision: revision, position, witness: witness.into() }, result))
    }
    fn execute_learned_step(&mut self, revision: u64, position: u64) -> Result<Transition, Error> {
        self.preflight_learned_step(revision, position)?;
        let result = self.broker.advance_hosted_learned(revision, position);
        // Preflight excluded an existing stop. Reuse the original idempotent
        // durable manual-stop cleanup if the original numerical call installed it.
        if let Some(receipt) = self.broker.stop_receipt() {
            let request = receipt.request();
            self.apply_core(&BaseEvent::Stop(request))?;
        }
        self.learned.as_mut().expect("checked learned source").pending = None;
        Ok(Transition::Learned(result))
    }
}
