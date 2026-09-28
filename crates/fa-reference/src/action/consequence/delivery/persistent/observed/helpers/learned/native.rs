//! Original native-model judgments inside the durable source-bound congress.
//! Numerical probes constrain each model; the original coordinator and journal
//! alone accept its commitment/reveal. A model answer is never an effect key.

pub mod sequence;

use super::super::{DurableSession, Event, FileOversight, JournalError, journal_error};
use super::super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::learned::MAX_LEARNED_MONITOR_COORDINATES;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::MAX_SAMPLING_ENTRIES;
use crate::action::consequence::activation::probe::learned::{LearnedProbeWork, MAX_CHECKED_KV_PRODUCTS};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::oversight::{CommitteeInput,
    helper_client::native::{NativeEvaluator, NativeEvaluationStatus, peer::MIN_NATIVE_SALT_BYTES},
    helper_workers::{HelperLimits, HelperPhase, HelperPort, HelperStatus, MAX_WORKER_SALT_BYTES,
        coordinator::Coordinator, wire},
    learned_host::sidecar::workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS},
    sidecar::receiver::{SidecarReceiver, SidecarReceiveBudget,
        native::{SidecarNativeEvaluator, SidecarProbeQuery, SidecarEvaluationError,
            SidecarEvaluationProgress, SidecarEvaluationStatus, MAX_SIDECAR_HELPER_PROBES}}};
use crate::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_NATIVE_REVIEW_EVALUATIONS: usize = 256;

// Private admission to a fresh ID or a sequence-owned, previously frozen lease.
enum NativeRoundAdmission { Fresh(u64), Leased(u64) }

/// Independently constructed original model, frozen queries and secret salt.
/// Consumed before the original round starts; no replacement or reset accessor.
/// Provisioning and model independence are operator obligations, not proven here.
pub struct NativeReviewMember {
    pub evaluator: NativeEvaluator,
    pub queries: Vec<SidecarProbeQuery>,
    pub salt: Vec<u8>,
}

