//! Fair, bounded multi-peer intake into ONE original durable supervisor.
//! This schedules existing inboxes; it creates no ledger, authority or executor.
use super::{
    ActorError, ActorProposal, ActorRequestPort, DriveBudget, ElapsedTick, Error,
    EvidenceFile, EvidenceIdentity, FileActorInbox, FileActorPeerDrive,
    FileActorPeerDriveError, FileActorSupervisor, FileEvidenceReport, FileOversight,
    FileRequestStatus, FileSupervisedDriver, JournalError, PeerAdmission, PeerRefusal,
    PeerSessionStatus, Port, UnixStream,
};
use crate::action::consequence::delivery::persistent::observed::driver::FileDriverPhase;
use crate::action::consequence::oversight::actor_transport::ConnectionStatus;
use crate::action::consequence::oversight::actor_wire::ChannelState;
use std::fmt;

pub const MAX_POOL_PEERS: usize = 16;

/// All peers share total; each visit is additionally capped by per_peer. Each
/// inbox is visited at most once per call. Limits charge socket work, NOT source
/// or journal latency, allocation, helper inference or a hard real-time deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolBudget {
    pub total: DriveBudget,
    pub per_peer: DriveBudget,
}
impl Default for PoolBudget {
    fn default() -> Self {
        Self {
            total: DriveBudget::default(),
            per_peer: DriveBudget { read_bytes: 8192, write_bytes: 8192, frames: 1, io_calls: 8 },
        }
    }
}

/// A failed construction returns every original inbox, including its tickets,
/// pending replies and ready hints. No configuration error discards the owners.
pub struct PoolSetupFailure<P: ActorRequestPort = Port> {
    pub error: Error,
    pub peers: Vec<(u64, FileActorInbox<P>)>,
}
impl<P: ActorRequestPort> fmt::Debug for PoolSetupFailure<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolSetupFailure").field("error", &self.error)
            .field("peers", &self.peers.len()).finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolAttachError { UnknownPeer, Refused(PeerRefusal) }

/// Supervisor-only diagnostics. No source report is serialized to an actor.
#[derive(Debug)]
pub struct PoolVisit {
    pub peer: u64,
    pub allowance: DriveBudget,
    pub result: Result<FileActorPeerDrive, FileActorPeerDriveError>,
}
#[derive(Debug)]
pub struct PoolDriveReport {
    pub visits: Vec<PoolVisit>,
    /// Unused shared allowance. A failed underlying drive does not report its
    /// partial work, so its ENTIRE issued allowance is charged conservatively.
    pub remaining: DriveBudget,
}
impl PoolDriveReport {
    /// A successful outer Result means preflight succeeded, not that every peer
    /// drive succeeded. A peer failure stops this call; inspect its exact error.
    pub fn stopped_on_error(&self) -> bool {
        self.visits.last().is_some_and(|visit| visit.result.is_err())
    }
}

/// A scheduling hint plus the freshly read original journal status, not a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolReady { pub peer: u64, pub status: FileRequestStatus }

/// Fixed, operator-named sessions with independent kernel credential policies,
/// tickets, framing and queues. Peers must ALL belong to the supplied supervisor;
/// one foreign member refuses the entire drive before any socket/source work.
/// Names select an existing session, not an actor-supplied identity or credential.
/// Only a trusted acceptor may choose a peer name for attach/reconnect.
///
/// Transport and ready selection use separate round-robin cursors. A partial
/// frame or blocked reply on one connection cannot monopolize the others' socket
/// work. This is cooperative bounded scheduling, not preemption of blocking I/O.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::FileActorPool;
/// fn grant(pool: FileActorPool) { pool.authorize(); }
/// ```
pub struct FileActorPool<P: ActorRequestPort = Port> {
    peers: Vec<(u64, FileActorInbox<P>)>,
    drive_cursor: usize,
    ready_cursor: usize,
}
impl<P: ActorRequestPort> FileActorPool<P> {
    pub fn new(peers: Vec<(u64, FileActorInbox<P>)>) -> Result<Self, PoolSetupFailure<P>> {
        let error = if peers.is_empty() || peers.len() > MAX_POOL_PEERS {
            Some(Error::Limit)
        } else if peers.iter().any(|(id, _)| *id == 0) {
            Some(Error::InvalidInput)
        } else if peers.iter().enumerate().any(|(i, (id, _))| peers[..i].iter().any(|(old, _)| old == id)) {
            Some(Error::Duplicate)
        } else { None };
        if let Some(error) = error { return Err(PoolSetupFailure { error, peers }); }
        Ok(Self { peers, drive_cursor: 0, ready_cursor: 0 })
    }

