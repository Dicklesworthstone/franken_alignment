//! External helper peers feeding the ORIGINAL durable learned-sidecar rounds.
//! No caller ballot, replacement peer, new wire format or alternate refiner.
#[cfg(test)]
mod tests;

use super::{connect, journal_error, Coordinator, DurableSession, Event, FileHelperSetupError,
    FileOversight, HelperConnection, HelperStatus, JournalError, UnixStream, WorkerIoError};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{CommitteeInput,
    helper_workers::io::IoProgress,
    learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule, MAX_LEARNED_REVIEW_POLLS},
    sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS};
use crate::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

/// Simultaneously owned round/member sockets, including not-yet-contacted peers.
pub const MAX_LEARNED_TRANSPORT_PEERS: usize = 256;
pub type LearnedRoundPeers = BTreeMap<u64, BTreeMap<String, UnixStream>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLearnedTransportStatus { Running, Finished, Cancelled, Failed }

/// Actual original connection calls, not votes, inference or elapsed time.
/// An interrupted call counts as attempted even if no result was returned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LearnedTransportRecord {
    pub attempted_steps: usize,
    pub returned_steps: usize,
    pub last: Option<Result<IoProgress, WorkerIoError>>,
    pub first_failure: Option<WorkerIoError>,
}
struct Active {
    coordinator: Coordinator,
    connections: BTreeMap<String, HelperConnection<UnixStream>>,
}

