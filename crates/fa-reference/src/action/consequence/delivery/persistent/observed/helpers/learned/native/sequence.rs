//! Fixed native-model rosters through original durable residual refinement.
use super::*;
use crate::action::consequence::oversight::sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS;

/// Provision every fresh original native owner before the first round. No
/// evaluator, query, salt, round, window or resource ceiling can be replaced.
pub type NativeSequenceRosters = BTreeMap<u64, BTreeMap<String, NativeReviewMember>>;

/// Frozen members include unstarted rounds: None is not a quiet observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSequenceRecord {
    pub started: bool,
    pub polls: usize,
    /// The last call did not return; retained counters can omit interrupted work.
    pub interrupted: bool,
    pub members: BTreeMap<String, Option<NativeMemberRecord>>,
    pub workers: BTreeMap<String, HelperStatus>,
    pub completed: bool,
}
impl NativeSequenceRecord {
    fn capture(&mut self, current: &FileNativeSidecarReview) {
        self.started = true;
        for (member, record) in current.records() {
            *self.members.get_mut(member).expect("frozen complete native roster") = Some(*record);
        }
        self.workers = current.worker_statuses();
        self.completed = current.outcome().is_some();
    }
}

/// A richer packet must receive fresh original probes AND native-model judgment.
/// Only the original witnessed finish decides whether another residual is bought.
/// Native owners are never reset/reused or replaced after their answer is known.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::native::sequence::FileNativeSidecarSequence;
/// fn retune(run: &mut FileNativeSidecarSequence) { run.replace_members(); }
/// ```
pub struct FileNativeSidecarSequence {
    issuer: Rc<()>,
    rounds: Vec<LearnedWorkerRound>,
    future: NativeSequenceRosters,
    limits: NativeReviewLimits,
    reservation: NativeReviewCost,
    active: Option<FileNativeSidecarReview>,
    index: usize,
    revision: u64,
    polls: usize,
    status: NativeReviewStatus,
    failure: Option<JournalError>,
    input: CommitteeInput,
    records: BTreeMap<u64, NativeSequenceRecord>,
    history: Vec<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileNativeSidecarSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileNativeSidecarSequence").field("status", &self.status)
            .field("round", &self.round()).field("polls", &self.polls).finish_non_exhaustive()
    }
}
impl NativeReviewCost {
    /// Sum ORIGINAL native-policy reservations over every provisioned round.
    /// The sequence admits this total once; terminal and unused work cannot make
    /// budget available for replacement rosters. No runtime or FLOP claim follows.
    pub fn required_sequence(rosters: &NativeSequenceRosters) -> Result<Self, Error> {
        if rosters.is_empty() { return Err(Error::InvalidInput); }
        if rosters.len() > MAX_SIDECAR_REFINEMENT_ROUNDS { return Err(Error::Limit); }
        let cost = rosters.values().try_fold(Self::default(), |sum, members| sum.add(Self::required(members)?))?;
        NativeReviewLimits::default().native.admits(cost)?;
        Ok(cost)
    }
}
impl FileOversight {
    /// Freeze complete future original-model rosters and all native reservations
    /// before Begin. limits.polls and limits.native apply to the ENTIRE sequence;
    /// receive/probe/helper limits retain their original per-evaluation scope.
    /// Every round ID is leased against manual phase fallback in this owner.
    pub fn begin_native_sidecar_sequence(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        rounds: Vec<LearnedWorkerRound>, mut rosters: NativeSequenceRosters,
        limits: NativeReviewLimits, snapshot: Snapshot) -> Result<FileNativeSidecarSequence, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if rounds.is_empty() || limits.polls == 0 { return Err(Error::InvalidInput.into()); }
        if rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS || limits.polls > MAX_LEARNED_REVIEW_POLLS {
            return Err(Error::Limit.into());
        }
        let reservation = NativeReviewCost::required_sequence(&rosters)?;
        limits.native.admits(reservation)?;
        let original = self.checked_learned_sidecar(&sidecar)?;
        if original.round().work().rounds != 1 { return Err(Error::WrongState.into()); }
        let mut previous = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        let mut ids = BTreeSet::new();
        for round in &rounds {
            if round.round == 0 || round.evidence_root == [0; 32]
                || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                    && round.window.reveal_by <= original.round().input().action().spec().deadline) {
                return Err(Error::InvalidInput.into());
            }
            if !ids.insert(round.round) || self.worker_rounds.contains(&round.round)
                || self.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round.round)) {
                return Err(Error::Duplicate.into());
            }
            previous = round.window.reveal_by;
        }
        if ids.iter().ne(rosters.keys()) { return Err(Error::Binding.into()); }
        let mut records = BTreeMap::new();
        for (round, members) in &rosters {
            if members.keys().ne(original.round().input().views().keys()) { return Err(Error::Binding.into()); }
            for (name, member) in members { check_member(member, original, name, limits)?; }
            records.insert(*round, NativeSequenceRecord { started: false, polls: 0, interrupted: false,
                members: members.keys().map(|name| (name.clone(), None)).collect(),
                workers: BTreeMap::new(), completed: false });
        }
        let input = original.round().input().clone();
        let mut history = Vec::new();
        history.try_reserve_exact(rounds.len()).map_err(|_| Error::Limit)?;
        let mut leased = self.worker_rounds.clone(); leased.extend(ids);
        let first = rounds[0];
        let active = self.begin_native_sidecar_review(revision, sidecar, first,
            rosters.remove(&first.round).expect("validated first roster"), limits, snapshot)?;
        self.worker_rounds = leased;
        records.get_mut(&first.round).expect("fixed first record").capture(&active);
        Ok(FileNativeSidecarSequence { issuer: Rc::clone(&self.issuer), rounds, future: rosters,
            limits, reservation, active: Some(active), index: 0, revision: 0, polls: 0,
            status: NativeReviewStatus::Running, failure: None, input, records, history })
    }
}
impl FileNativeSidecarSequence {
    pub fn round(&self) -> LearnedWorkerRound { self.rounds[self.index] }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn status(&self) -> NativeReviewStatus { self.status }
    pub fn reservation(&self) -> NativeReviewCost { self.reservation }
    pub fn failure(&self) -> Option<&JournalError> { self.failure.as_ref() }
    pub fn records(&self) -> &BTreeMap<u64, NativeSequenceRecord> { &self.records }
    pub fn history(&self) -> &[FileLearnedSidecarFinish] { &self.history }
    /// Historical latest input, not proof of current source or either effect key.
    pub fn input(&self) -> &CommitteeInput { &self.input }

