//! Retain original sidecar owners alongside the original learned state.
use super::{Machine, Transition};
use super::super::super::decoder::learned::sidecar::{FileSidecarSnapshot, SidecarEvent, validate_request};
use crate::action::consequence::oversight::learned_host::sidecar::LearnedSidecar;
use crate::Error;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Sidecars {
    // Bounded by the original attempt and cumulative helper-input capacities.
    owners: BTreeMap<u64, LearnedSidecar>,
}

impl Machine {
    pub(super) fn apply_learned_sidecar(&mut self, event: &SidecarEvent) -> Result<Transition, Error> {
        if !self.clock_ready || self.learned_paused() || self.pending_learned_step().is_some() {
            return Err(Error::Incomplete);
        }
        // The persistent path deliberately requires the immutable recipe flag.
        // Legacy profiles keep their explicit existing input behavior.
        if !self.learned_contract().is_some_and(|config| config.requires_sidecar()) {
            return Err(Error::Binding);
        }
        match event {
            SidecarEvent::Prepare { attempt, actor_revision, request } => {
                validate_request(*attempt, request)?;
                let sidecar = self.broker.begin_learned_sidecar(*attempt, *actor_revision, request.clone())?;
                self.check_source_inputs(sidecar.round().input())?;
                let revision = sidecar.input_revision();
                self.learned.as_mut().ok_or(Error::Incomplete)?.sidecars.owners.insert(*attempt, sidecar);
                Ok(Transition::Inputs(revision))
            }
        }
    }

    pub(in super::super::super) fn retained_learned_sidecar(&self, attempt: u64, journal_revision: u64)
        -> Result<FileSidecarSnapshot, Error>
    {
        let sidecar = self.learned.as_ref().ok_or(Error::Incomplete)?.sidecars.owners
            .get(&attempt).ok_or(Error::Missing)?;
        Ok(FileSidecarSnapshot { journal_revision, attempt, actor_revision: sidecar.actor_revision(),
            input_revision: sidecar.input_revision(), packet: sidecar.round().clone() })
    }
}
