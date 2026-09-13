//! Direct helper-process ownership for the durable full-input congress.
//! Reuse the existing executable launcher, socket transport and child reaper.
//! Programs are operator supplied; an exit code is never an accepted vote.
use super::{FileHelperFailure, FileHelperPool, FileOversight, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{CommitteeInput, ObservedReceipt, ReviewWindow};
use crate::action::consequence::oversight::helper_processes::{
    HelperChildren, HelperProgram, ProcessFailure, ProcessStatus, launch_helpers,
};
use crate::action::consequence::oversight::helper_workers::{HelperLimits, HelperPhase, HelperStatus};
use crate::action::consequence::oversight::helper_workers::coordinator::Coordinator;
use crate::action::consequence::oversight::helper_workers::io::{HelperPump, WorkerIoError};
use crate::action::consequence::oversight::helper_workers::wire::encode_request;
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

pub struct FileHelperProcessLaunch {
    pub attempt: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub programs: BTreeMap<String, HelperProgram>,
    pub limits: HelperLimits,
}

/// Admission of THIS launch, not an effect outcome or a process cleanup claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperRoundAdmission { NotStarted, Committed, Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileProcessFailure {
    Journal(JournalError),
    Transport(WorkerIoError),
    Launch { member: Option<String>, failure: ProcessFailure },
}
impl From<JournalError> for FileProcessFailure {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}
impl From<Error> for FileProcessFailure {
    fn from(error: Error) -> Self { Self::Journal(error.into()) }
}

/// A failed start can retain live children. Their original owner is returned,
/// with stop requested, for nonblocking polling until every child is reaped.
#[derive(Debug)]
#[must_use = "retain and reap any returned child owner; failed launch is not atomic"]
pub struct FileHelperProcessError {
    pub failure: FileProcessFailure,
    pub admission: HelperRoundAdmission,
    pub children: Option<HelperChildren>,
}
fn failed(failure: impl Into<FileProcessFailure>, admission: HelperRoundAdmission,
    mut children: Option<HelperChildren>) -> FileHelperProcessError
{
    if let Some(children) = &mut children { children.request_stop_all(); }
    FileHelperProcessError { failure: failure.into(), admission, children }
}

/// One nonblocking review pool and every direct child launched for it. There is
/// no process restart, replacement vote, alternative executor or child-to-effect
/// capability. The host must keep polling cleanup, including after an I/O fault.
#[derive(Debug)]
#[must_use = "retain this owner and poll cleanup until all direct children are reaped"]
pub struct FileHelperProcesses {
    pool: FileHelperPool,
    children: HelperChildren,
}

