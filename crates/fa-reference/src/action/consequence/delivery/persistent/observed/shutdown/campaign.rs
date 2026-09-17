//! At most one ORIGINAL per-domain mutation per cooperative advance.
use super::*;
use super::super::{BaseEvent, Event, Machine};

pub(super) struct Slot {
    pub(super) report: FileShutdownDomainReport,
    pub(super) head: Rc<[u8]>,
    pub(super) revision: usize,
}

/// Does not own or reopen domains, spawn work, wait for missing members, or
/// invoke caller callbacks. The caller supplies one reachable supervisor owner.
/// This is cooperative synchronous control, not a watchdog or an OS sandbox.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::FileShutdownCampaign;
/// fn duplicate(campaign: FileShutdownCampaign) { let _ = campaign.clone(); }
/// ```
pub struct FileShutdownCampaign {
    pub(super) plan: FileShutdownPlan,
    pub(super) slots: Vec<Slot>,
    pub(super) attempts: Vec<FileShutdownAttempt>,
    pub(super) head_bytes: usize,
}
impl fmt::Debug for FileShutdownCampaign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownCampaign").field("operation", &self.plan.operation)
            .field("domains", &self.slots.len()).field("attempts", &self.attempts.len())
            .finish_non_exhaustive()
    }
}
impl FileShutdownCampaign {
    pub(super) fn new(plan: FileShutdownPlan) -> Self {
        let slots = plan.domains.iter().map(|domain| Slot {
            report: FileShutdownDomainReport { domain: domain.id, scope: domain.scope(),
                clock_domain: domain.clock_domain(), registered_revision: domain.registered_revision(),
                last_observation: None, latest_succeeded: false },
            head: Rc::clone(&domain.anchor), revision: domain.revision,
        }).collect();
        let head_bytes = plan.domains.iter().map(|domain| domain.anchor.len()).sum();
        Self { plan, slots, attempts: Vec::new(), head_bytes }
    }
    pub fn plan(&self) -> &FileShutdownPlan { &self.plan }
    pub fn report(&self) -> FileShutdownReport {
        FileShutdownReport { operation: self.plan.operation,
            domains: self.slots.iter().map(|slot| slot.report.clone()).collect(),
            attempts: self.attempts.clone(), retained_head_bytes: self.head_bytes }
    }

    /// Stop first; a subsequent call drains through the ORIGINAL endpoint. A
    /// fully drained owner is only inspected, so exact repeats add no journal
    /// records. An unavailable member never prevents visiting another member.
    /// `at` is used only for draining, in the domain's registered clock system.
    pub fn advance(&mut self, domain: u64, host: &mut FileOversight, at: ElapsedTick)
        -> Result<FileShutdownObservation, JournalError>
    {
        let (index, attempt) = self.enter(domain, FileShutdownStep::InspectOwner)?;
        let result = self.advance_inner(index, attempt, host, at);
        self.complete(index, attempt, result)
    }

    /// Record an actual acquisition/connection failure supplied by the operator.
    /// This API can only WITHDRAW an aggregate success, never assert a stop.
    pub fn unavailable(&mut self, domain: u64, error: JournalError) -> Result<(), JournalError> {
        let (_, attempt) = self.enter(domain, FileShutdownStep::Unavailable)?;
        self.attempts[attempt].result = FileShutdownResult::Refused(error);
        Ok(())
    }

