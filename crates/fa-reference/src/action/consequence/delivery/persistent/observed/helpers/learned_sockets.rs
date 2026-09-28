//! External helpers reviewing the ORIGINAL learned sidecar through one journal.
//! Reuse native wire I/O, coordinator, durable commits and witnessed completion.
//! This module neither interprets helper verdicts nor grants publication keys.
pub mod sequence;

use super::{DurableSession, FileHelperFailure, FileHelperLaunch, FileHelperPool,
    FileHelperSetupError, FileOversight, HelperPump, JournalError, journal_error};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{ReviewWindow, helper_workers::{
    HelperLimits, coordinator::Coordinator}};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

/// Supervisor-provisioned sockets for one frozen roster. Identity and process
/// isolation remain the provisioner's responsibility. No member is replaced.
pub struct LearnedSocketLaunch {
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub streams: BTreeMap<String, UnixStream>,
    pub limits: HelperLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedSocketStatus { Running, Finished, Cancelled, Failed }

/// A source-bound live round. No underlying pool or sidecar handle escapes to
/// a manual-vote path. Original protocol custody survives cancellation/drop in
/// the live host; reopening fences old sessions instead of resuming their I/O.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::FileLearnedSocketRound;
/// fn override_vote(round: &mut FileLearnedSocketRound) { round.submit_verdict(); }
/// ```
pub struct FileLearnedSocketRound {
    issuer: Rc<()>,
    sidecar: FileLearnedSidecar,
    pool: Option<FileHelperPool>,
    round: u64,
    revision: u64,
    status: LearnedSocketStatus,
    failure: Option<JournalError>,
    progress: HelperPump,
    connection_steps: u64,
    result: Option<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileLearnedSocketRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedSocketRound").field("round", &self.round)
            .field("status", &self.status).field("revision", &self.revision).finish_non_exhaustive()
    }
}
impl FileOversight {
    /// Source/roster/encoding admission precedes the existing durable Begin.
    /// No request bytes are sent here. Only this returned owner can poll its
    /// sockets or complete its original source-bound review.
    pub fn begin_learned_socket_round(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        launch: LearnedSocketLaunch, snapshot: Snapshot)
        -> Result<FileLearnedSocketRound, FileHelperSetupError>
    {
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.current_learned_sidecar(&sidecar)?;
        let pool = self.begin_helper_review(revision, FileHelperLaunch {
            attempt: sidecar.attempt(), expected_input_revision: sidecar.input_revision(),
            round: launch.round, evidence_root: launch.evidence_root, window: launch.window,
            streams: launch.streams, limits: launch.limits,
        }, snapshot)?;
        let workers = pool.statuses();
        Ok(FileLearnedSocketRound { issuer: Rc::clone(&self.issuer), sidecar,
            pool: Some(pool), round: launch.round, revision: 0,
            status: LearnedSocketStatus::Running, failure: None,
            progress: HelperPump { io: BTreeMap::new(), workers }, connection_steps: 0, result: None })
    }
}
impl FileLearnedSocketRound {
    pub fn round(&self) -> u64 { self.round }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn status(&self) -> LearnedSocketStatus { self.status }
    pub fn failure(&self) -> Option<&JournalError> { self.failure.as_ref() }
    pub fn connection_steps(&self) -> u64 { self.connection_steps }
    pub fn progress(&self) -> &HelperPump { &self.progress }
    pub fn outcome(&self) -> Option<&FileLearnedSidecarFinish> { self.result.as_ref() }
    pub fn input_revision(&self) -> u64 { self.sidecar.input_revision() }
    pub fn ready_to_finish(&self) -> bool {
        self.status == LearnedSocketStatus::Running
            && self.pool.as_ref().is_some_and(FileHelperPool::ready_to_finish)
    }
    pub fn next_deadline(&self) -> Option<ElapsedTick> {
        self.pool.as_ref().and_then(FileHelperPool::next_deadline)
    }
    fn bind(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != LearnedSocketStatus::Running { return Err(Error::WrongState.into()); }
        Ok(())
    }
    fn latch(&mut self) -> Result<(), JournalError> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = LearnedSocketStatus::Failed;
        self.failure = Some(Error::Incomplete.into());
        Ok(())
    }
    fn close(&mut self) {
        if let Some(mut pool) = self.pool.take() {
            pool.close(); self.progress.workers = pool.statuses();
        }
    }
    fn failed_pump(&self, error: JournalError) -> FileHelperFailure {
        FileHelperFailure { error, progress: self.progress.clone() }
    }
    pub fn pump(&mut self, host: &mut FileOversight, revision: u64, now: ElapsedTick)
        -> Result<HelperPump, FileHelperFailure>
    {
        self.pump_with_clock(host, revision, || now)
    }
    /// Observe the caller's clock before and after each original bounded socket
    /// step. Recheck the live learned source BEFORE I/O and BEFORE accepting the
    /// resulting protocol event. Every accepted commit/reveal remains durable.
    /// Blocking journal sync is not a wall-clock-bounded operation.
    pub fn pump_with_clock<F>(&mut self, host: &mut FileOversight, revision: u64, mut clock: F)
        -> Result<HelperPump, FileHelperFailure>
    where F: FnMut() -> ElapsedTick {
        self.bind(host, revision).map_err(|error| self.failed_pump(error))?;
        let now = clock();
        let previous = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)
            .map_err(|error| self.failed_pump(error.into()))?;
        if now < previous || self.pool.as_ref().is_some_and(|pool| now < pool.coordinator.elapsed()) {
            return Err(self.failed_pump(Error::Stale.into()));
        }
        self.latch().map_err(|error| self.failed_pump(error))?;
        // Stack ownership closes sockets and ports even if a clock callback or
        // original journal operation unwinds. No resumed half-operation exists.
        let mut pool = self.pool.take().expect("running socket round");
        self.progress.io.clear();
        let result = (|| {
            let mut session = DurableSession { host, attempt: pool.attempt,
                round: pool.round, inputs: &pool.inputs };
            advance_bound(&mut pool.coordinator, &mut session, &self.sidecar, now)?;
            for (member, connection) in &mut pool.connections {
                advance_bound(&mut pool.coordinator, &mut session, &self.sidecar, clock())?;
                self.connection_steps = self.connection_steps.checked_add(1).ok_or(Error::Overflow)?;
                let observed = connection.step();
                // Retain real partial I/O even if the following sync fails.
                self.progress.io.insert(member.clone(), observed);
                advance_bound(&mut pool.coordinator, &mut session, &self.sidecar, clock())?;
            }
            Ok::<(), JournalError>(())
        })();
        match result {
            Ok(()) => {
                self.progress.workers = pool.statuses(); self.pool = Some(pool);
                self.status = LearnedSocketStatus::Running; self.failure = None;
                Ok(self.progress.clone())
            }
            Err(error) => {
                pool.close(); self.progress.workers = pool.statuses();
                self.failure = Some(error.clone()); Err(self.failed_pump(error))
            }
        }
    }
    /// Complete a single round without buying an unassigned next review. A
    /// schedule owner may enable original refinement only when a next round is
    /// already provisioned. Incomplete phases leave this owner usable.
    pub fn finish(&mut self, host: &mut FileOversight, revision: u64, snapshot: Snapshot)
        -> Result<&FileLearnedSidecarFinish, JournalError>
    {
        self.finish_bound(host, revision, false, snapshot)
    }
    fn finish_bound(&mut self, host: &mut FileOversight, revision: u64,
        refine: bool, snapshot: Snapshot) -> Result<&FileLearnedSidecarFinish, JournalError>
    {
        self.bind(host, revision)?;
        if !self.ready_to_finish() { return Err(Error::Incomplete.into()); }
        self.latch()?;
        let mut pool = self.pool.take().expect("complete socket round");
        let result = host.finish_learned_sidecar_review_inner(host.revision(),
            &mut self.sidecar, self.round, refine, snapshot);
        pool.close(); self.progress.workers = pool.statuses();
        match result {
            Ok(result) => {
                self.result = Some(result); self.failure = None; self.status = LearnedSocketStatus::Finished;
                Ok(self.result.as_ref().expect("acknowledged result"))
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        if revision != self.revision { return Err(Error::Stale); }
        if self.status != LearnedSocketStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.close(); self.status = LearnedSocketStatus::Cancelled;
        Ok(())
    }
}

fn advance_bound(coordinator: &mut Coordinator, session: &mut DurableSession<'_>,
    sidecar: &FileLearnedSidecar, now: ElapsedTick) -> Result<(), JournalError>
{
    if session.host.fault.is_some() { return Err(JournalError::Unavailable); }
    let previous = session.host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
    if now < previous { return Err(Error::Stale.into()); }
    if now != previous { session.host.observe_time(session.host.revision(), now)?; }
    session.host.current_learned_sidecar(sidecar)?;
    coordinator.advance(session, now).map_err(journal_error)
}