/// Complete peer custody freezes before the first durable Begin. Each round
/// receives its own exact current input; future peers receive no earlier view.
/// The supervisor still owns peer authentication, isolation and child reaping.
/// Original FNV comparison commitments are not cryptographic authentication.
///
/// Every acknowledged finish is retained, including original application Err.
/// A failure closes active and future sockets. Dropping a driver never resumes
/// its leased rounds, reruns a helper, supplies a ballot or refunds an effect.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_transport::FileLearnedTransportReview;
/// fn replace(run: &mut FileLearnedTransportReview) { run.replace_peer(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_transport::FileLearnedTransportReview;
/// fn vote(run: &mut FileLearnedTransportReview) { run.submit_verdict(); }
/// ```
pub struct FileLearnedTransportReview {
    issuer: Rc<()>,
    sidecar: FileLearnedSidecar,
    schedule: LearnedWorkerSchedule,
    pending: LearnedRoundPeers,
    active: Option<Active>,
    input: CommitteeInput,
    index: usize,
    revision: u64,
    polls: usize,
    status: FileLearnedTransportStatus,
    failure: Option<FileHelperSetupError>,
    last_statuses: BTreeMap<String, HelperStatus>,
    records: BTreeMap<(u64, String), LearnedTransportRecord>,
    history: Vec<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileLearnedTransportReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedTransportReview").field("status", &self.status)
            .field("round", &self.current_round()).field("polls", &self.polls).finish_non_exhaustive()
    }
}
impl FileOversight {
    /// All round IDs, windows, socket rosters and helper input ceilings are
    /// fixed before Begin. Preparing connections sends no bytes. There is no
    /// reconnect/fallback path when any scheduled helper subsequently fails.
    pub fn begin_learned_transport_review(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        schedule: LearnedWorkerSchedule, mut peers: LearnedRoundPeers, snapshot: Snapshot)
        -> Result<FileLearnedTransportReview, FileHelperSetupError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable.into()); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let original = self.checked_learned_sidecar(&sidecar)?;
        if original.round().work().rounds != 1 { return Err(Error::WrongState.into()); }
        if schedule.rounds.is_empty() || schedule.polls == 0 { return Err(Error::InvalidInput.into()); }
        let count = schedule.rounds.len().checked_mul(original.round().input().views().len()).ok_or(Error::Limit)?;
        if schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS
            || schedule.polls > MAX_LEARNED_REVIEW_POLLS || count > MAX_LEARNED_TRANSPORT_PEERS {
            return Err(Error::Limit.into());
        }
        if peers.len() != schedule.rounds.len() { return Err(Error::Binding.into()); }
        let mut ids = BTreeSet::new();
        let mut previous = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        let deadline = original.round().input().action().spec().deadline;
        for round in &schedule.rounds {
            if round.round == 0 || round.evidence_root == [0; 32]
                || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                    && round.window.reveal_by <= deadline) { return Err(Error::InvalidInput.into()); }
            if !ids.insert(round.round) || self.worker_rounds.contains(&round.round)
                || self.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round.round)) {
                return Err(Error::Duplicate.into());
            }
            let roster = peers.get(&round.round).ok_or(Error::Binding)?;
            if roster.keys().ne(original.round().input().views().keys()) { return Err(Error::Binding.into()); }
            for stream in roster.values() {
                stream.set_nonblocking(true).map_err(|error| FileHelperSetupError::Worker(WorkerIoError::Io(error.kind())))?;
            }
            previous = round.window.reveal_by;
        }
        let mut leased = self.worker_rounds.clone(); leased.extend(ids);
        let mut records = BTreeMap::new();
        for round in &schedule.rounds {
            for member in original.round().input().views().keys() {
                records.insert((round.round, member.clone()), LearnedTransportRecord::default());
            }
        }
        let mut history = Vec::new();
        history.try_reserve_exact(schedule.rounds.len()).map_err(|_| Error::Limit)?;
        let first = schedule.rounds[0];
        let (active, input) = prepare(self, &sidecar, &schedule, 0, &mut peers)?;
        self.begin_review(revision, sidecar.attempt(), first.round, first.evidence_root, first.window, snapshot)?;
        self.worker_rounds = leased;
        Ok(FileLearnedTransportReview { issuer: Rc::clone(&self.issuer), sidecar, schedule,
            pending: peers, active: Some(active), input, index: 0, revision: 0, polls: 0,
            status: FileLearnedTransportStatus::Running, failure: None,
            last_statuses: BTreeMap::new(), records, history })
    }
}
impl FileLearnedTransportReview {
    pub fn status(&self) -> FileLearnedTransportStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn current_round(&self) -> LearnedWorkerRound { self.schedule.rounds[self.index] }
    pub fn input_revision(&self) -> u64 { self.sidecar.input_revision() }
    /// Historical operator data; original current-source gates still apply.
    pub fn input(&self) -> &CommitteeInput { &self.input }
    pub fn failure(&self) -> Option<&FileHelperSetupError> { self.failure.as_ref() }
    pub fn records(&self) -> &BTreeMap<(u64, String), LearnedTransportRecord> { &self.records }
    pub fn history(&self) -> &[FileLearnedSidecarFinish] { &self.history }
    pub fn worker_statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.active.as_ref().map_or_else(|| self.last_statuses.clone(), |active| active.coordinator.statuses())
    }
    pub fn next_deadline(&self) -> Option<ElapsedTick> {
        if self.status != FileLearnedTransportStatus::Running { return None; }
        let round = self.current_round();
        let active = self.active.as_ref()?;
        if active.coordinator.elapsed() < round.window.commit_by
            && active.coordinator.statuses().values().any(|status| !status.committed) {
            Some(round.window.commit_by)
        } else { Some(round.window.reveal_by) }
    }
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.status != FileLearnedTransportStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.close(); self.status = FileLearnedTransportStatus::Cancelled;
        Ok(())
    }
    fn close(&mut self) {
        if let Some(mut active) = self.active.take() {
            active.coordinator.close(); self.last_statuses = active.coordinator.statuses();
        }
        self.pending.clear();
    }

    /// One original nonblocking connection step per current member, with original
    /// durable coordination before/after I/O. No next-round bytes are sent here.
    /// `now` is a trusted logical observation, not a wall-clock latency guarantee.
    /// A completed round uses the ORIGINAL witnessed finish/refinement transaction.
    pub fn advance(&mut self, host: &mut FileOversight, expected_revision: u64,
        now: ElapsedTick, snapshot: Snapshot) -> Result<FileLearnedTransportStatus, FileHelperSetupError>
    {
        self.advance_with_clock(host, expected_revision, now, snapshot, || now)
    }

    /// Bracket every original socket operation with fresh operator clock samples.
    /// `observed_at` supplies preflight's monotone predecessor; subsequent samples
    /// include time spent in earlier journal barriers. An arriving reply is only
    /// admitted at its POST-I/O observation, never backdated to the preflight tick.
    /// The original coordinator alone enforces commit/reveal deadlines.
    ///
    /// The clock is a trusted supervisor input, not a helper-controlled timestamp.
    /// It has no reference to the owner, sockets, source, coordinator or verdict.
    /// A panic after admission closes all peers and latches the driver. Calls and
    /// filesystem barriers remain synchronous, not preemptively time-bounded.
    pub fn advance_with_clock<F>(&mut self, host: &mut FileOversight, expected_revision: u64,
        observed_at: ElapsedTick, snapshot: Snapshot, mut clock: F)
        -> Result<FileLearnedTransportStatus, FileHelperSetupError>
    where F: FnMut() -> ElapsedTick {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if expected_revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != FileLearnedTransportStatus::Running { return Err(Error::WrongState.into()); }
        let previous = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if observed_at < previous || self.active.as_ref().is_some_and(|active| observed_at < active.coordinator.elapsed()) {
            return Err(Error::Stale.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = FileLearnedTransportStatus::Failed;
        self.failure = Some(Error::Incomplete.into());
        if self.polls == self.schedule.polls {
            self.close(); self.failure = Some(Error::Limit.into()); return Err(Error::Limit.into());
        }
        self.polls += 1;
        // Stack ownership closes ALL sockets, including future rounds, on an
        // unwind. No half-completed call can restore a previous protocol offset.
        let mut pending = std::mem::take(&mut self.pending);
        let mut active = self.active.take().ok_or(Error::WrongState)?;
        let now = match self.poll(host, observed_at, &mut active, &mut clock) {
            Ok(now) => now,
            Err(error) => {
                active.coordinator.close(); self.last_statuses = active.coordinator.statuses();
                self.failure = Some(error.clone().into()); return Err(error.into());
            }
        };
        self.last_statuses = active.coordinator.statuses();
        let round = self.current_round();
        let ready = now >= round.window.reveal_by || self.last_statuses.values().all(|status| status.revealed);
        if !ready {
            self.pending = pending; self.active = Some(active);
            self.status = FileLearnedTransportStatus::Running; self.failure = None; return Ok(self.status);
        }
        active.coordinator.close(); self.last_statuses = active.coordinator.statuses();
        drop(active);
        let result = (|| {
            let finished = host.finish_learned_sidecar_review_inner(host.revision(), &mut self.sidecar,
                round.round, self.index + 1 < self.schedule.rounds.len(), snapshot.clone())?;
            let refined = matches!(&finished, FileLearnedSidecarFinish::Refined { .. });
            self.history.push(finished);
            if !refined { return Ok(None); }
            self.input = host.current_learned_sidecar(&self.sidecar)?.clone();
            self.index += 1;
            let (next_active, input) = prepare(host, &self.sidecar, &self.schedule, self.index, &mut pending)?;
            let next = self.current_round();
            host.begin_review(host.revision(), self.sidecar.attempt(), next.round,
                next.evidence_root, next.window, snapshot)?;
            Ok::<_, FileHelperSetupError>(Some((next_active, input)))
        })();
        match result {
            Ok(Some((next_active, input))) => {
                self.active = Some(next_active); self.input = input; self.pending = pending;
                self.status = FileLearnedTransportStatus::Running; self.failure = None; Ok(self.status)
            }
            Ok(None) => { self.status = FileLearnedTransportStatus::Finished; self.failure = None; Ok(self.status) }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }
    fn poll<F>(&mut self, host: &mut FileOversight, observed_at: ElapsedTick,
        active: &mut Active, clock: &mut F) -> Result<ElapsedTick, JournalError>
    where F: FnMut() -> ElapsedTick {
        host.checked_learned_sidecar(&self.sidecar)?;
        let round = self.current_round();
        let mut session = DurableSession { host, attempt: self.sidecar.attempt(), round: round.round, inputs: &self.input };
        let mut last = observed_at;
        observe(&self.sidecar, &mut active.coordinator, &mut session, clock, &mut last)?;
        for (member, connection) in &mut active.connections {
            // A slow earlier journal write may have crossed a cutoff. Observe
            // again before sending any next-member request or reveal signal.
            observe(&self.sidecar, &mut active.coordinator, &mut session, clock, &mut last)?;
            let record = self.records.get_mut(&(round.round, member.clone())).expect("frozen peer record");
            record.attempted_steps += 1;
            let result = connection.step();
            record.returned_steps += 1; record.last = Some(result);
            if let Err(error) = result { record.first_failure.get_or_insert(error); }
            // Store actual I/O progress BEFORE clock/commit failure can escape.
            // A just-queued reply past the cutoff remains missing, not accepted.
            observe(&self.sidecar, &mut active.coordinator, &mut session, clock, &mut last)?;
        }
        // Include time spent acknowledging the final member before deciding
        // whether to finish or construct a subsequent round. No deadline slides.
        observe(&self.sidecar, &mut active.coordinator, &mut session, clock, &mut last)?;
        Ok(last)
    }
}
fn observe<F>(handle: &FileLearnedSidecar, coordinator: &mut Coordinator,
    session: &mut DurableSession<'_>, clock: &mut F, previous: &mut ElapsedTick)
    -> Result<(), JournalError>
where F: FnMut() -> ElapsedTick {
    let now = clock();
    if now < *previous { return Err(Error::Stale.into()); }
    // Check both sides of the ORIGINAL clock/protocol transition. A changed
    // source can never buy another wire operation through a clock callback.
    session.host.checked_learned_sidecar(handle)?;
    coordinator.advance(session, now).map_err(journal_error)?;
    *previous = now;
    session.host.checked_learned_sidecar(handle)?;
    Ok(())
}

fn prepare(host: &FileOversight, handle: &FileLearnedSidecar, schedule: &LearnedWorkerSchedule,
    index: usize, pending: &mut LearnedRoundPeers) -> Result<(Active, CommitteeInput), FileHelperSetupError>
{
    let original = host.checked_learned_sidecar(handle)?;
    let round = schedule.rounds.get(index).ok_or(Error::Limit)?;
    let now = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
    let (coordinator, ports) = Coordinator::new(round.round, round.evidence_root, original.round().input(),
        round.window, now, schedule.helpers)?;
    let peers = pending.remove(&round.round).ok_or(Error::Binding)?;
    let connections = connect(ports, peers).map_err(FileHelperSetupError::Worker)?;
    Ok((Active { coordinator, connections }, original.round().input().clone()))
}
