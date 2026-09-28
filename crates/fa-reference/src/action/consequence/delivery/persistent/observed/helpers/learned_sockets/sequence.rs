//! Fixed external-worker rounds and original durable abstention refinement.
use super::*;
use super::super::Event;
use crate::action::consequence::oversight::{CommitteeInput, helper_workers::{HelperStatus, io::WorkerIoError},
    learned_host::sidecar::workers::{LearnedWorkerSchedule, MAX_LEARNED_REVIEW_POLLS},
    sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS};
use std::collections::BTreeSet;

/// Original worker state and first wire failures for each declared round.
/// A never-started round has zero polls and no observed workers, not quiet votes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearnedSocketRecord {
    pub polls: usize,
    pub connection_steps: u64,
    pub workers: BTreeMap<String, HelperStatus>,
    pub wire_failures: BTreeMap<String, WorkerIoError>,
    pub completed: bool,
}
impl LearnedSocketRecord {
    fn capture(&mut self, round: &FileLearnedSocketRound) {
        self.connection_steps = round.connection_steps();
        self.workers = round.progress().workers.clone();
        for (member, result) in &round.progress().io {
            if let Err(error) = result { self.wire_failures.entry(member.clone()).or_insert(*error); }
        }
        self.completed = round.outcome().is_some();
    }
}

