//! Sealed, immutable actor access to one verified learned stream receipt.
//! The original recovery finalizers own every historical and durable decision.
use super::{FileLearnedTextStreamActorPort, LearnedTextRelease};
use super::super::super::{JournalError, observed::{
    decoder::learned::FileLearnedRecovery,
    guarded::{learned_recovery::FileGuardedLearnedRecovery,
        mediated::{learned::FileMediatedLearnedRecovery,
            predictive::FilePredictiveMediatedLearnedRecovery},
        predictive::{learned::FilePredictiveLearnedRecovery, owned::FileOwnedPredictiveRecovery}},
}};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{
    actor::{ActorError, ActorOutcome, ActorProposal, Knowledge},
    actor_wire::{ActorRequestPort, backend},
};
use std::{fmt, rc::{Rc, Weak}};

struct Receipt {
    request: u64,
    proposal: ActorProposal,
    outcome: Knowledge<ActorOutcome>,
}

/// One original request's immutable historical observation. This port contains
/// no journal, Store lock, source, live supervisor, numerical owner or role.
/// Only consuming a fully checked recovery can construct it. A successful
/// Submit is an exact receipt lookup, never a new admission. Cancel is refused.
///
/// The observation stays at its recovered cut, including Pending and Unknown;
/// polling cannot acquire evidence, reconcile, or make that cut current again.
/// The caller must authenticate access and bound its service lifetime.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::receipt::FileLearnedReceiptActorPort;
/// fn escape(port: FileLearnedReceiptActorPort) { port.host_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::receipt::FileLearnedReceiptActorPort;
/// fn forge() { let _ = FileLearnedReceiptActorPort::new(); }
/// ```
pub struct FileLearnedReceiptActorPort { receipt: Rc<Receipt> }

/// A request-local handle bound to the exact recovered port, not just its key.
/// Retaining a ticket cannot retain a Store lock or publication capability.
#[derive(Clone)]
pub struct FileLearnedReceiptTicket { receipt: Weak<Receipt>, request: u64 }

impl fmt::Debug for FileLearnedReceiptActorPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedReceiptActorPort")
            .field("request", &self.receipt.request).finish_non_exhaustive()
    }
}
impl fmt::Debug for FileLearnedReceiptTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedReceiptTicket")
            .field("request", &self.request).finish_non_exhaustive()
    }
}
impl FileLearnedReceiptTicket { pub fn request(&self) -> u64 { self.request } }

impl FileLearnedReceiptActorPort {
    // There is deliberately no public constructor accepting an outcome. The
    // only production callers below have consumed an original typed finalizer.
    fn verified(request: u64, proposal: ActorProposal, outcome: Knowledge<ActorOutcome>) -> Self {
        Self { receipt: Rc::new(Receipt { request, proposal, outcome }) }
    }

    pub fn request(&self) -> u64 { self.receipt.request }

    pub fn submit(&self, request: u64, proposal: &ActorProposal)
        -> Result<FileLearnedReceiptTicket, ActorError>
    {
        if request == 0 { return Err(ActorError::MalformedProposal); }
        if request != self.receipt.request { return Err(ActorError::Withheld); }
        // Compare the ENTIRE original source-only envelope, including all
        // sentinel target fields, charge, kind, request bytes and old deadline.
        // No malformed envelope can be normalized into a matching receipt.
        if proposal != &self.receipt.proposal { return Err(ActorError::IdempotencyConflict); }
        Ok(FileLearnedReceiptTicket { receipt: Rc::downgrade(&self.receipt), request })
    }

    pub fn poll(&self, ticket: &FileLearnedReceiptTicket) -> Knowledge<ActorOutcome> {
        if ticket.request != self.receipt.request
            || !Weak::ptr_eq(&ticket.receipt, &Rc::downgrade(&self.receipt)) {
            return Knowledge::Withheld { authority_required: "own_request" };
        }
        self.receipt.outcome.clone()
    }

    pub fn cancel(&self, _ticket: &FileLearnedReceiptTicket) -> Result<(), ActorError> {
        // Even an unsent reservation belongs to the retained journal, not this
        // immutable receipt. No cancel/refund is fabricated from a saved view.
        Err(ActorError::Withheld)
    }
}
impl backend::Sealed for FileLearnedReceiptActorPort {}
impl ActorRequestPort for FileLearnedReceiptActorPort {
    type Ticket = FileLearnedReceiptTicket;
    fn submit(&self, request: u64, proposal: &ActorProposal) -> Result<Self::Ticket, ActorError> {
        Self::submit(self, request, proposal)
    }
    fn poll(&self, ticket: &Self::Ticket) -> Knowledge<ActorOutcome> { Self::poll(self, ticket) }
    fn cancel(&self, ticket: &Self::Ticket) -> Result<(), ActorError> { Self::cancel(self, ticket) }
}

