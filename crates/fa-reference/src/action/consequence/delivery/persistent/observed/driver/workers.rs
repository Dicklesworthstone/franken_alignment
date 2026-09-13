//! Preserve the original owned child cohort across completion and every failure.
use super::{FileDriverLaunch, FileSupervisedDriver, Job, Phase, prepare_review, stage};
use super::super::FileOversight;
use super::super::helpers::{FileHelperPool, FileHelperFailure};
use super::super::helpers::processes::{FileHelperProcesses, FileHelperProcessLaunch,
    FileProcessFailure, HelperRoundAdmission};
use super::super::super::JournalError;
use super::super::super::requests::actor::FileActorSupervisor;
use crate::action::consequence::oversight::{CommitteeInput, ObservedReceipt};
use crate::action::consequence::oversight::helper_processes::{HelperChildren, HelperProgram, ProcessStatus};
use crate::action::consequence::oversight::helper_workers::io::HelperPump;
use crate::action::{ActionState, ElapsedTick};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

pub(super) enum WorkerSet { Sockets(FileHelperPool), Processes(FileHelperProcesses) }
impl WorkerSet {
    pub(super) fn next_deadline(&self) -> Option<ElapsedTick> {
        match self { Self::Sockets(pool) => pool.next_deadline(), Self::Processes(pool) => pool.next_deadline() }
    }
    pub(super) fn ready_to_finish(&self) -> bool {
        match self { Self::Sockets(pool) => pool.ready_to_finish(), Self::Processes(pool) => pool.ready_to_finish() }
    }
    pub(super) fn is_closed(&self) -> bool {
        match self { Self::Sockets(pool) => pool.is_closed(), Self::Processes(pool) => pool.is_closed() }
    }
    pub(super) fn pump_with_clock<F>(&mut self, host: &mut FileOversight, clock: F) -> Result<HelperPump, FileHelperFailure>
    where F: FnMut() -> ElapsedTick {
        match self { Self::Sockets(pool) => pool.pump_with_clock(host, clock), Self::Processes(pool) => pool.pump_with_clock(host, clock) }
    }
    pub(super) fn finish(&mut self, host: &mut FileOversight, now: ElapsedTick,
        current: Option<&CommitteeInput>, snapshot: Snapshot) -> Result<Result<ObservedReceipt, Error>, JournalError>
    {
        match self {
            Self::Sockets(pool) => pool.finish(host, now, current, snapshot),
            Self::Processes(pool) => pool.finish(host, now, current, snapshot),
        }
    }
}

/// Diagnostics for a failed start. Any returned child ownership from the
/// ORIGINAL launcher is retained by this DRIVER, not discarded with this error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDriverProcessError {
    pub failure: FileProcessFailure,
    pub admission: HelperRoundAdmission,
}
impl From<JournalError> for FileDriverProcessError {
    fn from(error: JournalError) -> Self {
        Self { failure: FileProcessFailure::Journal(error), admission: HelperRoundAdmission::NotStarted }
    }
}

/// Explicit handoff retains both the original authority and child-reap duty.
/// Withdrawing the local job is not cancelling/refunding any ledger obligation.
#[must_use = "retain the supervisor and poll any children until they are reaped"]
pub struct FileDriverRelease {
    pub supervisor: FileActorSupervisor<FileOversight>,
    pub children: Option<HelperChildren>,
}

