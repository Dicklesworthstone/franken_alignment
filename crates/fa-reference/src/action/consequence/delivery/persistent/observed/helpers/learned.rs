//! Computed learned-sidecar congress through the original durable protocol.
//! The fixed roster owns every original helper port. Numerical observations do
//! not grant either publication key; each accepted protocol phase is journaled.
#[cfg(test)]
mod tests;

use super::{DurableSession, Event, FileOversight, JournalError, journal_error};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{CommitteeInput,
    helper_workers::{HelperPhase, HelperPort, HelperStatus, MAX_WORKER_SALT_BYTES,
        coordinator::Coordinator, wire},
    learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule, MAX_LEARNED_REVIEW_POLLS,
        probes::{ProbeEvaluationRecord, ProbeReviewLimits, ProbeReviewMember, MAX_PROBE_REVIEW_EVALUATIONS}},
    sidecar::{MAX_SIDECAR_REFINEMENT_ROUNDS,
        probe_helper::{ProbeHelperBudget, ProbeHelperStatus, SidecarProbeEvaluator,
            peer::MIN_PROBE_HELPER_SALT_BYTES}}};
use crate::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLearnedProbeStatus { Running, Finished, Cancelled, Failed }

struct ProbeSlot {
    port: HelperPort,
    evaluator: SidecarProbeEvaluator,
    salt: Vec<u8>,
}
impl ProbeSlot {
    fn capture(&self, record: &mut ProbeEvaluationRecord) {
        if record.status.is_some() {
            record.status = Some(self.evaluator.status());
            record.work = self.evaluator.work();
        }
    }
    fn cancel(&mut self, record: &mut ProbeEvaluationRecord) {
        let _ = self.evaluator.cancel(self.evaluator.revision());
        self.capture(record);
        self.salt.clear();
    }
    fn evaluate_once(&mut self, record: &mut ProbeEvaluationRecord,
        evaluations: &mut usize, maximum: usize) -> Result<(), Error>
    {
        let status = self.evaluator.status();
        if status == ProbeHelperStatus::AwaitingInput {
            if *evaluations >= maximum { return Err(Error::Limit); }
            *evaluations += 1;
        }
        // An unwind retains this attempted-operation marker and prior costs.
        record.status = Some(ProbeHelperStatus::Busy);
        let result = match status {
            ProbeHelperStatus::AwaitingInput => {
                wire::encode_request(&self.port).and_then(|bytes| wire::decode_request(&bytes))
                    .and_then(|input| self.evaluator.begin(&input))
            }
            ProbeHelperStatus::Evaluating => self.evaluator.advance(self.evaluator.revision()),
            _ => Err(Error::WrongState),
        };
        self.capture(record);
        result.map(|_| ())
    }
}

