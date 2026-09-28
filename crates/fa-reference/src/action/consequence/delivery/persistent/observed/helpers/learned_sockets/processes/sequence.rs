//! Lazy process launch across witnessed refinement, with a direct-child drain barrier.
use super::*;
use crate::action::consequence::oversight::{CommitteeInput,
    learned_host::sidecar::workers::{LearnedWorkerSchedule, MAX_LEARNED_REVIEW_POLLS},
    sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS};
use super::super::sequence::LearnedSocketRecord;
use super::super::super::Event;
use std::{collections::BTreeSet, fmt};

/// Aggregate direct-child starts admitted across this entire frozen schedule.
/// Each program has the original constructor's text cap; this is not a global
/// subprocess quota, inference escrow or limit on hostile descendants.
pub const MAX_LEARNED_PROCESS_STARTS: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedProcessLimits { pub starts: usize }
impl Default for LearnedProcessLimits {
    fn default() -> Self { Self { starts: MAX_LEARNED_PROCESS_STARTS } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedProcessStatus { Running, Draining, Finished, Cancelled, Failed }

/// Unstarted slots have no admission or OS status. An acknowledged round and
/// observed child exit do not establish that a helper voted or an effect ran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearnedProcessRecord {
    pub admission: Option<HelperRoundAdmission>,
    pub socket: LearnedSocketRecord,
    pub processes: BTreeMap<String, ProcessStatus>,
}
impl LearnedProcessRecord {
    fn capture(&mut self, active: &FileLearnedProcessRound) {
        self.processes = active.process_statuses();
        self.socket.connection_steps = active.connection_steps();
        self.socket.workers = active.progress().workers.clone();
        for (member, result) in &active.progress().io {
            if let Err(error) = result { self.socket.wire_failures.entry(member.clone()).or_insert(*error); }
        }
        self.socket.completed = active.outcome().is_some();
    }
}

/// Owns at most ONE unreaped direct-child roster. Programs for later rounds are
/// fixed at admission but not spawned until original refinement is acknowledged
/// AND the preceding roster is completely reaped. Cleanup remains available
/// after the finite protocol poll allowance is exhausted.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::processes::sequence::FileLearnedProcessReview;
/// fn reroll(review: &mut FileLearnedProcessReview) { review.replace_programs(); }
/// ```
#[must_use = "retain the owner and poll direct-child cleanup after terminal review"]
pub struct FileLearnedProcessReview {
    issuer: Rc<()>,
    schedule: LearnedWorkerSchedule,
    programs: BTreeMap<u64, BTreeMap<String, HelperProgram>>,
    active: Option<FileLearnedProcessRound>,
    // A failed new roster has no socket owner, but can have real started children.
    failed_children: Option<HelperChildren>,
    // An unwound constructor may have lost its child owner. Absence is NOT reap.
    unresolved_startup: Option<u64>,
    index: usize,
    revision: u64,
    polls: usize,
    admitted_starts: usize,
    status: LearnedProcessStatus,
    failure: Option<FileProcessFailure>,
    input: CommitteeInput,
    records: BTreeMap<u64, LearnedProcessRecord>,
    history: Vec<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileLearnedProcessReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedProcessReview").field("status", &self.status)
            .field("round", &self.round()).field("polls", &self.polls)
            .field("all_reaped", &self.all_reaped()).finish_non_exhaustive()
    }
}

impl FileOversight {
    pub fn begin_learned_process_review<F>(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        schedule: LearnedWorkerSchedule, mut programs: BTreeMap<u64, BTreeMap<String, HelperProgram>>,
        limits: LearnedProcessLimits, snapshot: Snapshot, clock: F)
        -> Result<FileLearnedProcessReview, FileHelperProcessError>
    where F: FnMut() -> ElapsedTick {
        let admitted = (|| -> Result<_, FileProcessFailure> {
            if revision != self.revision() { return Err(Error::Stale.into()); }
            let original = self.checked_learned_sidecar(&sidecar)?;
            if original.round().work().rounds != 1 { return Err(Error::WrongState.into()); }
            if schedule.rounds.is_empty() || schedule.polls == 0 || limits.starts == 0 {
                return Err(Error::InvalidInput.into());
            }
            let starts = original.round().input().views().len().checked_mul(schedule.rounds.len()).ok_or(Error::Limit)?;
            if schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS || schedule.polls > MAX_LEARNED_REVIEW_POLLS
                || limits.starts > MAX_LEARNED_PROCESS_STARTS || starts > limits.starts { return Err(Error::Limit.into()); }
            let input = original.round().input().clone();
            let mut previous = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
            let mut ids = BTreeSet::new();
            for round in &schedule.rounds {
                if round.round == 0 || round.evidence_root == [0; 32]
                    || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                        && round.window.reveal_by <= input.action().spec().deadline) { return Err(Error::InvalidInput.into()); }
                if !ids.insert(round.round) { return Err(Error::Duplicate.into()); }
                unused(self, round.round)?;
                previous = round.window.reveal_by;
            }
            if !ids.iter().eq(programs.keys()) { return Err(Error::Binding.into()); }
            for roster in programs.values() {
                if !roster.keys().eq(input.views().keys()) { return Err(Error::Binding.into()); }
                for (member, program) in roster {
                    program.preflight().map_err(|failure| FileProcessFailure::Launch {
                        member: Some(member.clone()), failure,
                    })?;
                }
            }
            let mut history = Vec::new();
            history.try_reserve_exact(schedule.rounds.len()).map_err(|_| Error::Limit)?;
            let records = ids.into_iter().map(|id| (id, LearnedProcessRecord::default())).collect();
            Ok((input, starts, history, records))
        })().map_err(not_started)?;
        let (input, admitted_starts, history, records) = admitted;
        let first = schedule.rounds[0];
        let active = self.begin_learned_process_round(revision, sidecar, LearnedProcessLaunch {
            round: first,
            programs: programs.remove(&first.round).expect("complete program inventory"), limits: schedule.helpers,
        }, snapshot, clock)?;
        let mut owner = FileLearnedProcessReview { issuer: Rc::clone(&self.issuer), schedule, programs,
            active: Some(active), failed_children: None, unresolved_startup: None, index: 0, revision: 0, polls: 0,
            admitted_starts, status: LearnedProcessStatus::Running, failure: None, input, records, history };
        let first_id = owner.round();
        owner.records.get_mut(&first_id).expect("fixed slot").admission = Some(HelperRoundAdmission::Committed);
        owner.capture();
        Ok(owner)
    }
}

