//! Original external-process learned review feeding the original actor job.
//! The existing learned process sequence owns launch, sockets, refinement and
//! child reaping. This adapter preserves the original publication gates.
use super::{FileDriverPhase, FileSupervisedDriver, Job, Phase, admitted, stage};
use super::super::{FileOversight, JournalError};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use super::super::helpers::processes::{FileHelperProcessError, FileProcessFailure, HelperRoundAdmission};
use super::super::helpers::learned_sockets::processes::sequence::{
    FileLearnedProcessReview, LearnedProcessLimits, LearnedProcessStatus,
};
use super::super::super::requests::actor::FileActorSupervisor;
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::helper_processes::{HelperChildren, HelperProgram, ProcessStatus};
use crate::action::consequence::oversight::learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule};
use crate::action::consequence::oversight::sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS;
use crate::action::{ActionState, ElapsedTick};
use crate::{Error, Snapshot};
use std::{collections::BTreeMap, fmt, rc::Rc};

mod driving;
pub use driving::FileProcessDriverEvent;

/// Every original round and executable roster is fixed before the first Begin.
/// Successors launch only after witnessed refinement and complete predecessor
/// reaping. The original process limits bound admitted direct-child starts.
pub struct FileProcessDriverLaunch {
    pub journal_revision: u64,
    pub request: u64,
    pub sidecar: FileLearnedSidecar,
    pub schedule: LearnedWorkerSchedule,
    pub programs: BTreeMap<u64, BTreeMap<String, HelperProgram>>,
    pub limits: LearnedProcessLimits,
}

/// Return the same gateway and every recoverable partial child owner. A failed
/// start may already have committed its original Begin; it is never rollback.
#[derive(Debug)]
#[must_use = "retain the driver and reap every returned direct child"]
pub struct FileProcessDriverSetupError {
    pub error: FileProcessFailure,
    pub admission: HelperRoundAdmission,
    pub driver: FileSupervisedDriver,
    pub children: Option<HelperChildren>,
}

/// Supervisor inspection, never an approval or effect receipt. Admitted starts
/// include future unstarted rosters; records distinguish actual process starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileProcessDriverProgress {
    pub request: u64,
    pub review_revision: u64,
    pub status: LearnedProcessStatus,
    pub polls: usize,
    pub admitted_starts: usize,
    pub input_revision: u64,
    pub phase: FileDriverPhase,
}

/// Sealed custody of one original review and the already-admitted actor job.
/// No caller-supplied vote, native fallback, replacement process or second
/// driver can be introduced while the external review is running or draining.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::driver::process_learned::FileProcessSupervisedDriver;
/// fn bypass(run: &mut FileProcessSupervisedDriver) { run.driver_mut(); }
/// ```
#[must_use = "advance or cancel, then retain the terminal review for direct-child cleanup"]
pub struct FileProcessSupervisedDriver {
    driver: FileSupervisedDriver,
    review: FileLearnedProcessReview,
    request: u64,
    input_revision: u64,
    rounds: Vec<LearnedWorkerRound>,
}
impl fmt::Debug for FileProcessSupervisedDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileProcessSupervisedDriver").field("progress", &self.progress())
            .field("all_helpers_reaped", &self.all_helpers_reaped()).finish_non_exhaustive()
    }
}

/// The terminal original review keeps process custody, observed exits and work
/// records. The original driver retains both-key, fresh-source and publication
/// checks. A returned driver may be Idle; handoff itself grants no authority.
#[derive(Debug)]
#[must_use = "retain original review custody and drive or release the original gateway"]
pub struct FileProcessDriverHandoff {
    pub driver: FileSupervisedDriver,
    pub review: FileLearnedProcessReview,
}

fn not_started(error: impl Into<FileProcessFailure>) -> FileHelperProcessError {
    FileHelperProcessError { failure: error.into(), admission: HelperRoundAdmission::NotStarted, children: None }
}
fn unfinished(status: LearnedProcessStatus) -> bool {
    matches!(status, LearnedProcessStatus::Running | LearnedProcessStatus::Draining)
}

