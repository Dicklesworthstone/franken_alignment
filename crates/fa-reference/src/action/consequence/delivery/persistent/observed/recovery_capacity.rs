//! Same canonical recovery-space admission as the simpler persistent owner.
mod terminal;
use super::{BaseEvent, Event, FileOversight, JournalError};
use super::super::{JournalCapacity, RecoveryReserve};

impl FileOversight {
    /// Before any work, optionally alongside pre-proposal publication guarding.
    /// Mandatory human approval and all original authority rules stay unchanged.
    pub fn enable_recovery_reserve(&mut self, revision: u64, reserve: RecoveryReserve) -> Result<(), JournalError> {
        self.transact(revision, Event::Core(BaseEvent::ReserveRecovery(reserve)))?;
        Ok(())
    }
    pub fn journal_capacity(&self) -> Result<JournalCapacity, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let bytes = super::journal::encode(&self.profile, self.store.identity(), &self.events)?.len();
        let reserve = self.events.iter().find_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(reserve)) => Some(*reserve), _ => None,
        });
        Ok(JournalCapacity::new(self.profile.delivery.limits, reserve, self.events.len(), bytes))
    }

    /// Observe trusted time and reconcile ONE original attempt in one canonical
    /// replacement. This is query-only: it never publishes, seals an unexecuted
    /// effect, acquires review evidence, or issues an automatic/human permit.
    /// Unknown outcomes retain their original charges and remain recoverable.
    ///
    /// Both original records must fit before either is committed. A stale tick,
    /// invalid attempt or insufficient event/byte capacity cannot leave a lone
    /// Time record behind. Ordinary recovery does not spend a terminal reserve.
    /// An already-current live clock needs only the original Reconcile record;
    /// a recovered historical clock still requires an explicit Time record.
    ///
    /// An ambiguous replacement poisons this owner and returns no candidate
    /// receipt or refund. Inspection remains the last acknowledged cut; recovery
    /// must reopen the exclusive store to determine the actual durable outcome.
    pub fn reconcile_attempt_at(
        &mut self,
        revision: u64,
        attempt: u64,
        observed_tick: crate::action::ElapsedTick,
    ) -> Result<super::Reconciliation, JournalError> {
        use super::{JournalFailure, JournalIo, Machine, Transition, journal};
        use crate::Error;

        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision() {
            return Err(Error::Stale.into());
        }
        if self.clock_ready() && self.inspect().control.ledger.elapsed == Some(observed_tick) {
            return self.reconcile(revision, attempt);
        }
        // A fixed query-only sequence, not a public batch/event-import surface.
        let next = [
            Event::Core(BaseEvent::Time(observed_tick)),
            Event::Core(BaseEvent::Reconcile(attempt)),
        ];
        let count = self.events.len().checked_add(next.len()).ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        // Validate every encoded prefix, including ordinary/recovery-reserve
        // admission and byte limits, before native replay or external storage.
        let mut bytes = journal::encode_appended(
            &self.profile, self.store.identity(), &self.events, &next[0],
        )?;
        history.extend(self.events.iter().cloned());
        for (index, event) in next.iter().enumerate() {
            self.check_source_admission(event)?;
            if index != 0 {
                bytes = journal::encode_appended(
                    &self.profile, self.store.identity(), &history, event,
                )?;
            }
            history.push(event.clone());
        }
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let mut outcome = None;
        for event in &next {
            candidate.preflight_consistency(event)?;
            if let Transition::Reconciled(result) = candidate.apply(event)? {
                outcome = Some(result);
            }
        }
        let outcome = outcome.ok_or(Error::Incomplete)?;
        // Latch before replacement, including caught unwinds. Neither a newer
        // visible disk image nor a failed directory sync authorizes a refund.
        self.fault = Some(JournalFailure {
            operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other,
            replacement_may_be_visible: true,
        });
        if let Err(error) = self.store.replace(&bytes) {
            if let JournalError::Io(failure) = &error {
                self.fault = Some(failure.clone());
            }
            return Err(error);
        }
        for event in &next {
            self.source_operation_committed(event);
        }
        self.events = history;
        self.machine = candidate;
        self.fault = None;
        Ok(outcome)
    }
}