/// Every future member/socket roster is fixed before the first request is sent.
/// No round, socket, source, deadline, verdict or budget can be replaced. The
/// source-bound single-round owner executes every actual protocol transition.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::sequence::FileLearnedSocketReview;
/// fn retry(review: &mut FileLearnedSocketReview) { review.replace_socket(); }
/// ```
pub struct FileLearnedSocketReview {
    issuer: Rc<()>,
    schedule: LearnedWorkerSchedule,
    future: BTreeMap<u64, BTreeMap<String, UnixStream>>,
    active: Option<FileLearnedSocketRound>,
    index: usize,
    revision: u64,
    polls: usize,
    status: LearnedSocketStatus,
    failure: Option<FileHelperSetupError>,
    input: CommitteeInput,
    records: BTreeMap<u64, LearnedSocketRecord>,
    history: Vec<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileLearnedSocketReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedSocketReview").field("status", &self.status)
            .field("round", &self.round()).field("polls", &self.polls).finish_non_exhaustive()
    }
}
impl FileOversight {
    /// All socket setup and schedule checks precede the original first Begin.
    /// Future round IDs are checked, not leased globally: a concurrent use makes
    /// this sequence refuse, never choose another ID or accept that other review.
    pub fn begin_learned_socket_review(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        schedule: LearnedWorkerSchedule, mut streams: BTreeMap<u64, BTreeMap<String, UnixStream>>,
        snapshot: Snapshot) -> Result<FileLearnedSocketReview, FileHelperSetupError>
    {
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let original = self.checked_learned_sidecar(&sidecar)?;
        if original.round().work().rounds != 1 { return Err(Error::WrongState.into()); }
        if schedule.rounds.is_empty() || schedule.polls == 0 { return Err(Error::InvalidInput.into()); }
        if schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS || schedule.polls > MAX_LEARNED_REVIEW_POLLS {
            return Err(Error::Limit.into());
        }
        let input = original.round().input().clone();
        let mut previous = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        let mut ids = BTreeSet::new();
        for round in &schedule.rounds {
            if round.round == 0 || round.evidence_root == [0; 32]
                || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                    && round.window.reveal_by <= input.action().spec().deadline) { return Err(Error::InvalidInput.into()); }
            if !ids.insert(round.round) { return Err(Error::Duplicate.into()); }
            unused_round(self, round.round)?;
            previous = round.window.reveal_by;
        }
        if !ids.iter().eq(streams.keys()) { return Err(Error::Binding.into()); }
        for roster in streams.values() {
            if !roster.keys().eq(input.views().keys()) { return Err(Error::Binding.into()); }
            for stream in roster.values() {
                stream.set_nonblocking(true).map_err(|error| FileHelperSetupError::Worker(WorkerIoError::Io(error.kind())))?;
            }
        }
        let records = ids.into_iter().map(|id| (id, LearnedSocketRecord::default())).collect();
        let mut history = Vec::new();
        history.try_reserve_exact(schedule.rounds.len()).map_err(|_| Error::Limit)?;
        let first = schedule.rounds[0];
        let active = self.begin_learned_socket_round(revision, sidecar, LearnedSocketLaunch {
            round: first.round, evidence_root: first.evidence_root, window: first.window,
            streams: streams.remove(&first.round).expect("validated complete inventory"), limits: schedule.helpers,
        }, snapshot)?;
        Ok(FileLearnedSocketReview { issuer: Rc::clone(&self.issuer), schedule, future: streams,
            active: Some(active), index: 0, revision: 0, polls: 0, status: LearnedSocketStatus::Running,
            failure: None, input, records, history })
    }
}
impl FileLearnedSocketReview {
    pub fn round(&self) -> u64 { self.schedule.rounds[self.index].round }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn status(&self) -> LearnedSocketStatus { self.status }
    pub fn failure(&self) -> Option<&FileHelperSetupError> { self.failure.as_ref() }
    pub fn records(&self) -> &BTreeMap<u64, LearnedSocketRecord> { &self.records }
    pub fn history(&self) -> &[FileLearnedSidecarFinish] { &self.history }
    /// Retained evidence only; original currentness still gates effect keys.
    pub fn input(&self) -> &CommitteeInput { &self.input }
    pub fn advance(&mut self, host: &mut FileOversight, revision: u64, now: ElapsedTick,
        snapshot: Snapshot) -> Result<LearnedSocketStatus, FileHelperSetupError>
    {
        self.advance_with_clock(host, revision, || now, snapshot)
    }
    /// At most one bounded I/O pass in the CURRENT round. Completion can commit
    /// original refinement and create a new round, but cannot send its first byte
    /// during this invocation. Empty passes consume the fixed lifetime poll cap.
    pub fn advance_with_clock<F>(&mut self, host: &mut FileOversight, revision: u64,
        mut clock: F, snapshot: Snapshot) -> Result<LearnedSocketStatus, FileHelperSetupError>
    where F: FnMut() -> ElapsedTick {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != LearnedSocketStatus::Running { return Err(Error::WrongState.into()); }
        let now = clock();
        if now < host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)? { return Err(Error::Stale.into()); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = LearnedSocketStatus::Failed; self.failure = Some(Error::Incomplete.into());
        // Both active AND unused descriptors are stack-owned across callbacks and
        // synchronous durability. A caught unwind cannot leave a live retry path.
        let mut active = self.active.take();
        let mut future = std::mem::take(&mut self.future);
        let result: Result<bool, FileHelperSetupError> = (|| {
            if self.polls == self.schedule.polls { return Err(Error::Limit.into()); }
            self.polls += 1;
            let current = active.as_mut().expect("running sequence");
            let record = self.records.get_mut(&current.round()).expect("fixed record");
            record.polls += 1;
            let mut first = Some(now);
            let pumped = current.pump_with_clock(host, current.revision(), || {
                if let Some(tick) = first.take() { tick } else { clock() }
            });
            record.capture(current);
            pumped.map_err(|failure| FileHelperSetupError::Journal(failure.error))?;
            if !current.ready_to_finish() { return Ok(false); }
            let next = self.schedule.rounds.get(self.index + 1).copied();
            // A missed future window cannot justify buying another residual.
            let more = next.is_some_and(|round| host.inspect().control.ledger.elapsed
                .is_some_and(|now| now < round.window.commit_by));
            if more { unused_round(host, next.expect("selected next round").round)?; }
            let finished = current.finish_bound(host, current.revision(), more, snapshot.clone());
            let finished = finished.map(Clone::clone);
            self.records.get_mut(&current.round()).expect("fixed record").capture(current);
            let finished = finished?;
            let refined = matches!(&finished, FileLearnedSidecarFinish::Refined { .. });
            self.history.push(finished);
            if !refined { return Ok(true); }
            self.input = host.current_learned_sidecar(&current.sidecar)?.clone();
            let next = next.ok_or(Error::Binding)?;
            let streams = future.remove(&next.round).ok_or(Error::Missing)?;
            // Only this owner can move the existing sidecar into its successor.
            // No new planner is constructed and no disclosure spending is reset.
            let sidecar = active.take().expect("completed predecessor").sidecar;
            let next_active = host.begin_learned_socket_round(host.revision(), sidecar, LearnedSocketLaunch {
                round: next.round, evidence_root: next.evidence_root, window: next.window,
                streams, limits: self.schedule.helpers,
            }, snapshot)?;
            self.index += 1;
            self.active = Some(next_active);
            Ok(false)
        })();
        match result {
            Ok(done) => {
                // active was moved only if the new round has been installed.
                if self.active.is_none() && !done { self.active = active; }
                self.future = if done { BTreeMap::new() } else { future };
                self.status = if done { LearnedSocketStatus::Finished } else { LearnedSocketStatus::Running };
                self.failure = None; Ok(self.status)
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        if revision != self.revision { return Err(Error::Stale); }
        if self.status != LearnedSocketStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = LearnedSocketStatus::Failed; self.failure = Some(Error::Incomplete.into());
        if let Some(mut active) = self.active.take() {
            active.cancel(active.revision())?;
            self.records.get_mut(&active.round()).expect("fixed record").capture(&active);
        }
        self.future.clear(); self.status = LearnedSocketStatus::Cancelled; self.failure = None; Ok(())
    }
}
fn unused_round(host: &FileOversight, round: u64) -> Result<(), Error> {
    if host.worker_rounds.contains(&round)
        || host.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round)) {
        Err(Error::Duplicate)
    } else { Ok(()) }
}
