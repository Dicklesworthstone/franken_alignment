use super::*;
use super::super::{
    FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError, UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::EndpointOutcome;

impl FileActorSupervisor<FileOversight> {
    /// Separately grant finite plans and receipt-reconciliation operations.
    /// No source, time, history or admission snapshot is read while provisioning.
    /// The event ceiling includes the next Reconcile record; numerical replay
    /// remains unsupported regardless of the numerical size of this ceiling.
    pub fn resolution_port(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
    ) -> Result<
        (FileSupervisorResolutionPort, FileSupervisorResolutionGrant),
        JournalError,
    > {
        if queries == 0 || operations == 0 || max_replay_events == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_RESOLUTION_QUERIES
            || operations > MAX_RESOLUTION_OPERATIONS
            || max_replay_events > MAX_RESOLUTION_REPLAY_EVENTS
        {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let grant = Rc::new(Grant {
            queries: Cell::new(queries),
            operations: Cell::new(operations),
            max_replay_events,
            records: RefCell::new(BTreeMap::new()),
        });
        Ok((
            FileSupervisorResolutionPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorResolutionGrant { grant },
        ))
    }
}

impl FileSupervisorResolutionPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Observe one exact request and offer at most one connected native action.
    /// The caller supplies its prospective nonzero operation number. This read
    /// reserves no key, runs no replay and consumes no mutation allowance.
    /// Entered reads, including missing requests and repeats, share one budget.
    pub fn next(&self, request: u64, operation: u64) -> FileResolutionPlanResult {
        if request == 0 || operation == 0 {
            return Err(FileResolutionError::MalformedRequest);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some() {
            return Ok(unknown());
        }
        let remaining = grant.queries.get();
        if remaining == 0 {
            return Err(FileResolutionError::Capacity);
        }
        grant.queries.set(remaining - 1);
        let Some(cut) = capture(&owner.host, self.scope, request) else {
            return Ok(unknown());
        };
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| FileResolutionError::Unavailable)?;
        let block_reason = if records.contains_key(&operation) {
            Some(FileResolutionBlockReason::OperationAlreadyUsed)
        } else {
            blocked(&owner.host, &grant, &cut)
        };
        Ok(Knowledge::Known {
            value: FileResolutionPlan {
                basis: cut.basis,
                stage: cut.stage,
                operation,
                block_reason,
            },
            basis: cut.basis,
        })
    }

    /// Reconcile the request's original endpoint receipt through its original
    /// healthy owner. No caller tick is accepted: the native clock must already
    /// be ready, and its acknowledged tick is used without a freshness claim.
    ///
    /// Exact retries require this same live grant and owner. They return the
    /// original acknowledgment or refusal without another charge or native call,
    /// even after later owner progress. They cannot recover a result after grant
    /// replacement or reopening. Every changed input under an entered key
    /// conflicts; a stale new operation is refused before work or snapshot loss.
    ///
    /// Before entering native work, spend one operation, retain its full input
    /// and invalidate the untagged one-use admission snapshot. Failure or unwind
    /// never returns that allowance or snapshot. An interrupted cache entry can
    /// never rerun. The original storage-fault latch still suppresses all results.
    pub fn resolve(&self, input: FileResolutionRequest) -> FileResolutionResult {
        FileResolutionRequest::new(input.operation, input.expected)?;
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(mut owner) = owner.try_borrow_mut() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some()
            || owner.host.actor_explanation_scope() != self.scope
        {
            return Ok(unknown());
        }
        let mut records = grant
            .records
            .try_borrow_mut()
            .map_err(|_| FileResolutionError::Unavailable)?;
        if let Some(record) = records.get(&input.operation) {
            if record.request != input {
                return Err(FileResolutionError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FileResolutionError::Unavailable));
        }
        let Some(before) = capture(&owner.host, self.scope, input.expected.request) else {
            return Ok(unknown());
        };
        if input.expected != before.basis {
            return Err(FileResolutionError::StalePredecessor);
        }
        if let Some(reason) = blocked(&owner.host, &grant, &before) {
            return Err(match reason {
                FileResolutionBlockReason::ReplayLimit
                | FileResolutionBlockReason::OperationLimit => FileResolutionError::Capacity,
                FileResolutionBlockReason::ClockUnavailable => FileResolutionError::Unavailable,
                FileResolutionBlockReason::NotDispatched
                | FileResolutionBlockReason::Terminal
                | FileResolutionBlockReason::IrrecoverablyUnknown
                | FileResolutionBlockReason::NumericalReplayUnsupported => {
                    FileResolutionError::Unsupported
                }
                FileResolutionBlockReason::OperationAlreadyUsed => {
                    FileResolutionError::IdempotencyConflict
                }
            });
        }
        grant.operations.set(grant.operations.get() - 1);
        records.insert(
            input.operation,
            Record {
                request: input,
                result: None,
            },
        );
        // Match FileActorSupervisor::host_mut: a mutation cannot leave an
        // untagged admission snapshot reusable at its successor or after panic.
        owner.snapshot = None;
        let result = owner.host.reconcile(
            before.basis.journal_revision,
            before.basis.attempt,
        ).map_err(native_error).and_then(|outcome| {
            let after = capture(&owner.host, self.scope, before.basis.request)
                .ok_or(FileResolutionError::Unavailable)?;
            if after.basis.scope != before.basis.scope
                || after.basis.request != before.basis.request
                || after.basis.attempt != before.basis.attempt
                || after.basis.recorded_at != before.basis.recorded_at
                || before.basis.journal_revision.checked_add(1)
                    != Some(after.basis.journal_revision)
                || after.basis.request_generation < before.basis.request_generation
                || after.basis.control_sequence < before.basis.control_sequence
                || after.stage != outcome_stage(outcome)
            {
                return Err(FileResolutionError::Unavailable);
            }
            Ok(Knowledge::Known {
                value: FileResolutionReceipt {
                    before: before.basis,
                    after: after.basis,
                    operation: input.operation,
                    outcome,
                    stage: after.stage,
                },
                basis: after.basis,
            })
        });
        records
            .get_mut(&input.operation)
            .expect("entered operation")
            .result = Some(result.clone());
        result
    }
}

struct Cut {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
}

/// No replay, disk access or partial-page search: all coordinates and lifecycle
/// facts come from the exact durable request mapping under the caller's one
/// owner borrow. Direct attempts with equal numeric IDs never substitute.
fn capture(host: &FileOversight, scope: Scope, request: u64) -> Option<Cut> {
    if host.actor_explanation_scope() != scope {
        return None;
    }
    let status = host.request_status(request).ok()?;
    let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
        return None;
    };
    let action = host.request_action(request).ok()?.spec();
    let snapshot = host.inspect();
    if status.request != request
        || attempt == 0
        || action.scope != scope
        || snapshot.control.ledger.stages.get(&attempt) != Some(&stage)
        || snapshot.revision != host.revision()
    {
        return None;
    }
    Some(Cut {
        basis: FileHoldDiagnosisBasis {
            scope,
            request,
            request_generation: status.generation,
            attempt,
            journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence,
            recorded_at: snapshot.control.ledger.elapsed,
        },
        stage,
    })
}