impl FileSupervisedDriver {
    /// Whole-roster admission and the original durable round precede spawning.
    /// All execution, inherited sockets and post-launch clock checks belong to
    /// begin_helper_processes. A partial start keeps its children here for reaping.
    pub fn start_process_review<F>(&mut self, launch: FileDriverLaunch<HelperProgram>,
        snapshot: Snapshot, mut clock: F) -> Result<(), FileDriverProcessError>
    where F: FnMut() -> ElapsedTick {
        let result = (|| {
            if self.job.is_some() { return Err(JournalError::from(Error::WrongState).into()); }
            self.ensure_child_slot()?;
            let mut host = self.supervisor.host_mut()?;
            let (attempt, action, input_revision) = prepare_review(&mut host, &launch, &snapshot, &mut clock)?;
            let revision = host.revision();
            let workers = host.begin_helper_processes(revision, FileHelperProcessLaunch {
                attempt, round: launch.round, evidence_root: launch.evidence_root, window: launch.window,
                expected_input_revision: input_revision, programs: launch.workers, limits: launch.limits,
            }, snapshot, clock);
            match workers {
                Ok(pool) => {
                    self.job = Some(Job { issuer: Rc::clone(&host.issuer), request: launch.request, attempt, action, inputs: Some(launch.inputs),
                        input_revision, control_sequence: None, pool: Some(WorkerSet::Processes(pool)),
                        permit: None, phase: Phase::Review });
                    Ok(())
                }
                Err(error) => {
                    self.retiring = error.children;
                    Err(FileDriverProcessError { failure: error.failure, admission: error.admission })
                }
            }
        })();
        self.reap_helpers();
        result
    }

    /// Direct-child state only. This never counts as an endpoint acknowledgment,
    /// detector result, descendant containment or proof of complete shutdown.
    pub fn helper_processes(&self) -> BTreeMap<String, ProcessStatus> {
        if let Some(WorkerSet::Processes(pool)) = self.job.as_ref().and_then(|job| job.pool.as_ref()) {
            return pool.process_statuses();
        }
        self.retiring.as_ref().map_or_else(BTreeMap::new, HelperChildren::statuses)
    }
    pub fn helpers_reaped(&self) -> bool {
        if let Some(WorkerSet::Processes(pool)) = self.job.as_ref().and_then(|job| job.pool.as_ref()) {
            return pool.all_reaped();
        }
        self.retiring.as_ref().is_none_or(HelperChildren::all_reaped)
    }

    /// One nonblocking maintenance pass, also before clock/provider operations.
    /// Original cancellation, stop, storage failure and terminal review outcomes
    /// retire the whole cohort, but leave the next Stopped/result event intact.
    pub fn reap_helpers(&mut self) -> BTreeMap<String, ProcessStatus> {
        let retire = self.job.as_ref().is_some_and(|job| {
            job.phase != Phase::Review || self.supervisor.host().map_or(true, |host| {
                job.check_owner(&host).is_err() || host.storage_failure().is_some() || host.inspect().stop.is_some()
                    || stage(&host, job.request).map_or(true, |state| !matches!(state, ActionState::Reviewing | ActionState::Authorized))
            })
        });
        if retire {
            let pool = self.job.as_mut().and_then(|job| job.pool.take());
            if let Some(WorkerSet::Processes(pool)) = pool {
                // Start is forbidden until the earlier cohort is fully reaped
                // and removed. Neither a failed start nor a finished job evicts it.
                assert!(self.retiring.is_none(), "one retained child cohort");
                self.retiring = Some(pool.into_children());
            }
        }
        if let Some(WorkerSet::Processes(pool)) = self.job.as_mut().and_then(|job| job.pool.as_mut()) { pool.reap(); }
        if let Some(children) = &mut self.retiring { children.request_stop_all(); children.reap(); }
        self.helper_processes()
    }

    pub(super) fn ensure_child_slot(&mut self) -> Result<(), JournalError> {
        self.reap_helpers();
        if !self.helpers_reaped() { return Err(Error::Incomplete.into()); }
        self.retiring = None;
        Ok(())
    }

    /// No hidden wait or thread. Stop helper I/O, return the SAME supervisor and
    /// retained direct children, and leave every effect disposition in the ledger.
    pub fn release(mut self) -> FileDriverRelease {
        if let Some(job) = &mut self.job { job.close(); }
        self.reap_helpers();
        self.job = None;
        FileDriverRelease { supervisor: self.supervisor, children: self.retiring }
    }
}
