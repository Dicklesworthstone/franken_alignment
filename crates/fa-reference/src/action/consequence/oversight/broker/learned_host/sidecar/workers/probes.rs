//! Local computed probes through the original fixed-round refinement driver.
//! This adapter owns every port; callers cannot inject or replace a ballot.
//! Local numerical separation is not process isolation or statistical independence.
use super::{LearnedWorkerReview, LearnedWorkerSchedule, LearnedWorkerStatus, LearnedWorkerUpdate};
use super::super::{LearnedSidecar, OversightBroker};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::probe::learned::KvRow;
use crate::action::consequence::oversight::{CommitteeInput, ObservedReview,
    helper_workers::{HelperPhase, HelperPort, HelperStatus, MAX_WORKER_SALT_BYTES, wire},
    replay::ObservedDecisionArchive,
    sidecar::probe_helper::{ProbeHelperBudget, ProbeHelperStatus, ProbeHelperWork, SidecarProbeEvaluator,
        peer::MIN_PROBE_HELPER_SALT_BYTES}};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_PROBE_REVIEW_EVALUATIONS: usize = 256;

/// All coefficients and one independently provisioned salt per scheduled round
/// freeze before the first evaluation. The request never defines new probes.
#[derive(Clone)]
pub struct ProbeReviewMember {
    pub probes: BTreeMap<KvRow, Vec<LinearProbe>>,
    pub salts: Vec<Vec<u8>>,
}

/// Reserve every scheduled member/round at this common per-evaluation ceiling.
/// Aggregate component ceilings are evaluations * per_evaluation.component.
/// No early-stop discount, retry, replacement member or budget refill exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeReviewLimits {
    pub evaluations: usize,
    pub per_evaluation: ProbeHelperBudget,
}

/// Supervisor inspection, not a new judgment or permission. None means this
/// evaluator has not run. Busy after an unwind means work is incomplete;
/// counters represent completed original operations, not all failed work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeEvaluationRecord {
    pub status: Option<ProbeHelperStatus>,
    pub work: ProbeHelperWork,
    pub commitment_queued: bool,
    pub reveal_queued: bool,
    pub failure: Option<Error>,
}

struct LocalProbe {
    port: HelperPort,
    evaluator: SidecarProbeEvaluator,
    salt: Vec<u8>,
}
impl LocalProbe {
    fn capture(&self, record: &mut ProbeEvaluationRecord) {
        // Keep unstarted slots distinguishable from attempted/cancelled work.
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
            // Count an admitted attempt exactly once, before any fallible input
            // work. Every later probe spends this same frozen reservation.
            *evaluations += 1;
        }
        record.status = Some(ProbeHelperStatus::Busy);
        let result = match status {
            ProbeHelperStatus::AwaitingInput => {
                // Retain the original full local wire binding; no second input
                // constructor can omit round, root, profile or salt-limit fields.
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

/// One source, fixed coefficients, original planner and original HelperRound.
/// Only the completed original review can leave; application and all effect
/// keys remain separate. Each poll checks the original source before computing.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::probes::LearnedProbeReview;
/// fn override_vote(run: &mut LearnedProbeReview) { run.submit_verdict(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::probes::LearnedProbeReview;
/// fn replace(run: &mut LearnedProbeReview) { run.members_mut(); }
/// ```
pub struct LearnedProbeReview {
    run: LearnedWorkerReview,
    members: BTreeMap<String, ProbeReviewMember>,
    slots: BTreeMap<String, LocalProbe>,
    records: BTreeMap<(u64, String), ProbeEvaluationRecord>,
    reservation: ProbeReviewLimits,
    evaluations: usize,
}
impl fmt::Debug for LearnedProbeReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedProbeReview").field("status", &self.status())
            .field("evaluations", &self.evaluations).finish_non_exhaustive()
    }
}
impl LearnedProbeReview {
    pub fn status(&self) -> LearnedWorkerStatus { self.run.status() }
    pub fn revision(&self) -> u64 { self.run.revision() }
    pub fn input(&self) -> &CommitteeInput { self.run.input() }
    pub fn input_revision(&self) -> u64 { self.run.input_revision() }
    pub fn reservation(&self) -> ProbeReviewLimits { self.reservation }
    pub fn evaluations(&self) -> usize { self.evaluations }
    pub fn records(&self) -> &BTreeMap<(u64, String), ProbeEvaluationRecord> { &self.records }
    pub fn history(&self) -> &[ObservedDecisionArchive] { self.run.history() }
    pub fn worker_statuses(&self) -> BTreeMap<String, HelperStatus> { self.run.worker_statuses() }
    pub fn take_review(&mut self) -> Result<ObservedReview, Error> { self.run.take_review() }
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        self.run.cancel(expected_revision)?;
        release_slots(&mut self.slots, &mut self.records, self.run.current_round().round);
        self.slots.clear();
        Ok(())
    }

