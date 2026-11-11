use super::super::{
    FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError, UnknownReason,
};
use super::*;
use crate::Error;
use crate::action::consequence::delivery::persistent::FileAbandonRequest;

impl FileActorSupervisor<FileOversight> {
    /// Grant finite plans and unknown-liability abandonment separately from actor
    /// intake, diagnosis, reconciliation, cancellation and whole-domain Stop.
    /// Provisioning reads no source, time, history or one-use admission snapshot.
    /// The default event allowance never admits numerical reconstruction.
    pub fn abandonment_port(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorAbandonmentPort,
            FileSupervisorAbandonmentGrant,
        ),
        JournalError,
    > {
        self.abandonment_port_with_limits(queries, operations, max_replay_events, None)
    }

    /// Explicitly permit numerical-history reconstruction with a separate shared
    /// cumulative reservation. Raw-frame action predictors require it too.
    /// Before each entered AbandonUnknown, reserve the complete acknowledged
    /// history plus the proposed disposition event. Warm replay cannot refund
    /// that allowance. Original recipes still enforce their numerical limits.
    ///
    /// These are event reservations, not products, CPU time or model quality.
    /// Next and exact cached retries spend no numerical or operation allowance.
    /// A short budget refuses before key retention, snapshot loss or replay.
    pub fn abandonment_port_with_numerical_budget(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorAbandonmentPort,
            FileSupervisorAbandonmentGrant,
        ),
        JournalError,
    > {
        self.abandonment_port_with_limits(
            queries,
            operations,
            max_replay_events,
            Some(numerical_replay_events),
        )
    }

    fn abandonment_port_with_limits(
        &self,
        queries: usize,
        operations: usize,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<
        (
            FileSupervisorAbandonmentPort,
            FileSupervisorAbandonmentGrant,
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
        if queries > MAX_ABANDONMENT_QUERIES
            || operations > MAX_ABANDONMENT_OPERATIONS
            || max_replay_events > MAX_ABANDONMENT_REPLAY_EVENTS
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
            FileSupervisorAbandonmentPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorAbandonmentGrant { grant },
        ))
    }
}

impl FileSupervisorAbandonmentPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Observe the original request and offer only its native Unknown
    /// disposition. A read reserves no operation, runs no replay and consumes
    /// no mutation allowance. Missing or unadmitted requests stay Unknown.
    /// Entered reads, including repeated and unavailable requests, spend once.
    pub fn next(&self, request: u64, operation: u64) -> FileAbandonmentPlanResult {
        if request == 0 || operation == 0 {
            return Err(FileAbandonmentError::MalformedRequest);
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
            return Err(FileAbandonmentError::Capacity);
        }
        grant.queries.set(remaining - 1);
        let Some(cut) = capture(&owner.host, self.scope, request) else {
            return Ok(unknown());
        };
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| FileAbandonmentError::Unavailable)?;
        let block_reason = if records.contains_key(&operation)
            || owner.host.abandonment_receipt(operation).is_ok()
        {
            Some(FileAbandonmentBlockReason::OperationAlreadyUsed)
        } else {
            blocked(&owner.host, &grant, &cut)
        };
        Ok(Knowledge::Known {
            value: FileAbandonmentPlan {
                basis: cut.basis,
                stage: cut.stage,
                operation,
                block_reason,
            },
            basis: cut.basis,
        })
    }

    /// Record only an original Unknown liability at the complete expected cut.
    /// The original native owner retains the charge and withdraws its local
    /// send handle; no endpoint outcome, source, clock or approval is obtained.
    ///
    /// Exact retries under this same live grant return the original cached
    /// result before checking remaining work or the current predecessor.
    /// Changed input under an entered key conflicts. Existing durable native
    /// operation keys cannot be adopted by a new live grant.
    ///
    /// Reserve numerical work, spend one operation, bind its full input and
    /// invalidate the admission snapshot BEFORE entering the original native
    /// transaction. Failure or unwind never returns that work or snapshot.
    /// Storage failure suppresses even cached results until independent recovery
    /// examines the canonical image.
    pub fn abandon(&self, input: FileAbandonmentRequest) -> FileAbandonmentResult {
        FileAbandonmentRequest::new(input.operation, input.expected)?;
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
            .map_err(|_| FileAbandonmentError::Unavailable)?;
        if let Some(record) = records.get(&input.operation) {
            if record.request != input {
                return Err(FileAbandonmentError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FileAbandonmentError::Unavailable));
        }
        // The underlying AbandonUnknown operation namespace survives recovery.
        // An old durable receipt is not a newly entered scoped operation.
        if owner.host.abandonment_receipt(input.operation).is_ok() {
            return Err(FileAbandonmentError::IdempotencyConflict);
        }
        let Some(before) = capture(&owner.host, self.scope, input.expected.request) else {
            return Ok(unknown());
        };
        if input.expected != before.basis {
            return Err(FileAbandonmentError::StalePredecessor);
        }
        if let Some(reason) = blocked(&owner.host, &grant, &before) {
            return Err(match reason {
                FileAbandonmentBlockReason::ReplayLimit
                | FileAbandonmentBlockReason::NumericalReplayLimit
                | FileAbandonmentBlockReason::OperationLimit => FileAbandonmentError::Capacity,
                FileAbandonmentBlockReason::NotUnknown
                | FileAbandonmentBlockReason::Terminal
                | FileAbandonmentBlockReason::IrrecoverablyUnknown
                | FileAbandonmentBlockReason::NumericalReplayUnsupported => {
                    FileAbandonmentError::Unsupported
                }
                FileAbandonmentBlockReason::OperationAlreadyUsed => {
                    FileAbandonmentError::IdempotencyConflict
                }
            });
        }
        if numerical(&owner.host) {
            let events = replay_events(&before).ok_or(FileAbandonmentError::Capacity)?;
            let allowance = grant
                .remaining_numerical_replay_events
                .as_ref()
                .ok_or(FileAbandonmentError::Unsupported)?;
            let remaining = allowance
                .get()
                .checked_sub(events)
                .ok_or(FileAbandonmentError::Capacity)?;
            allowance.set(remaining);
        }
        let native = FileAbandonRequest {
            operation: input.operation,
            attempt: before.basis.attempt,
            expected_control_sequence: before.basis.control_sequence,
            expected_authority_epoch: before.authority_epoch,
        };
        grant.operations.set(grant.operations.get() - 1);
        records.insert(
            input.operation,
            Record {
                request: input,
                result: None,
            },
        );
        // Same invalidation as a trusted host_mut borrow, including on unwind.
        owner.snapshot = None;
        let result = owner
            .host
            .abandon_unknown(before.basis.journal_revision, native)
            .map_err(native_error)
            .and_then(|receipt| {
                let after = capture(&owner.host, self.scope, before.basis.request)
                    .ok_or(FileAbandonmentError::Unavailable)?;
                if receipt.request() != native
                    || receipt.action().spec().scope != before.basis.scope
                    || receipt.charged_units() != before.action_units
                    || after.basis.scope != before.basis.scope
                    || after.basis.request != before.basis.request
                    || after.basis.attempt != before.basis.attempt
                    || after.basis.recorded_at != before.basis.recorded_at
                    || before.basis.journal_revision.checked_add(1)
                        != Some(after.basis.journal_revision)
                    || after.basis.request_generation < before.basis.request_generation
                    || after.basis.control_sequence < before.basis.control_sequence
                    || after.stage != ActionState::IrrecoverablyUnknown
                    || after.authority_epoch != before.authority_epoch
                    || after.action_units != before.action_units
                    || after.available != before.available
                    || after.reserved != before.reserved
                    || after.charged != before.charged
                {
                    return Err(FileAbandonmentError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FileAbandonmentReceipt {
                        before: before.basis,
                        after: after.basis,
                        operation: input.operation,
                        before_stage: before.stage,
                        charged_units: receipt.charged_units(),
                        stage: after.stage,
                    },
                    basis: after.basis,
                })
            });
        records
            .get_mut(&input.operation)
            .expect("entered abandonment")
            .result = Some(result.clone());
        result
    }
}

struct Cut {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    authority_epoch: u64,
    action_units: u64,
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
        authority_epoch: snapshot.control.ledger.epoch,
        action_units: action.units,
        available: snapshot.control.ledger.available,
        reserved: snapshot.control.ledger.reserved,
        charged: snapshot.control.ledger.charged,
    })
}

fn blocked(host: &FileOversight, grant: &Grant, cut: &Cut) -> Option<FileAbandonmentBlockReason> {
    use FileAbandonmentBlockReason::*;
    match cut.stage {
        ActionState::Proposed
        | ActionState::Prepared
        | ActionState::Reviewing
        | ActionState::Authorized
        | ActionState::Dispatching => return Some(NotUnknown),
        ActionState::Denied
        | ActionState::Cancelled
        | ActionState::Confirmed
        | ActionState::ConfirmedNotExecuted => return Some(Terminal),
        ActionState::IrrecoverablyUnknown => return Some(IrrecoverablyUnknown),
        ActionState::Unknown => {}
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

fn native_error(error: JournalError) -> FileAbandonmentError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileAbandonmentError::Capacity,
        _ => FileAbandonmentError::Unavailable,
    }
}

fn unknown<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}

fn withheld<T>() -> Knowledge<T, FileHoldDiagnosisBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_abandonment",
    }
}
