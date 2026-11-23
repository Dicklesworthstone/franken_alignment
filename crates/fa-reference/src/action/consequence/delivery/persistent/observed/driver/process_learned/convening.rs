//! Explicit continuation through the original fixed executable roster.
//! Queue admission grants no vote, source observation, process or effect key.
use super::super::evidence::FileEvidenceReport;
use super::super::learned::evidence::policy_file_provider;
use super::super::{JournalError, Phase, observe, sample};
use super::driving::journal_failure;
use super::{FileProcessDriverProgress, FileProcessSupervisedDriver, LearnedWorkerRound, unfinished};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::FileConveneTicket;
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use std::rc::Rc;

impl FileProcessSupervisedDriver {
    /// Observe the next original slot on the exact acknowledged Hold. Child
    /// cleanup may still be pending; this observes no source or live clock and
    /// cannot itself launch or transfer an executable owner.
    pub fn held_successor_round(&self) -> Option<LearnedWorkerRound> {
        let host = self.supervisor().host().ok()?;
        let job = self.driver.job.as_ref()?;
        if job.request != self.request
            || job.phase != Phase::Closed
            || job.input_revision != self.input_revision
            || job.inputs.as_ref() != Some(self.review.input())
            || job.check_owner(&host).is_err()
            || !self.driver.helpers_reaped()
            || (host.convening_requires_topology() && self.driver.topology.is_none())
        {
            return None;
        }
        let (basis, blocked) = host.learned_convene_status(self.request).ok()?;
        if blocked.is_some() || basis.held_round != self.review.round() {
            return None;
        }
        self.review.check_held_successor_basis(&host, basis.recorded_at?).ok()
    }

    /// Preserve a bounded wait while old direct children drain. An exhausted
    /// grant can serve only its already accepted exact Pending operation.
    pub fn has_fixed_successor(&self) -> bool {
        let Some(next) = self.held_successor_round() else {
            return false;
        };
        self.convene_custody.is_some()
            && self.supervisor()
                .has_fixed_process_convene_offer(self.request, next.round)
                .ok() == Some(true)
    }

    /// Leave an independently queued ticket untouched until every original
    /// direct child is confirmed reaped. This performs no source or clock I/O,
    /// spends no protocol poll and never treats a missing owner as cleanup.
    pub fn take_pending_convene(&self) -> Result<Option<FileConveneTicket>, JournalError> {
        let Some(next) = self.held_successor_round() else {
            return Ok(None);
        };
        if self.convene_custody.is_none()
            || !self.all_helpers_reaped()
            || !self.driver.helpers_reaped()
        {
            return Ok(None);
        }
        let ticket = self.driver.supervisor.take_convene(self.request)?;
        if ticket.as_ref().is_some_and(|ticket| {
            !ticket.is_process_learned() || ticket.round() != next.round
        }) {
            return Err(Error::Binding.into());
        }
        Ok(ticket)
    }

