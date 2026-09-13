//! Same canonical recovery-space admission as the simpler persistent owner.
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
}
