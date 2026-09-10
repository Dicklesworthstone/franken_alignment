//! Enforce fleet admission state in the original delivery owner, not a wrapper.

use super::*;
use crate::action::consequence::delivery::DeliveryBroker;

impl DeliveryBroker {
    /// Join once before admitting any work. This transfers no rights and creates
    /// no controller: each domain keeps its original independently funded ledger.
    pub fn join_fleet(
        &mut self, fleet: &mut FleetCoordinator, domain: u64,
        expected_revision: u64, lease_until: ElapsedTick,
    ) -> Result<(), Error> {
        if self.fleet.is_some() { return Err(Error::Duplicate); }
        let inspection = self.inspect();
        if !inspection.ledger.stages.is_empty() || inspection.sequence != 0 { return Err(Error::WrongState); }
        if fleet.floor != 0 { return Err(Error::WrongState); }
        if expected_revision != fleet.revision { return Err(Error::Stale); }
        if domain == 0 { return Err(Error::InvalidInput); }
        if fleet.registrations.contains_key(&domain)
            || fleet.registrations.values().any(|r| r.scope == self.scope)
        { return Err(Error::Duplicate); }
        if fleet.registrations.len() >= fleet.max_domains { return Err(Error::Limit); }
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        fleet.clock.check_time(now)?;
        if lease_until <= now { return Err(Error::Stale); }
        if lease_until.0 - now.0 > fleet.max_lease_ticks { return Err(Error::Limit); }
        let revision = fleet.revision.checked_add(1).ok_or(Error::Overflow)?;
        let registration = Rc::new(Registration { domain, scope: self.scope,
            adapter: self.resource.adapter, contract_version: self.resource.contract_version, lease_until });
        self.fleet = Some(FleetDomain { issuer: Rc::clone(&fleet.issuer), clock: Rc::clone(&fleet.clock),
            registration: Rc::clone(&registration), installed: None, admissions: BTreeMap::new() });
        fleet.registrations.insert(domain, registration);
        fleet.clock.time.set(Some(now));
        fleet.revision = revision;
        Ok(())
    }

    pub fn fleet_floor(&self) -> Option<u64> {
        self.fleet.as_ref().map(|d| d.installed.as_ref().map_or(0, |a| a.transition.fleet_floor))
    }

    /// Stops new admissions, advances the actual revocation floor, cancels only
    /// undispatched work and returns its exact transition. Already sent messages
    /// may still execute: this is NOT an endpoint-drain acknowledgment.
    pub fn install_fleet_fence(
        &mut self, fence: &FleetFence, expected_sequence: u64, expected_epoch: u64,
    ) -> Result<DomainFenceAcknowledgment, Error> {
        let state = self.fleet.as_ref().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&state.issuer, &fence.0.issuer) { return Err(Error::Binding); }
        let target = fence.0.targets.get(&state.registration.domain).ok_or(Error::Binding)?;
        if !Rc::ptr_eq(target, &state.registration) { return Err(Error::Binding); }
        if let Some(previous) = &state.installed {
            if Rc::ptr_eq(&previous.command, &fence.0) { return Ok(previous.clone()); }
            if fence.floor() <= previous.transition.fleet_floor { return Err(Error::Stale); }
        }
        let clock = Rc::clone(&state.clock);
        let registration = Rc::clone(&state.registration);
        let installed_order = clock.next()?;
        let installed_at = clock.time.get().ok_or(Error::Incomplete)?;
        let local = self.controller.fence_authority(expected_sequence, expected_epoch, fence.floor())?;
        let acknowledgment = DomainFenceAcknowledgment {
            command: Rc::clone(&fence.0), registration,
            transition: DomainFenceTransition {
                domain: target.domain, fleet_floor: fence.floor(), installed_order, installed_at,
                control_sequence: local.sequence, previous_epoch: local.previous_epoch,
                revocation_floor: local.revocation_floor, cancelled: local.cancelled,
                refunded_units: local.refunded_units,
            },
        };
        self.fleet.as_mut().expect("enrolled domain").installed = Some(acknowledgment.clone());
        clock.order.set(installed_order);
        Ok(acknowledgment)
    }

    /// Explicit telemetry delivery is separate from command acknowledgment. An
    /// absent or old snapshot never establishes that no post-issue effects exist.
    pub fn fleet_observation(&self) -> Result<DomainObservation, Error> {
        let state = self.fleet.as_ref().ok_or(Error::Incomplete)?;
        let inspection = self.inspect();
        if self.records.len() != state.admissions.len() { return Err(Error::Incomplete); }
        let observed_at = state.clock.time.get().ok_or(Error::Incomplete)?;
        let through_order = state.clock.next()?;
        let mut dispatches = Vec::with_capacity(state.admissions.len());
        for (attempt, admission) in &state.admissions {
            let record = self.records.get(attempt).ok_or(Error::Missing)?;
            dispatches.push(FleetDispatchObservation {
                attempt: *attempt, admitted_order: admission.order, admitted_at: admission.at,
                state: *inspection.ledger.stages.get(attempt).ok_or(Error::Missing)?,
                outcome: record.resolution.as_ref().map(|receipt| receipt.outcome()),
            });
        }
        state.clock.order.set(through_order);
        Ok(DomainObservation { issuer: Rc::clone(&state.issuer), registration: Rc::clone(&state.registration),
            through_order, observed_at, dispatches })
    }

    pub(crate) fn check_fleet(&self) -> Result<(), Error> {
        let Some(state) = &self.fleet else { return Ok(()); };
        if state.installed.is_some() { return Err(Error::WrongState); }
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        state.clock.check_time(now)?;
        if now >= state.registration.lease_until { return Err(Error::Stale); }
        Ok(())
    }

    pub(crate) fn check_fleet_time(&self, now: ElapsedTick) -> Result<(), Error> {
        if let Some(state) = &self.fleet { state.clock.check_time(now)?; }
        Ok(())
    }

    pub(crate) fn publish_fleet_time(&self, now: ElapsedTick) {
        if let Some(state) = &self.fleet { state.clock.time.set(Some(now)); }
    }

    pub(crate) fn prepare_fleet_dispatch(&self, attempt: u64) -> Result<Option<(u64, ElapsedTick)>, Error> {
        self.check_fleet()?;
        let Some(state) = &self.fleet else { return Ok(None); };
        if state.admissions.contains_key(&attempt) { return Err(Error::Duplicate); }
        Ok(Some((state.clock.next()?, self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?)))
    }

    pub(crate) fn publish_fleet_dispatch(&mut self, attempt: u64, prepared: Option<(u64, ElapsedTick)>) {
        if let Some((order, at)) = prepared {
            let state = self.fleet.as_mut().expect("prepared enrolled dispatch");
            // The existing controller invokes no caller callbacks between staging
            // this order and consuming its permit. No other model step interleaves.
            state.admissions.insert(attempt, Admission { order, at });
            state.clock.order.set(order);
        }
    }
}
