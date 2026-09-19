//! Durable external-request completion, using ORIGINAL two-key capabilities and
//! endpoint evidence. No replayed key, new outcome ledger or implicit resend.
use super::super::super::{FileOversight, FilePermit, FileHumanPermit};
use super::super::super::credential::FileCredentialPermit;
use super::super::super::publication::CheckedCompletion;
use super::super::super::super::JournalError;
use super::super::super::super::requests::FileRequestDisposition;
use crate::action::{ActionState, FrozenAction};
use crate::action::consequence::delivery::{EndpointOutcome, EndpointReceipt};
use crate::Error;
use std::rc::Rc;

impl FileOversight {
    /// Read accepted ORIGINAL endpoint evidence by durable external request ID,
    /// including after restart and later credential/policy revocation. This is
    /// historical supervisor data, never an actor-facing permit or a live source
    /// observation. No clock, helper, credential or approval is reconstructed.
    ///
    /// None means NO ACCEPTED RECEIPT. It does not mean the effect did not occur:
    /// an unreconciled publication, unknown dispatch or expired receipt-retention
    /// interval can all have no accepted receipt. Never refund or resend on None.
    /// Refused and pre-dispatch-cancelled requests also have no endpoint receipt;
    /// request_status preserves those distinct dispositions.
    ///
    /// A faulted owner refuses rather than return its old acknowledged projection
    /// as current after a possibly visible, unacknowledged replacement.
    pub fn request_resolution(&self, request: u64) -> Result<Option<EndpointOutcome>, JournalError> {
        let FileRequestDisposition::Admitted { attempt, stage } = self.request_status(request)?.disposition else {
            return Ok(None);
        };
        match stage {
            ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing
            | ActionState::Authorized | ActionState::Denied | ActionState::Cancelled => Ok(None),
            ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => {
                // Even an endpoint-visible execution remains unaccepted here
                // until the original broker's reconciliation transition commits.
                Ok(self.machine.broker.resolution(attempt)?.map(EndpointReceipt::outcome))
            }
            ActionState::Confirmed | ActionState::ConfirmedNotExecuted => {
                let outcome = self.machine.broker.resolution(attempt)?
                    .map(EndpointReceipt::outcome).ok_or(Error::Incomplete)?;
                let compatible = matches!((stage, outcome),
                    (ActionState::Confirmed, EndpointOutcome::Executed { .. })
                    | (ActionState::ConfirmedNotExecuted, EndpointOutcome::NotExecuted { .. }));
                if !compatible { return Err(Error::Binding.into()); }
                Ok(Some(outcome))
            }
        }
    }

    /// Complete an already authorized durable request through the original
    /// atomic two-key path, including mandatory credentials when configured.
    /// The external request, automatic key, human key and ENTIRE frozen action
    /// must identify the same attempt. A key for another request cannot publish
    /// this payload or borrow this request's retained receipt.
    ///
    /// Same-owner retries of a settled request return the ORIGINAL accepted
    /// outcome without another journal write or dispatch. This lookup precedes
    /// current revision, evidence, deadline and credential-generation checks:
    /// subsequent revocation or source loss cannot erase a historical execution.
    /// Key brands and exact action binding are still checked on every retry.
    /// Reopened owners reject all old keys; use request_resolution to read history.
    ///
    /// An in-flight/unknown attempt is NEVER resent. Reconcile it using the
    /// existing receipt-only recovery APIs, then query its durable request ID.
    /// Source-bound completion still requires independent native acquisitions;
    /// use complete_publication_from_source for that profile, then this same
    /// request_resolution lookup for its accepted result.
    pub fn complete_request_publication(
        &mut self,
        revision: u64,
        request: u64,
        completion: CheckedCompletion<'_>,
        credential: Option<&FileCredentialPermit>,
    ) -> Result<EndpointOutcome, JournalError> {
        let stage = self.check_request_completion(
            request, completion.automatic, completion.human, completion.action,
        )?;
        // A changed action inside the supplied committee view is a conflicting
        // retry too. Historical lookup does not inspect its other observation bytes.
        self.check_action(completion.automatic.attempt, completion.current.action())?;
        if matches!(stage, ActionState::Confirmed | ActionState::ConfirmedNotExecuted) {
            return self.request_resolution(request)?.ok_or_else(|| Error::Incomplete.into());
        }
        if stage != ActionState::Authorized {
            return Err(Error::WrongState.into());
        }
        let publication = match credential {
            Some(credential) => self.complete_credentialed_publication(revision, completion, credential)?,
            None => self.complete_checked_publication(revision, completion)?,
        };
        Ok(publication.outcome)
    }

    fn check_request_completion(
        &self,
        request: u64,
        automatic: &FilePermit,
        human: &FileHumanPermit,
        action: &FrozenAction,
    ) -> Result<ActionState, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &automatic.issuer)
            || !Rc::ptr_eq(&self.issuer, &human.issuer)
            || automatic.attempt != human.attempt
        { return Err(Error::Binding.into()); }
        let FileRequestDisposition::Admitted { attempt, stage } = self.request_status(request)?.disposition else {
            return Err(Error::WrongState.into());
        };
        if automatic.attempt != attempt { return Err(Error::Binding.into()); }
        self.check_action(attempt, action)?;
        Ok(stage)
    }
}
