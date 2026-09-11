//! Nonblocking host-side fan-out over one preprovisioned Unix stream per helper.
//! The host authenticates and isolates peers before handing the streams here.
//! No listener, credential broker, process launcher or alternative runtime exists.

use super::{HelperConnection, IoProgress, WorkerIoError};
use super::super::{HelperLimits, HelperRound, HelperStatus};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{ObservedReview, ObservedSession, ReviewWindow};
use crate::Error;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

/// Only the supervising host receives individual worker status or I/O failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperPump {
    pub io: BTreeMap<String, Result<IoProgress, WorkerIoError>>,
    pub workers: BTreeMap<String, HelperStatus>,
}

pub struct HelperPool {
    round: HelperRound,
    connections: BTreeMap<String, HelperConnection<UnixStream>>,
    window: ReviewWindow,
    finished: bool,
}

impl HelperPool {
    /// Exact roster equality is checked before any request bytes are sent.
    /// Setup failures cannot fall back to a reduced roster or caller votes.
    pub fn new(
        session: ObservedSession, mut streams: BTreeMap<String, UnixStream>, limits: HelperLimits,
    ) -> Result<Self, WorkerIoError> {
        if !streams.keys().eq(session.inputs().views().keys()) {
            return Err(WorkerIoError::Protocol(Error::Binding));
        }
        let window = session.window();
        let (round, ports) = HelperRound::new(session, limits).map_err(WorkerIoError::Protocol)?;
        let mut connections = BTreeMap::new();
        for (member, port) in ports {
            let stream = streams.remove(&member).ok_or(WorkerIoError::Protocol(Error::Missing))?;
            stream.set_nonblocking(true).map_err(|error| WorkerIoError::Io(error.kind()))?;
            connections.insert(member, HelperConnection::new(port, stream).map_err(WorkerIoError::Protocol)?);
        }
        Ok(Self { round, connections, window, finished: false })
    }

    /// One atomic logical-tick pump, useful to hosts with controlled virtual time.
    /// A real elapsed-clock host should use pump_with_clock to refresh receipt
    /// time after each nonblocking I/O operation rather than freezing a batch.
    pub fn pump(&mut self, now: ElapsedTick) -> Result<HelperPump, Error> {
        self.pump_with_clock(|| now)
    }

    /// One bounded I/O step for EVERY frozen member, including when another is
    /// blocked or failed. The trusted host clock is checked before and after
    /// each operation: crossing a cutoff during receipt cannot backdate a vote.
    /// Supply the controller's monotone clock domain, never a worker clock.
    /// An outer clock/protocol error preserves prior progress; it is not rollback.
    pub fn pump_with_clock<F>(&mut self, mut clock: F) -> Result<HelperPump, Error>
    where F: FnMut() -> ElapsedTick {
        self.round.advance(clock())?;
        let mut io = BTreeMap::new();
        for (member, connection) in &mut self.connections {
            self.round.advance(clock())?;
            let result = connection.step();
            self.round.advance(clock())?;
            io.insert(member.clone(), result);
        }
        Ok(HelperPump { io, workers: self.round.statuses() })
    }

    pub fn statuses(&self) -> BTreeMap<String, HelperStatus> { self.round.statuses() }

    /// A scheduling hint for the host, not evidence that an effect is safe.
    pub fn ready_to_finish(&self) -> bool {
        !self.finished && (self.round.elapsed() >= self.window.reveal_by
            || self.round.statuses().values().all(|status| status.revealed))
    }

    pub fn next_deadline(&self) -> Option<ElapsedTick> {
        if self.finished || self.ready_to_finish() { return None; }
        if self.round.elapsed() < self.window.commit_by
            && self.round.statuses().values().any(|status| !status.committed)
        { Some(self.window.commit_by) } else { Some(self.window.reveal_by) }
    }

    /// No I/O, automatic clock advance or helper rerun occurs in finish. The
    /// returned review still belongs to its original broker and exact input cut.
    pub fn finish(&mut self, now: ElapsedTick) -> Result<ObservedReview, Error> {
        let review = self.round.finish(now)?;
        self.finished = true;
        Ok(review)
    }
}