    pub fn statuses(&self) -> impl Iterator<Item = (u64, PeerSessionStatus, usize)> + '_ {
        self.peers.iter().map(|(id, inbox)| (*id, inbox.status(), inbox.queued()))
    }
    pub fn attach(&mut self, peer: u64, socket: UnixStream) -> Result<PeerAdmission, PoolAttachError> {
        let (_, inbox) = self.peers.iter_mut().find(|(id, _)| *id == peer)
            .ok_or(PoolAttachError::UnknownPeer)?;
        inbox.attach(socket).map_err(PoolAttachError::Refused)
    }
    pub fn disconnect(&mut self, peer: u64) -> Result<bool, Error> {
        let (_, inbox) = self.peers.iter_mut().find(|(id, _)| *id == peer).ok_or(Error::Missing)?;
        Ok(inbox.disconnect())
    }
    pub fn revoke(&mut self, peer: u64) -> Result<bool, Error> {
        let (_, inbox) = self.peers.iter_mut().find(|(id, _)| *id == peer).ok_or(Error::Missing)?;
        Ok(inbox.revoke())
    }
    /// Withdraw ingress, not effect authority. Retain every ready obligation.
    pub fn revoke_all(&mut self) {
        for (_, inbox) in &mut self.peers { inbox.revoke(); }
    }
    /// Move back the exact original sessions; no reset of tickets or quotas.
    pub fn into_inboxes(self) -> Vec<(u64, FileActorInbox<P>)> { self.peers }

    fn check_all<C>(&self, driver: &FileSupervisedDriver, check: &C) -> Result<(), JournalError>
    where C: Fn(&FileActorSupervisor<FileOversight>, &P) -> Result<(), JournalError> {
        for (_, inbox) in &self.peers {
            check(driver.supervisor(), inbox.session.request_port())?;
        }
        if driver.supervisor().host()?.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        Ok(())
    }

    // Sealed composition: callers cannot replace identity or source admission.
    pub(in crate::action::consequence::delivery::persistent::requests::actor)
    fn drive_prepared<C, A>(&mut self, driver: &mut FileSupervisedDriver,
        budget: PoolBudget, check: C, mut prepare: A)
        -> Result<PoolDriveReport, FileActorPeerDriveError>
    where C: Fn(&FileActorSupervisor<FileOversight>, &P) -> Result<(), JournalError>,
        A: FnMut(&mut FileActorSupervisor<FileOversight>, u64, &ActorProposal,
            &mut Option<FileEvidenceReport<EvidenceIdentity>>) -> Result<(), ActorError>,
    {
        let result = (|| {
            budget.total.validate()?;
            budget.per_peer.validate()?;
            self.check_all(driver, &check)?;
            let mut report = PoolDriveReport { visits: Vec::new(), remaining: budget.total };
            report.visits.try_reserve_exact(self.peers.len())
                .map_err(|_| JournalError::from(Error::Limit))?;
            let count = self.peers.len();
            let start = self.drive_cursor;
            for offset in 0..count {
                let allowance = intersect(report.remaining, budget.per_peer);
                if empty(allowance) { break; }
                let slot = (start + offset) % count;
                let (peer, inbox) = &mut self.peers[slot];
                let Some(status) = inbox.status().transport else { continue; };
                if !runnable(status, allowance) { continue; }
                // An ineligible visit must not undo the rotation after the last
                // peer that actually spent the shared budget.
                self.drive_cursor = (slot + 1) % count;
                let result = inbox.drive_prepared(driver, allowance, &check, &mut prepare);
                let charge = match &result {
                    Ok(report) => {
                        let p = report.drive.progress;
                        DriveBudget { read_bytes: p.read_bytes, write_bytes: p.written_bytes,
                            frames: p.frames, io_calls: p.io_calls }
                    }
                    // Unknown partial socket work is not refunded, and no later
                    // peer may run after a drive failed against the shared owner.
                    Err(_) => allowance,
                };
                report.remaining = subtract(report.remaining, charge);
                if result.as_ref().is_ok_and(|report| report.drive.status.closed()) {
                    inbox.disconnect(); // no effect cancellation or queue loss
                }
                let failed = result.is_err();
                report.visits.push(PoolVisit { peer: *peer, allowance, result });
                if failed { break; }
            }
            Ok(report)
        })();
        driver.reap_helpers();
        result
    }

    pub(in crate::action::consequence::delivery::persistent::requests::actor)
    fn next_checked<C>(&mut self, driver: &FileSupervisedDriver, check: C)
        -> Result<Option<PoolReady>, JournalError>
    where C: Fn(&FileActorSupervisor<FileOversight>, &P) -> Result<(), JournalError> {
        self.check_all(driver, &check)?;
        // The original congress is serial. Queue fairness never permits two
        // concurrent reviews of its shared predecessor or a duplicate dispatch.
        if driver.phase() != FileDriverPhase::Idle { return Err(Error::WrongState.into()); }
        let count = self.peers.len();
        for _ in 0..count {
            let slot = self.ready_cursor;
            let (peer, inbox) = &mut self.peers[slot];
            let next = inbox.next_checked(driver, &check)?;
            self.ready_cursor = (slot + 1) % count;
            if let Some(status) = next { return Ok(Some(PoolReady { peer: *peer, status })); }
        }
        Ok(None)
    }
}

