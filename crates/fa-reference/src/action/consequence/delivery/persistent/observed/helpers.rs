//! Actual helper sockets feeding the original durable full-input congress.
//! Framing, per-member slots and phase coordination are the existing worker
//! implementation. Only the backing transition changes from RAM to the journal.
pub mod processes;
#[cfg(test)]
mod tests;
use super::{Event, FileOversight, JournalError, ObservedReceipt, Transition};
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow};
use crate::action::consequence::oversight::helper_workers::{HelperLimits, HelperPort, HelperStatus};
use crate::action::consequence::oversight::helper_workers::coordinator::{AdvanceError, Coordinator, Session};
use crate::action::consequence::oversight::helper_workers::io::{HelperConnection, HelperPump, WorkerIoError};
use crate::round::{Digest, Verdict};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

/// Explicit operator launch. Each stream must already be authenticated and
/// isolated by the host. No helper can choose its member, input or receipt time.
pub struct FileHelperLaunch {
    pub attempt: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub streams: BTreeMap<String, UnixStream>,
    pub limits: HelperLimits,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileHelperSetupError { Journal(JournalError), Worker(WorkerIoError) }
impl From<JournalError> for FileHelperSetupError {
    fn from(error: JournalError) -> Self { Self::Journal(error) }
}
impl From<Error> for FileHelperSetupError {
    fn from(error: Error) -> Self { Self::Journal(error.into()) }
}

/// An outer backend failure retains all socket operations already performed in
/// this pass. It is NOT rollback, a finished review, or authority to retry a vote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHelperFailure {
    pub error: JournalError,
    pub progress: HelperPump,
}

/// One leased durable round and the original bounded socket connections. It
/// does not own the FileOversight lock, reviewer role, automatic key or endpoint.
/// No stream replacement, round recovery, caller-vote fallback or clone exists.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::FileHelperPool;
/// fn bypass(pool: FileHelperPool) { pool.authorize(); }
/// ```
pub struct FileHelperPool {
    issuer: Rc<()>,
    attempt: u64,
    round: u64,
    inputs: CommitteeInput,
    window: ReviewWindow,
    coordinator: Coordinator,
    connections: BTreeMap<String, HelperConnection<UnixStream>>,
    closed: bool,
}
impl fmt::Debug for FileHelperPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileHelperPool").field("round", &self.round)
            .field("closed", &self.closed).finish_non_exhaustive()
    }
}

fn connect(ports: BTreeMap<String, HelperPort>, mut streams: BTreeMap<String, UnixStream>)
    -> Result<BTreeMap<String, HelperConnection<UnixStream>>, WorkerIoError>
{
    if !ports.keys().eq(streams.keys()) { return Err(WorkerIoError::Protocol(Error::Binding)); }
    let mut connections = BTreeMap::new();
    for (member, port) in ports {
        let stream = streams.remove(&member).ok_or(WorkerIoError::Protocol(Error::Missing))?;
        stream.set_nonblocking(true).map_err(|error| WorkerIoError::Io(error.kind()))?;
        let connection = HelperConnection::new(port, stream).map_err(WorkerIoError::Protocol)?;
        connections.insert(member, connection);
    }
    Ok(connections)
}

