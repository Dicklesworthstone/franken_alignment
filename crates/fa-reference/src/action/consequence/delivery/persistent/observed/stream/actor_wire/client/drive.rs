//! Amortize persistence over bounded native progress, not unbounded socket work.
use super::*;

impl<S: Read + Write> FileActorConnection<S> {
    /// One write-ahead intent and one acknowledged result for a bounded quantum
    /// of the ORIGINAL client. No response/error escapes before the second save.
    /// At the first Blocked/Response/error the quantum ends; no retry is hidden.
    /// A crash spends the saved ceiling and leaves the exact command unresolved.
    /// Larger quanta trade less disk work for a larger conservative crash charge.
    pub fn drive(&mut self, revision: u64, budget: ClientDriveBudget)
        -> Result<ClientDriveReport, FileClientError>
    {
        self.check(revision)?;
        if budget.steps() == 1 {
            // Preserve original one-step behavior AND image bytes, including its
            // more precise reservation. No format migration is necessary here.
            let before = self.state()?.work();
            let steps = u64::from(self.phase()?.is_some());
            let progress = self.step(revision)?;
            let after = self.state()?.work();
            return Ok(ClientDriveReport { progress, steps, work: ClientIoWork {
                exchanges: after.exchanges - before.exchanges,
                calls: after.calls - before.calls,
                written_bytes: after.written_bytes - before.written_bytes,
                read_bytes: after.read_bytes - before.read_bytes,
            } });
        }
        let Some(ceiling) = self.client.as_ref().ok_or(JournalError::Unavailable)?
            .drive_ceiling(budget)? else {
                return self.client.as_mut().ok_or(JournalError::Unavailable)?.drive(budget).map_err(Into::into);
            };
        self.persist_quantum(Some(ceiling), Some(budget))?;
        self.owner.unavailable = true;
        let result = self.client.as_mut().ok_or(JournalError::Unavailable)?.drive(budget);
        #[cfg(test)]
        if let Some(stage) = self.completion_failure.take() { self.owner.store.fail_once(stage); }
        self.persist(None)?;
        result.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
