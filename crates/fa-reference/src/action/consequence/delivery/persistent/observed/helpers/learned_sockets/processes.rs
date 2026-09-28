//! Original direct-child lifecycle coupled to a durable learned-source round.
//! Reuse the executable launcher, source-bound socket owner and witnessed finish.
use super::{FileLearnedSocketRound, FileLearnedSidecar, FileLearnedSidecarFinish,
    FileOversight, FileHelperFailure, HelperPump, JournalError, LearnedSocketStatus};
use super::super::processes::{FileHelperProcessError, FileHelperProcessLaunch,
    FileProcessFailure, HelperRoundAdmission};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{
    helper_processes::{HelperChildren, HelperProgram, ProcessStatus},
    helper_workers::{HelperLimits, HelperPhase},
    learned_host::sidecar::workers::LearnedWorkerRound,
};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Operator-owned launch configuration. Attempt and input revision are derived
/// from the sealed sidecar, never selected independently of the current source.
#[derive(Clone, Debug)]
pub struct LearnedProcessLaunch {
    pub round: LearnedWorkerRound,
    pub programs: BTreeMap<String, HelperProgram>,
    pub limits: HelperLimits,
}

/// One original learned round plus every direct child spawned for it. A process
/// exit is inspection data, never a commitment, reveal, approval or effect result.
/// Cleanup remains available after protocol failure, completion or cancellation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::processes::FileLearnedProcessRound;
/// fn bypass(run: &mut FileLearnedProcessRound) { run.replace_worker(); }
/// ```
#[derive(Debug)]
#[must_use = "retain the process owner and poll cleanup until every direct child is reaped"]
pub struct FileLearnedProcessRound {
    round: FileLearnedSocketRound,
    children: HelperChildren,
}

fn not_started(error: impl Into<FileProcessFailure>) -> FileHelperProcessError {
    FileHelperProcessError { failure: error.into(), admission: HelperRoundAdmission::NotStarted,
        children: None }
}

impl FileOversight {
    /// Check the source before the original durable Begin and again after OS
    /// launch. The existing launcher consumes Begin before spawning, returns
    /// partial children on ordinary failure and checks a fresh post-launch clock.
    /// No packet is sent here. A failed launch never enables manual-round fallback.
    pub fn begin_learned_process_round<F>(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        launch: LearnedProcessLaunch, snapshot: Snapshot, clock: F)
        -> Result<FileLearnedProcessRound, FileHelperProcessError>
    where F: FnMut() -> ElapsedTick {
        if revision != self.revision() { return Err(not_started(Error::Stale)); }
        self.current_learned_sidecar(&sidecar).map_err(not_started)?;
        let processes = self.begin_helper_processes(revision, FileHelperProcessLaunch {
            attempt: sidecar.attempt(), expected_input_revision: sidecar.input_revision(),
            round: launch.round.round, evidence_root: launch.round.evidence_root,
            window: launch.round.window, programs: launch.programs, limits: launch.limits,
        }, snapshot, clock)?;
        if let Err(error) = self.current_learned_sidecar(&sidecar) {
            return Err(FileHelperProcessError { failure: error.into(),
                admission: HelperRoundAdmission::Committed,
                children: Some(processes.into_children()) });
        }
        let (pool, children) = processes.into_running_parts();
        let workers = pool.statuses();
        let round = FileLearnedSocketRound { issuer: Rc::clone(&self.issuer), sidecar,
            pool: Some(pool), round: launch.round.round, revision: 0,
            status: LearnedSocketStatus::Running, failure: None,
            progress: HelperPump { io: BTreeMap::new(), workers }, connection_steps: 0, result: None };
        Ok(FileLearnedProcessRound { round, children })
    }
}

