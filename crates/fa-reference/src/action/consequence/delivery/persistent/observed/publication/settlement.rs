//! Targeted settlement through the original full-input, two-key owner.
//! Only native cancellation or endpoint receipts can release retained resources.

use super::super::{BaseEvent, Event, FileOversight, JournalError};
use super::super::super::requests::{FileRequestDisposition, FileRequestStatus};
use crate::action::{ActionState, ElapsedTick};
use crate::Error;

impl FileOversight {
    /// Cancel or settle ONE durable request without stopping unrelated work.
    ///
    /// Before dispatch, cancel the original reservation. After dispatch, ask
    /// the original endpoint to seal the exact key and reconcile its receipt.
    /// A real execution wins: its publication remains visible and charged.
    /// Confirmed nonexecution releases the charge and prevents a delayed message
    /// from executing later. A status miss alone never releases anything.
    /// `request_resolution` exposes the original accepted endpoint outcome.
    ///
    /// The fresh trusted elapsed observation and Cancel/Seal events use the same
    /// canonical transaction as checked publication. Neither a partial clock
    /// advance nor a candidate refund escapes a failed transaction. Storage
    /// failure latches the owner unavailable, including ambiguous replacement.
    /// Every native source-admission, consistency and retention rule still runs.
    /// An interrupted evidence source stays interrupted: settlement requires
    /// neither fresh helper input, replacement approvals nor a credential grant.
    ///
    /// Refused and terminal requests return their historical status without a
    /// write, even with an old revision/tick. This does not observe the supplied
    /// time. A faulted owner refuses even historical retries. Irrecoverably
    /// unknown work stays charged and refuses; it is not a cancelled request.
    ///
    /// This supervisor API also works after exclusive reopen, using the durable
    /// request ID instead of reconstructing old automatic/human keys. No request
    /// is resent, fenced or re-reviewed here. The actor wire cancellation remains
    /// deliberately weaker and does not gain this supervisor operation.
    ///
    /// Both original events must fit ordinary journal admission. This does not
    /// promise progress when ordinary capacity or receipt retention is exhausted.
    /// The sole effect sink is the existing journal, not a remote-provider or
    /// independently visible filesystem transaction.
    pub fn cancel_and_resolve_request(
        &mut self,
        revision: u64,
        request: u64,
        observed_tick: ElapsedTick,
    ) -> Result<FileRequestStatus, JournalError> {
        // The original lookup checks storage health before consulting old RAM.
        let status = self.request_status(request)?;
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Ok(status);
        };
        let event = match stage {
            ActionState::Proposed
            | ActionState::Prepared
            | ActionState::Reviewing
            | ActionState::Authorized => BaseEvent::Cancel(attempt),
            ActionState::Dispatching | ActionState::Unknown => BaseEvent::Seal(attempt),
            ActionState::IrrecoverablyUnknown => return Err(Error::WrongState.into()),
            ActionState::Confirmed
            | ActionState::ConfirmedNotExecuted
            | ActionState::Denied
            | ActionState::Cancelled => return Ok(status),
        };
        self.commit_publication_cut(
            revision,
            &[Event::Core(BaseEvent::Time(observed_tick)), Event::Core(event)],
        )?;
        self.request_status(request)
    }
}