fn blocked(host: &FileOversight, grant: &Grant, cut: &Cut) -> Option<FileResolutionBlockReason> {
    use FileResolutionBlockReason::*;
    match cut.stage {
        ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing
        | ActionState::Authorized => return Some(NotDispatched),
        ActionState::Denied | ActionState::Cancelled | ActionState::Confirmed
        | ActionState::ConfirmedNotExecuted => return Some(Terminal),
        ActionState::IrrecoverablyUnknown => return Some(IrrecoverablyUnknown),
        ActionState::Dispatching | ActionState::Unknown => {}
    }
    if !host.clock_ready() {
        return Some(ClockUnavailable);
    }
    if host.decoder_required() || host.learned_action_consistency_required() {
        return Some(NumericalReplayUnsupported);
    }
    if cut.basis.journal_revision.checked_add(1)
        .is_none_or(|events| events > grant.max_replay_events as u64)
    {
        return Some(ReplayLimit);
    }
    if grant.operations.get() == 0 {
        return Some(OperationLimit);
    }
    None
}

fn outcome_stage(outcome: Reconciliation) -> ActionState {
    match outcome {
        Reconciliation::Resolved(EndpointOutcome::Executed { .. }) => ActionState::Confirmed,
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }) => {
            ActionState::ConfirmedNotExecuted
        }
        Reconciliation::AwaitingResolution | Reconciliation::RetentionExpired => ActionState::Unknown,
    }
}

fn native_error(error: JournalError) -> FileResolutionError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileResolutionError::Capacity,
        _ => FileResolutionError::Unavailable,
    }
}

fn unknown<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
}

fn withheld<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Withheld { authority_required: "supervisor_resolution" }
}