    fn provision(&mut self, ports: BTreeMap<String, HelperPort>) -> Result<(), Error> {
        if ports.len() != self.members.len() || ports.keys().ne(self.members.keys()) {
            return Err(Error::Binding);
        }
        let mut slots = BTreeMap::new();
        let round = self.run.current_round().round;
        for (member, port) in ports {
            let definition = self.members.get(&member).ok_or(Error::Missing)?;
            let evaluator = SidecarProbeEvaluator::new(&port, self.run.sidecar.round(),
                self.run.sidecar.source().clone(), definition.probes.clone(), self.reservation.per_evaluation)?;
            let salt = definition.salts.get(self.run.index).ok_or(Error::Missing)?.clone();
            slots.insert(member, LocalProbe { port, evaluator, salt });
        }
        // All native construction checks precede making any slot runnable.
        for member in slots.keys() {
            self.records.insert((round, member.clone()), ProbeEvaluationRecord::default());
        }
        self.slots = slots;
        Ok(())
    }
}
impl OversightBroker {
    pub fn begin_learned_probe_review(&mut self, sidecar: LearnedSidecar,
        schedule: LearnedWorkerSchedule, members: BTreeMap<String, ProbeReviewMember>,
        limits: ProbeReviewLimits, snapshot: &Snapshot) -> Result<LearnedProbeReview, Error>
    {
        self.check_learned_sidecar(&sidecar)?;
        if members.is_empty() || schedule.rounds.is_empty() { return Err(Error::InvalidInput); }
        let required = members.len().checked_mul(schedule.rounds.len()).ok_or(Error::Limit)?;
        if limits.evaluations > MAX_PROBE_REVIEW_EVALUATIONS || required > limits.evaluations {
            return Err(Error::Limit);
        }
        if members.keys().ne(sidecar.round().input().views().keys()) { return Err(Error::Binding); }
        for (member, definition) in &members {
            if definition.salts.len() != schedule.rounds.len() { return Err(Error::Incomplete); }
            for salt in &definition.salts {
                if !(MIN_PROBE_HELPER_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len())
                    || salt.len() > schedule.helpers.salt_bytes { return Err(Error::Limit); }
            }
            let needed = SidecarProbeEvaluator::required_budget(sidecar.source(), sidecar.round(),
                member, &definition.probes)?;
            check_ceiling(limits.per_evaluation, needed)?;
        }
        // Original admission owns round identity, deadlines, source and helper caps.
        // Once that round begins, a native provisioning error consumes its identity
        // and closes its ports; it does not roll back into a retryable old round.
        let (run, ports) = self.begin_learned_worker_review(sidecar, schedule, snapshot)?;
        let mut review = LearnedProbeReview { run, members, slots: BTreeMap::new(),
            records: BTreeMap::new(), reservation: ProbeReviewLimits {
                evaluations: required, per_evaluation: limits.per_evaluation }, evaluations: 0 };
        review.provision(ports)?;
        Ok(review)
    }

