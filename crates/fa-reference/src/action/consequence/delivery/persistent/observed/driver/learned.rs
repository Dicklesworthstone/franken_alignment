//! Computed learned congress in the ORIGINAL actor/publication driver.
//! No second request book, numerical engine, vote importer or approval path.
pub mod evidence;
use super::{FileSupervisedDriver, FileDriverEvent, Job, Phase, admitted, observe, stage};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use super::super::helpers::learned::{FileLearnedProbeReview, FileLearnedProbeStatus};
use super::super::super::JournalError;
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::learned_host::sidecar::workers::{
    LearnedWorkerSchedule, probes::{ProbeReviewLimits, ProbeReviewMember},
};
use crate::action::{ActionState, ElapsedTick};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Supervisor-selected computation for an ALREADY submitted actor request.
/// The opaque plan supplies original source-bound input, not caller-made bytes.
pub struct FileDriverLearnedLaunch {
    pub request: u64,
    pub sidecar: FileLearnedSidecar,
    pub schedule: LearnedWorkerSchedule,
    pub members: BTreeMap<String, ProbeReviewMember>,
    pub limits: ProbeReviewLimits,
}

/// Local numerical work and acknowledged original round history stay owned
/// across success, cancellation, refusal and driver handoff. This is not a
/// durable physical-computation escrow or an independent helper qualification.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::driver::learned::RetainedReview;
/// fn replace_votes(saved: &mut RetainedReview) { saved.review_mut(); }
/// ```
pub struct RetainedReview {
    pub(super) request: u64,
    review: FileLearnedProbeReview,
}
impl RetainedReview {
    pub fn request(&self) -> u64 { self.request }
    pub fn review(&self) -> &FileLearnedProbeReview { &self.review }
}

/// Each advance either yields original bounded numerical/protocol progress or
/// returns the SAME applied/rejected event as the ordinary publication driver.
#[derive(Debug)]
pub enum FileDriverLearnedEvent {
    Progress { request: u64, round: u64, polls: usize, input_revision: u64 },
    Completed(FileDriverEvent),
}

impl FileSupervisedDriver {
    /// Keep this evidence until the consumer explicitly takes it, or return it
    /// with release(). Starting another computed review never silently evicts it.
    pub fn learned_probe_review(&self) -> Option<&RetainedReview> { self.learned_review.as_ref() }

    /// Extraction does not clear a request, refund work or restore a review.
    /// Running evaluators cannot be detached from the supervising driver.
    pub fn take_learned_probe_review(&mut self) -> Result<Option<RetainedReview>, Error> {
        if self.learned_review.as_ref().is_some_and(|saved|
            saved.review.status() == FileLearnedProbeStatus::Running) {
            return Err(Error::WrongState);
        }
        Ok(self.learned_review.take())
    }

    /// Bind the original actor request and current sidecar, then transfer the
    /// actual computed evaluator into this driver. No input overwrite or socket
    /// fallback occurs. The original numerical roster/schedule admission and
    /// durable Begin own their failure boundaries; no new permit is issued.
    pub fn start_learned_probe_review(&mut self, launch: FileDriverLearnedLaunch,
        snapshot: Snapshot, now: ElapsedTick) -> Result<(), JournalError>
    {
        if self.job.is_some() || self.learned_review.is_some() { return Err(Error::WrongState.into()); }
        self.ensure_child_slot()?;
        let mut host = self.supervisor.host_mut()?;
        let (attempt, state) = admitted(host.request_status(launch.request)?)?;
        if state != ActionState::Reviewing { return Err(Error::WrongState.into()); }
        if launch.sidecar.attempt() != attempt { return Err(Error::Binding.into()); }
        let action = host.request_action(launch.request)?.clone();
        if host.current_learned_sidecar(&launch.sidecar)?.action() != &action {
            return Err(Error::Binding.into());
        }
        if !snapshot.complete { return Err(Error::Incomplete.into()); }
        observe(&mut host, now)?;
        let revision = host.revision();
        let review = host.begin_learned_probe_review(revision, launch.sidecar,
            launch.schedule, launch.members, launch.limits, snapshot)?;
        self.job = Some(Job { issuer: Rc::clone(&host.issuer), request: launch.request,
            attempt, action, inputs: Some(review.input().clone()), input_revision: review.input_revision(),
            control_sequence: None, pool: None, permit: None, phase: Phase::Review });
        self.learned_review = Some(RetainedReview { request: launch.request, review });
        Ok(())
    }

