//! Full-roster recovery with time acquired only at each native terminal cut.
//! Retained-visit resolution is still passive and never invokes the clock.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownClockedPassMember {
    pub binding: FileShutdownClockBinding,
    /// The actual successful callback result, even if native time validation
    /// subsequently refuses it. None means the callback returned no tick to
    /// the native consumer, never zero or proof that the callback was not called.
    pub acquired_tick: Option<ElapsedTick>,
    pub result: FileShutdownPassResult,
}

/// One pass over the complete registered roster. No fabricated pass-wide tick,
/// imported stop result or role is returned. An acquired tick is an observation,
/// not evidence that a domain or coordinator write was acknowledged.
#[must_use = "Inspect each result; Ok does not mean every domain drained."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownClockedRecoveryPass {
    operation: u64,
    initial_revision: u64,
    final_revision: u64,
    members: Vec<FileShutdownClockedPassMember>,
}
impl FileShutdownClockedRecoveryPass {
    pub fn operation(&self) -> u64 { self.operation }
    pub fn initial_revision(&self) -> u64 { self.initial_revision }
    pub fn final_revision(&self) -> u64 { self.final_revision }
    pub fn members(&self) -> &[FileShutdownClockedPassMember] { &self.members }
    pub fn unobserved_stops(&self) -> Vec<u64> {
        self.members.iter().filter(|member| !self.stopped(member))
            .map(|member| member.binding.domain).collect()
    }
    pub fn all_observed_stopped(&self) -> bool {
        !self.members.is_empty() && self.members.iter().all(|member| self.stopped(member))
    }
    pub fn all_observed_drained(&self) -> bool {
        self.all_observed_stopped() && self.members.iter().all(|member| {
            matches!(&member.result, FileShutdownPassResult::Observed(observation)
                if observation.stop.as_ref().is_some_and(|stop| stop.drained()))
        })
    }
    fn stopped(&self, member: &FileShutdownClockedPassMember) -> bool {
        matches!(&member.result, FileShutdownPassResult::Observed(observation)
            if observation.stop.as_ref().is_some_and(|stop| stop.receipt.request().operation == self.operation))
    }
}

impl FileShutdownCoordinator {
    /// New visits over the original full roster. Acquire bound time AFTER each
    /// domain's complete replay and canonical check, only when settlement needs
    /// it. Identity, allowance and quantum admission precede every callback.
    /// Clock refusals are recorded per member; a coordinator failure stops the
    /// pass. Cancellation leaves all later members explicitly NotVisited.
    pub fn recover_registered_pass_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, clock: C, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        self.run_registered_pass_with_clocks(revision, bindings, max_events, false, clock, checkpoint)
    }

    /// Resolve eligible retained visits without clock calls or new commands.
    /// Only members selected by the ORIGINAL resume selector for new recovery
    /// enter the clocked native path. Evidence refusal never falls through.
    /// Neither callback renews a visit budget or changes saved observation time.
    pub fn resume_registered_pass_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, clock: C, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        self.run_registered_pass_with_clocks(revision, bindings, max_events, true, clock, checkpoint)
    }

    fn run_registered_pass_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, resume: bool,
        mut clock: C, mut checkpoint: P) -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let (ordered, resolutions) = self.prepare_registered_members(revision, bindings, resume,
            |binding| (binding.domain, binding.clock_domain))?;
        let mut members = Vec::new();
        members.try_reserve_exact(ordered.len()).map_err(|_| Error::Limit)?;
        members.extend(ordered.into_iter().map(|binding| FileShutdownClockedPassMember {
            binding, acquired_tick: None, result: FileShutdownPassResult::NotVisited,
        }));
        let mut pass = FileShutdownClockedRecoveryPass { operation: self.campaign.plan.operation,
            initial_revision: revision, final_revision: revision, members };
        for (member, resolution) in pass.members.iter_mut().zip(resolutions) {
            let binding = member.binding;
            if !checkpoint(FileShutdownRecoveryCheckpoint { domain: binding.domain, replay: None }) { break; }
            let mut cancelled = false;
            if let Some(visit) = resolution {
                let result = (|| {
                    let mut read = self.begin_resolve_shutdown_visit(self.revision(), visit)?;
                    loop {
                        let progress = read.progress();
                        if !checkpoint(FileShutdownRecoveryCheckpoint { domain: binding.domain, replay: Some(progress) }) {
                            cancelled = true;
                            read.cancel()?;
                            return Err(JournalError::Unavailable);
                        }
                        if progress.status == FileStopRecoveryStatus::Ready { return read.finish(); }
                        if read.advance(progress.replayed_events, max_events).is_err() { return read.finish(); }
                    }
                })();
                match result {
                    Ok(observation) => member.result = FileShutdownPassResult::Observed(Box::new(observation)),
                    Err(error) if !self.unavailable => member.result = FileShutdownPassResult::EvidenceRefused(error),
                    Err(error) => { member.result = FileShutdownPassResult::Unacknowledged(error); break; }
                }
            } else {
                let result = self.recover_and_drain_with_clock(self.revision(), binding, max_events,
                    |actual| {
                        let at = clock(actual)?;
                        member.acquired_tick = Some(at);
                        Ok(at)
                    }, |progress| {
                        let proceed = checkpoint(FileShutdownRecoveryCheckpoint { domain: binding.domain, replay: Some(progress) });
                        if !proceed { cancelled = true; }
                        proceed
                    });
                match result {
                    Ok(Ok(observation)) => member.result = FileShutdownPassResult::Observed(Box::new(observation)),
                    Ok(Err(error)) => member.result = FileShutdownPassResult::Refused(error),
                    Err(error) => { member.result = FileShutdownPassResult::Unacknowledged(error); break; }
                }
            }
            if cancelled { break; }
        }
        pass.final_revision = self.revision();
        Ok(pass)
    }
}

#[cfg(test)]
mod tests;