impl FileLearnedProcessRound {
    pub fn round(&self) -> u64 { self.round.round() }
    pub fn revision(&self) -> u64 { self.round.revision() }
    pub fn status(&self) -> LearnedSocketStatus { self.round.status() }
    pub fn failure(&self) -> Option<&JournalError> { self.round.failure() }
    pub fn progress(&self) -> &HelperPump { self.round.progress() }
    pub fn connection_steps(&self) -> u64 { self.round.connection_steps() }
    pub fn ready_to_finish(&self) -> bool { self.round.ready_to_finish() }
    pub fn next_deadline(&self) -> Option<ElapsedTick> { self.round.next_deadline() }
    pub fn outcome(&self) -> Option<&FileLearnedSidecarFinish> { self.round.outcome() }
    pub fn process_statuses(&self) -> BTreeMap<String, ProcessStatus> { self.children.statuses() }
    pub fn all_reaped(&self) -> bool { self.children.all_reaped() }

    /// Original bounded try_wait/kill operations only; no blocking wait or hidden
    /// reaper. A reaped child can still have buffered protocol bytes to consume.
    pub fn reap(&mut self) -> BTreeMap<String, ProcessStatus> {
        if self.status() != LearnedSocketStatus::Running {
            return self.children.request_stop_all();
        }
        for (member, status) in &self.round.progress().workers {
            if matches!(status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) {
                self.children.request_stop(member).expect("original complete child roster");
            }
        }
        self.children.reap()
    }

    pub fn pump(&mut self, host: &mut FileOversight, revision: u64, now: ElapsedTick)
        -> Result<HelperPump, FileHelperFailure>
    {
        self.pump_with_clock(host, revision, || now)
    }
    pub fn pump_with_clock<F>(&mut self, host: &mut FileOversight, revision: u64, clock: F)
        -> Result<HelperPump, FileHelperFailure>
    where F: FnMut() -> ElapsedTick {
        // Invalid owner/revision calls do not even poll child processes.
        self.round.bind(host, revision).map_err(|error| self.round.failed_pump(error))?;
        let mut guard = StopOnUnwind { round: &mut self.round, children: &mut self.children, armed: true };
        let result = guard.round.pump_with_clock(host, revision, clock);
        guard.armed = false;
        drop(guard);
        self.reap();
        result
    }

    pub fn finish(&mut self, host: &mut FileOversight, revision: u64, snapshot: Snapshot)
        -> Result<FileLearnedSidecarFinish, JournalError>
    {
        self.finish_bound(host, revision, false, snapshot)
    }
    fn finish_bound(&mut self, host: &mut FileOversight, revision: u64, refine: bool,
        snapshot: Snapshot) -> Result<FileLearnedSidecarFinish, JournalError>
    {
        self.round.bind(host, revision)?;
        let mut guard = StopOnUnwind { round: &mut self.round, children: &mut self.children, armed: true };
        let result = guard.round.finish_bound(host, revision, refine, snapshot).cloned();
        guard.armed = false;
        drop(guard);
        self.reap();
        result
    }

    /// Withdraw worker I/O, not the actor action or any dispatched obligation.
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        self.round.cancel(revision)?;
        self.children.request_stop_all();
        Ok(())
    }
    /// Transfer ONLY cleanup responsibility. The socket round is closed first;
    /// no port, live sidecar, manual ballot path or publication key is returned.
    pub fn into_children(mut self) -> HelperChildren {
        self.round.close();
        self.children.request_stop_all();
        self.children
    }
}

// A caught callback/journal unwind cannot retain usable sockets or leave the
// child owner inaccessible. The caller still holds it and can poll to reaping.
// Constructor unwinds retain the original launcher's best-effort Drop contract.
struct StopOnUnwind<'a> {
    round: &'a mut FileLearnedSocketRound,
    children: &'a mut HelperChildren,
    armed: bool,
}
impl Drop for StopOnUnwind<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.round.close();
            self.round.status = LearnedSocketStatus::Failed;
            self.round.failure.get_or_insert(Error::Incomplete.into());
            self.children.request_stop_all();
        }
    }
}
