//! Capacity for acknowledging an existing pending visit, never a new command.
use super::*;
use super::super::super::codec;

impl FileShutdownCoordinator {
    pub(super) fn resolution_head_limit(&self, visit: usize, index: usize)
        -> Result<usize, JournalError>
    {
        let completed = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let mut visits = Vec::new();
        visits.try_reserve_exact(self.visits.len()).map_err(|_| Error::Limit)?;
        visits.extend(self.visits.iter().cloned());
        let retained = visits.get_mut(visit).ok_or(Error::Missing)?;
        if retained.result != ShutdownVisitResult::Entered { return Err(Error::WrongState.into()); }
        // Sizing only: no synthetic observation, revision or head is installed.
        // Visit identity/version and every other retained domain remain included.
        retained.result = ShutdownVisitResult::Observed {
            domain_revision: self.campaign.slots[index].revision as u64,
        };
        let image = codec::encode(self.store.identity(), &self.plan_bytes, self.max_bytes,
            completed, &visits, &self.campaign)?;
        // The original encoder uses fixed-width counts/revisions/lengths. Only
        // head payload bytes can change while the resolution holds this owner.
        let overhead = image.len().checked_sub(self.campaign.head_bytes).ok_or(Error::Binding)?;
        self.max_bytes.checked_sub(overhead).ok_or(Error::Limit.into())
    }
}

#[cfg(test)]
mod tests;
