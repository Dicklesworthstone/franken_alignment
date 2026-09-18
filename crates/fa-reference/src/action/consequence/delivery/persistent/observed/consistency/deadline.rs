//! A typed, source-independent scheduling boundary for a silent actor.
use super::{ConsistencyEvent, FileConsistencyObserver, FileOversight, JournalError, Transition};
use super::super::{Event, JournalFailure, JournalIo, Machine, journal};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::consistency::ConsistencyDeadline;
use crate::Error;
use std::io;
use std::rc::Rc;

impl FileOversight {
    /// Historical pending basis in this acknowledged owner, including one whose
    /// coverage is already lost. No saved tick is asserted to be current time.
    pub fn action_consistency_deadline(&self) -> Result<Option<ConsistencyDeadline>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.consistency_deadline()?)
    }
}
impl FileConsistencyObserver {
    /// Exact pending forecast and an explicit observation in the host's original
    /// clock domain. An early check writes nothing. A stale basis, foreign role,
    /// stale journal revision or backwards tick refuses before disabling the owner.
    /// A due check disables it BEFORE capacity/encoding/replay, then records time,
    /// coverage loss and original terminal cleanup in ONE canonical replacement.
    ///
    /// A deadline carries no effect rights.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::consequence::oversight::consistency::ConsistencyDeadline;
    /// use fa_reference::action::Permit;
    /// fn grant(timer: ConsistencyDeadline) -> Permit { timer }
    /// ```
    ///
    /// Outer Err is unacknowledged. Inner Err is an acknowledged native containment
    /// failure, with coverage still lost. An exact recorded retry returns its
    /// historical result without a write, renewed clock or additional stop. Later
    /// actor/effect states are not rewound. The original pending forecast remains
    /// unknown: no action sample, refusal record or endpoint receipt is invented.
    pub fn expire_forecast(&self, host: &mut FileOversight, revision: u64,
        expected: ConsistencyDeadline, at: ElapsedTick)
        -> Result<Result<bool, Error>, JournalError>
    {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if let Some(index) = host.events.iter().position(|event| matches!(event,
            Event::Consistency(ConsistencyEvent::Expire(original, observed)) if *original == expected && *observed == at)) {
            // Replay only immutable original inputs, never a live owner or file
            // endpoint. The selected image is already this owner's acknowledged
            // validated history, including the suffix beyond this original event.
            let mut original = Machine::replay(&host.profile, &host.events[..index])?;
            return match original.apply(&host.events[index])? {
                Transition::ConsistencyExpired(result) => Ok(result),
                _ => unreachable!("recorded deadline transition"),
            };
        }
        if !host.machine.broker.check_consistency_deadline(expected, at)? { return Ok(Ok(false)); }
        // Due silence is already known. Failure cannot expose an older quiet
        // image for new work. The same original persistence cut clears this latch.
        host.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        if host.events.len() >= host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let event = Event::Consistency(ConsistencyEvent::Expire(expected, at));
        let bytes = journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event)?;
        let mut candidate = Machine::replay(&host.profile, &host.events)?;
        let result = candidate.apply(&event)?;
        match host.persist_candidate(event, bytes, candidate, result)? {
            Transition::ConsistencyExpired(result) => Ok(result),
            _ => unreachable!("deadline transition"),
        }
    }
}

impl super::super::driver::FileSupervisedDriver {
    /// Schedule only unresolved, still-covered forecasts. A terminally lost lane
    /// has no new deadline; its historical pending basis remains inspectable on
    /// the original owner. No clock, source or helper is consulted by this query.
    pub fn next_consistency_deadline(&self) -> Result<Option<ConsistencyDeadline>, JournalError> {
        let host = self.supervisor().host()?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if !host.action_consistency_required() || host.action_consistency_snapshot()?.coverage_lost {
            return Ok(None);
        }
        host.action_consistency_deadline()
    }

    /// The observer remains separately owned. The host must call this at a
    /// scheduling boundary even when no actor bytes arrive; no task is spawned.
    /// Original helper maintenance runs on returned failures too. As with every
    /// privileged mutable host borrow, any unused intake snapshot is withdrawn.
    pub fn expire_consistency_forecast(&mut self, observer: &FileConsistencyObserver,
        expected: ConsistencyDeadline, at: ElapsedTick)
        -> Result<Result<bool, Error>, JournalError>
    {
        let result = (|| {
            let mut host = self.supervisor_mut().host_mut()?;
            let revision = host.revision();
            observer.expire_forecast(&mut host, revision, expected, at)
        })();
        self.reap_helpers();
        result
    }
}