impl FileOversight {
    /// Reserve the original durable round BEFORE spawning any child. That round
    /// remains consumed if a later spawn or post-launch clock check fails. No
    /// input bytes are sent before the entire roster is ready and the fresh
    /// post-launch observation remains before the commit cutoff.
    pub fn begin_helper_processes<F>(&mut self, revision: u64, launch: FileHelperProcessLaunch,
        snapshot: Snapshot, mut clock: F) -> Result<FileHelperProcesses, FileHelperProcessError>
    where F: FnMut() -> ElapsedTick {
        let pending = (|| -> Result<_, JournalError> {
            if self.fault.is_some() { return Err(JournalError::Unavailable); }
            if revision != self.revision() { return Err(Error::Stale.into()); }
            if self.worker_rounds.contains(&launch.round) { return Err(Error::Duplicate.into()); }
            if !launch.programs.keys().eq(self.machine.broker.contracts().members().keys()) { return Err(Error::Binding.into()); }
            if self.machine.broker.input_revision(launch.attempt)? != launch.expected_input_revision { return Err(Error::Stale.into()); }
            let now = clock();
            // The process-independent clock domain is the same one already
            // bound to this file profile. A caller closure is not a worker clock.
            if !self.clock_ready() || self.inspect().control.ledger.elapsed != Some(now) {
                self.observe_time(self.revision(), now)?;
            }
            let input = self.machine.broker.current_inputs(launch.attempt)?.ok_or(Error::Incomplete)?;
            let (coordinator, ports) = Coordinator::new(launch.round, launch.evidence_root, input,
                launch.window, now, launch.limits)?;
            for port in ports.values() { encode_request(port)?; }
            Ok((input.clone(), coordinator, ports))
        })().map_err(|error| failed(error, HelperRoundAdmission::NotStarted, None))?;
        let (inputs, coordinator, ports) = pending;
        if let Err(error) = self.begin_review(self.revision(), launch.attempt, launch.round,
            launch.evidence_root, launch.window, snapshot)
        {
            let admission = if self.storage_failure().is_some() { HelperRoundAdmission::Unknown }
                else { HelperRoundAdmission::NotStarted };
            return Err(failed(error, admission, None));
        }
        self.worker_rounds.insert(launch.round);
        let (streams, children) = match launch_helpers(self.machine.broker.contracts(), &launch.programs) {
            Ok(launched) => launched,
            Err(error) => return Err(failed(FileProcessFailure::Launch {
                member: error.member, failure: error.failure,
            }, HelperRoundAdmission::Committed, Some(error.children))),
        };
        let now = clock();
        if let Err(error) = self.observe_time(self.revision(), now) {
            return Err(failed(error, HelperRoundAdmission::Committed, Some(children)));
        }
        if now >= launch.window.commit_by {
            return Err(failed(Error::Stale, HelperRoundAdmission::Committed, Some(children)));
        }
        let connections = match super::connect(ports, streams) {
            Ok(connections) => connections,
            Err(error) => return Err(failed(FileProcessFailure::Transport(error),
                HelperRoundAdmission::Committed, Some(children))),
        };
        let pool = FileHelperPool { issuer: Rc::clone(&self.issuer), attempt: launch.attempt,
            round: launch.round, inputs, window: launch.window, coordinator, connections, closed: false };
        Ok(FileHelperProcesses { pool, children })
    }
}

impl FileHelperProcesses {
    pub fn round(&self) -> u64 { self.pool.round() }
    pub fn statuses(&self) -> BTreeMap<String, HelperStatus> { self.pool.statuses() }
    pub fn process_statuses(&self) -> BTreeMap<String, ProcessStatus> { self.children.statuses() }
    pub fn all_reaped(&self) -> bool { self.children.all_reaped() }
    pub fn ready_to_finish(&self) -> bool { self.pool.ready_to_finish() }
    pub fn next_deadline(&self) -> Option<ElapsedTick> { self.pool.next_deadline() }
    pub fn is_closed(&self) -> bool { self.pool.is_closed() }

    /// Exit status alone does not fail or complete a helper slot: buffered
    /// commitment/reveal bytes must still pass the original socket decoder.
    pub fn reap(&mut self) -> BTreeMap<String, ProcessStatus> {
        if self.pool.is_closed() { self.children.request_stop_all(); }
        else {
            for (member, status) in self.pool.statuses() {
                if matches!(status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) {
                    // The roster is immutable and matches the original child map.
                    self.children.request_stop(&member).expect("frozen child roster");
                }
            }
        }
        self.children.reap()
    }

    pub fn pump(&mut self, host: &mut FileOversight, now: ElapsedTick) -> Result<HelperPump, FileHelperFailure> {
        self.pump_with_clock(host, || now)
    }
    pub fn pump_with_clock<F>(&mut self, host: &mut FileOversight, clock: F) -> Result<HelperPump, FileHelperFailure>
    where F: FnMut() -> ElapsedTick {
        self.reap();
        let result = self.pool.pump_with_clock(host, clock);
        self.reap();
        result
    }
    pub fn finish(&mut self, host: &mut FileOversight, now: ElapsedTick,
        current: Option<&CommitteeInput>, snapshot: Snapshot)
        -> Result<Result<ObservedReceipt, Error>, JournalError>
    {
        let result = self.pool.finish(host, now, current, snapshot);
        self.reap();
        result
    }

    /// Withdraw only helper I/O and request child termination. This does not
    /// cancel an actor action, release reservations or settle an unknown effect.
    pub fn stop_workers(&mut self) -> BTreeMap<String, ProcessStatus> {
        self.pool.close();
        self.reap()
    }
    pub fn into_children(mut self) -> HelperChildren {
        self.stop_workers();
        self.children
    }
}
