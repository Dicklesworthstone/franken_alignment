use super::super::{FileActorPort, FileActorSupervisor, JournalError, UnknownReason};
use super::*;
use crate::Error;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::round::review::andon::MAX_WINDOWS;

impl FileActorSupervisor<FileOversight> {
    /// Independently select one immutable original window. Provisioning does
    /// not observe time, register the window, read label evidence or withdraw
    /// the actor's unused admission snapshot.
    pub fn andon_window_port(
        &self,
        budget: AndonBudget,
        queries: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorAndonWindowPort,
            FileSupervisorAndonWindowGrant,
        ),
        JournalError,
    > {
        self.andon_window_port_with_limits(budget, queries, max_replay_events, None)
    }

    /// Explicitly fund full native/learned history reconstruction for the sole
    /// new registration. Exact existing-window observations and cached retries
    /// perform no replay. This grants no action-predictor custody.
    pub fn andon_window_port_with_numerical_budget(
        &self,
        budget: AndonBudget,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorAndonWindowPort,
            FileSupervisorAndonWindowGrant,
        ),
        JournalError,
    > {
        self.andon_window_port_with_limits(
            budget,
            queries,
            max_replay_events,
            Some(numerical_replay_events),
        )
    }

    fn andon_window_port_with_limits(
        &self,
        budget: AndonBudget,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<
        (
            FileSupervisorAndonWindowPort,
            FileSupervisorAndonWindowGrant,
        ),
        JournalError,
    > {
        // Native zero allowance and zero price remain valid. Conflicts with
        // current windows are observed by Next and checked again at mutation.
        if budget.window == 0
            || budget.start >= budget.end
            || queries == 0
            || max_replay_events == 0
            || numerical_replay_events == Some(0)
        {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_ANDON_WINDOW_QUERIES
            || max_replay_events > MAX_ANDON_WINDOW_REPLAY_EVENTS
            || numerical_replay_events.is_some_and(|events| events > max_replay_events)
        {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let binding = owner.host.andon_binding()?.clone();
        let scope = owner.host.actor_explanation_scope();
        let grant = Rc::new(Grant {
            binding,
            budget,
            queries: Cell::new(queries),
            operations: Cell::new(1),
            max_replay_events,
            remaining_numerical_replay_events: numerical_replay_events.map(Cell::new),
            record: RefCell::new(None),
        });
        Ok((
            FileSupervisorAndonWindowPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope,
                issuer: owner.host.journal_tail_identity(),
            },
            FileSupervisorAndonWindowGrant { grant },
        ))
    }
}

impl FileSupervisorAndonWindowPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Spend one original query to inspect this exact budget against the
    /// current native owner. A blocked plan reserves no mutation or replay.
    pub fn next(&self, operation: u64) -> FileAndonWindowPlanResult {
        if operation == 0 {
            return Err(FileAndonWindowError::MalformedRequest);
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
            return Err(FileAndonWindowError::Capacity);
        }
        let record = grant
            .record
            .try_borrow()
            .map_err(|_| FileAndonWindowError::Unavailable)?;
        grant.queries.set(remaining - 1);
        let state = owner.host.inspect();
        let basis = basis(&owner.host, self.scope, &state)?;
        let already_registered = registered(&owner.host, grant.budget)?;
        let block_reason = if let Some(record) = record.as_ref() {
            Some(if record.request.operation == operation {
                FileAndonWindowBlockReason::OperationAlreadyUsed
            } else {
                FileAndonWindowBlockReason::OperationLimit
            })
        } else {
            blocked(&owner.host, &grant, &state, already_registered)?
        };
        Ok(Knowledge::Known {
            value: FileAndonWindowPlan {
                basis,
                operation,
                budget: grant.budget,
                already_registered,
                block_reason,
            },
            basis,
        })
    }

    /// Register only the budget independently fixed by this grant, through the
    /// ORIGINAL durable transition. Current accounting, labels and clock are
    /// never supplied by the request. A genuinely missing old occurrence window
    /// may be registered; an existing price/allowance can never be rewritten.
    ///
    /// Cheap binding/resource/native-admission refusals preserve the operation.
    /// Once entered, a failure or unwind spends it and cannot start replay again.
    /// Exact existing-window observations preserve the unused actor snapshot.
    pub fn register(&self, input: FileAndonWindowRequest) -> FileAndonWindowResult {
        FileAndonWindowRequest::new(input.operation, input.expected, input.expected_budget)?;
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
        // A plan from another independently provisioned grant cannot select
        // this grant's different window even when the owner cut is identical.
        // This is a predecessor assertion, never caller authority to set limits.
        if input.expected_budget != grant.budget {
            return Err(FileAndonWindowError::IdempotencyConflict);
        }
        let mut record = grant
            .record
            .try_borrow_mut()
            .map_err(|_| FileAndonWindowError::Unavailable)?;
        if let Some(record) = record.as_ref() {
            if record.request.operation != input.operation {
                return Err(FileAndonWindowError::Capacity);
            }
            if record.request != input {
                return Err(FileAndonWindowError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FileAndonWindowError::Unavailable));
        }
        let before = owner.host.inspect();
        let before_basis = basis(&owner.host, self.scope, &before)?;
        if input.expected != before_basis {
            return Err(FileAndonWindowError::StalePredecessor);
        }
        let already_registered = registered(&owner.host, grant.budget)?;
        if let Some(reason) = blocked(&owner.host, &grant, &before, already_registered)? {
            return Err(match reason {
                FileAndonWindowBlockReason::WindowLimit
                | FileAndonWindowBlockReason::NumericalReplayLimit
                | FileAndonWindowBlockReason::ReplayLimit
                | FileAndonWindowBlockReason::OperationLimit => FileAndonWindowError::Capacity,
                FileAndonWindowBlockReason::WindowConflict
                | FileAndonWindowBlockReason::OperationAlreadyUsed => {
                    FileAndonWindowError::IdempotencyConflict
                }
                _ => FileAndonWindowError::Unsupported,
            });
        }
        if !already_registered && owner.host.decoder_required() {
            let events = replay_events(&before).ok_or(FileAndonWindowError::Capacity)?;
            let allowance = grant
                .remaining_numerical_replay_events
                .as_ref()
                .ok_or(FileAndonWindowError::Unsupported)?;
            let remaining = allowance
                .get()
                .checked_sub(events)
                .ok_or(FileAndonWindowError::Capacity)?;
            allowance.set(remaining);
        }
        grant.operations.set(0);
        *record = Some(Record {
            request: input,
            result: None,
        });
        if !already_registered {
            owner.snapshot = None;
        }
        let clock_ready = owner.host.clock_ready();
        let result = owner
            .host
            .register_andon_window(before_basis.journal_revision, grant.budget)
            .map_err(native_error)
            .and_then(|changed| {
                let after = owner.host.inspect();
                let after_basis = basis(&owner.host, self.scope, &after)?;
                let expected_revision = before_basis
                    .journal_revision
                    .checked_add(u64::from(changed))
                    .ok_or(FileAndonWindowError::Unavailable)?;
                let mut expected_basis = before_basis;
                expected_basis.journal_revision = expected_revision;
                let mut expected_state = before.clone();
                expected_state.revision = expected_revision;
                if changed == already_registered
                    || after_basis != expected_basis
                    || after != expected_state
                    || owner.host.clock_ready() != clock_ready
                    || !registered(&owner.host, grant.budget)?
                {
                    return Err(FileAndonWindowError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FileAndonWindowReceipt {
                        before: before_basis,
                        after: after_basis,
                        operation: input.operation,
                        budget: grant.budget,
                        registered: changed,
                    },
                    basis: after_basis,
                })
            });
        record
            .as_mut()
            .expect("entered original Andon registration")
            .result = Some(result.clone());
        result
    }

    fn healthy(&self, host: &FileOversight, grant: &Grant) -> bool {
        host.storage_failure().is_none()
            && host.actor_explanation_scope() == self.scope
            && Weak::ptr_eq(&self.issuer, &host.journal_tail_identity())
            && host.andon_binding().ok() == Some(&grant.binding)
    }
}

fn basis(
    host: &FileOversight,
    scope: Scope,
    state: &FileDeliverySnapshot,
) -> Result<FileAndonWindowBasis, FileAndonWindowError> {
    if state.revision != host.revision() {
        return Err(FileAndonWindowError::Unavailable);
    }
    let (congress_generation, evaluation_revision) = host
        .credibility_promotion_identity()
        .map_err(native_error)?;
    Ok(FileAndonWindowBasis {
        scope,
        accounting: host.andon_binding().map_err(native_error)?.identity(),
        journal_revision: state.revision,
        control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch,
        evaluation_revision,
        congress_generation,
        policy_generation: host.current_policy().map_err(native_error)?.generation(),
        recorded_at: state.control.ledger.elapsed,
    })
}

fn registered(host: &FileOversight, budget: AndonBudget) -> Result<bool, FileAndonWindowError> {
    Ok(host
        .andon_ledger()
        .map_err(native_error)?
        .windows()
        .get(&budget.window)
        .is_some_and(|existing| existing.budget == budget))
}

/// Match the original bounded window laws without cloning a ledger containing
/// every historical confirmation. Native admission and mutation remain owned by
/// FileOversight; this preview cannot make the requested window authoritative.
fn blocked(
    host: &FileOversight,
    grant: &Grant,
    state: &FileDeliverySnapshot,
    already_registered: bool,
) -> Result<Option<FileAndonWindowBlockReason>, FileAndonWindowError> {
    use FileAndonWindowBlockReason::*;
    if grant.operations.get() == 0 {
        return Ok(Some(OperationLimit));
    }
    let windows = host.andon_ledger().map_err(native_error)?.windows();
    if let Some(existing) = windows.get(&grant.budget.window) {
        return Ok((existing.budget != grant.budget).then_some(WindowConflict));
    }
    if already_registered {
        return Err(FileAndonWindowError::Unavailable);
    }
    if windows.len() >= MAX_WINDOWS {
        return Ok(Some(WindowLimit));
    }
    if windows.values().any(|existing| {
        grant.budget.start < existing.budget.end && existing.budget.start < grant.budget.end
    }) {
        return Ok(Some(WindowOverlap));
    }
    // These gates apply only to a NEW event. The native exact-window fast path
    // is read-only even after a Fence or pause, and needs no numerical replay.
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
    if host.decoder_required()
        && grant
            .remaining_numerical_replay_events
            .as_ref()
            .is_none_or(|allowance| allowance.get() < events)
    {
        return Ok(Some(NumericalReplayLimit));
    }
    match host.andon_window_admission(grant.budget) {
        Ok(()) => {}
        Err(JournalError::Contract(Error::Limit | Error::Overflow)) => {
            return Ok(Some(ReplayLimit));
        }
        Err(JournalError::Contract(_)) => return Ok(Some(NativeAdmissionUnavailable)),
        Err(error) => return Err(native_error(error)),
    }
    Ok(None)
}

fn replay_events(state: &FileDeliverySnapshot) -> Option<usize> {
    usize::try_from(state.revision).ok()?.checked_add(1)
}
fn native_error(error: JournalError) -> FileAndonWindowError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileAndonWindowError::Capacity,
        JournalError::Contract(Error::Stale) => FileAndonWindowError::StalePredecessor,
        JournalError::Contract(Error::Binding) => FileAndonWindowError::IdempotencyConflict,
        JournalError::Contract(Error::InvalidInput) => FileAndonWindowError::MalformedRequest,
        JournalError::Contract(Error::WrongState | Error::Missing | Error::Incomplete) => {
            FileAndonWindowError::Unsupported
        }
        _ => FileAndonWindowError::Unavailable,
    }
}
fn unknown<T>() -> Knowledge<T, FileAndonWindowBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}
fn withheld<T>() -> Knowledge<T, FileAndonWindowBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_andon_window",
    }
}
