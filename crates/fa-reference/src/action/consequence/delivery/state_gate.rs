//! The original delivery owner rechecks its live captured policy-state source.
use super::DeliveryBroker;
use crate::action::ActionState;
use crate::action::consequence::oversight::policy_state::{
    CapturedSnapshot, PolicyStateCapture, PolicyStateWriter, StateCaptureStatus,
    StateEvent, StateLimits, StateSource,
};
use crate::{Error, Snapshot};

pub const MAX_POLICY_STATE_GENERATIONS: usize = 16;

#[derive(Debug)]
pub(super) struct CapturedStateGate {
    capture: PolicyStateCapture,
    generations: usize,
    minimum_semantic_epoch: Option<u64>,
}

/// Source replacement fences undecided work, not already admitted effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicySourceChange {
    pub previous: StateSource,
    pub current: StateSource,
    pub revocation_floor: u64,
    pub minimum_semantic_epoch: Option<u64>,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

impl DeliveryBroker {
    /// Mandatory once enabled. No actor/helper can retrieve the writer from the
    /// broker, disable this prerequisite, or pass a snapshot around its checks.
    pub fn enable_policy_state(
        &mut self, source: StateSource, limits: StateLimits,
    ) -> Result<PolicyStateWriter, Error> {
        if self.policy_state.is_some() { return Err(Error::Duplicate); }
        let inspection = self.inspect();
        if !inspection.ledger.stages.is_empty() || inspection.sequence != 0 { return Err(Error::WrongState); }
        if source.scope != self.scope { return Err(Error::Binding); }
        let (capture, writer) = PolicyStateCapture::new(source, limits)?;
        self.policy_state = Some(CapturedStateGate { capture, generations: 1, minimum_semantic_epoch: None });
        Ok(writer)
    }

    pub fn policy_state_status(&self) -> Option<StateCaptureStatus> {
        self.policy_state.as_ref().map(|gate| gate.capture.status())
    }

    pub fn capture_policy_state(&self) -> Result<CapturedSnapshot, Error> {
        let gate = self.policy_state.as_ref().ok_or(Error::Incomplete)?;
        let cut = gate.capture.capture()?;
        if gate.minimum_semantic_epoch.is_some_and(|floor| cut.snapshot().semantic_epoch < floor) {
            return Err(Error::Stale);
        }
        Ok(cut)
    }

    /// Explicit host-governed replacement, including repair after writer loss or
    /// a poisoned stream. A new generation begins unavailable and needs its own
    /// full image and closure. Old publishers can change only their retired source.
    pub fn replace_policy_state(
        &mut self, expected_generation: u64, expected_epoch: u64,
        next: StateSource, limits: StateLimits,
    ) -> Result<(PolicyStateWriter, PolicySourceChange), Error> {
        let gate = self.policy_state.as_ref().ok_or(Error::Incomplete)?;
        let previous = gate.capture.source();
        let inspection = self.inspect();
        if previous.generation != expected_generation || inspection.ledger.epoch != expected_epoch {
            return Err(Error::Stale);
        }
        if next.scope != self.scope || next.source != previous.source { return Err(Error::Binding); }
        if next.generation <= previous.generation { return Err(Error::Stale); }
        if gate.generations >= MAX_POLICY_STATE_GENERATIONS { return Err(Error::Limit); }
        let generations = gate.generations + 1;
        let observed_epoch = gate.capture.event(gate.capture.status().applied_through).map(|event| match event {
            StateEvent::Snapshot { semantic_epoch, .. } | StateEvent::Delta { semantic_epoch, .. } => semantic_epoch,
        });
        let minimum_semantic_epoch = gate.minimum_semantic_epoch.max(observed_epoch);
        let floor = inspection.ledger.epoch.checked_add(1).ok_or(Error::Overflow)?;
        let (capture, writer) = PolicyStateCapture::new(next, limits)?;
        let cancelled: Vec<_> = inspection.ledger.stages.iter().filter_map(|(id, stage)| {
            matches!(stage, ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized)
                .then_some(*id)
        }).collect();
        let change = PolicySourceChange { previous, current: next, revocation_floor: floor,
            minimum_semantic_epoch, cancelled, refunded_units: inspection.ledger.reserved };
        // All fallible configuration, generation, allocation and overflow checks
        // precede mutation. The original ledger implements revocation and refunds.
        self.controller.revoke_epoch()?;
        for id in &change.cancelled { self.controller.cancel(*id).expect("prevalidated undispatched attempt"); }
        self.policy_state = Some(CapturedStateGate { capture, generations, minimum_semantic_epoch });
        Ok((writer, change))
    }

    /// Exact historical state consumed by dispatch; later closure or source
    /// replacement never rewrites it. These bytes do not enter endpoint requests.
    pub fn delivery_policy_state(&self, attempt: u64) -> Result<Option<&CapturedSnapshot>, Error> {
        Ok(self.records.get(&attempt).ok_or(Error::Missing)?.policy_state.as_ref())
    }

    pub(super) fn check_policy_state(&self, supplied: &Snapshot) -> Result<Option<CapturedSnapshot>, Error> {
        if self.policy_state.is_none() { return Ok(None); }
        let cut = self.capture_policy_state()?;
        if cut.snapshot() != supplied { return Err(Error::Binding); }
        Ok(Some(cut))
    }
}
