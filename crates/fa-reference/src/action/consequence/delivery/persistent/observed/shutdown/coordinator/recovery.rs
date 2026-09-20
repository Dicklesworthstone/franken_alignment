//! Restart progress without a surviving in-memory domain owner. Coordinator
//! intent precedes domain I/O; the ORIGINAL registered history remains authority.
use super::*;
use super::super::campaign::observe;
use super::super::super::{BaseEvent, Event, Machine};

impl FileShutdownCoordinator {
    /// Recover and terminally drain ONE registered local domain. No arbitrary
    /// path/profile, caller-supplied receipt, helper or reviewer is accepted.
    /// The independently registered profile and newest acknowledged whole prefix
    /// are checked under the original exclusive Store lock BEFORE any mutation.
    /// Existing numerical-bootstrap admission is preserved by read_canonical.
    ///
    /// Durable visit intent comes first. Stop, fencing and drain then use the
    /// same FileOversight terminal cut; coordinator completion is a separate
    /// acknowledgment. An outer error may therefore follow a successful domain
    /// shutdown. Reopen the coordinator and inspect/recover again, never infer
    /// nonexecution or resend a publication from that error.
    ///
    /// An already drained, matching canonical stop is only reread and confirmed:
    /// it needs no new domain event, time observation, key or recovery capacity.
    /// Otherwise at must be fresh in the registered domain's clock system. All
    /// missing/busy/refused members remain in the original fixed denominator;
    /// other members may be visited after a durably recorded domain refusal.
    pub fn recover_and_drain(
        &mut self, revision: u64, domain: u64, at: ElapsedTick,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError> {
        let (index, attempt) = self.begin(revision, domain, ShutdownVisitKind::RecoverStopped { at })?;
        let result = self.recover_domain(index, attempt, at);
        self.finish(index, attempt, result)
    }

    fn recover_domain(&mut self, index: usize, attempt: usize, at: ElapsedTick)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let store = storage::Store::open(&self.campaign.plan.domains[index].directory)?;
        self.recover_locked(index, attempt, at, store)
    }

    // The private split permits deterministic Store barrier injection. There is
    // no external callback and no way for an actor to supply a replacement owner.
    fn recover_locked(&mut self, index: usize, attempt: usize, at: ElapsedTick, store: storage::Store)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let domain = self.campaign.plan.domains[index].clone();
        if store.identity() != domain.directory.as_path() { return Err(Error::Binding.into()); }
        // Reuse ALL passive checks, including the latest whole-prefix floor and
        // refusal to execute an unacknowledged numerical bootstrap. The same
        // cooperating lock stays held through this read and the final replacement.
        let (before, head, revision) = self.campaign.read_canonical(index)?;
        let request = match &before.stop {
            Some(progress) => {
                if progress.receipt.request().operation != self.campaign.plan.operation {
                    return Err(Error::Binding.into());
                }
                if progress.drained() {
                    store.confirm_and_cleanup()?;
                    return Ok((before, head, revision));
                }
                progress.receipt.request()
            }
            None => StopRequest { operation: self.campaign.plan.operation,
                expected_control_sequence: before.control_sequence,
                expected_authority_epoch: before.authority_epoch },
        };
        let expected_revision = revision.checked_add(3).ok_or(Error::Limit)?;
        if expected_revision > domain.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let events = journal::decode(&domain.profile, store.identity(), &head)?;
        let mut projected = events.clone();
        let mut expected = Vec::new();
        for event in [Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence),
            Event::Core(BaseEvent::StopProgress(at))] {
            expected = journal::encode_appended(&domain.profile, store.identity(), &projected, &event)?;
            projected.push(event);
        }
        // Comparison/evidence capacity cannot be discovered only AFTER stopping
        // the native owner. Its own encoder independently checks every prefix.
        self.campaign.check_head_size(index, expected.len())?;
        let expected: Rc<[u8]> = expected.into();
        let machine = Machine::replay(&domain.profile, &events)?;
        self.campaign.attempts[attempt].step = FileShutdownStep::Drain {
            clock_domain: domain.clock_domain(), at,
        };
        store.confirm_and_cleanup()?;
        let (mut host, _reviewer) = FileOversight::owner((*domain.profile).clone(), store, events, machine);
        let sweep = host.finish_stopped_recovery(request, at)?;
        if host.events.len() != expected_revision { return Err(Error::Binding.into()); }
        let observation = observe(&domain, &host.machine, &host.events,
            FileShutdownSource::AcknowledgedOwner,
            Some(FileShutdownDrain { journal_revision: host.revision(), sweep }))?;
        // The stopped owner and every process-local role are dropped here. Only
        // original observed evidence survives into the coordinator's completion.
        Ok((observation, expected, expected_revision))
    }
}

#[cfg(test)]
mod tests;