    /// Consume one independently selected fixed process slot, keeping the same
    /// actor, sidecar, executable inventory, windows and cumulative allowances.
    /// The predecessor must be reaped before topology/source acquisition, and
    /// the original launcher rechecks current time after spawning its children.
    /// Failure retains cleanup custody and spends this ticket without a retry.
    pub fn convene_from_policy_file<S, F>(
        &mut self,
        source: &mut S,
        mut ticket: FileConveneTicket,
        mut clock: F,
    ) -> FileEvidenceReport<FileProcessDriverProgress>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        if let Err(error) = self.check_process_convene_ticket(&ticket) {
            ticket.finish(None);
            return FileEvidenceReport { observations, source_updates, result: Err(error) };
        }
        let mut operation = ProcessConveneOperation { owner: self, acknowledged: false };
        let result = (|| {
            let owner = &mut *operation.owner;
            let job = owner.driver.job.as_ref().ok_or(Error::Missing)?;
            let mut host = owner.driver.supervisor.host_mut()?;
            // The ticket compared the complete held predecessor before this
            // exclusive topology refresh may append its own current cut.
            match owner.driver.topology.as_mut() {
                Some(topology) => topology.refresh_for_review(
                    &mut host, ticket.request(), ticket.expected().attempt,
                )?,
                None if host.convening_requires_topology() => return Err(Error::Incomplete.into()),
                None => {}
            }
            observe(&mut host, clock())?;
            let captured = sample(
                &mut host,
                job,
                &mut policy_file_provider(
                    Some((job.attempt, job.input_revision)),
                    source,
                    &mut observations,
                    &mut source_updates,
                ),
                &mut clock,
            )?;
            if let Some(error) = captured.failure {
                return Err(error.into());
            }
            if captured.evidence.inputs.as_ref() != job.inputs.as_ref() {
                return Err(Error::Stale.into());
            }
            let now = clock();
            observe(&mut host, now)?;
            let next = owner.review.check_held_successor(&host, now)?;
            if next.round != ticket.round() {
                return Err(Error::Binding.into());
            }
            let revision = owner.review.revision();
            owner.review.convene_held_with_clock(
                &mut host,
                revision,
                captured.evidence.snapshot,
                &mut clock,
            ).map_err(journal_failure)?;
            let opened = host.convene_opened(ticket.request(), ticket.round())?;
            let job = owner.driver.job.as_mut().ok_or(Error::Missing)?;
            job.control_sequence = None;
            job.permit = None;
            job.phase = Phase::Review;
            Ok(opened)
        })();
        let result = match result {
            Ok(opened) => {
                ticket.finish(Some(opened));
                match operation.owner.clear_process_convene_offer() {
                    Ok(()) => {
                        operation.acknowledged = true;
                        Ok(operation.owner.progress())
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => {
                ticket.finish(None);
                Err(error)
            }
        };
        drop(operation);
        FileEvidenceReport { observations, source_updates, result }
    }

    fn check_process_convene_ticket(&self, ticket: &FileConveneTicket) -> Result<(), JournalError> {
        if !ticket.belongs_to(&self.driver.supervisor)
            || !ticket.is_process_learned()
            || ticket.request() != self.request
            || self.convene_custody.is_none()
        {
            return Err(Error::Binding.into());
        }
        let host = self.supervisor().host()?;
        ticket.check(&host)?;
        let next = self.held_successor_round().ok_or(Error::WrongState)?;
        if next.round != ticket.round() || ticket.expected().input_revision != self.input_revision {
            return Err(Error::Binding.into());
        }
        if !self.all_helpers_reaped() || !self.driver.helpers_reaped() {
            return Err(Error::Incomplete.into());
        }
        self.review.check_held_successor(
            &host, ticket.expected().recorded_at.ok_or(Error::Incomplete)?,
        )?;
        Ok(())
    }

    pub(super) fn sync_process_convene_offer(&mut self) -> Result<(), JournalError> {
        if let Some(next) = self.held_successor_round()
            && self.supervisor().has_live_convening()?
        {
            let custody = self.convene_custody.get_or_insert_with(|| Rc::new(()));
            let weak = Rc::downgrade(custody);
            self.driver.supervisor.offer_process_convene(
                self.request, self.review.round(), next, weak,
            )
        } else {
            self.clear_process_convene_offer()
        }
    }

    pub(super) fn clear_process_convene_offer(&mut self) -> Result<(), JournalError> {
        self.convene_custody = None;
        self.driver.supervisor.clear_process_convene(self.request)
    }
}

/// Source/clock/launch failures cannot leave a restartable held roster. A
/// partially launched or uncertain cohort stays in its original review owner
/// for cleanup, with every earlier result and spent operation preserved.
struct ProcessConveneOperation<'a> {
    owner: &'a mut FileProcessSupervisedDriver,
    acknowledged: bool,
}
impl Drop for ProcessConveneOperation<'_> {
    fn drop(&mut self) {
        if self.acknowledged {
            return;
        }
        let _ = self.owner.clear_process_convene_offer();
        if unfinished(self.owner.review.status()) || self.owner.review.has_held_successor_custody() {
            let _ = self.owner.review.cancel(self.owner.review.revision());
        }
        if let Some(job) = &mut self.owner.driver.job {
            job.close();
        }
        self.owner.reap_helpers();
    }
}