/// Conservative reservation from the original native policies, not elapsed time
/// or actual FLOPs. Unused/failed evaluations never refund this reservation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeReviewCost {
    pub evaluations: usize,
    pub scalar_products: u64,
    pub sampling_entries: u64,
}
impl NativeReviewCost {
    pub fn required(members: &BTreeMap<String, NativeReviewMember>) -> Result<Self, Error> {
        if members.is_empty() { return Err(Error::InvalidInput); }
        if members.len() > MAX_NATIVE_REVIEW_EVALUATIONS { return Err(Error::Limit); }
        members.values().try_fold(Self::default(), |sum, member| {
            let policy = &member.evaluator.policy().generation;
            sum.add(Self { evaluations: 1, scalar_products: policy.scalar_products,
                sampling_entries: policy.sampling_entries })
        })
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self { evaluations: self.evaluations.checked_add(other.evaluations).ok_or(Error::Overflow)?,
            scalar_products: self.scalar_products.checked_add(other.scalar_products).ok_or(Error::Overflow)?,
            sampling_entries: self.sampling_entries.checked_add(other.sampling_entries).ok_or(Error::Overflow)? })
    }
    fn admits(self, cost: Self) -> Result<(), Error> {
        if self.evaluations > MAX_NATIVE_REVIEW_EVALUATIONS
            || self.scalar_products > MAX_DECODER_PRODUCTS * MAX_NATIVE_REVIEW_EVALUATIONS as u64
            || self.sampling_entries > MAX_SAMPLING_ENTRIES * MAX_NATIVE_REVIEW_EVALUATIONS as u64
            || cost.evaluations > self.evaluations || cost.scalar_products > self.scalar_products
            || cost.sampling_entries > self.sampling_entries { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeReviewLimits {
    pub helpers: HelperLimits,
    pub receive: SidecarReceiveBudget,
    pub probes: LearnedProbeWork,
    pub polls: usize,
    pub native: NativeReviewCost,
}
impl Default for NativeReviewLimits {
    fn default() -> Self {
        Self { helpers: HelperLimits::default(), receive: SidecarReceiveBudget::default(),
            probes: LearnedProbeWork { coordinates: MAX_LEARNED_MONITOR_COORDINATES,
                reconstruction_products: MAX_CHECKED_KV_PRODUCTS }, polls: MAX_LEARNED_REVIEW_POLLS,
            native: NativeReviewCost { evaluations: MAX_NATIVE_REVIEW_EVALUATIONS,
                scalar_products: MAX_DECODER_PRODUCTS * MAX_NATIVE_REVIEW_EVALUATIONS as u64,
                sampling_entries: MAX_SAMPLING_ENTRIES * MAX_NATIVE_REVIEW_EVALUATIONS as u64 } }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReviewStatus { Running, Finished, Cancelled, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeMemberFailure { Evaluation(SidecarEvaluationError), Protocol(Error) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMemberRecord {
    pub progress: SidecarEvaluationProgress,
    /// A call began but did not return. Completed counters can understate work.
    pub interrupted: bool,
    pub failure: Option<NativeMemberFailure>,
    pub commitment_queued: bool,
    pub reveal_queued: bool,
}
struct NativeSlot { port: HelperPort, evaluator: SidecarNativeEvaluator, salt: Vec<u8> }
impl NativeSlot {
    fn cancel(&mut self, record: &mut NativeMemberRecord) {
        self.evaluator.cancel();
        record.progress = self.evaluator.progress();
        self.salt.clear();
    }
    fn quantum(&mut self, record: &mut NativeMemberRecord) -> Result<(), NativeMemberFailure> {
        record.interrupted = true;
        let previous = self.evaluator.progress();
        let result = if previous.status == SidecarEvaluationStatus::AwaitingInput {
            wire::encode_request(&self.port).and_then(|bytes| wire::decode_request(&bytes))
                .map_err(SidecarEvaluationError::from).and_then(|input| self.evaluator.begin(&input))
        } else { self.evaluator.advance(previous.revision) };
        record.progress = self.evaluator.progress();
        record.interrupted = false;
        result.map(|_| ()).map_err(NativeMemberFailure::Evaluation)
    }
}

/// One complete native-model committee, with the original durable phase machine.
/// Every poll yields after at most one admission/probe/model token/queued message
/// per member. Original admission, a single token and journal I/O are synchronous.
/// No OS isolation, wall-clock bound or statistical independence follows.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::native::FileNativeSidecarReview;
/// fn fabricate(run: &mut FileNativeSidecarReview) { run.submit_verdict(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::native::FileNativeSidecarReview;
/// fn retune(run: &mut FileNativeSidecarReview) { run.evaluator_mut(); }
/// ```
pub struct FileNativeSidecarReview {
    issuer: Rc<()>,
    sidecar: FileLearnedSidecar,
    round: LearnedWorkerRound,
    limits: NativeReviewLimits,
    reservation: NativeReviewCost,
    input: CommitteeInput,
    revision: u64,
    polls: usize,
    status: NativeReviewStatus,
    failure: Option<JournalError>,
    active: Option<Coordinator>,
    slots: BTreeMap<String, NativeSlot>,
    records: BTreeMap<String, NativeMemberRecord>,
    statuses: BTreeMap<String, HelperStatus>,
    outcome: Option<FileLearnedSidecarFinish>,
}
impl fmt::Debug for FileNativeSidecarReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileNativeSidecarReview").field("status", &self.status)
            .field("round", &self.round.round).field("polls", &self.polls).finish_non_exhaustive()
    }
}
impl FileOversight {
    /// Freeze a full independently supplied native roster before durable Begin.
    /// Complete quiet NUMERICAL probes admit each native model's WHOLE original
    /// input; only that model's final reviewed result can supply its native vote.
    /// No native failure defaults to Allow or a fabricated abstention.
    pub fn begin_native_sidecar_review(&mut self, revision: u64, sidecar: FileLearnedSidecar,
        round: LearnedWorkerRound, members: BTreeMap<String, NativeReviewMember>,
        limits: NativeReviewLimits, snapshot: Snapshot) -> Result<FileNativeSidecarReview, JournalError>
    {
        self.begin_native_sidecar_review_bound(NativeRoundAdmission::Fresh(revision), sidecar, round, members, limits, snapshot)
    }

    // Only the fixed sequence may consume one of its already admitted leases.
    // Public single-round callers cannot adopt another driver's reserved ID.
    fn begin_native_sidecar_review_bound(&mut self, admission: NativeRoundAdmission, sidecar: FileLearnedSidecar,
        round: LearnedWorkerRound, members: BTreeMap<String, NativeReviewMember>,
        limits: NativeReviewLimits, snapshot: Snapshot)
        -> Result<FileNativeSidecarReview, JournalError>
    {
        let (revision, preleased) = match admission {
            NativeRoundAdmission::Fresh(revision) => (revision, false),
            NativeRoundAdmission::Leased(revision) => (revision, true),
        };
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if limits.polls == 0 { return Err(Error::InvalidInput.into()); }
        if limits.polls > MAX_LEARNED_REVIEW_POLLS { return Err(Error::Limit.into()); }
        let reservation = NativeReviewCost::required(&members)?;
        limits.native.admits(reservation)?;
        let original = self.checked_learned_sidecar(&sidecar)?;
        let now = self.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if round.round == 0 || round.evidence_root == [0; 32]
            || !(now < round.window.commit_by && round.window.commit_by < round.window.reveal_by
                && round.window.reveal_by <= original.round().input().action().spec().deadline) {
            return Err(Error::InvalidInput.into());
        }
        if self.worker_rounds.contains(&round.round) != preleased
            || self.events.iter().any(|event| matches!(event, Event::Begin(_, id, ..) if *id == round.round)) {
            return Err(Error::Duplicate.into());
        }
        if members.keys().ne(original.round().input().views().keys()) { return Err(Error::Binding.into()); }
        let (active, ports) = Coordinator::new(round.round, round.evidence_root, original.round().input(),
            round.window, now, limits.helpers)?;
        let mut members = members;
        let mut slots = BTreeMap::new(); let mut records = BTreeMap::new();
        for (member, port) in ports {
            let definition = members.remove(&member).ok_or(Error::Missing)?;
            check_member(&definition, original, &member, limits)?;
            let receiver = SidecarReceiver::new(&port, original.round(), original.source().clone(), limits.receive)?;
            let evaluator = SidecarNativeEvaluator::new(definition.evaluator, receiver, definition.queries, limits.probes)?;
            records.insert(member.clone(), NativeMemberRecord { progress: evaluator.progress(), interrupted: false,
                failure: None, commitment_queued: false, reveal_queued: false });
            slots.insert(member, NativeSlot { port, evaluator, salt: definition.salt });
        }
        let input = original.round().input().clone();
        let mut leased = self.worker_rounds.clone(); leased.insert(round.round);
        self.begin_review(revision, sidecar.attempt(), round.round, round.evidence_root, round.window, snapshot)?;
        self.worker_rounds = leased;
        Ok(FileNativeSidecarReview { issuer: Rc::clone(&self.issuer), sidecar, round, limits, reservation,
            input, revision: 0, polls: 0, status: NativeReviewStatus::Running, failure: None,
            active: Some(active), slots, records, statuses: BTreeMap::new(), outcome: None })
    }
}

fn check_member(member: &NativeReviewMember,
    original: &crate::action::consequence::oversight::learned_host::sidecar::LearnedSidecar,
    name: &str, limits: NativeReviewLimits) -> Result<(), Error>
{
    if member.evaluator.status() != NativeEvaluationStatus::AwaitingInput
        || member.evaluator.position() != 0 || member.evaluator.sampled_draws() != 0 { return Err(Error::WrongState); }
    let view = original.round().input().views().get(name).ok_or(Error::Binding)?;
    if &member.evaluator.policy().input_profile != view.actual_input().input_profile() { return Err(Error::Binding); }
    if !(MIN_NATIVE_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&member.salt.len())
        || member.salt.len() > limits.helpers.salt_bytes { return Err(Error::Limit); }
    if member.queries.is_empty() { return Err(Error::Incomplete); }
    if member.queries.len() > MAX_SIDECAR_HELPER_PROBES { return Err(Error::Limit); }
    let expected: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
    let mut rows = BTreeSet::new(); let mut unique = BTreeSet::new(); let mut coordinates = 0_usize;
    for query in &member.queries {
        let id = query.probe.identity();
        if !unique.insert((query.row, id.id, id.generation)) { return Err(Error::Duplicate); }
        let (frame, heads, channels) = original.source().row_shape(query.row)?;
        if id.profile != frame.profile || id.dimensions != heads.checked_mul(channels).ok_or(Error::Overflow)? {
            return Err(Error::Binding);
        }
        coordinates = coordinates.checked_add(id.dimensions).ok_or(Error::Overflow)?;
        rows.insert(query.row);
    }
    if rows != expected { return Err(Error::Incomplete); }
    let products = (coordinates as u64).checked_mul(original.source().image().codec().policy().rank() as u64)
        .ok_or(Error::Overflow)?;
    if limits.probes.coordinates > MAX_LEARNED_MONITOR_COORDINATES
        || limits.probes.reconstruction_products > MAX_CHECKED_KV_PRODUCTS
        || coordinates > limits.probes.coordinates || products > limits.probes.reconstruction_products {
        return Err(Error::Limit);
    }
    Ok(())
}

impl FileNativeSidecarReview {
    pub fn status(&self) -> NativeReviewStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn polls(&self) -> usize { self.polls }
    pub fn round(&self) -> LearnedWorkerRound { self.round }
    pub fn reservation(&self) -> NativeReviewCost { self.reservation }
    pub fn input(&self) -> &CommitteeInput { &self.input }
    pub fn records(&self) -> &BTreeMap<String, NativeMemberRecord> { &self.records }
    pub fn failure(&self) -> Option<&JournalError> { self.failure.as_ref() }
    pub fn outcome(&self) -> Option<&FileLearnedSidecarFinish> { self.outcome.as_ref() }
    pub fn worker_statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.active.as_ref().map_or_else(|| self.statuses.clone(), Coordinator::statuses)
    }
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.status != NativeReviewStatus::Running { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.close(); self.status = NativeReviewStatus::Cancelled;
        Ok(())
    }
    fn close(&mut self) {
        if let Some(mut active) = self.active.take() { active.close(); self.statuses = active.statuses(); }
        release(&mut self.slots, &mut self.records);
        self.slots.clear();
    }
    pub fn advance(&mut self, host: &mut FileOversight, expected_revision: u64,
        now: ElapsedTick, snapshot: Snapshot) -> Result<NativeReviewStatus, JournalError>
    {
        self.advance_bound(host, expected_revision, now, snapshot, false)
    }

    // A single round never buys disclosure. The preprovisioned sequence alone
    // may request original refinement when its fixed successor is still usable.
    fn advance_bound(&mut self, host: &mut FileOversight, expected_revision: u64,
        now: ElapsedTick, snapshot: Snapshot, allow_refinement: bool)
        -> Result<NativeReviewStatus, JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if expected_revision != self.revision { return Err(Error::Stale.into()); }
        if self.status != NativeReviewStatus::Running { return Err(Error::WrongState.into()); }
        let previous = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < previous || self.active.as_ref().is_some_and(|active| now < active.elapsed()) {
            return Err(Error::Stale.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = NativeReviewStatus::Failed; self.failure = Some(Error::Incomplete.into());
        if self.polls == self.limits.polls {
            self.close(); self.failure = Some(Error::Limit.into()); return Err(Error::Limit.into());
        }
        self.polls += 1;
        // Active ports and native owners live on the stack across untrusted work.
        // Unwind leaves the driver poisoned and cannot restore an older quantum.
        let mut active = self.active.take().ok_or(Error::WrongState)?;
        let mut slots = std::mem::take(&mut self.slots);
        let result = self.poll(host, now, &mut active, &mut slots);
        if let Err(error) = result {
            release(&mut slots, &mut self.records); active.close(); self.statuses = active.statuses();
            self.failure = Some(error.clone()); return Err(error);
        }
        self.statuses = active.statuses();
        if now < self.round.window.reveal_by && !self.statuses.values().all(|status| status.revealed) {
            self.active = Some(active); self.slots = slots;
            self.status = NativeReviewStatus::Running; self.failure = None; return Ok(self.status);
        }
        let result = host.finish_learned_sidecar_review_inner(host.revision(), &mut self.sidecar,
            self.round.round, allow_refinement, snapshot);
        release(&mut slots, &mut self.records); active.close(); self.statuses = active.statuses();
        match result {
            Ok(outcome) => {
                self.outcome = Some(outcome); self.status = NativeReviewStatus::Finished; self.failure = None;
                Ok(self.status)
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }
    fn poll(&mut self, host: &mut FileOversight, now: ElapsedTick,
        active: &mut Coordinator, slots: &mut BTreeMap<String, NativeSlot>) -> Result<(), JournalError>
    {
        host.checked_learned_sidecar(&self.sidecar)?;
        let mut session = DurableSession { host, attempt: self.sidecar.attempt(), round: self.round.round, inputs: &self.input };
        active.advance(&mut session, now).map_err(journal_error)?;
        for (member, slot) in slots {
            session.host.checked_learned_sidecar(&self.sidecar)?;
            let record = self.records.get_mut(member).expect("frozen member record");
            if record.failure.is_some() { continue; }
            let result = match slot.port.phase() {
                HelperPhase::AwaitCommit if now < self.round.window.commit_by => {
                    if let SidecarEvaluationStatus::Judged(verdict) = slot.evaluator.progress().status {
                        slot.port.request().commitment(verdict, &slot.salt)
                            .and_then(|digest| slot.port.submit_commitment(digest))
                            .map(|()| { record.commitment_queued = true; }).map_err(NativeMemberFailure::Protocol)
                    } else { slot.quantum(record) }
                }
                HelperPhase::ReadyReveal if now < self.round.window.reveal_by => {
                    let result = match slot.evaluator.progress().status {
                        SidecarEvaluationStatus::Judged(verdict) => slot.port.reveal(verdict, &slot.salt),
                        _ => Err(Error::Incomplete),
                    };
                    result.map(|()| { record.reveal_queued = true; }).map_err(NativeMemberFailure::Protocol)
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
fn release(slots: &mut BTreeMap<String, NativeSlot>, records: &mut BTreeMap<String, NativeMemberRecord>) {
    for (member, slot) in slots { slot.cancel(records.get_mut(member).expect("frozen member record")); }
}