    fn advance_inner(&mut self, index: usize, attempt: usize, host: &mut FileOversight,
        at: ElapsedTick) -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let domain = self.plan.domains[index].clone();
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if host.store.identity() != domain.directory.as_path() { return Err(Error::Binding.into()); }
        // Use the actual owner's bootstrap too: same counters/path under another
        // supplied profile are not the registered authority/history.
        self.check_prefix(index, &host.profile, host.store.identity(), &host.events)?;
        let before = host.inspect();
        let (step, event) = match &before.stop {
            None => {
                let request = StopRequest { operation: self.plan.operation,
                    expected_control_sequence: before.control.sequence,
                    expected_authority_epoch: before.control.ledger.epoch };
                (FileShutdownStep::Stop(request), Some(Event::Core(BaseEvent::Stop(request))))
            }
            Some(receipt) => {
                if receipt.request().operation != self.plan.operation { return Err(Error::Binding.into()); }
                if host.stop_progress()?.drained() { (FileShutdownStep::InspectOwner, None) }
                else { (FileShutdownStep::Drain { clock_domain: domain.clock_domain(), at },
                    Some(Event::Core(BaseEvent::StopProgress(at)))) }
            }
        };
        self.attempts[attempt].step = step;
        // Bound retained comparison material BEFORE any native mutation. The
        // original encoder also enforces journal capacity and recovery reserves.
        let expected = match &event {
            Some(event) => journal::encode_appended(&host.profile, host.store.identity(), &host.events, event)?,
            None => journal::encode(&host.profile, host.store.identity(), &host.events)?,
        };
        self.check_head_size(index, expected.len())?;
        let expected_revision = host.events.len().checked_add(usize::from(event.is_some())).ok_or(Error::Limit)?;
        let expected: Rc<[u8]> = expected.into();
        let mut last_drain = self.slots[index].report.last_observation.as_ref()
            .and_then(|observation| observation.last_drain.clone());
        match step {
            FileShutdownStep::Stop(request) => { host.request_stop(host.revision(), request)?; }
            FileShutdownStep::Drain { at, .. } => {
                let sweep = host.progress_stop(host.revision(), at)?;
                last_drain = Some(FileShutdownDrain { journal_revision: host.revision(), sweep });
            }
            FileShutdownStep::InspectOwner => {}
            FileShutdownStep::Unavailable => return Err(Error::WrongState.into()),
        }
        // A successful original write is acknowledged before its result can be
        // reported. Never import or independently derive a native stop receipt.
        if host.events.len() != expected_revision { return Err(Error::Binding.into()); }
        let observation = observe(&domain, &host.machine, &host.events, FileShutdownSource::AcknowledgedOwner, last_drain)?;
        Ok((observation, expected, expected_revision))
    }

    pub(super) fn enter(&mut self, domain: u64, step: FileShutdownStep) -> Result<(usize, usize), JournalError> {
        let index = self.plan.domains.binary_search_by_key(&domain, |domain| domain.id).map_err(|_| Error::Missing)?;
        if self.attempts.len() >= self.plan.max_attempts { return Err(Error::Limit.into()); }
        self.attempts.try_reserve(1).map_err(|_| Error::Limit)?;
        let attempt = self.attempts.len();
        self.attempts.push(FileShutdownAttempt { domain, step, result: FileShutdownResult::Interrupted });
        self.slots[index].report.latest_succeeded = false;
        Ok((index, attempt))
    }
    pub(super) fn check_head_size(&self, index: usize, bytes: usize) -> Result<(), Error> {
        let total = self.head_bytes.checked_sub(self.slots[index].head.len())
            .and_then(|n| n.checked_add(bytes)).ok_or(Error::Limit)?;
        if total > self.plan.max_head_bytes { return Err(Error::Limit); }
        Ok(())
    }
    pub(super) fn check_prefix(&self, index: usize, profile: &FileOversightProfile,
        path: &Path, events: &[Event]) -> Result<(), Error>
    {
        let slot = &self.slots[index];
        if events.len() < slot.revision { return Err(Error::Stale); }
        if journal::encode(profile, path, &events[..slot.revision])?.as_slice() != slot.head.as_ref() {
            return Err(Error::Binding);
        }
        Ok(())
    }
    pub(super) fn complete(&mut self, index: usize, attempt: usize,
        result: Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>)
        -> Result<FileShutdownObservation, JournalError>
    {
        match result {
            Ok((observation, head, revision)) => {
                self.check_head_size(index, head.len())?;
                let retained = observation.clone();
                let recorded = Box::new(observation.clone());
                self.head_bytes = self.head_bytes - self.slots[index].head.len() + head.len();
                let slot = &mut self.slots[index];
                slot.head = head;
                slot.revision = revision;
                slot.report.last_observation = Some(retained);
                self.attempts[attempt].result = FileShutdownResult::Observed(recorded);
                slot.report.latest_succeeded = true;
                Ok(observation)
            }
            Err(error) => {
                self.attempts[attempt].result = FileShutdownResult::Refused(error.clone());
                Err(error)
            }
        }
    }
}

pub(super) fn observe(domain: &FileShutdownDomain, machine: &Machine, events: &[Event],
    source: FileShutdownSource, last_drain: Option<FileShutdownDrain>) -> Result<FileShutdownObservation, Error>
{
    if events.len() < domain.revision { return Err(Error::Stale); }
    let snapshot = machine.snapshot(events.len());
    let stop = if snapshot.stop.is_some() { Some(machine.broker.stop_progress()?) } else { None };
    let dispatches_since_registration = events[domain.revision..].iter().filter_map(|event| {
        if let Event::Dispatch(id, ..) = event { Some(*id) } else { None }
    }).collect();
    Ok(FileShutdownObservation { source, journal_revision: snapshot.revision,
        control_sequence: snapshot.control.sequence, authority_epoch: snapshot.control.ledger.epoch,
        executions: snapshot.executions, stop, last_drain, dispatches_since_registration })
}
