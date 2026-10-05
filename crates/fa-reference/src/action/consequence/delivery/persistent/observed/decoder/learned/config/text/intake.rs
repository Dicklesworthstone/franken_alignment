//! Source derivation for the original durable request book, not another ledger.
mod stream;
use super::{FileOversight, JournalError, ActionSpec, FrozenAction, VERSION};
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextProposal;
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Error;

impl FileOversight {
    /// The caller cannot install message bytes. Historical request binding is
    /// checked BEFORE live source/clock requirements so recovery can reacquire a
    /// ticket without resuming the generator or recreating either effect key.
    pub(in crate::action::consequence::delivery::persistent) fn prepare_learned_text_request(
        &self, request: u64, proposal: LearnedTextProposal,
    ) -> Result<ActionSpec, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        if !self.learned_text_required() || self.learned_text_stream_required() {
            return Err(Error::Binding.into());
        }
        let retained = self.machine.requests.original_spec(request);
        let payload = match retained {
            Some(spec) => spec.payload.clone(),
            None => {
                // These live-owner latches must not be mistaken for an older
                // quiet reconstructed machine after an interrupted operation.
                if self.source_interrupted || self.machine.pending_learned_reset().is_some() {
                    return Err(Error::Incomplete.into());
                }
                self.learned_text_message(LearnedEvidenceLimits::default())?.bytes().to_vec()
            }
        };
        let spec = ActionSpec { version: VERSION, scope: self.profile.delivery.scope,
            target: Some(proposal.target), payload, required_witnesses: Vec::new(),
            policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline,
            units: proposal.units };
        if retained.is_some_and(|original| original != &spec) { return Err(Error::Binding.into()); }
        if spec.payload.len() as u64 > spec.units { return Err(Error::Limit.into()); }
        FrozenAction::freeze(spec.clone())?;
        Ok(spec)
    }
}

#[cfg(test)]
mod tests;
