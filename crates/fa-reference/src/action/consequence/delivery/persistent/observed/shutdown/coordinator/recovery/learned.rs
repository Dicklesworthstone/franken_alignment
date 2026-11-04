//! Registered fleet recovery through the ORIGINAL fully guarded learned cursor.
//! Durable visit intent precedes domain acquisition; no callback supplies a result.
use super::super::super::recovery::FileStopRecoveryProgress;
use super::*;
mod inputs;
mod resolution;
pub use inputs::{
    FileShutdownLearnedInputs, FileShutdownLearnedMember, FileShutdownLearnedPass,
    FileShutdownLearnedRequirements,
};
pub use resolution::FileShutdownLearnedVisitResolution;

#[derive(Clone, Copy)]
enum Phase {
    Combined,
    Admission,
    Drain,
}

impl FileShutdownCoordinator {
    /// Reconstruct the registered learned history once, under all independently
    /// supplied guards, and consume the original fixed combined stop. The newest
    /// registered whole prefix is admitted before event zero. Native time is
    /// requested only after Ready/canonical checks and only if drain needs it.
    pub fn recover_learned_and_drain_with_clock<C, P>(
        &mut self,
        revision: u64,
        binding: FileShutdownClockBinding,
        inputs: &FileShutdownLearnedInputs,
        max_events: usize,
        clock: C,
        checkpoint: P,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where
        C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        self.recover_learned_phase(
            revision,
            binding,
            inputs,
            max_events,
            Phase::Combined,
            Some(clock),
            checkpoint,
        )
    }

    /// Withdraw admission with only the native Stop/Fence, without a clock.
    /// The result preserves actual drain evidence, dispatches and liabilities;
    /// stopped admission is not a claim that external effects have settled.
    pub fn recover_learned_admission_stop<P>(
        &mut self,
        revision: u64,
        binding: FileShutdownClockBinding,
        inputs: &FileShutdownLearnedInputs,
        max_events: usize,
        checkpoint: P,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        self.recover_learned_phase(
            revision,
            binding,
            inputs,
            max_events,
            Phase::Admission,
            None::<fn(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>>,
            checkpoint,
        )
    }

    /// Drain only the existing matching Stop with one native StopProgress.
    /// A failed clock cannot undo admission withdrawal. Exact drained retries
    /// retain their real last sweep and require no clock or domain event.
    pub fn drain_learned_stopped_with_clock<C, P>(
        &mut self,
        revision: u64,
        binding: FileShutdownClockBinding,
        inputs: &FileShutdownLearnedInputs,
        max_events: usize,
        clock: C,
        checkpoint: P,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where
        C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        self.recover_learned_phase(
            revision,
            binding,
            inputs,
            max_events,
            Phase::Drain,
            Some(clock),
            checkpoint,
        )
    }

    fn recover_learned_phase<C, P>(
        &mut self,
        revision: u64,
        binding: FileShutdownClockBinding,
        inputs: &FileShutdownLearnedInputs,
        max_events: usize,
        phase: Phase,
        clock: Option<C>,
        mut checkpoint: P,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where
        C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        if self.unavailable {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision {
            return Err(Error::Stale.into());
        }
        if max_events == 0 {
            return Err(Error::InvalidInput.into());
        }
        if max_events > MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        let index = self
            .campaign
            .plan
            .domains
            .binary_search_by_key(&binding.domain, |domain| domain.id())
            .map_err(|_| Error::Missing)?;
        if self.campaign.plan.domains[index].clock_domain() != binding.clock_domain {
            return Err(Error::Binding.into());
        }
        inputs.check_registration(&self.campaign.plan.domains[index])?;
        let kind = match phase {
            Phase::Combined => ShutdownVisitKind::RecoverStoppedAtCompletion,
            Phase::Admission => ShutdownVisitKind::StopAdmission,
            Phase::Drain => ShutdownVisitKind::DrainStoppedAtCompletion,
        };
        let (index, attempt) = self.begin_registered_terminal(revision, binding.domain, kind)?;
        let result = (|| {
            let mut run = inputs.begin(&self.campaign, index)?;
            loop {
                let progress = run.progress();
                if !checkpoint(progress) {
                    return Err(JournalError::Unavailable);
                }
                if progress.status == FileStopRecoveryStatus::Ready {
                    break;
                }
                run.advance(progress.replayed_events, max_events)?;
            }
            let terminal = run.into_terminal(self.campaign.plan.operation)?;
            match phase {
                Phase::Combined => {
                    let clock = clock.ok_or(JournalError::Unavailable)?;
                    terminal.finish_registered_with_clock(
                        &mut self.campaign,
                        index,
                        attempt,
                        || clock(binding),
                    )
                }
                Phase::Admission | Phase::Drain => terminal.finish_registered_phase(
                    &mut self.campaign,
                    index,
                    attempt,
                    clock.map(|clock| move || clock(binding)),
                ),
            }
        })();
        self.finish(index, attempt, result)
    }
}
