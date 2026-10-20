//! The original two terminal phases with registered-head admission BEFORE writes.
use super::*;

impl FileStopRecovery {
    pub(in crate::action::consequence::delivery::persistent::observed::shutdown)
    fn finish_registered_phase<C>(self, campaign: &mut FileShutdownCampaign,
        index: usize, attempt: usize, clock: Option<C>)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    where C: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        let domain = &campaign.plan.domains[index];
        let mut observed = observe(domain, &self.machine, &self.events,
            FileShutdownSource::CanonicalImage, self.last_drain.clone())?;
        let request = self.stop_request(campaign.plan.operation)?;
        let drain = clock.is_some();
        let needed = if drain {
            let progress = observed.stop.as_ref().ok_or(Error::WrongState)?;
            usize::from(!progress.drained())
        } else if observed.stop.is_none() { 2 } else { 0 };
        let revision = self.events.len().checked_add(needed).ok_or(Error::Limit)?;
        if revision > self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let at = match clock {
            Some(clock) if needed != 0 => Some(clock()?),
            _ => None,
        };
        let tail = if let Some(at) = at { vec![Event::Core(BaseEvent::StopProgress(at))] }
            else if needed != 0 { vec![Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence)] }
            else { Vec::new() };
        let mut projected = self.events.clone();
        let mut expected = self.canonical.clone();
        for event in &tail {
            expected = journal::encode_appended(&self.profile, self.store.identity(), &projected, event)?;
            projected.push(event.clone());
        }
        campaign.check_head_size(index, expected.len())?;
        let head: Rc<[u8]> = expected.into();
        if let Some(at) = at {
            campaign.attempts[attempt].step = FileShutdownStep::Drain { clock_domain: domain.clock_domain(), at };
        } else if needed != 0 { campaign.attempts[attempt].step = FileShutdownStep::Stop(request); }
        if drain {
            let history = self.finish_stopped_drain(request, || at.ok_or(JournalError::Unavailable))?;
            let stopped = history.stopped();
            let state = stopped.snapshot();
            observed.journal_revision = state.revision;
            observed.control_sequence = state.control.sequence;
            observed.authority_epoch = state.control.ledger.epoch;
            observed.executions = state.executions;
            observed.stop = Some(stopped.progress().clone());
            if let FileStoppedRecovery::Advanced { sweep, .. } = stopped {
                observed.last_drain = Some(FileShutdownDrain { journal_revision: state.revision, sweep: sweep.clone() });
            }
        } else {
            let stopped = self.finish_admission_stop(request)?;
            let state = stopped.snapshot();
            observed.journal_revision = state.revision;
            observed.control_sequence = state.control.sequence;
            observed.authority_epoch = state.control.ledger.epoch;
            observed.executions = state.executions;
            observed.stop = Some(stopped.progress().clone());
            // Retain the prior native sweep and dispatch list; do not fabricate
            // an empty settlement receipt for a clock-free admission stop.
        }
        if observed.journal_revision != revision as u64 { return Err(Error::Binding.into()); }
        if needed != 0 { observed.source = FileShutdownSource::AcknowledgedOwner; }
        Ok((observed, head, revision))
    }
}
