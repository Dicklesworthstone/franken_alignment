//! Trusted irreversible disposition of an original unknown delivery.
//! This preserves its charge and uncertainty; it is never an endpoint receipt.
use super::super::abandonment::{FileAbandonReceipt, FileAbandonRequest};
use super::{BaseEvent, Event, FileOversight, JournalError, Transition};
use crate::Error;

impl FileOversight {
    /// Durably mark this original Unknown delivery irrecoverably unknown.
    /// Its charge, published bytes and execution count remain unchanged. The
    /// actor still sees Unknown, and stop progress still reports the liability.
    /// No fresh clock, helper input or human publication key is required.
    ///
    /// An exact operation retry returns its original receipt only from a healthy
    /// owner at the supplied current journal revision. It cannot append another
    /// event or bind the operation to another attempt or authority predecessor.
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
        if let Some(receipt) = self.machine.preflight_abandon_unknown(request)? {
            return Ok(receipt);
        }
        match self.transact(revision, Event::Core(BaseEvent::AbandonUnknown(request)))? {
            Transition::Abandoned(receipt) => Ok(receipt),
            _ => unreachable!("observed unknown abandonment transition"),
        }
    }

    /// The original durable disposition, not proof of execution or nonexecution.
    /// After ambiguous storage, reopen or inspect the canonical image instead.
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
