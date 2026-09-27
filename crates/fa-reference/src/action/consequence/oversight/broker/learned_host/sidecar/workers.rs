//! Bounded independent-worker review of one original learned sidecar.
//! The original HelperRound consumes replies; the original planner buys evidence.
#[cfg(unix)]
pub mod transport;
pub mod probes;

use super::{LearnedSidecar, OversightBroker};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{CommitteeInput, ObservedReview, ReviewWindow,
    helper_workers::{HelperLimits, HelperPort, HelperRound, HelperStatus, MAX_WORKER_SALT_BYTES},
    replay::ObservedDecisionArchive,
    sidecar::{SidecarRefinementOutcome, MAX_SIDECAR_REFINEMENT_ROUNDS}, MAX_COMMITTEE_BYTES};
use crate::reducer::MAX_VOTES;
use crate::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_LEARNED_REVIEW_POLLS: usize = 65_536;

/// Caller-retained reference roots are not cryptographic source authentication.
/// All identities/windows freeze before this sequence releases its first ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedWorkerRound {
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
}
#[derive(Clone, Debug)]
pub struct LearnedWorkerSchedule {
    pub rounds: Vec<LearnedWorkerRound>,
    pub helpers: HelperLimits,
    /// Includes empty polls. Expiry is never manufactured to finish early.
    pub polls: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedWorkerStop { Decided, Missing, Unresolved, RefinementBudget, RoundLimit }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedWorkerStatus { Running, Stopped(LearnedWorkerStop), Cancelled, Failed(Error) }

/// Fresh, non-cloneable ports for the NEXT original round, never replacement
/// ports for a missing worker in the same round. Only the host provisions them.
#[derive(Debug)]
pub enum LearnedWorkerUpdate {
    Waiting,
    NextRound(BTreeMap<String, HelperPort>),
    Stopped,
}

/// Consumes the sidecar so the caller cannot retune/reopen its disclosure plan.
/// No coordinator vote API, mutable arm or automatic permission path exists.
/// Retains at most 32 complete original review archives, each bounded by the
/// per-round helper input cap. This is not a peak-RSS or a global research escrow.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::LearnedWorkerReview;
/// fn override_vote(run: &mut LearnedWorkerReview) { run.submit_verdict(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::LearnedWorkerReview;
/// fn replenish(run: LearnedWorkerReview) { run.into_sidecar(); }
/// ```
pub struct LearnedWorkerReview {
    sidecar: LearnedSidecar,
    schedule: LearnedWorkerSchedule,
    index: usize,
    revision: u64,
    polls: usize,
    status: LearnedWorkerStatus,
    active: Option<HelperRound>,
    last_statuses: BTreeMap<String, HelperStatus>,
    history: Vec<ObservedDecisionArchive>,
    review: Option<ObservedReview>,
}
impl fmt::Debug for LearnedWorkerReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedWorkerReview").field("status", &self.status)
            .field("round", &self.current_round()).field("polls", &self.polls).finish_non_exhaustive()
    }
}
impl LearnedWorkerReview {
    pub fn status(&self) -> LearnedWorkerStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn current_round(&self) -> LearnedWorkerRound { self.schedule.rounds[self.index] }
    pub fn input_revision(&self) -> u64 { self.sidecar.input_revision() }
    /// Historical input; apply_review/authorize still require original currentness.
    pub fn input(&self) -> &CommitteeInput { self.sidecar.round().input() }
    pub fn history(&self) -> &[ObservedDecisionArchive] { &self.history }
    pub fn worker_statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.active.as_ref().map_or_else(|| self.last_statuses.clone(), HelperRound::statuses)
    }
    /// Exactly the original review, including missing/abstaining members. Taking
    /// it neither applies a consequence nor authorizes the action. Source loss
    /// after completion still blocks a permitting original apply/dispatch.
    pub fn take_review(&mut self) -> Result<ObservedReview, Error> {
        if !matches!(self.status, LearnedWorkerStatus::Stopped(_)) { return Err(Error::WrongState); }
        self.review.take().ok_or(Error::WrongState)
    }
    /// Abandon worker work, not the action or a dispatched effect. No rights or
    /// input-retention budget is refunded, and there is no restart on this owner.
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.status != LearnedWorkerStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.close_active();
        self.status = LearnedWorkerStatus::Cancelled;
        Ok(())
    }
    fn close_active(&mut self) {
        if let Some(round) = self.active.take() { self.last_statuses = round.statuses(); }
    }
    fn stopped(&mut self, reason: LearnedWorkerStop, review: ObservedReview) -> LearnedWorkerUpdate {
        self.review = Some(review);
        self.status = LearnedWorkerStatus::Stopped(reason);
        LearnedWorkerUpdate::Stopped
    }
}
impl OversightBroker {
    pub fn begin_learned_worker_review(&mut self, sidecar: LearnedSidecar,
        schedule: LearnedWorkerSchedule, snapshot: &Snapshot)
        -> Result<(LearnedWorkerReview, BTreeMap<String, HelperPort>), Error>
    {
        self.check_learned_sidecar(&sidecar)?;
        if sidecar.round().work().rounds != 1 { return Err(Error::WrongState); }
        if schedule.rounds.is_empty() || schedule.polls == 0 { return Err(Error::InvalidInput); }
        if schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS || schedule.polls > MAX_LEARNED_REVIEW_POLLS {
            return Err(Error::Limit);
        }
        check_helpers(schedule.helpers, sidecar.round().input())?;
        let deadline = sidecar.round().input().action().spec().deadline;
        let mut previous = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        let mut ids = BTreeSet::new();
        for round in &schedule.rounds {
            if round.round == 0 || round.evidence_root == [0; 32]
                || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                    && round.window.reveal_by <= deadline) { return Err(Error::InvalidInput); }
            if !ids.insert(round.round) || self.started_rounds.contains(&round.round) { return Err(Error::Duplicate); }
            previous = round.window.reveal_by;
        }
        let mut history = Vec::new();
        history.try_reserve_exact(schedule.rounds.len()).map_err(|_| Error::Limit)?;
        let first = schedule.rounds[0];
        let session = self.begin_learned_sidecar_review(&sidecar, first.round, first.evidence_root, first.window, snapshot)?;
        let (active, ports) = HelperRound::new(session, schedule.helpers)?;
        Ok((LearnedWorkerReview { sidecar, schedule, index: 0, revision: 0, polls: 0,
            status: LearnedWorkerStatus::Running, active: Some(active), last_statuses: BTreeMap::new(),
            history, review: None }, ports))
    }

    /// At most one queued reply per member in one ORIGINAL round. No user callback
    /// runs while source validation, receipt-time observation and refinement occur.
    /// A completed abstention may prepare the next round, but cannot poll it here.
    pub fn advance_learned_worker_review(&mut self, run: &mut LearnedWorkerReview,
        expected_revision: u64, now: ElapsedTick, snapshot: &Snapshot)
        -> Result<LearnedWorkerUpdate, Error>
    {
        self.advance_learned_workers_with_io(run, expected_revision, now, snapshot, || {})
    }

    fn check_learned_worker_call(&self, run: &LearnedWorkerReview, expected_revision: u64,
        now: ElapsedTick) -> Result<(), Error>
    {
        if !Rc::ptr_eq(&self.issuer, &run.sidecar.issuer) { return Err(Error::Binding); }
        if expected_revision != run.revision { return Err(Error::Stale); }
        if run.status != LearnedWorkerStatus::Running { return Err(Error::WrongState); }
        let elapsed = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < elapsed || run.active.as_ref().is_some_and(|round| now < round.elapsed()) { return Err(Error::Stale); }
        Ok(())
    }

    // Only crate-owned adapters supply this hook. Source validation and the
    // unwind/port guard precede I/O; callers cannot insert arbitrary callbacks.
    fn advance_learned_workers_with_io(&mut self, run: &mut LearnedWorkerReview,
        expected_revision: u64, now: ElapsedTick, snapshot: &Snapshot, before_poll: impl FnOnce())
        -> Result<LearnedWorkerUpdate, Error>
    {
        self.check_learned_worker_call(run, expected_revision, now)?;
        run.revision = run.revision.checked_add(1).ok_or(Error::Overflow)?;
        run.status = LearnedWorkerStatus::Failed(Error::Incomplete);
        if run.polls == run.schedule.polls {
            run.close_active(); run.status = LearnedWorkerStatus::Failed(Error::Limit); return Err(Error::Limit);
        }
        run.polls += 1;
        // The local round is dropped on errors OR unwinds, closing original ports
        // immediately. The poisoned status prevents retry after partial work.
        let mut active = run.active.take().ok_or(Error::WrongState)?;
        let result = (|| {
            self.observe_time(now)?;
            self.check_learned_sidecar(&run.sidecar)?;
            before_poll();
            match active.finish(now) {
                Err(Error::Incomplete) => Ok(None),
                Err(error) => Err(error),
                Ok(review) => Ok(Some(review)),
            }
        })();
        run.last_statuses = active.statuses();
        let review = match result {
            Ok(None) => { run.active = Some(active); run.status = LearnedWorkerStatus::Running; return Ok(LearnedWorkerUpdate::Waiting); }
            Ok(Some(review)) => review,
            Err(error) => { run.status = LearnedWorkerStatus::Failed(error); return Err(error); }
        };
        // Keep every completed original transcript, including failed/missing
        // worker slots. No success-only replacement of earlier coarse reviews.
        run.history.push(review.replay_archive());
        let result = self.continue_learned_workers(run, review, snapshot);
        if let Err(error) = result { run.status = LearnedWorkerStatus::Failed(error); run.close_active(); }
        result
    }

    fn continue_learned_workers(&mut self, run: &mut LearnedWorkerReview, review: ObservedReview,
        snapshot: &Snapshot) -> Result<LearnedWorkerUpdate, Error>
    {
        if !review.missing().is_empty() { return Ok(run.stopped(LearnedWorkerStop::Missing, review)); }
        if review.abstained().is_empty() { return Ok(run.stopped(LearnedWorkerStop::Decided, review)); }
        if run.index + 1 == run.schedule.rounds.len() { return Ok(run.stopped(LearnedWorkerStop::RoundLimit, review)); }
        match self.refine_learned_sidecar(&mut run.sidecar, &review)? {
            SidecarRefinementOutcome::Refined { .. } => {}
            SidecarRefinementOutcome::BudgetExhausted { .. } => return Ok(run.stopped(LearnedWorkerStop::RefinementBudget, review)),
            SidecarRefinementOutcome::Unresolved { .. } => return Ok(run.stopped(LearnedWorkerStop::Unresolved, review)),
            SidecarRefinementOutcome::Missing { .. } => return Ok(run.stopped(LearnedWorkerStop::Missing, review)),
            SidecarRefinementOutcome::Final => return Ok(run.stopped(LearnedWorkerStop::Decided, review)),
        }
        run.index += 1;
        let next = run.schedule.rounds[run.index];
        check_helpers(run.schedule.helpers, run.input())?;
        let session = self.begin_learned_sidecar_review(&run.sidecar, next.round, next.evidence_root, next.window, snapshot)?;
        let (active, ports) = HelperRound::new(session, run.schedule.helpers)?;
        run.active = Some(active);
        run.status = LearnedWorkerStatus::Running;
        Ok(LearnedWorkerUpdate::NextRound(ports))
    }
}

fn check_helpers(limits: HelperLimits, input: &CommitteeInput) -> Result<(), Error> {
    if limits.members == 0 || limits.input_bytes == 0 || limits.salt_bytes == 0 { return Err(Error::InvalidInput); }
    if limits.members > MAX_VOTES || limits.input_bytes > MAX_COMMITTEE_BYTES
        || limits.salt_bytes > MAX_WORKER_SALT_BYTES || input.views().len() > limits.members
        || input.logical_bytes() > limits.input_bytes { return Err(Error::Limit); }
    Ok(())
}
