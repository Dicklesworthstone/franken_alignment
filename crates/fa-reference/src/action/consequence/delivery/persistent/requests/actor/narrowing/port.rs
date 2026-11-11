use super::*;
use super::super::{FileActorPort, FileActorSupervisor, JournalError, UnknownReason};
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::action::consequence::delivery::persistent::observed::narrowing::FileAuthorityNarrowRequest;
use crate::Error;

impl FileActorSupervisor<FileOversight> {
    /// Provision finite independent ordinary target restriction. This samples
    /// no clock/source, captures no snapshot, replays nothing and grants no key.
    pub fn narrow_port(&self, queries: usize, operations: usize, max_replay_events: usize)
        -> Result<(FileSupervisorNarrowPort, FileSupervisorNarrowGrant), JournalError>
    {
        if queries == 0 || operations == 0 || max_replay_events == 0 { return Err(Error::InvalidInput.into()); }
        if queries > MAX_NARROW_QUERIES || operations > MAX_NARROW_OPERATIONS
            || max_replay_events > MAX_NARROW_REPLAY_EVENTS { return Err(Error::Limit.into()); }
        let owner = self.owner.try_borrow().map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let scope = owner.host.actor_explanation_scope();
        let grant = Rc::new(Grant { queries: Cell::new(queries), operations: Cell::new(operations),
            max_replay_events, records: RefCell::new(BTreeMap::new()) });
        Ok((FileSupervisorNarrowPort { owner: Rc::downgrade(&self.owner), grant: Rc::downgrade(&grant),
            scope, issuer: owner.host.journal_tail_identity() },
            FileSupervisorNarrowGrant { grant }))
    }
}

