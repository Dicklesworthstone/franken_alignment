//! Trusted state-source bootstrap and inspection, never exposed by ActorPort.
use super::OversightBroker;
use crate::action::consequence::delivery::PolicySourceChange;
use crate::action::consequence::oversight::policy_state::{
    CapturedSnapshot, PolicyStateWriter, StateCaptureStatus, StateLimits, StateSource,
};
use crate::Error;

impl OversightBroker {
    pub fn enable_policy_state(&mut self, source: StateSource, limits: StateLimits) -> Result<PolicyStateWriter, Error> {
        self.delivery.enable_policy_state(source, limits)
    }
    pub fn policy_state_status(&self) -> Option<StateCaptureStatus> { self.delivery.policy_state_status() }
    pub fn capture_policy_state(&self) -> Result<CapturedSnapshot, Error> { self.delivery.capture_policy_state() }
    pub fn delivery_policy_state(&self, attempt: u64) -> Result<Option<&CapturedSnapshot>, Error> {
        self.delivery.delivery_policy_state(attempt)
    }
    pub fn replace_policy_state(
        &mut self, expected_generation: u64, expected_epoch: u64, next: StateSource, limits: StateLimits,
    ) -> Result<(PolicyStateWriter, PolicySourceChange), Error> {
        let result = self.delivery.replace_policy_state(expected_generation, expected_epoch, next, limits)?;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(result)
    }
}
