use super::super::{
    FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError, UnknownReason,
};
use super::*;
use crate::Error;

impl FileActorSupervisor<FileOversight> {
    /// Grant finite plans and one-request cancellation separately from actor
    /// intake, diagnosis, reconciliation and whole-domain Stop. Provisioning
    /// reads no source, time, history or one-use admission snapshot.
    /// The default event allowance never admits numerical reconstruction.
    pub fn cancellation_port(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorCancellationPort,
            FileSupervisorCancellationGrant,
        ),
        JournalError,
    > {
        self.cancellation_port_with_limits(queries, operations, max_replay_events, None)
    }

    /// Explicitly permit numerical-history reconstruction with a separate shared
    /// cumulative reservation. Raw-frame action predictors require it too.
    /// Before each entered Cancel, reserve the complete acknowledged history
    /// plus its proposed Cancel event. Warm replay cannot refund that allowance.
    /// Original immutable recipes still enforce their own numerical limits.
    ///
    /// These are event reservations, not products, CPU time or model quality.
    /// Next and exact cached retries spend no numerical or operation allowance.
    /// A short budget refuses before key retention, snapshot loss or replay.
    pub fn cancellation_port_with_numerical_budget(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorCancellationPort,
            FileSupervisorCancellationGrant,
        ),
        JournalError,
    > {
        self.cancellation_port_with_limits(
            queries,
            operations,
            max_replay_events,
            Some(numerical_replay_events),
        )
    }

    fn cancellation_port_with_limits(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<
        (
            FileSupervisorCancellationPort,
            FileSupervisorCancellationGrant,
        ),
        JournalError,
    > {
        if queries == 0
            || operations == 0
            || max_replay_events == 0
            || numerical_replay_events == Some(0)
        {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_CANCELLATION_QUERIES
            || operations > MAX_CANCELLATION_OPERATIONS
            || max_replay_events > MAX_CANCELLATION_REPLAY_EVENTS
        {
            return Err(Error::Limit.into());
        }
        let possible = operations
            .checked_mul(max_replay_events)
            .ok_or(Error::Overflow)?;
        if numerical_replay_events.is_some_and(|events| events > possible) {
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
            remaining_numerical_replay_events: numerical_replay_events.map(Cell::new),
            records: RefCell::new(BTreeMap::new()),
        });
        Ok((
            FileSupervisorCancellationPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorCancellationGrant { grant },
        ))
    }
}

impl FileSupervisorCancellationPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Observe the original request and offer only its native undispatched
    /// cancellation. A read reserves no operation, runs no replay and consumes
    /// no mutation allowance. Missing or unadmitted requests stay Unknown.
    /// Entered reads, including repeated and unavailable requests, spend once.
    pub fn next(&self, request: u64, operation: u64) -> FileCancellationPlanResult {
        if request == 0 || operation == 0 {
            return Err(FileCancellationError::MalformedRequest);
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
            return Err(FileCancellationError::Capacity);
        }
        grant.queries.set(remaining - 1);
        let Some(cut) = capture(&owner.host, self.scope, request) else {
            return Ok(unknown());
        };
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| FileCancellationError::Unavailable)?;
        let block_reason = if records.contains_key(&operation) {
            Some(FileCancellationBlockReason::OperationAlreadyUsed)
        } else {
            blocked(&owner.host, &grant, &cut)
        };
        Ok(Knowledge::Known {
            value: FileCancellationPlan {
                basis: cut.basis,
                stage: cut.stage,
                operation,
                block_reason,
            },
            basis: cut.basis,
        })
    }

    /// Cancel only an original undispatched request at the complete expected
    /// predecessor. No clock, source or approval is needed to withdraw work.
    /// A dispatched or Unknown effect cannot be cancelled or refunded here;
    /// terminal requests refuse a new operation instead of masquerading as a
    /// newly applied cancellation.
    ///
    /// Exact retries under this same live grant return the original cached
    /// result before checking remaining work or the current predecessor.
    /// Changed input under an entered key conflicts. A replacement grant or
    /// reopened owner cannot recover this process-local acknowledgment cache.
    ///
    /// Reserve numerical work, spend one operation, bind its full input and
    /// invalidate the admission snapshot BEFORE entering the original native
    /// cancellation. Failure or unwind never returns that work or snapshot.
    /// An interrupted cache entry cannot rerun; storage failure suppresses even
    /// cached results until independent recovery examines the canonical image.
    pub fn cancel(&self, input: FileCancellationRequest) -> FileCancellationResult {
        FileCancellationRequest::new(input.operation, input.expected)?;
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
            .map_err(|_| FileCancellationError::Unavailable)?;
        if let Some(record) = records.get(&input.operation) {
            if record.request != input {
                return Err(FileCancellationError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FileCancellationError::Unavailable));
        }
        let Some(before) = capture(&owner.host, self.scope, input.expected.request) else {
            return Ok(unknown());
        };
        if input.expected != before.basis {
            return Err(FileCancellationError::StalePredecessor);
        }
        if let Some(reason) = blocked(&owner.host, &grant, &before) {
            return Err(match reason {
                FileCancellationBlockReason::ReplayLimit
                | FileCancellationBlockReason::NumericalReplayLimit
                | FileCancellationBlockReason::OperationLimit => FileCancellationError::Capacity,
                FileCancellationBlockReason::AlreadyDispatched
                | FileCancellationBlockReason::Terminal
                | FileCancellationBlockReason::IrrecoverablyUnknown
                | FileCancellationBlockReason::NumericalReplayUnsupported => {
                    FileCancellationError::Unsupported
                }
                FileCancellationBlockReason::OperationAlreadyUsed => {
                    FileCancellationError::IdempotencyConflict
                }
            });
        }
        if numerical(&owner.host) {
            let events = replay_events(&before).ok_or(FileCancellationError::Capacity)?;
            let allowance = grant
                .remaining_numerical_replay_events
                .as_ref()
                .ok_or(FileCancellationError::Unsupported)?;
            let remaining = allowance
                .get()
                .checked_sub(events)
                .ok_or(FileCancellationError::Capacity)?;
            allowance.set(remaining);
        }
        grant.operations.set(grant.operations.get() - 1);
        records.insert(
            input.operation,
            Record {
                request: input,
                result: None,
            },
        );
        // Exactly the same invalidation as a trusted host_mut borrow. A panic
        // must not leave an untagged observation reusable for another action.
        owner.snapshot = None;
        let result = owner
            .host
            .cancel_request(before.basis.journal_revision, before.basis.request)
            .map_err(native_error)
            .and_then(|()| {
                let after = capture(&owner.host, self.scope, before.basis.request)
                    .ok_or(FileCancellationError::Unavailable)?;
                let refunded_units = before
                    .reserved
                    .checked_sub(after.reserved)
                    .ok_or(FileCancellationError::Unavailable)?;
                if after.basis.scope != before.basis.scope
                    || after.basis.request != before.basis.request
                    || after.basis.attempt != before.basis.attempt
                    || after.basis.recorded_at != before.basis.recorded_at
                    || before.basis.journal_revision.checked_add(1)
                        != Some(after.basis.journal_revision)
                    || after.basis.request_generation < before.basis.request_generation
                    || after.basis.control_sequence < before.basis.control_sequence
                    || after.stage != ActionState::Cancelled
                    || after.charged != before.charged
                    || after.available.checked_sub(before.available) != Some(refunded_units)
                {
                    return Err(FileCancellationError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FileCancellationReceipt {
                        before: before.basis,
                        after: after.basis,
                        operation: input.operation,
                        before_stage: before.stage,
                        refunded_units,
                        stage: after.stage,
                    },
                    basis: after.basis,
                })
            });
        records
            .get_mut(&input.operation)
            .expect("entered cancellation")
            .result = Some(result.clone());
        result
    }
}

