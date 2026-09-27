//! Reconstruct original source/provenance objects inside each private machine.
mod finish;

use super::{Machine, Transition};
use super::super::super::{Event, decoder::{DecoderEvent, learned::{LearnedEvent,
    sidecar::{FileSidecarSnapshot, SidecarEvent, validate_request}}}};
use crate::action::consequence::oversight::learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest};
use crate::Error;
use std::rc::Rc;

impl Machine {
    pub(in super::super::super) fn retained_learned_sidecar(&self, attempt: u64, journal_revision: u64)
        -> Result<FileSidecarSnapshot, Error>
    {
        let sidecar = self.learned.as_ref().ok_or(Error::Incomplete)?.sidecars
            .get(&attempt).ok_or(Error::Missing)?;
        Ok(FileSidecarSnapshot { journal_revision, attempt, actor_revision: sidecar.actor_revision(),
            input_revision: sidecar.input_revision(), packet: sidecar.round().clone() })
    }
    pub(in super::super::super) fn checked_learned_sidecar(&self, attempt: u64)
        -> Result<&LearnedSidecar, Error>
    {
        if !self.clock_ready || self.learned_paused() || self.pending_learned_step().is_some() {
            return Err(Error::Incomplete);
        }
        let original = self.learned.as_ref().ok_or(Error::Incomplete)?.sidecars.get(&attempt).ok_or(Error::Missing)?;
        self.broker.current_learned_sidecar(original)?;
        self.check_source_inputs(original.round().input())?;
        Ok(original)
    }

    pub(super) fn apply_learned_sidecar(&mut self, event: &SidecarEvent) -> Result<Transition, Error> {
        match event {
            SidecarEvent::Finish { .. } => self.apply_learned_sidecar_finish(event),
            SidecarEvent::Prepare { attempt, actor_revision, request } => {
                let revision = self.execute_learned_sidecar(*attempt, *actor_revision, request.clone())?;
                Ok(Transition::Inputs(revision))
            }
            SidecarEvent::Begin { attempt, actor_revision, request, expected_payload } => {
                let revision = self.execute_learned_sidecar(*attempt, *actor_revision, request.clone())?;
                if self.checked_learned_sidecar(*attempt)?.round().payload() != expected_payload.as_ref() {
                    return Err(Error::Binding);
                }
                Ok(Transition::Inputs(revision))
            }
        }
    }

    pub(in super::super::super) fn prepare_learned_sidecar(&mut self, attempt: u64,
        actor_revision: u64, request: LearnedSidecarRequest) -> Result<(SidecarEvent, u64), Error>
    {
        super::super::super::decoder::learned::sidecar::check_request(&request)?;
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Begin { attempt, actor_revision, request: request.clone(),
                expected_payload: Rc::from(&b""[..]) })));
        self.check_decoder_admission(&shape)?;
        self.check_consistency_route(&shape)?;
        let input_revision = self.execute_learned_sidecar(attempt, actor_revision, request.clone())?;
        let expected_payload = Rc::from(self.checked_learned_sidecar(attempt)?.round().payload());
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok((SidecarEvent::Begin { attempt, actor_revision, request, expected_payload }, input_revision))
    }

    fn execute_learned_sidecar(&mut self, attempt: u64, actor_revision: u64,
        request: LearnedSidecarRequest) -> Result<u64, Error>
    {
        if !self.clock_ready || self.learned_paused() || self.pending_learned_step().is_some() {
            return Err(Error::Incomplete);
        }
        if !self.learned_contract().is_some_and(|config| config.requires_sidecar()) {
            return Err(Error::Binding);
        }
        validate_request(attempt, &request)?;
        if self.learned.as_ref().ok_or(Error::Incomplete)?.sidecars.contains_key(&attempt) {
            return Err(Error::Duplicate);
        }
        let sidecar = self.broker.begin_learned_sidecar(attempt, actor_revision, request)?;
        self.check_source_inputs(sidecar.round().input())?;
        let input_revision = sidecar.input_revision();
        self.learned.as_mut().expect("checked original learned owner").sidecars.insert(attempt, sidecar);
        Ok(input_revision)
    }
}
