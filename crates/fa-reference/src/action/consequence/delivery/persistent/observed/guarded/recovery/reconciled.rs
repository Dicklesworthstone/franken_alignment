//! Complete original guarded replay and reconcile existing deliveries in one cut.
//! A sweep queries the original endpoint; it cannot recreate sendable envelopes.
use super::{BaseEvent, Error, Event, FileGuardedRecovery, FileGuardedRecoveryStatus,
    FileOversight, FileOversightRoles, JournalError, Machine, Transition, journal};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::{JournalFailure, JournalIo,
    requests::PendingReconciliations};

/// The original owner and all separately held roles, released only after its
/// Fence/Time/Sweep replacement is acknowledged. Inspect every outcome: returning
/// an owner does not assert that all old liabilities have been resolved.
///
/// Source/identity/reviewer eligibility stays withdrawn by the original fence.
/// A fresh time observation is not fresh evidence or permission to dispatch.
#[must_use = "retain original reconciliation outcomes and separate privileged roles"]
pub struct ReconciledGuardedOwner {
    pub owner: FileOversight,
    pub roles: FileOversightRoles,
    pub outcomes: PendingReconciliations,
}

impl FileGuardedRecovery {
    /// Require a PREEXISTING actor request on the complete, independently
    /// verified history before spending any recovery fence. This does not expose
    /// a ticket, action bytes, result or authority, and does not validate a new
    /// proposal. The original actor gateway still checks exact retry equality.
    /// Missing never becomes permission for new admission during receipt recovery.
    pub fn require_existing_request(self, request: u64) -> Result<Self, JournalError> {
        match self.status {
            FileGuardedRecoveryStatus::Ready => {}
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        self.expected.check(&self.profile, &self.machine, &self.events)?;
        self.machine.requests.status(request)?;
        Ok(self)
    }

    /// Consume fully verified guarded replay into the ORIGINAL fixed
    /// Fence -> Time -> Sweep transaction, with one canonical replacement.
    /// There is no second replay, intermediate live owner, new observation of
    /// source evidence, caller-asserted settlement or effect resend.
    ///
    /// The independently selected guard inventory, effective policy, credential
    /// epoch and floors are checked again before the clock. An anchored cursor
    /// retains its exact original prefix. A changed canonical image refuses
    /// before the clock and is checked again before cleanup/replacement.
    /// Incomplete, failed and interrupted cursors return no owner or roles.
    ///
    /// The supplied trusted clock is called once, AFTER complete replay. The
    /// caller must enforce its deadline/cancellation at Ready before consuming
    /// the cursor. An unwinding clock drops the same locked cursor without
    /// cleaning staging, spending a fence or returning a candidate. No hard
    /// I/O, reducer or synchronization latency bound is claimed.
    ///
    /// All original prefix/byte limits apply. Time and Sweep still require
    /// ordinary capacity; this API never spends the terminal-only stop reserve
    /// for a live recovery. Use finish_stopped for terminal recovery at ordinary
    /// exhaustion. No recovery path enlarges or replenishes an allowance.
    ///
    /// Failed persistence can leave the complete replacement visible, but never
    /// yields a candidate owner, role or sweep. Recovery of that visible cut
    /// applies another ordinary fence and queries original receipts, not effects.
    /// Unlike an already-drained stop, live recovery is NOT a zero-event retry.
    pub fn finish_reconciled<F>(self, clock: F) -> Result<ReconciledGuardedOwner, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self.status {
            FileGuardedRecoveryStatus::Ready => {}
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        self.expected.check(&self.profile, &self.machine, &self.events)?;
        let count = self.events.len().checked_add(3).ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let now = clock();
        let Self { profile, store, canonical, events, mut machine, .. } = self;
        // The original full machine stays private. As in live finish(), an
        // empty placeholder allows only the acknowledged candidate to become
        // the owner; no historical or numerical event is executed twice.
        let placeholder = Machine::new(&profile)?;
        let (mut host, human) = FileOversight::owner(profile, store, events, placeholder);
        let next = [Event::Core(BaseEvent::Fence), Event::Core(BaseEvent::Time(now)),
            Event::Core(BaseEvent::Sweep)];
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        // Check the first appended prefix before retaining a cloned history.
        let mut bytes = journal::encode_appended(&host.profile, host.store.identity(),
            &host.events, &next[0])?;
        history.extend(host.events.iter().cloned());
        let mut outcomes = None;
        for (index, event) in next.iter().enumerate() {
            host.check_source_admission(event)?;
            if index != 0 {
                bytes = journal::encode_appended(&host.profile, host.store.identity(), &history, event)?;
            }
            machine.preflight_consistency(event)?;
            match (index, machine.apply(event)?) {
                (0 | 1, Transition::Unit) => {}
                (2, Transition::Swept(result)) => outcomes = Some(result),
                _ => return Err(Error::WrongState.into()),
            }
            history.push(event.clone());
        }
        let outcomes = outcomes.ok_or(Error::Incomplete)?;
        // Neither a failed clock/preflight nor a competing canonical image may
        // cause staging cleanup or overwrite the image with our earlier cut.
        if host.store.read(host.profile.delivery.limits.bytes)? != canonical {
            return Err(Error::Binding.into());
        }
        host.store.confirm_and_cleanup()?;
        host.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: true });
        // Self is consumed: any error or unwind drops the private candidate and
        // the locked owner. No old view or returned role can continue using it.
        host.store.replace(&bytes)?;
        for event in &next { host.source_operation_committed(event); }
        host.events = history;
        host.machine = machine;
        host.fault = None;
        let roles = FileOversightRoles::provision(&host, human);
        Ok(ReconciledGuardedOwner { owner: host, roles, outcomes })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "reconciled/request_tests.rs"]
mod request_tests;