    /// One ORIGINAL per-member quantum in the current round. A transition may
    /// durably refine and begin its provisioned successor, but never evaluates a
    /// successor probe/model token in this call. Deadlines and polls never slide.
    pub fn advance(&mut self, host: &mut FileOversight, revision: u64, now: ElapsedTick,
        snapshot: Snapshot) -> Result<NativeReviewStatus, JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != NativeReviewStatus::Running { return Err(Error::WrongState.into()); }
        if now < host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?
            || self.active.as_ref().is_some_and(|current| current.active.as_ref().is_some_and(|a| now < a.elapsed())) {
            return Err(Error::Stale.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = NativeReviewStatus::Failed; self.failure = Some(Error::Incomplete.into());
        // Own ALL remaining models on the stack across potentially unwinding
        // numerical/journal calls. Failure cannot leave another usable roster.
        let mut active = self.active.take();
        let mut future = std::mem::take(&mut self.future);
        let result: Result<bool, JournalError> = (|| {
            if self.polls == self.limits.polls { return Err(Error::Limit.into()); }
            self.polls += 1;
            let current = active.as_mut().ok_or(Error::WrongState)?;
            let round = current.round().round;
            let next = self.rounds.get(self.index + 1).copied();
            let more = self.polls < self.limits.polls && next.is_some_and(|r| now < r.window.commit_by);
            let record = self.records.get_mut(&round).expect("fixed current record");
            record.polls += 1; record.interrupted = true;
            let polled = current.advance_bound(host, current.revision(), now, snapshot.clone(), more);
            record.capture(current); record.interrupted = false;
            if polled? == NativeReviewStatus::Running { return Ok(false); }
            let finished = current.outcome().ok_or(Error::Incomplete)?.clone();
            let refined = matches!(&finished, FileLearnedSidecarFinish::Refined { .. });
            self.history.push(finished);
            if !refined { return Ok(true); }
            // The acknowledged refinement and its costs remain even when the
            // richer receiver/input cannot fit its independently fixed limits.
            self.input = host.current_learned_sidecar(&current.sidecar)?.clone();
            let next = next.ok_or(Error::Binding)?;
            let members = future.remove(&next.round).ok_or(Error::Missing)?;
            let sidecar = active.take().expect("completed original round").sidecar;
            let successor = host.begin_native_sidecar_review_bound(NativeRoundAdmission::Leased(host.revision()),
                sidecar, next, members, self.limits, snapshot)?;
            self.index += 1;
            self.records.get_mut(&next.round).expect("fixed successor record").capture(&successor);
            self.active = Some(successor);
            Ok(false)
        })();
        match result {
            Ok(done) => {
                if !done && self.active.is_none() { self.active = active; }
                if !done { self.future = future; }
                self.status = if done { NativeReviewStatus::Finished } else { NativeReviewStatus::Running };
                self.failure = None; Ok(self.status)
            }
            Err(error) => {
                if let Some(mut current) = active {
                    current.close();
                    self.records.get_mut(&current.round().round).expect("fixed record").capture(&current);
                }
                self.failure = Some(error.clone()); Err(error)
            }
        }
    }
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        if revision != self.revision { return Err(Error::Stale); }
        if self.status != NativeReviewStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = NativeReviewStatus::Failed; self.failure = Some(Error::Incomplete.into());
        let active = self.active.take();
        let _future = std::mem::take(&mut self.future);
        if let Some(mut current) = active {
            current.cancel(current.revision())?;
            self.records.get_mut(&current.round().round).expect("fixed record").capture(&current);
        }
        self.status = NativeReviewStatus::Cancelled; self.failure = None;
        Ok(())
    }
}
