//! Registered fleet visits consume the SAME original stop-only replay cursor.
//! Only the coordinator's admitted domain and newest whole prefix can enter.
use super::*;
use super::super::super::{FileShutdownCampaign, FileShutdownObservation, FileShutdownSource,
    FileShutdownStep, campaign::observe};
use std::rc::Rc;

mod phases;

impl FileStopRecovery {
    // Passive learned resolution releases only the actual original observation
    // after full numerical/guard checks and a canonical reread under its lock.
    // It performs no confirmation, cleanup, clock, fence or domain write.
    pub(in crate::action::consequence::delivery::persistent::observed::shutdown)
    fn finish_registered_observation(self, campaign: &FileShutdownCampaign, index: usize)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        self.check_ready()?;
        let domain = campaign.plan.domains.get(index).ok_or(Error::Missing)?;
        if self.store.identity() != domain.directory.as_path() {
            return Err(Error::Binding.into());
        }
        campaign.check_head_size(index, self.canonical.len())?;
        campaign.check_canonical_events(index, self.store.identity(), &self.events)?;
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let observation = observe(domain, &self.machine, &self.events,
            FileShutdownSource::CanonicalImage, self.last_drain)?;
        Ok((observation, self.canonical.into(), self.events.len()))
    }

    pub(in crate::action::consequence::delivery::persistent::observed::shutdown) fn begin_registered(
        store: storage::Store, campaign: &FileShutdownCampaign, index: usize,
    ) -> Result<Self, JournalError> {
        let domain = campaign.plan.domains.get(index).ok_or(Error::Missing)?;
        if store.identity() != domain.directory.as_path() { return Err(Error::Binding.into()); }
        let profile = (*domain.profile).clone();
        let canonical = store.read(profile.delivery.limits.bytes)?;
        campaign.check_head_size(index, canonical.len())?;
        let events = journal::decode(&profile, store.identity(), &canonical)?;
        // Exactly the passive admission used by canonical inspection, BEFORE
        // any historical event runs. No second open or observation is involved.
        campaign.check_canonical_events(index, store.identity(), &events)?;
        let machine = Machine::new(&profile)?;
        Ok(Self { profile, store, canonical, events, machine, last_drain: None,
            replayed: 0, attempted: 0, status: FileStopRecoveryStatus::Replaying })
    }

    pub(in crate::action::consequence::delivery::persistent::observed::shutdown) fn finish_registered(
        self, campaign: &mut FileShutdownCampaign, index: usize, attempt: usize, at: ElapsedTick,
    ) -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError> {
        self.finish_registered_with_clock(campaign, index, attempt, || Ok(at))
    }

    // A supplied-time visit keeps its old meaning. A clocked visit calls this
    // with a real fallible acquisition, after the entire original replay.
    pub(in crate::action::consequence::delivery::persistent::observed::shutdown) fn finish_registered_with_clock(
        self, campaign: &mut FileShutdownCampaign, index: usize, attempt: usize,
        clock: impl FnOnce() -> Result<ElapsedTick, JournalError>,
    ) -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError> {
        self.check_ready()?;
        let domain = &campaign.plan.domains[index];
        let before = observe(domain, &self.machine, &self.events,
            FileShutdownSource::CanonicalImage, self.last_drain.clone())?;
        let request = match &before.stop {
            Some(progress) => {
                if progress.receipt.request().operation != campaign.plan.operation {
                    return Err(Error::Binding.into());
                }
                if progress.drained() {
                    if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
                        return Err(Error::Binding.into());
                    }
                    // Reserve return storage before even the confirmation/cleanup.
                    let head: Rc<[u8]> = self.canonical.into();
                    self.store.confirm_and_cleanup()?;
                    return Ok((before, head, self.events.len()));
                }
                progress.receipt.request()
            }
            None => StopRequest { operation: campaign.plan.operation,
                expected_control_sequence: before.control_sequence,
                expected_authority_epoch: before.authority_epoch },
        };
        let expected_revision = self.events.len().checked_add(3).ok_or(Error::Limit)?;
        if expected_revision > self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        // Refuse changed canonical custody before invoking external time. The
        // original terminal finalizer compares again after clock and preflight.
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let at = clock()?;
        // Preserve the original BEFORE-effect aggregate evidence-space check,
        // in addition to every native journal prefix and recovery-reserve check.
        let mut projected = self.events.clone();
        let mut expected = Vec::new();
        for event in [Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence),
            Event::Core(BaseEvent::StopProgress(at))] {
            expected = journal::encode_appended(&self.profile, self.store.identity(), &projected, &event)?;
            projected.push(event);
        }
        campaign.check_head_size(index, expected.len())?;
        let head: Rc<[u8]> = expected.into();
        campaign.attempts[attempt].step = FileShutdownStep::Drain { clock_domain: domain.clock_domain(), at };
        // No historical event or numerical step is replayed again. The existing
        // finalizer rechecks the frozen canonical bytes before cleaning staging.
        let stopped = self.finish_stopped(request, || at)?;
        let FileStoppedRecovery::Advanced { snapshot, sweep } = stopped else { return Err(Error::Binding.into()); };
        if snapshot.revision != expected_revision as u64 { return Err(Error::Binding.into()); }
        let observation = FileShutdownObservation {
            source: FileShutdownSource::AcknowledgedOwner, journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence, authority_epoch: snapshot.control.ledger.epoch,
            executions: snapshot.executions, stop: Some(sweep.progress.clone()),
            last_drain: Some(FileShutdownDrain { journal_revision: snapshot.revision, sweep }),
            // The fixed terminal tail contains no dispatch event. Retain the
            // exact verified pre-tail dispatch list, not a fabricated empty one.
            dispatches_since_registration: before.dispatches_since_registration,
        };
        Ok((observation, head, expected_revision))
    }

    #[cfg(test)]
    pub(in crate::action::consequence::delivery::persistent::observed::shutdown) fn fail_registered_once(
        &self, barrier: crate::action::consequence::delivery::persistent::JournalIo,
    ) { self.store.fail_once(barrier); }
}