impl FileLearnedProcessReview {
    pub fn round(&self) -> u64 { self.schedule.rounds[self.index].round }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn admitted_starts(&self) -> usize { self.admitted_starts }
    pub fn status(&self) -> LearnedProcessStatus { self.status }
    pub fn failure(&self) -> Option<&FileProcessFailure> { self.failure.as_ref() }
    pub fn records(&self) -> &BTreeMap<u64, LearnedProcessRecord> { &self.records }
    pub fn history(&self) -> &[FileLearnedSidecarFinish] { &self.history }
    pub fn input(&self) -> &CommitteeInput { &self.input }
    /// A constructor that did not return cannot establish cleanup. The original
    /// launcher's Drop may have requested stop, but no reap receipt is available.
    pub fn unresolved_startup(&self) -> Option<u64> { self.unresolved_startup }
    pub fn all_reaped(&self) -> bool {
        self.unresolved_startup.is_none() && self.active.as_ref().is_none_or(FileLearnedProcessRound::all_reaped)
            && self.failed_children.as_ref().is_none_or(HelperChildren::all_reaped)
    }
    fn capture(&mut self) {
        let id = self.round();
        let record = self.records.get_mut(&id).expect("fixed process slot");
        if let Some(active) = &self.active { record.capture(active); }
        else if let Some(children) = &self.failed_children { record.processes = children.statuses(); }
    }
    /// Bounded original OS cleanup only. It consumes no protocol poll allowance,
    /// launches nothing, and remains callable after cancellation/failure/finish.
    pub fn reap(&mut self) {
        if let Some(active) = &mut self.active { active.reap(); }
        if let Some(children) = &mut self.failed_children { children.reap(); }
        self.capture();
    }
    fn stop_children(&mut self) {
        if let Some(active) = &mut self.active {
            active.round.close(); active.children.request_stop_all();
        }
        if let Some(children) = &mut self.failed_children { children.request_stop_all(); }
        self.programs.clear();
        self.capture();
    }
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        if revision != self.revision { return Err(Error::Stale); }
        if !matches!(self.status, LearnedProcessStatus::Running | LearnedProcessStatus::Draining) {
            return Err(Error::WrongState);
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.stop_children(); self.status = LearnedProcessStatus::Cancelled; self.failure = None;
        Ok(())
    }
    pub fn advance(&mut self, host: &mut FileOversight, revision: u64, now: ElapsedTick,
        snapshot: Snapshot) -> Result<LearnedProcessStatus, FileProcessFailure>
    {
        self.advance_with_clock(host, revision, || now, snapshot)
    }
    pub fn advance_with_clock<F>(&mut self, host: &mut FileOversight, revision: u64,
        mut clock: F, snapshot: Snapshot) -> Result<LearnedProcessStatus, FileProcessFailure>
    where F: FnMut() -> ElapsedTick {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if !matches!(self.status, LearnedProcessStatus::Running | LearnedProcessStatus::Draining) {
            return Err(Error::WrongState.into());
        }
        let mut operation = ReviewOperation { owner: self, returned: false };
        let result = operation.owner.advance_inner(host, &mut clock, snapshot);
        operation.returned = true;
        result
    }
    fn advance_inner<F>(&mut self, host: &mut FileOversight, clock: &mut F, snapshot: Snapshot)
        -> Result<LearnedProcessStatus, FileProcessFailure>
    where F: FnMut() -> ElapsedTick {
        let now = clock();
        if now < host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)? { return Err(Error::Stale.into()); }
        let draining = self.status == LearnedProcessStatus::Draining;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = LearnedProcessStatus::Failed; self.failure = Some(Error::Incomplete.into());
        let result = (|| -> Result<LearnedProcessStatus, FileProcessFailure> {
            if self.polls == self.schedule.polls { return Err(Error::Limit.into()); }
            self.polls += 1;
            if draining { return self.start_successor(host, now, snapshot, clock); }
            let id = self.round();
            self.records.get_mut(&id).expect("fixed slot").socket.polls += 1;
            let active = self.active.as_mut().expect("running original round");
            let mut first = Some(now);
            let pumped = active.pump_with_clock(host, active.revision(), || first.take().unwrap_or_else(|| clock()));
            self.capture();
            pumped.map_err(|failure| FileProcessFailure::Journal(failure.error))?;
            let active = self.active.as_mut().expect("pumped original round");
            if !active.ready_to_finish() { return Ok(LearnedProcessStatus::Running); }
            let next = self.schedule.rounds.get(self.index + 1);
            let more = next.is_some_and(|round| host.inspect().control.ledger.elapsed
                .is_some_and(|tick| tick < round.window.commit_by));
            if more { unused(host, next.expect("next window").round)?; }
            let result = active.finish_bound(host, active.revision(), more, snapshot);
            self.capture();
            let result = result?;
            let refined = matches!(&result, FileLearnedSidecarFinish::Refined { .. });
            self.history.push(result);
            if !refined { return Ok(LearnedProcessStatus::Finished); }
            self.input = host.current_learned_sidecar(&self.active.as_ref().expect("finished round").round.sidecar)?.clone();
            // Yield after acknowledged refinement. No successor launch happens in
            // this call, even if the earlier children have already exited.
            Ok(LearnedProcessStatus::Draining)
        })();
        match result {
            Ok(status) => { self.status = status; self.failure = None; Ok(status) }
            Err(error) => { self.stop_children(); self.failure = Some(error.clone()); Err(error) }
        }
    }
    fn start_successor<F>(&mut self, host: &mut FileOversight, now: ElapsedTick,
        snapshot: Snapshot, clock: &mut F) -> Result<LearnedProcessStatus, FileProcessFailure>
    where F: FnMut() -> ElapsedTick {
        if host.inspect().control.ledger.elapsed != Some(now) { host.observe_time(host.revision(), now)?; }
        let current = self.active.as_mut().ok_or(Error::Incomplete)?;
        host.current_learned_sidecar(&current.round.sidecar)?;
        let next = *self.schedule.rounds.get(self.index + 1).ok_or(Error::Incomplete)?;
        unused(host, next.round)?;
        if now >= next.window.commit_by { return Err(Error::Stale.into()); }
        self.reap();
        if !self.all_reaped() { return Ok(LearnedProcessStatus::Draining); }
        let previous = self.active.take().expect("drained predecessor");
        // All direct children have reaping confirmation BEFORE the next Begin or
        // spawn. Only the SAME sidecar moves, retaining its disclosure spending.
        let sidecar = previous.round.sidecar;
        let programs = self.programs.remove(&next.round).ok_or(Error::Missing)?;
        self.index += 1;
        // Mark uncertainty BEFORE entering code that can spawn or unwind. Clear
        // it only when that code actually returns an owner or structured error.
        self.unresolved_startup = Some(next.round);
        self.records.get_mut(&next.round).expect("fixed slot").admission = Some(HelperRoundAdmission::Unknown);
        let started = host.begin_learned_process_round(host.revision(), sidecar, LearnedProcessLaunch {
            round: next,
            programs, limits: self.schedule.helpers,
        }, snapshot, clock);
        self.unresolved_startup = None;
        match started {
            Ok(active) => {
                self.active = Some(active);
                self.records.get_mut(&next.round).expect("fixed slot").admission = Some(HelperRoundAdmission::Committed);
                self.capture(); Ok(LearnedProcessStatus::Running)
            }
            Err(error) => {
                self.records.get_mut(&next.round).expect("fixed slot").admission = Some(error.admission);
                self.failed_children = error.children;
                self.capture(); Err(error.failure)
            }
        }
    }
    /// Transfer only the at-most-one outstanding direct-child owner for cleanup.
    /// None means no recoverable owner, NOT confirmation of reaping: inspect
    /// unresolved_startup before consuming an owner whose constructor unwound.
    pub fn into_children(mut self) -> Option<HelperChildren> {
        self.stop_children();
        match self.active.take() {
            Some(active) => Some(active.into_children()),
            None => self.failed_children.take(),
        }
    }
}

struct ReviewOperation<'a> { owner: &'a mut FileLearnedProcessReview, returned: bool }
impl Drop for ReviewOperation<'_> {
    fn drop(&mut self) {
        if !self.returned {
            self.owner.stop_children(); self.owner.status = LearnedProcessStatus::Failed;
            self.owner.failure = Some(Error::Incomplete.into());
        }
    }
}
fn unused(host: &FileOversight, round: u64) -> Result<(), Error> {
    if host.worker_rounds.contains(&round)
        || host.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round)) {
        Err(Error::Duplicate)
    } else { Ok(()) }
}