// Only these concrete, independently checked cursor types receive this
// convenience composition. There is no generic callback/trait accepting caller
// observations and no unchecked conversion out of a guarded recovery wrapper.
macro_rules! learned_receipt_ports {
    ($($recovery:ty),+ $(,)?) => { $(
        impl $recovery {
            /// Consume the original verified receipt without a fence, cleanup,
            /// clock, reconciliation or live owner. The returned immutable port
            /// accepts only this key's exact original source-only Submit.
            /// None selects a message; Some selects its exact finish predecessor.
            pub fn finish_receipt_port(self, request: u64, finish_after: Option<u64>)
                -> Result<FileLearnedReceiptActorPort, JournalError>
            {
                let (original, release) = match finish_after {
                    None => (self.learned_text_stream_message_request(request)?, LearnedTextRelease::Message),
                    Some(after) => (self.learned_text_stream_finish_request(after, request)?, LearnedTextRelease::Finish),
                };
                // Freeze the envelope before consuming recovery; a serialization
                // refusal must never follow a successful durable mutation.
                let proposal = FileLearnedTextStreamActorPort::encode_release(request, release, original.deadline)
                    .map_err(|_| crate::Error::Binding)?;
                let outcome = self.finish_receipt(request, finish_after)?;
                Ok(FileLearnedReceiptActorPort::verified(request, proposal, outcome))
            }

            /// Explicitly perform the original fixed Fence/Time/Sweep before
            /// returning the immutable port. All guards, canonical rechecks,
            /// ordinary-capacity and unknown-liability rules remain those of
            /// finish_reconciled_receipt. The trusted clock runs only there.
            /// No role, journal owner or later reconciliation escapes the call.
            pub fn finish_reconciled_receipt_port<F>(self, request: u64,
                finish_after: Option<u64>, clock: F) -> Result<FileLearnedReceiptActorPort, JournalError>
            where F: FnOnce() -> ElapsedTick {
                let (original, release) = match finish_after {
                    None => (self.learned_text_stream_message_request(request)?, LearnedTextRelease::Message),
                    Some(after) => (self.learned_text_stream_finish_request(after, request)?, LearnedTextRelease::Finish),
                };
                let proposal = FileLearnedTextStreamActorPort::encode_release(request, release, original.deadline)
                    .map_err(|_| crate::Error::Binding)?;
                let outcome = self.finish_reconciled_receipt(request, finish_after, clock)?;
                Ok(FileLearnedReceiptActorPort::verified(request, proposal, outcome))
            }

            /// Preserve the SAME acknowledged receipt history separately from
            /// this immutable actor port. The original source-only envelope is
            /// encoded before any clock or mutation; custody never enters it.
            pub fn finish_reconciled_receipt_port_with_history<F>(self, request: u64,
                finish_after: Option<u64>, clock: F)
                -> Result<(FileLearnedReceiptActorPort,
                    crate::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt), JournalError>
            where F: FnOnce() -> ElapsedTick {
                let (original, release) = match finish_after {
                    None => (self.learned_text_stream_message_request(request)?, LearnedTextRelease::Message),
                    Some(after) => (self.learned_text_stream_finish_request(after, request)?, LearnedTextRelease::Finish),
                };
                let proposal = FileLearnedTextStreamActorPort::encode_release(request, release, original.deadline)
                    .map_err(|_| crate::Error::Binding)?;
                let receipt = self.finish_reconciled_receipt_with_history(request, finish_after, clock)?;
                let port = FileLearnedReceiptActorPort::verified(request, proposal, receipt.outcome().clone());
                Ok((port, receipt))
            }

            /// Require the exact retained source checkpoint before mutation,
            /// then expose only the original immutable port and operator custody.
            pub fn finish_reconciled_receipt_port_with_checkpoint<F>(self, request: u64,
                finish_after: Option<u64>,
                saved: &crate::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpoint,
                clock: F)
                -> Result<(FileLearnedReceiptActorPort,
                    crate::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt), JournalError>
            where F: FnOnce() -> ElapsedTick {
                let (original, release) = match finish_after {
                    None => (self.learned_text_stream_message_request(request)?, LearnedTextRelease::Message),
                    Some(after) => (self.learned_text_stream_finish_request(after, request)?, LearnedTextRelease::Finish),
                };
                let proposal = FileLearnedTextStreamActorPort::encode_release(request, release, original.deadline)
                    .map_err(|_| crate::Error::Binding)?;
                let receipt = self.finish_reconciled_receipt_with_checkpoint(request, finish_after, saved, clock)?;
                let port = FileLearnedReceiptActorPort::verified(request, proposal, receipt.outcome().clone());
                Ok((port, receipt))
            }
        }
    )+ };
}
learned_receipt_ports!(FileLearnedRecovery, FileGuardedLearnedRecovery,
    FilePredictiveLearnedRecovery, FileOwnedPredictiveRecovery,
    FileMediatedLearnedRecovery, FilePredictiveMediatedLearnedRecovery);

#[cfg(test)]
mod tests;