impl FileSupervisorNarrowPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner)
    }

    /// Inspect this exact selected intersection. Entry spends one query even if
    /// the current contract blocks execution. A plan reserves no operation key,
    /// replay work or authorization; current eligibility is never inferred.
    pub fn next(&self, operation: u64, retained_targets: Vec<ResolvedTarget>) -> FileNarrowPlanResult {
        if operation == 0 { return Err(FileNarrowError::MalformedRequest); }
        let retained_targets = canonical(&retained_targets)?;
        let Some(grant) = self.grant.upgrade() else { return Ok(withheld()); };
        let Some(owner) = self.owner.upgrade() else { return Ok(unknown()); };
        let Ok(owner) = owner.try_borrow() else { return Ok(unknown()); };
        if owner.host.storage_failure().is_some() || owner.host.actor_explanation_scope() != self.scope
            || !Weak::ptr_eq(&self.issuer, &owner.host.journal_tail_identity()) {
            return Ok(unknown());
        }
        let remaining = grant.queries.get();
        if remaining == 0 { return Err(FileNarrowError::Capacity); }
        let records = grant.records.try_borrow().map_err(|_| FileNarrowError::Unavailable)?;
        grant.queries.set(remaining - 1);
        let state = owner.host.inspect();
        let basis = basis(self.scope, &state);
        let selected = TargetCeiling::new(&retained_targets).map_err(|_| FileNarrowError::MalformedRequest)?;
        let effective_targets = state.control.ceiling.as_ref()
            .map_or_else(|| selected.clone(), |current| current.intersect(&selected)).targets();
        let block_reason = if records.contains_key(&operation)
            || owner.host.authority_narrowing_receipt(operation).is_ok() {
            Some(FileNarrowBlockReason::OperationAlreadyUsed)
        } else { blocked(&owner.host, &grant, &state) };
        Ok(Knowledge::Known { value: FileNarrowPlan { basis, operation, retained_targets,
            current_ceiling: state.control.ceiling.as_ref().map(TargetCeiling::targets),
            effective_targets, block_reason }, basis })
    }

    /// Spend on entry and call the original native transaction once. The shared
    /// cache preserves the original acknowledgment after unrelated progression.
    /// Pending/error entries never rerun; dropped custody or unhealthy storage
    /// suppress even cached receipts. Unsupported histories and stale cuts fail
    /// before key retention, replay, mutation allowance or snapshot consumption.
    pub fn narrow(&self, input: FileNarrowRequest) -> FileNarrowResult {
        let Some(grant) = self.grant.upgrade() else { return Ok(withheld()); };
        let Some(owner) = self.owner.upgrade() else { return Ok(unknown()); };
        let Ok(mut owner) = owner.try_borrow_mut() else { return Ok(unknown()); };
        if owner.host.storage_failure().is_some() || owner.host.actor_explanation_scope() != self.scope
            || !Weak::ptr_eq(&self.issuer, &owner.host.journal_tail_identity()) {
            return Ok(unknown());
        }
        let mut records = grant.records.try_borrow_mut().map_err(|_| FileNarrowError::Unavailable)?;
        if let Some(record) = records.get(&input.operation) {
            if record.request != input { return Err(FileNarrowError::IdempotencyConflict); }
            return record.result.clone().unwrap_or(Err(FileNarrowError::Unavailable));
        }
        if owner.host.authority_narrowing_receipt(input.operation).is_ok() {
            return Err(FileNarrowError::IdempotencyConflict);
        }
        let before = owner.host.inspect();
        let before_basis = basis(self.scope, &before);
        if input.expected != before_basis { return Err(FileNarrowError::StalePredecessor); }
        if let Some(reason) = blocked(&owner.host, &grant, &before) {
            return Err(match reason {
                FileNarrowBlockReason::ReplayLimit | FileNarrowBlockReason::OperationLimit => FileNarrowError::Capacity,
                _ => FileNarrowError::Unsupported,
            });
        }
        let native = FileAuthorityNarrowRequest::new(input.operation, before_basis.control_sequence,
            before_basis.authority_epoch, input.retained_targets.clone()).map_err(|_| FileNarrowError::MalformedRequest)?;
        grant.operations.set(grant.operations.get() - 1);
        records.insert(input.operation, Record { request: input.clone(), result: None });
        owner.snapshot = None;
        let result = owner.host.narrow_authority(before_basis.journal_revision, native.clone())
            .map_err(native_error).and_then(|receipt| {
                let after = owner.host.inspect();
                let after_basis = basis(self.scope, &after);
                let change = receipt.change();
                if receipt.request() != &native || change.scope != self.scope
                    || change.previous_sequence != before_basis.control_sequence
                    || change.sequence != after_basis.control_sequence
                    || change.authority_epoch != before_basis.authority_epoch
                    || change.previous_ceiling != before.control.ceiling
                    || before_basis.journal_revision.checked_add(1) != Some(after_basis.journal_revision)
                    || before_basis.control_sequence.checked_add(1) != Some(after_basis.control_sequence)
                    || after_basis.authority_epoch != before_basis.authority_epoch
                    || after_basis.recorded_at != before_basis.recorded_at
                    || after.control.ceiling.as_ref() != Some(&change.ceiling)
                    || after.control.ledger.charged != before.control.ledger.charged
                    || before.control.ledger.available.checked_add(change.refunded_units) != Some(after.control.ledger.available)
                    || before.control.ledger.reserved.checked_sub(change.refunded_units) != Some(after.control.ledger.reserved)
                    || after.executions != before.executions || after.payload != before.payload
                    || after.target != before.target || after.dispatcher_epoch != before.dispatcher_epoch
                    || after.control.suspended != before.control.suspended
                    || after.stop != before.stop {
                    return Err(FileNarrowError::Unavailable);
                }
                Ok(Knowledge::Known { value: FileNarrowReceipt { before: before_basis, after: after_basis,
                    operation: input.operation, retained_targets: input.retained_targets.clone(),
                    previous_ceiling: change.previous_ceiling.as_ref().map(TargetCeiling::targets),
                    effective_targets: change.ceiling.targets(), cancelled: change.cancelled.clone(),
                    refunded_units: change.refunded_units }, basis: after_basis })
            });
        records.get_mut(&input.operation).expect("entered narrowing").result = Some(result.clone());
        result
    }
}

fn basis(scope: Scope, state: &FileDeliverySnapshot) -> FileNarrowBasis {
    FileNarrowBasis { scope, journal_revision: state.revision, control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch, recorded_at: state.control.ledger.elapsed }
}
fn blocked(host: &FileOversight, grant: &Grant, state: &FileDeliverySnapshot) -> Option<FileNarrowBlockReason> {
    use FileNarrowBlockReason::*;
    if state.stop.is_some() { return Some(Stopped); }
    if state.control.suspended { return Some(Suspended); }
    if !host.authority_narrowing_supported() { return Some(UnsupportedHistory); }
    let events = usize::try_from(state.revision).ok().and_then(|n| n.checked_add(1));
    if events.is_none_or(|events| events > grant.max_replay_events) { return Some(ReplayLimit); }
    if grant.operations.get() == 0 { return Some(OperationLimit); }
    None
}
fn native_error(error: JournalError) -> FileNarrowError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileNarrowError::Capacity,
        _ => FileNarrowError::Unavailable,
    }
}
fn unknown<T>() -> Knowledge<T, FileNarrowBasis> { Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable } }
fn withheld<T>() -> Knowledge<T, FileNarrowBasis> { Knowledge::Withheld { authority_required: "supervisor_narrow" } }