    /// One original cooperative quantum; the native evaluator checks current
    /// source before scoring and journals its real commit/reveal/refinement.
    /// Refined bytes stay Reviewing until a fresh complete round judges them.
    /// Only original Continue can enter the existing mandatory two-key path.
    pub fn advance_learned_probe_review(&mut self, expected_revision: u64,
        now: ElapsedTick, snapshot: Snapshot) -> Result<FileDriverLearnedEvent, JournalError>
    {
        let saved = self.learned_review.as_ref().ok_or(Error::Missing)?;
        if expected_revision != saved.review.revision() { return Err(Error::Stale.into()); }
        if saved.review.status() != FileLearnedProbeStatus::Running { return Err(Error::WrongState.into()); }
        self.reap_helpers();
        let result = (|| {
            let saved = self.learned_review.as_mut().ok_or(Error::Missing)?;
            let job = self.job.as_mut().ok_or(Error::Missing)?;
            if job.request != saved.request || job.phase != Phase::Review
                || saved.review.status() != FileLearnedProbeStatus::Running {
                return Err(Error::WrongState.into());
            }
            let mut host = self.supervisor.host_mut()?;
            job.check_owner(&host)?;
            let advanced = saved.review.advance(&mut host, expected_revision, now, snapshot);
            if let Err(error) = advanced {
                if saved.review.status() != FileLearnedProbeStatus::Running { job.close(); }
                return Err(error);
            }
            // Copy only the ORIGINAL acknowledged plan revision, including any
            // residual bought by the completed round. No old input earns a key.
            job.inputs = Some(saved.review.input().clone());
            job.input_revision = saved.review.input_revision();
            if saved.review.status() == FileLearnedProbeStatus::Running {
                return Ok(FileDriverLearnedEvent::Progress { request: saved.request,
                    round: saved.review.current_round().round, polls: saved.review.polls(),
                    input_revision: saved.review.input_revision() });
            }
            let completed = match saved.review.history().last() {
                Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. }) => {
                    if receipt.policy.control.decision.consequence == Consequence::Continue {
                        job.control_sequence = Some(receipt.policy.control.sequence);
                        job.phase = Phase::Ready;
                    } else { job.close(); }
                    FileDriverEvent::ReviewApplied { request: saved.request, receipt: Box::new(receipt.clone()) }
                }
                Some(FileLearnedSidecarFinish::Applied { receipt: Err(error), .. }) => {
                    job.close();
                    FileDriverEvent::ReviewRejected { request: saved.request, error: *error }
                }
                _ => { job.close(); return Err(Error::Incomplete.into()); }
            };
            Ok(FileDriverLearnedEvent::Completed(completed))
        })();
        self.reap_helpers();
        if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
        result
    }

    pub(super) fn learned_review_deadline(&self) -> Option<ElapsedTick> {
        let saved = self.learned_review.as_ref()?;
        if saved.review.status() != FileLearnedProbeStatus::Running { return None; }
        let window = saved.review.current_round().window;
        let now = self.supervisor.host().ok()?.inspect().control.ledger.elapsed?;
        Some(if now < window.commit_by { window.commit_by } else { window.reveal_by })
    }

    /// Called by EVERY existing worker-maintenance path, including cancel,
    /// policy/source replacement, stop, terminal retirement and explicit release.
    /// Uses live original ledger state, never a historical receipt's cancellations.
    /// Cancelling local numerical custody neither cancels nor refunds the ledger.
    pub(super) fn maintain_learned_review(&mut self) {
        let Some(saved) = &mut self.learned_review else { return; };
        let active = self.job.as_ref().is_some_and(|job| job.request == saved.request
            && job.phase == Phase::Review && self.supervisor.host().is_ok_and(|host| {
                job.check_owner(&host).is_ok() && host.storage_failure().is_none()
                    && host.inspect().stop.is_none()
                    && stage(&host, job.request) == Ok(ActionState::Reviewing)
            }));
        if saved.review.status() == FileLearnedProbeStatus::Running && !active {
            let _ = saved.review.cancel(saved.review.revision());
        }
        // An interrupted advance can leave terminal native state while this
        // driver's phase was not updated. Never route that state to socket I/O.
        if saved.review.status() != FileLearnedProbeStatus::Running {
            if let Some(job) = &mut self.job {
                if job.request == saved.request && job.phase == Phase::Review { job.close(); }
            }
        }
    }
}

#[cfg(test)]
#[path = "learned_tests.rs"]
mod tests;
