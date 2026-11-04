//! Explicit durable disposition of an original unknown effect liability.
//! Abandonment never establishes nonexecution, refunds rights or advances a
//! stream. The original broker remains the sole authority on effect stages.

use super::{Event, FileDelivery, JournalError, Machine, Transition};
use crate::Error;
use crate::action::consequence::gate::ControlInspection;
use crate::action::{ActionState, FrozenAction, MAX_ATTEMPTS};

/// One trusted supervisor instruction for one original attempt. All fields are
/// immutable retry identity; the current journal revision is a separate write
/// precondition. An operation ID is local to this owner's abandonment history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileAbandonRequest {
    pub operation: u64,
    pub attempt: u64,
    pub expected_control_sequence: u64,
    pub expected_authority_epoch: u64,
}

impl FileAbandonRequest {
    pub(super) fn check(self) -> Result<(), Error> {
        if self.operation == 0 || self.attempt == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}

/// Acknowledged supervisor disposition, bound to the exact original action.
/// This is historical data, not an endpoint outcome or authority to release the
/// charge. The action's complete effect identity is retained without inventing
/// a new digest or decoding caller-asserted receipt data from the journal.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FileAbandonReceipt;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn refund(receipt: FileAbandonReceipt) -> EndpointReceipt { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAbandonReceipt {
    request: FileAbandonRequest,
    action: FrozenAction,
}

impl FileAbandonReceipt {
    pub fn request(&self) -> FileAbandonRequest {
        self.request
    }

    pub fn action(&self) -> &FrozenAction {
        &self.action
    }

    /// The original effect's full conservative charge remains a liability.
    pub fn charged_units(&self) -> u64 {
        self.action.spec().units
    }
}

/// Recomputed only from successfully applied original abandonment inputs.
/// At most one entry can exist per original attempt, hence MAX_ATTEMPTS bounds
/// this book. A replayed duplicate event is invalid, not a second disposition.
#[derive(Default)]
pub(super) struct Abandonments {
    history: Vec<FileAbandonReceipt>,
}

/// Preflight owns its receipt bytes before the original native transition. It
/// is private to the durable host family and cannot itself change authority.
pub(super) struct PreparedAbandonment(FileAbandonReceipt);

impl Abandonments {
    pub(super) fn receipt(&self, operation: u64) -> Option<&FileAbandonReceipt> {
        self.history
            .iter()
            .find(|receipt| receipt.request.operation == operation)
    }

    pub(super) fn retry(
        &self,
        request: &FileAbandonRequest,
    ) -> Result<Option<FileAbandonReceipt>, Error> {
        self.receipt(request.operation)
            .map(|receipt| {
                if receipt.request == *request {
                    Ok(receipt.clone())
                } else {
                    Err(Error::Binding)
                }
            })
            .transpose()
    }

    pub(super) fn prepare(
        &mut self,
        request: FileAbandonRequest,
        action: &FrozenAction,
        inspection: &ControlInspection,
    ) -> Result<PreparedAbandonment, Error> {
        self.check_new(request, inspection)?;
        self.history.try_reserve(1).map_err(|_| Error::Limit)?;
        Ok(PreparedAbandonment(FileAbandonReceipt {
            request,
            action: action.clone(),
        }))
    }

    /// Read-only admission for a restricted recovery finalizer. Exact retries
    /// retain their historical identity; a new request is checked against the
    /// same original stage and predecessor used by semantic replay.
    pub(super) fn preflight(
        &self,
        request: &FileAbandonRequest,
        inspection: &ControlInspection,
    ) -> Result<Option<FileAbandonReceipt>, Error> {
        if let Some(receipt) = self.retry(request)? {
            return Ok(Some(receipt));
        }
        self.check_new(*request, inspection)?;
        Ok(None)
    }

    fn check_new(
        &self,
        request: FileAbandonRequest,
        inspection: &ControlInspection,
    ) -> Result<(), Error> {
        request.check()?;
        if self.receipt(request.operation).is_some() {
            return Err(Error::Duplicate);
        }
        if inspection.sequence != request.expected_control_sequence
            || inspection.ledger.epoch != request.expected_authority_epoch
        {
            return Err(Error::Stale);
        }
        if inspection
            .ledger
            .stages
            .get(&request.attempt)
            .ok_or(Error::Missing)?
            != &ActionState::Unknown
        {
            return Err(Error::WrongState);
        }
        if self.history.len() >= MAX_ATTEMPTS {
            return Err(Error::Limit);
        }
        Ok(())
    }

    pub(super) fn record(&mut self, prepared: PreparedAbandonment) -> FileAbandonReceipt {
        let receipt = prepared.0;
        self.history.push(receipt.clone());
        receipt
    }
}

impl Machine {
    pub(super) fn apply_abandonment(
        &mut self,
        request: FileAbandonRequest,
    ) -> Result<Transition, Error> {
        let action = self.actions.get(&request.attempt).ok_or(Error::Missing)?;
        let prepared = self
            .abandonments
            .prepare(request, action, &self.broker.inspect())?;
        self.broker.abandon_unknown(request.attempt)?;
        // Only this owner's still-sendable handle is withdrawn. This is not an
        // endpoint seal, remote cancellation or evidence that a prior effect
        // did not run. Published payloads and original outcomes stay unchanged.
        self.envelopes.remove(&request.attempt);
        Ok(Transition::Abandoned(self.abandonments.record(prepared)))
    }
}

impl FileDelivery {
    /// Record an explicit trusted disposition of one original Unknown attempt.
    /// The original broker changes it to IrrecoverablyUnknown and retains its
    /// charge. No endpoint, clock, source, helper or human-review operation runs.
    /// It does not clear a pending stream, make a stopped domain drained or
    /// manufacture a nonexecution receipt. The retained local send handle for
    /// this attempt is withdrawn; all other requests remain untouched.
    ///
    /// A healthy owner and its CURRENT journal revision are required even for
    /// an exact retry. That retry returns the original receipt without another
    /// event, including after recovery advances the current authority epoch.
    /// Reusing its operation ID with changed arguments refuses. New requests
    /// must match the current control sequence/epoch and an Unknown stage.
    ///
    /// The new event uses ordinary journal capacity. Existing terminal recovery
    /// reserves retain their original Stop/Fence/StopProgress-only contract.
    /// Storage failure returns no candidate receipt and poisons the owner until
    /// exclusive recovery. The actual canonical history then decides whether
    /// the operation was recorded; an error never proves it did not commit.
    pub fn abandon_unknown(
        &mut self,
        revision: u64,
        request: FileAbandonRequest,
    ) -> Result<FileAbandonReceipt, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision() {
            return Err(Error::Stale.into());
        }
        if let Some(receipt) = self.machine.abandonments.retry(&request)? {
            return Ok(receipt);
        }
        match self.transact(revision, Event::AbandonUnknown(request))? {
            Transition::Abandoned(receipt) => Ok(receipt),
            _ => unreachable!("abandonment transition"),
        }
    }

    /// Original acknowledged liability disposition. A storage-faulted owner
    /// refuses to present its old in-memory cut as current evidence.
    pub fn abandonment_receipt(&self, operation: u64) -> Result<&FileAbandonReceipt, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        self.machine
            .abandonments
            .receipt(operation)
            .ok_or_else(|| Error::Missing.into())
    }
}