struct Cut {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    available: u64,
    reserved: u64,
    charged: u64,
}

/// One acknowledged original request mapping under one exclusive owner cut.
/// Equal numeric direct-attempt IDs never substitute for external requests.
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
        available: snapshot.control.ledger.available,
        reserved: snapshot.control.ledger.reserved,
        charged: snapshot.control.ledger.charged,
    })
}

fn blocked(host: &FileOversight, grant: &Grant, cut: &Cut) -> Option<FileCancellationBlockReason> {
    use FileCancellationBlockReason::*;
    match cut.stage {
        ActionState::Dispatching | ActionState::Unknown => return Some(AlreadyDispatched),
        ActionState::Denied
        | ActionState::Cancelled
        | ActionState::Confirmed
        | ActionState::ConfirmedNotExecuted => return Some(Terminal),
        ActionState::IrrecoverablyUnknown => return Some(IrrecoverablyUnknown),
        ActionState::Proposed
        | ActionState::Prepared
        | ActionState::Reviewing
        | ActionState::Authorized => {}
    }
    let numerical = numerical(host);
    if numerical && grant.remaining_numerical_replay_events.is_none() {
        return Some(NumericalReplayUnsupported);
    }
    let Some(events) = replay_events(cut) else {
        return Some(ReplayLimit);
    };
    if events > grant.max_replay_events {
        return Some(ReplayLimit);
    }
    if grant.operations.get() == 0 {
        return Some(OperationLimit);
    }
    if numerical
        && grant
            .remaining_numerical_replay_events
            .as_ref()
            .is_none_or(|allowance| allowance.get() < events)
    {
        return Some(NumericalReplayLimit);
    }
    None
}

fn numerical(host: &FileOversight) -> bool {
    host.decoder_required() || host.action_consistency_required()
}

fn replay_events(cut: &Cut) -> Option<usize> {
    usize::try_from(cut.basis.journal_revision)
        .ok()?
        .checked_add(1)
}

fn native_error(error: JournalError) -> FileCancellationError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileCancellationError::Capacity,
        _ => FileCancellationError::Unavailable,
    }
}

fn unknown<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}

fn withheld<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_cancellation",
    }
}