impl FileSupervisedDriver {
    /// Bind the existing external learned process sequence to this gateway's
    /// admitted request. Original Begin precedes every process launch. The
    /// original launcher samples the clock again after launch before any input
    /// can be sent; callers must supply their current monotone supervisor clock.
    pub fn start_process_learned_sequence<F>(mut self, launch: FileProcessDriverLaunch,
        snapshot: Snapshot, clock: F)
        -> Result<FileProcessSupervisedDriver, Box<FileProcessDriverSetupError>>
    where F: FnMut() -> ElapsedTick {
        let request = launch.request;
        let result = (|| {
            if self.job.is_some() || self.learned_probe_review().is_some() {
                return Err(not_started(Error::WrongState));
            }
            self.ensure_child_slot().map_err(not_started)?;
            let mut host = self.supervisor.host_mut().map_err(not_started)?;
            if host.storage_failure().is_some() { return Err(not_started(JournalError::Unavailable)); }
            if launch.journal_revision != host.revision() { return Err(not_started(Error::Stale)); }
            let (attempt, current) = admitted(host.request_status(request).map_err(not_started)?)
                .map_err(not_started)?;
            if current != ActionState::Reviewing { return Err(not_started(Error::WrongState)); }
            if attempt != launch.sidecar.attempt() { return Err(not_started(Error::Binding)); }
            let action = host.request_action(request).map_err(not_started)?.clone();
            let input = host.current_learned_sidecar(&launch.sidecar).map_err(not_started)?;
            if input.action() != &action { return Err(not_started(Error::Binding)); }
            let input = input.clone();
            let input_revision = host.input_revision(attempt).map_err(not_started)?;
            if input_revision != launch.sidecar.input_revision() { return Err(not_started(Error::Stale)); }
            if !snapshot.complete { return Err(not_started(Error::Incomplete)); }
            if launch.schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS {
                return Err(not_started(Error::Limit));
            }
            let rounds = launch.schedule.rounds.clone();
            let revision = host.revision();
            let review = host.begin_learned_process_review(revision, launch.sidecar,
                launch.schedule, launch.programs, launch.limits, snapshot, clock)?;
            self.job = Some(Job { issuer: Rc::clone(&host.issuer), request, attempt, action,
                inputs: Some(input), input_revision, control_sequence: None, pool: None,
                permit: None, phase: Phase::Review });
            Ok((review, input_revision, rounds))
        })();
        match result {
            Ok((review, input_revision, rounds)) => Ok(FileProcessSupervisedDriver {
                driver: self, review, request, input_revision, rounds,
            }),
            Err(error) => Err(Box::new(FileProcessDriverSetupError { error: error.failure,
                admission: error.admission, driver: self, children: error.children })),
        }
    }
}

impl FileProcessSupervisedDriver {
    pub fn supervisor(&self) -> &FileActorSupervisor<FileOversight> { self.driver.supervisor() }
    /// Trusted host integration; this role must never be given to the actor.
    pub fn supervisor_mut(&mut self) -> &mut FileActorSupervisor<FileOversight> { self.driver.supervisor_mut() }
    pub fn review(&self) -> &FileLearnedProcessReview { &self.review }
    pub fn progress(&self) -> FileProcessDriverProgress {
        FileProcessDriverProgress { request: self.request, review_revision: self.review.revision(),
            status: self.review.status(), polls: self.review.polls(), admitted_starts: self.review.admitted_starts(),
            input_revision: self.input_revision, phase: self.driver.phase() }
    }
    pub fn child_statuses(&self) -> BTreeMap<u64, BTreeMap<String, ProcessStatus>> {
        self.review.records().iter().map(|(round, record)| (*round, record.processes.clone())).collect()
    }
    pub fn all_helpers_reaped(&self) -> bool { self.review.all_reaped() }
    /// Only bounded original OS cleanup. This spends no review poll allowance,
    /// can run after every terminal state and never launches a successor.
    pub fn reap_helpers(&mut self) {
        self.review.reap();
        self.driver.reap_helpers();
    }
    /// Withdraw worker I/O and request cleanup without settling the actor's
    /// request or refunding a dispatched/unknown external effect.
    pub fn request_stop_helpers(&mut self) {
        if unfinished(self.review.status()) { let _ = self.review.cancel(self.review.revision()); }
        if let Some(job) = &mut self.driver.job {
            if job.phase == Phase::Review { job.close(); }
        }
        self.reap_helpers();
    }

