use super::super::{FileActorPort, FileActorSupervisor, JournalError, UnknownReason};
use super::*;
use crate::Error;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::action::consequence::delivery::persistent::observed::credibility::FileCredibilityUpdate;

impl FileActorSupervisor<FileOversight> {
    /// Delegate one deterministic helper-weight transition on this original
    /// owner. The native joint gate must already have been frozen at bootstrap.
    /// Pending or unqualified evidence does not prevent independent provisioning;
    /// Next can describe its actual refusal without granting a mutation.
    ///
    /// Provisioning does not read source, observe time, reconstruct history,
    /// evaluate weights, or consume an unused actor admission snapshot.
    pub fn credibility_promotion_port(
        &self,
        queries: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorCredibilityPromotionPort,
            FileSupervisorCredibilityPromotionGrant,
        ),
        JournalError,
    > {
        self.credibility_promotion_port_with_limits(queries, max_replay_events, None)
    }

    /// Explicitly fund native/learned history reconstruction. The sole entered
    /// mutation reserves the complete acknowledged history plus its proposed
    /// event. The original immutable numerical recipe still owns numerical
    /// ceilings; event units do not claim a CPU-time limit.
    ///
    /// Next and exact cached retries perform no numerical replay. Required
    /// action forecasts are excluded even when this allowance is selected.
    pub fn credibility_promotion_port_with_numerical_budget(
        &self,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorCredibilityPromotionPort,
            FileSupervisorCredibilityPromotionGrant,
        ),
        JournalError,
    > {
        self.credibility_promotion_port_with_limits(
            queries,
            max_replay_events,
            Some(numerical_replay_events),
        )
    }