impl FileOversight {
    /// Validate the complete roster, input budget, socket setup and request
    /// encoding before beginning the durable round. No request bytes are sent
    /// here. Once begun, only its returned pool can supply protocol events;
    /// dropping it cannot reopen a manual-vote or reduced-roster alternative.
    pub fn begin_helper_review(&mut self, revision: u64, launch: FileHelperLaunch,
        snapshot: Snapshot) -> Result<FileHelperPool, FileHelperSetupError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable.into()); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.clock_ready() { return Err(Error::Incomplete.into()); }
        if self.worker_rounds.contains(&launch.round) { return Err(Error::Duplicate.into()); }
        if self.machine.broker.input_revision(launch.attempt)? != launch.expected_input_revision { return Err(Error::Stale.into()); }
        let inputs = self.machine.broker.current_inputs(launch.attempt)?.ok_or(Error::Incomplete)?;
        if !launch.streams.keys().eq(inputs.views().keys()) { return Err(Error::Binding.into()); }
        let now = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        let (coordinator, ports) = Coordinator::new(launch.round, launch.evidence_root, inputs,
            launch.window, now, launch.limits)?;
        let inputs = inputs.clone();
        let connections = connect(ports, launch.streams).map_err(FileHelperSetupError::Worker)?;
        self.begin_review(revision, launch.attempt, launch.round, launch.evidence_root, launch.window, snapshot)?;
        self.worker_rounds.insert(launch.round);
        Ok(FileHelperPool { issuer: Rc::clone(&self.issuer), attempt: launch.attempt, round: launch.round,
            inputs, window: launch.window, coordinator, connections, closed: false })
    }

    /// Process-local custody only. Reopening never resumes any native session,
    /// so losing this marker cannot revive a worker round across process loss.
    pub(super) fn check_manual_round(&self, round: u64) -> Result<(), JournalError> {
        if self.worker_rounds.contains(&round) { return Err(Error::WrongState.into()); }
        Ok(())
    }
}

impl FileHelperPool {
    pub fn round(&self) -> u64 { self.round }
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn is_closed(&self) -> bool { self.closed }
    pub fn statuses(&self) -> BTreeMap<String, HelperStatus> { self.coordinator.statuses() }
    pub fn ready_to_finish(&self) -> bool {
        !self.closed && (self.coordinator.elapsed() >= self.window.reveal_by
            || self.statuses().values().all(|status| status.revealed))
    }
    pub fn next_deadline(&self) -> Option<ElapsedTick> {
        if self.closed || self.ready_to_finish() { return None; }
        if self.coordinator.elapsed() < self.window.commit_by
            && self.statuses().values().any(|status| !status.committed)
        { Some(self.window.commit_by) } else { Some(self.window.reveal_by) }
    }

    fn bind(&self, host: &FileOversight) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if self.closed { return Err(Error::WrongState.into()); }
        Ok(())
    }
    fn close(&mut self) {
        self.closed = true;
        self.coordinator.close();
        self.connections.clear();
    }

    pub fn pump(&mut self, host: &mut FileOversight, now: ElapsedTick) -> Result<HelperPump, FileHelperFailure> {
        self.pump_with_clock(host, || now)
    }

    /// One original nonblocking socket step per frozen member. Observe trusted
    /// receipt time before and after each step; accepted phases are durable
    /// before another helper can receive a reveal request. Equal observations
    /// reuse the same clock event, not a new vote or an extended deadline.
    ///
    /// File commits are synchronous. Bounded socket steps are NOT a wall-clock
    /// latency bound for journal replay, allocation, filesystem writes or sync.
    pub fn pump_with_clock<F>(&mut self, host: &mut FileOversight, mut clock: F)
        -> Result<HelperPump, FileHelperFailure>
    where F: FnMut() -> ElapsedTick {
        if let Err(error) = self.bind(host) {
            return Err(FileHelperFailure { error, progress: HelperPump { io: BTreeMap::new(), workers: self.statuses() } });
        }
        let mut io = BTreeMap::new();
        let result = (|| {
            let mut session = DurableSession { host, attempt: self.attempt, round: self.round, inputs: &self.inputs };
            self.coordinator.advance(&mut session, clock()).map_err(journal_error)?;
            for (member, connection) in &mut self.connections {
                self.coordinator.advance(&mut session, clock()).map_err(journal_error)?;
                let result = connection.step();
                // Retain this actual I/O progress even when the post-I/O clock
                // or durable transition fails. It must not disappear in Err.
                io.insert(member.clone(), result);
                self.coordinator.advance(&mut session, clock()).map_err(journal_error)?;
            }
            Ok::<(), JournalError>(())
        })();
        match result {
            Ok(()) => Ok(HelperPump { io, workers: self.statuses() }),
            Err(error) => {
                self.close();
                Err(FileHelperFailure { error, progress: HelperPump { io, workers: self.statuses() } })
            }
        }
    }

    /// No socket I/O or implicit evidence refresh. A fresh caller-supplied view
    /// and snapshot go through the original completed-review application. Inner
    /// Err is a COMMITTED refusal; an incomplete phase retains this pool.
    pub fn finish(&mut self, host: &mut FileOversight, now: ElapsedTick,
        current: Option<&CommitteeInput>, snapshot: Snapshot)
        -> Result<Result<ObservedReceipt, Error>, JournalError>
    {
        self.bind(host)?;
        if let Some(input) = current { host.check_action(self.attempt, input.action())?; }
        let mut session = DurableSession { host, attempt: self.attempt, round: self.round, inputs: &self.inputs };
        if let Err(error) = self.coordinator.advance(&mut session, now).map_err(journal_error) {
            self.close();
            return Err(error);
        }
        let supplied = current.map(|input| input.views().clone());
        let result = session.host.transact(session.host.revision(), Event::Finish(self.round, supplied, snapshot));
        match result {
            Ok(Transition::Reviewed(review)) => { self.close(); Ok(review) }
            Ok(_) => unreachable!("durable worker review transition"),
            Err(JournalError::Contract(Error::Incomplete)) => Err(Error::Incomplete.into()),
            Err(error) => { self.close(); Err(error) }
        }
    }
}