/// One privately sourced, fixed-coefficient review and refinement sequence.
/// Each poll performs at most one input admission, probe, commitment OR reveal
/// per member. Original reconstruction and journal transactions are synchronous;
/// this bound is not a wall-clock or process-isolation guarantee.
///
/// Completed work and acknowledged round outcomes remain inspectable after
/// cancellation/failure. They are process-local receipts, not a persisted global
/// computation escrow. Journal replay reconstructs the original congress and
/// disclosure plan, and recovery revokes every old session and publication key.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::FileLearnedProbeReview;
/// fn override_vote(run: &mut FileLearnedProbeReview) { run.submit_verdict(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::FileLearnedProbeReview;
/// fn replace(run: &mut FileLearnedProbeReview) { run.members_mut(); }
/// ```
pub struct FileLearnedProbeReview {
    issuer: Rc<()>,
    sidecar: FileLearnedSidecar,
    schedule: LearnedWorkerSchedule,
    members: BTreeMap<String, ProbeReviewMember>,
    limits: ProbeReviewLimits,
    index: usize,
    revision: u64,
    polls: usize,
    evaluations: usize,
    status: FileLearnedProbeStatus,
    failure: Option<JournalError>,
    input: CommitteeInput,
    active: Option<Coordinator>,
    slots: BTreeMap<String, ProbeSlot>,
    last_statuses: BTreeMap<String, HelperStatus>,
    records: BTreeMap<(u64, String), ProbeEvaluationRecord>,
    history: Vec<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileLearnedProbeReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedProbeReview").field("status", &self.status)
            .field("round", &self.current_round()).field("polls", &self.polls)
            .field("evaluations", &self.evaluations).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze the original source, all member coefficients, round salts and
    /// finite allowances before starting the first durable review. No request
    /// supplies a source or verdict. All scheduled round IDs are leased against
    /// manual protocol fallback in this owner, including after driver loss.
    pub fn begin_learned_probe_review(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        schedule: LearnedWorkerSchedule, members: BTreeMap<String, ProbeReviewMember>,
        limits: ProbeReviewLimits, snapshot: Snapshot) -> Result<FileLearnedProbeReview, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let original = self.checked_learned_sidecar(&sidecar)?;
        if original.round().work().rounds != 1 { return Err(Error::WrongState.into()); }
        if schedule.rounds.is_empty() || schedule.polls == 0 || members.is_empty() {
            return Err(Error::InvalidInput.into());
        }
        let required = members.len().checked_mul(schedule.rounds.len()).ok_or(Error::Limit)?;
        if schedule.rounds.len() > MAX_SIDECAR_REFINEMENT_ROUNDS
            || schedule.polls > MAX_LEARNED_REVIEW_POLLS
            || limits.evaluations > MAX_PROBE_REVIEW_EVALUATIONS || required > limits.evaluations {
            return Err(Error::Limit.into());
        }
        if members.keys().ne(original.round().input().views().keys()) { return Err(Error::Binding.into()); }
        let mut previous = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        let deadline = original.round().input().action().spec().deadline;
        let mut ids = BTreeSet::new();
        for round in &schedule.rounds {
            if round.round == 0 || round.evidence_root == [0; 32]
                || !(previous < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                    && round.window.reveal_by <= deadline) { return Err(Error::InvalidInput.into()); }
            if !ids.insert(round.round) || self.worker_rounds.contains(&round.round)
                || self.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round.round)) {
                return Err(Error::Duplicate.into());
            }
            previous = round.window.reveal_by;
        }
        for (member, definition) in &members {
            if definition.salts.len() != schedule.rounds.len() { return Err(Error::Incomplete.into()); }
            for salt in &definition.salts {
                if !(MIN_PROBE_HELPER_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len())
                    || salt.len() > schedule.helpers.salt_bytes { return Err(Error::Limit.into()); }
            }
            check_ceiling(limits.per_evaluation, SidecarProbeEvaluator::required_budget(
                original.source(), original.round(), member, &definition.probes)?)?;
        }
        // Allocate custody, inspection slots and all first-round native workers
        // before Begin. Only infallible moves publish the acknowledged lease.
        let mut leased = self.worker_rounds.clone();
        leased.extend(ids);
        let mut records = BTreeMap::new();
        for round in &schedule.rounds {
            for member in members.keys() {
                records.insert((round.round, member.clone()), ProbeEvaluationRecord::default());
            }
        }
        let mut history = Vec::new();
        history.try_reserve_exact(schedule.rounds.len()).map_err(|_| Error::Limit)?;
        let (active, slots, input) = prepare(self, &sidecar, &schedule, &members, limits, 0)?;
        let first = schedule.rounds[0];
        self.begin_review(revision, sidecar.attempt(), first.round, first.evidence_root, first.window, snapshot)?;
        self.worker_rounds = leased;
        Ok(FileLearnedProbeReview { issuer: Rc::clone(&self.issuer), sidecar, schedule, members,
            limits: ProbeReviewLimits { evaluations: required, ..limits }, index: 0, revision: 0,
            polls: 0, evaluations: 0, status: FileLearnedProbeStatus::Running, failure: None,
            input, active: Some(active), slots, last_statuses: BTreeMap::new(), records, history })
    }
}

