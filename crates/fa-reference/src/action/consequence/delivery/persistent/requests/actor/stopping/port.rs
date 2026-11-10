use super::*;
use super::super::{
    FileActorPort, FileActorSupervisor, JournalError, UnknownReason,
};
use crate::action::consequence::delivery::StopRequest;
use crate::Error;

impl FileActorSupervisor<FileOversight> {
    /// Grant exactly one original admission Stop, with bounded historical work.
    /// The owner must already have its original terminal recovery reserve.
    /// Provisioning runs no native replay, reads no live source and samples no
    /// clock. Numerical history remains unsupported under this event-only grant.
    pub fn fence_port(
        &self,
        max_replay_events: usize,
    ) -> Result<(FileSupervisorFencePort, FileSupervisorFenceGrant), JournalError> {
        self.fence_port_with_limits(max_replay_events, None)
    }

    /// Explicitly reserve work for a decoder or action-predictor history.
    /// Before the first new Stop, the full acknowledged history plus that one
    /// event is charged. A warm replay may do less; no allowance is refunded.
    /// Original immutable numerical recipes enforce their own kernel limits.
    /// These event units do not measure CPU, memory or cancellation latency.
    pub fn fence_port_with_numerical_budget(
        &self,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<(FileSupervisorFencePort, FileSupervisorFenceGrant), JournalError> {
        self.fence_port_with_limits(max_replay_events, Some(numerical_replay_events))
    }

    fn fence_port_with_limits(
        &self,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<(FileSupervisorFencePort, FileSupervisorFenceGrant), JournalError> {
        if max_replay_events == 0 || numerical_replay_events == Some(0) {
            return Err(Error::InvalidInput.into());
        }
        if max_replay_events > MAX_FENCE_REPLAY_EVENTS
            || numerical_replay_events.is_some_and(|events| events > max_replay_events)
        {
            return Err(Error::Limit.into());
        }
        let owner = self.owner.try_borrow().map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        if owner.host.journal_capacity()?.reserve().is_none() {
            return Err(Error::Incomplete.into());
        }
        let grant = Rc::new(Grant {
            operations: Cell::new(1),
            max_replay_events,
            remaining_numerical_replay_events: numerical_replay_events.map(Cell::new),
            record: RefCell::new(None),
        });
        Ok((
            FileSupervisorFencePort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorFenceGrant { grant },
        ))
    }
}

impl FileSupervisorFencePort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Permanently close this original controller's admission. A complete
    /// predecessor is required, and the native transition owns all cancellation,
    /// revocation and retained liabilities. No endpoint drain is performed.
    ///
    /// Exact retries under the same healthy owner and live grant return their
    /// historical result without another event, snapshot loss or reservation.
    /// Revocation, owner loss and ambiguous storage suppress cached disclosure.
    /// A fresh grant cannot adopt an old result or append another Stop to an
    /// already-stopped owner. Entered failures and unwinds never restore work.
    pub fn fence(&self, input: FileFenceRequest) -> FileFenceResult {
        FileFenceRequest::new(input.operation, input.expected)?;
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
        let mut record = grant.record.try_borrow_mut()
            .map_err(|_| FileFenceError::Unavailable)?;
        if let Some(previous) = record.as_ref() {
            if previous.request.operation != input.operation {
                return Err(FileFenceError::Capacity);
            }
            if previous.request != input {
                return Err(FileFenceError::IdempotencyConflict);
            }
            return previous.result.clone().unwrap_or(Err(FileFenceError::Unavailable));
        }

        let before = capture(&owner.host, self.scope)?;
        if input.expected != before {
            return Err(FileFenceError::StalePredecessor);
        }
        // Only the original live grant can return its cached acknowledgment.
        // A new operation never records an idempotent duplicate native Stop.
        if owner.host.inspect().stop.is_some() {
            return Err(FileFenceError::Unsupported);
        }
        let events = usize::try_from(before.journal_revision).ok()
            .and_then(|events| events.checked_add(1))
            .ok_or(FileFenceError::Capacity)?;
        if events > grant.max_replay_events || grant.operations.get() == 0 {
            return Err(FileFenceError::Capacity);
        }
        let numerical = owner.host.decoder_required() || owner.host.action_consistency_required();
        let remaining_numerical = if numerical {
            let allowance = grant.remaining_numerical_replay_events.as_ref()
                .ok_or(FileFenceError::Unsupported)?;
            Some(allowance.get().checked_sub(events).ok_or(FileFenceError::Capacity)?)
        } else {
            None
        };
        // Encoding this bounded acknowledged history is not numerical replay.
        // The original transaction still checks the exact terminal byte/event
        // admission before it touches storage or applies the native Stop.
        if owner.host.journal_capacity().map_err(native_error)?.reserve().is_none() {
            return Err(FileFenceError::Unsupported);
        }

        let native = StopRequest {
            operation: input.operation,
            expected_control_sequence: before.control_sequence,
            expected_authority_epoch: before.authority_epoch,
        };
        if let Some(remaining) = remaining_numerical {
            grant.remaining_numerical_replay_events.as_ref()
                .expect("numerical reservation admitted").set(remaining);
        }
        grant.operations.set(0);
        *record = Some(Record { request: input, result: None });
        // Match the supervisor's privileged mutation boundary. No untagged
        // one-use admission observation survives native work, error or unwind.
        owner.snapshot = None;

        let result = owner.host.request_stop(before.journal_revision, native)
            .map_err(native_error)
            .and_then(|stop| {
                let after = capture(&owner.host, self.scope)?;
                let progress = owner.host.stop_progress().map_err(native_error)?;
                if before.scope != after.scope
                    || before.journal_revision.checked_add(1) != Some(after.journal_revision)
                    || before.authority_epoch.checked_add(1) != Some(after.authority_epoch)
                    || stop.request() != native
                    || stop.revocation_floor() != after.authority_epoch
                    || stop.control_sequence() <= before.control_sequence
                    || stop.control_sequence() > after.control_sequence
                    || progress.receipt != stop
                    || progress.dispatcher_epoch != stop.dispatcher_epoch()
                    || progress.endpoint_fenced
                    || progress.reserved_units != 0
                {
                    return Err(FileFenceError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FileFenceReceipt {
                        before,
                        after,
                        operation: input.operation,
                        stop,
                        progress,
                    },
                    basis: after,
                })
            });
        record.as_mut().expect("entered fence").result = Some(result.clone());
        result
    }
}

fn capture(host: &FileOversight, scope: Scope) -> Result<FileFenceBasis, FileFenceError> {
    let snapshot = host.inspect();
    if host.actor_explanation_scope() != scope
        || snapshot.revision != host.revision()
        || host.storage_failure().is_some()
    {
        return Err(FileFenceError::Unavailable);
    }
    Ok(FileFenceBasis {
        scope,
        journal_revision: snapshot.revision,
        control_sequence: snapshot.control.sequence,
        authority_epoch: snapshot.control.ledger.epoch,
    })
}

fn native_error(error: JournalError) -> FileFenceError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => FileFenceError::Capacity,
        _ => FileFenceError::Unavailable,
    }
}

fn unknown() -> FileFenceObservation {
    Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
}

fn withheld() -> FileFenceObservation {
    Knowledge::Withheld { authority_required: "supervisor_fence" }
}