struct DurableSession<'a> {
    host: &'a mut FileOversight,
    attempt: u64,
    round: u64,
    inputs: &'a CommitteeInput,
}
impl Session for DurableSession<'_> {
    type Failure = JournalError;
    fn observe(&mut self, now: ElapsedTick) -> Result<(), JournalError> {
        if self.host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !self.host.clock_ready() { return Err(Error::Incomplete.into()); }
        if !self.host.worker_rounds.contains(&self.round) { return Err(Error::Binding.into()); }
        let (attempt, native) = self.host.machine.sessions.get(&self.round).ok_or(Error::Missing)?;
        if *attempt != self.attempt || native.inputs() != self.inputs { return Err(Error::Binding.into()); }
        let inspection = self.host.inspect().control;
        // Withdrawing this action also stops further helper I/O. This checks the
        // original ledger; it does not invent another admission state machine.
        if !matches!(inspection.ledger.stages.get(&self.attempt), Some(ActionState::Reviewing | ActionState::Authorized)) {
            return Err(Error::WrongState.into());
        }
        let previous = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < previous { return Err(Error::Stale.into()); }
        if now != previous { self.host.observe_time(self.host.revision(), now)?; }
        Ok(())
    }
    fn commit(&mut self, member: &str, digest: Digest, _now: ElapsedTick) -> Result<(), AdvanceError<JournalError>> {
        member_result(self.host.transact(self.host.revision(), Event::Commit(self.round, member.to_owned(), digest)))
    }
    fn open(&mut self, _now: ElapsedTick) -> Result<(), AdvanceError<JournalError>> {
        member_result(self.host.transact(self.host.revision(), Event::OpenReveals(self.round)))
    }
    fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8], _now: ElapsedTick) -> Result<(), AdvanceError<JournalError>> {
        member_result(self.host.transact(self.host.revision(), Event::Reveal(self.round, member.to_owned(), verdict, salt.to_vec())))
    }
}
fn member_result(result: Result<Transition, JournalError>) -> Result<(), AdvanceError<JournalError>> {
    match result {
        Ok(Transition::Unit) => Ok(()),
        Ok(_) => unreachable!("worker protocol transition"),
        Err(JournalError::Contract(error)) if !matches!(error, Error::Limit | Error::Overflow) => Err(AdvanceError::Protocol(error)),
        Err(error) => Err(AdvanceError::Backend(error)),
    }
}
fn journal_error(error: AdvanceError<JournalError>) -> JournalError {
    match error { AdvanceError::Protocol(error) => error.into(), AdvanceError::Backend(error) => error }
}