    /// One original poll; at most one admission, original probe, commitment OR
    /// reveal per member. Complete judgment yields before queuing a commitment.
    /// Fresh refined slots are not evaluated in the same invocation.
    /// The caller's elapsed tick is a trusted observation, not measured wall time.
    pub fn advance_learned_probe_review(&mut self, review: &mut LearnedProbeReview,
        expected_revision: u64, now: ElapsedTick, snapshot: &Snapshot)
        -> Result<LearnedWorkerStatus, Error>
    {
        // Stale/foreign calls preserve ownership. The original inner check is
        // intentionally retained as well, before any source validation or work.
        self.check_learned_worker_call(&review.run, expected_revision, now)?;
        let mut slots = std::mem::take(&mut review.slots);
        let round = review.run.current_round();
        let records = &mut review.records;
        let evaluations = &mut review.evaluations;
        let maximum = review.reservation.evaluations;
        let update = self.advance_learned_workers_with_io(&mut review.run,
            expected_revision, now, snapshot, || {
                for (member, slot) in &mut slots {
                    let record = records.get_mut(&(round.round, member.clone()))
                        .expect("every provisioned slot has an inspection record");
                    if record.failure.is_some() { continue; }
                    let result = match slot.port.phase() {
                        HelperPhase::AwaitCommit if now < round.window.commit_by => {
                            match slot.evaluator.status() {
                                ProbeHelperStatus::Judged(verdict) => {
                                    slot.port.request().commitment(verdict, &slot.salt)
                                        .and_then(|digest| slot.port.submit_commitment(digest))
                                        .map(|()| { record.commitment_queued = true; })
                                }
                                _ => slot.evaluate_once(record, evaluations, maximum),
                            }
                        }
                        HelperPhase::ReadyReveal if now < round.window.reveal_by => {
                            let result = slot.evaluator.report().ok_or(Error::Incomplete)
                                .and_then(|report| slot.port.reveal(report.verdict(), &slot.salt));
                            if result.is_ok() { record.reveal_queued = true; }
                            result
                        }
                        HelperPhase::AwaitCommit => {
                            // Stop unfinished numerical ownership at commit expiry.
                            // The original coordinator records the deadline failure;
                            // do not relabel it as a worker-supplied disconnection.
                            slot.cancel(record);
                            Ok(())
                        }
                        _ => Ok(()),
                    };
                    if let Err(error) = result {
                        record.failure = Some(error);
                        slot.cancel(record);
                        slot.port.disconnect();
                    }
                }
            });
        // slots is stack-owned: original ports close on ordinary errors AND
        // unwinds. Busy records/attempt counts survive interrupted work.
        match update {
            Err(error) => {
                release_slots(&mut slots, &mut review.records, round.round);
                Err(error)
            }
            Ok(LearnedWorkerUpdate::Waiting) => {
                review.slots = slots;
                Ok(review.status())
            }
            Ok(LearnedWorkerUpdate::Stopped) => {
                release_slots(&mut slots, &mut review.records, round.round);
                Ok(review.status())
            }
            Ok(LearnedWorkerUpdate::NextRound(ports)) => {
                release_slots(&mut slots, &mut review.records, round.round);
                drop(slots);
                if let Err(error) = review.provision(ports) {
                    review.run.status = LearnedWorkerStatus::Failed(error);
                    review.run.close_active();
                    return Err(error);
                }
                Ok(review.status())
            }
        }
    }
}

fn release_slots(slots: &mut BTreeMap<String, LocalProbe>,
    records: &mut BTreeMap<(u64, String), ProbeEvaluationRecord>, round: u64)
{
    for (member, slot) in slots {
        let record = records.get_mut(&(round, member.clone()))
            .expect("every provisioned slot has an inspection record");
        slot.cancel(record);
    }
}

// Preflight only. SidecarProbeEvaluator::new repeats native admission for EACH
// actual round. No second scoring, refinement, or congress algorithm lives here.
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