impl FileActorPool<Port> {
    pub fn drive<S, F>(&mut self, driver: &mut FileSupervisedDriver, source: &mut S,
        mut clock: F, budget: PoolBudget) -> Result<PoolDriveReport, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.drive_prepared(driver, budget,
            |supervisor, port| supervisor.check_source_wire(port),
            |supervisor, request, _, intake| {
                supervisor.prepare_wire_submission(request, source, &mut clock, intake)
            })
    }
    /// Service recorded retries, polls and cancellation without acquiring source
    /// evidence or sampling a clock. NEW submissions are withheld before the
    /// original port can consume any waiting admission snapshot. This is useful
    /// during an active congress or terminal reply grace, not an effect stop.
    pub fn observe(&mut self, driver: &mut FileSupervisedDriver, budget: PoolBudget)
        -> Result<PoolDriveReport, FileActorPeerDriveError>
    {
        self.drive_prepared(driver, budget,
            |supervisor, port| supervisor.check_source_wire(port),
            |supervisor, request, _, _| require_recorded(supervisor, request))
    }
    pub fn next_request(&mut self, driver: &FileSupervisedDriver) -> Result<Option<PoolReady>, JournalError> {
        self.next_checked(driver, |supervisor, port| supervisor.check_source_wire(port))
    }
}

// Existing retries STILL pass the original port's exact binding checks. A
// lookup is not permission to replace a frozen action or renew its authority.
pub(in crate::action::consequence::delivery::persistent::requests::actor)
fn require_recorded(supervisor: &FileActorSupervisor<FileOversight>, request: u64)
    -> Result<(), ActorError>
{
    match supervisor.host().and_then(|host| host.request_status(request)) {
        Ok(_) => Ok(()),
        Err(JournalError::Contract(Error::Missing)) => Err(ActorError::Withheld),
        Err(_) => Err(ActorError::Unavailable),
    }
}

fn intersect(a: DriveBudget, b: DriveBudget) -> DriveBudget {
    DriveBudget { read_bytes: a.read_bytes.min(b.read_bytes), write_bytes: a.write_bytes.min(b.write_bytes),
        frames: a.frames.min(b.frames), io_calls: a.io_calls.min(b.io_calls) }
}
fn empty(b: DriveBudget) -> bool {
    b.read_bytes == 0 && b.write_bytes == 0 && b.frames == 0 && b.io_calls == 0
}
fn runnable(status: ConnectionStatus, budget: DriveBudget) -> bool {
    match status.channel {
        ChannelState::Reading => budget.frames > 0 && (status.buffered_input_bytes > 0
            || (budget.read_bytes > 0 && budget.io_calls > 0)),
        ChannelState::ReplyReady => budget.io_calls > 0
            && (status.pending_output_bytes == 0 || budget.write_bytes > 0),
        ChannelState::Closed(_) => false,
    }
}
fn subtract(a: DriveBudget, b: DriveBudget) -> DriveBudget {
    // Original UnixActorConnection guarantees each counter is bounded by the
    // issued allowance. Buffered consumed bytes are NOT read bytes and are not
    // subtracted here. No saturating arithmetic can hide a budget violation.
    DriveBudget { read_bytes: a.read_bytes - b.read_bytes, write_bytes: a.write_bytes - b.write_bytes,
        frames: a.frames - b.frames, io_calls: a.io_calls - b.io_calls }
}

#[cfg(test)]
mod tests;
