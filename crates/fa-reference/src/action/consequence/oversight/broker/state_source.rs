//! Trusted state-source bootstrap and inspection, never exposed by ActorPort.
use super::OversightBroker;
use crate::action::consequence::delivery::PolicySourceChange;
use crate::action::consequence::oversight::policy_state::{
    CapturedSnapshot, PolicyStateWriter, StateCaptureStatus, StateFreshness, StateLimits, StateSource,
};
use crate::Error;

impl OversightBroker {
    pub fn enable_policy_state(&mut self, source: StateSource, limits: StateLimits) -> Result<PolicyStateWriter, Error> {
        self.delivery.enable_policy_state(source, limits)
    }
    pub fn enable_fresh_policy_state(
        &mut self, source: StateSource, limits: StateLimits, freshness: StateFreshness,
    ) -> Result<PolicyStateWriter, Error> {
        self.delivery.enable_fresh_policy_state(source, limits, freshness)
    }
    pub fn policy_state_freshness(&self) -> Option<StateFreshness> { self.delivery.policy_state_freshness() }
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

    /// A coupled two-key endpoint rechecks ORIGINAL evidence before its first
    /// execution. This read-only check cannot issue a key, refresh a review or
    /// settle an outcome. A historical terminal receipt must be handled first.
    pub(in crate::action::consequence) fn revalidate_publication(
        &self, attempt: u64, approval: crate::action::consequence::delivery::DispatchApproval,
        current: Option<&super::CommitteeInput>, snapshot: &crate::Snapshot,
    ) -> Result<(), Error> {
        self.delivery.check_not_stopping()?;
        if !snapshot.complete { return Err(Error::Incomplete); }
        let _ = self.delivery.check_policy_state(snapshot)?;
        self.check_approval(attempt, current)?;
        let request = self.human_request(approval.request())?;
        let status = self.human_status(approval.request())?;
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        if request.attempt() != attempt || request.action() != &slot.action
            || request.reviewer_id() != approval.reviewer()
            || status.issued_at != Some(approval.issued_at())
            || request.expires_at() != approval.expires_at()
            || status.disposition != super::human::HumanDisposition::Consumed
        { return Err(Error::Binding); }
        if request.input_revision() != slot.revision
            || slot.current.as_deref() != Some(request.inputs())
            || request.policy_generation() != self.delivery.controller().policy().generation()
        { return Err(Error::Stale); }
        self.delivery.controller().recheck_publication(attempt, request.control_sequence(), snapshot)
    }
}