impl FileLearnedProbeReview {
    pub fn status(&self) -> FileLearnedProbeStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn evaluations(&self) -> usize { self.evaluations }
    pub fn reservation(&self) -> ProbeReviewLimits { self.limits }
    pub fn current_round(&self) -> LearnedWorkerRound { self.schedule.rounds[self.index] }
    pub fn input_revision(&self) -> u64 { self.sidecar.input_revision() }
    /// Historical bytes; original currentness still gates both publication keys.
    pub fn input(&self) -> &CommitteeInput { &self.input }
    pub fn failure(&self) -> Option<&JournalError> { self.failure.as_ref() }
    pub fn records(&self) -> &BTreeMap<(u64, String), ProbeEvaluationRecord> { &self.records }
    /// Acknowledged original results, in the frozen schedule's order. An inner
    /// application error is retained here; it is not an uncommitted retry.
    pub fn history(&self) -> &[FileLearnedSidecarFinish] { &self.history }
    pub fn worker_statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.active.as_ref().map_or_else(|| self.last_statuses.clone(), Coordinator::statuses)
    }

    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.status != FileLearnedProbeStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.close();
        self.status = FileLearnedProbeStatus::Cancelled;
        Ok(())
    }
    fn close(&mut self) {
        let round = self.current_round().round;
        if let Some(mut active) = self.active.take() {
            active.close(); self.last_statuses = active.statuses();
        }
        release(&mut self.slots, &mut self.records, round);
        self.slots.clear();
    }

    /// Check the live durable source before any numerical quantum. Completed
    /// judgment yields before queuing a commitment; every original phase passes
    /// its journal barrier before reveal or refinement can proceed.
    pub fn advance(&mut self, host: &mut FileOversight, expected_revision: u64,
        now: ElapsedTick, snapshot: Snapshot) -> Result<FileLearnedProbeStatus, JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if expected_revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != FileLearnedProbeStatus::Running { return Err(Error::WrongState.into()); }
        let previous = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < previous || self.active.as_ref().is_some_and(|active| now < active.elapsed()) {
            return Err(Error::Stale.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = FileLearnedProbeStatus::Failed;
        self.failure = Some(Error::Incomplete.into());
        if self.polls == self.schedule.polls {
            self.close(); self.failure = Some(Error::Limit.into()); return Err(Error::Limit.into());
        }
        self.polls += 1;
        // Stack ownership closes every port on ordinary errors AND unwinds.
        // The terminal marker and in-place cost records survive interrupted work.
        let mut active = self.active.take().ok_or(Error::WrongState)?;
        let mut slots = std::mem::take(&mut self.slots);
        let round = self.current_round();
        let result = self.poll(host, now, &mut active, &mut slots);
        self.last_statuses = active.statuses();
        if let Err(error) = result {
            release(&mut slots, &mut self.records, round.round);
            self.failure = Some(error.clone()); return Err(error);
        }
        let ready = now >= round.window.reveal_by || self.last_statuses.values().all(|status| status.revealed);
        if !ready {
            self.active = Some(active); self.slots = slots;
            self.status = FileLearnedProbeStatus::Running; self.failure = None;
            return Ok(self.status);
        }
        let more = self.index + 1 < self.schedule.rounds.len();
        let result = host.finish_learned_sidecar_review_inner(host.revision(), &mut self.sidecar,
            round.round, more, snapshot.clone());
        release(&mut slots, &mut self.records, round.round);
        active.close();
        self.last_statuses = active.statuses();
        let finished = match result {
            Ok(value) => value,
            Err(error) => { self.failure = Some(error.clone()); return Err(error); }
        };
        let refined = matches!(&finished, FileLearnedSidecarFinish::Refined { .. });
        self.history.push(finished);
        if !refined {
            self.status = FileLearnedProbeStatus::Finished; self.failure = None;
            return Ok(self.status);
        }
        self.input = match host.current_learned_sidecar(&self.sidecar) {
            Ok(input) => input.clone(),
            Err(error) => { self.failure = Some(error.clone()); return Err(error); }
        };
        self.index += 1;
        // Re-resolve the current machine's source after committed refinement.
        // The original broker issuer changes on every durable transaction.
        let result = (|| {
            let (next_active, next_slots, input) = prepare(host, &self.sidecar,
                &self.schedule, &self.members, self.limits, self.index)?;
            let next = self.current_round();
            host.begin_review(host.revision(), self.sidecar.attempt(), next.round,
                next.evidence_root, next.window, snapshot)?;
            Ok::<_, JournalError>((next_active, next_slots, input))
        })();
        match result {
            Ok((next_active, next_slots, input)) => {
                self.active = Some(next_active); self.slots = next_slots; self.input = input;
                self.status = FileLearnedProbeStatus::Running; self.failure = None;
                Ok(self.status)
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }

    fn poll(&mut self, host: &mut FileOversight, now: ElapsedTick,
        active: &mut Coordinator, slots: &mut BTreeMap<String, ProbeSlot>) -> Result<(), JournalError>
    {
        host.checked_learned_sidecar(&self.sidecar)?;
        let round = self.current_round();
        let mut session = DurableSession { host, attempt: self.sidecar.attempt(),
            round: round.round, inputs: &self.input };
        active.advance(&mut session, now).map_err(journal_error)?;
        for (member, slot) in slots {
            session.host.checked_learned_sidecar(&self.sidecar)?;
            let record = self.records.get_mut(&(round.round, member.clone()))
                .expect("all frozen evaluations have inspection slots");
            if record.failure.is_some() { continue; }
            let result = match slot.port.phase() {
                HelperPhase::AwaitCommit if now < round.window.commit_by => {
                    match slot.evaluator.status() {
                        ProbeHelperStatus::Judged(verdict) => {
                            slot.port.request().commitment(verdict, &slot.salt)
                                .and_then(|digest| slot.port.submit_commitment(digest))
                                .map(|()| { record.commitment_queued = true; })
                        }
                        _ => slot.evaluate_once(record, &mut self.evaluations, self.limits.evaluations),
                    }
                }
                HelperPhase::ReadyReveal if now < round.window.reveal_by => {
                    let result = slot.evaluator.report().ok_or(Error::Incomplete)
                        .and_then(|report| slot.port.reveal(report.verdict(), &slot.salt));
                    if result.is_ok() { record.reveal_queued = true; }
                    result
                }
                HelperPhase::AwaitCommit | HelperPhase::Failed | HelperPhase::Closed => {
                    slot.cancel(record); Ok(())
                }
                _ => Ok(()),
            };
            if let Err(error) = result {
                record.failure = Some(error); slot.cancel(record); slot.port.disconnect();
            }
        }
        active.advance(&mut session, now).map_err(journal_error)
    }
}

fn prepare(host: &FileOversight, handle: &FileLearnedSidecar, schedule: &LearnedWorkerSchedule,
    members: &BTreeMap<String, ProbeReviewMember>, limits: ProbeReviewLimits, index: usize)
    -> Result<(Coordinator, BTreeMap<String, ProbeSlot>, CommitteeInput), JournalError>
{
    let sidecar = host.checked_learned_sidecar(handle)?;
    let round = schedule.rounds.get(index).ok_or(Error::Limit)?;
    let now = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
    let (active, ports) = Coordinator::new(round.round, round.evidence_root, sidecar.round().input(),
        round.window, now, schedule.helpers)?;
    if ports.keys().ne(members.keys()) { return Err(Error::Binding.into()); }
    let mut slots = BTreeMap::new();
    for (member, port) in ports {
        let definition = members.get(&member).ok_or(Error::Missing)?;
        let evaluator = SidecarProbeEvaluator::new(&port, sidecar.round(), sidecar.source().clone(),
            definition.probes.clone(), limits.per_evaluation)?;
        let salt = definition.salts.get(index).ok_or(Error::Missing)?.clone();
        slots.insert(member, ProbeSlot { port, evaluator, salt });
    }
    Ok((active, slots, sidecar.round().input().clone()))
}

fn release(slots: &mut BTreeMap<String, ProbeSlot>,
    records: &mut BTreeMap<(u64, String), ProbeEvaluationRecord>, round: u64)
{
    for (member, slot) in slots {
        let record = records.get_mut(&(round, member.clone()))
            .expect("all frozen evaluations have inspection slots");
        slot.cancel(record);
    }
}

fn check_ceiling(ceiling: ProbeHelperBudget, needed: ProbeHelperBudget) -> Result<(), Error> {
    let maximum = ProbeHelperBudget::default();
    let fields = |b: ProbeHelperBudget| [b.input_bytes, b.probes, b.probe_coordinates,
        b.refinement_bytes, b.materialized_values];
    if fields(ceiling).into_iter().zip(fields(maximum)).any(|(a, b)| a > b)
        || fields(needed).into_iter().zip(fields(ceiling)).any(|(a, b)| a > b)
        || ceiling.reconstruction_products > maximum.reconstruction_products
        || needed.reconstruction_products > ceiling.reconstruction_products { return Err(Error::Limit); }
    Ok(())
}
