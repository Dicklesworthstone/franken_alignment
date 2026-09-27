//! Durable source-bound helper inputs from the ORIGINAL learned generator.
//! The journal records disclosure choices and comparison bytes, never a source
//! to trust, a replacement planner, or a vote. Original replay owns all state.
mod finish;
mod outcomes;
pub use outcomes::FileLearnedSidecarOutcome;
pub use finish::FileLearnedSidecarFinish;

use super::{DecoderEvent, Event, FileOversight, JournalError, LearnedEvent, Machine,
    Reader, Transition, Writer, journal, MAX_WITNESS_BYTES};
use crate::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::oversight::{CommitteeInput,
    learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest},
    sidecar::{SidecarCommitteeRound, SidecarCongressBudget, SidecarIdentity, MAX_SIDECAR_ACCUMULATED_BYTES,
        MAX_SIDECAR_PRIORITY_GROUPS, MAX_SIDECAR_REFINEMENT_ROUNDS}};
use crate::{Error, Snapshot};
use std::rc::Rc;

#[derive(Clone)]
pub(in super::super::super) enum SidecarEvent {
    // Existing request-only journal record retains its original tag and bytes.
    Prepare { attempt: u64, actor_revision: u64, request: LearnedSidecarRequest },
    Begin { attempt: u64, actor_revision: u64, request: LearnedSidecarRequest,
        expected_payload: Rc<[u8]> },
    Finish { attempt: u64, actor_revision: u64, input_revision: u64,
        round: u64, allow_refinement: bool, snapshot: Snapshot, expected: Rc<[u8]> },
}

/// A retained, acknowledged packet and its original source/input revisions.
/// A snapshot survives recovery as HISTORY, not as current observation or a key.
/// Latest-position K/V coverage is not silently upgraded to full-prefix coverage.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::FileSidecarSnapshot;
/// fn authorize(packet: FileSidecarSnapshot) -> Permit { packet }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSidecarSnapshot {
    pub journal_revision: u64,
    pub attempt: u64,
    pub actor_revision: u64,
    pub input_revision: u64,
    pub packet: SidecarCommitteeRound,
}

/// A reference to one acknowledged original disclosure plan. Machine replay may
/// replace the in-memory broker on each transaction; this handle binds the same
/// durable owner and its original attempt, actor and input revisions instead.
/// It contains no planner, numerical source, permit or automatically current key.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::FileLearnedSidecar;
/// fn copy_plan(plan: FileLearnedSidecar) { let _ = plan.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::FileLearnedSidecar;
/// fn replace_source(plan: &mut FileLearnedSidecar) { plan.source_mut(); }
/// ```
#[derive(Debug)]
pub struct FileLearnedSidecar {
    issuer: Rc<()>,
    attempt: u64,
    actor_revision: u64,
    input_revision: u64,
}
impl FileLearnedSidecar {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn input_revision(&self) -> u64 { self.input_revision }
}

impl FileOversight {
    pub fn learned_sidecar_required(&self) -> bool {
        self.machine.learned_contract().is_some_and(super::FileLearnedConfig::requires_sidecar)
    }

