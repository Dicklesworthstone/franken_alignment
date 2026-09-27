//! Nonblocking, original helper wire protocol over fixed supervisor-owned peers.
//! No new executor, helper vote API, reconnect, process launcher or sandbox.
use super::{LearnedSidecar, LearnedWorkerReview, LearnedWorkerSchedule, LearnedWorkerStatus,
    LearnedWorkerUpdate, OversightBroker, MAX_SIDECAR_REFINEMENT_ROUNDS};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{ObservedReview,
    helper_workers::{HelperPort, io::{HelperConnection, WorkerIoError}}};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::os::unix::net::UnixStream;

/// One private full roster for every scheduled round, selected before any input
/// is sent. Existing launch_helpers can supply these streams; its child owners
/// MUST remain separately retained and reaped by the supervisor.
pub type LearnedWorkerSockets = BTreeMap<u64, BTreeMap<String, UnixStream>>;
type Connections = BTreeMap<String, HelperConnection<UnixStream>>;

/// Owns both the original worker sequence and its transports. No raw ports,
/// mutable review owner, peer substitution or executable actor can be extracted.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::transport::LearnedSocketReview;
/// fn bypass(run: &mut LearnedSocketReview) { run.ports_mut(); }
/// ```
pub struct LearnedSocketReview {
    run: LearnedWorkerReview,
    connections: Connections,
    future: LearnedWorkerSockets,
    io_steps: u64,
    failures: BTreeMap<(u64, String), WorkerIoError>,
}
impl fmt::Debug for LearnedSocketReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedSocketReview").field("review", &self.run)
            .field("io_steps", &self.io_steps).field("failures", &self.failures).finish_non_exhaustive()
    }
}
impl LearnedSocketReview {
    pub fn review(&self) -> &LearnedWorkerReview { &self.run }
    /// Counts attempted HelperConnection steps, not syscalls or transferred bytes.
    pub fn io_steps(&self) -> u64 { self.io_steps }
    /// First transport/protocol failure per original round/member. Later polls
    /// and successful different rounds cannot erase earlier failed observations.
    pub fn failures(&self) -> &BTreeMap<(u64, String), WorkerIoError> { &self.failures }
    pub fn take_review(&mut self) -> Result<ObservedReview, Error> { self.run.take_review() }
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        self.run.cancel(expected_revision)?;
        self.close();
        Ok(())
    }
    fn close(&mut self) { self.connections.clear(); self.future.clear(); }
}
impl OversightBroker {
    /// Preflight the ENTIRE socket/round/member inventory and set nonblocking
    /// mode before creating the first original review or sending any evidence.
    /// Names bind caller-provisioned sockets, not authenticated process identity.
    pub fn begin_learned_socket_review(&mut self, sidecar: LearnedSidecar,
        schedule: LearnedWorkerSchedule, mut sockets: LearnedWorkerSockets, snapshot: &Snapshot)
        -> Result<LearnedSocketReview, WorkerIoError>
    {
        if schedule.rounds.is_empty() || schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS
            || sockets.len() != schedule.rounds.len() { return Err(WorkerIoError::Protocol(Error::Binding)); }
        for round in &schedule.rounds {
            let peers = sockets.get(&round.round).ok_or(WorkerIoError::Protocol(Error::Missing))?;
            if !peers.keys().eq(self.contracts().members().keys()) { return Err(WorkerIoError::Protocol(Error::Binding)); }
            for stream in peers.values() { stream.set_nonblocking(true).map_err(|e| WorkerIoError::Io(e.kind()))?; }
        }
        let (run, ports) = self.begin_learned_worker_review(sidecar, schedule, snapshot).map_err(WorkerIoError::Protocol)?;
        let peers = sockets.remove(&run.current_round().round).ok_or(WorkerIoError::Protocol(Error::Missing))?;
        let connections = connect(ports, peers)?;
        Ok(LearnedSocketReview { run, connections, future: sockets, io_steps: 0, failures: BTreeMap::new() })
    }

    /// At most one bounded nonblocking read/write step per member, followed by
    /// the original coordinator pass. A failed member stays missing. Success
    /// never implies that a model ran, a verdict is correct, or an effect is safe.
    pub fn pump_learned_socket_review(&mut self, driver: &mut LearnedSocketReview,
        expected_revision: u64, now: ElapsedTick, snapshot: &Snapshot)
        -> Result<LearnedWorkerStatus, WorkerIoError>
    {
        // Rejected predecessor/owner/time calls do not close usable transports.
        self.check_learned_worker_call(&driver.run, expected_revision, now).map_err(WorkerIoError::Protocol)?;
        let current = driver.run.current_round();
        let round_id = current.round;
        let statuses = driver.run.worker_statuses();
        // A caught unwind drops these sockets AND the core's active HelperRound.
        let mut connections = std::mem::take(&mut driver.connections);
        let failures = &mut driver.failures;
        let steps = &mut driver.io_steps;
        let update = self.advance_learned_workers_with_io(&mut driver.run, expected_revision, now, snapshot, || {
            for (member, connection) in &mut connections {
                // Do not send a first request after commit expiry, or any wire
                // data after reveal expiry. The original coordinator still
                // records each deadline failure and the complete denominator.
                if now >= current.window.reveal_by
                    || (now >= current.window.commit_by && !statuses[member].committed) { continue; }
                // Bounded by 65,536 admitted polls * the original MAX_VOTES.
                *steps += 1;
                if let Err(error) = connection.step() {
                    failures.entry((round_id, member.clone())).or_insert(error);
                }
            }
        });
        match update {
            Ok(LearnedWorkerUpdate::Waiting) => driver.connections = connections,
            Ok(LearnedWorkerUpdate::NextRound(ports)) => {
                drop(connections);
                let prepared = driver.future.remove(&driver.run.current_round().round)
                    .ok_or(WorkerIoError::Protocol(Error::Missing)).and_then(|peers| connect(ports, peers));
                match prepared {
                    Ok(connections) => driver.connections = connections,
                    Err(error) => {
                        driver.run.status = LearnedWorkerStatus::Failed(match error {
                            WorkerIoError::Protocol(reason) => reason, WorkerIoError::Io(_) => Error::Incomplete });
                        driver.run.close_active(); driver.close(); return Err(error);
                    }
                }
            }
            Ok(LearnedWorkerUpdate::Stopped) => { drop(connections); driver.close(); }
            Err(error) => { drop(connections); driver.close(); return Err(WorkerIoError::Protocol(error)); }
        }
        Ok(driver.run.status())
    }
}
fn connect(ports: BTreeMap<String, HelperPort>, mut peers: BTreeMap<String, UnixStream>)
    -> Result<Connections, WorkerIoError>
{
    if !ports.keys().eq(peers.keys()) { return Err(WorkerIoError::Protocol(Error::Binding)); }
    let mut connections = BTreeMap::new();
    for (member, port) in ports {
        let stream = peers.remove(&member).ok_or(WorkerIoError::Protocol(Error::Missing))?;
        let connection = HelperConnection::new(port, stream).map_err(WorkerIoError::Protocol)?;
        connections.insert(member, connection);
    }
    Ok(connections)
}
