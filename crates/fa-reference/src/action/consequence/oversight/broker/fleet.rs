//! Fleet stops reach the same delivery owner used by every oversight dispatch.
//! No human, helper, activation or forecast availability is needed to stop or
//! observe an existing authority. Existing settlement paths remain unchanged.

use super::OversightBroker;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::fleet::{
    DomainFenceAcknowledgment, DomainObservation, FleetCoordinator, FleetFence,
};
use crate::Error;

impl OversightBroker {
    pub fn join_fleet(
        &mut self, fleet: &mut FleetCoordinator, domain: u64,
        expected_revision: u64, lease_until: ElapsedTick,
    ) -> Result<(), Error> {
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        self.delivery.join_fleet(fleet, domain, expected_revision, lease_until)
    }

    pub fn fleet_floor(&self) -> Option<u64> { self.delivery.fleet_floor() }

    pub fn install_fleet_fence(
        &mut self, fence: &FleetFence, expected_sequence: u64, expected_epoch: u64,
    ) -> Result<DomainFenceAcknowledgment, Error> {
        let acknowledgment = self.delivery.install_fleet_fence(fence, expected_sequence, expected_epoch)?;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(acknowledgment)
    }

    pub fn fleet_observation(&self) -> Result<DomainObservation, Error> {
        self.delivery.fleet_observation()
    }
}