    /// The original snapshot-returning constructor retains its request-only
    /// record format. The packet is historical data, not a current plan handle.
    pub fn begin_learned_sidecar(&mut self, revision: u64, attempt: u64,
        actor_revision: u64, request: LearnedSidecarRequest)
        -> Result<FileSidecarSnapshot, JournalError>
    {
        validate_request(attempt, &request)?;
        self.transact(revision, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Prepare { attempt, actor_revision, request }))))?;
        self.retained_learned_sidecar(attempt)
    }

    /// Original packet at the acknowledged cut, including paused or stale state.
    /// No current observation, helper session or publication key is returned.
    pub fn retained_learned_sidecar(&self, attempt: u64) -> Result<FileSidecarSnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.retained_learned_sidecar(attempt, self.revision())?)
    }

    /// Prepare and persist the proposal's FIRST original coarse input. Only the
    /// independently configured generator supplies its checked source. Returned
    /// evidence follows acknowledgment of the same canonical journal replacement.
    pub fn begin_learned_sidecar_plan(&mut self, revision: u64, attempt: u64,
        actor_revision: u64, request: LearnedSidecarRequest)
        -> Result<FileLearnedSidecar, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        validate_request(attempt, &request)?;
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Begin { attempt, actor_revision, request: request.clone(),
                expected_payload: Rc::from(&b""[..]) })));
        self.check_source_admission(&shape)?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let (event, input_revision) = candidate.prepare_learned_sidecar(attempt, actor_revision, request)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(event)));
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.persist_candidate(event, bytes, candidate, Transition::Inputs(input_revision))?;
        Ok(FileLearnedSidecar { issuer: Rc::clone(&self.issuer), attempt,
            actor_revision, input_revision })
    }

    /// Read the exact acknowledged helper input only while its original source,
    /// plan, proposal and input revision remain eligible. A retained handle is
    /// historical after recovery, source advancement or input replacement.
    pub fn current_learned_sidecar(&self, sidecar: &FileLearnedSidecar)
        -> Result<&CommitteeInput, JournalError>
    {
        Ok(self.checked_learned_sidecar(sidecar)?.round().input())
    }

    // Only original observed-owner adapters can provision source-bound workers.
    // This never exports a mutable plan, broker, evaluator or source selector.
    pub(in super::super::super) fn checked_learned_sidecar(&self, sidecar: &FileLearnedSidecar)
        -> Result<&LearnedSidecar, JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &sidecar.issuer) { return Err(Error::Binding.into()); }
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.source_interrupted || !self.clock_ready() || self.machine.learned_paused()
            || self.machine.pending_learned_step().is_some() { return Err(Error::Incomplete.into()); }
        let original = self.machine.checked_learned_sidecar(sidecar.attempt)?;
        if original.actor_revision() != sidecar.actor_revision
            || original.input_revision() != sidecar.input_revision { return Err(Error::Stale.into()); }
        Ok(original)
    }
}