    fn credibility_promotion_port_with_limits(
        &self,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<
        (
            FileSupervisorCredibilityPromotionPort,
            FileSupervisorCredibilityPromotionGrant,
        ),
        JournalError,
    > {
        if queries == 0 || max_replay_events == 0 || numerical_replay_events == Some(0) {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_CREDIBILITY_PROMOTION_QUERIES
            || max_replay_events > MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS
            || numerical_replay_events.is_some_and(|events| events > max_replay_events)
        {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let joint_policy = owner
            .host
            .joint_credibility_policy()?
            .ok_or(Error::Incomplete)?;
        let scope = owner.host.actor_explanation_scope();
        let grant = Rc::new(Grant {
            joint_policy,
            queries: Cell::new(queries),
            operations: Cell::new(1),
            max_replay_events,
            remaining_numerical_replay_events: numerical_replay_events.map(Cell::new),
            record: RefCell::new(None),
        });
        Ok((
            FileSupervisorCredibilityPromotionPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope,
                issuer: owner.host.journal_tail_identity(),
            },
            FileSupervisorCredibilityPromotionGrant { grant },
        ))
    }
}

impl FileSupervisorCredibilityPromotionPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Spend one query to inspect the original marginal and joint predicates.
    /// Labels/cases are not copied into the result. A blocked plan spends its
    /// query but preserves the independent operation and actor snapshot.
    pub fn next(&self, operation: u64) -> FileCredibilityPromotionPlanResult {
        if operation == 0 {
            return Err(FileCredibilityPromotionError::MalformedRequest);
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
        if !self.healthy(&owner.host, &grant) {
            return Ok(unknown());
        }
        let remaining = grant.queries.get();
        if remaining == 0 {
            return Err(FileCredibilityPromotionError::Capacity);
        }
        let record = grant
            .record
            .try_borrow()
            .map_err(|_| FileCredibilityPromotionError::Unavailable)?;
        grant.queries.set(remaining - 1);
        let state = owner.host.inspect();
        let basis = basis(&owner.host, self.scope, &state)?;
        let block_reason = if let Some(record) = record.as_ref() {
            Some(if record.request.operation == operation {
                FileCredibilityPromotionBlockReason::OperationAlreadyUsed
            } else {
                FileCredibilityPromotionBlockReason::OperationLimit
            })
        } else if let Some(reason) = blocked(&owner.host, &grant, &state, operation)? {
            Some(reason)
        } else {
            qualification(&owner.host, &grant, basis)?
        };
        Ok(Knowledge::Known {
            value: FileCredibilityPromotionPlan {
                basis,
                operation,
                block_reason,
            },
            basis,
        })
    }

    /// Enter the ORIGINAL durable operation under the complete expected cut.
    /// Original marginal qualification and joint replay run inside that native
    /// operation; the caller cannot substitute a plan or positive report.
    ///
    /// Cheap predecessor/resource refusals do not spend the operation. Once
    /// entered, even a refused qualification, I/O failure or unwind spends it.
    /// The entered sentinel precedes replay and prevents an uncertain retry.
    /// Exact results are historical and disclose only while original owner and
    /// independent grant custody remain healthy. No clock is refreshed here.
    pub fn promote(&self, input: FileCredibilityPromotionRequest) -> FileCredibilityPromotionResult {
        FileCredibilityPromotionRequest::new(input.operation, input.expected)?;
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        if input.expected.scope != self.scope {
            return Ok(withheld());
        }
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(mut owner) = owner.try_borrow_mut() else {
            return Ok(unknown());
        };
        if !self.healthy(&owner.host, &grant) {
            return Ok(unknown());
        }
        let mut record = grant
            .record
            .try_borrow_mut()
            .map_err(|_| FileCredibilityPromotionError::Unavailable)?;
        if let Some(record) = record.as_ref() {
            if record.request.operation != input.operation {
                return Err(FileCredibilityPromotionError::Capacity);
            }
            if record.request != input {
                return Err(FileCredibilityPromotionError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FileCredibilityPromotionError::Unavailable));
        }
        let before = owner.host.inspect();
        let before_basis = basis(&owner.host, self.scope, &before)?;
        if input.expected != before_basis {
            return Err(FileCredibilityPromotionError::StalePredecessor);
        }
        if let Some(reason) = blocked(&owner.host, &grant, &before, input.operation)? {
            return Err(match reason {
                FileCredibilityPromotionBlockReason::ReplayLimit
                | FileCredibilityPromotionBlockReason::NumericalReplayLimit
                | FileCredibilityPromotionBlockReason::EvaluationWorkLimit
                | FileCredibilityPromotionBlockReason::OperationLimit => {
                    FileCredibilityPromotionError::Capacity
                }
                FileCredibilityPromotionBlockReason::OperationAlreadyUsed => {
                    FileCredibilityPromotionError::IdempotencyConflict
                }
                _ => FileCredibilityPromotionError::Unsupported,
            });
        }
        if owner.host.decoder_required() {
            let events = replay_events(&before).ok_or(FileCredibilityPromotionError::Capacity)?;
            let allowance = grant
                .remaining_numerical_replay_events
                .as_ref()
                .ok_or(FileCredibilityPromotionError::Unsupported)?;
            let remaining = allowance
                .get()
                .checked_sub(events)
                .ok_or(FileCredibilityPromotionError::Capacity)?;
            allowance.set(remaining);
        }
        grant.operations.set(0);
        *record = Some(Record {
            request: input,
            result: None,
        });
        owner.snapshot = None;
        let update = FileCredibilityUpdate {
            operation: input.operation,
            expected_control_sequence: before_basis.control_sequence,
            expected_authority_epoch: before_basis.authority_epoch,
            expected_evaluation_revision: before_basis.evaluation_revision,
        };
        let result = owner
            .host
            .promote_credibility(before_basis.journal_revision, &update)
            .map_err(native_error)
            .and_then(|receipt| {
                let after = owner.host.inspect();
                let after_basis = basis(&owner.host, self.scope, &after)?;
                let joint = owner
                    .host
                    .joint_credibility_promotion(input.operation)
                    .map_err(native_error)?;
                let change = &receipt.change;
                if !receipt.report.qualified()
                    || receipt.report.revision != before_basis.evaluation_revision
                    || receipt.report.policy_generation != before_basis.policy_generation
                    || !joint.qualified()
                    || joint.policy != grant.joint_policy
                    || joint.evaluation_revision != before_basis.evaluation_revision
                    || joint.policy_generation != before_basis.policy_generation
                    || joint.baseline != change.previous
                    || joint.candidate != change.current
                    || change.previous.generation != before_basis.congress_generation
                    || change.current.generation != after_basis.congress_generation
                    || change.sequence != after_basis.control_sequence
                    || change.revocation_floor != after_basis.authority_epoch
                    || before_basis.journal_revision.checked_add(1)
                        != Some(after_basis.journal_revision)
                    || before_basis.control_sequence.checked_add(1)
                        != Some(after_basis.control_sequence)
                    || before_basis.authority_epoch.checked_add(1)
                        != Some(after_basis.authority_epoch)
                    || before_basis.evaluation_revision.checked_add(1)
                        != Some(after_basis.evaluation_revision)
                    || before_basis.congress_generation.checked_add(1)
                        != Some(after_basis.congress_generation)
                    || after_basis.policy_generation != before_basis.policy_generation
                    || after_basis.recorded_at != before_basis.recorded_at
                    || after.control.ledger.charged != before.control.ledger.charged
                    || before
                        .control
                        .ledger
                        .available
                        .checked_add(change.refunded_units)
                        != Some(after.control.ledger.available)
                    || before
                        .control
                        .ledger
                        .reserved
                        .checked_sub(change.refunded_units)
                        != Some(after.control.ledger.reserved)
                    || after.control.ceiling != before.control.ceiling
                    || after.executions != before.executions
                    || after.payload != before.payload
                    || after.target != before.target
                    || after.dispatcher_epoch != before.dispatcher_epoch
                    || after.control.suspended != before.control.suspended
                    || after.stop != before.stop
                {
                    return Err(FileCredibilityPromotionError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FileCredibilityPromotionReceipt {
                        before: before_basis,
                        after: after_basis,
                        operation: input.operation,
                        cancelled: change.cancelled.clone(),
                        refunded_units: change.refunded_units,
                    },
                    basis: after_basis,
                })
            });
        record
            .as_mut()
            .expect("entered original helper-weight promotion")
            .result = Some(result.clone());
        result
    }

    fn healthy(&self, host: &FileOversight, grant: &Grant) -> bool {
        host.storage_failure().is_none()
            && host.actor_explanation_scope() == self.scope
            && Weak::ptr_eq(&self.issuer, &host.journal_tail_identity())
            && host.joint_credibility_policy().ok() == Some(Some(grant.joint_policy))
    }
}

fn basis(
    host: &FileOversight,
    scope: Scope,
    state: &FileDeliverySnapshot,
) -> Result<FileCredibilityPromotionBasis, FileCredibilityPromotionError> {
    if state.revision != host.revision() {
        return Err(FileCredibilityPromotionError::Unavailable);
    }
    let (congress_generation, evaluation_revision) = host
        .credibility_promotion_identity()
        .map_err(native_error)?;
    Ok(FileCredibilityPromotionBasis {
        scope,
        journal_revision: state.revision,
        control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch,
        evaluation_revision,
        congress_generation,
        policy_generation: host.current_policy().map_err(native_error)?.generation(),
        recorded_at: state.control.ledger.elapsed,
    })
}

/// Cheap original preconditions only. Do not re-tally labels on a repeatedly
/// refused mutation: the one entered native operation owns that paid work.
fn blocked(
    host: &FileOversight,
    grant: &Grant,
    state: &FileDeliverySnapshot,
    operation: u64,
) -> Result<Option<FileCredibilityPromotionBlockReason>, FileCredibilityPromotionError> {
    use FileCredibilityPromotionBlockReason::*;
    if state.stop.is_some() {
        return Ok(Some(Stopped));
    }
    if state.control.suspended {
        return Ok(Some(Suspended));
    }
    if !host.clock_ready() {
        return Ok(Some(ClockUnavailable));
    }
    if host
        .file_source_status()
        .is_some_and(|source| source.interrupted)
    {
        return Ok(Some(SourceUnavailable));
    }
    if host.action_consistency_required()
        || (host.decoder_required() && grant.remaining_numerical_replay_events.is_none())
    {
        return Ok(Some(NumericalReplayUnsupported));
    }
    let Some(events) = replay_events(state) else {
        return Ok(Some(ReplayLimit));
    };
    if events > grant.max_replay_events {
        return Ok(Some(ReplayLimit));
    }
    if grant.operations.get() == 0 {
        return Ok(Some(OperationLimit));
    }
    if host.decoder_required()
        && grant
            .remaining_numerical_replay_events
            .as_ref()
            .is_none_or(|allowance| allowance.get() < events)
    {
        return Ok(Some(NumericalReplayLimit));
    }
    match host.credibility_promotion(operation) {
        Ok(_) => return Ok(Some(OperationAlreadyUsed)),
        Err(JournalError::Contract(Error::Missing)) => {}
        Err(error) => return Err(native_error(error)),
    }
    Ok(None)
}

/// Only Next invokes this preview, after spending its original finite query.
/// The actual Promote always invokes the native predicates afresh.
fn qualification(
    host: &FileOversight,
    grant: &Grant,
    basis: FileCredibilityPromotionBasis,
) -> Result<Option<FileCredibilityPromotionBlockReason>, FileCredibilityPromotionError> {
    use FileCredibilityPromotionBlockReason::*;
    let marginal = host.credibility_report().map_err(native_error)?;
    if marginal.revision != basis.evaluation_revision
        || marginal.policy_generation != basis.policy_generation
    {
        return Err(FileCredibilityPromotionError::Unavailable);
    }
    if !marginal.qualified() {
        return Ok(Some(MarginalUnqualified));
    }
    match host.joint_credibility_report(basis.evaluation_revision) {
        Ok(joint) => {
            if joint.policy != grant.joint_policy
                || joint.evaluation_revision != basis.evaluation_revision
                || joint.policy_generation != basis.policy_generation
                || joint.baseline.generation != basis.congress_generation
                || basis.congress_generation.checked_add(1) != Some(joint.candidate.generation)
            {
                return Err(FileCredibilityPromotionError::Unavailable);
            }
            Ok((!joint.qualified()).then_some(JointUnqualified))
        }
        Err(JournalError::Contract(Error::WrongState)) => Ok(Some(NoWeightChange)),
        Err(JournalError::Contract(Error::Incomplete)) => Ok(Some(JointUnqualified)),
        Err(JournalError::Contract(Error::Limit | Error::Overflow)) => {
            Ok(Some(EvaluationWorkLimit))
        }
        Err(error) => Err(native_error(error)),
    }
}

fn replay_events(state: &FileDeliverySnapshot) -> Option<usize> {
    usize::try_from(state.revision).ok()?.checked_add(1)
}

fn native_error(error: JournalError) -> FileCredibilityPromotionError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => {
            FileCredibilityPromotionError::Capacity
        }
        JournalError::Contract(Error::Stale) => FileCredibilityPromotionError::StalePredecessor,
        JournalError::Contract(Error::WrongState | Error::Missing | Error::Incomplete) => {
            FileCredibilityPromotionError::Unsupported
        }
        _ => FileCredibilityPromotionError::Unavailable,
    }
}

fn unknown<T>() -> Knowledge<T, FileCredibilityPromotionBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}

fn withheld<T>() -> Knowledge<T, FileCredibilityPromotionBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_credibility_promotion",
    }
}
