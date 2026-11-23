//! Explicit use of a prepaid native successor after an acknowledged Hold.
//! No replacement evaluator, packet, window or numerical allowance is accepted.
use super::super::evidence::FileEvidenceReport;
use super::super::learned::evidence::policy_file_provider;
use super::super::{JournalError, Phase, observe, sample};
use super::{FileNativeDriverProgress, FileNativeSupervisedDriver, LearnedWorkerRound, NativeReviewStatus};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::FileConveneTicket;
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use std::rc::Rc;

mod reserved;

impl FileNativeSupervisedDriver {
    /// Observe the next originally provisioned round, while its original packet
    /// and acknowledged Hold remain current. This performs no source or clock
    /// acquisition and grants neither a new round nor an effect key.
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
        let (basis, blocked) = host.native_convene_status(self.request).ok()?;
        if blocked.is_some() || basis.held_round != self.review.round().round {
            return None;
        }
        self.review.check_held_successor(&host, basis.recorded_at?).ok()
    }

    /// Current eligibility to wait for the next fixed native slot. A live
    /// grant with no remaining operations can continue only its already queued
    /// exact successor. This observes no source/clock and spends no model work.
    pub fn has_fixed_successor(&self) -> bool {
        let Some(next) = self.held_successor_round() else {
            return false;
        };
        self.convene_custody.is_some()
            && self
                .supervisor()
                .has_fixed_native_convene_offer(self.request, next.round)
                .ok()
                == Some(true)
    }

    /// Consume only an independently queued request for this retained native
    /// owner and its next fixed slot. No polling path selects a successor itself.
    pub fn take_pending_convene(&self) -> Result<Option<FileConveneTicket>, JournalError> {
        let next = self.held_successor_round();
        let reserved = self.has_reserved_successor();
        if next.is_none() && !reserved {
            return Ok(None);
        }
        if self.convene_custody.is_none() {
            return Ok(None);
        }
        let ticket = self.driver.supervisor.take_convene(self.request)?;
        if ticket.as_ref().is_some_and(|ticket| {
            !ticket.is_native_learned()
                || match next {
                    Some(next) => {
                        ticket.is_reserved_native_learned() || ticket.round() != next.round
                    }
                    None => !ticket.is_reserved_native_learned(),
                }
        }) {
            return Err(Error::Binding.into());
        }
        Ok(ticket)
    }

    /// Reopen the original job with its next prepaid native roster, after a
    /// fresh policy-only file capture. The frozen packet, all work counters,
    /// full history and original windows survive. This starts no model quantum.
    /// An error spends the one-use ticket and cannot restore the old offer.
    pub fn convene_from_policy_file<S, F>(
        &mut self,
        source: &mut S,
        mut ticket: FileConveneTicket,
        mut clock: F,
    ) -> FileEvidenceReport<FileNativeDriverProgress>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let checked = self.check_native_convene_ticket(&ticket);
        if let Err(error) = checked {
            ticket.finish(None);
            return FileEvidenceReport {
                observations,
                source_updates,
                result: Err(error),
            };
        }
        let mut operation = NativeConveneOperation {
            owner: self,
            acknowledged: false,
        };
        let result = (|| {
            let owner = &mut *operation.owner;
            let job = owner.driver.job.as_ref().ok_or(Error::Missing)?;
            let mut host = owner.driver.supervisor.host_mut()?;
            // The original held basis was checked before any callback above.
            // Its exclusively owned topology refresh may itself journal a cut.
            match owner.driver.topology.as_mut() {
                Some(topology) => topology.refresh_for_review(
                    &mut host,
                    ticket.request(),
                    ticket.expected().attempt,
                )?,
                None if host.convening_requires_topology() => {
                    return Err(Error::Incomplete.into());
                }
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
            owner.review.convene_held(
                &mut host,
                revision,
                now,
                captured.evidence.snapshot,
            )?;
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
                match operation.owner.clear_native_convene_offer() {
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
        FileEvidenceReport {
            observations,
            source_updates,
            result,
        }
    }

    fn check_native_convene_ticket(&self, ticket: &FileConveneTicket) -> Result<(), JournalError> {
        if !ticket.belongs_to(&self.driver.supervisor)
            || !ticket.is_native_learned()
            || ticket.is_reserved_native_learned()
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
        Ok(())
    }

    pub(super) fn sync_native_convene_offer(&mut self) -> Result<(), JournalError> {
        let next = self.held_successor_round();
        if let Some(next) = next
            && self.supervisor().has_live_convening()?
        {
            let custody = self.convene_custody.get_or_insert_with(|| Rc::new(()));
            self.driver.supervisor.offer_native_convene(
                self.request,
                self.review.round().round,
                next,
                Rc::downgrade(custody),
            )
        } else if self.reserved_successor_ready() && self.supervisor().has_live_convening()? {
            let custody = self.convene_custody.get_or_insert_with(|| Rc::new(()));
            self.driver.supervisor.offer_reserved_native_convene(
                self.request,
                self.review.round().round,
                Rc::downgrade(custody),
            )
        } else {
            self.clear_native_convene_offer()
        }
    }

    pub(super) fn clear_native_convene_offer(&mut self) -> Result<(), JournalError> {
        self.convene_custody = None;
        self.driver.supervisor.clear_native_convene(self.request)
    }
}

/// A source/clock failure or unwind retires the retained unused native owners.
/// Their original reservations and records survive; no new ticket can revive it.
struct NativeConveneOperation<'a> {
    owner: &'a mut FileNativeSupervisedDriver,
    acknowledged: bool,
}
impl Drop for NativeConveneOperation<'_> {
    fn drop(&mut self) {
        if self.acknowledged {
            return;
        }
        if self.owner.review.status() == NativeReviewStatus::Running
            || self.owner.review.held_successor_round().is_some()
            || self.owner.review.reserved_successor_available()
        {
            let _ = self.owner.review.cancel(self.owner.review.revision());
        }
        if let Some(job) = &mut self.owner.driver.job {
            job.close();
        }
        let _ = self.owner.clear_native_convene_offer();
    }
}