pub(super) fn write(w: &mut Writer, event: &SidecarEvent) -> Result<(), Error> {
    match event {
        SidecarEvent::Finish { attempt, actor_revision, input_revision, round,
            allow_refinement, snapshot, expected } => {
            if expected.is_empty() { return Err(Error::Incomplete); }
            if expected.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            w.u8(2)?; w.u64(*attempt)?; w.u64(*actor_revision)?; w.u64(*input_revision)?;
            w.u64(*round)?; w.u8(u8::from(*allow_refinement))?; w.snapshot(snapshot)?; w.blob(expected)?;
        }
        SidecarEvent::Prepare { attempt, actor_revision, request } => {
            validate_request(*attempt, request)?;
            w.u8(0)?; w.u64(*attempt)?; w.u64(*actor_revision)?;
            write_request(w, request)?;
        }
        SidecarEvent::Begin { attempt, actor_revision, request, expected_payload } => {
            if expected_payload.is_empty() { return Err(Error::Incomplete); }
            if expected_payload.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            validate_request(*attempt, request)?;
            w.u8(1)?; w.u64(*attempt)?; w.u64(*actor_revision)?;
            write_request(w, request)?; w.blob(expected_payload)?;
        }
    }
    Ok(())
}
pub(super) fn read(r: &mut Reader<'_>) -> Result<SidecarEvent, Error> {
    match r.u8()? {
        2 => {
            let attempt = r.u64()?; let actor_revision = r.u64()?; let input_revision = r.u64()?;
            let round = r.u64()?;
            let allow_refinement = match r.u8()? { 0 => false, 1 => true, _ => return Err(Error::InvalidInput) };
            let snapshot = r.snapshot()?; let expected = r.blob(MAX_WITNESS_BYTES)?;
            if expected.is_empty() { return Err(Error::Incomplete); }
            Ok(SidecarEvent::Finish { attempt, actor_revision, input_revision, round,
                allow_refinement, snapshot, expected: Rc::from(expected) })
        }
        0 => {
            let attempt = r.u64()?; let actor_revision = r.u64()?;
            let request = read_request(r)?;
            validate_request(attempt, &request)?;
            Ok(SidecarEvent::Prepare { attempt, actor_revision, request })
        }
        1 => {
            let attempt = r.u64()?; let actor_revision = r.u64()?;
            let request = read_request(r)?;
            validate_request(attempt, &request)?;
            let expected_payload = r.blob(MAX_WITNESS_BYTES)?;
            if expected_payload.is_empty() { return Err(Error::Incomplete); }
            Ok(SidecarEvent::Begin { attempt, actor_revision, request,
                expected_payload: Rc::from(expected_payload) })
        }
        _ => Err(Error::InvalidInput),
    }
}
pub(in super::super::super) fn check_request(request: &LearnedSidecarRequest) -> Result<(), Error> {
    if request.identity.object_id == 0 || request.identity.transform_id == 0
        || request.budget.rounds == 0 || request.budget.residual_bytes == 0
        || request.budget.committee_bytes == 0 { return Err(Error::InvalidInput); }
    if request.priority.len() > MAX_SIDECAR_PRIORITY_GROUPS
        || request.budget.rounds > MAX_SIDECAR_REFINEMENT_ROUNDS
        || request.budget.residual_bytes > MAX_SIDECAR_ACCUMULATED_BYTES
        || request.budget.committee_bytes > MAX_SIDECAR_ACCUMULATED_BYTES { return Err(Error::Limit); }
    Ok(())
}
pub(in super::super::super) fn validate_request(attempt: u64, request: &LearnedSidecarRequest)
    -> Result<(), Error>
{
    if attempt == 0 { return Err(Error::InvalidInput); }
    check_request(request)?;
    let mut groups = std::collections::BTreeSet::new();
    for group in &request.priority {
        if !groups.insert(*group) { return Err(Error::Duplicate); }
    }
    Ok(())
}
fn write_request(w: &mut Writer, request: &LearnedSidecarRequest) -> Result<(), Error> {
    check_request(request)?;
    w.u64(request.identity.object_id)?; w.u64(request.identity.generation)?;
    w.u64(request.identity.transform_id)?;
    w.count(request.budget.rounds)?; w.count(request.budget.residual_bytes)?;
    w.count(request.budget.committee_bytes)?; w.count(request.priority.len())?;
    for group in &request.priority { write_group(w, *group)?; }
    Ok(())
}
fn read_request(r: &mut Reader<'_>) -> Result<LearnedSidecarRequest, Error> {
    let identity = SidecarIdentity { object_id: r.u64()?, generation: r.u64()?, transform_id: r.u64()? };
    let budget = SidecarCongressBudget { rounds: r.count(MAX_SIDECAR_REFINEMENT_ROUNDS)?,
        residual_bytes: r.count(MAX_SIDECAR_ACCUMULATED_BYTES)?,
        committee_bytes: r.count(MAX_SIDECAR_ACCUMULATED_BYTES)? };
    let count = r.count(MAX_SIDECAR_PRIORITY_GROUPS)?;
    let mut priority = Vec::new(); priority.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let layer = r.u64()?;
        let side = match r.u8()? { 0 => KvSide::Key, 1 => KvSide::Value, _ => return Err(Error::InvalidInput) };
        priority.push(KvGroup { row: KvRow { layer, side, position: r.u64()? }, head: r.count(usize::MAX)? });
    }
    let request = LearnedSidecarRequest { identity, priority, budget };
    check_request(&request)?;
    Ok(request)
}

pub(in super::super::super) fn write_group(w: &mut Writer, group: KvGroup) -> Result<(), Error> {
    w.u64(group.row.layer)?; w.u8(match group.row.side { KvSide::Key => 0, KvSide::Value => 1 })?;
    w.u64(group.row.position)?; w.count(group.head)
}
