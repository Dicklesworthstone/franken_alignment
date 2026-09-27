//! Journal the original source-bound sidecar, never a caller-asserted source.
//! Replay reconstructs the original planner from the already replayed learned
//! generation. The request contains disclosure choices, not numerical evidence.
use super::{DecoderEvent, Event, FileOversight, JournalError, LearnedEvent, Reader, Writer};
use crate::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::oversight::learned_host::sidecar::LearnedSidecarRequest;
use crate::action::consequence::oversight::sidecar::{SidecarCommitteeRound, SidecarCongressBudget,
    SidecarIdentity, MAX_SIDECAR_ACCUMULATED_BYTES, MAX_SIDECAR_PRIORITY_GROUPS,
    MAX_SIDECAR_REFINEMENT_ROUNDS};
use crate::Error;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(in super::super::super) enum SidecarEvent {
    Prepare { attempt: u64, actor_revision: u64, request: LearnedSidecarRequest },
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

impl FileOversight {
    pub fn learned_sidecar_required(&self) -> bool {
        self.machine.learned_contract().is_some_and(super::FileLearnedConfig::requires_sidecar)
    }

    /// One original planner per attempt. Prepare in RAM, then commit the source-
    /// bound input and mandatory revision marker through the sole journal write.
    /// No new inference, caller source, manual provenance marker or permit enters.
    /// A storage failure returns no packet and poisons the original owner.
    pub fn begin_learned_sidecar(&mut self, revision: u64, attempt: u64,
        actor_revision: u64, request: LearnedSidecarRequest)
        -> Result<FileSidecarSnapshot, JournalError>
    {
        validate_request(attempt, &request)?;
        self.transact(revision, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Prepare { attempt, actor_revision, request }))))?;
        self.retained_learned_sidecar(attempt)
    }

    /// Original packet at the acknowledged cut. Does not assert source freshness,
    /// unpaused execution, active authority or continued eligibility after reset.
    pub fn retained_learned_sidecar(&self, attempt: u64) -> Result<FileSidecarSnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.retained_learned_sidecar(attempt, self.revision())?)
    }
}

pub(in super::super::super) fn validate_request(attempt: u64, request: &LearnedSidecarRequest)
    -> Result<(), Error>
{
    let b = request.budget;
    if attempt == 0 || request.identity.object_id == 0 || request.identity.transform_id == 0
        || b.rounds == 0 || b.residual_bytes == 0 || b.committee_bytes == 0 { return Err(Error::InvalidInput); }
    if request.priority.len() > MAX_SIDECAR_PRIORITY_GROUPS || b.rounds > MAX_SIDECAR_REFINEMENT_ROUNDS
        || b.residual_bytes > MAX_SIDECAR_ACCUMULATED_BYTES
        || b.committee_bytes > MAX_SIDECAR_ACCUMULATED_BYTES { return Err(Error::Limit); }
    let mut groups = BTreeSet::new();
    for group in &request.priority {
        if !groups.insert(*group) { return Err(Error::Duplicate); }
    }
    Ok(())
}

// Learned-event subtag 4, local sidecar subtag 0. The pre-existing 0..3 learned
// encodings and every outer journal tag are unchanged. Counts precede allocation.
pub(super) fn write(w: &mut Writer, event: &SidecarEvent) -> Result<(), Error> {
    match event {
        SidecarEvent::Prepare { attempt, actor_revision, request } => {
            validate_request(*attempt, request)?;
            w.u8(0)?; w.u64(*attempt)?; w.u64(*actor_revision)?;
            for value in [request.identity.object_id, request.identity.generation, request.identity.transform_id] {
                w.u64(value)?;
            }
            for value in [request.budget.rounds, request.budget.residual_bytes, request.budget.committee_bytes] {
                w.count(value)?;
            }
            w.count(request.priority.len())?;
            for group in &request.priority {
                w.u64(group.row.layer)?;
                w.u8(match group.row.side { KvSide::Key => 0, KvSide::Value => 1 })?;
                w.u64(group.row.position)?; w.count(group.head)?;
            }
        }
    }
    Ok(())
}
pub(super) fn read(r: &mut Reader<'_>) -> Result<SidecarEvent, Error> {
    if r.u8()? != 0 { return Err(Error::InvalidInput); }
    let attempt = r.u64()?; let actor_revision = r.u64()?;
    let identity = SidecarIdentity { object_id: r.u64()?, generation: r.u64()?, transform_id: r.u64()? };
    let budget = SidecarCongressBudget { rounds: r.count(MAX_SIDECAR_REFINEMENT_ROUNDS)?,
        residual_bytes: r.count(MAX_SIDECAR_ACCUMULATED_BYTES)?,
        committee_bytes: r.count(MAX_SIDECAR_ACCUMULATED_BYTES)? };
    let count = r.count(MAX_SIDECAR_PRIORITY_GROUPS)?;
    let mut priority = Vec::new(); priority.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let layer = r.u64()?;
        let side = match r.u8()? { 0 => KvSide::Key, 1 => KvSide::Value, _ => return Err(Error::InvalidInput) };
        let position = r.u64()?; let head = r.count(usize::MAX)?;
        priority.push(KvGroup { row: KvRow { layer, side, position }, head });
    }
    let request = LearnedSidecarRequest { identity, priority, budget };
    validate_request(attempt, &request)?;
    Ok(SidecarEvent::Prepare { attempt, actor_revision, request })
}