    /// A manual quantum with a fixed logical clock, useful for deterministic
    /// supervisors. Live operators use advance_with_clock or the policy loop.
    pub fn advance(&mut self, expected_revision: u64, now: ElapsedTick, snapshot: Snapshot)
        -> Result<FileProcessDriverProgress, FileProcessFailure>
    {
        self.advance_with_clock(expected_revision, || now, snapshot)
    }

    /// Advance only the original sequence. Its launcher, socket operations and
    /// acknowledged finish retain their fresh pre/post-I/O clock observations.
    /// Refinement never makes the previous input eligible for publication.
    pub fn advance_with_clock<F>(&mut self, expected_revision: u64, clock: F, snapshot: Snapshot)
        -> Result<FileProcessDriverProgress, FileProcessFailure>
    where F: FnMut() -> ElapsedTick {
        if expected_revision != self.review.revision() { return Err(Error::Stale.into()); }
        if !unfinished(self.review.status()) { return Err(Error::WrongState.into()); }
        {
            let mut host = self.driver.supervisor.host_mut()?;
            let job = self.driver.job.as_mut().ok_or(Error::Missing)?;
            job.check_owner(&host)?;
            if host.storage_failure().is_some() {
                self.review.cancel(expected_revision)?;
                job.close();
                return Err(JournalError::Unavailable.into());
            }
            let current = stage(&host, self.request)?;
            if current != ActionState::Reviewing || host.inspect().stop.is_some() {
                self.review.cancel(expected_revision)?;
                if matches!(current, ActionState::Dispatching | ActionState::Unknown) {
                    job.permit = None;
                    job.phase = Phase::Reconcile;
                } else { job.close(); }
            } else {
                if !snapshot.complete { return Err(Error::Incomplete.into()); }
                let result = self.review.advance_with_clock(&mut host, expected_revision, clock, snapshot);
                if self.review.status() == LearnedProcessStatus::Failed { job.close(); }
                if let Some(input_revision) = self.review.history().iter().rev().find_map(|event| {
                    match event {
                        FileLearnedSidecarFinish::Refined { input_revision, .. } => Some(*input_revision),
                        _ => None,
                    }
                }) { self.input_revision = input_revision; }
                result?;
                if unfinished(self.review.status()) && job.input_revision != self.input_revision {
                    job.inputs = Some(self.review.input().clone());
                    job.input_revision = self.input_revision;
                }
                if self.review.status() == LearnedProcessStatus::Finished {
                    job.close();
                    let Some(FileLearnedSidecarFinish::Applied { receipt, .. }) = self.review.history().last() else {
                        return Err(Error::Incomplete.into());
                    };
                    if let Ok(receipt) = receipt {
                        if receipt.policy.control.decision.consequence == Consequence::Continue {
                            if host.input_revision(job.attempt)? != self.input_revision
                                || host.inspect().control.sequence != receipt.policy.control.sequence
                                || self.review.input().action() != &job.action {
                                return Err(Error::Stale.into());
                            }
                            job.inputs = Some(self.review.input().clone());
                            job.input_revision = self.input_revision;
                            job.control_sequence = Some(receipt.policy.control.sequence);
                            job.phase = Phase::Ready;
                        }
                    }
                }
            }
        }
        self.reap_helpers();
        Ok(self.progress())
    }

    /// Cancel original worker custody and the original request. The original
    /// gateway preserves query-only reconciliation for unknown external effects.
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), JournalError> {
        if expected_revision != self.review.revision() { return Err(Error::Stale.into()); }
        if unfinished(self.review.status()) { self.review.cancel(expected_revision)?; }
        let result = self.driver.cancel_active().map(|_| ());
        self.reap_helpers();
        result
    }

    /// A partial or draining review cannot escape into an ordinary review loop.
    /// Terminal custody is retained even when direct-child cleanup is pending.
    pub fn into_handoff(mut self) -> Result<FileProcessDriverHandoff, Box<Self>> {
        if unfinished(self.review.status()) { return Err(Box::new(self)); }
        if let Some(job) = &mut self.driver.job {
            if job.phase == Phase::Review { job.close(); }
        }
        if self.driver.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.driver.job = None; }
        self.reap_helpers();
        Ok(FileProcessDriverHandoff { driver: self.driver, review: self.review })
    }
}
